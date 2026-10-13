//! RED M-18 / CT-RFC-04 (sacred, architect-only): venue-binance-futures ПЕРП обязан капчить
//! сырой fstream `@depth` diff как `MdPayload::L2Delta` с `prev_final_update_id = Some(pu)`.
//!
//! RED-first: `venue_binance_futures::l2delta_event` + публичность `DepthDiff` ещё не существуют
//! (compile-RED). venue-dev: (а) делает `pub struct DepthDiff` (own-crate, scope-guard разрешает
//! свои типы); (б) реализует `l2delta_event(&DepthDiff) -> EventKind::Md(L2Delta)`, кладя
//! futures `pu` в `prev_final_update_id` (continuity перп-книги чейнится по `pu`, НЕ по `U==last+1`
//! — урок TD-014). Вызывает его в emit-пути на каждый распарсенный diff.
//!
//! Анти-плацебо: тест ПАДАЕТ, если `pu` потерян/подставлен `None` (перп-gap-детекция сломана)
//! или поля дельты искажены.

use contracts::{EventKind, MdEvent, MdPayload, Venue};
use venue_binance_futures::{l2delta_event, DepthDiff};

#[test]
fn futures_diff_maps_to_l2delta_with_pu() {
    let diff = DepthDiff {
        event_time_ms: 1_752_000_000_599,
        pu: 500, // previous final update id — чейнится на предыдущий final_update_id
        u_first: 501,
        u_final: 510, // U/u у перпа ПРЫГАЮТ (не +1) — continuity только по pu
        bids: vec![(6_500_050_000_000, 45_000_000)],
        asks: vec![(6_500_060_000_000, 0)], // remove на аск-стороне
    };
    let ev = l2delta_event("BTCUSDT", &diff);
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
        panic!("venue-binance-futures обязан эмитить EventKind::Md(L2Delta)");
    };
    assert_eq!(venue, Venue::BinanceFutures);
    assert_eq!(symbol, "BTCUSDT");
    assert_eq!(
        (bids[0].price, bids[0].size),
        (6_500_050_000_000, 45_000_000)
    );
    assert_eq!(
        (asks[0].price, asks[0].size),
        (6_500_060_000_000, 0),
        "size==0 remove сохранён"
    );
    assert_eq!((first_update_id, final_update_id), (501, 510));
    assert_eq!(
        prev_final_update_id,
        Some(500),
        "ФЬЮЧЕРС: pu ОБЯЗАН попасть в prev_final_update_id (иначе перп-gap-детекция сломана)"
    );
    assert_eq!(ts_exch_ms, 1_752_000_000_599);
}
