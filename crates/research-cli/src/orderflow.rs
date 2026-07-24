//! OF-I-2/3/4 trade-flow reducers (M-17 Phase A) + DV-I-6 order-flow faithfulness (M-32 Q2б).
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
//! DV-I-6 (M-32 Q2б, sacred в `tests/red_orderflow_faith.rs`):
//!   - `consistency(events, window_ms) -> FaithReport`
//!     Поток diff'а ВЕРЕН? Trade на цене P объёмом S ДОЛЖЕН сопровождаться убыванием книги
//!     на P (дельта, уменьшающая/снимающая P) в seq-окне `(ts, ts+window_ms]` — открытый
//!     слева (Delta на ТОМ ЖЕ ts, что Trade, не считается), закрытый справа.
//!
//! Детерминизм: BTreeMap для стабильной итерации; чистые редьюсеры (без wall-clock/rand/I/O).

use contracts::{Level, Side};
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

// ── DV-I-6: order-flow faithfulness (M-32 Q2б) ──────────────────────────────────────────────

/// Событие в потоке `MdPayload` (Trade + Delta — минимально достаточная проекция
/// без `venue`/`symbol`). Для DV-I-6 нас интересуют только эти два типа; L2Snapshot
/// не входит — снапшоты НЕ «уменьшают» размер уровня в той же семантике, что отмены.
#[derive(Debug, Clone, PartialEq)]
pub enum FaithEvent {
    Delta {
        ts_ms: i64,
        bids: Vec<Level>,
        asks: Vec<Level>,
    },
    Trade {
        ts_ms: i64,
        price: i64,
        side: Side,
        size: i64,
    },
}

/// Отчёт о согласованности trade-flow с book-flow.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FaithReport {
    /// Всего проверено сделок.
    pub checked: u64,
    /// Из них: в окне `(ts, ts+window_ms]` пришла Delta, уменьшающая размер на P
    /// минимум на size сделки (или снимающая P).
    pub consistent: u64,
    /// Из них: в окне НЕ нашлось подходящей Delta — поток НЕ отразил филл.
    pub inconsistent: u64,
}

/// Снимок running-книги на момент сделки (для второго прохода forward-scan).
#[allow(dead_code)]
type TradeSnapshot = (usize, i64, i64, i64, BTreeMap<i64, i64>);

/// Проверить согласованность trade-flow с book-flow. Чистый редьюсер; детерминирован.
///
/// **Алгоритм (single-pass O(N)):**
///   - `running_book`: price → size, инкрементально обновляется по мере итерации.
///   - `pending`: VecDeque незарезолвленных сделок (ts, price, size, current_size_at_P).
///   - Для каждого Trade: проверяем, декрементируется ли size_at(P) при обработке Delta;
///     записываем в `pending` (если ещё не резолвлено) и резолвим при первом подходящем Delta.
///   - Для каждой Delta: применяем к `running_book`; затем пробегаем по `pending`,
///     проверяя декремент; резолвленные переводим в `consistent`/`inconsistent` по исходу.
///   - При достижении конца потока все ещё pending → `inconsistent` (поток не отразил филл).
///   - При переполнении окна `ts - pending_trade.ts > window_ms` → торговля timed out,
///     переводим в `inconsistent` (исключаем из pending).
///
/// Сложность: каждый Delta проходит по `pending` один раз (после резолва — больше не
/// рассматривается); суммарно O(N + T·W), где W — среднее число pending на момент Delta.
/// На реальном потоке BTCUSDT (window=1с, ~10ms/event) W ≤ ~100, итого O(N).
pub fn consistency(events: &[FaithEvent], window_ms: i64) -> FaithReport {
    let mut r = FaithReport::default();
    if window_ms <= 0 {
        // Защита от мусорного окна — нулевая отчётность (как в `footprint_delta`).
        return r;
    }
    let mut book: BTreeMap<i64, i64> = BTreeMap::new();
    let mut pending: std::collections::VecDeque<PendingTrade> = std::collections::VecDeque::new();
    for ev in events {
        match ev {
            FaithEvent::Delta {
                ts_ms: dts,
                bids,
                asks,
            } => {
                let dts_val = *dts;
                apply_delta_to_book(&mut book, bids, asks);
                // Резолвим pending: для каждой сделки проверяем, накопленный декремент с момента
                // сделки до текущего состояния >= size ИЛИ book[P] == 0.
                // Отслеживаем per-trade running_max_size (= book[P] при КАЖДОМ delta в окне).
                // Если book[P] когда-либо был > max_so_far (например, populate), max_so_far
                // обновляется; декремент считается как `max_so_far - cur_size`.
                // Дельта вне окна — игнорируем; trade остаётся в pending до eviction'а.
                let mut i = 0;
                while i < pending.len() {
                    let trade = &mut pending[i];
                    if dts_val.saturating_sub(trade.ts_ms) > window_ms {
                        i += 1;
                        continue;
                    }
                    if dts_val <= trade.ts_ms {
                        i += 1;
                        continue;
                    }
                    let cur_size = book.get(&trade.price).copied().unwrap_or(0);
                    // Обновляем пик book[P] с момента сделки: populate (0 → 10) тоже считается.
                    if cur_size > trade.max_size_since_trade {
                        trade.max_size_since_trade = cur_size;
                    }
                    // Декремент считается от пика — `max_size - cur_size` (или cur_size == 0).
                    let peak = trade.max_size_since_trade;
                    if cur_size == 0 || peak.saturating_sub(cur_size) >= trade.size {
                        r.consistent += 1;
                        pending.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }
            FaithEvent::Trade {
                ts_ms,
                price,
                side: _,
                size,
            } => {
                r.checked += 1;
                // Eviction: сначала убираем просроченные.
                while let Some(front) = pending.front() {
                    if ts_ms.saturating_sub(front.ts_ms) > window_ms {
                        r.inconsistent += 1;
                        pending.pop_front();
                    } else {
                        break;
                    }
                }
                // Запоминаем текущий size_at(price) — это «до» любой будущей Delta.
                let cur_size = book.get(price).copied().unwrap_or(0);
                // Всегда пушим в pending: даже если cur_size == 0, последующая Delta может
                // populate + decrement в окне — тогда учитываем. При end-of-window eviction'е
                // (end-of-stream или на следующей сделке) trade без разрешения → inconsistent.
                pending.push_back(PendingTrade {
                    ts_ms: *ts_ms,
                    price: *price,
                    size: *size,
                    cur_size,
                    max_size_since_trade: cur_size,
                });
            }
        }
    }
    // Конец потока: все, что осталось в pending, не нашли декремента → inconsistent.
    r.inconsistent += pending.len() as u64;
    r
}

/// Запись в очереди незарезолвленных сделок: `(ts_ms, price, size, cur_size_at_P,
/// max_size_since_trade)`. `max_size_since_trade` обновляется при КАЖДОЙ Delta в окне:
/// populate (0→10) тоже считается, чтобы декремент после populate корректно засчитался.
struct PendingTrade {
    ts_ms: i64,
    price: i64,
    size: i64,
    /// `size_at(price)` в момент ПОЯВЛЕНИЯ сделки — для детекции декремента в будущем.
    cur_size: i64,
    /// Пик `size_at(price)` начиная с момента сделки: обновляется при каждой Delta в окне
    /// (populate=0→10 увеличивает peak; дальнейший декремент считается от этого пика).
    max_size_since_trade: i64,
}

/// Применить дельту к running-книге (size==0 = remove, size>0 = upsert).
fn apply_delta_to_book(book: &mut BTreeMap<i64, i64>, bids: &[Level], asks: &[Level]) {
    for l in bids {
        if l.size == 0 {
            book.remove(&l.price);
        } else if l.size > 0 {
            book.insert(l.price, l.size);
        }
    }
    for l in asks {
        if l.size == 0 {
            book.remove(&l.price);
        } else if l.size > 0 {
            book.insert(l.price, l.size);
        }
    }
}
