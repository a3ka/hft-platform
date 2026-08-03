//! RED DV-I-9 — полоса 30–60% (`[3000,6000)` bps) различима (sacred, architect-only) — M-33.
//!
//! Founder APPROVED TPP-полосы 1.5–60% (M-32, граница C), но живость доказана лишь 1.5–30% —
//! схема `BANDS_BPS` кончалась на `[1500,3000)`. Этот оракул требует ОТДЕЛЬНУЮ полосу `[3000,6000)`,
//! чтобы 30–60% можно было переснять (born/cancel), а не клампить в `[1500,3000)`.
//!
//! Контракт (research-dev impl): расширить `BANDS_BPS` → добавить `(3000, 6000)`. Атрибуция уровня
//! на 45% от mid (4500 bps) ⇒ `lo_bps=3000`; уровень на 20% (2000 bps) ⇒ `lo_bps=1500` (отдельно).
//!
//! Анти-плацебо: текущий impl клампит `>=3000` в последнюю полосу `[1500,3000)` ⇒ `band(_,3000)=None`
//! → FAIL; после расширения `BANDS_BPS` → GREEN. compile-RED против отсутствия символов.

use contracts::{Level, Side};
use research_cli::depth_lifetime::{analyze, DeltaTick};

const UNIT: i64 = 100_000_000;
const MID: i64 = 64_000 * UNIT;

const B3060_LO: i64 = 3000; // полоса [3000,6000) bps = 30–60% (founder-диапазон, верхняя TPP-полоса)
const B1530_LO: i64 = 1500; // полоса [1500,3000) bps = 15–30% (уже верифицирована M-32)
const PCT_45: f64 = 0.45; // 4500 bps → в [3000,6000)
const PCT_20: f64 = 0.20; // 2000 bps → в [1500,3000)
const NEAR_PCT: f64 = 0.0005; // стабильный near-seed для однозначного mid

fn lvl(pct: f64, side: Side, size_units: i64) -> Level {
    let price = match side {
        Side::Buy => MID as f64 * (1.0 - pct),
        Side::Sell => MID as f64 * (1.0 + pct),
    };
    Level {
        price: price as i64,
        size: size_units * UNIT,
    }
}

fn tick(u_first: u64, u_final: u64, ts_ms: i64, bids: Vec<Level>, asks: Vec<Level>) -> DeltaTick {
    DeltaTick {
        bids,
        asks,
        first_update_id: u_first,
        final_update_id: u_final,
        prev_final_update_id: None,
        ts_exch_ms: ts_ms,
    }
}

fn seed_bids() -> Vec<Level> {
    vec![lvl(NEAR_PCT, Side::Buy, 20)]
}
fn seed_asks() -> Vec<Level> {
    vec![lvl(NEAR_PCT, Side::Sell, 20)]
}

// ── DV-I-9: 45%-уровень попадает в ОТДЕЛЬНУЮ полосу [3000,6000), не клампится в [1500,3000) ──────
#[test]
fn dv_i_9_band_3060_distinct() {
    let ticks = vec![tick(
        1,
        1,
        1_000,
        {
            let mut b = seed_bids();
            b.push(lvl(PCT_20, Side::Buy, 10)); // 20% → [1500,3000)
            b.push(lvl(PCT_45, Side::Buy, 10)); // 45% → [3000,6000) (новая полоса)
            b
        },
        seed_asks(),
    )];
    let r = analyze(&ticks);
    // Новая полоса 30–60% должна СУЩЕСТВОВАТЬ и содержать 45%-уровень.
    let b3060 = r
        .band(Side::Buy, B3060_LO)
        .expect("полоса [3000,6000) должна существовать (impl клампит 4500→1500 → None → FAIL)");
    assert!(b3060.lives_born >= 1, "45%-уровень рождён в [3000,6000) ⇒ born≥1");
    assert_eq!(
        b3060.hi_bps, 6000,
        "верхняя граница полосы = 6000 bps (60%)"
    );
    // Полоса 15–30% отдельна и содержит 20%-уровень (не слилась с 45%).
    let b1530 = r.band(Side::Buy, B1530_LO).expect("полоса [1500,3000)");
    assert!(
        b1530.lives_born >= 1,
        "20%-уровень отдельно в [1500,3000) ⇒ born≥1"
    );
    // 45%-уровень НЕ должен оказаться в [1500,3000) (различимость).
    assert!(
        b1530.lives_born < 2,
        "20% и 45% — РАЗНЫЕ полосы; [1500,3000) не должна поглотить 45%-уровень"
    );
}

// ── DV-I-9b: живость новой полосы — явный size=0 на 45%-уровне ⇒ cancelled в [3000,6000) ─────────
#[test]
fn dv_i_9_cancel_in_3060() {
    let ticks = vec![
        tick(
            1,
            1,
            1_000,
            {
                let mut b = seed_bids();
                b.push(lvl(PCT_45, Side::Buy, 10));
                b
            },
            seed_asks(),
        ),
        // contiguous: явная отмена 45%-уровня
        tick(2, 2, 2_000, vec![lvl(PCT_45, Side::Buy, 0)], vec![]),
    ];
    let r = analyze(&ticks);
    let b3060 = r
        .band(Side::Buy, B3060_LO)
        .expect("полоса [3000,6000) должна существовать");
    assert!(
        b3060.lives_cancelled >= 1,
        "явный size=0 на 45%-уровне ⇒ cancelled≥1 в [3000,6000) (живость новой полосы)"
    );
    assert_eq!(b3060.lives_frozen, 0, "отменённый уровень не frozen");
}
