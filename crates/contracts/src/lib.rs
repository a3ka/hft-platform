//! Контрактный слой T1 — единый источник правды для форм, пересекающих границы
//! (docs/fa/contracts.md, docs/05-contract-layer.md).
//!
//! Кодировки (locked): деньги/цены/размеры — fixed-point i64 ×1e8 (PRICE_SCALE); время —
//! ts_mono_ns (порядок) + ts_wall_ms (int64 UTC, отчёты) + биржевой ts_exch_ms в payload.
//! Изменения T1 — только через contract-RFC (CT-I-2). schema_version в каждом сегменте (CT-I-6).

use serde::{Deserialize, Serialize};

/// Множитель fixed-point для денег/цен/размеров (×1e8). Никогда не f64 в деньгах (JR-I-7).
pub const PRICE_SCALE: i64 = 100_000_000;

/// Версия схемы журнального формата. В каждом сегменте (CT-I-6).
pub const SCHEMA_VERSION: u32 = 0;

/// Единица упорядоченного журнала (docs/fa/journal.md §5). `seq` — тотальный порядок,
/// назначается журналом (единственный писатель, JR-I-1). Коннекторы seq НЕ проставляют.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub ts_mono_ns: u64,
    pub ts_wall_ms: i64,
    pub kind: EventKind,
}

/// Закрытый версионируемый enum видов событий. Новые варианты — только аддитивно (в конец)
/// через contract-RFC (CT-I §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventKind {
    /// Системное: жив/связь.
    Sys(SysEvent),
    /// Рыночные данные (нормализованные из venue-адаптеров).
    Md(MdEvent),
    // Ord(..), Risk(..), Recon(..), Ctl(..) — добавляются в P3 via contract-RFC.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SysEvent {
    Heartbeat,
    ConnUp(Venue),
    ConnDown(Venue),
}

/// Площадка. Расширяется аддитивно.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Venue {
    Binance,
    Hyperliquid,
}

/// Сторона.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

/// Уровень стакана. price/size — fixed-point ×1e8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub price: i64,
    pub size: i64,
}

/// Нормализованное рыночное событие. `symbol` — канонический тикер площадки как есть
/// (Binance "BTCUSDT" / Hyperliquid "BTC"); нормализация кросс-venue — задача выше (book/strategy).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MdEvent {
    pub venue: Venue,
    pub symbol: String,
    pub payload: MdPayload,
}

/// Тип рыночного апдейта. price/size — fixed-point ×1e8; ставки фандинга — ×1e8.
/// L2Snapshot: и Binance @depth20, и HL l2Book шлют СНАПШОТ стакана целиком на апдейте —
/// пишем как снапшот (снимает нужду в snapshot+diff-sync на старте).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MdPayload {
    Trade {
        price: i64,
        size: i64,
        side: Side,
        ts_exch_ms: i64,
    },
    L2Snapshot {
        bids: Vec<Level>,
        asks: Vec<Level>,
        ts_exch_ms: i64,
    },
    Funding {
        rate_e8: i64,
        ts_exch_ms: i64,
    },
}

impl EventKind {
    /// Хелпер: собрать рыночное событие.
    pub fn md(venue: Venue, symbol: impl Into<String>, payload: MdPayload) -> Self {
        EventKind::Md(MdEvent {
            venue,
            symbol: symbol.into(),
            payload,
        })
    }
}

/// Перевод float-цены в fixed-point ×1e8 (для парсеров venue).
pub fn to_fixed(x: f64) -> i64 {
    (x * PRICE_SCALE as f64).round() as i64
}

/// Обратно в float (для отчётов/логов).
pub fn from_fixed(x: i64) -> f64 {
    x as f64 / PRICE_SCALE as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_roundtrips_through_json() {
        let e = Event {
            seq: 1,
            ts_mono_ns: 42,
            ts_wall_ms: 1_700_000_000_000,
            kind: EventKind::md(
                Venue::Hyperliquid,
                "BTC",
                MdPayload::Trade {
                    price: to_fixed(65000.5),
                    size: to_fixed(0.1),
                    side: Side::Buy,
                    ts_exch_ms: 1_700_000_000_123,
                },
            ),
        };
        let s = serde_json::to_string(&e).unwrap();
        let back: Event = serde_json::from_str(&s).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn fixed_point_roundtrip() {
        assert_eq!(PRICE_SCALE, 100_000_000);
        assert_eq!(to_fixed(1.0), 100_000_000);
        assert!((from_fixed(to_fixed(65000.5)) - 65000.5).abs() < 1e-6);
    }
}
