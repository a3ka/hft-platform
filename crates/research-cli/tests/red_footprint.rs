//! RED OF-I-2/3 TRADE-FLOW ЭКСПОРТ: footprint-дельта + cumulative delta (sacred, architect-only) — M-17.
//!
//! Order-flow (Фабио) trade-flow: агрессия сделок. Сторону агрессора мы УЖЕ пишем
//! (`MdPayload::Trade.side` = taker). Это ЭКСПОРТ-аналитики (unbounded значения для линейного/
//! гистограммного графика), НЕ `Signal`-trait (там bounded directional score). Phase A: без raw-дельт.
//!
//! Контракт (research-dev impl), вход = агрессорные сделки `(ts_ms, side, size)`:
//!   `research_cli::orderflow::footprint_delta(trades, timeframe_ms) -> Vec<(i64 bucket_s, i64 delta)>`
//!       delta бакета = Σ(size где side=Buy) − Σ(size где side=Sell)  [знаковая агрессия per бар];
//!   `research_cli::orderflow::cumulative_delta(trades, timeframe_ms) -> Vec<(i64 bucket_s, i64 cum)>`
//!       cum = НАКОПЛЕННАЯ знаковая агрессия до конца бакета (running, НЕ сброс per бакет).
//!
//! Анти-плацебо: swap buy↔sell → знак дельты переворачивается (падает); `|buy|+|sell|` вместо знаковой
//! → падает; cumulative со сбросом per-бакет (не running) → падает; «точка-на-сделку» → падает timeframe.

use contracts::Side;
use research_cli::orderflow::{cumulative_delta, footprint_delta};

fn tr(ts: i64, side: Side, size: i64) -> (i64, Side, i64) {
    (ts, side, size)
}

/// (OF-I-2 детерминизм)
#[test]
fn footprint_delta_is_deterministic() {
    let trades = vec![tr(100, Side::Buy, 5), tr(200, Side::Sell, 2)];
    assert_eq!(
        footprint_delta(&trades, 1000),
        footprint_delta(&trades, 1000),
        "footprint_delta недетерминирована"
    );
}

/// (OF-I-2) дельта бакета = ЗНАКОВАЯ (buy − sell), НЕ |buy|+|sell|; сторона агрессора не перепутана.
#[test]
fn footprint_delta_is_signed_buy_minus_sell() {
    // bucket [0,1000): buy 5+3=8, sell 2 → delta = 8 − 2 = +6.
    let trades = vec![
        tr(100, Side::Buy, 5),
        tr(300, Side::Sell, 2),
        tr(700, Side::Buy, 3),
    ];
    let fp = footprint_delta(&trades, 1000);
    assert_eq!(fp.len(), 1, "3 сделки в одном бакете 1000мс → 1 точка");
    assert_eq!(fp[0].0, 0, "bucket_start в секундах = 0");
    assert_eq!(
        fp[0].1, 6,
        "дельта = buy(8) − sell(2) = +6, получено {}. Либо сторона перепутана (buy↔sell), \
         либо |buy|+|sell|=10 вместо знаковой",
        fp[0].1
    );
}

/// (OF-I-2 asymmetry) перевес продаж → ОТРИЦАТЕЛЬНАЯ дельта (знак реально зависит от стороны).
#[test]
fn sell_heavy_bucket_is_negative() {
    let trades = vec![tr(100, Side::Buy, 1), tr(200, Side::Sell, 9)];
    let fp = footprint_delta(&trades, 1000);
    assert_eq!(
        fp[0].1, -8,
        "buy(1)−sell(9) = −8, получено {} — знак стороны неверен",
        fp[0].1
    );
}

/// (OF-I-3) cumulative delta — RUNNING накопление (не сброс per бакет).
#[test]
fn cumulative_delta_accumulates_across_buckets() {
    // bucket0 [0,1000): +6 (buy 8, sell 2); bucket1 [1000,2000): −4 (sell 4).
    let trades = vec![
        tr(100, Side::Buy, 8),
        tr(200, Side::Sell, 2),
        tr(1500, Side::Sell, 4),
    ];
    let cd = cumulative_delta(&trades, 1000);
    assert_eq!(cd.len(), 2, "2 бакета → 2 точки");
    assert_eq!(cd[0], (0, 6), "бакет0 cum = +6");
    assert_eq!(
        cd[1],
        (1, 2),
        "бакет1 cum = +6 + (−4) = +2 (RUNNING), получено {:?}. impl со сбросом per-бакет дал бы (1,−4)",
        cd[1]
    );
}

/// (OF-I-3) знак агрессии: только buy → монотонно растёт; только sell → монотонно падает.
#[test]
fn cumulative_sign_follows_aggressor() {
    let buys = vec![tr(100, Side::Buy, 3), tr(1100, Side::Buy, 2)];
    let cd = cumulative_delta(&buys, 1000);
    assert_eq!(cd[0].1, 3);
    assert_eq!(
        cd[1].1, 5,
        "только покупки → cum растёт 3→5; получено {}",
        cd[1].1
    );
    let sells = vec![tr(100, Side::Sell, 3), tr(1100, Side::Sell, 2)];
    let cd2 = cumulative_delta(&sells, 1000);
    assert_eq!(
        cd2[1].1, -5,
        "только продажи → cum падает до −5; получено {}",
        cd2[1].1
    );
}

/// (границы) пустой вход → пустой ряд.
#[test]
fn empty_trades_yield_empty() {
    let empty: Vec<(i64, Side, i64)> = Vec::new();
    assert!(footprint_delta(&empty, 1000).is_empty());
    assert!(cumulative_delta(&empty, 1000).is_empty());
}
