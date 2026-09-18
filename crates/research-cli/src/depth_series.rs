//! OF-I-6 depth time-series: per (side, band) per-bucket depth of book `depth_within`.
//!
//! M-17: pure reducer over a stream of L2-snapshots → linear chart data for the frontend
//! (`code2alpha` + lightweight-charts v5 `LineData{time,value}`). BID and ASK MUST be
//! separate series (not summed) — order-flow signal depends on asymmetry.
//!
//! Контракт (sacred in `tests/red_depth_series.rs`):
//!   `compute(snapshots: &[(i64 /*ts_ms*/, OrderBook)], side: Side, band_pct: f64,
//!           timeframe_ms: i64) -> Vec<(i64 /*bucket_time_s*/, i64 /*depth*/)>`
//!
//!   - `bucket = ts_ms / timeframe_ms`; `bucket_time_s = bucket * timeframe_ms / 1000`
//!     (UDF UTCTimestamp — начало бакета в СЕКУНДАХ);
//!   - внутри бакета — close-семантика: побеждает ПОСЛЕДНИЙ снапшот в бакете;
//!   - пустые бакеты (без снапшотов) НЕ эмитятся (нет выдуманных точек);
//!   - детерминирован: один и тот же вход → один и тот же выход (BTreeMap для стабильного
//!     порядка итерации; итерация по срезу снапшотов сохраняет порядок входа).
//!
//! Граница A: чистый редьюсер. Без wall-clock, без rand, без I/O. ВСЯ информация — во входе.

use book::OrderBook;
use contracts::Side;
use std::collections::BTreeMap;

/// Вычислить временной ряд суммарной глубины `depth_within(side, band_pct)` по бакетам.
///
/// `snapshots` — отсортированный по `ts_ms` срез снапшотов из журнала (порядок задаётся
/// стримом `journal::stream` — стабильный для одного и того же сегмента). Пара `(ts_ms, book)`
/// уже материализована вызывающим (read-only wiring: см. `export_io`); сам reducer работает
/// на `&[(i64, OrderBook)]`, не открывает журнал.
pub fn compute(
    snapshots: &[(i64, OrderBook)],
    side: Side,
    band_pct: f64,
    timeframe_ms: i64,
) -> Vec<(i64, i64)> {
    if timeframe_ms <= 0 {
        // Защита от мусорного timeframe (e.g. 0 или отрицательный) — не паника, не выдумка.
        return Vec::new();
    }
    // BTreeMap по bucket_s: close-семантика (последний снапшот в бакете перезаписывает),
    // детерминированная итерация → детерминированный выход.
    let mut bucket_last: BTreeMap<i64, i64> = BTreeMap::new();
    for (ts_ms, book) in snapshots {
        // ts_ms может быть отрицательным (pre-1970 в синтетике) — деление с остатком в Rust
        // для отрицательных округляет к нулю; эпоха привязки — на совести вызывающего.
        let bucket_idx = ts_ms.div_euclid(timeframe_ms);
        let bucket_s = bucket_idx
            .checked_mul(timeframe_ms)
            .map(|ms| ms / 1000)
            .unwrap_or(0);
        let depth = book.depth_within(side, band_pct);
        bucket_last.insert(bucket_s, depth);
    }
    bucket_last.into_iter().collect()
}
