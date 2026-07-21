//! RED — ЧЕСТНОСТЬ СТЕКА КАК ИЗМЕРИТЕЛЬНОГО ИНСТРУМЕНТА (sacred, architect-only).
//! Находки risk-critic C-020 B/C/D. Цель R-001 (Objective M-10) — «первое сквозное испытание
//! стека». Прогон вскрыл, что абсолютные величины отчёта нефизичны: стек как ИЗМЕРИТЕЛЬ ещё не
//! провалидирован. Для Kill сигнала это не важно (Sharpe масштаб-инвариантен), но ДО того как
//! отчёту доверят НЕ-Kill вывод, эти дефекты обязаны быть закрыты. Все — компайл-RED против
//! текущего impl (новые символы отсутствуют), анти-плацебо в обе стороны.
//!
//! Контракт (research-dev impl):
//!   B: `report::fill_probability(filled_intents, intents) -> f64` ∈ [0,1] (честная вероятность);
//!      поле отчёта `fill_rate` (=fills/intents, может быть >1) ПЕРЕИМЕНОВАНО в `fills_per_intent`.
//!   C: `report::validate_sizing_honesty(sizing_applied: bool, capacity_notional_e8: i64) -> Result<(),String>`
//!      — при `!sizing_applied` положительная capacity ЗАПРЕЩЕНА (unsized прогон не измеряет ёмкость
//!      для аккаунта $500–2k; §4 лимиты не применены). Отчёт несёт `sizing_applied`.
//!   D: `metrics::robust_family_variance(&[f64]) -> f64` — устойчива к ОДНОМУ экстремальному выбросу
//!      (winsorize/trim/MAD); `deflated_sharpe` берёт ЕЁ, а не наивную `variance(&family_sharpes)`.

use research_cli::metrics::robust_family_variance;
use research_cli::report::{fill_probability, validate_sizing_honesty};

// ── B: fill-МЕТРИКА честна по имени и диапазону (C-020 B) ───────────────────────────────────────
// Прод-факт: отчёт нёс `fill_rate = 1.99` (>1) — «доля исполнения» физически ∈[0,1]. По коду это
// fills/intents (≈2 = вход+выход). Пре-рег критерий №5 ссылается на ВЕРОЯТНОСТЬ исполнения → метрика
// измеряла НЕ ТО. Честная вероятность обязана быть ∈[0,1] по построению (filled_intents ≤ intents).

#[test]
fn b_fill_probability_is_in_unit_interval() {
    assert_eq!(
        fill_probability(0, 0),
        0.0,
        "нет интентов → вероятность 0, не NaN/деление на ноль"
    );
    assert!(
        (fill_probability(5, 10) - 0.5).abs() < 1e-12,
        "5 из 10 интентов исполнены → вероятность 0.5"
    );
    assert!(
        (0.0..=1.0).contains(&fill_probability(10, 10)),
        "все интенты исполнены → вероятность 1.0, НЕ выше"
    );
    // Прод-режим значение (реальные числа R-001): даже при 1.99 fills/intent вероятность ≤ 1.
    for (filled, intents) in [(3usize, 7usize), (7, 7), (1, 1000), (0, 5)] {
        let p = fill_probability(filled, intents);
        assert!(
            (0.0..=1.0).contains(&p),
            "fill_probability({filled},{intents})={p} вне [0,1] — метрика с именем «вероятность/rate» \
             ОБЯЗАНА быть в единичном интервале (C-020 B); fills/intents=1.99 — это НЕ вероятность"
        );
    }
}

// ── C: unsized прогон не смеет заявлять ёмкость (C-020 C) ─────────────────────────────────────────
// Прод-факт: capacity_notional = $508M, turnover = $10B на аккаунт $500–2k — sim гонял БЕЗ
// max_position/max_order (§4 DESIGN). Для Kill Sharpe масштаб-инвариантен (вывод верен), но поле
// capacity вводит в заблуждение. Решение architect: R-001 kill-screen идёт UNSIZED (профиль лимитов
// — зона M-11/M-12 risk/portfolio); отчёт ОБЯЗАН это назвать и НЕ выдавать нефизичную ёмкость.

const R001_CAPACITY_E8: i64 = 50_794_443_807_313_248; // реальное поле прогона (нефизично для $500–2k)

#[test]
fn c_unsized_run_must_not_claim_capacity() {
    assert!(
        validate_sizing_honesty(false, R001_CAPACITY_E8).is_err(),
        "unsized прогон (sizing_applied=false) с положительной capacity вводит в заблуждение: он измеряет \
         безлимитный оборот, а не ёмкость для наших размеров. При !sizing_applied capacity обязана быть N/A (0)"
    );
    assert!(
        validate_sizing_honesty(false, 0).is_ok(),
        "unsized прогон с capacity=0 (явно N/A) — честно"
    );
    assert!(
        validate_sizing_honesty(true, R001_CAPACITY_E8).is_ok(),
        "sized прогон (лимиты §4 применены) ВПРАВЕ заявлять ёмкость — тогда число физично"
    );
}

// ── D: V[SR] семейства робастна к выбросу (C-020 D, форвардный риск) ──────────────────────────────
// Прогон дописал в ledger obi-испытания с Sharpe до −226.79. `deflated_sharpe` берёт
// variance(family_sharpes) (report.rs) → выброс раздувает V[SR] → раздувает SR₀ → будущие DSR
// занижены. Здесь роли не играет (raw SR<0 → DSR=0), но всплывёт на первой ячейке SR>0.

#[test]
fn d_family_variance_is_robust_to_single_extreme_outlier() {
    // Штатное семейство (тесное), затем тот же ряд + ОДИН реальный выброс −226.79.
    let tight = vec![
        -1.0, -1.2, -0.9, -1.1, -1.05, -0.95, -1.3, -0.8, -1.15, -0.85,
    ];
    let with_outlier = {
        let mut v = tight.clone();
        v.push(-226.79); // реальное значение из прогона (прод-режим, не выдуманное)
        v
    };

    let vr_tight = robust_family_variance(&tight);
    let vr_out = robust_family_variance(&with_outlier);

    assert!(
        vr_tight.is_finite() && vr_tight >= 0.0,
        "робастная дисперсия конечна и ≥0"
    );
    // ОДИН выброс не смеет раздуть робастную V[SR] более чем в ~2× (winsorize/trim/MAD его гасит).
    // Наивная variance тут выросла бы на ПОРЯДКИ (mean уезжает к −21, var ~4500) — анти-плацебо:
    // реализация robust_family_variance = наивная variance ЗДЕСЬ ПАДАЕТ.
    assert!(
        vr_out <= vr_tight * 2.0 + 0.05,
        "робастная V[SR] раздулась от ОДНОГО выброса −226.79: tight={vr_tight}, с выбросом={vr_out}. \
         Наивная variance раздувает SR₀ → будущие deflated-Sharpe искусственно занижены (C-020 D). \
         Нужна winsorize/trim/MAD-устойчивая оценка"
    );
}

#[test]
fn d_robust_variance_still_tracks_real_spread() {
    // Анти-плацебо со стороны «всегда 0»: робастная дисперсия не смеет быть тождественным нулём —
    // на реально разбросанном (без выбросов) семействе она положительна.
    let spread = vec![0.2, -0.4, 0.9, -0.7, 0.5, -0.3, 0.8, -0.6, 0.1, -0.9];
    let vr = robust_family_variance(&spread);
    assert!(
        vr > 0.0,
        "робастная дисперсия реально разбросанного семейства = {vr}; тождественный ноль = заглушка, \
         которая убила бы поправку deflated-Sharpe (оверфит-защита исчезла)"
    );
}
