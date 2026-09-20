//! book — L2-стакан + производные (docs/fa/book.md). Детерминированный, без I/O.
//!
//! Данные приходят снапшотами (Binance @depth20, HL l2Book) → `apply_snapshot` заменяет книгу.
//! Примитив OBI: `depth_within(side, pct)` — суммарный размер в полосе pct от mid.
//! Всё в fixed-point i64 ×1e8 (contracts::PRICE_SCALE).

use std::collections::BTreeMap;
use std::collections::HashMap;

use contracts::{Level, MdEvent, MdPayload, Side, Venue};
use serde::{Deserialize, Serialize};

/// Результат `apply_l2delta` по непрерывности update-id (M-30 GD-I-1..6).
/// `Applied` — дельта чейнится к предыдущей, книга обновлена.
/// `Gap` — разрыв непрерывности ИЛИ книга уже `stale`; дельта НЕ применена (fail-closed,
/// тот же принцип, что риск-слой `RK`: неизвестный/разорванный вход → отказ, не «применить
/// наугад»). `apply_snapshot` — единственный выход из `Gap` (ресинк, ребутстрап чейна).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ContinuityStatus {
    Applied,
    Gap,
}

/// L2-стакан одного инструмента. bids/asks: цена(i64) → размер(i64), оба отсортированы по цене.
///
/// M-38b (TD-044): `#[derive(Serialize, Deserialize)]` обязателен для чекпоинт-редьюсера
/// (`crates/gateway/checkpoint`): все четыре поля — bids/asks (через public levels),
/// `last_final_update_id` и `stale` (через приватные поля). Соблазнительная реализация
/// чекпоинта «сохранить `levels()`, восстановить через `apply_snapshot()`» теряет оба
/// приватных поля и роняет gap-детекцию (класс тихой лжи, см. milestone §Findings и
/// `crates/book/tests/red_orderbook_serde_roundtrip.rs`). Serde сериализует приватные
/// поля напрямую — обходного конструктора не появляется, приватизация соблюдена.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    bids: BTreeMap<i64, i64>,
    asks: BTreeMap<i64, i64>,
    /// `final_update_id` последней УСПЕШНО чейнённой дельты (`apply_l2delta → Applied`).
    /// `None` сразу после `new` / `apply_snapshot` (книга «свежая», чейн ещё не заведён —
    /// следующая дельта будет bootstrap).
    /// M-30 GD-I-1..6.
    last_final_update_id: Option<u64>,
    /// Книга недостоверна из-за разрыва непрерывности (gap) дельт. Fail-closed: дальнейшие
    /// дельты (даже «валидные» по виду) отвергаются до `apply_snapshot` (ресинк). M-30 GD-I-2/4/6.
    stale: bool,
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
    /// M-30: ресинк — снапшот сбрасывает `last_final_update_id = None` и `stale = false`,
    /// чтобы следующая дельта завела чейн заново (bootstrap). Это ЕДИНСТВЕННЫЙ путь выхода
    /// из `stale` (GD-I-6).
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
        self.last_final_update_id = None;
        self.stale = false;
    }

    /// Применить L2-де льту С ВАЛИДАЦИЕЙ НЕПРЕРЫВНОСТИ update-id (M-30 GD-I-1..6,
    /// чейнинг как в `venue-binance::handle_diff`).
    ///
    /// Правила:
    /// - **Bootstrap** (`last_final_update_id == None`): дельта — первая после снапшота
    ///   (или `new`); применить (`apply_delta`), `last_final = final_update_id`, `Applied`.
    /// - **Stale** (книга уже недостоверна из-за прошлого gap): НЕ применять, вернуть `Gap`
    ///   (fail-closed, консюмер (gateway heatmap/depth) видит период недостоверным через
    ///   `is_stale()`).
    /// - **Continuity OK** (чейн сходится): применить, `last_final = final_update_id`, `Applied`.
    ///   - Спот (`prev_final_update_id == None`): `first_update_id == last_final + 1`
    ///     (Binance spot: `U == prev.u + 1`).
    ///   - Фьючерс (`prev_final_update_id == Some(pu)`): `pu == last_final`
    ///     (Binance futures: `pu == prev.u`).
    /// - **Gap** (continuity нарушена): НЕ применять, `stale = true`, `Gap`. Применение
    ///   разорванной дельты портит книгу (fail-closed: «нет апдейта» лучше, чем ложный апдейт).
    ///
    /// Без побочных эффектов кроме состояния книги; детерминированный (нет `rand()`/wall-clock,
    /// BK-I-1/4). Размеры/цены — fixed-point (i64 ×1e8) из `Level`.
    pub fn apply_l2delta(
        &mut self,
        bids: &[Level],
        asks: &[Level],
        first_update_id: u64,
        final_update_id: u64,
        prev_final_update_id: Option<u64>,
    ) -> ContinuityStatus {
        // Stale-книга отвергает всё до ресинка (GD-I-2/4/6). Fail-closed: даже «валидная по
        // виду» дельта на stale-книге — не применять, пока не пришёл свежий снапшот.
        if self.stale {
            return ContinuityStatus::Gap;
        }

        let last = match self.last_final_update_id {
            // Bootstrap (GD-I-5): чейн ещё не заведён → принимаем дельту как первую.
            None => {
                self.apply_delta(bids, asks);
                self.last_final_update_id = Some(final_update_id);
                return ContinuityStatus::Applied;
            }
            Some(l) => l,
        };

        // Continuity check (GD-I-1..4):
        //   спот — `U == prev.u + 1`; фьючерс — `pu == prev.u`.
        let ok = match prev_final_update_id {
            None => first_update_id == last.saturating_add(1),
            Some(pu) => pu == last,
        };
        if !ok {
            self.stale = true;
            return ContinuityStatus::Gap;
        }

        self.apply_delta(bids, asks);
        self.last_final_update_id = Some(final_update_id);
        ContinuityStatus::Applied
    }

    /// Книга недостоверна (gap в чейне дельт) — консюмер ОБЯЗАН пометить период данных
    /// недостоверным (heatmap/depth — `stale`-флаг на окне). M-30 GD-I-2/4/6.
    pub fn is_stale(&self) -> bool {
        self.stale
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
    /// (полная замена, ресинк) и L2Delta (инкрементальный diff — M-29 raw `apply_delta`
    /// для путей без чейнинга; M-30 `apply_l2delta` для чейнинг-aware пути, default).
    ///
    /// **M-30:** L2Delta-путь в `Books::apply` ИДЁТ через `apply_l2delta` с передачей
    /// sequencing-полей (`first_update_id`/`final_update_id`/`prev_final_update_id`) — это
    /// единственный публичный путь дельт через `Books`, чтобы gap-детекция была
    /// материал-и-и-дефолт (BK-I-3, нельзя «забыть»). `apply_delta` (raw) сохранён как
    /// публичный метод на `OrderBook` для случаев, где sequencing не нужен (фикстуры,
    /// unit-тесты редьюсера). На `Gap` книга остаётся `stale` (доступно через
    /// `OrderBook::is_stale()`).
    pub fn apply(&mut self, md: &MdEvent) {
        match &md.payload {
            MdPayload::L2Snapshot { bids, asks, .. } => {
                self.map
                    .entry((md.venue, md.symbol.clone()))
                    .or_default()
                    .apply_snapshot(bids, asks);
            }
            MdPayload::L2Delta {
                bids,
                asks,
                first_update_id,
                final_update_id,
                prev_final_update_id,
                ..
            } => {
                self.map
                    .entry((md.venue, md.symbol.clone()))
                    .or_default()
                    .apply_l2delta(
                        bids,
                        asks,
                        *first_update_id,
                        *final_update_id,
                        *prev_final_update_id,
                    );
            }
            _ => {}
        }
    }

    pub fn get(&self, venue: Venue, symbol: &str) -> Option<&OrderBook> {
        self.map.get(&(venue, symbol.to_string()))
    }

    /// M-51 (DET-I-2/PL-I-1): детерминированный обход проекции — инструменты в
    /// возрастающем порядке `(venue, symbol)`. Без него состояние проекции невозможно
    /// снять целиком, не положившись на порядок `HashMap` (недетерминирован между
    /// процессами/прогонами). `Venue` (T1, `crates/contracts`) не несёт `Ord` — вне зоны
    /// engine-dev расширять контракт ради этого — поэтому порядок задаётся тем же
    /// представлением, в котором `Venue` уже уходит наружу (`{venue:?}`), а не памятью.
    pub fn iter_sorted(&self) -> Vec<((Venue, &str), &OrderBook)> {
        // DET-OK: обход HashMap сразу пересортировывается ниже по (venue Debug, symbol) —
        // итоговый порядок определяется данными, а не хэш-сидом процесса (DET-I-2/PL-I-1).
        let mut out: Vec<((Venue, &str), &OrderBook)> = self
            .map
            .iter()
            .map(|((v, s), b)| ((*v, s.as_str()), b))
            .collect();
        out.sort_by(|a, b| {
            let ka = (format!("{:?}", (a.0).0), (a.0).1);
            let kb = (format!("{:?}", (b.0).0), (b.0).1);
            ka.cmp(&kb)
        });
        out
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
