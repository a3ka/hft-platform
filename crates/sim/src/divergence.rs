//! divergence — sim-vs-live (FA §8). Полный paper-цикл — P3/P4; в M-04 живёт
//! gate-checker: P4-ворота ФОРМАЛЬНО требуют отчёт о дивергенции (SM-I-10) —
//! отсутствие отчёта блокирует promotion, это verify-гейт, не телеметрия.
//!
//! Реализация — engine-dev (M-04 task 2).

use serde::{Deserialize, Serialize};

/// T2: {window, fill_rate_delta, pnl_delta} (FA §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DivergenceMetric {
    pub window_ms: i64,
    pub fill_rate_delta: f64,
    pub pnl_delta_e8: i64,
}

/// Допуски фиксируются на выходе P3 (FA §O); тип существует уже сейчас.
#[derive(Debug, Clone, PartialEq)]
pub struct DivergenceTolerance {
    pub max_fill_rate_delta: f64,
    pub max_pnl_delta_e8: i64,
}

#[derive(Debug, PartialEq)]
pub enum GateBlocked {
    /// SM-I-10: нет отчёта → ворота закрыты.
    MissingReport,
    OutOfTolerance {
        fill_rate_delta: f64,
        pnl_delta_e8: i64,
    },
}

/// P4-ворота promotion paper→live. None → Err(MissingReport) БЕЗУСЛОВНО.
pub fn p4_gate(
    report: Option<&DivergenceMetric>,
    tol: &DivergenceTolerance,
) -> Result<(), GateBlocked> {
    let report = report.ok_or(GateBlocked::MissingReport)?;
    if report.fill_rate_delta.abs() > tol.max_fill_rate_delta
        || report.pnl_delta_e8.abs() > tol.max_pnl_delta_e8
    {
        return Err(GateBlocked::OutOfTolerance {
            fill_rate_delta: report.fill_rate_delta,
            pnl_delta_e8: report.pnl_delta_e8,
        });
    }
    Ok(())
}
