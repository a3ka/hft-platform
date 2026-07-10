//! T2/T3 доменный словарь sim (FA §3). Владеет крейт; architect-каркас.

use contracts::{Side, Venue};

/// Maker (лимитка в очередь) | Taker (проедание видимой книги).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderKind {
    Maker,
    Taker,
}

/// Намерение от oms/harness — форма идентична тому, что получил бы реальный venue (FA §2).
#[derive(Debug, Clone, PartialEq)]
pub struct OrderIntent {
    pub venue: Venue,
    pub symbol: String,
    pub side: Side,
    /// Лимит-цена ×1e8 (maker) / marketable-предел (taker).
    pub price: i64,
    /// Размер ×1e8.
    pub qty: i64,
    pub kind: OrderKind,
}

/// Состояние очереди maker-ордера (FA §5). ahead = видимый объём ПЕРЕД нами на уровне
/// в момент активации (хвост уровня); cum_traded — суммарный traded-объём по нашей цене
/// с момента активации; filled — уже исполнено нам.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueState {
    pub ahead: i64,
    pub cum_traded: i64,
    pub filled: i64,
}

/// Открытый ордер в симуляторе.
#[derive(Debug, Clone, PartialEq)]
pub struct SimOrder {
    pub id: u64,
    pub intent: OrderIntent,
    /// seq последнего события, видимого на момент submit (граница SM-I-4).
    pub submitted_seq: u64,
    /// Момент (ts_mono_ns), с которого ордер «появился» на рынке: submit + δ_submit (FA §6).
    pub effective_ts_mono_ns: u64,
    pub queue: QueueState,
}

/// Результат fill_model на одном тике (FA §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillDecision {
    NoFill,
    Partial { qty: i64 },
    Full { qty: i64 },
}

/// Трейд на уровне (из Md(Trade) журнала) — единственный источник maker-заполнений (SM-I-6).
#[derive(Debug, Clone, PartialEq)]
pub struct TradedTick {
    pub price: i64,
    pub qty: i64,
    pub side: Side,
    pub seq: u64,
}

/// Исполнение, которое отдаёт sim (T2; в журнал как Ord(...) — только с P3 contract-RFC).
#[derive(Debug, Clone, PartialEq)]
pub struct SimFill {
    pub order_id: u64,
    pub price: i64,
    pub qty: i64,
    pub maker: bool,
    /// Комиссия ×1e8 USD (отрицательная = ребейт).
    pub fee_e8: i64,
    pub seq: u64,
    pub ts_mono_ns: u64,
}

#[derive(Debug)]
pub enum SimError {
    /// SM-I-8: нет измеренного распределения — Halt, не default.
    MissingLatency {
        venue: Venue,
        symbol: String,
    },
    /// FA §7: нет тарифа — Halt, не «нулевая комиссия».
    MissingFees {
        venue: Venue,
        symbol: String,
    },
    /// Рынок ещё не виден (submit до первого события).
    NoMarketData,
    Io(std::io::Error),
    Parse(String),
}
