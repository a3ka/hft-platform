//! StrategyBacktest — раннер бэктеста НАСТОЯЩЕЙ стратегии (M-07 D3).
//!
//! Заменяет ad-hoc harness `research-cli/src/grid.rs` (taker-in/taker-out по horizon):
//! тот harness мерил логику, которой не будет в live. Здесь через `BacktestExchange`
//! гоняется тот же `dyn Strategy`, который в P3+ будет гонять `runner` на живом фиде.
//!
//! Порядок на каждом событии — СТРОГО (no-lookahead, ST-I-5/SM-I-4):
//!   1. `fills = exchange.on_event(ev)` — биржа применяет событие первой;
//!   2. `strategy.on_fill(FillReport)` по каждому филлу (мост SimFill→FillReport, D2);
//!   3. `intents = strategy.on_event(ev)` — стратегия видит ТОЛЬКО событие ≤ seq;
//!   4. `exchange.submit(intent)` — интенты уходят на биржу (эффект — через δ_submit).
//!
//! Каркас — architect (M-07 task 1). Реализация — engine-dev (M-07 task 5).

use std::collections::BTreeMap;

use alpha::Instrument;
use contracts::{Event, Side};
use strategy::Strategy;

use crate::exchange::BacktestExchange;
use crate::fees::FeeSchedule;
use crate::latency::LatencyTable;
use crate::types::SimFill;

/// Детерминированный отчёт прогона (D7). Всё — fixed-point ×1e8; никаких f64 в деньгах.
#[derive(Debug, Clone, PartialEq)]
pub struct BacktestReport {
    /// Сколько интентов стратегия отдала бирже.
    pub intents: usize,
    pub fills: Vec<SimFill>,
    /// Кэш: buy → −(notional + fee); sell → +(notional − fee).
    pub cash_e8: i64,
    /// Итоговые нетто-позиции (знаковые), отсортированы по инструменту.
    pub positions: BTreeMap<Instrument, i64>,
    /// Σ |notional| по всем филлам.
    pub turnover_e8: i64,
    /// Mark-to-market equity (`cash + Σ position × mid`) на каждом событии, где были филлы.
    pub equity_curve_e8: Vec<i64>,
}

/// Мост исполнения: помнит, какому (инструмент, сторона) принадлежит order_id, чтобы
/// собрать `strategy::FillReport` из `SimFill` (в `SimFill` этого нет — и не должно быть).
#[allow(dead_code)] // снимается в GREEN (engine-dev, M-07 task 5)
pub struct StrategyBacktest {
    exchange: BacktestExchange,
    /// BTreeMap, не HashMap: порядок = часть детерминизма (DESIGN §1).
    order_meta: BTreeMap<u64, (Instrument, Side)>,
}

impl StrategyBacktest {
    pub fn new(_latency: LatencyTable, _fees: FeeSchedule, _seed: u64) -> Self {
        todo!("M-07 task 5 (engine-dev): инициализация BacktestExchange + мост order_meta")
    }

    /// Прогнать поток событий через стратегию. Прогон детерминирован при фиксированном
    /// seed: два вызова на одном входе дают идентичный `BacktestReport` (ST-I-8).
    pub fn run(&mut self, _events: &[Event], _strategy: &mut dyn Strategy) -> BacktestReport {
        todo!("M-07 task 5 (engine-dev): цикл on_event → on_fill → on_event → submit (D3)")
    }
}
