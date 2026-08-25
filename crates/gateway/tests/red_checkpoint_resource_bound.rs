//! RED M-38b (sacred, architect-only) — **GW-I-11: read-путь ограничен ХВОСТОМ, а не историей.**
//!
//! Это второй (наряду с `red_checkpoint_byte_identity::foreign_checkpoint_changes_output`)
//! форсинг того, что чекпоинт РЕАЛЬНО используется. Реализация, которая тихо реплеит от START,
//! даёт правильные байты и падает ровно здесь — по счётчику.
//!
//! ## Как меряем (урок TD-040 — «гейт мерит инвариант, а не окружение»)
//!
//! **ДЕТЕРМИНИРОВАННЫЕ счётчики** `ReadStats { events_decoded, segments_opened }`, инкрементируемые
//! в `EventStream`, — НЕ аллокатор, НЕ wall-time, НЕ RSS. Аллокатор-оракул M-37 (TD-040) флакал:
//! глобальный allocator ловил чужие аллокации при параллельном прогоне тестов. Счётчик событий
//! зависит ТОЛЬКО от того, сколько журнала прочитано, и одинаков на любой машине и при любой
//! параллельности.
//!
//! COMPILE-RED: `gateway::ReadStats`, `gateway::snapshot_from_checkpoint`,
//! `gateway::checkpoint::advance_to` ещё не существуют.
//!
//! testing.md п.5 **прод-масштаб** — журнал из МНОГИХ сегментов, часть сжата в `.zst`
//! (на проде 96 `.zst` + 7 raw): скип обязан работать и по сжатым, и по границе raw↔zst,
//! а не только на одном маленьком сегменте.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;
/// Событий в фикстуре. Хвост после K — единицы, «история» — тысячи: разрыв на 2 порядка,
/// чтобы граница не зависела от мелких деталей реализации.
const N: u64 = 3_000;
/// Маленький сегмент → много сегментов (прод-профиль «много закрытых + активный»).
const SEG_BYTES: u64 = 32 * 1024;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(price: f64, size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
    }
}

/// Многосегментный журнал; часть закрытых сегментов сжата в `.zst` (граница raw↔zst попадает
/// в перебор). `keep_raw = 2` — последние два закрытых остаются raw, как на проде.
fn big_journal() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            let side = if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            };
            j.append(trade(
                100.0 + (i % 7) as f64,
                1.0 + (i % 3) as f64,
                side,
                D2_MS - (N as i64 * 100) + (i as i64 * 100),
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    dir
}

fn n_segments(dir: &std::path::Path) -> usize {
    journal::list_segments(dir).expect("segments").len()
}

// ─────────────────────────────────────────────────────────────────────────────
// GW-I-11 — снапшот из чекпоинта у хвоста НЕ декодирует историю
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn snapshot_from_tail_checkpoint_decodes_only_tail() {
    let dir = big_journal();
    let segs = n_segments(dir.path());
    assert!(
        segs >= 5,
        "фикстура обязана быть многосегментной (прод-профиль), получено {segs}"
    );

    let tail = 50_u64;
    let k = Cursor {
        upto_seq: Some(N - 1 - tail),
    };
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        k,
    )
    .expect("advance_to");

    let (_snap, stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("snapshot_from_checkpoint");

    // Допуск ×4 на хвост: реализация вправе дочитать сегмент, в котором лежит K, целиком
    // (внутрисегментный forward-скан — zstd не Seek). Но НЕ всю историю.
    let budget = tail * 4;
    assert!(
        stats.events_decoded <= budget,
        "GW-I-11 НАРУШЕН: чекпоинт стоит в {tail} событиях от конца, но декодировано \
         {} событий (бюджет {budget}, всего в журнале {N}). Это значит, что чекпоинт \
         НЕ используется — реализация реплеит историю, и TD-044 (409.74 s на проде) не вылечен.",
        stats.events_decoded
    );
    assert!(
        stats.events_decoded < N,
        "GW-I-11: декодировано {} из {N} — весь журнал. Тихий rebuild вместо чтения чекпоинта.",
        stats.events_decoded
    );
    // Сегментный скип: открывать все сегменты незачем, даже если события пропускаются дёшево.
    assert!(
        (stats.segments_opened as usize) < segs,
        "GW-I-11: открыто {} сегментов из {segs} — сегментный пропуск по first_seq не работает \
         (на проде это 96 .zst, каждый из которых надо распаковать)",
        stats.segments_opened
    );
}

/// Парный vantage (testing.md п.7): при K=START бюджета нет — обязан быть ПОЛНЫЙ реплей.
/// Заглушка «всегда возвращать events_decoded = 0» (или пустой ReadStats) падает здесь.
#[test]
fn without_checkpoint_full_replay_is_reported() {
    let dir = big_journal();
    let ckpt = tempfile::tempdir().expect("ckpt");
    // Чекпоинта нет вовсе → тихий rebuild от START (GW-I-9б) → декодируется всё.
    let (_snap, stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        Cursor::LATEST,
    )
    .expect("rebuild");
    assert_eq!(
        stats.events_decoded, N,
        "ReadStats обязан быть ЧЕСТНЫМ счётчиком: без чекпоинта декодируется весь журнал \
         ({N} событий), получено {}. Счётчик, который всегда мал, обесценивает оракул выше.",
        stats.events_decoded
    );
}

/// Чекпоинт у хвоста, но `at` — РАНЬШЕ хвоста и позже K: бюджет считается от фактического
/// расстояния K→at, а не «всегда мало». Граница (п.4) + защита от реализации, которая
/// обрезает чтение по фиксированному капу и молча теряет события.
#[test]
fn budget_scales_with_distance_not_constant() {
    let dir = big_journal();
    let k = Cursor {
        upto_seq: Some(N - 1 - 500),
    };
    let at = Cursor {
        upto_seq: Some(N - 1 - 100),
    };
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance_to(
        dir.path(),
        ckpt.path(),
        &sel(),
        EpochFilter::OwnCaptureOnly,
        k,
    )
    .expect("advance_to");

    let (snap, stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        ckpt.path(),
        at,
    )
    .expect("snapshot_from_checkpoint");

    assert_eq!(
        snap.cursor, at,
        "курсор снапшота обязан быть ровно запрошенным `at` (GW-I-8)"
    );
    let want = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), at)
        .expect("snapshot(START, at)");
    assert_eq!(
        serde_json::to_vec(&snap).expect("ser"),
        serde_json::to_vec(&want).expect("ser"),
        "GW-I-9: досчёт до промежуточного `at` обязан совпасть с реплеем от START до того же `at`"
    );
    assert!(
        stats.events_decoded >= 400,
        "между K и at ровно 400 событий — их нельзя не декодировать (получено {}). \
         Реализация, обрезающая чтение по константе, теряет события молча.",
        stats.events_decoded
    );
    assert!(
        stats.events_decoded < N,
        "и при этом история до K декодироваться не должна (получено {} из {N})",
        stats.events_decoded
    );
}
