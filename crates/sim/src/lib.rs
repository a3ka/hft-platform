//! sim — честный исполнительный симулятор (docs/fa/sim.md). Backtest-режим (M-04);
//! paper-обвязка — P3 (требует oms/risk).
//!
//! Пессимизм — системная поза (FA §2/§5): очередь = хвост уровня без cancel-credit;
//! maker-fill только при ПРЕВЫШЕНИИ traded-объёмом глубины впереди; латентность —
//! ТОЛЬКО из измеренной таблицы (SM-I-7/8); тарифы — только из артефакта (без
//! «нулевой комиссии»). Инварианты SM-I-1..10 — RED-оракулы в `tests/` (sacred).
//!
//! Каркас (типы+сигнатуры) — architect (M-04 task 1); реализация — engine-dev (task 2).

pub mod divergence;
pub mod exchange;
pub mod fees;
pub mod fill_model;
pub mod funding;
pub mod latency;
pub mod rng;
pub mod strategy_backtest;
pub mod types;

pub use divergence::{p4_gate, DivergenceMetric, DivergenceTolerance, GateBlocked};
pub use exchange::BacktestExchange;
pub use fees::{FeeRates, FeeSchedule};
pub use latency::{LatencyDraw, LatencyTable};
pub use rng::SplitMix64;
pub use strategy_backtest::{BacktestReport, StrategyBacktest};
pub use types::{
    FillDecision, OrderIntent, OrderKind, QueueState, SimError, SimFill, SimOrder, TradedTick,
};
