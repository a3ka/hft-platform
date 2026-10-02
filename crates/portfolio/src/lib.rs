//! portfolio — Слой 3 (docs/fa/strategy-brain.md §5): `Forecast` + лимиты → сколько ДЕРЖАТЬ.
//!
//! ⚠ Это НЕ риск-слой. `PF-I-2` (кап позиции) — pre-trade sanity, чтобы конвейер решений
//! не мог выразить абсурдный размер. Настоящий fail-closed риск-гейт (`RK-I-1..10`,
//! `RiskApproved<Order>` с приватным конструктором) вводится M-08 и встаёт МЕЖДУ
//! `strategy` и `oms`. Ни один тест этого крейта не читается как «риск уже есть».
//!
//! Каркас (T2-типы + сигнатуры) — architect (M-07 task 1, sacred-контракт).
//! Реализация — engine-dev (M-07 task 3). Инварианты PF-I-1..4 — RED в `tests/` (sacred).

use std::collections::{BTreeMap, BTreeSet};

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
pub struct RiskBudget {
    /// BTreeMap — детерминизм обхода (DESIGN §1).
    limits: BTreeMap<Instrument, i64>,
}

impl RiskBudget {
    /// `max_position_e8` обязан быть > 0 (0/отрицательный лимит — ошибка конфига, не «запрет торговли»).
    pub fn new(limits: Vec<(Instrument, i64)>) -> Result<Self, PortfolioError> {
        let mut map: BTreeMap<Instrument, i64> = BTreeMap::new();
        for (inst, lim) in limits {
            let key = format!("{}|{}", inst.symbol, lim);
            if lim <= 0 {
                return Err(PortfolioError::InvalidLimit(key));
            }
            if map.insert(inst.clone(), lim).is_some() {
                return Err(PortfolioError::DuplicateLimit(key));
            }
        }
        Ok(RiskBudget { limits: map })
    }

    /// None ⟺ лимита нет (fail-closed: PF-I-3 → target 0).
    pub fn max_position_e8(&self, instrument: &Instrument) -> Option<i64> {
        self.limits.get(instrument).copied()
    }
}

/// Сайзинг v1 (FA §5). Чистая функция; выход отсортирован по `instrument`.
///
/// - `target = clamp(edge_e8 · max_position_e8 / 1e8, ±max_position_e8)` (арифметика i128);
/// - инструмент без лимита → `target = 0` (PF-I-3);
/// - инструмент с позицией, но без форкаста → `target = 0` (flatten, PF-I-4);
/// - `|target| ≤ max_position_e8` ВСЕГДА, при любом входе (PF-I-2, fail-safe).
pub fn size(
    forecasts: &[Forecast],
    positions: &[Position],
    budget: &RiskBudget,
) -> Vec<TargetPosition> {
    // ── Собираем объединение инструментов (упоминание в форкастах ИЛИ в позициях).
    let mut seen: BTreeSet<Instrument> = BTreeSet::new();
    for f in forecasts {
        seen.insert(f.instrument.clone());
    }
    for p in positions {
        seen.insert(p.instrument.clone());
    }
    let scale = contracts::PRICE_SCALE as i128;

    let mut out = Vec::with_capacity(seen.len());
    for inst in &seen {
        let cap = match budget.max_position_e8(inst) {
            Some(c) if c > 0 => c,
            _ => {
                // PF-I-3: инструмент без лимита → 0 (а не «default-лимит»).
                out.push(TargetPosition {
                    instrument: inst.clone(),
                    qty_e8: 0,
                });
                continue;
            }
        };

        // Берём первый матч (форкаст per (инструмент,signals) — единственный в этой схеме).
        let target_qty: i64 = match forecasts.iter().find(|f| &f.instrument == inst) {
            Some(f) => {
                // ── i128 арифметика: edge · cap может переполнить i64 (PF-I-2). ──
                let raw = (f.edge_e8 as i128) * (cap as i128) / scale;
                let clamped = raw.clamp(-(cap as i128), cap as i128);
                clamped as i64
            }
            None => 0, // PF-I-4: позиция без форкаста → flatten.
        };

        out.push(TargetPosition {
            instrument: inst.clone(),
            qty_e8: target_qty,
        });
    }
    // BTreeSet-обход детерминированно отсортирован по Instrument Ord.
    out
}

#[derive(Debug, PartialEq, Eq)]
pub enum PortfolioError {
    /// Лимит ≤ 0 — конфиг-ошибка, не «нулевая позиция».
    InvalidLimit(String),
    DuplicateLimit(String),
}
