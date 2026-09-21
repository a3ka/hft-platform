//! RED M-88 (sacred, architect-only) — ФОРМА контракта обновления.
//!
//! COMPILE-RED по построению: полей `SeriesBundle.heatmap_observed_time_s` /
//! `SeriesBundle.cob_observed` и типа `gateway::ApplyOutcome` в коде ЕЩЁ НЕТ. Файл перестанет
//! падать компиляцией после задач 1 и 7 спеки `milestones/M-88-liquidity-removal-contract.md`.
//!
//! Зачем ОТДЕЛЬНЫЙ файл: предметный набор `red_m88_update_contract.rs` падает АССЕРТАМИ на
//! сегодняшнем коде — он доказывает существование дефекта и не смешивается с формой. Форма
//! проверяется здесь, и компиляционный отказ здесь не маскирует предметный результат там
//! (в Rust каждый файл `tests/` — отдельный бинарник).
//!
//! Предмет: «наблюдения не было» и «полный срез оказался пуст» обязаны быть РАЗЛИЧИМЫ
//! ПО ПРОВОДУ. Сегодня неразличимы (`merge_cob`: `if incoming.is_empty() { return existing }`,
//! `crates/gateway/src/lib.rs:2753`), и это делает честное правило склейки невыразимым.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{ApplyOutcome, Cursor, Selector, Snapshot};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_752_000_010_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvls(v: &[(f64, f64)]) -> Vec<Level> {
    v.iter()
        .map(|&(p, s)| Level {
            price: to_fixed(p),
            size: to_fixed(s),
        })
        .collect()
}

fn snapshot_ev(bids: &[(f64, f64)], asks: &[(f64, f64)], ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lvls(bids),
            asks: lvls(asks),
            ts_exch_ms: ts,
        },
    )
}

fn delta_ev(bids: &[(f64, f64)], asks: &[(f64, f64)], u0: u64, u1: u64, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: lvls(bids),
            asks: lvls(asks),
            first_update_id: u0,
            final_update_id: u1,
            prev_final_update_id: None,
            ts_exch_ms: ts,
        },
    )
}

fn trade_ev(price: f64, size: f64, side: Side, ts: i64) -> EventKind {
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

fn journal_of(events: Vec<EventKind>) -> (tempfile::TempDir, Vec<u64>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut seqs = Vec::new();
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            seqs.push(j.append(e).expect("append").seq);
        }
        j.flush().expect("flush");
    }
    (dir, seqs)
}

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn sel(bands: Vec<f64>) -> Selector {
    let w = bands.iter().copied().fold(0.0_f64, f64::max);
    if w > 0.0 {
        gateway::set_effective_heatmap_window_frac(w);
    }
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
        window_ms: None,
        depth_cadence_ms: None,
    }
}

/// Форма провода объявлена и поднята: смена правила склейки есть смена формы (`VB-I-4`).
#[test]
fn form_schema_version_is_bumped_to_11() {
    assert_eq!(
        gateway::GATEWAY_SCHEMA_VERSION,
        11,
        "смена контракта обновления = смена формы выдачи ⇒ бамп обязателен (VB-I-4)"
    );
}

/// Кадр, наблюдавший карту, ОБЪЯВЛЯЕТ наблюдённые бакеты. Без объявления правило
/// «заменить колонку» невыразимо: пустой срез неотличим от отсутствия наблюдения.
#[test]
fn form_frame_declares_observed_buckets() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        delta_ev(&[(64_990.0, 0.0)], &[], 1, 2, T + 1),
    ]);
    let (frames, _next) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::at(seqs[0]),
        usize::MAX,
    )
    .expect("frames");
    let f = frames.first().expect("кадр");

    assert!(
        f.delta.heatmap_observed_time_s.contains(&(T / 1000)),
        "кадр наблюдал бакет {} и обязан его объявить, объявлено: {:?}",
        T / 1000,
        f.delta.heatmap_observed_time_s
    );
    assert!(
        f.delta.cob_observed,
        "кадр нёс L2-событие ⇒ стакан наблюдался"
    );
    assert!(
        f.delta
            .heatmap_observed_time_s
            .windows(2)
            .all(|w| w[0] < w[1]),
        "список наблюдённых бакетов обязан быть строго возрастающим (детерминизм формы)"
    );
}

/// Кадр БЕЗ L2-событий не объявляет наблюдений вовсе — это `NoChange`, и он обязан
/// отличаться от «наблюдали пустоту».
#[test]
fn form_frame_without_book_events_declares_nothing() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        trade_ev(65_000.0, 1.0, Side::Buy, T + 10),
    ]);
    let (frames, _next) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::at(seqs[0]),
        usize::MAX,
    )
    .expect("frames");
    let f = frames.first().expect("кадр");

    assert!(
        f.delta.heatmap_observed_time_s.is_empty(),
        "кадр без L2-событий объявил наблюдение карты: {:?}",
        f.delta.heatmap_observed_time_s
    );
    assert!(!f.delta.cob_observed, "кадр без L2-событий объявил стакан");
}

/// Полный срез, ОКАЗАВШИЙСЯ ПУСТЫМ, объявляется как наблюдение. Это тот случай, ради
/// которого форма и меняется: иначе снятие последнего уровня неотличимо от молчания.
#[test]
fn form_empty_slice_is_still_an_observation() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        delta_ev(&[(64_990.0, 0.0)], &[(65_010.0, 0.0)], 1, 2, T + 1),
    ]);
    let (frames, _next) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::at(seqs[0]),
        usize::MAX,
    )
    .expect("frames");
    let f = frames.first().expect("кадр");

    assert!(
        f.delta.heatmap.is_empty(),
        "setup-страж: после снятия обоих уровней срез обязан быть пуст"
    );
    assert!(
        f.delta.heatmap_observed_time_s.contains(&(T / 1000)),
        "ПУСТОЙ полный срез — это наблюдение, и оно обязано быть объявлено"
    );
}

/// Исход применения кадра называется ЯВНО. Молчаливый no-op запрещён: его нечем наблюдать.
#[test]
fn form_apply_reports_outcome() {
    let _g = serial();
    let s = sel(vec![0.001]);
    let (dir, seqs) = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        trade_ev(65_000.0, 1.0, Side::Buy, T + 10),
    ]);
    let (frames, _next) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::at(seqs[0]),
        usize::MAX,
    )
    .expect("frames");
    let f = frames.first().expect("кадр").clone();

    let mut acc: Snapshot = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::at(seqs[0]),
    )
    .expect("snapshot");

    assert_eq!(
        acc.apply(&f),
        ApplyOutcome::Applied,
        "кадр, продолжающий курсор, обязан быть принят"
    );
    assert_eq!(
        acc.apply(&f),
        ApplyOutcome::OutOfOrder,
        "повторно применённый кадр обязан быть отвергнут ЯВНО, а не молча проигнорирован"
    );
}
