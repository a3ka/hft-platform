//! RED FB-I-1 — funding-breadth: эмитить Funding по ВСЕМ перпам, не только трек-выборке
//! (sacred, architect-only) — M-34. MD-only (read premiumIndex funding, без order-пути) → reviewer.
//!
//! Мотивация (founder 2026-07-25): фандинг собирать ПО ВСЕМУ (для 4-групп breadth-метрики TPP).
//! Инфра уже тянет ВСЕ ~400 перпов одним `premiumIndex`-вызовом, но `poll_premium_index`
//! ФИЛЬТРУЕТ до subscribed перед записью (строка «фильтруем на нашу выборку») → вселенная
//! выбрасывается. M-34 снимает фильтр в breadth-режиме.
//!
//! Контракт (venue-dev impl): вынести решение emit-множества в чистую функцию
//! `venue_binance_futures::select_funding_emit(parsed, subscribed, breadth) -> Vec<MdEvent>`.
//! `breadth == true` → вернуть ВСЕ parsed (вселенная перпов), порядок сохранён; `breadth == false` →
//! отфильтровать до `subscribed` (legacy-режим сохранён, регрессия). `poll_premium_index` зовёт с `breadth=true`.
//!
//! Анти-плацебо: legacy inline-фильтр (breadth игнорируется, всегда фильтрует до subscribed) →
//! breadth=true тест видит только subscribed → FAIL. compile-RED против отсутствия символа.

use std::collections::HashSet;

use contracts::{MdEvent, MdPayload, Venue};
use venue_binance_futures::select_funding_emit;

fn funding(sym: &str, rate_e8: i64) -> MdEvent {
    MdEvent {
        venue: Venue::BinanceFutures,
        symbol: sym.to_string(),
        payload: MdPayload::Funding {
            rate_e8,
            ts_exch_ms: 1_784_900_000_000,
        },
    }
}

fn subscribed(syms: &[&str]) -> HashSet<String> {
    syms.iter().map(|s| s.to_string()).collect()
}

// ── FB-I-1: breadth=true эмитит ВСЕ перпы (включая не-subscribed) ────────────────────────────────
#[test]
fn fb_i_1_breadth_emits_all_perps() {
    // premiumIndex вернул 4 перпа; трекаем только BTCUSDT.
    let parsed = vec![
        funding("BTCUSDT", 10_000),
        funding("ETHUSDT", -5_000),
        funding("DOGEUSDT", 20_000),
        funding("AGLDUSDT", 3_000),
    ];
    let sub = subscribed(&["BTCUSDT"]);

    let out = select_funding_emit(parsed.clone(), &sub, true);
    assert_eq!(
        out.len(),
        4,
        "breadth ⇒ ВСЕ 4 перпа эмитятся (legacy-фильтр вернул бы 1 → FAIL)"
    );
    let syms: Vec<&str> = out.iter().map(|e| e.symbol.as_str()).collect();
    assert!(
        syms.contains(&"DOGEUSDT") && syms.contains(&"AGLDUSDT"),
        "не-subscribed перпы обязаны присутствовать в breadth-режиме"
    );
    // Порядок сохранён (детерминизм записи).
    assert_eq!(syms, vec!["BTCUSDT", "ETHUSDT", "DOGEUSDT", "AGLDUSDT"]);
}

// ── FB-I-1b: breadth=false сохраняет legacy-фильтр (регрессия трек-режима) ───────────────────────
#[test]
fn fb_i_1_legacy_mode_filters_to_subscribed() {
    let parsed = vec![
        funding("BTCUSDT", 10_000),
        funding("ETHUSDT", -5_000),
        funding("DOGEUSDT", 20_000),
    ];
    let sub = subscribed(&["BTCUSDT", "ETHUSDT"]);

    let out = select_funding_emit(parsed, &sub, false);
    assert_eq!(out.len(), 2, "legacy ⇒ только subscribed (BTC/ETH)");
    let syms: Vec<&str> = out.iter().map(|e| e.symbol.as_str()).collect();
    assert!(
        !syms.contains(&"DOGEUSDT"),
        "не-subscribed отфильтрован в legacy"
    );
}

// ── FB-I-1c: пустой parsed → пусто (fail-closed, не паника) ──────────────────────────────────────
#[test]
fn fb_i_1_empty_is_empty() {
    let out = select_funding_emit(Vec::new(), &subscribed(&["BTCUSDT"]), true);
    assert!(out.is_empty());
}
