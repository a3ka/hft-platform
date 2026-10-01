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
use contracts::{Event, EventKind, MdPayload, Side, Venue};
use serde::Deserialize;

use crate::{RegistryStatus, Signal, SignalId, SignalMeta, SignalOut, SignalSpecRef};

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
        let parsed: ObiParams = serde_json::from_value(params.clone())
            .map_err(|e| crate::SignalError::InvalidParams(e.to_string()))?;
        Ok(Self::new(id, version, status, parsed))
    }
}

impl Signal for Obi {
    fn on_event(&mut self, ev: &Event) -> Option<SignalOut> {
        // Только Md-события своего (venue, symbol); прочее — книга не меняется, None.
        let md = match &ev.kind {
            EventKind::Md(md) => md,
            _ => return None,
        };
        if md.venue != self.params.venue || md.symbol != self.params.symbol {
            return None;
        }
        let (bids, asks) = match &md.payload {
            MdPayload::L2Snapshot { bids, asks, .. } => (bids, asks),
            _ => return None,
        };
        self.book.apply_snapshot(bids, asks);

        let (depth_bid, depth_ask) = match &self.params.mode {
            ObiMode::TopN { n_levels } => (
                self.book.top_n_depth(Side::Buy, *n_levels),
                self.book.top_n_depth(Side::Sell, *n_levels),
            ),
            ObiMode::Bands {
                d_bid_pct,
                d_ask_pct,
            } => (
                self.book.depth_within(Side::Buy, *d_bid_pct),
                self.book.depth_within(Side::Sell, *d_ask_pct),
            ),
        };

        let denom = depth_bid + depth_ask;
        if denom == 0 {
            return None; // обе стороны 0 → «нет мнения», не 0/0
        }

        let imbalance = depth_bid as f64 / denom as f64;
        let raw = 2.0 * (imbalance - 0.5) * crate::SIGNAL_VALUE_SCALE as f64;
        let score_e8 =
            (raw.round() as i64).clamp(-crate::SIGNAL_VALUE_SCALE, crate::SIGNAL_VALUE_SCALE);

        if score_e8.abs() < self.params.theta_e8 {
            return None; // ниже порога — «нет мнения», не «мнение=0» (D1)
        }

        Some(SignalOut {
            signal_id: self.id.clone(),
            ts_event_mono_ns: ev.ts_mono_ns,
            value: score_e8,
            status: self.status,
            meta: SignalMeta {
                horizon_ms: self.params.horizon_ms,
            },
        })
    }

    fn spec(&self) -> SignalSpecRef {
        SignalSpecRef {
            id: self.id.clone(),
            version: self.version,
        }
    }
}
