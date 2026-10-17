//! RED OF-I-4 OHLCV-БАРЫ ИЗ СДЕЛОК (sacred, architect-only) — M-17 экспорт под `code2alpha` DataFeed.
//!
//! Свечи для фронта: агрегируем сделки в 1s-OHLCV (фронт агрегирует дальше клиентски). Формат под
//! lightweight-charts/UDF (`time` в СЕКУНДАХ). Это данные для чарта, не Signal.
//!
//! Контракт (research-dev impl), вход = сделки `(ts_ms, price, size)`:
//!   `research_cli::export::ohlcv_bars(trades, timeframe_ms) -> Vec<OhlcvBar>`
//!   `OhlcvBar { time_s, open, high, low, close, volume }` (pub-поля).
//!   Правила per бакет: open = ПЕРВАЯ цена, high = MAX, low = MIN, close = ПОСЛЕДНЯЯ, volume = Σsize.
//!
//! Анти-плацебо: open=last (вместо first) → падает; high=last (вместо max) → падает; volume=число
//! сделок (вместо Σsize) → падает; «бар-на-сделку» (нет агрегации) → падает timeframe. Против отсутствия — compile-RED.

use research_cli::export::{ohlcv_bars, OhlcvBar};

fn tr(ts: i64, price: i64, size: i64) -> (i64, i64, i64) {
    (ts, price, size)
}

/// (детерминизм)
#[test]
fn ohlcv_is_deterministic() {
    let trades = vec![tr(100, 100, 1), tr(200, 105, 2)];
    assert_eq!(
        ohlcv_bars(&trades, 1000),
        ohlcv_bars(&trades, 1000),
        "ohlcv_bars недетерминирована"
    );
}

/// OHLC + volume верны: open=первая, high=max, low=min, close=последняя, volume=Σsize.
#[test]
fn ohlcv_fields_are_correct() {
    // bucket [0,1000): цены 100(open) →105(high) →98(low) →102(close); size 1+2+1+3=7.
    let trades = vec![
        tr(100, 100, 1),
        tr(300, 105, 2),
        tr(500, 98, 1),
        tr(900, 102, 3),
    ];
    let bars = ohlcv_bars(&trades, 1000);
    assert_eq!(bars.len(), 1, "4 сделки в одном бакете → 1 бар");
    let b: &OhlcvBar = &bars[0];
    assert_eq!(b.time_s, 0, "time в СЕКУНДАХ, начало бакета = 0");
    assert_eq!(
        b.open, 100,
        "open = ПЕРВАЯ цена (100), получено {} (impl взял last?)",
        b.open
    );
    assert_eq!(
        b.high, 105,
        "high = MAX (105), получено {} (impl взял last/first?)",
        b.high
    );
    assert_eq!(b.low, 98, "low = MIN (98), получено {}", b.low);
    assert_eq!(
        b.close, 102,
        "close = ПОСЛЕДНЯЯ цена (102), получено {}",
        b.close
    );
    assert_eq!(
        b.volume, 7,
        "volume = Σsize (7), получено {} (impl посчитал число сделок=4?)",
        b.volume
    );
}

/// один бар на бакет таймфрейма; разные бакеты — раздельно.
#[test]
fn ohlcv_one_bar_per_timeframe_bucket() {
    let trades = vec![tr(100, 100, 1), tr(900, 101, 1), tr(1500, 102, 1)];
    let bars = ohlcv_bars(&trades, 1000);
    assert_eq!(
        bars.len(),
        2,
        "3 сделки в 2 бакетах → 2 бара (нет агрегации → 3)"
    );
    assert_eq!(bars[0].time_s, 0);
    assert_eq!(bars[1].time_s, 1, "второй бакет [1000,2000)мс → t=1с");
    assert_eq!(
        bars[0].close, 101,
        "бар0 close = последняя в бакете (@900, 101)"
    );
    assert_eq!(
        bars[1].open, 102,
        "бар1 open = первая в бакете (@1500, 102)"
    );
}

/// (границы) пустой вход → пустой ряд.
#[test]
fn empty_trades_yield_no_bars() {
    let empty: Vec<(i64, i64, i64)> = Vec::new();
    assert!(
        ohlcv_bars(&empty, 1000).is_empty(),
        "пустой вход → пустые бары, не выдуманная свеча"
    );
}
