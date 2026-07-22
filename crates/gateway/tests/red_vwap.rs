//! RED M-20 VW-I-1..4 (sacred, architect-only) — session-anchored VWAP в gateway SeriesBundle.
//!
//! VWAP = Σ(price·size)/Σ(size) по сессии (якорь 00:00 UTC, VB-I-6), чистый Trade-редьюсер в gateway
//! `Reducer` (стримовый fold, bounded — GW-I-2). Новая серия `SeriesBundle.vwap: Vec<(time_s, vwap_e8)>`.
//!
//! COMPILE-RED сейчас: поле `snap.series.vwap` ещё НЕ существует → тест не компилируется (как M-17
//! `red_depth_series` против несуществующего `compute`). engine-dev добавляет поле + аккумулятор +
//! bump GATEWAY_SCHEMA_VERSION→3 → GREEN. Анти-плацебо: пустой `vwap: Vec::new()`-заглушка компилирует,
//! но `.last()` == None → падение; i64/f64-impl переполняется на VW-I-2; impl без session-сброса — на VW-I-3.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(venue: Venue, symbol: &str, price: f64, size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        venue,
        symbol,
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

/// Журнал из заданных сделок (порядок = seq).
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

fn sel(timeframe_ms: i64) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms,
        bands: vec![0.001],
    }
}

fn snap(dir: &std::path::Path, s: &Selector) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, s, Cursor::LATEST).expect("snapshot")
}

/// Последнее значение VWAP-серии (vwap_e8).
fn vwap_last(snap: &gateway::Snapshot) -> i64 {
    snap.series
        .vwap
        .last()
        .map(|&(_, v)| v)
        .expect("vwap-серия непуста")
}

#[test]
fn vwap_exact() {
    // VW-I-1: (100·2 + 200·1)/(2+1) = 400/3 = 133.333...; e8 = 4e18/3e8 = 13_333_333_333.
    let t = 1_752_000_010_000; // один UTC-день, timeframe 1000ms → один бакет
    let dir = journal_of(vec![
        trade(Venue::Binance, "BTCUSDT", 100.0, 2.0, Side::Buy, t),
        trade(Venue::Binance, "BTCUSDT", 200.0, 1.0, Side::Sell, t + 10),
    ]);
    let s = sel(1_000);
    let a = snap(dir.path(), &s);
    assert_eq!(
        vwap_last(&a),
        13_333_333_333,
        "VWAP = Σ(px·sz)/Σ(sz) = 400/3 ×1e8 = 13_333_333_333"
    );
    // Детерминизм: повторный прогон идентичен.
    let b = snap(dir.path(), &s);
    assert_eq!(a.series.vwap, b.series.vwap, "VWAP недетерминирован");
}

#[test]
fn vwap_i128_prod_scale() {
    // VW-I-2: BTC-масштаб — px 120000 (1.2e13) × sz 5 (5e8) = 6e21 переполняет i64 на ОДНОМ произведении.
    // VWAP = (120000·5 + 121000·5)/(5+5) = 1_205_000/10 = 120500 → e8 = 12_050_000_000_000.
    let t = 1_752_000_010_000;
    let dir = journal_of(vec![
        trade(Venue::Binance, "BTCUSDT", 120_000.0, 5.0, Side::Buy, t),
        trade(
            Venue::Binance,
            "BTCUSDT",
            121_000.0,
            5.0,
            Side::Sell,
            t + 10,
        ),
    ]);
    let a = snap(dir.path(), &sel(1_000));
    assert_eq!(
        vwap_last(&a),
        12_050_000_000_000,
        "i128-аккумуляция: VWAP=120500 ×1e8; i64-impl переполнился бы на px·sz=6e21"
    );
}

#[test]
fn vwap_session_reset() {
    // VW-I-3: сделки по разные стороны UTC-полуночи → аккумулятор СБРАСЫВАЕТСЯ.
    let midnight = 20_278 * DAY_MS; // = 1_752_019_200_000, граница UTC-дня
    let dir = journal_of(vec![
        // сессия D (день 20277): цена 100
        trade(
            Venue::Binance,
            "BTCUSDT",
            100.0,
            1.0,
            Side::Buy,
            midnight - 30_000,
        ),
        // сессия D+1 (день 20278): цена 200 — VWAP обязан СБРОСИТЬСЯ, не блендить со 100
        trade(
            Venue::Binance,
            "BTCUSDT",
            200.0,
            1.0,
            Side::Buy,
            midnight + 30_000,
        ),
    ]);
    let a = snap(dir.path(), &sel(60_000)); // 1-мин бакеты → пред/пост полуночи разные бакеты
    assert_eq!(
        vwap_last(&a),
        20_000_000_000,
        "VW-I-3: пост-полуночный VWAP = 200 ×1e8 (сессия сброшена); impl без сброса дал бы 150 (блендинг)"
    );
    // Пред-полуночный бакет обязан присутствовать со значением 100 (сессия D).
    assert!(
        a.series.vwap.iter().any(|&(_, v)| v == 10_000_000_000),
        "VW-I-3: бакет сессии D обязан нести VWAP=100 ×1e8"
    );
}

#[test]
fn vwap_per_venue() {
    // VW-I-4: тот же символ, разные venue → VWAP берёт ТОЛЬКО Selector.venue (Binance).
    let t = 1_752_000_010_000;
    let dir = journal_of(vec![
        trade(Venue::Binance, "BTCUSDT", 100.0, 1.0, Side::Buy, t),
        trade(Venue::Hyperliquid, "BTCUSDT", 999.0, 1.0, Side::Buy, t + 10),
    ]);
    let a = snap(dir.path(), &sel(1_000));
    assert_eq!(
        vwap_last(&a),
        10_000_000_000,
        "VW-I-4: VWAP = 100 ×1e8 (только Binance); чужой venue 999 не подмешивается"
    );
}
