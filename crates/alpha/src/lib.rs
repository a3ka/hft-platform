//! alpha — Слой 3 (docs/fa/strategy-brain.md §4): ансамбль калиброванных сигналов →
//! `Forecast` («каков край и на каком горизонте»), БЕЗ решения о размере (это portfolio).
//!
//! Чистый детерминированный редьюсер над потоком `Event` + `SignalOut`: никакого I/O,
//! wall-clock, rand, итерации по HashMap (DESIGN §1 журнал-принцип).
//!
//! Каркас (T2-типы + трейт) — architect (M-07 task 1, sacred-контракт).
//! Реализация `LinearAlpha` — engine-dev (M-07 task 2).
//! Инварианты AL-I-1..5 — RED-оракулы в `tests/` (sacred, architect-only).

use std::collections::{BTreeMap, BTreeSet};

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
pub struct LinearAlpha {
    /// (Инструмент, signal_id как строка) → ПОДПИСАННЫЙ вес ×1e8. BTreeMap — детерминизм.
    /// SignalId (signals::*[​[derive]] не имеет Ord) → ключ строковый, value Ord есть.
    weights: BTreeMap<(Instrument, String), i64>,
    /// Сумма |w| на инструмент (для confidence_e8 = доля живого веса ансамбля, FA §4).
    weight_sum_per_instrument: BTreeMap<Instrument, i128>,
    /// Последний сэмпл на (инструмент, signal_id). BTreeMap — детерминизм обхода.
    last: BTreeMap<(Instrument, String), Sample>,
}

impl LinearAlpha {
    /// Веса валидируются на входе (fail-closed): пустой набор / нулевой вес / дубль → Err.
    pub fn new(weights: Vec<SignalWeight>) -> Result<Self, AlphaError> {
        if weights.is_empty() {
            return Err(AlphaError::EmptyWeights);
        }
        let mut by_key: BTreeMap<(Instrument, String), i64> = BTreeMap::new();
        let mut sum_per_inst: BTreeMap<Instrument, i128> = BTreeMap::new();
        let mut seen_keys: BTreeSet<(Instrument, String)> = BTreeSet::new();
        for w in weights {
            if w.weight_e8 == 0 {
                return Err(AlphaError::ZeroWeight(format!(
                    "{}|{}",
                    w.instrument.ord_key().1,
                    w.signal_id.as_str()
                )));
            }
            let key = (w.instrument.clone(), w.signal_id.as_str().to_string());
            if !seen_keys.insert(key.clone()) {
                return Err(AlphaError::DuplicateWeight(format!(
                    "{}|{}",
                    w.instrument.ord_key().1,
                    w.signal_id.as_str()
                )));
            }
            by_key.insert(key, w.weight_e8);
            let entry = sum_per_inst.entry(w.instrument.clone()).or_insert(0);
            *entry += (w.weight_e8 as i128).abs();
        }
        Ok(LinearAlpha {
            weights: by_key,
            weight_sum_per_instrument: sum_per_inst,
            last: BTreeMap::new(),
        })
    }
}

impl Alpha for LinearAlpha {
    fn update(&mut self, ev: &Event, signal_outs: &[SignalOut]) -> Vec<Forecast> {
        // ── 1. Обновить last[(inst, sid)] по каждому входящему SignalOut. Один и тот же
        // signal_id может быть на разные инструменты (multi-instrument ensemble) —
        // тогда сэмпл записывается во ВСЕ его (inst, sid) ключи. ─────────────────────
        for out in signal_outs {
            let sample = Sample {
                value_e8: out.value,
                horizon_ms: out.meta.horizon_ms,
                ts_event_mono_ns: out.ts_event_mono_ns,
            };
            // Детерминированный порядок ключей BTreeMap; collect чтобы избежать aliasing.
            let matches: Vec<(Instrument, String)> = self
                .weights
                .keys()
                .filter(|(_, sid)| sid.as_str() == out.signal_id.as_str())
                .cloned()
                .collect();
            for key in matches {
                self.last.insert(key, sample);
            }
        }

        // ── 2. Детерминированно обойти инструменты, присутствующие в конфиге. ───────────
        let instruments: Vec<Instrument> = self
            .weight_sum_per_instrument
            .keys()
            .cloned()
            .collect();

        let mut forecasts = Vec::new();
        for inst in &instruments {
            let total_abs_w: i128 = self
                .weight_sum_per_instrument
                .get(inst)
                .copied()
                .unwrap_or(0);
            if total_abs_w <= 0 {
                continue;
            }

            // ── 3. По каждому (instrument, signal_id)-весу проверить свежесть sample. ──
            let mut num: i128 = 0; // Σ w·v
            let mut den_fresh: i128 = 0; // Σ |w| для свежих
            let mut max_horizon_ms: i64 = 0;

            for ((i, sid), &w) in &self.weights {
                if i != inst {
                    continue;
                }
                let sample = match self.last.get(&(i.clone(), sid.clone())) {
                    Some(s) => *s,
                    None => continue, // ни разу не было сэмпла → не свежий, не считаем
                };
                let horizon_ns = match (sample.horizon_ms as u64).checked_mul(1_000_000) {
                    Some(v) => v,
                    None => continue, // pathological horizon → безопаснее исключить
                };
                let threshold = sample.ts_event_mono_ns.saturating_add(horizon_ns);
                let fresh = ev.ts_mono_ns <= threshold;
                if !fresh {
                    continue;
                }
                num += (w as i128) * (sample.value_e8 as i128);
                den_fresh += (w as i128).abs();
                if sample.horizon_ms > max_horizon_ms {
                    max_horizon_ms = sample.horizon_ms;
                }
            }

            if den_fresh == 0 {
                // Все протухли (либо сэмплов никогда не было) — отсутствие мнения ≠ edge=0.
                continue;
            }

            // ── 4. Edge = clamp(num / den_fresh, ±EDGE_SCALE). num и den_fresh оба в ×1e8,
            // результат — в ×1e8. ───────────────────────────────────────────────────────
            let edge_raw = num / den_fresh;
            let edge_clamped = edge_raw.clamp(-(EDGE_SCALE as i128), EDGE_SCALE as i128);
            let edge_e8 = edge_clamped as i64;

            // ── 5. Confidence = доля ЖИВОГО веса × EDGE_SCALE (i128 — без переполнения). ──
            let conf_raw = den_fresh * (EDGE_SCALE as i128) / total_abs_w;
            let confidence_e8 = conf_raw.clamp(0, EDGE_SCALE as i128) as i64;

            forecasts.push(Forecast {
                instrument: inst.clone(),
                ts_mono_ns: ev.ts_mono_ns,
                edge_e8,
                horizon_ms: max_horizon_ms,
                confidence_e8,
            });
        }

        // Выход отсортирован по instrument (FA §4). BTreeMap-обход и так отсортирован.
        forecasts
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AlphaError {
    EmptyWeights,
    ZeroWeight(String),
    /// Дубль (signal_id, instrument) в весах — двусмысленность конфига.
    DuplicateWeight(String),
}
