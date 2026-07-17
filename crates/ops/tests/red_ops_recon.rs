//! RED OPS-I-1 (sacred, architect-only) — сверка книги с REST-снапшотом ловит порчу данных.
//!
//! Это ЕДИНСТВЕННЫЙ оракул класса, который поймал бы C1 (эвикция стёрла best bid при зелёном
//! healthcheck). Чек-лист `.claude/rules/testing.md` — деградированные входы, НЕ «счастливый путь»:
//!  • асимметрия: порча ТОЛЬКО одной стороны;
//!  • отсутствие: пропавший уровень (best bid) — это порча, а не «дифф молчит»;
//!  • множественность: несколько уровней разошлись;
//!  • границы: пустая книга / один уровень.
//!
//! NEAR-BOOK семантика (redesign 2026-07-17, founder ★): REST достаёт лишь ~1.1% от mid → recon
//! валидирует БЛИЖНЮЮ книгу мелкими полосами (`RECON_BANDS` ≤0.8%), полосы за пределами
//! reference-reach ПРОПУСКАЕТ. best_price толерантен к sub-bp timing-skew. Поэтому фикстуры здесь
//! достают до полос (уровни на 0.05–0.55% от mid), НЕ ±100 тиков — иначе всё за пределами полос.
//! Пороговая машина (`ReconThresholds`, ε_test/ε_prod/ε_max) — полосонезависима.
//! Анти-плацебо: против `todo!()` все падают; против «reconcile всегда без расхождения» падают все,
//! кроме identity; против «best всегда diverged» падает identity.

use book::OrderBook;
use contracts::{Level, ReconAction, Venue};
use ops::recon::{reconcile, ReconThresholds, EPS_MAX_BPS, EPS_PROD_DEFAULT_BPS};

const MID: i64 = 65_000_000_000_000; // $65k ×1e8
const UNIT: i64 = 100_000_000; // 1.0 ×1e8

fn bid_at(pct: f64, size_units: i64) -> Level {
    Level {
        price: (MID as f64 * (1.0 - pct)) as i64,
        size: size_units * UNIT,
    }
}
fn ask_at(pct: f64, size_units: i64) -> Level {
    Level {
        price: (MID as f64 * (1.0 + pct)) as i64,
        size: size_units * UNIT,
    }
}

/// Уровни на 0.05..0.55% от mid, объём 5.0 — достаёт до полос recon (0.1/0.3/0.5%), reach≈0.55%.
const PCTS: [f64; 6] = [0.0005, 0.0015, 0.0025, 0.0035, 0.0045, 0.0055];

fn full_book() -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = PCTS.iter().map(|&p| bid_at(p, 5)).collect();
    let asks: Vec<Level> = PCTS.iter().map(|&p| ask_at(p, 5)).collect();
    b.apply_snapshot(&bids, &asks);
    b
}

/// (identity) Локальная книга == REST-референс → расхождения НЕТ, алерта НЕТ.
#[test]
fn ops_i_1_identical_books_do_not_alert() {
    let out = reconcile(&full_book(), &full_book());
    assert!(
        !out.best_price_diverged,
        "идентичные книги не расходятся по best"
    );
    assert!(
        !out.exceeds_test(),
        "идентичные книги подняли ложный алерт — recon будет кричать на здоровые данные"
    );
}

/// (АСИММЕТРИЯ + ОТСУТСТВИЕ, C1-класс) В локальной книге пропал BEST BID (эвикция стёрла), ask цел.
/// best уходит на следующий уровень (>skew) И near-touch полоса теряет объём → алерт ВСЕГДА (ε_test).
#[test]
fn ops_i_1_missing_best_bid_must_alert() {
    let reference = full_book();
    // local без ближайшего к mid бида (0.05% уровень удалён) — ask нетронут.
    let mut local = OrderBook::new();
    let bids: Vec<Level> = PCTS.iter().skip(1).map(|&p| bid_at(p, 5)).collect();
    let asks: Vec<Level> = PCTS.iter().map(|&p| ask_at(p, 5)).collect();
    local.apply_snapshot(&bids, &asks);

    let out = reconcile(&local, &reference);
    assert!(
        out.exceeds_test(),
        "пропавший best bid НЕ поднял алерт (divergence_bps={}, best={}) — ровно дефект C1, который \
         recon обязан ловить (healthcheck его не видел)",
        out.divergence_bps,
        out.best_price_diverged
    );
    let audit = out.to_audit(Venue::Binance, "BTCUSDT", ReconAction::Resynced);
    assert!(
        audit.divergence_bps > 0 || audit.best_price_diverged,
        "аудит-событие не отразило порчу (CT-RFC-03), иначе офлайн не узнает"
    );
}

// (МНОЖЕСТВЕННОСТЬ/ОБЪЁМ) 10× занижение near-book сумм — это ПЕРСИСТЕНТНЫЙ ОБЪЁМНЫЙ дефицит →
// ПЕРЕЕХАЛ В ОКОННЫЙ ДЕТЕКТОР (второй §8-провал: per-cycle объём churn'ит). Оракул —
// `red_recon_window::persistent_volume_deficit_alerts`. Здесь (reconcile, per-cycle) остаётся
// BEST-ONLY ε_test: `divergence_bps` считается как гейдж, но эмиссию по объёму принимает окно.

/// (ГРАНИЦА) Пустая локальная книга vs непустой референс → best расходится (нечего сравнивать).
#[test]
fn ops_i_1_empty_local_book_diverges() {
    let out = reconcile(&OrderBook::new(), &full_book());
    assert!(
        out.best_price_diverged && out.exceeds_test(),
        "пустая книга против живого снапшота — максимальная порча, обязана алертить"
    );
}

/// ε_test НЕ калибруется: `exceeds_test` не зависит от рабочего порога. Порча near-book поднимает
/// алерт даже при самом мягком ε_prod (= потолок).
#[test]
fn ops_i_1_eps_test_is_not_calibratable() {
    let reference = full_book();
    let mut local = OrderBook::new();
    let bids: Vec<Level> = PCTS.iter().skip(1).map(|&p| bid_at(p, 5)).collect();
    let asks: Vec<Level> = PCTS.iter().map(|&p| ask_at(p, 5)).collect();
    local.apply_snapshot(&bids, &asks);
    let out = reconcile(&local, &reference);

    let lax = ReconThresholds::new(EPS_MAX_BPS).expect("ε_prod == ε_max допустим");
    assert!(
        out.exceeds_test(),
        "ε_test обязан срабатывать на порче near-book независимо от ε_prod"
    );
    assert!(
        out.exceeds_prod(&lax),
        "порча near-book обязана превышать и любой рабочий порог"
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
