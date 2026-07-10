//! OBI — сигнал №1 (FA §7; hypothesis research/hypotheses/H-20260710-obi-asym.md).
//!
//! Параметрическое семейство: два режима (Трек A/B карточки):
//! - TopN: имбаланс суммарного размера top-N уровней каждой стороны;
//! - Bands: имбаланс depth-полос d_bid%/d_ask% от mid (примитив book::depth_within, D9).
//!
//! score = 2·(imbalance − 0.5) ∈ [-1,1], ×1e8 (D1); эмиссия ТОЛЬКО при |score| ≥ theta_e8,
//! иначе None («нет мнения», не «мнение=0»).
//!
//! Реализация — signal-engineer (M-04 task 3). Каркас типов — architect (sacred-контракт).

use book::OrderBook;
use contracts::{Event, Venue};
use serde::Deserialize;

use crate::{RegistryStatus, Signal, SignalId, SignalOut, SignalSpecRef};

/// Режим вычисления глубины сторон (H-карточка: Трек A / Трек B).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ObiMode {
    /// Трек A: фиксированное число лучших уровней на сторону.
    TopN { n_levels: usize },
    /// Трек B: ценовые полосы в долях от mid (0.03 = 3%).
    Bands { d_bid_pct: f64, d_ask_pct: f64 },
}

/// Params-структура (T3), десериализуется из signals.json.params / grid-ячейки.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ObiParams {
    #[serde(flatten)]
    pub mode: ObiMode,
    /// Порог эмиссии на |score| (×1e8; D1).
    pub theta_e8: i64,
    /// Горизонт для downstream (D2).
    pub horizon_ms: i64,
    pub venue: Venue,
    pub symbol: String,
}

/// Состояние сигнала: params + собственный L2-стакан (book-примитивы, D9).
pub struct Obi {
    id: SignalId,
    version: u32,
    status: RegistryStatus,
    params: ObiParams,
    book: OrderBook,
}

impl Obi {
    pub fn new(id: SignalId, version: u32, status: RegistryStatus, params: ObiParams) -> Self {
        Self {
            id,
            version,
            status,
            params,
            book: OrderBook::new(),
        }
    }

    /// Для registry/grid: params из JSON (валидация → SG-I-8 Reject на мусоре).
    pub fn from_json_params(
        id: SignalId,
        version: u32,
        status: RegistryStatus,
        params: &serde_json::Value,
    ) -> Result<Self, crate::SignalError> {
        let _ = (id, version, status, params);
        todo!("signal-engineer: M-04 task 3")
    }
}

impl Signal for Obi {
    fn on_event(&mut self, ev: &Event) -> Option<SignalOut> {
        let _ = ev;
        let _ = (
            &self.id,
            &self.version,
            &self.status,
            &self.params,
            &self.book,
        );
        todo!("signal-engineer: M-04 task 3 — чистый редьюсер, время только из ev")
    }

    fn spec(&self) -> SignalSpecRef {
        SignalSpecRef {
            id: self.id.clone(),
            version: self.version,
        }
    }
}
