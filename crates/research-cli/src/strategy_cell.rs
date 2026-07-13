//! strategy_cell — контракт D7/D8 грида на strategy-пайплайне (M-07, carve-out A2).
//!
//! Каркас (типы + сигнатуры + `todo!()`) — architect; реализация — research-dev (task 6).
//! Введён по вердикту critic C-004 (находка C2): без ЭТИХ форм задача 6 гейтилась только
//! грепами, которые удовлетворяются комментарием — т.е. грид мог формально «мигрировать»,
//! оставив returns/params_hash/дефолты неверными.
//!
//! D8: grid-ячейка = params сигнала (`ObiParams`) + ОПЦИОНАЛЬНЫЙ блок `strategy`.
//! Отсутствует → документированные дефолты ниже. `params_hash` ОБЯЗАН покрывать блок
//! `strategy` (иначе две разные стратегии пишутся в ledger под одним хэшем — фальсификация
//! анти-оверфит счётчика, RC-I-9/RC-I-10).
//!
//! D7: returns грида считаются из mark-to-market equity `BacktestReport.equity_curve_e8`,
//! нормированной на `capital_ref_e8` — НЕ из старой формулы entry/exit-нотионалов
//! ad-hoc harness'а (та мерила логику, которой не будет в live).

use serde::{Deserialize, Serialize};
use strategy::OrderKind;

use crate::types::{CostsMode, RcError};

/// Дефолты блока `strategy` grid-ячейки (D8). Меняются только через milestone/RFC —
/// молчаливое изменение дефолта переписывает смысл всех прошлых trial-записей.
pub const DEFAULT_MAX_POSITION_E8: i64 = 100_000_000; // 1.0 базовой единицы
pub const DEFAULT_MIN_ORDER_E8: i64 = 1_000_000; // 0.01
pub const DEFAULT_INTENT_TTL_MS: i64 = 1_000;
pub const DEFAULT_MARKETABLE_MARGIN_BP: i64 = 100; // 1%

/// Разобранный блок `strategy` ячейки (T3 research-cli).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyCellConfig {
    pub max_position_e8: i64,
    pub min_order_e8: i64,
    pub intent_ttl_ms: i64,
    pub marketable_margin_bp: i64,
    /// "taker" (v1) | "maker" — строкой в JSON ячейки.
    pub kind: String,
}

impl StrategyCellConfig {
    /// Форма для `strategy::StrategyConfig`. Невалидный `kind` → Err (fail-closed).
    pub fn order_kind(&self) -> Result<OrderKind, RcError> {
        todo!("M-07 task 6 (research-dev): \"taker\"|\"maker\" → OrderKind, иначе RcError")
    }
}

/// Разбор блока `strategy` ячейки (D8). Блока нет → ДЕФОЛТЫ (не «нулевые лимиты»).
/// Блок есть, но кривой (неизвестное поле/тип/неположительные значения) → Err (fail-closed:
/// молча подставленный дефолт на кривом конфиге = тихо другая стратегия в отчёте).
pub fn strategy_cell_config(_cell: &serde_json::Value) -> Result<StrategyCellConfig, RcError> {
    todo!("M-07 task 6 (research-dev): парсинг блока strategy + дефолты D8")
}

/// Канонический хэш ячейки для ledger (D8). ОБЯЗАН зависеть и от params сигнала,
/// и от блока `strategy`, и от `costs_mode` (стресс-прогон = другой хэш, RC-I-10).
/// Детерминирован: одинаковый вход → одинаковый хэш при любом порядке ключей JSON.
pub fn cell_params_hash(_cell: &serde_json::Value, _costs_mode: CostsMode) -> String {
    todo!("M-07 task 6 (research-dev): канонический sha256(params + strategy + costs_mode)")
}

/// Опорный капитал ячейки (D7): `capital_ref_e8 = max_position_e8 · mid_e8 / 1e8` —
/// нотионал максимально допустимой позиции по первому наблюдённому mid.
/// `mid_e8 ≤ 0` (книги не было) → 0 (returns тогда пусты, не «деление на 1»).
pub fn capital_ref_e8(_max_position_e8: i64, _first_mid_e8: i64) -> i64 {
    todo!("M-07 task 6 (research-dev): нотионал max-позиции по первому mid")
}

/// Пошаговые доходности из mark-to-market equity (D7):
/// `returns[i] = (equity[i+1] − equity[i]) / capital_ref_e8`.
/// `capital_ref_e8 ≤ 0` ИЛИ < 2 точек equity → пустой вектор (не NaN/inf в метрики).
pub fn returns_from_equity(_equity_curve_e8: &[i64], _capital_ref_e8: i64) -> Vec<f64> {
    todo!("M-07 task 6 (research-dev): Δequity / capital_ref")
}
