//! walkforward — оконный перебор поверх grid (FA §5): серия окон, оценивающая
//! устойчивость сигнала во времени (режимная зависимость — чек-лист 02 §4.4).
//!
//! Реализация — research-dev (M-04 task 4).

use contracts::Event;
use sim::{FeeSchedule, LatencyTable};

use crate::ledger::Ledger;
use crate::types::{GridSpec, RcError, WalkForwardWindow};

#[derive(Debug, Clone, PartialEq)]
pub struct WindowResult {
    pub train_range_ms: (i64, i64),
    pub test_range_ms: (i64, i64),
    /// Sharpe лучшей train-ячейки на СЛЕДУЮЩЕМ (out-of-sample) окне.
    pub oos_sharpe: f64,
}

/// Скользящие окна: на каждом train-окне грид → лучшая ячейка → оценка на следующем
/// окне. Недостаточно данных для окна → окно пропускается с явной пометкой (метрика
/// N/A, не экстраполяция — FA §3 таблица).
pub fn run_walkforward(
    events: &[Event],
    spec: &GridSpec,
    window: &WalkForwardWindow,
    ledger: &mut Ledger,
    latency: &LatencyTable,
    fees: &FeeSchedule,
) -> Result<Vec<WindowResult>, RcError> {
    let _ = (events, spec, window, ledger, latency, fees);
    todo!("research-dev: M-04 task 4")
}
