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
        #[derive(serde::Deserialize)]
        struct FeeArtifact {
            #[allow(dead_code)]
            schema_version: u32,
            venue: Venue,
            /// Методика/ссылка на доку биржи — обязательное поле честности (D7).
            provenance: String,
            maker_rate_e8: i64,
            taker_rate_e8: i64,
        }

        let raw = std::fs::read_to_string(path).map_err(SimError::Io)?;
        let artifact: FeeArtifact =
            serde_json::from_str(&raw).map_err(|e| SimError::Parse(e.to_string()))?;
        if artifact.provenance.trim().is_empty() {
            return Err(SimError::Parse(
                "fee artifact: provenance пуст (D7 честность)".into(),
            ));
        }
        self.entries.insert(
            artifact.venue,
            FeeRates {
                maker_rate_e8: artifact.maker_rate_e8,
                taker_rate_e8: artifact.taker_rate_e8,
            },
        );
        Ok(())
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
        let _ = symbol;
        let rates = self
            .entries
            .get(&venue)
            .ok_or_else(|| SimError::MissingFees {
                venue,
                symbol: symbol.to_string(),
            })?;
        let rate_e8 = if maker {
            rates.maker_rate_e8
        } else {
            rates.taker_rate_e8
        };
        let scale = contracts::PRICE_SCALE as i128;
        let fee = (notional_e8 as i128 * rate_e8 as i128) / scale;
        Ok(fee as i64)
    }

    /// Стресс ×k к издержкам (отдельный прогон, RC-I-10).
    pub fn scaled(&self, factor: f64) -> FeeSchedule {
        let entries = self
            .entries
            .iter()
            .map(|(&venue, rates)| {
                (
                    venue,
                    FeeRates {
                        maker_rate_e8: (rates.maker_rate_e8 as f64 * factor).round() as i64,
                        taker_rate_e8: (rates.taker_rate_e8 as f64 * factor).round() as i64,
                    },
                )
            })
            .collect();
        FeeSchedule { entries }
    }
}
