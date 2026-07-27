//! RED M-38a (sacred, architect-only) — GATEWAY_SCHEMA_VERSION bump 6→7 является ГЕЙТОМ.
//!
//! C-028 K1: `red_gateway_export_v2` проверяет только `snap.schema_version == GATEWAY_SCHEMA_VERSION`
//! (тавтология — зелёная при ЛЮБОМ значении константы). Она НЕ доказывает, что M-38a поднял версию
//! до 7. engine-dev мог бы реализовать per-session CVD и оставить публичную схему на v6 — named-гейт
//! Task 9 остался бы зелёным. Здесь версия ПРИБИТА к 7 явно, в трёх местах:
//!   (1) сама константа `GATEWAY_SCHEMA_VERSION == 7`;
//!   (2) `Snapshot.schema_version == 7` (то, что видит консюмер envelope через snapshot);
//!   (3) `Frame.schema_version == 7` из `frames_since` (live-push путь).
//!
//! Анти-плацебо: RUNTIME-RED против текущего кода (`GATEWAY_SCHEMA_VERSION = 6`) — все три assert'а
//! падают по ЗНАЧЕНИЮ (6 != 7). GREEN только после bump'а в Task 9. Форма v1-аддитивности и провенанс
//! остаются в `red_gateway_export_v2` как отдельная регрессия (они НЕ доказывают non-additive bump).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector, GATEWAY_SCHEMA_VERSION};
use journal::{EpochFilter, Journal, WriterConfig};

/// M-38a: версия схемы non-additively поднята до 7 (session-reset CVD + форма `cvd_session_base` Vec).
const EXPECTED_SCHEMA_VERSION: u32 = 7;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
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

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
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

/// (1) Сама константа обязана быть ровно 7.
#[test]
fn schema_version_constant_is_7() {
    assert_eq!(
        GATEWAY_SCHEMA_VERSION, EXPECTED_SCHEMA_VERSION,
        "M-38a обязан поднять GATEWAY_SCHEMA_VERSION до 7 (non-additive: session-reset CVD + \
         cvd_session_base скаляр→Vec). Текущее значение {GATEWAY_SCHEMA_VERSION} != 7 → bump не сделан"
    );
}

/// (2) Snapshot несёт версию 7 (то, что уходит консюмеру в envelope через snapshot).
#[test]
fn snapshot_schema_version_is_7() {
    let t0 = 20_278_i64 * 86_400_000;
    let dir = journal_of(vec![
        trade(100.0, 3.0, Side::Buy, t0 + 1_000),
        trade(100.0, 2.0, Side::Sell, t0 + 2_000),
    ]);
    let snap = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("snapshot");
    assert_eq!(
        snap.schema_version, EXPECTED_SCHEMA_VERSION,
        "Snapshot.schema_version обязан быть 7 после M-38a bump'а, а не {}",
        snap.schema_version
    );
}

/// (3) Frame из `frames_since` несёт версию 7 (live-push путь). frames обязаны быть НЕПУСТЫ,
/// иначе `all(==7)` вырождается в vacuous-true (анти-плацебо: проверяем, что кадры реально есть).
#[test]
fn frame_schema_version_is_7() {
    let t0 = 20_278_i64 * 86_400_000;
    let dir = journal_of(vec![
        trade(100.0, 3.0, Side::Buy, t0 + 1_000),
        trade(100.0, 2.0, Side::Sell, t0 + 2_000),
        trade(100.0, 1.0, Side::Buy, t0 + 3_000),
    ]);
    let (frames, _next) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::START,
        usize::MAX,
    )
    .expect("frames_since");
    assert!(
        !frames.is_empty(),
        "предусловие: frames_since(START..) обязан вернуть ≥1 кадр (иначе all(==7) vacuous)"
    );
    assert!(
        frames
            .iter()
            .all(|f| f.schema_version == EXPECTED_SCHEMA_VERSION),
        "каждый Frame.schema_version обязан быть 7 после M-38a bump'а; получено: {:?}",
        frames.iter().map(|f| f.schema_version).collect::<Vec<_>>()
    );
}
