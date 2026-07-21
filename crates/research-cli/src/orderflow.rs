//! OF-I-2/3/4 trade-flow reducers (M-17 Phase A).
//!
//! Сторону АГРЕССОРА мы УЖЕ пишем в `MdPayload::Trade.side` (taker; Binance m-flag инверсия) —
//! значит trade-flow order-flow вычислим из данных, что УЖЕ собираем, без изменения захвата и
//! без T1. Phase A: без raw book-дельт (book-flow = absorption/DOM = M-18 / CT-RFC-04).
//!
//! Контракты (sacred в `tests/red_footprint.rs` и `tests/red_footprint_bins.rs`):
//!   - `footprint_delta(trades, timeframe_ms) -> Vec<(i64 /*bucket_s*/, i64 /*delta бакетa*)>`
//!     `delta = Σ(size | side=Buy) − Σ(size | side=Sell)` — ЗНАКОВАЯ агрессия per бар;
//!   - `cumulative_delta(trades, timeframe_ms) -> Vec<(i64, i64)>`
//!     `cum[b] = cum[b-1] + delta[b]` — НАКОПЛЕННАЯ знаковая агрессия до конца бакета
//!     (running, НЕ сброс per-бакет);
//!   - `footprint_bins(trades, timeframe_ms) -> Vec<FootprintBar>`
//!     Полный footprint для custom-series фронта (M-19 Тир2): матрица
//!     `(bucket, price) → {buy_vol, sell_vol, delta=buy−sell}`; разные цены = разные bins.
//!
//! Детерминизм: BTreeMap для стабильной итерации; чистые редьюсеры (без wall-clock/rand/I/O).

use contracts::Side;
use std::collections::BTreeMap;

/// Бар полного footprint'а для кастомного рендера (M-19 Тир2 — cluster chart).
/// `bins` отсортированы по `price` (BTreeMap на этапе редьюсера → детерминированный выход).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootprintBar {
    /// Начало бакета таймфрейма в СЕКУНДАХ (UDF UTCTimestamp).
    pub time_s: i64,
    pub bins: Vec<PriceBin>,
}

/// Per-ценовой бин: агрессия покупок/продаж на конкретной цене внутри бакета.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceBin {
    pub price: i64,
    pub buy_vol: i64,
    pub sell_vol: i64,
    /// `buy_vol − sell_vol` (ЗНАКОВАЯ, не |buy|+|sell|).
    pub delta: i64,
}

/// Per-бар footprint-дельта (скаляр): Σbuy − Σsell по всему бакету.
///
/// `trades`: `(ts_ms, side, size)`. Порядок — по возрастанию `ts_ms` (стрим журнала
/// гарантирует стабильный порядок для одного и того же сегмента; на фикстурах тестов —
/// задан явно).
pub fn footprint_delta(trades: &[(i64, Side, i64)], timeframe_ms: i64) -> Vec<(i64, i64)> {
    if timeframe_ms <= 0 {
        return Vec::new();
    }
    let mut bucket_delta: BTreeMap<i64, i64> = BTreeMap::new();
    for &(ts_ms, side, size) in trades {
        let bucket_s = bucket_index_to_seconds(ts_ms, timeframe_ms);
        let entry = bucket_delta.entry(bucket_s).or_insert(0);
        match side {
            Side::Buy => *entry += size,
            Side::Sell => *entry -= size,
        }
    }
    bucket_delta.into_iter().collect()
}

/// Cumulative delta: НАКОПЛЕННАЯ знаковая агрессия до конца каждого бакета (running).
///
/// Реализация — `fold` поверх `footprint_delta` (та же бакетизация, та же агрегация;
/// running-накопление — снаружи, чтобы не дублировать).
pub fn cumulative_delta(trades: &[(i64, Side, i64)], timeframe_ms: i64) -> Vec<(i64, i64)> {
    if timeframe_ms <= 0 {
        return Vec::new();
    }
    let mut cum: i64 = 0;
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(trades.len());
    // Делаем один проход по отсортированным по ts_ms сделкам; в пределах бакета
    // суммируем ЗНАКОВУЮ агрессию, при переходе — flush кумуляты с running.
    //
    // Контракт теста: cum ОДИН РАЗ per бакет (на конце бакета); внутри бакета
    // накапливаем, на границе — emit. Это эквивалентно: sum per-bucket → running fold,
    // но без двойного прохода и без аллокации промежуточного Vec.
    let mut current_bucket: Option<i64> = None;
    let mut current_delta: i64 = 0;
    for &(ts_ms, side, size) in trades {
        let bucket_s = bucket_index_to_seconds(ts_ms, timeframe_ms);
        match current_bucket {
            Some(b) if b == bucket_s => {
                current_delta += signed_size(side, size);
            }
            Some(b) => {
                cum += current_delta;
                out.push((b, cum));
                current_bucket = Some(bucket_s);
                current_delta = signed_size(side, size);
            }
            None => {
                current_bucket = Some(bucket_s);
                current_delta = signed_size(side, size);
            }
        }
    }
    if let Some(b) = current_bucket {
        cum += current_delta;
        out.push((b, cum));
    }
    out
}

/// Per-ценовой footprint: для каждого бакета — bins по ВСЕМ ценам, на которых были сделки.
///
/// `trades`: `(ts_ms, price, side, size)`. Бин создаётся ТОЛЬКО для цены, по которой
/// пришла хотя бы одна сделка — не выдумываем уровни. `delta = buy − sell`.
pub fn footprint_bins(trades: &[(i64, i64, Side, i64)], timeframe_ms: i64) -> Vec<FootprintBar> {
    if timeframe_ms <= 0 {
        return Vec::new();
    }
    // bucket_s -> price -> (buy_vol, sell_vol)
    let mut bucket_prices: BTreeMap<i64, BTreeMap<i64, (i64, i64)>> = BTreeMap::new();
    for &(ts_ms, price, side, size) in trades {
        let bucket_s = bucket_index_to_seconds(ts_ms, timeframe_ms);
        let bin = bucket_prices
            .entry(bucket_s)
            .or_default()
            .entry(price)
            .or_insert((0, 0));
        match side {
            Side::Buy => bin.0 += size,
            Side::Sell => bin.1 += size,
        }
    }
    bucket_prices
        .into_iter()
        .map(|(time_s, prices)| {
            let bins: Vec<PriceBin> = prices
                .into_iter()
                .map(|(price, (buy_vol, sell_vol))| PriceBin {
                    price,
                    buy_vol,
                    sell_vol,
                    delta: buy_vol - sell_vol,
                })
                .collect();
            FootprintBar { time_s, bins }
        })
        .collect()
}

#[inline]
fn signed_size(side: Side, size: i64) -> i64 {
    match side {
        Side::Buy => size,
        Side::Sell => -size,
    }
}

/// `ts_ms → начало бакета в СЕКУНДАХ`. `ts_ms / timeframe_ms` (целочисленное, с эвклидовым
/// делением для отрицательных, чтобы бакет был предсказуем для синтетики с pre-1970).
#[inline]
fn bucket_index_to_seconds(ts_ms: i64, timeframe_ms: i64) -> i64 {
    let bucket_idx = ts_ms.div_euclid(timeframe_ms);
    // (bucket_idx * timeframe_ms) / 1000 — переполнение i64 при |ts_ms| > ~9.2e15 мс
    // (= 290k лет) — нереалистично для journal-потока, но клемпим для гигиены.
    bucket_idx
        .checked_mul(timeframe_ms)
        .map(|ms| ms / 1000)
        .unwrap_or(0)
}
