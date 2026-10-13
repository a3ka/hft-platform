//! RED MI-I-2/MI-I-4 — parse available-inventory → MarginInventory (sacred, architect-only) — M-35.
//! MD-only auth read (spot-домен `api.binance.com/sapi/v1/margin/available-inventory`), без order-пути.
//!
//! Контракт (venue-dev impl, `crates/venue-binance/src/`): чистый парсер
//!   `venue_binance::parse_available_inventory(json: &str, assets: &[&str]) -> Vec<MdEvent>`
//!   - JSON `{"assets":{"USDT":"19932592.28...","USDC":"...",...},"updateTime":<sec>}`;
//!   - для каждого asset ∈ `assets` (фильтр) → MdEvent{Venue::Binance, symbol=asset,
//!     MarginInventory{available_e8 = to_fixed(value), ts_exch_ms = updateTime×1000}};
//!   - asset вне фильтра / битый JSON → пропуск (fail-closed, не паника).
//!
//! Анти-плацебо: неверный asset-фильтр / без ×1e8-scale / без updateTime×1000 → FAIL.
//! compile-RED против отсутствия символа.

use contracts::{from_fixed, MdPayload, Venue};
use venue_binance::parse_available_inventory;

const BODY: &str = r#"{"assets":{"USDT":"19932592.2856805","USDC":"20514052.57370351","BTC":"181.49188335"},"updateTime":1784991235}"#;

// ── MI-I-2: 2 актива в фильтре → 2 события, верный symbol/venue/ts ───────────────────────────────
#[test]
fn mi_i_2_parses_filtered_assets() {
    let out = parse_available_inventory(BODY, &["USDT", "USDC"]);
    assert_eq!(
        out.len(),
        2,
        "USDT+USDC в фильтре ⇒ 2 события (BTC отфильтрован)"
    );
    for ev in &out {
        assert_eq!(ev.venue, Venue::Binance, "margin = spot-домен Binance");
        let MdPayload::MarginInventory {
            available_e8,
            ts_exch_ms,
        } = ev.payload
        else {
            panic!("ожидался MarginInventory, получен {:?}", ev.payload);
        };
        assert!(available_e8 > 0, "пул ≥0, USDT/USDC непусты");
        assert_eq!(
            ts_exch_ms, 1_784_991_235_000,
            "updateTime(сек) × 1000 = ms (анти-плацебо: без ×1000 → FAIL)"
        );
        assert!(ev.symbol == "USDT" || ev.symbol == "USDC", "symbol = актив");
    }
}

// ── MI-I-4: fixed-point точность (×1e8, без потери >8 знаков) ────────────────────────────────────
#[test]
fn mi_i_4_fixed_point_scale() {
    let out = parse_available_inventory(BODY, &["USDT"]);
    assert_eq!(out.len(), 1);
    let MdPayload::MarginInventory { available_e8, .. } = out[0].payload else {
        panic!("MarginInventory");
    };
    // "19932592.2856805" × 1e8 = 1_993_259_228_568_050
    assert_eq!(
        available_e8, 1_993_259_228_568_050,
        "verbatim ×1e8 (анти-плацебо: без scale → 19932592 → FAIL)"
    );
    assert!((from_fixed(available_e8) - 19_932_592.2856805).abs() < 1e-6);
}

// ── MI-I-2b: asset вне фильтра / битый JSON → пусто (fail-closed) ────────────────────────────────
#[test]
fn mi_i_2_absent_and_malformed_are_empty() {
    assert!(
        parse_available_inventory(BODY, &["DOGE"]).is_empty(),
        "asset не в ответе → пусто"
    );
    assert!(
        parse_available_inventory("{not json", &["USDT"]).is_empty(),
        "битый JSON → пусто, не паника"
    );
    assert!(
        parse_available_inventory(r#"{"updateTime":1}"#, &["USDT"]).is_empty(),
        "нет assets → пусто"
    );
}
