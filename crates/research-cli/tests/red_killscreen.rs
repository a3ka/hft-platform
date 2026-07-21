//! RED KS-I-* KILL-SCREEN ЧЕСТНОСТЬ (sacred, architect-only) — M-10 R-001. `docs/02-quant-desk.md` §4.
//!
//! R-001 — первое сквозное испытание research-стека. На коротком окне данных точечный Sharpe
//! недостоверен (SE годового Sharpe ≈ ±11 на 3–7 днях). Отчёт НЕ СМЕЕТ заявить промоушабельный
//! сигнал на шуме — это тот же анти-плацебо класс, что 7 раундов TD-027, но для ВЫВОДА research'а.
//!
//! Контракт (research-dev impl): `research_cli::report::{Verdict, KillScreenInputs, classify_verdict}`:
//!   `classify_verdict(&KillScreenInputs, bar: f64) -> Verdict { Kill(String) | Inconclusive(String) | Pass }`
//! Логика (порядок важен):
//!   1. сработал ЛЮБОЙ пре-рег критерий фальсификации (H-20260710) → `Kill(reason)`:
//!      oos_sharpe ≤ 0.5 | deflated_sharpe ≤ 0 | walkforward_min_sharpe < 0 (нестабилен) |
//!      half_life_ms < horizon_ms (decay) | worst_stress_net_pnl_e8 < 0 (fill-PnL под ×1.5/×2);
//!   2. иначе `sharpe − 2·se_sharpe > bar` (нижняя 95%-граница выше бара) → `Pass`;
//!   3. иначе → `Inconclusive`.
//!
//! Анти-плацебо В ОБЕ СТОРОНЫ:
//!  - impl, штампующий `Pass` игнорируя `se_sharpe` (шумный SR) → валит KS-I-1a;
//!  - impl «всё Inconclusive» → валит KS-I-4* (реальный Kill-критерий) И KS-I-1b (достижимый Pass);
//!  - impl «всё Kill» → валит KS-I-1b.
//!
//! ВТОРОЙ контракт (KS-I-5/KS-I-3, C-019 B2/B3 — ЧЕСТНОСТЬ ОТЧЁТА, отдельно от классификатора):
//!   `research_cli::report::{ReportHonesty, validate_report_honesty}`:
//!   `validate_report_honesty(&ReportHonesty) -> Result<(), String>`.
//! Вердикт классификатора БЕЗ доказательства честности окна и эпохи ledger'а НЕДОСТОВЕРЕН.
//! Валидатор УЗКИЙ (не часть `KillScreenInputs`): gap-статистика (E8) и эпоха ledger'а (TD-015)
//! генерируются ВНЕ `classify_verdict`, поэтому гейтятся отдельным report-level оракулом.
//! Правила (research-dev обязан завести их и звать ПЕРЕД записью R-001):
//!   - `gap_ref` пуст → `Err` (KS-I-5: вердикт без честности окна = ложь);
//!   - `ledger_cutoff` пуст ИЛИ = пре-M-07 эпохе `f7f4761` → `Err` (TD-015/KS-I-3: несопоставимо);
//!   - `data_span_days` не конечен / ≤ 0 → `Err`; `se_sharpe` не конечен / < 0 → `Err`.
//!
//! Анти-плацебо В ОБЕ СТОРОНЫ: валидатор «всегда Ok» валит err-тесты; «всегда Err» валит
//! `honest_report_is_reachable_valid` (полностью честный отчёт обязан проходить).

use research_cli::report::{
    classify_verdict, sharpe_se, validate_report_honesty, KillScreenInputs, ReportHonesty, Verdict,
};

/// Бар нижней CI-границы Sharpe (KS-I-1) — фикс, не калибруется (согласован с пре-рег «≤0.5 → мёртв»).
const BAR: f64 = 0.5;

/// Здоровые (не-Kill) значения пре-рег критериев; конкретный тест переопределяет нужные поля.
fn healthy() -> KillScreenInputs {
    KillScreenInputs {
        sharpe: 2.0,
        se_sharpe: 0.3,
        data_span_days: 120.0,
        oos_sharpe: 1.2,             // > 0.5
        deflated_sharpe: 0.8,        // > 0
        walkforward_min_sharpe: 0.6, // ≥ 0 (стабилен)
        half_life_ms: 5_000,         // ≥ horizon
        horizon_ms: 1_000,
        worst_stress_net_pnl_e8: 10_000_000, // > 0
    }
}

// ── KS-I-4: пре-рег критерий сработал → Kill (машинно, не «на глаз») ───────────────────────────

#[test]
fn ks_i_4_low_oos_sharpe_is_killed() {
    let mut i = healthy();
    i.oos_sharpe = 0.3; // ≤ 0.5 → пре-рег «мёртв»
    assert!(
        matches!(classify_verdict(&i, BAR), Verdict::Kill(_)),
        "oos_sharpe=0.3 (≤0.5, пре-рег критерий) НЕ дал Kill — критерий фальсификации не применён \
         машинно (KS-I-4). Отчёт обязан УБИВАТЬ по пре-рег числам, а не звать это Inconclusive"
    );
}

#[test]
fn ks_i_4_nonpositive_deflated_is_killed() {
    let mut i = healthy();
    i.deflated_sharpe = -0.1; // ≤ 0 → мёртв (перебор съел edge)
    assert!(
        matches!(classify_verdict(&i, BAR), Verdict::Kill(_)),
        "deflated_sharpe=-0.1 (≤0, поправка на trials-ledger) НЕ дал Kill — оверфит не отсечён (KS-I-4)"
    );
}

#[test]
fn ks_i_4_negative_stress_pnl_is_killed() {
    let mut i = healthy();
    i.worst_stress_net_pnl_e8 = -5_000_000; // отрицательный PnL под ×1.5-издержки/×2-латентность
    assert!(
        matches!(classify_verdict(&i, BAR), Verdict::Kill(_)),
        "отрицательный net-PnL под стрессом (×1.5/×2) НЕ дал Kill — сигнал не выживает честные издержки \
         (пре-рег критерий fill-PnL, KS-I-4)"
    );
}

#[test]
fn ks_i_4_decay_faster_than_horizon_is_killed() {
    let mut i = healthy();
    i.half_life_ms = 400;
    i.horizon_ms = 1_000; // полураспад < горизонта удержания → edge выеден раньше выхода
    assert!(
        matches!(classify_verdict(&i, BAR), Verdict::Kill(_)),
        "half-life (400мс) < horizon (1000мс) НЕ дал Kill — decay-критерий не применён (KS-I-4)"
    );
}

// ── KS-I-1: PASS запрещён без нижней CI-границы над баром (шумный SR ≠ живой сигнал) ────────────

#[test]
fn ks_i_1a_high_sharpe_wide_se_is_not_pass() {
    let mut i = healthy();
    i.sharpe = 3.0;
    i.se_sharpe = 5.0; // короткое окно → огромная SE: sharpe−2·se = 3−10 = −7 ≤ BAR
    i.data_span_days = 5.0; // 3–7 дней — kill-screen территория
    let v = classify_verdict(&i, BAR);
    assert!(
        !matches!(v, Verdict::Pass),
        "sharpe=3.0, но se=5.0 (нижняя CI-граница −7 ≤ BAR=0.5, окно 5 дней) → классификатор дал Pass \
         (`{v:?}`). Это ложный промоушен на ШУМЕ — ровно то, что kill-screen обязан запретить: PASS \
         требует sharpe−2·se > BAR (KS-I-1). Impl, игнорирующий se, тут падает"
    );
    assert!(
        matches!(v, Verdict::Inconclusive(_)),
        "здоровые пре-рег критерии + недостоверный SR → обязан быть Inconclusive, а не Kill/Pass"
    );
}

#[test]
fn ks_i_1b_high_sharpe_tight_se_is_reachable_pass() {
    let mut i = healthy();
    i.sharpe = 3.0;
    i.se_sharpe = 0.5; // достаточно данных: sharpe−2·se = 3−1 = 2 > BAR=0.5
    i.data_span_days = 400.0;
    assert!(
        matches!(classify_verdict(&i, BAR), Verdict::Pass),
        "sharpe=3.0, se=0.5 (нижняя CI-граница 2.0 > BAR, окно 400 дней), все пре-рег критерии \
         здоровы → Pass ДОСТИЖИМ. Оракул валит impl «всё Inconclusive/Kill» (Pass обязан быть \
         возможен на хороших+обильных данных, иначе kill-screen — не гейт, а заглушка)"
    );
}

#[test]
fn ks_i_1_kill_precedes_pass() {
    // Даже при формально проходящей CI-границе, сработавший Kill-критерий имеет ПРИОРИТЕТ.
    let mut i = healthy();
    i.sharpe = 3.0;
    i.se_sharpe = 0.5; // CI-граница прошла бы Pass...
    i.deflated_sharpe = -0.2; // ...но deflated ≤ 0 → мёртв
    assert!(
        matches!(classify_verdict(&i, BAR), Verdict::Kill(_)),
        "Kill-критерий (deflated ≤0) обязан иметь приоритет над Pass — иначе оверфитнутый сигнал \
         с узкой CI проскочит в Pass (порядок KS-I-4 → KS-I-1)"
    );
}

// ── Детерминизм (чистая функция) ──────────────────────────────────────────────────────────────

#[test]
fn classify_verdict_is_deterministic() {
    let i = healthy();
    let a = format!("{:?}", classify_verdict(&i, BAR));
    let b = format!("{:?}", classify_verdict(&i, BAR));
    assert_eq!(
        a, b,
        "classify_verdict недетерминирована — вердикт research'а обязан быть воспроизводим"
    );
}

// ── KS-I-5 / KS-I-3: ЧЕСТНОСТЬ ОТЧЁТА (C-019 B2/B3) ─────────────────────────────────────────────
// Отдельный, УЗКИЙ валидатор: gap-статистика окна (E8) и эпоха ledger'а (TD-015) не входят в
// `classify_verdict`, поэтому гейтятся здесь. Прод-РЕЖИМ значения (урок TD-031): `ledger_cutoff`
// сравнивается с РЕАЛЬНЫМ пре-M-07 хэшем `f7f4761` (фикстура не выдумывает удобный хэш).

/// Полностью честный отчёт; конкретный тест портит ОДНО поле.
fn honest_report() -> ReportHonesty {
    ReportHonesty {
        data_span_days: 11.0, // реальное окно own-capture (2026-07-10..21)
        se_sharpe: 3.4,       // на 11 днях SE большая — честно
        gap_ref: "research/data-quality/gaps-own-2026-07.json".to_string(),
        ledger_cutoff: "5141fd9".to_string(), // эпоха «strategy brain» (TD-015 граница)
    }
}

#[test]
fn ks_i_5_empty_gap_ref_is_invalid() {
    // Отсутствие: gap_ref не задан → вердикт без честности окна (E8) недостоверен.
    let mut r = honest_report();
    r.gap_ref = String::new();
    assert!(
        validate_report_honesty(&r).is_err(),
        "отчёт БЕЗ gap_ref (E8 gap-артефакт тестового окна) обязан быть НЕВАЛИДЕН — KS-I-5: вердикт \
         не смеет молчать о дырах в данных, на которых он посчитан. Валидатор-заглушка «всегда Ok» тут падает"
    );
}

#[test]
fn ks_i_3_pre_m07_ledger_epoch_is_invalid() {
    // Прод-режим значение: РЕАЛЬНЫЙ пре-M-07 хэш, а не выдуманный (TD-015 дискриминатор).
    let mut r = honest_report();
    r.ledger_cutoff = "f7f4761".to_string();
    assert!(
        validate_report_honesty(&r).is_err(),
        "ledger_cutoff = пре-M-07 эпоха f7f4761 (меряла ЛОГИКУ, КОТОРОЙ НЕТ — TD-015) обязан ВАЛИТЬ \
         валидацию: смешение эпох в deflated-Sharpe = ложный ориентир для подписи founder'а (KS-I-3)"
    );
}

#[test]
fn ks_i_3_empty_ledger_cutoff_is_invalid() {
    // Отсутствие: отчёт обязан НАЗВАТЬ эпоху (KS-I-3), молчание = невалиден.
    let mut r = honest_report();
    r.ledger_cutoff = String::new();
    assert!(
        validate_report_honesty(&r).is_err(),
        "отчёт БЕЗ названной эпохи ledger'а (пустой ledger_cutoff) невалиден — KS-I-3 требует, чтобы \
         отчёт САМ называл диапазон записей, по которым посчитан deflated-Sharpe"
    );
}

#[test]
fn ks_i_5_nonfinite_span_or_negative_se_is_invalid() {
    // Границы: span должен быть конечным и > 0; se — конечным и ≥ 0.
    let mut r = honest_report();
    r.data_span_days = f64::NAN;
    assert!(
        validate_report_honesty(&r).is_err(),
        "data_span_days = NaN (окно неизвестно) обязан валить — без длины окна SE-достоверность неоценима"
    );
    let mut r2 = honest_report();
    r2.se_sharpe = -1.0;
    assert!(
        validate_report_honesty(&r2).is_err(),
        "se_sharpe < 0 бессмысленна (стандартная ошибка неотрицательна) — отчёт с ней невалиден"
    );
}

#[test]
fn honest_report_is_reachable_valid() {
    // Достижимость: полностью честный отчёт ОБЯЗАН проходить, иначе валидатор «всегда Err» —
    // заглушка, а не гейт (анти-плацебо со стороны Ok).
    assert!(
        validate_report_honesty(&honest_report()).is_ok(),
        "полностью честный отчёт (gap_ref задан, эпоха ≥5141fd9, span/se конечны) обязан быть ВАЛИДЕН"
    );
}

// ── KS-I-1 ЯДРО: se_sharpe ПРИВЯЗАН К КАЛЕНДАРНОМУ ОКНУ, не к числу шагов (C-020 A, БЛОКЕР) ─────
// Находка risk-critic C-020 A (HIGH): `sharpe_se(returns, sharpe)` считал se от returns.len()
// (~1.4e4 шагов) → se=0.27 на окне 0.353 дня → гейт KS-I-1 (sharpe−2·se>BAR) DEFEATED: ложный PASS
// достижим на 8-часовом окне (sharpe>~1.04). Это ровно исход, объявленный «АРХИТЕКТУРНО ЗАПРЕЩЁН».
// Здесь Kill не дал ему проявиться (oos<0), но на положительном сигнале защита не сработала бы.
//
// КОНТРАКТ (research-dev impl): сигнатура МЕНЯЕТСЯ — se зависит от КАЛЕНДАРНОГО span, не от шагов:
//   `pub fn sharpe_se(sharpe: f64, data_span_days: f64) -> f64`
// Масштаб задан ПРЕМИССОЙ самого milestone'а («SE годового Sharpe ≈ ±11 на 3–7 днях»):
//   se ≈ sqrt(DAYS_PER_YEAR / data_span_days · (1 + 0.5·sharpe²/ppy)) ⇒ на 3д ≈11, на 7д ≈7, на 0.35д ≈32.
// Точную константу (252 trading / 365 календарь; форма SR-члена) выбирает research-dev и ДОКУМЕНТИРУЕТ;
// оракул пиннит МАСШТАБ и ПОВЕДЕНИЕ, а не константу. Компайл-RED против старой сигнатуры (returns).
//
// Прод-режим значение (урок TD-031): фикстуры используют РЕАЛЬНОЕ окно R-001 (0.353 дня), где дефект
// и проявился, а не «удобное» большое окно, которое замаскировало бы step-count реализацию.

/// Здоровые пре-рег критерии + заданные sharpe/se — чтобы Kill не преempt'ил KS-I-1.
fn healthy_with(sharpe: f64, se_sharpe: f64, span: f64) -> KillScreenInputs {
    KillScreenInputs {
        sharpe,
        se_sharpe,
        data_span_days: span,
        ..healthy()
    }
}

#[test]
fn a1_se_is_huge_on_short_window() {
    // Реальное окно R-001: 0.353 дня. Любая календарная формула даёт se ~27..40; step-count → 0.27.
    let se = sharpe_se(1.0, 0.353);
    assert!(
        se > 10.0,
        "se на окне 0.353 дня = {se} (ожидается ~30). Значение <10 означает привязку к числу ШАГОВ \
         (returns.len), а не к КАЛЕНДАРНОМУ окну — KS-I-1 defeated (C-020 A). se=0.27 тут падает"
    );
}

#[test]
fn a2_se_matches_milestone_premise_on_days() {
    // Премисса milestone: «SE годового Sharpe ≈ ±11 на 3–7 днях». Пиннит масштаб к дням.
    let se3 = sharpe_se(1.0, 3.0);
    let se7 = sharpe_se(1.0, 7.0);
    assert!(
        (5.0..=25.0).contains(&se3),
        "se на 3 днях = {se3}, ожидается порядок ±11 (премисса milestone). Вне [5,25] → масштаб сломан"
    );
    assert!(
        (3.0..=20.0).contains(&se7),
        "se на 7 днях = {se7}, ожидается порядок ±7. Вне [3,20] → масштаб сломан"
    );
}

#[test]
fn a3_se_decreases_monotonically_with_span() {
    // Больше КАЛЕНДАРНЫХ данных → теснее оценка. Step-count реализация этого не гарантирует
    // (при равномерной частоте returns.len ∝ span, но при неравномерной — нет; календарь — гарантирует).
    let s0 = sharpe_se(1.0, 0.353);
    let s1 = sharpe_se(1.0, 3.0);
    let s2 = sharpe_se(1.0, 30.0);
    let s3 = sharpe_se(1.0, 365.0);
    assert!(
        s0 > s1 && s1 > s2 && s2 > s3,
        "se обязан УБЫВАТЬ с календарным окном: {s0} > {s1} > {s2} > {s3} — иначе окно не влияет на достоверность"
    );
}

#[test]
fn a4_se_is_tight_on_long_window() {
    // 10 лет данных → se мал → Pass достижим честно.
    let se = sharpe_se(1.0, 3650.0);
    assert!(
        se < 1.0,
        "se на 10-летнем окне = {se}, ожидается <1 (данных много) — иначе Pass недостижим НИКОГДА"
    );
}

#[test]
fn a5_short_window_defeats_pass_long_window_reaches_it() {
    // ИНТЕГРАЦИЯ с classify_verdict: честный se закрывает ложный PASS на коротком окне
    // и оставляет Pass достижимым на длинном (иначе kill-screen — заглушка, а не гейт).
    let short = healthy_with(3.0, sharpe_se(3.0, 0.353), 0.353);
    assert!(
        !matches!(classify_verdict(&short, BAR), Verdict::Pass),
        "sharpe=3 на окне 0.353 дня при ЧЕСТНОМ se не смеет дать Pass — на 8 часах годовой SR это шум (KS-I-1)"
    );
    let long = healthy_with(3.0, sharpe_se(3.0, 3650.0), 3650.0);
    assert!(
        matches!(classify_verdict(&long, BAR), Verdict::Pass),
        "sharpe=3 на 10-летнем окне при честном se ОБЯЗАН давать Pass — иначе honest se сделал kill-screen \
         неспособным к Pass вообще (заглушка). Достижимость Pass — анти-плацебо со стороны «всё зарубить»"
    );
}
