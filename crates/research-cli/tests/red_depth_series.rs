//! RED OF-I-6 DEPTH TIME-SERIES per (side, band) (sacred, architect-only) — M-17. `docs/fa/ops.md` §3.
//!
//! Founder-требование: бэкенд отдаёт временной ряд суммарной глубины `depth_within(side, band)`
//! ОТДЕЛЬНО для BID и ASK на указанных полосах, по таймфреймам → линейный график (N линий:
//! BID/ASK × полосы). Phase A: вычислимо из `L2Snapshot` (уже пишем), БЕЗ raw book-дельт.
//!
//! Контракт (research-dev impl):
//!   `research_cli::depth_series::compute(snapshots: &[(i64 /*ts_ms*/, OrderBook)], side: Side,
//!        band_pct: f64, timeframe_ms: i64) -> Vec<(i64 /*bucket_time_s*/, i64 /*depth*/)>`
//!   - на каждый снапшот: `book.depth_within(side, band_pct)`;
//!   - бакетирование по `timeframe_ms` (bucket = ts_ms / timeframe_ms);
//!   - значение бакета = глубина ПОСЛЕДНЕГО снапшота в бакете (close-семантика, детерминир.);
//!   - `bucket_time_s` = начало бакета в СЕКУНДАХ (UDF UTCTimestamp).
//!
//! Анти-плацебо: impl, суммирующий BID+ASK → падает OF-I-6-asymmetry; impl с неверной полосой →
//! падает band-monotonicity; impl «точка-на-снапшот» (нет агрегации) → падает timeframe; impl «first/
//! mean» вместо last → падает last-semantics. Против отсутствия — compile-RED.

use book::OrderBook;
use contracts::{Level, Side};
use research_cli::depth_series::compute;

const UNIT: i64 = 100_000_000;
const MID: i64 = 65_000 * UNIT;
const B1: f64 = 0.001; // 0.1%
const B3: f64 = 0.003; // 0.3%
const B5: f64 = 0.005; // 0.5%

/// Книга с уровнями на заданных (pct, size-units) для каждой стороны. Цены: bid = mid·(1−p), ask = mid·(1+p).
fn book(bids: &[(f64, i64)], asks: &[(f64, i64)]) -> OrderBook {
    let mut b = OrderBook::new();
    let bl: Vec<Level> = bids
        .iter()
        .map(|&(p, s)| Level {
            price: (MID as f64 * (1.0 - p)) as i64,
            size: s * UNIT,
        })
        .collect();
    let al: Vec<Level> = asks
        .iter()
        .map(|&(p, s)| Level {
            price: (MID as f64 * (1.0 + p)) as i64,
            size: s * UNIT,
        })
        .collect();
    b.apply_snapshot(&bl, &al);
    b
}

/// АСИММЕТРИЧНАЯ книга: bid тяжелее ask (внутри 0.3% bid=5+3=8, ask=4+1=5 units) → BID ≠ ASK.
fn asym_book() -> OrderBook {
    book(
        &[(0.0005, 5), (0.002, 3), (0.004, 2)],
        &[(0.0005, 4), (0.002, 1), (0.004, 6)],
    )
}

/// (OF-I-6 детерминизм) одинаковый вход → одинаковый выход (чистый редьюсер).
#[test]
fn depth_series_is_deterministic() {
    let snaps = vec![(500, asym_book()), (1500, asym_book())];
    let a = compute(&snaps, Side::Buy, B3, 1000);
    let b = compute(&snaps, Side::Buy, B3, 1000);
    assert_eq!(
        a, b,
        "depth_series недетерминирован — ряд для графика обязан быть воспроизводим"
    );
}

/// (OF-I-6 asymmetry) BID и ASK — РАЗНЫЕ ряды (не суммированы, не перепутаны). Ловит impl bid+ask.
#[test]
fn bid_and_ask_are_separate_not_summed() {
    let snaps = vec![(0, asym_book())];
    let bid = compute(&snaps, Side::Buy, B3, 1000);
    let ask = compute(&snaps, Side::Sell, B3, 1000);
    assert_eq!(bid.len(), 1);
    assert_eq!(ask.len(), 1);
    assert_ne!(
        bid[0].1, ask[0].1,
        "BID и ASK глубина в 0.3% совпали на асимметричной книге (bid=8, ask=5 units) — стороны \
         суммированы/перепутаны. Для order-flow BID и ASK ОБЯЗАНЫ быть раздельными сериями"
    );
    // Привязка к доверенному примитиву: ряд == depth_within последнего снапшота.
    let bk = asym_book();
    assert_eq!(
        bid[0].1,
        bk.depth_within(Side::Buy, B3),
        "BID-серия ≠ book.depth_within(Buy,0.3%)"
    );
    assert_eq!(
        ask[0].1,
        bk.depth_within(Side::Sell, B3),
        "ASK-серия ≠ book.depth_within(Sell,0.3%)"
    );
}

/// (OF-I-6 band-monotonicity) глубже полоса ⊇ мельче → depth(0.5%) ≥ depth(0.3%) ≥ depth(0.1%).
/// Ловит impl с перепутанной/неверной полосой.
#[test]
fn deeper_band_includes_shallower() {
    let snaps = vec![(0, asym_book())];
    for side in [Side::Buy, Side::Sell] {
        let d1 = compute(&snaps, side, B1, 1000)[0].1;
        let d3 = compute(&snaps, side, B3, 1000)[0].1;
        let d5 = compute(&snaps, side, B5, 1000)[0].1;
        assert!(
            d5 >= d3 && d3 >= d1,
            "{side:?}: depth(0.5%)={d5} ≥ depth(0.3%)={d3} ≥ depth(0.1%)={d1} нарушено — полоса \
             считается неверно (глубокая полоса обязана включать мелкую)"
        );
        assert!(
            d5 > d1,
            "{side:?}: 0.5% и 0.1% дали одно число — полоса не различает глубину"
        );
    }
}

/// (OF-I-6 timeframe) N снапшотов в ОДНОМ бакете → ОДНА точка (значение = ПОСЛЕДНИЙ снапшот, close-
/// семантика); снапшоты в РАЗНЫХ бакетах → раздельные точки; time = начало бакета в СЕКУНДАХ.
#[test]
fn timeframe_bucketing_takes_last_per_bucket() {
    // bucket [0,1000): снап@100 (глубже) и снап@900 (мельче); bucket [1000,2000): снап@1200.
    let heavy = book(&[(0.0005, 9), (0.002, 9)], &[(0.0005, 9)]); // within 0.3% bid = 18
    let light = book(&[(0.0005, 1), (0.002, 1)], &[(0.0005, 1)]); // within 0.3% bid = 2
    let snaps = vec![(100, heavy), (900, light.clone()), (1200, light.clone())];
    let series = compute(&snaps, Side::Buy, B3, 1000);
    assert_eq!(
        series.len(),
        2,
        "3 снапшота в 2 бакетах таймфрейма 1000мс дали {} точек — агрегации по таймфрейму нет \
         (impl «точка-на-снапшот»)",
        series.len()
    );
    assert_eq!(
        series[0].0, 0,
        "первый бакет обязан начинаться в t=0с (bucket_start в секундах)"
    );
    assert_eq!(series[1].0, 1, "второй бакет [1000,2000)мс → t=1с");
    // Значение первого бакета = ПОСЛЕДНИЙ снапшот в нём (@900, light=2), НЕ первый (@100, heavy=18) и НЕ среднее.
    let light_bid = light.depth_within(Side::Buy, B3);
    assert_eq!(
        series[0].1, light_bid,
        "значение бакета [0,1000) = {} != глубина ПОСЛЕДНЕГО снапшота (@900, ={light_bid}) — \
         агрегация не close-семантика (impl first/mean вместо last)",
        series[0].1
    );
}

/// (OF-I-6 границы) пустой вход → пустой ряд (не паника, не выдуманная точка).
#[test]
fn empty_input_yields_empty_series() {
    let snaps: Vec<(i64, OrderBook)> = Vec::new();
    assert!(
        compute(&snaps, Side::Buy, B3, 1000).is_empty(),
        "пустой вход дал непустой ряд — выдуманные точки (та же дисциплина, что «дифф не говорит об отсутствующем»)"
    );
}
