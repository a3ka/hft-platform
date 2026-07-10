//! walkforward — оконный перебор поверх grid (FA §5): серия окон, оценивающая
//! устойчивость сигнала во времени (режимная зависимость — чек-лист 02 §4.4).
//!
//! Реализация — research-dev (M-04 task 4).

use contracts::Event;
use sim::{FeeSchedule, LatencyTable};

use crate::grid::{run_grid, top_k, GridRunEnv};
use crate::ledger::Ledger;
use crate::types::{GridSpec, RcError, SplitKind, WalkForwardWindow};

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
///
/// Оба под-прогона (train-грид и oos-оценка лучшей ячейки) используют
/// `SplitKind::Train`/`SplitKind::Val` — окна walk-forward являются ВНУТРЕННЕЙ
/// оценкой устойчивости (режимная зависимость, 02 §4.4), а не глобальным
/// hold-out test-сегментом гипотезы (тот гейтится отдельно через `split::SplitState`
/// + `&ValGateToken`, RC-I-8). Каждый под-прогон журналирует свои ячейки (FA §5/§6).
pub fn run_walkforward(
    events: &[Event],
    spec: &GridSpec,
    window: &WalkForwardWindow,
    ledger: &mut Ledger,
    latency: &LatencyTable,
    fees: &FeeSchedule,
) -> Result<Vec<WindowResult>, RcError> {
    if events.is_empty() {
        return Ok(Vec::new());
    }
    let max_ts = events.iter().map(|e| e.ts_wall_ms).max().unwrap_or(0);
    let mut train_start = events.iter().map(|e| e.ts_wall_ms).min().unwrap_or(0);

    let mut results = Vec::new();

    while train_start + window.train_window_ms + window.test_window_ms <= max_ts + 1 {
        let train_end = train_start + window.train_window_ms;
        let test_start = train_end;
        let test_end = test_start + window.test_window_ms;

        let train_results = {
            let mut env = GridRunEnv {
                ledger,
                latency,
                fees,
            };
            run_grid(
                events,
                spec,
                SplitKind::Train,
                (train_start, train_end),
                &mut env,
                None,
            )?
        };

        if !train_results.is_empty() {
            let best = top_k(&train_results, 1);
            let mut oos_spec = spec.clone();
            oos_spec.cells = vec![best[0].params.clone()];

            let oos_results = {
                let mut env = GridRunEnv {
                    ledger,
                    latency,
                    fees,
                };
                run_grid(
                    events,
                    &oos_spec,
                    SplitKind::Val,
                    (test_start, test_end),
                    &mut env,
                    None,
                )?
            };

            let oos_sharpe = oos_results.first().map(|r| r.sharpe).unwrap_or(0.0);
            results.push(WindowResult {
                train_range_ms: (train_start, train_end),
                test_range_ms: (test_start, test_end),
                oos_sharpe,
            });
        }

        train_start += window.step_ms;
    }

    Ok(results)
}
