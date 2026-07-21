//! BacktestExchange — приёмник за швом OrderGateway (FA §2): «встаёт РОВНО туда,
//! где иначе стоял бы venue». Backtest-режим M-04: потребляет Event-поток
//! (book-снапшоты + trades), управляет открытыми SimOrder, отдаёт SimFill in-memory.
//! Формальный trait OrderGateway появится вместе с oms (P3) — форма submit/cancel/
//! on_event уже совместима.
//!
//! Латентность (FA §6): ордер «появляется» на рынке через δ_submit от момента submit
//! (рынок успевает уйти); отмена — через δ_cancel. Гэп в реконструкции книги →
//! эпизод stale, исключается (FA §3 таблица).
//!
//! Реализация — engine-dev (M-04 task 2).

use std::collections::HashMap;

use book::OrderBook;
use contracts::{Event, EventKind, MdPayload, Venue};

use crate::fees::FeeSchedule;
use crate::fill_model;
use crate::latency::LatencyTable;
use crate::rng::SplitMix64;
use crate::types::{
    FillDecision, OrderIntent, OrderKind, QueueState, SimError, SimFill, SimOrder, TradedTick,
};

/// Ордер, поданный, но ещё не «появившийся» на рынке (submit → effective_ts = submit_ts +
/// δ_submit ещё не наступил; FA §6). Активируется при обработке первого события, чьё
/// ts_mono_ns ≥ effective_ts_mono_ns.
struct PendingOrder {
    id: u64,
    intent: OrderIntent,
    submitted_seq: u64,
    effective_ts_mono_ns: u64,
}

/// Отложенный эффект отмены (δ_cancel; FA §6) — до наступления effective_ts ордер
/// может успеть исполниться нормально.
struct PendingCancel {
    order_id: u64,
    effective_ts_mono_ns: u64,
}

pub struct BacktestExchange {
    latency: LatencyTable,
    fees: FeeSchedule,
    rng: SplitMix64,
    /// Реконструированные книги по (venue, symbol) — двигает ТОЛЬКО L2Snapshot (как в `book::Books`).
    books: HashMap<(Venue, String), OrderBook>,
    pending: Vec<PendingOrder>,
    pending_cancels: Vec<PendingCancel>,
    active: HashMap<u64, SimOrder>,
    next_id: u64,
    /// Часы последнего обработанного события (None до первого on_event → submit=NoMarketData).
    last_seq: Option<u64>,
    last_ts_mono_ns: u64,
}

impl BacktestExchange {
    /// Seed ОБЯЗАТЕЛЕН (SM-I-2: «случайный» бэктест недопустим — отсутствие seed
    /// невозможно выразить этим API, что и требуется).
    pub fn new(latency: LatencyTable, fees: FeeSchedule, seed: u64) -> Self {
        Self {
            latency,
            fees,
            rng: SplitMix64::new(seed),
            books: HashMap::new(),
            pending: Vec::new(),
            pending_cancels: Vec::new(),
            active: HashMap::new(),
            next_id: 1,
            last_seq: None,
            last_ts_mono_ns: 0,
        }
    }

    /// Подать намерение. Часы/seq — от последнего on_event (submit до первого события →
    /// Err::NoMarketData). Нет латентности/тарифа для инструмента → Err (SM-I-8; fail-closed
    /// на submit, не на fill). Возвращает order_id.
    pub fn submit(&mut self, intent: OrderIntent) -> Result<u64, SimError> {
        let last_seq = self.last_seq.ok_or(SimError::NoMarketData)?;

        if !self.latency.has(intent.venue, &intent.symbol) {
            return Err(SimError::MissingLatency {
                venue: intent.venue,
                symbol: intent.symbol.clone(),
            });
        }
        if !self.fees.has(intent.venue) {
            return Err(SimError::MissingFees {
                venue: intent.venue,
                symbol: intent.symbol.clone(),
            });
        }

        let draw = self
            .latency
            .draw(intent.venue, &intent.symbol, &mut self.rng)?;
        let id = self.next_id;
        self.next_id += 1;
        self.pending.push(PendingOrder {
            id,
            intent,
            submitted_seq: last_seq,
            effective_ts_mono_ns: self.last_ts_mono_ns + draw.delta_submit_ns,
        });
        Ok(id)
    }

    /// Отмена (эффект через δ_cancel; до эффекта ордер может успеть исполниться).
    pub fn cancel(&mut self, order_id: u64) -> Result<(), SimError> {
        let (venue, symbol) = if let Some(p) = self.pending.iter().find(|p| p.id == order_id) {
            (p.intent.venue, p.intent.symbol.clone())
        } else if let Some(a) = self.active.get(&order_id) {
            (a.intent.venue, a.intent.symbol.clone())
        } else {
            // Уже нет среди открытых (исполнен/уже отменён) — no-op.
            return Ok(());
        };
        self.last_seq.ok_or(SimError::NoMarketData)?;
        let draw = self.latency.draw(venue, &symbol, &mut self.rng)?;
        self.pending_cancels.push(PendingCancel {
            order_id,
            effective_ts_mono_ns: self.last_ts_mono_ns + draw.delta_cancel_ns,
        });
        Ok(())
    }

    /// Активировать созревшие pending-ордера. Maker: ahead = видимый объём на НАШЕМ
    /// ценовом уровне на момент активации (FA §5; 0 если уровня нет) — состояние после
    /// последнего применённого L2Snapshot, консервативно (FA §O). Taker: исполняется
    /// сразу против текущей видимой книги через fill_model::taker_fills; остаток
    /// истекает (решает oms, не sim — FA §5).
    fn activate_matured(&mut self, seq: u64, ts_mono_ns: u64) -> Vec<SimFill> {
        let mut fills = Vec::new();
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].effective_ts_mono_ns <= ts_mono_ns {
                let p = self.pending.remove(i);
                let book = self.books.get(&(p.intent.venue, p.intent.symbol.clone()));
                match p.intent.kind {
                    OrderKind::Maker => {
                        let ahead = book
                            .map(|b| b.size_at(p.intent.side, p.intent.price))
                            .unwrap_or(0);
                        let order = SimOrder {
                            id: p.id,
                            intent: p.intent,
                            submitted_seq: p.submitted_seq,
                            effective_ts_mono_ns: p.effective_ts_mono_ns,
                            queue: QueueState {
                                ahead,
                                cum_traded: 0,
                                filled: 0,
                            },
                        };
                        self.active.insert(order.id, order);
                    }
                    OrderKind::Taker => {
                        if let Some(b) = book {
                            for (price, qty) in fill_model::taker_fills(
                                b,
                                p.intent.side,
                                p.intent.qty,
                                p.intent.price,
                            ) {
                                let notional_e8 = ((price as i128 * qty as i128)
                                    / contracts::PRICE_SCALE as i128)
                                    as i64;
                                let fee_e8 = self
                                    .fees
                                    .fee_e8(p.intent.venue, &p.intent.symbol, false, notional_e8)
                                    .unwrap_or(0);
                                fills.push(SimFill {
                                    order_id: p.id,
                                    price,
                                    qty,
                                    maker: false,
                                    fee_e8,
                                    seq,
                                    ts_mono_ns,
                                });
                            }
                        }
                        // Остаток taker-ордера НЕ додумывается — ордер закрыт.
                    }
                }
            } else {
                i += 1;
            }
        }
        fills
    }

    /// Применить созревшие отложенные отмены — снять ордер из pending/active.
    fn apply_matured_cancels(&mut self, ts_mono_ns: u64) {
        let mut i = 0;
        while i < self.pending_cancels.len() {
            if self.pending_cancels[i].effective_ts_mono_ns <= ts_mono_ns {
                let c = self.pending_cancels.remove(i);
                self.pending.retain(|p| p.id != c.order_id);
                self.active.remove(&c.order_id);
            } else {
                i += 1;
            }
        }
    }

    /// Продвинуть симулятор одним событием журнала; вернуть исполнения на этом тике.
    /// Модель видит только события с seq ≤ текущего (SM-I-4).
    pub fn on_event(&mut self, ev: &Event) -> Vec<SimFill> {
        self.last_seq = Some(ev.seq);
        self.last_ts_mono_ns = ev.ts_mono_ns;

        // Активация/отмена ДО применения эффекта самого события — вновь активированный
        // ордер обязан увидеть traded-тик ЭТОГО ЖЕ события (test_replay_deterministic_given_seed).
        let mut fills = self.activate_matured(ev.seq, ev.ts_mono_ns);
        self.apply_matured_cancels(ev.ts_mono_ns);

        let EventKind::Md(md) = &ev.kind else {
            return fills;
        };

        match &md.payload {
            MdPayload::L2Snapshot { bids, asks, .. } => {
                self.books
                    .entry((md.venue, md.symbol.clone()))
                    .or_default()
                    .apply_snapshot(bids, asks);
            }
            MdPayload::Trade {
                price, size, side, ..
            } => {
                let tick = TradedTick {
                    price: *price,
                    qty: *size,
                    side: *side,
                    seq: ev.seq,
                };
                let mut filled_orders = Vec::new();
                for (id, order) in self.active.iter_mut() {
                    if order.intent.venue != md.venue || order.intent.symbol != md.symbol {
                        continue;
                    }
                    if order.intent.kind != OrderKind::Maker {
                        continue;
                    }
                    let (new_q, decision) = fill_model::on_traded_tick(order, &tick);
                    order.queue = new_q;
                    if let FillDecision::Partial { qty } | FillDecision::Full { qty } = decision {
                        let notional_e8 = ((order.intent.price as i128 * qty as i128)
                            / contracts::PRICE_SCALE as i128)
                            as i64;
                        let fee_e8 = self
                            .fees
                            .fee_e8(order.intent.venue, &order.intent.symbol, true, notional_e8)
                            .unwrap_or(0);
                        fills.push(SimFill {
                            order_id: *id,
                            price: order.intent.price,
                            qty,
                            maker: true,
                            fee_e8,
                            seq: ev.seq,
                            ts_mono_ns: ev.ts_mono_ns,
                        });
                        if order.queue.filled >= order.intent.qty {
                            filled_orders.push(*id);
                        }
                    }
                }
                for id in filled_orders {
                    self.active.remove(&id);
                }
            }
            MdPayload::Funding { .. }
            | MdPayload::OpenInterest { .. }
            | MdPayload::Liquidation { .. }
            | MdPayload::MarginRate { .. } => {}
            // CT-RFC-04: сырая L2-дельта игнорируется симом — честный fill-путь ведёт
            // книгу из L2Snapshot+Trade; учёт дельты поверх снапшота был бы двойным
            // счётом книги, а не входом бэктеста.
            MdPayload::L2Delta { .. } => {}
        }

        fills
    }

    pub fn open_orders(&self) -> usize {
        self.pending.len() + self.active.len()
    }
}
