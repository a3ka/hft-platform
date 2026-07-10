//! latency — сэмплирование из ИЗМЕРЕННЫХ распределений (FA §6; артефакт per D7).
//!
//! Формат артефакта research/latency/<venue>-<symbol>.json:
//! { "schema_version": 1, "venue": "Binance", "symbol": "BTCUSDT",
//!   "provenance": "<методика измерения — ОБЯЗАТЕЛЬНОЕ поле>",
//!   "delta_submit_ns": [отсортированные сэмплы], "delta_cancel_ns": [...],
//!   "delta_md_ns": [...] }
//!
//! Сэмплирование: inverse-CDF по эмпирическим сэмплам (индекс = floor(u·len)) от
//! SplitMix64 — детерминировано при фиксированном seed (SM-I-2).
//! ОТСУТСТВИЕ записи для (venue,symbol) = Err (SM-I-8), не default. В коде НЕТ
//! пути с нулевой/захардкоженной задержкой (SM-I-7, grep-канарейка в tests/structural.rs).
//!
//! Реализация — engine-dev (M-04 task 2).

use std::collections::HashMap;
use std::path::Path;

use contracts::Venue;
use serde::Deserialize;

use crate::rng::SplitMix64;
use crate::types::SimError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatencyDraw {
    pub delta_submit_ns: u64,
    pub delta_cancel_ns: u64,
    pub delta_md_ns: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LatencyArtifact {
    pub schema_version: u32,
    pub venue: Venue,
    pub symbol: String,
    /// Методика измерения — обязательное поле честности (D7); пустая строка = Parse-ошибка.
    pub provenance: String,
    pub delta_submit_ns: Vec<u64>,
    pub delta_cancel_ns: Vec<u64>,
    pub delta_md_ns: Vec<u64>,
}

#[derive(Debug, Default)]
pub struct LatencyTable {
    entries: HashMap<(Venue, String), LatencyArtifact>,
}

impl LatencyTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Загрузить один артефакт-файл в таблицу. Пустые сэмплы/пустой provenance → Err.
    pub fn load_artifact(&mut self, path: &Path) -> Result<(), SimError> {
        let _ = path;
        let _ = &self.entries;
        todo!("engine-dev: M-04 task 2")
    }

    /// Синтетика для тестов/грида.
    pub fn insert_samples(
        &mut self,
        venue: Venue,
        symbol: &str,
        submit_ns: Vec<u64>,
        cancel_ns: Vec<u64>,
        md_ns: Vec<u64>,
        provenance: &str,
    ) {
        let _ = (venue, symbol, submit_ns, cancel_ns, md_ns, provenance);
        todo!("engine-dev: M-04 task 2")
    }

    pub fn has(&self, venue: Venue, symbol: &str) -> bool {
        self.entries.contains_key(&(venue, symbol.to_string()))
    }

    /// SM-I-8: отсутствующая запись → Err::MissingLatency, никаких default.
    pub fn draw(
        &self,
        venue: Venue,
        symbol: &str,
        rng: &mut SplitMix64,
    ) -> Result<LatencyDraw, SimError> {
        let _ = (venue, symbol, rng);
        todo!("engine-dev: M-04 task 2")
    }

    /// Стресс ×k к латентности (RC-I-10: стресс — отдельный прогон через ту же модель).
    pub fn scaled(&self, factor: f64) -> LatencyTable {
        let _ = factor;
        todo!("engine-dev: M-04 task 2")
    }
}
