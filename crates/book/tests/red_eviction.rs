//! RED M-31 EV-I-1..6 (sacred, architect-only) — эвикция книги (TD-016): bound + recon-near.
//!
//! `enforce_cap(max_per_side)` — best-relative топ-N (bound памяти, OOM-бэкстоп). `reconcile_near(rest,
//! near_pct)` — в окне near_pct от mid КНИГИ держать только уровни из REST-эталона (мёртвые ближние
//! стереть); дальнее (>near_pct) НЕ трогать (inherently diff-реконструкция, честный провенанс на export).
//!
//! АНТИ-ПЛАЦЕБО (урок TD-016 v1-реджект): EV-I-1 (асимметрия) + EV-I-2 (cap) ПАДАЮТ против v1 (кап 5000 +
//! эвикция по mid диффа/широкому окну), GREEN против 3б (best-relative + book-mid).
//! COMPILE-RED: `enforce_cap`/`reconcile_near` ещё НЕ существуют (engine-dev tasks 2-3).

use book::OrderBook;
use contracts::{Level, Side};

fn lvl(price: i64, size: i64) -> Level {
    Level { price, size }
}

fn seeded(bids: &[(i64, i64)], asks: &[(i64, i64)]) -> OrderBook {
    let mut b = OrderBook::new();
    b.apply_snapshot(
        &bids.iter().map(|&(p, s)| lvl(p, s)).collect::<Vec<_>>(),
        &asks.iter().map(|&(p, s)| lvl(p, s)).collect::<Vec<_>>(),
    );
    b
}

#[test]
fn asymmetric_keeps_best_bid_and_far_live() {
    // EV-I-1 (анти-плацебо #1): АСИММЕТРИЧНЫЙ ask-only апдейт (best bid НЕ менялся). reconcile_near
    // ОБЯЗАН брать окно от mid КНИГИ (не диффа): дальний ЖИВОЙ bid (за окном 1.3%, НЕ в мелком REST) —
    // diff-реконструкция, ЦЕЛ; best bid цел. v1 (эвикция по mid диффа → окно смещено/расширено →
    // дальний bid ошибочно «ближний» и не в REST → стёрт) → FAIL.
    // bids 10000(best),9900,9000(дальний живой); asks 10010. mid=10005; окно 1.3% bids [9875,∞).
    let mut b = seeded(&[(10_000, 5), (9_900, 3), (9_000, 7)], &[(10_010, 4)]);
    // асимметрия: обновляется ТОЛЬКО ask; best bid (10000) не меняется.
    b.apply_delta(&[], &[lvl(10_010, 8)]);
    // REST-эталон МЕЛКИЙ (near-touch): 9000 в него НЕ входит (за окном; diff-реконструкция).
    let rest_bids = [lvl(10_000, 5), lvl(9_900, 3)];
    let rest_asks = [lvl(10_010, 8)];
    let _ = b.reconcile_near(&rest_bids, &rest_asks, 0.013);
    assert_eq!(
        b.best_bid(),
        Some(10_000),
        "EV-I-1: живой best bid СОХРАНЁН (окно от mid книги, не диффа)"
    );
    assert_eq!(
        b.size_at(Side::Buy, 9_000),
        7,
        "дальний ЖИВОЙ bid (за окном, не в мелком REST) НЕ стёрт (diff-реконструкция)"
    );
    assert_eq!(
        b.size_at(Side::Buy, 9_900),
        3,
        "ближний живой bid (в REST) цел"
    );
}

#[test]
fn recon_near_evicts_dead_keeps_far() {
    // EV-I-3: мёртвый БЛИЖНИЙ уровень (в окне, НЕ в REST) стёрт; ДАЛЬНИЙ (за окном) цел.
    // bids 10000,9950(мёртвый),9000(дальний); asks 10010. mid=10005; окно 1.3% → [9875,∞).
    let mut b = seeded(&[(10_000, 5), (9_950, 9), (9_000, 7)], &[(10_010, 4)]);
    let rest_bids = [lvl(10_000, 5)]; // 9950 НЕ в REST (мёртвый ближний); 9000 за окном (не важен)
    let rest_asks = [lvl(10_010, 4)];
    let evicted = b.reconcile_near(&rest_bids, &rest_asks, 0.013);
    assert_eq!(
        b.size_at(Side::Buy, 9_950),
        0,
        "EV-I-3: мёртвый ближний 9950 (не в REST) стёрт"
    );
    assert_eq!(
        b.size_at(Side::Buy, 9_000),
        7,
        "дальний 9000 (за окном) СОХРАНЁН (diff-реконструкция)"
    );
    assert_eq!(b.size_at(Side::Buy, 10_000), 5, "живой best bid цел");
    assert!(evicted >= 1, "хотя бы один мёртвый ближний эвикнут");
}

#[test]
fn cap_evicts_farthest_keeps_top() {
    // EV-I-2 (анти-плацебо #2): 5 bids > cap 2 → оставить 2 БЛИЖАЙШИХ к лучшей (топ), эвикт дальних.
    // v1 (кап 5000) не эвиктит → n_levels==5 → FAIL.
    let mut b = seeded(
        &[(10_000, 1), (9_900, 1), (9_800, 1), (9_700, 1), (9_600, 1)],
        &[(10_010, 1), (10_020, 1), (10_030, 1)],
    );
    b.enforce_cap(2);
    assert_eq!(b.n_levels(Side::Buy), 2, "EV-I-2: кэп bids до 2");
    assert_eq!(b.n_levels(Side::Sell), 2, "кэп asks до 2");
    assert_eq!(b.best_bid(), Some(10_000), "топ bid сохранён");
    assert_eq!(b.size_at(Side::Buy, 9_900), 1, "2-й ближайший bid сохранён");
    assert_eq!(b.size_at(Side::Buy, 9_600), 0, "дальний bid 9600 эвикнут");
    assert_eq!(b.best_ask(), Some(10_010), "топ ask сохранён");
    assert_eq!(b.size_at(Side::Sell, 10_030), 0, "дальний ask эвикнут");
}

#[test]
fn absence_not_deletion() {
    // EV-I-4: эвикция НЕ удаляет уровни «просто так». enforce_cap(10) при 3 уровнях → no-op.
    let mut b = seeded(&[(10_000, 5), (9_900, 3), (9_800, 2)], &[(10_010, 4)]);
    let evicted = b.enforce_cap(10);
    assert_eq!(
        evicted, 0,
        "EV-I-4: кэп выше числа уровней → ничего не удалено"
    );
    assert_eq!(b.n_levels(Side::Buy), 3, "все bids на месте");
    // reconcile_near с REST, содержащим все ближние → ничего не эвиктит.
    let rest_bids = [lvl(10_000, 5), lvl(9_900, 3), lvl(9_800, 2)];
    let rest_asks = [lvl(10_010, 4)];
    assert_eq!(
        b.reconcile_near(&rest_bids, &rest_asks, 0.013),
        0,
        "REST подтверждает все ближние → 0 эвикций"
    );
}

#[test]
fn backstop_bounds_growth() {
    // EV-I-5: OOM-бэкстоп — после роста enforce_cap(cap) ЖЁСТКО держит n_levels ≤ cap.
    let mut b = OrderBook::new();
    b.apply_snapshot(&[lvl(100_000, 1)], &[lvl(100_001, 1)]);
    // рост книги дельтами (мёртвые уровни накапливаются — корень TD-016).
    for i in 1..2000i64 {
        b.apply_delta(&[lvl(100_000 - i, 1)], &[lvl(100_001 + i, 1)]);
    }
    assert!(b.n_levels(Side::Buy) > 1000, "предусловие: книга выросла");
    let cap = 500;
    b.enforce_cap(cap);
    assert!(b.n_levels(Side::Buy) <= cap, "EV-I-5: bids ограничены cap");
    assert!(b.n_levels(Side::Sell) <= cap, "asks ограничены cap");
    assert_eq!(
        b.best_bid(),
        Some(100_000),
        "лучший bid сохранён под бэкстопом"
    );
}

#[test]
fn determinism() {
    // EV-I-6: тот же вход+эвикция → идентичные levels().
    let run = || {
        let mut b = seeded(
            &[(10_000, 1), (9_900, 1), (9_800, 1), (9_700, 1)],
            &[(10_010, 1), (10_020, 1)],
        );
        b.enforce_cap(2);
        let rest_bids = [lvl(10_000, 1), lvl(9_900, 1)];
        let rest_asks = [lvl(10_010, 1)];
        b.reconcile_near(&rest_bids, &rest_asks, 0.013);
        b
    };
    let a = run();
    let b = run();
    assert_eq!(a.levels(Side::Buy), b.levels(Side::Buy), "bid детерминизм");
    assert_eq!(
        a.levels(Side::Sell),
        b.levels(Side::Sell),
        "ask детерминизм"
    );
}
