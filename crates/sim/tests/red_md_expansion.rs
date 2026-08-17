//! M-06 RED (sacred, architect) — sim ЯВНО игнорирует новые md-варианты CT-RFC-01
//! (OpenInterest/Liquidation/MarginRate): не паникует, не создаёт fills. C-003 §3:
//! явный ignore-arm, НЕ молчаливый wildcard (чтобы будущие типы не терялись незаметно
//! и не порождали фантомные исполнения).
//!
//! Падает КОМПАЙЛОМ на текущем коде (exchange.rs:223 exhaustive match не покрывает
//! новые варианты) → dev добавляет явный ignore-arm → GREEN. Анти-плацебо: если dev
//! случайно СМАРШРУТИЗИРУЕТ Liquidation в fill-логику — assert поймает.

use contracts::{to_fixed, Event, EventKind, MdPayload, Side, Venue};
use sim::{BacktestExchange, FeeSchedule, LatencyTable};

fn ev(seq: u64, payload: MdPayload) -> Event {
    Event {
        seq,
        ts_mono_ns: seq,
        ts_wall_ms: seq as i64,
        kind: EventKind::md(Venue::BinanceFutures, "BTCUSDT", payload),
    }
}

#[test]
fn sim_ignores_new_md_variants_no_fills_no_panic() {
    let mut ex = BacktestExchange::new(LatencyTable::new(), FeeSchedule::new(), 42);

    let cases = [
        MdPayload::OpenInterest {
            oi_e8: to_fixed(12345.0),
            ts_exch_ms: 1,
        },
        MdPayload::Liquidation {
            price: to_fixed(64000.0),
            size: to_fixed(1.0),
            side: Side::Sell,
            ts_exch_ms: 2,
        },
        MdPayload::MarginRate {
            rate_e8: to_fixed(0.0001),
            ts_exch_ms: 3,
        },
    ];

    for (i, p) in cases.into_iter().enumerate() {
        let fills = ex.on_event(&ev(i as u64, p));
        assert!(
            fills.is_empty(),
            "sim обязан ИГНОРИРОВАТЬ новый md-вариант (нет активных ордеров → нет fills), \
             получено {} fills",
            fills.len()
        );
    }
    assert_eq!(ex.open_orders(), 0, "новые md-варианты не создают ордеров");
}
