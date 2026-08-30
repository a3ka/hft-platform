//! RED (sacred, architect-only) — **bump `GATEWAY_SCHEMA_VERSION` является ГЕЙТОМ.**
//!
//! **Файл намеренно версионно-АГНОСТИЧЕН по имени (M-48, C-032 R1).** Раньше он назывался
//! `red_gateway_schema_v7.rs` и пиннил 7. M-48 поднимает версию до 8 — и оракул, прибитый к
//! прошлой версии, превратился в блокер: engine-dev не мог провести bump, не тронув sacred-тест.
//! Это МОЙ процессный промах, второй раз подряд того же класса (reviewer предупреждал на M-38b:
//! «смена публичной сигнатуры без адаптации call-site'ов в том же RED-коммите вынуждает dev'а
//! править sacred-тесты»). Правило: при смене контракта architect обновляет ВСЕ свои оракулы и
//! verify-скрипты в ТОМ ЖЕ коммите, а имя файла не привязывается к номеру версии.
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

/// Текущая версия контракта провода. M-38a: 6→7 (session-reset CVD). **M-48: 7→8** —
/// `history_start_seq` + `history_truncated` (VB-I-11): поля аддитивны, но консюмер ОБЯЗАН
/// узнать, что история может быть усечённой, иначе продолжит считать её полной.
const EXPECTED_SCHEMA_VERSION: u32 = 8;

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
fn schema_version_constant_matches_expected() {
    assert_eq!(
        GATEWAY_SCHEMA_VERSION, EXPECTED_SCHEMA_VERSION,
        "GATEWAY_SCHEMA_VERSION обязан быть {EXPECTED_SCHEMA_VERSION} (M-48: смена формы провода — \
         history_start_seq/history_truncated, VB-I-11). Текущее {GATEWAY_SCHEMA_VERSION} → bump не сделан"
    );
}

/// (2) Snapshot несёт версию 7 (то, что уходит консюмеру в envelope через snapshot).
#[test]
fn snapshot_carries_expected_schema_version() {
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
        "Snapshot.schema_version обязан быть {EXPECTED_SCHEMA_VERSION}, а не {}",
        snap.schema_version
    );
}

/// (3) Frame из `frames_since` несёт версию 7 (live-push путь). frames обязаны быть НЕПУСТЫ,
/// иначе `all(==7)` вырождается в vacuous-true (анти-плацебо: проверяем, что кадры реально есть).
#[test]
fn frame_carries_expected_schema_version() {
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
        "каждый Frame.schema_version обязан быть {EXPECTED_SCHEMA_VERSION}; получено: {:?}",
        frames.iter().map(|f| f.schema_version).collect::<Vec<_>>()
    );
}
