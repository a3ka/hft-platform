//! RED M-36 VW-I-1..4 (sacred, architect-only) — **journal-cumulative (all-time) VWAP** в gateway SeriesBundle.
//!
//! ПЕРЕСМОТР M-20 (VB-I-6, founder-decision M-36): VWAP БОЛЬШЕ НЕ session-anchored. VWAP =
//! Σ(price·size)/Σ(size) по ВСЕМ сделкам от старта курсора, БЕЗ сброса на 00:00 UTC. `sum_pv/sum_v`
//! копятся по всему `journal::stream` (чистый Trade-редьюсер, bounded — GW-I-2). SVP/CVD остаются
//! session-anchored (не трогаются). Серия `SeriesBundle.vwap: Vec<(time_s, vwap_e8)>`.
//!
//! Анти-плацебо (impl-заглушки, которые эти тесты обязаны ЛОВИТЬ):
//!  • пустой `vwap: Vec::new()` → `.last()` == None → падение (VW-I-1);
//!  • i64/f64-аккумуляция → переполнение на BTC-масштабе (VW-I-2);
//!  • **impl с session-reset** (текущий M-20 код) → на кросс-полуночной сделке даёт 200 вместо
//!    блендированного 150 → падение (VW-I-3 all-time). Тест инвертирован против M-20.

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
        window_ms: None,
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
fn vwap_cumulative_across_midnight() {
    // VW-I-3 (M-36 all-time, ИНВЕРСИЯ M-20): сделки по разные стороны UTC-полуночи → аккумулятор
    // НЕ сбрасывается, VWAP БЛЕНДИТ через границу дня.
    let midnight = 20_278 * DAY_MS; // = 1_752_019_200_000, граница UTC-дня
    let dir = journal_of(vec![
        // день 20277: цена 100 @ size 1
        trade(
            Venue::Binance,
            "BTCUSDT",
            100.0,
            1.0,
            Side::Buy,
            midnight - 30_000,
        ),
        // день 20278: цена 200 @ size 1 — VWAP обязан БЛЕНДИТЬ со 100 (нет session-reset)
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
        15_000_000_000,
        "VW-I-3 all-time: пост-полуночный VWAP = (100·1+200·1)/(1+1) = 150 ×1e8 (кумулятив от старта); \
         session-reset impl дал бы 200 ×1e8 (20_000_000_000) — анти-плацебо"
    );
    // Пред-полуночный бакет обязан нести кумулятив на тот момент = 100 ×1e8.
    assert!(
        a.series.vwap.iter().any(|&(_, v)| v == 10_000_000_000),
        "VW-I-3 all-time: первый бакет обязан нести VWAP=100 ×1e8 (кумулятив после первой сделки)"
    );
    // Детерминизм: повторный прогон идентичен (VB-I-1).
    let b = snap(dir.path(), &sel(60_000));
    assert_eq!(
        a.series.vwap, b.series.vwap,
        "all-time VWAP недетерминирован"
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
