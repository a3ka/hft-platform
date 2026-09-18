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
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

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

/// Проверить согласованность trade-flow с book-flow. Чистый редьюсер; детерминирован.
///
/// **Алгоритм (один forward-pass):**
///   - `book` обновляется инкрементально во внешнем цикле;
///   - `PendingSet::by_time` истекает строго с front по timestamp;
///   - `PendingSet::by_price` направляет Delta только к сделкам на затронутой цене;
///   - rebuild `events[..i]` и полный скан pending на каждой Delta отсутствуют.
///
/// Каждая pending-сделка хранит пик видимого размера книги после trade. Её динамический
/// target равен `max_size_since_trade - trade.size`; это позволяет сначала увидеть
/// уровень после старта сегмента, а затем проверить его декремент. Сделка согласована,
/// когда будущая Delta в `(ts, ts+window_ms]` опускает уровень до target или снимает
/// его. Работа ограничена событиями той же цены внутри конечного окна.
pub fn consistency(events: &[FaithEvent], window_ms: i64) -> FaithReport {
    let mut report = FaithReport::default();
    if window_ms <= 0 {
        return report;
    }

    let mut book: BTreeMap<i64, i64> = BTreeMap::new();
    let mut pending = PendingSet::default();

    for event in events {
        match event {
            FaithEvent::Delta { ts_ms, bids, asks } => {
                pending.expire_before(*ts_ms, window_ms, &mut report);
                let touched = apply_delta_to_book(&mut book, bids, asks);
                for price in touched {
                    let current_size = book.get(&price).copied().unwrap_or(0);
                    pending.resolve_price(price, *ts_ms, window_ms, current_size, &mut report);
                }
            }
            FaithEvent::Trade {
                ts_ms,
                price,
                side: _,
                size,
            } => {
                pending.expire_before(*ts_ms, window_ms, &mut report);
                report.checked += 1;
                let size_at_trade = book.get(price).copied().unwrap_or(0);
                pending.push(PendingTrade {
                    ts_ms: *ts_ms,
                    price: *price,
                    size: *size,
                    max_size_since_trade: size_at_trade,
                });
            }
        }
    }

    pending.finish(&mut report);
    report
}

/// Активная сделка. ID хранится отдельно в двух индексах: глобальном временном и
/// ценовом. Сама запись существует только пока сделка не resolved/expired.
struct PendingTrade {
    ts_ms: i64,
    price: i64,
    size: i64,
    max_size_since_trade: i64,
}

#[derive(Default)]
struct PendingSet {
    next_id: u64,
    active: HashMap<u64, PendingTrade>,
    by_time: VecDeque<u64>,
    by_price: HashMap<i64, VecDeque<u64>>,
}

impl PendingSet {
    fn push(&mut self, trade: PendingTrade) {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let price = trade.price;
        self.active.insert(id, trade);
        self.by_time.push_back(id);
        self.by_price.entry(price).or_default().push_back(id);
    }

    /// Истечение идёт только с front временной очереди. Уже resolved ID удаляются
    /// лениво; каждый ID покидает очередь ровно один раз.
    fn expire_before(&mut self, ts_ms: i64, window_ms: i64, report: &mut FaithReport) {
        while let Some(id) = self.by_time.front().copied() {
            let Some(trade) = self.active.get(&id) else {
                self.by_time.pop_front();
                continue;
            };
            if ts_ms.saturating_sub(trade.ts_ms) <= window_ms {
                break;
            }

            let trade = self
                .active
                .remove(&id)
                .expect("front active trade disappeared");
            self.by_time.pop_front();
            report.inconsistent += 1;
            self.prune_price_front(trade.price);
        }
    }

    /// Проверить только pending на цене, реально затронутой Delta. Delta на том же
    /// timestamp не проходит открытую левую границу окна.
    fn resolve_price(
        &mut self,
        price: i64,
        ts_ms: i64,
        window_ms: i64,
        current_size: i64,
        report: &mut FaithReport,
    ) {
        let Some(mut ids) = self.by_price.remove(&price) else {
            return;
        };
        let mut unresolved = VecDeque::with_capacity(ids.len());

        while let Some(id) = ids.pop_front() {
            let matched = self.active.get_mut(&id).is_some_and(|trade| {
                if current_size > trade.max_size_since_trade {
                    trade.max_size_since_trade = current_size;
                }
                ts_ms > trade.ts_ms
                    && ts_ms.saturating_sub(trade.ts_ms) <= window_ms
                    && (current_size == 0
                        || trade.max_size_since_trade.saturating_sub(current_size) >= trade.size)
            });
            if matched {
                self.active.remove(&id);
                report.consistent += 1;
            } else if self.active.contains_key(&id) {
                unresolved.push_back(id);
            }
        }

        if !unresolved.is_empty() {
            self.by_price.insert(price, unresolved);
        }
    }

    /// Удалить resolved/expired ID с начала одной ценовой очереди. Поскольку глобальное
    /// истечение идёт по времени, истёкший ID всегда находится перед всеми ещё живыми
    /// ID той же цены (возможно после уже-resolved tombstones).
    fn prune_price_front(&mut self, price: i64) {
        loop {
            let stale_front = self
                .by_price
                .get(&price)
                .and_then(VecDeque::front)
                .is_some_and(|id| !self.active.contains_key(id));
            if !stale_front {
                break;
            }
            if let Some(ids) = self.by_price.get_mut(&price) {
                ids.pop_front();
            }
        }
        if self.by_price.get(&price).is_some_and(VecDeque::is_empty) {
            self.by_price.remove(&price);
        }
    }

    fn finish(self, report: &mut FaithReport) {
        report.inconsistent += self.active.len() as u64;
    }
}

/// Применить дельту к running-книге и вернуть distinct-набор затронутых цен.
fn apply_delta_to_book(
    book: &mut BTreeMap<i64, i64>,
    bids: &[Level],
    asks: &[Level],
) -> BTreeSet<i64> {
    let mut touched = BTreeSet::new();
    for level in bids.iter().chain(asks) {
        touched.insert(level.price);
        if level.size == 0 {
            book.remove(&level.price);
        } else if level.size > 0 {
            book.insert(level.price, level.size);
        }
    }
    touched
}
