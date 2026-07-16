//! RED OPS-I-1 (sacred, architect-only) — сверка книги с REST-снапшотом ловит порчу данных.
//!
//! Это ЕДИНСТВЕННЫЙ оракул класса, который поймал бы C1 (эвикция стёрла best bid при зелёном
//! healthcheck). Чек-лист `.claude/rules/testing.md` — деградированные входы, НЕ «счастливый путь»:
//!  • асимметрия: порча ТОЛЬКО одной стороны;
//!  • отсутствие: пропавший уровень (best bid) — это порча, а не «дифф молчит»;
//!  • множественность: несколько уровней разошлись;
//!  • границы: пустая книга / один уровень;
//!  • ε_test НЕ калибруется (const), ε_prod ≤ ε_max (fail-closed потолок).
//! Анти-плацебо: против `todo!()`-скелета все падают; против «reconcile всегда без расхождения»
//! падают все, кроме identity.

use book::OrderBook;
use contracts::{Level, ReconAction, Venue};
use ops::recon::{reconcile, ReconThresholds, EPS_MAX_BPS, EPS_PROD_DEFAULT_BPS, EPS_TEST_BPS};

const MID: i64 = 65_000_000_000_000; // $65k ×1e8
const TICK: i64 = 1_000_000; // $0.01 ×1e8

fn lvl(price: i64, size: i64) -> Level {
    Level { price, size }
}

/// Плотная симметричная книга ±100 тиков от mid, объём 5.0 на уровень.
fn full_book() -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = (1..=100)
        .map(|k| lvl(MID - k * TICK, 5 * 100_000_000))
        .collect();
    let asks: Vec<Level> = (1..=100)
        .map(|k| lvl(MID + k * TICK, 5 * 100_000_000))
        .collect();
    b.apply_snapshot(&bids, &asks);
    b
}

/// (identity) Локальная книга == REST-референс → расхождения НЕТ, алерта НЕТ.
#[test]
fn ops_i_1_identical_books_do_not_alert() {
    let local = full_book();
    let reference = full_book();
    let out = reconcile(&local, &reference);
    assert!(
        !out.best_price_diverged,
        "идентичные книги не расходятся по best"
    );
    assert!(
        !out.exceeds_test(),
        "идентичные книги подняли ложный алерт — recon будет кричать на здоровые данные"
    );
}

/// (АСИММЕТРИЯ + ОТСУТСТВИЕ, C1-класс) В локальной книге пропал BEST BID (эвикция стёрла),
/// ask цел. Расхождение best bid — это ПОРЧА, обязано поднять алерт ВСЕГДА (ε_test).
#[test]
fn ops_i_1_missing_best_bid_must_alert() {
    let reference = full_book();
    // local без ближайшего к mid бида (эвикция C1) — ask нетронут.
    let mut local = OrderBook::new();
    let bids: Vec<Level> = (2..=100)
        .map(|k| lvl(MID - k * TICK, 5 * 100_000_000))
        .collect();
    let asks: Vec<Level> = (1..=100)
        .map(|k| lvl(MID + k * TICK, 5 * 100_000_000))
        .collect();
    local.apply_snapshot(&bids, &asks);

    let out = reconcile(&local, &reference);
    assert!(
        out.best_price_diverged,
        "пропавший best bid НЕ распознан как расхождение best — ровно дефект C1, который recon \
         обязан ловить (healthcheck его не видел)"
    );
    assert!(
        out.exceeds_test(),
        "порча best bid обязана превышать ε_test с первой минуты (ε_test не калибруется)"
    );
    let audit = out.to_audit(Venue::Binance, "BTCUSDT", ReconAction::Resynced);
    assert!(
        audit.best_price_diverged,
        "аудит-событие обязано пометить порчу best (CT-RFC-03), иначе офлайн не узнает"
    );
}

/// (МНОЖЕСТВЕННОСТЬ) Много дальних уровней локально занижены → суммы полос расходятся на ≥ ε_test.
#[test]
fn ops_i_1_multiple_far_levels_diverge_by_band_sum() {
    let reference = full_book();
    // local: дальние bid-уровни занижены в 10× (фантомная нехватка ликвидности в полосах).
    let mut local = OrderBook::new();
    let bids: Vec<Level> = (1..=100)
        .map(|k| {
            let size = if k > 5 {
                5 * 10_000_000
            } else {
                5 * 100_000_000
            };
            lvl(MID - k * TICK, size)
        })
        .collect();
    let asks: Vec<Level> = (1..=100)
        .map(|k| lvl(MID + k * TICK, 5 * 100_000_000))
        .collect();
    local.apply_snapshot(&bids, &asks);

    let out = reconcile(&local, &reference);
    assert!(
        out.divergence_bps >= EPS_TEST_BPS,
        "10× занижение сумм дальних полос дало divergence_bps={} < ε_test={} — recon не заметил \
         фантомную ликвидность в полосах OBI",
        out.divergence_bps,
        EPS_TEST_BPS
    );
    assert!(out.exceeds_test());
}

/// (ГРАНИЦА) Пустая локальная книга vs непустой референс → best расходится (нечего сравнивать).
#[test]
fn ops_i_1_empty_local_book_diverges() {
    let local = OrderBook::new();
    let reference = full_book();
    let out = reconcile(&local, &reference);
    assert!(
        out.best_price_diverged && out.exceeds_test(),
        "пустая книга против живого снапшота — максимальная порча, обязана алертить"
    );
}

/// ε_test НЕ калибруется: `exceeds_test` не зависит от рабочего порога. Best-price расхождение
/// поднимает алерт даже при самом мягком ε_prod.
#[test]
fn ops_i_1_eps_test_is_not_calibratable() {
    let reference = full_book();
    let mut local = OrderBook::new();
    let bids: Vec<Level> = (2..=100)
        .map(|k| lvl(MID - k * TICK, 5 * 100_000_000))
        .collect();
    let asks: Vec<Level> = (1..=100)
        .map(|k| lvl(MID + k * TICK, 5 * 100_000_000))
        .collect();
    local.apply_snapshot(&bids, &asks);
    let out = reconcile(&local, &reference);

    // Самый мягкий допустимый ε_prod (= потолок) не отменяет ε_test по best-price.
    let lax = ReconThresholds::new(EPS_MAX_BPS).expect("ε_prod == ε_max допустим");
    assert!(
        out.exceeds_test(),
        "ε_test обязан срабатывать на порче best независимо от ε_prod"
    );
    assert!(
        out.exceeds_prod(&lax),
        "порча best обязана превышать и любой рабочий порог"
    );
}

/// ε_prod ≤ ε_max (fail-closed): нельзя «откалибровать порог до бесконечности».
#[test]
fn ops_i_1_eps_prod_capped_at_max() {
    assert!(
        ReconThresholds::new(EPS_MAX_BPS + 10).is_err(),
        "ε_prod > ε_max ({}) принят — recon можно оглушить, задрав порог (fail-open)",
        EPS_MAX_BPS
    );
    assert!(
        ReconThresholds::new(EPS_PROD_DEFAULT_BPS).is_ok(),
        "дефолтный ε_prod обязан быть допустим"
    );
}
