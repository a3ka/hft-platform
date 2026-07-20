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

use research_cli::report::{classify_verdict, KillScreenInputs, Verdict};

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
