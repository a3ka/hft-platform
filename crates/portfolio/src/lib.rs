//! portfolio — Слой 3 (docs/fa/strategy-brain.md §5): `Forecast` + лимиты → сколько ДЕРЖАТЬ.
//!
//! ⚠ Это НЕ риск-слой. `PF-I-2` (кап позиции) — pre-trade sanity, чтобы конвейер решений
//! не мог выразить абсурдный размер. Настоящий fail-closed риск-гейт (`RK-I-1..10`,
//! `RiskApproved<Order>` с приватным конструктором) вводится M-08 и встаёт МЕЖДУ
//! `strategy` и `oms`. Ни один тест этого крейта не читается как «риск уже есть».
//!
//! Каркас (T2-типы + сигнатуры) — architect (M-07 task 1, sacred-контракт).
//! Реализация — engine-dev (M-07 task 3). Инварианты PF-I-1..4 — RED в `tests/` (sacred).

use std::collections::BTreeMap;

use alpha::{Forecast, Instrument};

/// T2: текущая позиция (знаковая: + long, − short), размер ×1e8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub instrument: Instrument,
    pub qty_e8: i64,
}

/// T2: целевая позиция — то, ЧТО мозг хочет держать (не ордер).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPosition {
    pub instrument: Instrument,
    pub qty_e8: i64,
}

/// Бюджет позиций. **Дефолта нет** (fail-closed, анти-`risk_guard` DESIGN §9):
/// инструмент без явного лимита → target 0, а не «какой-нибудь разумный лимит».
#[allow(dead_code)] // снимается в GREEN (engine-dev, M-07 task 3)
pub struct RiskBudget {
    limits: BTreeMap<Instrument, i64>,
}

impl RiskBudget {
    /// `max_position_e8` обязан быть > 0 (0/отрицательный лимит — ошибка конфига, не «запрет торговли»).
    pub fn new(_limits: Vec<(Instrument, i64)>) -> Result<Self, PortfolioError> {
        todo!("M-07 task 3 (engine-dev): валидация лимитов (>0, без дублей) → BTreeMap")
    }

    /// None ⟺ лимита нет (fail-closed: PF-I-3 → target 0).
    pub fn max_position_e8(&self, _instrument: &Instrument) -> Option<i64> {
        todo!("M-07 task 3 (engine-dev)")
    }
}

/// Сайзинг v1 (FA §5). Чистая функция; выход отсортирован по `instrument`.
///
/// - `target = clamp(edge_e8 · max_position_e8 / 1e8, ±max_position_e8)` (арифметика i128);
/// - инструмент без лимита → `target = 0` (PF-I-3);
/// - инструмент с позицией, но без форкаста → `target = 0` (flatten, PF-I-4);
/// - `|target| ≤ max_position_e8` ВСЕГДА, при любом входе (PF-I-2, fail-safe).
pub fn size(
    _forecasts: &[Forecast],
    _positions: &[Position],
    _budget: &RiskBudget,
) -> Vec<TargetPosition> {
    todo!("M-07 task 3 (engine-dev): сайзинг + кап + flatten + сортировка")
}

#[derive(Debug, PartialEq, Eq)]
pub enum PortfolioError {
    /// Лимит ≤ 0 — конфиг-ошибка, не «нулевая позиция».
    InvalidLimit(String),
    DuplicateLimit(String),
}
