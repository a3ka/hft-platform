//! M-06 RED (sacred, architect) — TD-014 T4: Funding via REST poll (WS markPrice НЕ доставляется).
//!
//! §8 REJECT ×5 подряд на Funding=0. ЭМПИРИЧЕСКИЙ диагноз architect'а (live WS-capture, exact
//! adapter URL, 400 depth-msg'ей): mark-price WS-стрим (`!markPrice@arr`, per-symbol `@markPrice@1s`,
//! `@markPrice`, raw) доставляет **0 сообщений**, тогда как depth льётся стабильно — И на sandbox-IP,
//! И на прод-VPS (depth L2=637, Funding=0). Т.е. WS mark-price path не работает в этом сетапе;
//! гонять его дальше бесперспективно (5 rejects это доказали).
//!
//! ПИВОТ (надёжный путь): funding из REST `/fapi/v1/premiumIndex` (all-perps в 1 вызове,
//! поле `lastFundingRate` + `time`), поллингом — ТОЧНО как OpenInterest (`/fapi/v1/openInterest`),
//! который УЖЕ работает live (OI=66 персистится). Тот же REST-poll механизм → высокая уверенность,
//! что Funding польётся.
//!
//! Оракул: `parse_premium_index(json)` → `Vec<MdEvent{BinanceFutures, Funding}>` по всем записям
//! (поллер фильтрует на выборку). compile-RED на 99b1329 (функции нет). Диагностика (§5): различает
//! parse (ставка/ts/символ) от источника (WS не доставляет → REST poll). Финальный гейт: §8 Funding>0.

use contracts::{to_fixed, MdEvent, MdPayload, Venue};
use venue_binance_futures::parse_premium_index;

// Реальная форма /fapi/v1/premiumIndex (all-symbols array): lastFundingRate + time.
const PREMIUM_INDEX: &str = r#"[
{"symbol":"BTCUSDT","markPrice":"62933.47","indexPrice":"62949.68","estimatedSettlePrice":"63040.43","lastFundingRate":"0.00005257","interestRate":"0.00010000","nextFundingTime":1783958400000,"time":1783939924000},
{"symbol":"ETHUSDT","markPrice":"3000.0","indexPrice":"3001.0","estimatedSettlePrice":"3002.0","lastFundingRate":"-0.00002000","interestRate":"0.00010000","nextFundingTime":1783958400000,"time":1783939924000},
{"symbol":"XRPUSDT","markPrice":"0.5","indexPrice":"0.5","estimatedSettlePrice":"0.5","lastFundingRate":"0.00010000","interestRate":"0.00010000","nextFundingTime":1783958400000,"time":1783939924000}
]"#;

fn find<'a>(evs: &'a [MdEvent], sym: &str) -> Option<&'a MdEvent> {
    evs.iter()
        .find(|m| m.venue == Venue::BinanceFutures && m.symbol == sym)
}

#[test]
fn td014_t4_funding_from_premium_index_rest_poll() {
    let evs = parse_premium_index(PREMIUM_INDEX);

    // BTCUSDT: ставка + биржевое время из `time`.
    match find(&evs, "BTCUSDT").map(|m| &m.payload) {
        Some(MdPayload::Funding {
            rate_e8,
            ts_exch_ms,
        }) => {
            assert_eq!(
                *rate_e8,
                to_fixed(0.00005257),
                "BTCUSDT lastFundingRate → rate_e8"
            );
            assert_eq!(*ts_exch_ms, 1783939924000, "ts из поля time");
        }
        other => panic!("TD-014 T4: BTCUSDT Funding из premiumIndex обязателен, got {other:?}"),
    }

    // ETHUSDT: ОТРИЦАТЕЛЬНЫЙ фандинг — знак сохранён (breadth зависит от знака).
    match find(&evs, "ETHUSDT").map(|m| &m.payload) {
        Some(MdPayload::Funding { rate_e8, .. }) => {
            assert_eq!(
                *rate_e8,
                to_fixed(-0.00002000),
                "негативный funding знак сохранён"
            )
        }
        other => panic!("ETHUSDT Funding обязателен, got {other:?}"),
    }

    // Все 3 записи распарсены (поллер отфильтрует на нашу выборку выше).
    assert!(
        evs.len() >= 3,
        "все записи premiumIndex → Funding events, got {}",
        evs.len()
    );
}
