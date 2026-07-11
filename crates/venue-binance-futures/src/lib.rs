//! venue-binance-futures — Binance USDT-M перп (fstream). M-06 SKELETON (architect).
//!
//! Emitter-not-owner (docs/fa/venues.md): WS/REST -> parse -> normalize -> MdEvent
//! (`Venue::BinanceFutures`). seq/ts_wall/ts_mono НЕ проставляет — это журнал (JR-I-1),
//! поэтому парс-функции возвращают `MdEvent`, не `Event`.
//!
//! Здесь — ЧИСТЫЕ детерминированные парс-функции (граница нормализации), покрытые
//! RED-оракулами `tests/red_parse.rs`. Тела — STUB (`None`), impl — venue-dev (M-06).
//! Fail-closed: неизвестная/битая форма → `None` (не паникуем, не выдумываем).

use contracts::MdEvent;

/// forceOrder (ликвидация) fstream → `MdEvent{BinanceFutures, Liquidation}`.
/// `side` = сторона ФОРС-ордера `o.S`: `SELL` ⟺ ликвидируется LONG, `BUY` ⟺ ликвидируется
/// SHORT (C-003 note: ЛИКВИДИРУЕМАЯ сторона, НЕ агрессор — иначе CVD/liq-flow инвертирует знак).
pub fn parse_force_order(_json: &str) -> Option<MdEvent> {
    None // STUB — venue-dev (M-06 task 2)
}

/// `/fapi/v1/depth` снапшот → `MdEvent{BinanceFutures, L2Snapshot}`. `ts_exch_ms` = поле `T`.
pub fn parse_depth_snapshot(_symbol: &str, _json: &str) -> Option<MdEvent> {
    None // STUB — venue-dev (M-06 task 2)
}

/// `/fapi/v1/openInterest` → `MdEvent{BinanceFutures, OpenInterest}`. `oi_e8` = БАЗОВЫЙ актив ×1e8.
pub fn parse_open_interest(_symbol: &str, _json: &str) -> Option<MdEvent> {
    None // STUB — venue-dev (M-06 task 3)
}
