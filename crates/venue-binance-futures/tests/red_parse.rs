//! M-06 RED (sacred, architect) — парс-граница venue-binance-futures. Падают на STUB
//! (None) → venue-dev делает GREEN. Анти-плацебо: C2 проверяет ОБЕ стороны ликвидации,
//! хардкод одной не пройдёт.

use contracts::{to_fixed, MdEvent, MdPayload, Side, Venue};
use venue_binance_futures::{parse_depth_snapshot, parse_force_order, parse_open_interest};

/// C2 — Liquidation.side = сторона форс-ордера (o.S). SELL⟺LONG-ликвидация, BUY⟺SHORT
/// (C-003 note). Проверяем ОБЕ стороны → нельзя удовлетворить хардкодом.
#[test]
fn c2_force_order_side_is_liquidated_side() {
    let sell = r#"{"e":"forceOrder","E":1568014460891,"o":{"s":"BTCUSDT","S":"SELL","q":"0.014","p":"9910","ap":"9910","X":"FILLED","l":"0.014","z":"0.014","T":1568014460893}}"#;
    assert_eq!(
        parse_force_order(sell),
        Some(MdEvent {
            venue: Venue::BinanceFutures,
            symbol: "BTCUSDT".to_string(),
            payload: MdPayload::Liquidation {
                price: to_fixed(9910.0),
                size: to_fixed(0.014),
                side: Side::Sell,
                ts_exch_ms: 1568014460893,
            },
        }),
        "SELL forceOrder ⟺ ликвидируется LONG → side=Sell"
    );

    let buy = r#"{"e":"forceOrder","E":1568014470000,"o":{"s":"ETHUSDT","S":"BUY","q":"2.5","p":"1800","ap":"1800","X":"FILLED","l":"2.5","z":"2.5","T":1568014470111}}"#;
    assert_eq!(
        parse_force_order(buy),
        Some(MdEvent {
            venue: Venue::BinanceFutures,
            symbol: "ETHUSDT".to_string(),
            payload: MdPayload::Liquidation {
                price: to_fixed(1800.0),
                size: to_fixed(2.5),
                side: Side::Buy,
                ts_exch_ms: 1568014470111,
            },
        }),
        "BUY forceOrder ⟺ ликвидируется SHORT → side=Buy"
    );
}

/// C2b — depth снапшот → L2Snapshot с Venue::BinanceFutures; ts_exch_ms = T.
#[test]
fn c2b_depth_snapshot_parses_futures_l2() {
    let json = r#"{"lastUpdateId":100,"E":1568014460000,"T":1568014460050,"bids":[["64000.0","1.5"],["63999.0","3.0"]],"asks":[["64001.0","2.0"]]}"#;
    let got = parse_depth_snapshot("BTCUSDT", json);
    let MdEvent {
        venue,
        symbol,
        payload,
    } = got.expect("depth снапшот обязан распарситься");
    assert_eq!(venue, Venue::BinanceFutures);
    assert_eq!(symbol, "BTCUSDT");
    match payload {
        MdPayload::L2Snapshot {
            bids,
            asks,
            ts_exch_ms,
        } => {
            assert_eq!(bids.len(), 2);
            assert_eq!(asks.len(), 1);
            assert_eq!(bids[0].price, to_fixed(64000.0));
            assert_eq!(bids[0].size, to_fixed(1.5));
            assert_eq!(asks[0].price, to_fixed(64001.0));
            assert_eq!(ts_exch_ms, 1568014460050);
        }
        other => panic!("ожидался L2Snapshot, получено {other:?}"),
    }
}

/// C3 — openInterest → OpenInterest{oi_e8}; ts_exch_ms = time.
#[test]
fn c3_open_interest_parses() {
    let json = r#"{"openInterest":"10659.509","symbol":"BTCUSDT","time":1583127900000}"#;
    assert_eq!(
        parse_open_interest("BTCUSDT", json),
        Some(MdEvent {
            venue: Venue::BinanceFutures,
            symbol: "BTCUSDT".to_string(),
            payload: MdPayload::OpenInterest {
                oi_e8: to_fixed(10659.509),
                ts_exch_ms: 1583127900000,
            },
        })
    );
}
