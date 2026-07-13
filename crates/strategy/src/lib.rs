//! strategy — Слой 4 (docs/fa/strategy-brain.md §6): ЕДИНСТВЕННЫЙ код торговых решений.
//! `Event → signals → alpha → portfolio → diff(target vs current) → OrderIntent`.
//!
//! Тот же самый объект гоняет бэктест (`sim::StrategyBacktest`) и будущий live (`runner`) —
//! отсюда равенство DESIGN §1 №2 (`backtest == paper == live`). Крейт структурно не знает,
//! кто его исполняет: нет зависимостей на `sim`/`venue-*`/`journal`/`risk` (ST-I-6).
//!
//! ⚠ Здесь НЕТ риск-гейта. Между `strategy` и `oms` в M-08 встанет fail-closed `risk`
//! (`RiskApproved<Order>`, RK-I-1..10). `OrderIntent` — намерение, не разрешение.
//!
//! Каркас (T2-типы + трейт) — architect (M-07 task 1, sacred-контракт).
//! Реализация `DirectionalStrategy` — engine-dev (M-07 task 4).

pub mod types;

use std::collections::BTreeMap;

use alpha::{Alpha, Instrument};
use book::Books;
use contracts::{Event, EventKind, Side};
use portfolio::{size as portfolio_size, Position, RiskBudget};
use signals::Signal;

pub use types::{FillReport, OrderIntent, OrderKind, StrategyConfig, StrategyError};

/// Граница Слоя 4 (FA §6). `on_event` — чистый редьюсер (никакого I/O/часов/будущего);
/// `on_fill` — единственный способ подвинуть позицию (фантомных позиций не существует).
pub trait Strategy {
    fn on_event(&mut self, ev: &Event) -> Vec<OrderIntent>;
    fn on_fill(&mut self, fill: &FillReport);
    fn position_e8(&self, instrument: &Instrument) -> i64;
}

/// Ордер в полёте: сколько мы уже запросили, но ещё не получили филлом (M-07 D4).
/// Истекает по event-time через `StrategyConfig::intent_ttl_ms` — без него стратегия
/// шлёт новый интент на каждом тике, пока филл не дошёл (ST-I-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InFlight {
    /// Знаковая запрошенная дельта (×1e8).
    pub delta_e8: i64,
    pub submitted_ts_mono_ns: u64,
}

/// Directional-стратегия v1 (taker-вход/выход по целевой позиции).
pub struct DirectionalStrategy {
    signals: Vec<Box<dyn Signal>>,
    alpha: Box<dyn Alpha>,
    budget: RiskBudget,
    cfg: StrategyConfig,
    /// Реконструкция стакана — только из Md-событий (marketable-цена интента).
    books: Books,
    /// BTreeMap, не HashMap: порядок обхода = часть детерминизма (ST-I-4).
    positions: BTreeMap<Instrument, i64>,
    in_flight: BTreeMap<Instrument, InFlight>,
}

impl DirectionalStrategy {
    /// Конфиг валидируется на входе (fail-closed): `min_order_e8 > 0`, `intent_ttl_ms > 0`,
    /// `marketable_margin_bp ≥ 0`.
    pub fn new(
        signals: Vec<Box<dyn Signal>>,
        alpha: Box<dyn Alpha>,
        budget: RiskBudget,
        cfg: StrategyConfig,
    ) -> Result<Self, StrategyError> {
        if cfg.min_order_e8 <= 0 {
            return Err(StrategyError::InvalidConfig(
                "min_order_e8 must be > 0".into(),
            ));
        }
        if cfg.intent_ttl_ms <= 0 {
            return Err(StrategyError::InvalidConfig(
                "intent_ttl_ms must be > 0".into(),
            ));
        }
        if cfg.marketable_margin_bp < 0 {
            return Err(StrategyError::InvalidConfig(
                "marketable_margin_bp must be >= 0".into(),
            ));
        }
        Ok(DirectionalStrategy {
            signals,
            alpha,
            budget,
            cfg,
            books: Books::new(),
            positions: BTreeMap::new(),
            in_flight: BTreeMap::new(),
        })
    }
}

impl Strategy for DirectionalStrategy {
    /// Конвейер (FA §6): expire in-flight по event-time → books.apply → сигналы (в порядке
    /// объявления) → alpha.update → portfolio::size → diff → интенты.
    /// Нет книги/лучшей цены → интента НЕТ (не «отправим по любой цене»).
    fn on_event(&mut self, ev: &Event) -> Vec<OrderIntent> {
        // ── 1. In-flight expiry по event-time (никакого wall-clock, M-07 D4). ──────────
        let ttl_ns = match (self.cfg.intent_ttl_ms as u64).checked_mul(1_000_000) {
            Some(v) => v,
            None => {
                // intent_ttl_ms уже проверен на >0; overflow нам не грозит де-факто, но
                // защищаемся: тогда TTL = u64::MAX (никогда не протухнет — для эв. тайм).
                u64::MAX
            }
        };
        self.in_flight
            .retain(|_, f| ev.ts_mono_ns <= f.submitted_ts_mono_ns.saturating_add(ttl_ns));

        // ── 2. Применить Md-событие к реконструкции книги (если есть). ─────────────────
        if let EventKind::Md(md) = &ev.kind {
            self.books.apply(md);
        }

        // ── 3. Прогнать сигналы в фиксированном порядке, собрать выходы. ───────────────
        let mut outs = Vec::with_capacity(self.signals.len());
        for s in self.signals.iter_mut() {
            if let Some(out) = s.on_event(ev) {
                outs.push(out);
            }
        }

        // ── 4. Alpha. ────────────────────────────────────────────────────────────────
        let forecasts = self.alpha.update(ev, &outs);

        // ── 5. Portfolio: текущие позиции + budget → target. ─────────────────────────
        let positions_vec: Vec<Position> = self
            .positions
            .iter()
            .map(|(inst, &qty_e8)| Position {
                instrument: inst.clone(),
                qty_e8,
            })
            .collect();
        let targets = portfolio_size(&forecasts, &positions_vec, &self.budget);

        // ── 6. Diff current vs target → интенты (маркетабельная цена; deadband). ───────
        // Marketable формула (FA §6, целочисленно i128):
        //   BUY:  price = best_ask · (10_000 + margin_bp) / 10_000
        //   SELL: price = best_bid · (10_000 − margin_bp) / 10_000
        let margin = self.cfg.marketable_margin_bp as i128;
        let kind = self.cfg.kind;

        let mut intents = Vec::new();
        for target in &targets {
            let current_pos = self.positions.get(&target.instrument).copied().unwrap_or(0);
            let current_inflight = self
                .in_flight
                .get(&target.instrument)
                .map(|f| f.delta_e8)
                .unwrap_or(0);
            let effective_pos = current_pos + current_inflight;
            let delta = target.qty_e8 - effective_pos;

            if delta.abs() < self.cfg.min_order_e8 {
                continue;
            }

            // Source: только Md-события дают стакан; без видимой книги интента НЕТ.
            let (venue, symbol) = match &ev.kind {
                EventKind::Md(md) => (md.venue, md.symbol.clone()),
                _ => continue,
            };
            let book = match self.books.get(venue, &symbol) {
                Some(b) => b,
                None => continue,
            };

            let (side, raw_price) = if delta > 0 {
                let ask = match book.best_ask() {
                    Some(p) => p,
                    None => continue,
                };
                (Side::Buy, ask)
            } else {
                let bid = match book.best_bid() {
                    Some(p) => p,
                    None => continue,
                };
                (Side::Sell, bid)
            };
            let qty = delta.abs();

            // Маркетабельная цена (i128 — переполнение исключено).
            let priced = if margin >= 10_000 {
                match side {
                    Side::Buy => raw_price as i128 * (10_000 + margin) / 10_000,
                    // margin >= 10_000 для SELL — экзотика; ограничиваем 0 (отрицательной цены быть не может).
                    Side::Sell => {
                        let p = raw_price as i128 * (10_000 - margin) / 10_000;
                        if p < 0 {
                            0
                        } else {
                            p
                        }
                    }
                }
            } else {
                let raw = raw_price as i128;
                match side {
                    Side::Buy => raw * (10_000 + margin) / 10_000,
                    Side::Sell => {
                        let p = raw * (10_000 - margin) / 10_000;
                        if p < 0 {
                            0
                        } else {
                            p
                        }
                    }
                }
            };
            let price = priced.clamp(i64::MIN as i128, i64::MAX as i128) as i64;

            // Купить по цене ≤ 0 — бессмысленно; SELL по цене ≤ 0 — отбрасываем.
            if price <= 0 {
                continue;
            }

            intents.push(OrderIntent {
                venue,
                symbol: symbol.clone(),
                side,
                price,
                qty,
                kind,
            });
        }

        // ── 7. Зеркальное обновление in_flight по инструментам, на которые УШЛИ интенты. ──
        let emitted: BTreeMap<Instrument, i64> = {
            let mut m: BTreeMap<Instrument, i64> = BTreeMap::new();
            for intent in &intents {
                let inst = Instrument::new(intent.venue, intent.symbol.clone());
                let signed = match intent.side {
                    Side::Buy => intent.qty,
                    Side::Sell => -intent.qty,
                };
                m.insert(inst, signed);
            }
            m
        };
        for (inst, signed_delta) in emitted {
            self.in_flight.insert(
                inst,
                InFlight {
                    delta_e8: signed_delta,
                    submitted_ts_mono_ns: ev.ts_mono_ns,
                },
            );
        }

        intents
    }

    fn on_fill(&mut self, fill: &FillReport) {
        // Подвинуть позицию на ЗНАКОВЫЙ размер; погасить in-flight на этот инструмент
        // (FA §6, шаг 6: «`on_fill` двигает `position` и гасит `in_flight`»).
        let signed = match fill.side {
            Side::Buy => fill.qty_e8,
            Side::Sell => -fill.qty_e8,
        };
        let entry = self.positions.entry(fill.instrument.clone()).or_insert(0);
        *entry += signed;
        // In-flight для этого инструмента считается отработанным.
        self.in_flight.remove(&fill.instrument);
    }

    fn position_e8(&self, instrument: &Instrument) -> i64 {
        self.positions.get(instrument).copied().unwrap_or(0)
    }
}
