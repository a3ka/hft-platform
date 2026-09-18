//! StrategyBacktest — раннер бэктеста НАСТОЯЩЕЙ стратегии (M-07 D3).
//!
//! Заменяет ad-hoc harness `research-cli/src/grid.rs` (taker-in/Taker-выход по horizon):
//! тот harness мерил логику, которой не будет в live. Здесь через `BacktestExchange`
//! гоняется тот же `dyn Strategy`, который в P3+ будет гонять `runner` на живом фиде.
//!
//! Порядок на каждом событии — СТРОГО (no-lookahead, ST-I-5/SM-I-4):
//!   1. `fills = exchange.on_event(ev)` — биржа применяет событие первой;
//!   2. `strategy.on_fill(FillReport)` по каждому филлу (мост SimFill→FillReport, D2);
//!   3. `intents = strategy.on_event(ev)` — стратегия видит ТОЛЬКО событие ≤ seq;
//!   4. `exchange.submit(intent)` — интенты уходят на биржу (эффект — через δ_submit).
//!
//! Каркас — architect (M-07 task 1). Реализация — engine-dev (M-07 task 5).

use std::collections::{BTreeMap, BTreeSet};

use alpha::Instrument;
use book::Books;
use contracts::{Event, EventKind, Side};
use strategy::{FillReport, Strategy};

use crate::exchange::BacktestExchange;
use crate::fees::FeeSchedule;
use crate::latency::LatencyTable;
use crate::types::SimFill;

/// Детерминированный отчёт прогона (D7). Всё — fixed-point ×1e8; никаких f64 в деньгах.
#[derive(Debug, Clone, PartialEq)]
pub struct BacktestReport {
    /// Сколько интентов стратегия отдала бирже.
    pub intents: usize,
    pub fills: Vec<SimFill>,
    /// Кэш: buy → −(notional + fee); sell → +(notional − fee).
    pub cash_e8: i64,
    /// Итоговые нетто-позиции (знаковые), отсортированы по инструменту.
    pub positions: BTreeMap<Instrument, i64>,
    /// Σ |notional| по всем филлам.
    pub turnover_e8: i64,
    /// Mark-to-market equity-кривая (D7, УТОЧНЕНО M-07 rev 3 после reviewer-находки).
    ///
    /// **Семантика (обязательная, оракул ST-I-8g):**
    /// - РОВНО ОДНА точка на КАЖДОЕ СОБЫТИЕ, на котором биржа вернула ≥1 филл —
    ///   независимо от ЧИСЛА филлов на этом событии (2 филла на одном событии = 1 точка);
    /// - на событиях БЕЗ филлов точек НЕ появляется (ни «добора», ни «догоняющих»);
    /// - точка снимается ПОСЛЕ применения события к книге и ПОСЛЕ доклада всех филлов
    ///   события стратегии: `equity = cash_e8 + Σ position_e8 × mid_e8 / 1e8`;
    /// - следствие-инвариант: `equity_curve_e8.len()` == число РАЗЛИЧНЫХ `seq` в `fills`
    ///   (≤ `fills.len()`, строго меньше при мульти-филловом событии).
    ///
    /// ⚠ Привязка точки к НАКОПЛЕННОМУ числу филлов (`curve.len() < fills.len()`) НЕВЕРНА:
    /// она добирает дефицит на последующих бесфилловых событиях → фантомные точки, снятые
    /// не в те моменты → лишние near-zero доходности → заниженная σ и ЗАВЫШЕННЫЙ Sharpe
    /// в `ValidationReport`/trials-ledger (путь к подписи founder'а, gates §6/§7).
    pub equity_curve_e8: Vec<i64>,
}

/// Мост исполнения: помнит, какому (инструмент, сторона) принадлежит order_id, чтобы
/// собрать `strategy::FillReport` из `SimFill` (в `SimFill` этого нет — и не должно быть).
pub struct StrategyBacktest {
    exchange: BacktestExchange,
    /// BTreeMap, не HashMap: порядок = часть детерминизма (DESIGN §1).
    order_meta: BTreeMap<u64, (Instrument, Side)>,
    /// Все инструменты, которых коснулся прогон (для отчёта positions на каждом);
    /// BTreeSet — порядок итерации детерминирован.
    instruments_seen: BTreeSet<Instrument>,
}

impl StrategyBacktest {
    pub fn new(latency: LatencyTable, fees: FeeSchedule, seed: u64) -> Self {
        StrategyBacktest {
            exchange: BacktestExchange::new(latency, fees, seed),
            order_meta: BTreeMap::new(),
            instruments_seen: BTreeSet::new(),
        }
    }

    /// Прогнать поток событий через стратегию. Прогон детерминирован при фиксированном
    /// seed: два вызова на одном входе дают идентичный `BacktestReport` (ST-I-8).
    pub fn run(&mut self, events: &[Event], strategy: &mut dyn Strategy) -> BacktestReport {
        let mut intents_count: usize = 0;
        let mut fills_out: Vec<SimFill> = Vec::new();
        let mut cash_e8: i128 = 0;
        let mut turnover_e8: i128 = 0;
        let mut equity_curve_e8: Vec<i64> = Vec::new();
        let mut books = Books::new();
        let scale = contracts::PRICE_SCALE as i128;

        for ev in events {
            // ── 1. Биржа видит событие первой (SM-I-4: модель не видит будущего). ────────
            let new_fills = self.exchange.on_event(ev);
            let had_new_fill_this_event = !new_fills.is_empty();

            // ── 2. Доложить филлы стратегии (мост SimFill → FillReport, M-07 D2). ───────
            // ВАЖНО: все on_fill'ы до books.apply — тогда equity-марк отражает позицию,
            // в которую УЖЕ включены все филлы этого события (M-07 D7 / ST-I-8g oracle).
            for sim_fill in &new_fills {
                let (instrument, side) = match self.order_meta.get(&sim_fill.order_id) {
                    Some(meta) => meta.clone(),
                    None => continue, // ордер не наш (теоретически не должно случиться, защита)
                };
                self.instruments_seen.insert(instrument.clone());

                let notional_e128 = (sim_fill.price as i128) * (sim_fill.qty as i128) / scale;
                // Buy тратит (notional + fee); sell приносит (notional − fee).
                let cash_delta = if matches!(side, Side::Buy) {
                    -(notional_e128 + sim_fill.fee_e8 as i128)
                } else {
                    notional_e128 - sim_fill.fee_e8 as i128
                };
                cash_e8 += cash_delta;
                turnover_e8 += notional_e128.abs();

                let report = FillReport {
                    instrument: instrument.clone(),
                    side,
                    price_e8: sim_fill.price,
                    qty_e8: sim_fill.qty,
                    fee_e8: sim_fill.fee_e8,
                    ts_mono_ns: sim_fill.ts_mono_ns,
                };
                strategy.on_fill(&report);
            }
            fills_out.extend(new_fills);

            // ── 2.5 Применить Md-событие к локальной реконструкции книги (mid для MTM). ──
            if let EventKind::Md(md) = &ev.kind {
                books.apply(md);
            }

            // ── 2.7 Equity-точка: РОВНО ОДНА на событие, где биржа дала ≥1 филл (D7/ST-I-8g).
            // Привязка к флагу `had_new_fill_this_event` (НЕ к накопленному fills_out.len()),
            // иначе на бесфилловых событиях между филлами появляются фантомные точки,
            // которые занижают σ returns-серии и ЗАВЫШАЮТ Sharpe → путь в trials-ledger.
            if had_new_fill_this_event {
                let mut eq: i128 = cash_e8;
                for inst in &self.instruments_seen {
                    let pos = strategy.position_e8(inst) as i128;
                    let mid_price = match books.get(inst.venue, &inst.symbol).and_then(|b| b.mid())
                    {
                        Some(p) => p as i128,
                        None => continue,
                    };
                    eq += pos * mid_price / scale;
                }
                let clamped = eq.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
                equity_curve_e8.push(clamped);
            }

            // ── 3. Стратегия видит событие (только ev.seq, без будущего). ─────────────────
            let intents = strategy.on_event(ev);

            // ── 4. Подать интенты на биржу; запомнить order_meta. ────────────────────────
            for intent in &intents {
                intents_count += 1;
                let inst = Instrument::new(intent.venue, intent.symbol.clone());
                self.instruments_seen.insert(inst.clone());
                if let Ok(order_id) = self.exchange.submit(intent.clone()) {
                    self.order_meta.insert(order_id, (inst, intent.side));
                }
                // Err → пропускаем (Bi/SM-SM-I-8 на sim: нет тарифа/латентности → submit не состоится).
            }
        }

        // Финальные positions — через strategy.position_e8 по каждому известному инструменту
        // (ST-I-8b: позиция стратегии = нетто исполнений; берём её прямо).
        let mut positions: BTreeMap<Instrument, i64> = BTreeMap::new();
        for inst in &self.instruments_seen {
            positions.insert(inst.clone(), strategy.position_e8(inst));
        }

        BacktestReport {
            intents: intents_count,
            fills: fills_out,
            cash_e8: cash_e8.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            positions,
            turnover_e8: turnover_e8.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            equity_curve_e8,
        }
    }
}
