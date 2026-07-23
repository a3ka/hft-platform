//! book — L2-стакан + производные (docs/fa/book.md). Детерминированный, без I/O.
//!
//! Данные приходят снапшотами (Binance @depth20, HL l2Book) → `apply_snapshot` заменяет книгу.
//! Примитив OBI: `depth_within(side, pct)` — суммарный размер в полосе pct от mid.
//! Всё в fixed-point i64 ×1e8 (contracts::PRICE_SCALE).

use std::collections::BTreeMap;
use std::collections::HashMap;

use contracts::{Level, MdEvent, MdPayload, Side, Venue};

/// L2-стакан одного инструмента. bids/asks: цена(i64) → размер(i64), оба отсортированы по цене.
#[derive(Debug, Default, Clone)]
pub struct OrderBook {
    bids: BTreeMap<i64, i64>,
    asks: BTreeMap<i64, i64>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Применить ИНКРЕМЕНТАЛЬНУЮ дельту (Binance `@depth` diff / эквивалент).
    /// Семантика зеркалит `venue-binance::apply_diff_to_book` §A (live == replay книги):
    /// `size == 0` → удалить уровень; `size > 0` → upsert (set). Неупомянутые цены НЕ
    /// трогаются (diff — НЕ источник истины о неупомянутом). Пустая сторона `[]` — цикл
    /// пуст → no-op, НЕ очистка стороны (testing.md «отсутствие», класс TD-016).
    /// M-29.
    pub fn apply_delta(&mut self, bids: &[Level], asks: &[Level]) {
        for l in bids {
            if l.size == 0 {
                self.bids.remove(&l.price);
            } else {
                self.bids.insert(l.price, l.size);
            }
        }
        for l in asks {
            if l.size == 0 {
                self.asks.remove(&l.price);
            } else {
                self.asks.insert(l.price, l.size);
            }
        }
    }

    /// Заменить книгу снапшотом (наши данные — снапшоты; JR-first, без diff-sync на старте).
    pub fn apply_snapshot(&mut self, bids: &[Level], asks: &[Level]) {
        self.bids.clear();
        self.asks.clear();
        for l in bids {
            if l.size > 0 {
                self.bids.insert(l.price, l.size);
            }
        }
        for l in asks {
            if l.size > 0 {
                self.asks.insert(l.price, l.size);
            }
        }
    }

    /// Лучший бид (наибольшая цена покупки).
    pub fn best_bid(&self) -> Option<i64> {
        self.bids.keys().next_back().copied()
    }
    /// Лучший аск (наименьшая цена продажи).
    pub fn best_ask(&self) -> Option<i64> {
        self.asks.keys().next().copied()
    }

    /// Середина (i64, целочисленное деление). None если книга односторонняя.
    pub fn mid(&self) -> Option<i64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(b), Some(a)) => Some((b + a) / 2),
            _ => None,
        }
    }

    /// Спред (аск − бид) в fixed-point.
    pub fn spread(&self) -> Option<i64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(b), Some(a)) => Some(a - b),
            _ => None,
        }
    }

    /// Микроцена: size-weighted mid = (bid*ask_sz + ask*bid_sz)/(bid_sz+ask_sz). f64.
    pub fn microprice(&self) -> Option<f64> {
        let (&bp, &bs) = self.bids.iter().next_back()?;
        let (&ap, &as_) = self.asks.iter().next()?;
        let denom = (bs + as_) as f64;
        if denom == 0.0 {
            return None;
        }
        Some((bp as f64 * as_ as f64 + ap as f64 * bs as f64) / denom)
    }

    /// Суммарный размер в полосе `pct` (доля, напр. 0.03 = 3%) от mid, на стороне `side`.
    /// Bid: цены ≥ mid*(1−pct). Ask: цены ≤ mid*(1+pct). Возвращает 0 если mid нет.
    pub fn depth_within(&self, side: Side, pct: f64) -> i64 {
        let mid = match self.mid() {
            Some(m) => m as f64,
            None => return 0,
        };
        match side {
            Side::Buy => {
                let thr = (mid * (1.0 - pct)) as i64;
                self.bids.range(thr..).map(|(_, &s)| s).sum()
            }
            Side::Sell => {
                let thr = (mid * (1.0 + pct)) as i64;
                self.asks.range(..=thr).map(|(_, &s)| s).sum()
            }
        }
    }

    /// Кумулятивный НОТИОНАЛ (USD) в полосе `pct` от mid: Σ (price·size). Для сверки с
    /// платформенным BID/ASK индикатором (значения в $).
    pub fn notional_within(&self, side: Side, pct: f64) -> f64 {
        let mid = match self.mid() {
            Some(m) => m as f64,
            None => return 0.0,
        };
        let scale = contracts::PRICE_SCALE as f64;
        let level_usd = |p: i64, s: i64| (p as f64 / scale) * (s as f64 / scale);
        match side {
            Side::Buy => {
                let thr = (mid * (1.0 - pct)) as i64;
                self.bids.range(thr..).map(|(&p, &s)| level_usd(p, s)).sum()
            }
            Side::Sell => {
                let thr = (mid * (1.0 + pct)) as i64;
                self.asks
                    .range(..=thr)
                    .map(|(&p, &s)| level_usd(p, s))
                    .sum()
            }
        }
    }

    /// Суммарный размер N ЛУЧШИХ уровней стороны (Трек A OBI: top-N imbalance;
    /// M-04 C1 per research/critiques/C-001-M-04-plan.md). Меньше N уровней —
    /// суммируем что есть; пустая сторона/n=0 → 0. Реализация — engine-dev
    /// (M-04 task 2, узкий carve-out на этот метод).
    pub fn top_n_depth(&self, side: Side, n: usize) -> i64 {
        if n == 0 {
            return 0;
        }
        match side {
            // bid: лучшие = наибольшие цены → идём с конца (BTreeMap отсортирован по возрастанию).
            Side::Buy => self.bids.values().rev().take(n).sum(),
            // ask: лучшие = наименьшие цены → идём с начала.
            Side::Sell => self.asks.values().take(n).sum(),
        }
    }

    /// Уровни стороны в порядке ОТ ЛУЧШЕГО К ХУДШЕМУ: (price, size).
    /// SVR-резолюция M-04 (engine-dev, task 2): нужен sim::fill_model::taker_fills
    /// (проедание книги по уровням) — из одних агрегатов уровни не восстановить.
    /// Реализация — engine-dev (расширенный carve-out per milestone M-04).
    pub fn levels(&self, side: Side) -> Vec<(i64, i64)> {
        match side {
            // bid: лучший = наибольшая цена → обход BTreeMap с конца.
            Side::Buy => self.bids.iter().rev().map(|(&p, &s)| (p, s)).collect(),
            // ask: лучший = наименьшая цена → обход с начала.
            Side::Sell => self.asks.iter().map(|(&p, &s)| (p, s)).collect(),
        }
    }

    /// Видимый размер на конкретном ценовом уровне (0, если уровня нет).
    /// SVR-резолюция M-04: queue ahead maker-ордера = объём на НАШЕЙ цене (FA sim §5),
    /// не на лучшем уровне. Реализация — engine-dev (расширенный carve-out).
    pub fn size_at(&self, side: Side, price: i64) -> i64 {
        match side {
            Side::Buy => self.bids.get(&price).copied().unwrap_or(0),
            Side::Sell => self.asks.get(&price).copied().unwrap_or(0),
        }
    }

    pub fn n_levels(&self, side: Side) -> usize {
        match side {
            Side::Buy => self.bids.len(),
            Side::Sell => self.asks.len(),
        }
    }

    /// Насколько далеко крайний уровень стороны от mid, в долях (для диагностики глубины данных).
    pub fn max_reach_pct(&self, side: Side) -> Option<f64> {
        let mid = self.mid()? as f64;
        match side {
            Side::Buy => self.bids.keys().next().map(|&p| (mid - p as f64) / mid),
            Side::Sell => self
                .asks
                .keys()
                .next_back()
                .map(|&p| (p as f64 - mid) / mid),
        }
    }
}

/// Реестр стаканов по (площадка, символ). Кормится MdEvent (L2Snapshot).
#[derive(Debug, Default)]
pub struct Books {
    map: HashMap<(Venue, String), OrderBook>,
}

impl Books {
    pub fn new() -> Self {
        Self::default()
    }

    /// Применить событие. Trade/Funding/прочее игнорируются; книгу двигает L2Snapshot
    /// (полная замена) и L2Delta (инкрементальный diff — M-29, replay/reducer-путь).
    /// Sequencing-поля L2Delta в M-29 НЕ валидируются (gap-detection — follow-up).
    pub fn apply(&mut self, md: &MdEvent) {
        match &md.payload {
            MdPayload::L2Snapshot { bids, asks, .. } => {
                self.map
                    .entry((md.venue, md.symbol.clone()))
                    .or_default()
                    .apply_snapshot(bids, asks);
            }
            MdPayload::L2Delta { bids, asks, .. } => {
                self.map
                    .entry((md.venue, md.symbol.clone()))
                    .or_default()
                    .apply_delta(bids, asks);
            }
            _ => {}
        }
    }

    pub fn get(&self, venue: Venue, symbol: &str) -> Option<&OrderBook> {
        self.map.get(&(venue, symbol.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(price: f64, size: f64) -> Level {
        Level {
            price: contracts::to_fixed(price),
            size: contracts::to_fixed(size),
        }
    }

    #[test]
    fn best_mid_spread() {
        let mut b = OrderBook::new();
        b.apply_snapshot(
            &[lvl(100.0, 1.0), lvl(99.0, 2.0)],
            &[lvl(101.0, 3.0), lvl(102.0, 4.0)],
        );
        assert_eq!(b.best_bid(), Some(contracts::to_fixed(100.0)));
        assert_eq!(b.best_ask(), Some(contracts::to_fixed(101.0)));
        assert_eq!(b.mid(), Some(contracts::to_fixed(100.5)));
        assert_eq!(b.spread(), Some(contracts::to_fixed(1.0)));
    }

    #[test]
    fn depth_bands_filter_by_pct() {
        // mid=100.5; bids at 100 (0.5%), 99 (~1.5%), 90 (~10%)
        let mut b = OrderBook::new();
        b.apply_snapshot(
            &[lvl(100.0, 1.0), lvl(99.0, 2.0), lvl(90.0, 5.0)],
            &[lvl(101.0, 1.0), lvl(110.0, 7.0)],
        );
        // 2% band on bid → includes 100 and 99 (both within 2% of 100.5), not 90
        let d2 = b.depth_within(Side::Buy, 0.02);
        assert_eq!(d2, contracts::to_fixed(3.0)); // 1 + 2
                                                  // 12% band on bid → includes all three
        let d12 = b.depth_within(Side::Buy, 0.12);
        assert_eq!(d12, contracts::to_fixed(8.0)); // 1 + 2 + 5
    }

    #[test]
    fn microprice_between_bid_ask() {
        let mut b = OrderBook::new();
        b.apply_snapshot(&[lvl(100.0, 1.0)], &[lvl(102.0, 1.0)]);
        let mp = b.microprice().unwrap();
        assert!(mp > contracts::to_fixed(100.0) as f64 && mp < contracts::to_fixed(102.0) as f64);
    }
}
