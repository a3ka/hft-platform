//! OF-I-4 OHLCV-бары (M-17 экспорт под `code2alpha` DataFeed / TradingView UDF / lightweight-charts).
//!
//! Свечи для фронта: агрегируем сделки в 1s-OHLCV (фронт агрегирует дальше клиентски до
//! 1m/1h/D). Формат под UDF (`time` в СЕКУНДАХ). Это данные для чарта, не `Signal`-trait.
//!
//! Контракт (sacred в `tests/red_ohlcv.rs`):
//!   `ohlcv_bars(trades: &[(i64 /*ts_ms*/, i64 /*price*/, i64 /*size*/)], timeframe_ms: i64)
//!       -> Vec<OhlcvBar>`
//!   `OhlcvBar { time_s, open, high, low, close, volume }` (pub-поля).
//!   Правила per бакет: open = ПЕРВАЯ цена, high = MAX, low = MIN, close = ПОСЛЕДНЯЯ,
//!   volume = Σsize. 1s база для UDF-совместимости.
//!
//! Детерминизм: BTreeMap для стабильного порядка; чистый редьюсер.

use std::collections::BTreeMap;

/// OHLCV-бар: агрегат trades в бакете таймфрейма. Готов к сериализации в JSON / UDF-shape
/// (`t=time_s`, остальные поля — `o`/`h`/`l`/`c`/`v`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OhlcvBar {
    /// Начало бакета таймфрейма в СЕКУНДАХ (UDF UTCTimestamp).
    pub time_s: i64,
    /// ПЕРВАЯ цена в бакете (по `ts_ms`).
    pub open: i64,
    /// MAX цены в бакете.
    pub high: i64,
    /// MIN цены в бакете.
    pub low: i64,
    /// ПОСЛЕДНЯЯ цена в бакете (по `ts_ms`).
    pub close: i64,
    /// Σsize по сделкам в бакете.
    pub volume: i64,
}

/// Per-бар OHLCV-агрегатор. Инициализируется на первой сделке бакета, дальше —
/// инкрементальный update.
#[derive(Debug, Clone)]
struct Accumulator {
    open: i64,
    high: i64,
    low: i64,
    close: i64,
    volume: i64,
}

impl Accumulator {
    #[inline]
    fn seed(price: i64, size: i64) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume: size,
        }
    }

    #[inline]
    fn update(&mut self, price: i64, size: i64) {
        if price > self.high {
            self.high = price;
        }
        if price < self.low {
            self.low = price;
        }
        self.close = price;
        self.volume += size;
    }
}

/// Свечи из сделок. Бакетизация по `timeframe_ms`, время = `ts_ms / timeframe_ms * timeframe_ms / 1000`.
///
/// `trades` ожидается отсортированным по `ts_ms` (стрим журнала — стабильный порядок
/// сегментов; фикстуры в RED — заданы явно). Не сортируем внутри — это лишний проход
/// и шум для детерминизма-теста; reducer ОДИН проход.
pub fn ohlcv_bars(trades: &[(i64, i64, i64)], timeframe_ms: i64) -> Vec<OhlcvBar> {
    if timeframe_ms <= 0 {
        return Vec::new();
    }
    let mut buckets: BTreeMap<i64, Accumulator> = BTreeMap::new();
    for &(ts_ms, price, size) in trades {
        let bucket_s = bucket_index_to_seconds(ts_ms, timeframe_ms);
        match buckets.get_mut(&bucket_s) {
            Some(acc) => acc.update(price, size),
            None => {
                buckets.insert(bucket_s, Accumulator::seed(price, size));
            }
        }
    }
    buckets
        .into_iter()
        .map(|(time_s, acc)| OhlcvBar {
            time_s,
            open: acc.open,
            high: acc.high,
            low: acc.low,
            close: acc.close,
            volume: acc.volume,
        })
        .collect()
}

#[inline]
fn bucket_index_to_seconds(ts_ms: i64, timeframe_ms: i64) -> i64 {
    let bucket_idx = ts_ms.div_euclid(timeframe_ms);
    bucket_idx
        .checked_mul(timeframe_ms)
        .map(|ms| ms / 1000)
        .unwrap_or(0)
}
