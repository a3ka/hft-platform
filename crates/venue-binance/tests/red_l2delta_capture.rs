//! RED M-18 / CT-RFC-04 (sacred, architect-only): venue-binance СПОТ обязан капчить СЫРОЙ
//! `@depth` diff как `MdPayload::L2Delta` БЕЗ ПОТЕРЬ.
//!
//! RED-first: `venue_binance::l2delta_event` ещё НЕ существует → крейт-тесты не компилируются
//! (compile-RED). venue-dev реализует чистый транслятор `&DepthDiff -> EventKind::Md(L2Delta)`
//! и вызывает его в emit-пути для КАЖДОГО распарсенного diff'а (raw-капча независима от
//! book-sync FSM: сырой diff — это ground-truth рыночное событие; update-id'ы несут continuity).
//!
//! Анти-плацебо: тест ПАДАЕТ, если реализация роняет поле, путает сторону, теряет `size==0`
//! remove, схлопывает пустую сторону или подставляет `prev_final` на споте.
//! `.claude/rules/testing.md`: асимметрия (asks пустой), множественность (2 бида),
//! отсутствие/`size==0` (remove-маркер), границы (U/u).

use contracts::{EventKind, MdEvent, MdPayload, Venue};
use venue_binance::{l2delta_event, DepthDiff};

fn asym_diff() -> DepthDiff {
    DepthDiff {
        event_time_ms: 1_752_000_000_499,
        u_first: 101,
        u_final: 103,
        // bids: upsert + remove(size==0); asks: пусто (не менялось, НЕ очистка).
        bids: vec![(6_500_050_000_000, 30_000_000), (6_500_040_000_000, 0)],
        asks: vec![],
    }
}

#[test]
fn spot_diff_maps_to_lossless_l2delta() {
    let ev = l2delta_event("BTCUSDT", &asym_diff());
    let EventKind::Md(MdEvent {
        venue,
        symbol,
        payload:
            MdPayload::L2Delta {
                bids,
                asks,
                first_update_id,
                final_update_id,
                prev_final_update_id,
                ts_exch_ms,
            },
    }) = ev
    else {
        panic!("venue-binance обязан эмитить EventKind::Md(L2Delta)");
    };
    assert_eq!(venue, Venue::Binance);
    assert_eq!(symbol, "BTCUSDT");
    assert_eq!(bids.len(), 2, "оба бид-уровня сохранены (множественность)");
    assert_eq!(
        (bids[0].price, bids[0].size),
        (6_500_050_000_000, 30_000_000)
    );
    assert_eq!(
        (bids[1].price, bids[1].size),
        (6_500_040_000_000, 0),
        "size==0 remove сохранён как явный маркер"
    );
    assert!(
        asks.is_empty(),
        "пустая сторона осталась пустой (отсутствие)"
    );
    assert_eq!(first_update_id, 101, "U → first_update_id");
    assert_eq!(final_update_id, 103, "u → final_update_id");
    assert_eq!(
        prev_final_update_id, None,
        "СПОТ: prev_final_update_id == None (нет pu)"
    );
    assert_eq!(ts_exch_ms, 1_752_000_000_499, "E → ts_exch_ms");
}
