//! M-67 `MD-I-6` (sacred, architect-only) — СТОРОЖ journal-first на пути выдачи.
//!
//! # Классификация: это СТОРОЖ, а не RED. Названо честно
//!
//! Против сегодняшнего кода этот файл ЗЕЛЁНЫЙ, и выдавать его за RED нельзя
//! (`testing.md`: «оракул зелёный с первого запуска» — симптом, требующий стоп-проверки).
//! Замер, обосновывающий классификацию: ВСЕ публичные входы `gateway` принимают каталог
//! журнала параметром и читают через `journal::stream*` —
//! `snapshot`/`frames_since`/`frames_since_with_stats`/`replay`/`snapshot_from_checkpoint`
//! (`lib.rs:1770,1819,1869,1905,1997`) и `LiveReducer::resume`/`pump` (`:2994,:3107`).
//! Конструктора, принимающего события напрямую, в публичной поверхности нет ⇒ обходного
//! пути сегодня НЕ СУЩЕСТВУЕТ, и свойство выполняется тривиально.
//!
//! # Почему сторож всё равно обязателен
//!
//! `C-091` F-4 прав по существу: `MD-I-6` в редакции rev1 («значение стрима = значение
//! реплея») journal-first НЕ доказывает. Два редьюсера над ОДНИМ входом дают равные байты
//! независимо от того, состоялась ли запись, — это тавтология, а не оракул (тот же класс,
//! что `testing.md` «зависимый эталон»). Существующий `red_gateway_live_eq_replay.rs`
//! сравнивает live-хвост с реплеем ТОГО ЖЕ каталога и потому меряет согласованность
//! редьюсеров, а не источник данных.
//!
//! Риск, который вносит `M-67` задача «стрим ежетиковых глубин»: соблазн считать глубину
//! из потока venue в памяти и отдавать её в WS, а в журнал писать «заодно». Тогда live
//! перестаёт быть воспроизводимым, `DESIGN.md` §1 («каждая цифра выводится реплеем»)
//! нарушается молча, и Replay-режим кокпита показывает не то, что видел пользователь.
//!
//! Сторож фиксирует НЕГАТИВНЫЙ путь, которого в rev1 не было: **запись не состоялась ⇒
//! значения нет в выдаче.** Отказ записи моделируется прод-механизмом, а не мока́ми:
//! disk-guard `WriterConfig.min_free_bytes` (`journal/src/lib.rs:224-230`) — тот самый,
//! что останавливает сбор на проде.
//!
//! # Мутационный контроль (предъявляется прогоном, `testing.md`)
//!
//! Нейтрализация disk-guard (`min_free_bytes: u64::MAX` → `0`) делает запись успешной ⇒
//! `g1` падает: значение появляется в выдаче. Это доказывает, что `g1` привязан к ФАКТУ
//! записи, а не к форме ответа. Обратная сторона — `g2`: реализация «всегда отдавать
//! базовую серию» проходит `g1` тривиально и обязана падать на `g2`.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;
/// Объём «удержанной» сделки — намеренно велик, чтобы её появление в OHLCV нельзя было
/// списать на округление.
const WITHHELD_SIZE: f64 = 777.0;

fn cfg(min_free_bytes: u64) -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes,
        source: DataSource::OwnCapture,
        provenance: "M-67 MD-I-6 journal-first guard".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn book() -> (Vec<Level>, Vec<Level>) {
    let offs = [0.0005_f64, 0.005, 0.010];
    (
        offs.iter()
            .map(|o| Level {
                price: to_fixed(MID * (1.0 - o)),
                size: to_fixed(2.0),
            })
            .collect(),
        offs.iter()
            .map(|o| Level {
                price: to_fixed(MID * (1.0 + o)),
                size: to_fixed(2.0),
            })
            .collect(),
    )
}

fn withheld_trade() -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(MID + 500.0),
            size: to_fixed(WITHHELD_SIZE),
            side: Side::Buy,
            ts_exch_ms: T0 + 5_000,
        },
    )
}

/// Базовый журнал: 20 тактов книги и сделок. `min_free_bytes = 0` — запись разрешена.
fn baseline(dir: &std::path::Path) {
    let (bids, asks) = book();
    let mut j = Journal::open_with(dir, cfg(0)).expect("open_with");
    for i in 0..20i64 {
        let ts = T0 + i * 100;
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: bids.clone(),
                asks: asks.clone(),
                ts_exch_ms: ts,
            },
        ))
        .expect("append snapshot");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID),
                size: to_fixed(1.0),
                side: [Side::Buy, Side::Sell][(i % 2) as usize],
                ts_exch_ms: ts + 5,
            },
        ))
        .expect("append trade");
    }
    j.flush().expect("flush");
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
    }
}

fn total_volume(dir: &std::path::Path) -> i64 {
    let snap = gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST)
        .expect("snapshot");
    snap.series.ohlcv.iter().map(|r| r.volume).sum()
}

/// **G1 — НЕГАТИВНЫЙ путь, которого не было в rev1.** Запись ОТКАЗАНА disk-guard'ом ⇒
/// значения нет в выдаче. Реализация, отдающая значение из памяти в обход журнала, падает.
#[test]
fn md_i6_g1_value_whose_append_failed_never_reaches_the_stream() {
    let dir = tempfile::tempdir().expect("dir");
    baseline(dir.path());
    let before = total_volume(dir.path());

    // Отказ записи прод-механизмом: свободного места «не хватает» всегда.
    let mut j = Journal::open_with(dir.path(), cfg(u64::MAX)).expect("reopen");
    let err = j.append(withheld_trade());
    assert!(
        err.is_err(),
        "SETUP не состоялся: disk-guard не отказал, значит сценарий «запись не состоялась» \
         не смоделирован и проба тестирует не тот случай"
    );
    drop(j);

    let after = total_volume(dir.path());
    assert_eq!(
        after, before,
        "MD-I-6 нарушен: запись события ОТКАЗАНА (disk-guard), но выдача изменилась \
         {before} → {after}. Значит источник стрима — не журнал, и live невоспроизводим \
         реплеем (DESIGN.md §1)."
    );
}

/// **G2 — обратная сторона.** То же событие, записанное УСПЕШНО, обязано появиться в выдаче.
/// Реализация «всегда отдавать базовую серию» проходит G1 тривиально и падает здесь.
/// Эталон назван явно (`testing.md`: «ассерт „изменилось“ обязан называть, ОТ ЧЕГО»).
#[test]
fn md_i6_g2_the_same_value_appears_once_its_append_succeeds() {
    let dir = tempfile::tempdir().expect("dir");
    baseline(dir.path());
    let before = total_volume(dir.path());

    let mut j = Journal::open_with(dir.path(), cfg(0)).expect("reopen");
    j.append(withheld_trade()).expect("append must succeed");
    j.flush().expect("flush");
    drop(j);

    let after = total_volume(dir.path());
    assert_eq!(
        after - before,
        to_fixed(WITHHELD_SIZE),
        "G2: после УСПЕШНОЙ записи объём обязан вырасти ровно на {WITHHELD_SIZE} \
         (эталон — записанное событие, а не «что-нибудь изменилось»); было {before}, стало {after}"
    );
}

/// **G3.** Живой путь (`LiveReducer`), который `M-67` расширяет ежетиковой глубиной, обязан
/// подчиняться тому же свойству: тик после ОТКАЗАННОЙ записи не приносит кадров.
#[test]
fn md_i6_g3_live_path_obeys_the_same_rule() {
    let dir = tempfile::tempdir().expect("dir");
    let ckpt = tempfile::tempdir().expect("ckpt");
    baseline(dir.path());

    let (mut live, _) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .expect("resume");
    // Дренируем весь текущий backlog — дальше стрим стоит на хвосте.
    let _ = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, usize::MAX)
        .expect("pump drain");

    let mut j = Journal::open_with(dir.path(), cfg(u64::MAX)).expect("reopen");
    assert!(
        j.append(withheld_trade()).is_err(),
        "SETUP не состоялся: disk-guard не отказал"
    );
    drop(j);

    let (frames, _, _) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, usize::MAX)
        .expect("pump after refused append");
    assert!(
        frames.is_empty(),
        "MD-I-6 нарушен на ЖИВОМ пути: запись отказана, а стрим выдал {} кадр(ов). \
         Ежетиковая величина обязана СНАЧАЛА попасть в журнал и лишь потом читаться \
         стримом (M-67 §4.3, DESIGN.md §1).",
        frames.len()
    );
}
