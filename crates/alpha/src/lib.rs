//! alpha — Слой 3 (docs/fa/strategy-brain.md §4): ансамбль калиброванных сигналов →
//! `Forecast` («каков край и на каком горизонте»), БЕЗ решения о размере (это portfolio).
//!
//! Чистый детерминированный редьюсер над потоком `Event` + `SignalOut`: никакого I/O,
//! wall-clock, rand, итерации по HashMap (DESIGN §1 журнал-принцип).
//!
//! Каркас (T2-типы + трейт) — architect (M-07 task 1, sacred-контракт).
//! Реализация `LinearAlpha` — engine-dev (M-07 task 2).
//! Инварианты AL-I-1..5 — RED-оракулы в `tests/` (sacred, architect-only).

use std::collections::BTreeMap;

use contracts::{Event, Venue};
use signals::{SignalId, SignalOut};

/// Масштаб edge/confidence (×1e8), зеркало `signals::SIGNAL_VALUE_SCALE`.
pub const EDGE_SCALE: i64 = contracts::PRICE_SCALE;

/// Инструмент = (площадка, символ площадки как есть). Тотальный порядок обязателен:
/// весь конвейер решений итерируется в детерминированном порядке (DESIGN §1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Instrument {
    pub venue: Venue,
    pub symbol: String,
}

impl Instrument {
    pub fn new(venue: Venue, symbol: impl Into<String>) -> Self {
        Self {
            venue,
            symbol: symbol.into(),
        }
    }

    /// Ключ порядка. `Venue` — fieldless enum → стабильный дискриминант (порядок
    /// вариантов T1-enum'а закреплён CT-I §6: расширение только в конец).
    fn ord_key(&self) -> (u8, &str) {
        (self.venue as u8, self.symbol.as_str())
    }
}

impl PartialOrd for Instrument {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Instrument {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ord_key().cmp(&other.ord_key())
    }
}

/// T2 (FA §3): выход alpha. `edge_e8 ∈ [-EDGE_SCALE, +EDGE_SCALE]` — направленный край;
/// `confidence_e8 ∈ [0, EDGE_SCALE]` — доля живого веса ансамбля (v1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forecast {
    pub instrument: Instrument,
    pub ts_mono_ns: u64,
    pub edge_e8: i64,
    pub horizon_ms: i64,
    pub confidence_e8: i64,
}

/// Граница alpha (FA §4). Единственный вход — событие + выходы сигналов ЭТОГО события.
/// Выход отсортирован по `instrument` (детерминизм, AL-I-1).
pub trait Alpha {
    fn update(&mut self, ev: &Event, signal_outs: &[SignalOut]) -> Vec<Forecast>;
}

/// Вес сигнала в ансамбле (конфиг; в P3 приходит из `signals.json` — граница B).
#[derive(Debug, Clone, PartialEq)]
pub struct SignalWeight {
    pub signal_id: SignalId,
    pub instrument: Instrument,
    /// ×1e8; знак допустим (инверсия сигнала). Ноль запрещён (AlphaError::ZeroWeight).
    pub weight_e8: i64,
}

/// Последний непротухший сэмпл сигнала (T3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub value_e8: i64,
    pub horizon_ms: i64,
    pub ts_event_mono_ns: u64,
}

/// v1-ансамбль: взвешенная сумма (FA §4). Сэмпл участвует, пока
/// `ev.ts_mono_ns ≤ ts_event + horizon_ms·1e6` (stale-expiry, AL-I-4).
#[allow(dead_code)] // снимается в GREEN (engine-dev, M-07 task 2)
pub struct LinearAlpha {
    weights: Vec<SignalWeight>,
    /// Ключ — (инструмент, signal_id как строка): BTreeMap, не HashMap (порядок = детерминизм).
    last: BTreeMap<(Instrument, String), Sample>,
}

impl LinearAlpha {
    /// Веса валидируются на входе (fail-closed): пустой набор / нулевой вес → Err.
    pub fn new(_weights: Vec<SignalWeight>) -> Result<Self, AlphaError> {
        todo!("M-07 task 2 (engine-dev): валидация весов + инициализация состояния")
    }
}

impl Alpha for LinearAlpha {
    fn update(&mut self, _ev: &Event, _signal_outs: &[SignalOut]) -> Vec<Forecast> {
        todo!("M-07 task 2 (engine-dev): stale-expiry → взвешенная сумма → clamp → сортировка")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AlphaError {
    EmptyWeights,
    ZeroWeight(String),
    /// Дубль (signal_id, instrument) в весах — двусмысленность конфига.
    DuplicateWeight(String),
}
