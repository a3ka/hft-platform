//! metrics — чистые функции над рядами (FA §3 core). Без I/O, без LLM (RC-I-1/11).
//!
//! Реализация — research-dev (M-04 task 4).

use crate::ledger::LedgerTrialCount;

/// Годовой Sharpe по ряду пошаговых доходностей.
pub fn sharpe(returns: &[f64], periods_per_year: f64) -> f64 {
    let _ = (returns, periods_per_year);
    todo!("research-dev: M-04 task 4")
}

/// Deflated Sharpe (D4, Bailey & López de Prado 2014):
/// DSR = Φ(((SR−SR₀)·√(T−1)) / √(1−γ₃·SR+((γ₄−1)/4)·SR²)),
/// SR₀ = √(V[SR_family])·((1−γ)·Φ⁻¹(1−1/N)+γ·Φ⁻¹(1−1/(N·e))), γ — Эйлера–Маскерони.
/// N — СТРОГО из ledger'а (тип LedgerTrialCount не конструируем извне, RC-I-3);
/// family_sr_variance — V[SR] по trial-записям семейства; N<2 → SR₀=0 (PSR-вырождение).
pub fn deflated_sharpe(
    sr: f64,
    t_observations: usize,
    skew: f64,
    kurtosis: f64,
    trials: &LedgerTrialCount,
    family_sr_variance: f64,
) -> f64 {
    let _ = (
        sr,
        t_observations,
        skew,
        kurtosis,
        trials,
        family_sr_variance,
    );
    todo!("research-dev: M-04 task 4")
}

/// Максимальная просадка equity-ряда (×1e8), ≥ 0.
pub fn max_drawdown_e8(equity_e8: &[i64]) -> i64 {
    let _ = equity_e8;
    todo!("research-dev: M-04 task 4")
}

pub fn fill_rate(fills: usize, intents: usize) -> f64 {
    let _ = (fills, intents);
    todo!("research-dev: M-04 task 4")
}

/// Capacity v1 (D5): participation_cap × медиана(traded notional за горизонт), ×1e8.
pub fn capacity_v1_e8(
    traded_notional_per_horizon_e8: &mut Vec<i64>,
    participation_cap: f64,
) -> i64 {
    let _ = (traded_notional_per_horizon_e8, participation_cap);
    todo!("research-dev: M-04 task 4")
}
