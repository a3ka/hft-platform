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

use contracts::Event;

use crate::fees::FeeSchedule;
use crate::latency::LatencyTable;
use crate::rng::SplitMix64;
use crate::types::{OrderIntent, SimError, SimFill};

pub struct BacktestExchange {
    latency: LatencyTable,
    fees: FeeSchedule,
    rng: SplitMix64,
    // внутреннее состояние (книги, открытые ордера, часы последнего события) — engine-dev
}

impl BacktestExchange {
    /// Seed ОБЯЗАТЕЛЕН (SM-I-2: «случайный» бэктест недопустим — отсутствие seed
    /// невозможно выразить этим API, что и требуется).
    pub fn new(latency: LatencyTable, fees: FeeSchedule, seed: u64) -> Self {
        Self {
            latency,
            fees,
            rng: SplitMix64::new(seed),
        }
    }

    /// Подать намерение. Часы/seq — от последнего on_event (submit до первого события →
    /// Err::NoMarketData). Нет латентности/тарифа для инструмента → Err (SM-I-8; fail-closed
    /// на submit, не на fill). Возвращает order_id.
    pub fn submit(&mut self, intent: OrderIntent) -> Result<u64, SimError> {
        let _ = (intent, &self.latency, &self.fees, &mut self.rng);
        todo!("engine-dev: M-04 task 2")
    }

    /// Отмена (эффект через δ_cancel; до эффекта ордер может успеть исполниться).
    pub fn cancel(&mut self, order_id: u64) -> Result<(), SimError> {
        let _ = order_id;
        todo!("engine-dev: M-04 task 2")
    }

    /// Продвинуть симулятор одним событием журнала; вернуть исполнения на этом тике.
    /// Модель видит только события с seq ≤ текущего (SM-I-4).
    pub fn on_event(&mut self, ev: &Event) -> Vec<SimFill> {
        let _ = ev;
        todo!("engine-dev: M-04 task 2")
    }

    pub fn open_orders(&self) -> usize {
        todo!("engine-dev: M-04 task 2")
    }
}
