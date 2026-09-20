//! RED OF-I-4 PER-PRICE FOOTPRINT BINS (sacred, architect-only) — M-17. Закрывает C-016 blocker.
//!
//! C-016 (critic): `docs/archive/verify_M-17.sh` (гейт M-17, сдан в архив по норме Р-2 —
//! на приёмке зеленел БЕЗ обещанных per-price footprint-bins). `footprint_delta`
//! (per-БАР скаляр) НЕ покрывает ПОЛНЫЙ footprint — матрицу `(price → {buy_vol, sell_vol, delta})`,
//! которую рисует custom-series фронта (M-19 Тир2). Без этого impl мог не отдать bins → false-green.
//!
//! Контракт (research-dev impl), вход = агрессорные сделки `(ts_ms, price, side, size)`:
//!   `research_cli::orderflow::footprint_bins(trades, timeframe_ms) -> Vec<FootprintBar>`
//!   `FootprintBar { time_s, bins: Vec<PriceBin> }`, `PriceBin { price, buy_vol, sell_vol, delta }`
//!   где `delta = buy_vol − sell_vol`, bins per УРОВНЮ ЦЕНЫ (разные цены = разные bins).
//!
//! Анти-плацебо: слияние всех цен в один bin → падает per-price-separation; swap buy↔sell → падает;
//! `delta ≠ buy−sell` → падает; buy/sell не разделены per цена → падает. Против отсутствия — compile-RED.

use contracts::Side;
use research_cli::orderflow::{footprint_bins, FootprintBar, PriceBin};

fn tr(ts: i64, price: i64, side: Side, size: i64) -> (i64, i64, Side, i64) {
    (ts, price, side, size)
}

/// Найти bin по цене в баре (порядок bins не фиксируем).
fn bin(bar: &FootprintBar, price: i64) -> Option<&PriceBin> {
    bar.bins.iter().find(|b| b.price == price)
}

/// (детерминизм)
#[test]
fn footprint_bins_is_deterministic() {
    let trades = vec![tr(100, 65000, Side::Buy, 5), tr(200, 65010, Side::Sell, 3)];
    assert_eq!(
        footprint_bins(&trades, 1000),
        footprint_bins(&trades, 1000),
        "footprint_bins недетерминирована"
    );
}

/// РАЗНЫЕ цены → РАЗНЫЕ bins (не слиты в один). Ловит impl, схлопывающий цену.
#[test]
fn distinct_prices_produce_distinct_bins() {
    let trades = vec![tr(100, 65000, Side::Buy, 5), tr(200, 65010, Side::Sell, 3)];
    let bars = footprint_bins(&trades, 1000);
    assert_eq!(bars.len(), 1, "один бакет таймфрейма → один FootprintBar");
    assert_eq!(
        bars[0].bins.len(),
        2,
        "две РАЗНЫЕ цены (65000, 65010) дали {} bins — цены слиты (footprint без per-price = не footprint)",
        bars[0].bins.len()
    );
    let b0 = bin(&bars[0], 65000).expect("bin 65000 отсутствует");
    let b1 = bin(&bars[0], 65010).expect("bin 65010 отсутствует");
    assert_eq!(
        (b0.buy_vol, b0.sell_vol, b0.delta),
        (5, 0, 5),
        "bin 65000: buy=5 sell=0 delta=5"
    );
    assert_eq!(
        (b1.buy_vol, b1.sell_vol, b1.delta),
        (0, 3, -3),
        "bin 65010: buy=0 sell=3 delta=-3"
    );
}

/// На ОДНОЙ цене buy и sell РАЗДЕЛЕНЫ; delta = buy − sell (не |buy|+|sell|, стороны не перепутаны).
#[test]
fn same_price_separates_buy_and_sell() {
    let trades = vec![tr(100, 65000, Side::Buy, 5), tr(500, 65000, Side::Sell, 2)];
    let bars = footprint_bins(&trades, 1000);
    assert_eq!(bars[0].bins.len(), 1, "одна цена → один bin");
    let b = bin(&bars[0], 65000).unwrap();
    assert_eq!(
        (b.buy_vol, b.sell_vol, b.delta),
        (5, 2, 3),
        "bin 65000: buy=5, sell=2, delta=+3. Получено (buy={}, sell={}, delta={}) — стороны \
         перепутаны/не разделены или delta≠buy−sell",
        b.buy_vol,
        b.sell_vol,
        b.delta
    );
}

/// Bins принадлежат бакету таймфрейма; разные бакеты — раздельные бары.
#[test]
fn bins_are_bucketed_by_timeframe() {
    let trades = vec![tr(100, 65000, Side::Buy, 5), tr(1500, 65000, Side::Sell, 4)];
    let bars = footprint_bins(&trades, 1000);
    assert_eq!(bars.len(), 2, "2 бакета → 2 FootprintBar");
    assert_eq!(bars[0].time_s, 0);
    assert_eq!(bars[1].time_s, 1);
    assert_eq!(
        bin(&bars[0], 65000).unwrap().delta,
        5,
        "бар0: только buy 5 → delta +5"
    );
    assert_eq!(
        bin(&bars[1], 65000).unwrap().delta,
        -4,
        "бар1: только sell 4 → delta −4"
    );
}

/// (границы) пустой вход → пустой ряд.
#[test]
fn empty_trades_yield_no_bars() {
    let empty: Vec<(i64, i64, Side, i64)> = Vec::new();
    assert!(footprint_bins(&empty, 1000).is_empty());
}
