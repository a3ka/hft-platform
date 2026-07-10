//! metrics — чистые функции над рядами (FA §3 core). Без I/O, без LLM (RC-I-1/11).
//!
//! Реализация — research-dev (M-04 task 4).

use crate::ledger::LedgerTrialCount;

/// Постоянная Эйлера–Маскерони (для формулы D4 SR₀).
const EULER_MASCHERONI: f64 = 0.577_215_664_901_532_9;

/// Годовой Sharpe по ряду пошаговых доходностей (std по n-1; пустой/нулевой std → 0.0).
pub fn sharpe(returns: &[f64], periods_per_year: f64) -> f64 {
    let n = returns.len();
    if n < 2 {
        return 0.0;
    }
    let mean = returns.iter().sum::<f64>() / n as f64;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    let std = var.sqrt();
    if std <= 0.0 {
        return 0.0;
    }
    (mean / std) * periods_per_year.sqrt()
}

/// erf через аппроксимацию Abramowitz & Stegun 7.1.26 (макс. абсолютная ошибка ~1.5e-7).
fn erf(x: f64) -> f64 {
    let sign: f64 = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    const A1: f64 = 0.254_829_592;
    const A2: f64 = -0.284_496_736;
    const A3: f64 = 1.421_413_741;
    const A4: f64 = -1.453_152_027;
    const A5: f64 = 1.061_405_429;
    const P: f64 = 0.327_591_1;
    let t = 1.0 / (1.0 + P * x);
    let y = 1.0 - (((((A5 * t + A4) * t) + A3) * t + A2) * t + A1) * t * (-x * x).exp();
    sign * y
}

/// Φ — стандартная нормальная CDF, через erf выше.
fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Φ⁻¹ — рациональная аппроксимация Acklam (без внешних крейтов).
fn norm_inv_cdf(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    const A: [f64; 6] = [
        -3.969_683_028_665_376e+01,
        2.209_460_984_245_205e+02,
        -2.759_285_104_469_687e+02,
        1.383_577_518_672_69e+02,
        -3.066_479_806_614_716e+01,
        2.506_628_277_459_239e+00,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e+01,
        1.615_858_368_580_409e+02,
        -1.556_989_798_598_866e+02,
        6.680_131_188_771_972e+01,
        -1.328_068_155_288_572e+01,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-03,
        -3.223_964_580_411_365e-01,
        -2.400_758_277_161_838e+00,
        -2.549_732_539_343_734e+00,
        4.374_664_141_464_968e+00,
        2.938_163_982_698_783e+00,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-03,
        3.224_671_290_700_398e-01,
        2.445_134_137_142_996e+00,
        3.754_408_661_907_416e+00,
    ];
    const P_LOW: f64 = 0.024_25;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= P_HIGH {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
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
    let n = trials.n();
    let sr0 = if n < 2 {
        0.0
    } else {
        let n_f = n as f64;
        let std_sr = family_sr_variance.max(0.0).sqrt();
        std_sr
            * ((1.0 - EULER_MASCHERONI) * norm_inv_cdf(1.0 - 1.0 / n_f)
                + EULER_MASCHERONI * norm_inv_cdf(1.0 - 1.0 / (n_f * std::f64::consts::E)))
    };

    let t = t_observations as f64;
    if t <= 1.0 {
        return 0.0;
    }
    let denom_sq = 1.0 - skew * sr + ((kurtosis - 1.0) / 4.0) * sr * sr;
    if denom_sq <= 0.0 {
        return 0.0;
    }
    let z = ((sr - sr0) * (t - 1.0).sqrt()) / denom_sq.sqrt();
    norm_cdf(z).clamp(0.0, 1.0)
}

/// Максимальная просадка equity-ряда (×1e8), ≥ 0.
pub fn max_drawdown_e8(equity_e8: &[i64]) -> i64 {
    let mut peak = i64::MIN;
    let mut max_dd: i64 = 0;
    for &eq in equity_e8 {
        if eq > peak {
            peak = eq;
        }
        let dd = peak.saturating_sub(eq);
        if dd > max_dd {
            max_dd = dd;
        }
    }
    max_dd.max(0)
}

pub fn fill_rate(fills: usize, intents: usize) -> f64 {
    if intents == 0 {
        0.0
    } else {
        fills as f64 / intents as f64
    }
}

/// Capacity v1 (D5): participation_cap × медиана(traded notional за горизонт), ×1e8.
/// Чётное число элементов — нижняя из двух средних (простая, воспроизводимая конвенция).
pub fn capacity_v1_e8(traded_notional_per_horizon_e8: &mut [i64], participation_cap: f64) -> i64 {
    if traded_notional_per_horizon_e8.is_empty() {
        return 0;
    }
    traded_notional_per_horizon_e8.sort_unstable();
    let n = traded_notional_per_horizon_e8.len();
    let median = if n % 2 == 1 {
        traded_notional_per_horizon_e8[n / 2]
    } else {
        traded_notional_per_horizon_e8[n / 2 - 1]
    };
    (median as f64 * participation_cap) as i64
}
