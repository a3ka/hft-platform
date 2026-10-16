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
use sha2::{Digest, Sha256};
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
#[serde(default, deny_unknown_fields)]
pub struct StrategyCellConfig {
    pub max_position_e8: i64,
    pub min_order_e8: i64,
    pub intent_ttl_ms: i64,
    pub marketable_margin_bp: i64,
    /// "taker" (v1) | "maker" — строкой в JSON ячейки.
    pub kind: String,
}

impl Default for StrategyCellConfig {
    fn default() -> Self {
        Self {
            max_position_e8: DEFAULT_MAX_POSITION_E8,
            min_order_e8: DEFAULT_MIN_ORDER_E8,
            intent_ttl_ms: DEFAULT_INTENT_TTL_MS,
            marketable_margin_bp: DEFAULT_MARKETABLE_MARGIN_BP,
            kind: "taker".to_string(),
        }
    }
}

impl StrategyCellConfig {
    /// Форма для `strategy::StrategyConfig`. Невалидный `kind` → Err (fail-closed).
    pub fn order_kind(&self) -> Result<OrderKind, RcError> {
        match self.kind.as_str() {
            "taker" => Ok(OrderKind::Taker),
            "maker" => Ok(OrderKind::Maker),
            other => Err(RcError::Parse(format!(
                "strategy.kind должен быть `taker` или `maker`, получен `{other}`"
            ))),
        }
    }
}

/// Разбор блока `strategy` ячейки (D8). Блока нет → ДЕФОЛТЫ (не «нулевые лимиты»).
/// Блок есть, но кривой (неизвестное поле/тип/неположительные значения) → Err (fail-closed:
/// молча подставленный дефолт на кривом конфиге = тихо другая стратегия в отчёте).
pub fn strategy_cell_config(cell: &serde_json::Value) -> Result<StrategyCellConfig, RcError> {
    let object = cell
        .as_object()
        .ok_or_else(|| RcError::Parse("grid-ячейка должна быть JSON-объектом".into()))?;
    let config = match object.get("strategy") {
        None => StrategyCellConfig::default(),
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|e| RcError::Parse(format!("невалидный блок strategy: {e}")))?,
    };

    if config.max_position_e8 <= 0 {
        return Err(RcError::Parse(
            "strategy.max_position_e8 должен быть > 0".into(),
        ));
    }
    if config.min_order_e8 <= 0 {
        return Err(RcError::Parse(
            "strategy.min_order_e8 должен быть > 0".into(),
        ));
    }
    if config.intent_ttl_ms <= 0 {
        return Err(RcError::Parse(
            "strategy.intent_ttl_ms должен быть > 0".into(),
        ));
    }
    if config.marketable_margin_bp < 0 {
        return Err(RcError::Parse(
            "strategy.marketable_margin_bp должен быть >= 0".into(),
        ));
    }
    config.order_kind()?;
    Ok(config)
}

/// Рекурсивно сортирует ключи JSON-объектов перед сериализацией. Это не полагается на
/// внутренний map serde_json и сохраняет хэш при любом порядке ключей входной ячейки.
fn canonical_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(canonical_json).collect())
        }
        serde_json::Value::Object(object) => {
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort_unstable();
            let sorted = keys
                .into_iter()
                .map(|key| (key.clone(), canonical_json(&object[key])))
                .collect();
            serde_json::Value::Object(sorted)
        }
        scalar => scalar.clone(),
    }
}

fn costs_mode_name(costs_mode: CostsMode) -> &'static str {
    match costs_mode {
        CostsMode::Baseline => "baseline",
        CostsMode::CostX15 => "cost_x15",
        CostsMode::LatencyX2 => "latency_x2",
    }
}

/// Канонический хэш ячейки для ledger (D8). ОБЯЗАН зависеть и от params сигнала,
/// и от блока `strategy`, и от `costs_mode` (стресс-прогон = другой хэш, RC-I-10).
/// Детерминирован: одинаковый вход → одинаковый хэш при любом порядке ключей JSON.
pub fn cell_params_hash(cell: &serde_json::Value, costs_mode: CostsMode) -> String {
    let envelope = serde_json::json!({
        "hash_schema": "strategy-grid-v1",
        "cell": cell,
        "costs_mode": costs_mode_name(costs_mode),
    });
    let canonical = serde_json::to_vec(&canonical_json(&envelope))
        .expect("serde_json::Value serialization is infallible");
    let mut hasher = Sha256::new();
    hasher.update(canonical);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Опорный капитал ячейки (D7): `capital_ref_e8 = max_position_e8 · mid_e8 / 1e8` —
/// нотионал максимально допустимой позиции по первому наблюдённому mid.
/// `mid_e8 ≤ 0` (книги не было) → 0 (returns тогда пусты, не «деление на 1»).
pub fn capital_ref_e8(max_position_e8: i64, first_mid_e8: i64) -> i64 {
    if max_position_e8 <= 0 || first_mid_e8 <= 0 {
        return 0;
    }
    let notional =
        (max_position_e8 as i128) * (first_mid_e8 as i128) / (contracts::PRICE_SCALE as i128);
    notional.clamp(0, i64::MAX as i128) as i64
}

/// Пошаговые доходности из mark-to-market equity (D7):
/// `returns[i] = (equity[i+1] − equity[i]) / capital_ref_e8`.
/// `capital_ref_e8 ≤ 0` ИЛИ < 2 точек equity → пустой вектор (не NaN/inf в метрики).
pub fn returns_from_equity(equity_curve_e8: &[i64], capital_ref_e8: i64) -> Vec<f64> {
    if capital_ref_e8 <= 0 || equity_curve_e8.len() < 2 {
        return Vec::new();
    }
    equity_curve_e8
        .windows(2)
        .map(|window| {
            let delta = window[1] as i128 - window[0] as i128;
            delta as f64 / capital_ref_e8 as f64
        })
        .collect()
}
