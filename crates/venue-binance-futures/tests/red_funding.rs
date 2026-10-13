//! M-06 RED (sacred, architect) — N3: futures funding-парсер (markPriceUpdate → Funding).
//! Реальный вход для derive funding-breadth (M-06 #5). Падает на STUB (None) → venue-dev GREEN.
//! Анти-плацебо: положительная И отрицательная ставка — знак нельзя захардкодить.

use contracts::{to_fixed, MdEvent, MdPayload, Venue};
use venue_binance_futures::parse_mark_price;

#[test]
fn n3_mark_price_parses_funding_rate_with_sign() {
    // markPriceUpdate: r = funding rate, E = event time, s = symbol.
    let pos = r#"{"e":"markPriceUpdate","E":1562305380000,"s":"BTCUSDT","p":"64000.0","i":"63999.0","P":"64010.0","r":"0.00010000","T":1562306400000}"#;
    assert_eq!(
        parse_mark_price(pos),
        Some(MdEvent {
            venue: Venue::BinanceFutures,
            symbol: "BTCUSDT".to_string(),
            payload: MdPayload::Funding {
                rate_e8: to_fixed(0.0001),
                ts_exch_ms: 1562305380000,
            },
        }),
        "положительный funding rate r → Funding.rate_e8 = r×1e8"
    );

    let neg = r#"{"e":"markPriceUpdate","E":1562305390000,"s":"ETHUSDT","p":"1800.0","i":"1799.0","P":"1801.0","r":"-0.00020000","T":1562306400000}"#;
    assert_eq!(
        parse_mark_price(neg),
        Some(MdEvent {
            venue: Venue::BinanceFutures,
            symbol: "ETHUSDT".to_string(),
            payload: MdPayload::Funding {
                rate_e8: to_fixed(-0.0002),
                ts_exch_ms: 1562305390000,
            },
        }),
        "ОТРИЦАТЕЛЬНЫЙ funding rate → знак сохранён (breadth зависит от знака)"
    );
}
