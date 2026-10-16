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
        let raw = std::fs::read_to_string(path).map_err(SimError::Io)?;
        let artifact: LatencyArtifact =
            serde_json::from_str(&raw).map_err(|e| SimError::Parse(e.to_string()))?;
        if artifact.provenance.trim().is_empty()
            || artifact.delta_submit_ns.is_empty()
            || artifact.delta_cancel_ns.is_empty()
            || artifact.delta_md_ns.is_empty()
        {
            return Err(SimError::Parse(
                "latency artifact: пустые сэмплы либо пустой provenance (D7/SM-I-8)".into(),
            ));
        }
        self.entries
            .insert((artifact.venue, artifact.symbol.clone()), artifact);
        Ok(())
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
        self.entries.insert(
            (venue, symbol.to_string()),
            LatencyArtifact {
                schema_version: 1,
                venue,
                symbol: symbol.to_string(),
                provenance: provenance.to_string(),
                delta_submit_ns: submit_ns,
                delta_cancel_ns: cancel_ns,
                delta_md_ns: md_ns,
            },
        );
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
        let artifact = self
            .entries
            .get(&(venue, symbol.to_string()))
            .ok_or_else(|| SimError::MissingLatency {
                venue,
                symbol: symbol.to_string(),
            })?;
        // Фиксированный порядок вызовов rng: submit, cancel, md.
        let pick = |rng: &mut SplitMix64, samples: &[u64]| -> u64 {
            let u = rng.next_f64();
            let idx = ((u * samples.len() as f64) as usize).min(samples.len() - 1);
            samples[idx]
        };
        let delta_submit_ns = pick(rng, &artifact.delta_submit_ns);
        let delta_cancel_ns = pick(rng, &artifact.delta_cancel_ns);
        let delta_md_ns = pick(rng, &artifact.delta_md_ns);
        Ok(LatencyDraw {
            delta_submit_ns,
            delta_cancel_ns,
            delta_md_ns,
        })
    }

    /// Стресс ×k к латентности (RC-I-10: стресс — отдельный прогон через ту же модель).
    pub fn scaled(&self, factor: f64) -> LatencyTable {
        let entries = self
            .entries
            .iter()
            .map(|(key, artifact)| {
                let scale = |v: &[u64]| -> Vec<u64> {
                    v.iter()
                        .map(|&x| (x as f64 * factor).round() as u64)
                        .collect()
                };
                (
                    key.clone(),
                    LatencyArtifact {
                        schema_version: artifact.schema_version,
                        venue: artifact.venue,
                        symbol: artifact.symbol.clone(),
                        provenance: artifact.provenance.clone(),
                        delta_submit_ns: scale(&artifact.delta_submit_ns),
                        delta_cancel_ns: scale(&artifact.delta_cancel_ns),
                        delta_md_ns: scale(&artifact.delta_md_ns),
                    },
                )
            })
            .collect();
        LatencyTable { entries }
    }
}
