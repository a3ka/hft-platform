//! M-06 RED (sacred, architect) — #4: рекордер обязан супервизить BinanceFutures, иначе
//! futures depth/liquidations/OI/funding НЕ пишутся в живой журнал (нет входа для
//! funding-breadth C5). Падает на STUB default_venues (без BinanceFutures) → engine-dev GREEN.
//! Runtime-gate реального потока данных — §8 eyes-on на VPS (прод-поведенческое изменение).

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
    // Существующие площадки не потеряны.
    assert!(
        v.contains(&Venue::Binance) && v.contains(&Venue::Hyperliquid),
        "Binance + Hyperliquid должны сохраниться"
    );
}
