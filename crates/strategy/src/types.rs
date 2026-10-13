//! T2-словарь Слоя 4 (docs/fa/strategy-brain.md §3). Владеет крейт `strategy`.
//!
//! `OrderIntent`/`OrderKind` ПЕРЕЕХАЛИ сюда из `sim` (M-07 D1): продюсер формы — strategy,
//! консюмеры — `sim::BacktestExchange` (бэктест) и `oms`/venue (live). Если бы форма жила
//! в `sim`, live-`runner` линковал бы СИМУЛЯТОР ради типа. `sim` ре-экспортирует
//! (`pub use strategy::{OrderIntent, OrderKind}`) — определение ровно одно (ST-I-7).

use contracts::{Side, Venue};

use alpha::Instrument;

/// Maker (лимитка в очередь) | Taker (проедание видимой книги).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderKind {
    Maker,
    Taker,
}

/// Намерение стратегии — форма идентична тому, что получил бы реальный venue.
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

/// Обратная связь исполнения (T2, M-07 D2). `strategy` НЕ знает про `sim::SimFill`
/// (иначе зависимость Слой4→Слой6 и цикл): мост строит раннер — `sim::StrategyBacktest`
/// в бэктесте, `runner` из `Ord(Fill)` в live.
#[derive(Debug, Clone, PartialEq)]
pub struct FillReport {
    pub instrument: Instrument,
    pub side: Side,
    pub price_e8: i64,
    pub qty_e8: i64,
    /// Комиссия ×1e8 USD (отрицательная = ребейт).
    pub fee_e8: i64,
    pub ts_mono_ns: u64,
}

/// Конфиг directional-стратегии v1.
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyConfig {
    /// Деадбенд: `|target − position − in_flight| < min_order_e8` → интента НЕТ
    /// (иначе стратегия дребезжит ордерами на шуме edge).
    pub min_order_e8: i64,
    /// Срок жизни записи in-flight по **event-time** (никакого wall-clock): интент,
    /// не давший филла за это время, считается умершим → можно переотправить (ST-I-3).
    pub intent_ttl_ms: i64,
    /// Запас маркетабельности лимит-цены тейкера в базисных пунктах (100 bp = 1%).
    pub marketable_margin_bp: i64,
    /// v1: `Taker` (directional). MM-котирование — следующая итерация (нужен oms+risk).
    pub kind: OrderKind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StrategyError {
    InvalidConfig(String),
}
