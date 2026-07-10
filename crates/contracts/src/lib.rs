//! Контрактный слой T1 — единый источник правды для форм, пересекающих границы
//! (docs/fa/contracts.md, docs/05-contract-layer.md). M-00: минимальный каркас.
//!
//! Кодировки (locked): деньги/цены — fixed-point i64 ×1e8; время — mono_ns (порядок)
//! + wall_ms (int64 UTC, отчёты). Изменения T1 — только через contract-RFC (CT-I-2).

use serde::{Deserialize, Serialize};

/// Множитель fixed-point для денег/цен (×1e8). Никогда не f64 в деньгах (JR-I-7/CT-I §6).
pub const PRICE_SCALE: i64 = 100_000_000;

/// Версия схемы журнального формата. В каждом сегменте (CT-I-6).
pub const SCHEMA_VERSION: u32 = 0;

/// Единица упорядоченного журнала (docs/fa/journal.md §5). `seq` — тотальный порядок.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub ts_mono_ns: u64,
    pub ts_wall_ms: i64,
    pub kind: EventKind,
}

/// Закрытый версионируемый enum видов событий. Новые варианты — только аддитивно
/// (в конец) через contract-RFC (CT-I §6). M-00: минимальный набор; расширяется пофазно.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventKind {
    /// Системное: recorder/движок жив (heartbeat), связь вверх/вниз.
    Sys(SysEvent),
    // Md(..), Ord(..), Risk(..), Recon(..), Ctl(..) — добавляются в P0/P1/P3 via contract-RFC.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SysEvent {
    Heartbeat,
    ConnUp,
    ConnDown,
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
            kind: EventKind::Sys(SysEvent::Heartbeat),
        };
        let s = serde_json::to_string(&e).unwrap();
        let back: Event = serde_json::from_str(&s).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn price_scale_is_1e8() {
        assert_eq!(PRICE_SCALE, 100_000_000);
    }
}
