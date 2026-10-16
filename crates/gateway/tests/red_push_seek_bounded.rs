//! `TD-083` (P0) — RED-оракул на регресс: **push-цикл обязан делать SEEK, а не читать журнал
//! с головы на каждом тике**.
//!
//! ## Что случилось (найдено reviewer'ом `R-025` на sidecar-прогоне против ЖИВОГО прода)
//!
//! `frames_since` (`crates/gateway/src/lib.rs:1772`) открывает поток `journal::stream(dir,
//! filter)` — **от НАЧАЛА журнала** — и лишь потом отбрасывает всё до курсора. Фикс M-38b
//! (`GW-I-11`, сегментный skip) был применён к snapshot-пути
//! (`snapshot_from_checkpoint:1885` использует `journal::stream_from(..., ckpt.upto_seq)`),
//! но НЕ к live-push-пути.
//!
//! На проде это означало: до курсора надо промотать ≈139M событий при измеренной скорости
//! ≈190k событий/с ⇒ **≈12 минут на ОДИН тик**, который планируется каждые 250 ms. Вместе с
//! однопоточным рантаймом (`#[tokio::main(flavor = "current_thread")]`) и синхронным вызовом
//! внутри `tokio::select!` без `spawn_blocking` это давало:
//!
//! - `frames_received = 0` — live-push молчит навсегда, панель показывает застывший снапшот;
//! - accept-loop не исполняется ⇒ **следующий клиент не подключится вообще** (два
//!   connect-timeout подряд, в логе сервера тишина);
//! - таск не завершается при уходе клиента ⇒ сокеты копятся в `CLOSE_WAIT`, ядро горит на 100%;
//! - **healthcheck остаётся ЗЕЛЁНЫМ**: `</dev/tcp/127.0.0.1/8080` удовлетворяется ядром из
//!   listen-backlog, даже когда приложение никогда не вызывает `accept()`.
//!
//! Это дословно сценарий, о котором предупреждает `.claude/rules/gates.md` §8: «rollback ловит
//! падение healthcheck, но не тихую деградацию».
//!
//! ## Почему этого НЕ ловил ни один существующий оракул
//!
//! На журнале в сотни байт чтение «с головы» отрабатывает за микросекунды — поэтому
//! `red_ws_frames_converge_to_latest` (M-46 O-3) зелёный и остаётся зелёным. Дефект виден
//! ТОЛЬКО на прод-масштабе. `.claude/rules/testing.md` требует прод-масштабный кейс для
//! sacred-оракулов I/O-пути (урок TD-011, `crates/journal/tests/red_open_bounded.rs`) — у
//! push-пути такого оракула не было.
//!
//! ## Что меряет этот оракул
//!
//! **РАБОТУ, а не время** — сознательно (урок TD-078: оракул с потолком wall-clock
//! превращается в измеритель CI-машины). Единица работы — `ReadStats.segments_opened`:
//! чтение с головы открывает ВСЕ сегменты журнала, seek — единицы последних. Разница
//! структурная и от скорости машины не зависит.
//!
//! Требует API `gateway::frames_since_with_stats(..) -> (Vec<Frame>, Cursor, ReadStats)` —
//! симметрично `snapshot_from_checkpoint`, который уже возвращает честные `ReadStats`
//! «для §8 eyes-on». **Пока API нет, оракул не компилируется — это и есть RED.**

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, ReadStats, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const BASE_MS: i64 = 1_784_116_800_000;

/// Мелкий сегмент — чтобы гарантированно получить ИХ МНОГО, не раздувая фикстуру по времени.
/// Прод-масштаб здесь моделируется ЧИСЛОМ СЕГМЕНТОВ (на проде их 164), а не объёмом: именно
/// число сегментов отличает «прочитал всё» от «сделал seek».
const SEGMENT_BYTES: u64 = 4 * 1024;

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEGMENT_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "td-083".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        // Прод работает в windowed-режиме (GATEWAY_WINDOW_MS=60000) — держим тот же режим.
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

/// Журнал из МНОГИХ сегментов; возвращает (dir, число сегментов, последний seq).
fn journal_many_segments(events: i64) -> (tempfile::TempDir, usize, u64) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut last_seq = 0u64;
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..events {
            let ev = j
                .append(EventKind::md(
                    Venue::Binance,
                    "BTCUSDT",
                    MdPayload::Trade {
                        price: to_fixed(65_000.0 + (i % 50) as f64),
                        size: to_fixed(1.0),
                        side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                        ts_exch_ms: BASE_MS + i * 10,
                    },
                ))
                .expect("append");
            last_seq = ev.seq;
        }
        j.flush().expect("flush");
    }
    let segs = journal::list_segments(dir.path())
        .expect("list_segments")
        .len();
    (dir, segs, last_seq)
}

/// **TD-083 (главный).** Push-тик у ХВОСТА журнала обязан открывать ЕДИНИЦЫ сегментов,
/// а не весь журнал.
///
/// Анти-плацебо: на реализации через `journal::stream(dir, filter)` (чтение с головы)
/// `segments_opened` равно ПОЛНОМУ числу сегментов ⇒ тест падает. На реализации через
/// `journal::stream_from(dir, filter, after.upto_seq)` — единицы ⇒ проходит.
#[test]
fn td083_push_tick_seeks_instead_of_reading_from_head() {
    let (dir, total_segments, last_seq) = journal_many_segments(4_000);
    assert!(
        total_segments >= 8,
        "TD-083: фикстура дала {total_segments} сегмент(ов) — этого мало, чтобы отличить seek \
         от чтения с головы; тест ничего не доказывает"
    );

    // Курсор У САМОГО ХВОСТА — ровно позиция живого клиента, который уже получил снапшот
    // и дальше живёт кадрами. Именно этот сценарий заклинивал прод.
    let after = Cursor::at(last_seq.saturating_sub(1));

    let (_frames, _cursor, stats): (_, _, ReadStats) = gateway::frames_since_with_stats(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        after,
        256,
    )
    .expect("frames_since_with_stats");

    // Порог: seek обязан уложиться в ЕДИНИЦЫ сегментов независимо от длины журнала.
    // 3 = запас на границу сегмента (курсор может стоять в предпоследнем) + активный.
    assert!(
        stats.segments_opened <= 3,
        "TD-083 ВОСПРОИЗВЕДЁН: push-тик открыл {} сегментов из {} — журнал читается С ГОЛОВЫ \
         на каждом тике (250 ms). На проде это ≈12 минут на тик, live-push молчит навсегда, \
         accept-loop не исполняется, следующий клиент не подключится. Нужен \
         `journal::stream_from(dir, filter, after.upto_seq)` — как в snapshot-пути (GW-I-11).",
        stats.segments_opened,
        total_segments
    );
}

/// **TD-083 (масштабная инвариантность).** Стоимость тика НЕ растёт с длиной журнала.
///
/// Это то свойство, которого требует push-цикл с периодом 250 ms: его цена обязана зависеть
/// от ОБЪЁМА ПРИРАЩЕНИЯ, а не от накопленной истории. Один замер на одном журнале мог бы
/// пройти случайно; два журнала разной длины делают утверждение фальсифицируемым.
#[test]
fn td083_tick_cost_is_independent_of_journal_length() {
    let (small_dir, small_segs, small_last) = journal_many_segments(1_000);
    let (big_dir, big_segs, big_last) = journal_many_segments(6_000);
    assert!(
        big_segs >= small_segs * 3,
        "TD-083: журналы слишком похожи ({small_segs} vs {big_segs} сегментов) — \
         масштабная зависимость на них не проявится"
    );

    let read_tail = |dir: &std::path::Path, last: u64| -> u32 {
        let (_f, _c, st): (_, _, ReadStats) = gateway::frames_since_with_stats(
            dir,
            EpochFilter::OwnCaptureOnly,
            &sel(),
            Cursor::at(last.saturating_sub(1)),
            256,
        )
        .expect("frames_since_with_stats");
        st.segments_opened
    };

    let small = read_tail(small_dir.path(), small_last);
    let big = read_tail(big_dir.path(), big_last);

    assert_eq!(
        small, big,
        "TD-083: тик у хвоста открыл {small} сегментов на коротком журнале и {big} на длинном \
         ({small_segs} vs {big_segs} сегментов) — стоимость тика РАСТЁТ С ИСТОРИЕЙ. Push-цикл \
         обязан стоить по приращению, иначе сервис деградирует тем сильнее, чем дольше живёт."
    );
}
