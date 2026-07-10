//! fees — тарифы maker/taker из версионируемого артефакта (FA §7; D7).
//! Отсутствие тарифа для инструмента = Err (не «нулевая комиссия» — классический
//! оптимизм бэктеста). Формат research/fees/<venue>.json:
//! { "schema_version": 1, "venue": "...", "provenance": "<ссылка на доку биржи + дата>",
//!   "maker_rate_e8": 10000, "taker_rate_e8": 45000 }   // rate ×1e8 (0.001 = 100_000)
//!
//! Реализация — engine-dev (M-04 task 2).

use std::collections::HashMap;
use std::path::Path;

use contracts::Venue;

use crate::types::SimError;

#[derive(Debug, Clone, Copy)]
pub struct FeeRates {
    /// ×1e8; отрицательный maker = ребейт.
    pub maker_rate_e8: i64,
    pub taker_rate_e8: i64,
}

#[derive(Debug, Default)]
pub struct FeeSchedule {
    entries: HashMap<Venue, FeeRates>,
}

impl FeeSchedule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_artifact(&mut self, path: &Path) -> Result<(), SimError> {
        let _ = path;
        let _ = &self.entries;
        todo!("engine-dev: M-04 task 2")
    }

    pub fn insert_rates(&mut self, venue: Venue, rates: FeeRates) {
        self.entries.insert(venue, rates);
    }

    pub fn has(&self, venue: Venue) -> bool {
        self.entries.contains_key(&venue)
    }

    /// Комиссия ×1e8 USD от нотионала (notional_e8 = price·qty/PRICE_SCALE).
    /// Нет тарифа → Err::MissingFees (Halt прогона, FA §3 таблица).
    pub fn fee_e8(
        &self,
        venue: Venue,
        symbol: &str,
        maker: bool,
        notional_e8: i64,
    ) -> Result<i64, SimError> {
        let _ = (venue, symbol, maker, notional_e8);
        todo!("engine-dev: M-04 task 2")
    }

    /// Стресс ×k к издержкам (отдельный прогон, RC-I-10).
    pub fn scaled(&self, factor: f64) -> FeeSchedule {
        let _ = factor;
        todo!("engine-dev: M-04 task 2")
    }
}
