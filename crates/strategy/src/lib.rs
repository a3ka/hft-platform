//! strategy — Слой 4 (docs/fa/strategy-brain.md §6): ЕДИНСТВЕННЫЙ код торговых решений.
//! `Event → signals → alpha → portfolio → diff(target vs current) → OrderIntent`.
//!
//! Тот же самый объект гоняет бэктест (`sim::StrategyBacktest`) и будущий live (`runner`) —
//! отсюда равенство DESIGN §1 №2 (`backtest == paper == live`). Крейт структурно не знает,
//! кто его исполняет: нет зависимостей на `sim`/`venue-*`/`journal`/`risk` (ST-I-6).
//!
//! ⚠ Здесь НЕТ риск-гейта. Между `strategy` и `oms` в M-08 встанет fail-closed `risk`
//! (`RiskApproved<Order>`, RK-I-1..10). `OrderIntent` — намерение, не разрешение.
//!
//! Каркас (T2-типы + трейт) — architect (M-07 task 1, sacred-контракт).
//! Реализация `DirectionalStrategy` — engine-dev (M-07 task 4).

pub mod types;

use std::collections::BTreeMap;

use alpha::{Alpha, Instrument};
use book::Books;
use contracts::Event;
use portfolio::RiskBudget;
use signals::Signal;

pub use types::{FillReport, OrderIntent, OrderKind, StrategyConfig, StrategyError};

/// Граница Слоя 4 (FA §6). `on_event` — чистый редьюсер (никакого I/O/часов/будущего);
/// `on_fill` — единственный способ подвинуть позицию (фантомных позиций не существует).
pub trait Strategy {
    fn on_event(&mut self, ev: &Event) -> Vec<OrderIntent>;
    fn on_fill(&mut self, fill: &FillReport);
    fn position_e8(&self, instrument: &Instrument) -> i64;
}

/// Ордер в полёте: сколько мы уже запросили, но ещё не получили филлом (M-07 D4).
/// Истекает по event-time через `StrategyConfig::intent_ttl_ms` — без него стратегия
/// шлёт новый интент на каждом тике, пока филл не дошёл (ST-I-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InFlight {
    /// Знаковая запрошенная дельта (×1e8).
    pub delta_e8: i64,
    pub submitted_ts_mono_ns: u64,
}

/// Directional-стратегия v1 (taker-вход/выход по целевой позиции).
#[allow(dead_code)] // снимается в GREEN (engine-dev, M-07 task 4)
pub struct DirectionalStrategy {
    signals: Vec<Box<dyn Signal>>,
    alpha: Box<dyn Alpha>,
    budget: RiskBudget,
    cfg: StrategyConfig,
    /// Реконструкция стакана — только из Md-событий (marketable-цена интента).
    books: Books,
    /// BTreeMap, не HashMap: порядок обхода = часть детерминизма (ST-I-4).
    positions: BTreeMap<Instrument, i64>,
    in_flight: BTreeMap<Instrument, InFlight>,
}

impl DirectionalStrategy {
    /// Конфиг валидируется на входе (fail-closed): `min_order_e8 > 0`, `intent_ttl_ms > 0`,
    /// `marketable_margin_bp ≥ 0`.
    pub fn new(
        _signals: Vec<Box<dyn Signal>>,
        _alpha: Box<dyn Alpha>,
        _budget: RiskBudget,
        _cfg: StrategyConfig,
    ) -> Result<Self, StrategyError> {
        todo!("M-07 task 4 (engine-dev): валидация конфига + инициализация состояния")
    }
}

impl Strategy for DirectionalStrategy {
    /// Конвейер (FA §6): expire in-flight по event-time → books.apply → сигналы (в порядке
    /// объявления) → alpha.update → portfolio::size → diff → интенты.
    /// Нет книги/лучшей цены → интента НЕТ (не «отправим по любой цене»).
    fn on_event(&mut self, _ev: &Event) -> Vec<OrderIntent> {
        todo!("M-07 task 4 (engine-dev): конвейер решений + diff + in-flight")
    }

    fn on_fill(&mut self, _fill: &FillReport) {
        todo!("M-07 task 4 (engine-dev): position += signed(qty); гашение in_flight")
    }

    fn position_e8(&self, _instrument: &Instrument) -> i64 {
        todo!("M-07 task 4 (engine-dev)")
    }
}
