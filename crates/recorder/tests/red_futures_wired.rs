//! M-06 RED (sacred, architect) — #4 (reland, post-TD-013): рекордер обязан супервизить
//! BinanceFutures, иначе futures depth/liquidations/OI/funding НЕ пишутся в живой журнал
//! (нет входа для funding-breadth C5). Падает, пока `default_venues()` не включит BinanceFutures
//! (engine-dev reland 2eee4bf). Runtime-gate реального потока — §8 eyes-on (прод-поведенческое,
//! + LIVE-проверка TD-013 backoff против 418-hot-loop).

use contracts::Venue;
use recorder::default_venues;

#[test]
fn j4_recorder_supervises_binance_futures() {
    let v = default_venues();
    assert!(
        v.contains(&Venue::BinanceFutures),
        "recorder обязан супервизить BinanceFutures — иначе futures MD (depth/liq/OI/funding) \
         не попадает в журнал (#4)"
    );
    assert!(
        v.contains(&Venue::Binance) && v.contains(&Venue::Hyperliquid),
        "Binance + Hyperliquid должны сохраниться"
    );
}
