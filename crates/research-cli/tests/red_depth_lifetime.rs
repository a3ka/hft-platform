//! RED DV-I-1..5 — L2Delta lifetime/staleness анализатор (sacred, architect-only) — M-32 Q2а.
//!
//! ЦЕЛЬ (docs/07 §6, M-32 §Мотивация): доказать/опровергнуть достоверность дальних полос 3-30%
//! БЕЗ эталона глубже 1.3%. Прямой измеритель — СЫРОЙ `MdPayload::L2Delta` (BTCUSDT, CT-RFC-04):
//! `size==0` = явный remove от биржи, sequencing (`U/u/pu`) = gap-детекция. Вопрос: дальний уровень
//! получает `size=0` при жизни (ЖИВОЙ) или замерзает до конца окна (ФАНТОМ)?
//!
//! Ключ (то, что shell-notional depth_probe НЕ мог): de-конфаунд resync'а. Уровень, исчезнувший
//! ЧЕРЕЗ sequence-GAP, — CENSORED (fate неизвестен), НЕ отмена и НЕ заморозка. depth_probe оперировал
//! реконструированными снапшотами (resync их обнулял → dd=100% ложно); здесь — события отмен напрямую.
//!
//! Контракт (research-dev impl, `crates/research-cli/src/depth_lifetime.rs`):
//!   `research_cli::depth_lifetime::analyze(ticks: &[DeltaTick]) -> LifetimeReport`
//!   - чистый редьюсер; трекает mid (running best bid/ask из дельт — raw apply, БЕЗ stale-FSM,
//!     чтобы mid не терялся после gap); атрибуция уровня к полосе — по дистанции от mid ПРИ РОЖДЕНИИ;
//!   - per-price жизненный цикл: born → (explicit size=0 в contiguous окне = cancelled) |
//!     (жив на конце окна, ни разу size=0 = frozen) | (исчез через seq-gap = censored);
//!   - gap-правило (то же, что `book::OrderBook::apply_l2delta`): спот `U==prev.u+1`; фьюч `pu==prev.u`;
//!   - вывод детерминирован (BTreeMap-порядок; `bands` отсортированы по (side, lo_bps)).
//!   `cancel_fraction = cancelled / (cancelled + frozen)` — censored ИСКЛЮЧЕНЫ из знаменателя.
//!
//! Анти-плацебо (ОБЕ стороны): заглушка «всё frozen» → падает DV-I-1; «всё cancelled» → DV-I-2;
//! наивный «gap-исчезновение = отмена» → DV-I-3 (ядро); «истекает после N молчаливых тиков» → DV-I-4;
//! HashMap-итерация в выводе → DV-I-5. compile-RED против отсутствия модуля.

use contracts::{Level, Side};
use research_cli::depth_lifetime::{analyze, DeltaTick};

const UNIT: i64 = 100_000_000;
const MID: i64 = 64_000 * UNIT;

/// Уровень на `pct` от mid: bid = mid·(1−pct), ask = mid·(1+pct). `size_units`·UNIT (0 = remove).
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

/// Спот-тик (`pu=None`); id'ы задаются явно для контроля непрерывности/gap.
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

// Полосы (bps от mid), совпадают с impl-константой: [0,150)[150,300)[300,500)[500,800)[800,1500)[1500,3000).
const NEAR_LO: i64 = 0; // полоса [0,150) bps — near, в пределах валидированного REST ≤1.3%
const FAR_LO: i64 = 500; // полоса [500,800) bps — дальняя (5-8%), где живёт вопрос фантома
const FAR_PCT: f64 = 0.06; // 600 bps → в [500,800)
const NEAR_PCT: f64 = 0.0005; // 5 bps → в [0,150)

/// Стабильный near-seed (best bid/ask ~5bps), чтобы mid ≈ MID и атрибуция полос была однозначна.
fn seed_bids() -> Vec<Level> {
    vec![lvl(NEAR_PCT, Side::Buy, 20)]
}
fn seed_asks() -> Vec<Level> {
    vec![lvl(NEAR_PCT, Side::Sell, 20)]
}

// ── DV-I-1: explicit size=0 = CANCELLED (живой уровень) ────────────────────────────────────────
#[test]
fn dv_i_1_explicit_cancel_is_live() {
    let ticks = vec![
        // bootstrap: near-seed + дальний bid на 6%
        tick(
            1,
            1,
            1_000,
            {
                let mut b = seed_bids();
                b.push(lvl(FAR_PCT, Side::Buy, 10));
                b
            },
            seed_asks(),
        ),
        // contiguous: явная отмена дальнего уровня (size=0)
        tick(2, 2, 2_000, vec![lvl(FAR_PCT, Side::Buy, 0)], vec![]),
    ];
    let r = analyze(&ticks);
    let far = r
        .band(Side::Buy, FAR_LO)
        .expect("дальняя полоса должна присутствовать");
    assert!(far.cancelled >= 1, "явный size=0 ⇒ cancelled≥1 (живой)");
    assert_eq!(far.frozen, 0, "отменённый уровень не может быть frozen");
}

// ── DV-I-2: born, никогда не size=0 до конца окна = FROZEN (фантом-кандидат) ─────────────────────
#[test]
fn dv_i_2_never_cancelled_is_frozen() {
    let ticks = vec![
        tick(
            1,
            1,
            1_000,
            {
                let mut b = seed_bids();
                b.push(lvl(FAR_PCT, Side::Buy, 10));
                b
            },
            seed_asks(),
        ),
        // contiguous тики, дальний уровень НИ РАЗУ не упомянут, окно кончается
        tick(2, 2, 2_000, seed_bids(), vec![]),
        tick(3, 3, 3_000, seed_bids(), vec![]),
    ];
    let r = analyze(&ticks);
    let far = r.band(Side::Buy, FAR_LO).expect("дальняя полоса");
    assert!(far.frozen >= 1, "жив на конце окна, ни разу size=0 ⇒ frozen≥1");
    assert_eq!(far.cancelled, 0, "не было явной отмены ⇒ cancelled=0");
    assert_eq!(far.censored, 0, "gap'а не было ⇒ censored=0");
}

// ── DV-I-3 (ЯДРО): исчезновение через seq-GAP = CENSORED, НЕ отмена и НЕ заморозка ───────────────
#[test]
fn dv_i_3_gap_vanish_is_censored_not_cancel() {
    let ticks = vec![
        tick(
            1,
            1,
            1_000,
            {
                let mut b = seed_bids();
                b.push(lvl(FAR_PCT, Side::Buy, 10));
                b
            },
            seed_asks(),
        ),
        // GAP: u_first=100 (≠ prev.u_final+1=2) ⇒ непрерывность нарушена; дальний уровень далее отсутствует
        tick(100, 100, 2_000, seed_bids(), vec![]),
        tick(101, 101, 3_000, seed_bids(), vec![]),
    ];
    let r = analyze(&ticks);
    assert!(r.gaps >= 1, "скачок update-id ⇒ gap задетектирован");
    let far = r.band(Side::Buy, FAR_LO).expect("дальняя полоса");
    assert!(far.censored >= 1, "исчез через gap ⇒ censored≥1 (fate неизвестен)");
    assert_eq!(
        far.cancelled, 0,
        "gap-исчезновение НЕ отмена (наивный анализатор соврал бы 'живой')"
    );
    assert_eq!(
        far.frozen, 0,
        "gap-исчезновение НЕ заморозка (уровень не дожил до конца окна валидно)"
    );
}

// ── DV-I-4: отсутствие ≠ удаление — молчание о уровне не старит и не отменяет его ────────────────
#[test]
fn dv_i_4_absence_is_not_deletion() {
    let ticks = vec![
        tick(
            1,
            1,
            1_000,
            {
                let mut b = seed_bids();
                b.push(lvl(FAR_PCT, Side::Buy, 10));
                b
            },
            seed_asks(),
        ),
        // много contiguous тиков про near-уровень (полоса [0,150)) — дальний [500,800) молчит (НЕ удалён)
        tick(2, 2, 2_000, vec![lvl(NEAR_PCT, Side::Buy, 21)], vec![]),
        tick(3, 3, 3_000, vec![lvl(NEAR_PCT, Side::Buy, 22)], vec![]),
        tick(4, 4, 4_000, vec![lvl(NEAR_PCT, Side::Buy, 23)], vec![]),
        // и только теперь — явная отмена дальнего уровня
        tick(5, 5, 5_000, vec![lvl(FAR_PCT, Side::Buy, 0)], vec![]),
    ];
    let r = analyze(&ticks);
    let far = r.band(Side::Buy, FAR_LO).expect("дальняя полоса");
    // «истекает после N молчаливых тиков» пометил бы уровень frozen ДО отмены → провал.
    assert!(far.cancelled >= 1, "уровень дожил до явной size=0 ⇒ cancelled≥1");
    assert_eq!(far.frozen, 0, "молчание не должно преждевременно замораживать");
}

// ── DV-I-5: детерминизм — тот же вход → идентичный отчёт; полосы отсортированы ────────────────────
#[test]
fn dv_i_5_determinism() {
    let mk = || {
        vec![
            tick(
                1,
                1,
                1_000,
                {
                    let mut b = seed_bids();
                    b.push(lvl(FAR_PCT, Side::Buy, 10));
                    b
                },
                {
                    let mut a = seed_asks();
                    a.push(lvl(FAR_PCT, Side::Sell, 10));
                    a
                },
            ),
            tick(2, 2, 2_000, vec![lvl(FAR_PCT, Side::Buy, 0)], seed_asks()),
        ]
    };
    let r1 = analyze(&mk());
    let r2 = analyze(&mk());
    assert_eq!(r1, r2, "тот же вход ⇒ байт-идентичный отчёт (VB-I-1 класс)");
    // явная сортировка вывода (HashMap-итерация дала бы нестабильный порядок)
    let keyed: Vec<(i64, i64)> = r1.bands.iter().map(|b| (b.side as i64, b.lo_bps)).collect();
    let mut sorted = keyed.clone();
    sorted.sort();
    assert_eq!(keyed, sorted, "bands обязаны быть отсортированы (детерминизм порядка)");
}

// ── Анти-плацебо чек-лист: асимметрия + множественность ─────────────────────────────────────────
#[test]
fn dv_i_checklist_asymmetry_and_multiplicity() {
    let ticks = vec![
        // bootstrap: ДВА дальних bid + дальний ask; near-seed обе стороны
        tick(
            1,
            1,
            1_000,
            {
                let mut b = seed_bids();
                b.push(lvl(FAR_PCT, Side::Buy, 10));
                b.push(lvl(0.065, Side::Buy, 10)); // второй уровень в той же полосе [500,800)
                b
            },
            {
                let mut a = seed_asks();
                a.push(lvl(FAR_PCT, Side::Sell, 10));
                a
            },
        ),
        // АСИММЕТРИЧНЫЙ тик: обновляются ТОЛЬКО bids (ask-сторона молчит — не должна съезжать/цензуриться),
        // МНОЖЕСТВЕННОСТЬ: два дальних bid отменены в ОДНОМ тике.
        tick(
            2,
            2,
            2_000,
            vec![lvl(FAR_PCT, Side::Buy, 0), lvl(0.065, Side::Buy, 0)],
            vec![],
        ),
    ];
    let r = analyze(&ticks);
    let far_bid = r.band(Side::Buy, FAR_LO).expect("дальняя bid-полоса");
    assert!(
        far_bid.cancelled >= 2,
        "две отмены в одном тике ⇒ cancelled≥2 (наивный 'один' падает)"
    );
    let far_ask = r.band(Side::Sell, FAR_LO).expect("дальняя ask-полоса");
    // Асимметрия: ask молчал ⇒ его дальний уровень frozen (жив), НЕ censored/cancelled.
    assert_eq!(far_ask.cancelled, 0, "ask-сторона молчала ⇒ не отменена");
    assert_eq!(far_ask.censored, 0, "односторонний bid-тик не роняет ask в censored");
    assert!(far_ask.frozen >= 1, "молчащий ask-уровень жив ⇒ frozen≥1");
}
