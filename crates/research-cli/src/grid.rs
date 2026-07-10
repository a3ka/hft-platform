//! grid — оркестрация перебора (FA §5): для КАЖДОЙ ячейки — инстанс сигнала →
//! реплей событий диапазона через harness (сигнал → интенты → sim fills → PnL) →
//! ЗАПИСЬ В LEDGER (независимо от исхода и попадания в топ-K — RC-I-9/§5).
//! Стресс-режимы — отдельные прогоны с scaled() таблицами (RC-I-10).
//!
//! Harness v1 (M-04): направленный вход по SignalOut (taker по умолчанию; maker —
//! параметр ячейки), выход taker через horizon_ms. Отказ ledger-записи → abort
//! ВСЕГО прогона (FA §3).
//!
//! Реализация — research-dev (M-04 task 4).

use contracts::Event;
use sim::{FeeSchedule, LatencyTable};

use crate::ledger::Ledger;
use crate::split::ValGateToken;
use crate::types::{CellResult, GridSpec, RcError, SplitKind};

/// Окружение прогона: ledger (единственная точка записи) + честные таблицы sim.
pub struct GridRunEnv<'a> {
    pub ledger: &'a mut Ledger,
    pub latency: &'a LatencyTable,
    pub fees: &'a FeeSchedule,
}

/// Прогнать грид над событиями диапазона range_ms (полуинтервал [from, to) по ts_wall_ms).
/// Для SplitKind::Test ОБЯЗАТЕЛЕН &ValGateToken (RC-I-8): Test без токена →
/// Err::GateDenied; для Train/Val — test_proof = None.
pub fn run_grid(
    events: &[Event],
    spec: &GridSpec,
    split: SplitKind,
    range_ms: (i64, i64),
    env: &mut GridRunEnv<'_>,
    test_proof: Option<&ValGateToken>,
) -> Result<Vec<CellResult>, RcError> {
    let _ = (events, spec, split, range_ms, env, test_proof);
    todo!("research-dev: M-04 task 4")
}

/// Механическая сортировка топ-K по предварительной метрике (Sharpe без deflation —
/// deflation только на финальной валидации, FA §5). НЕ трогает val/test.
pub fn top_k(results: &[CellResult], k: usize) -> Vec<CellResult> {
    let _ = (results, k);
    todo!("research-dev: M-04 task 4")
}
