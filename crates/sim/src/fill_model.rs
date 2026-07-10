//! fill_model — core, чистые функции (FA §3/§5). Пессимизм по конструкции:
//! хвост уровня, без cancel-credit, fill только при ПРЕВЫШЕНИИ traded-объёмом
//! глубины впереди. Реализация — engine-dev (M-04 task 2).

use book::OrderBook;
use contracts::Side;

use crate::types::{FillDecision, QueueState, SimOrder, TradedTick};

/// Maker-решение на одном traded-тике (FA §5, SM-I-1/6):
/// - тик не по нашей цене ИЛИ tick.seq ≤ order.submitted_seq → NoFill, очередь не движется;
/// - иначе cum_traded += tick.qty; fill возможен ТОЛЬКО когда cum_traded > ahead;
///   заполняемый объём = min(остаток_наш, cum_traded − ahead − filled); излишек НЕ
///   переносится вперёд оптимистично.
/// - при нулевом traded-объёме fill невозможен безусловно (FA §3 таблица).
///
/// Чистая функция: (order, tick) → (новая очередь, решение); та же пара входов даёт
/// то же решение (SM-I-2).
pub fn on_traded_tick(order: &SimOrder, tick: &TradedTick) -> (QueueState, FillDecision) {
    let _ = (order, tick);
    todo!("engine-dev: M-04 task 2")
}

/// SM-I-5: отмена ордера впереди нас НЕ освобождает место — функция обязана быть
/// тождеством по QueueState (существует, чтобы соблазн «улучшить» был виден в ревью
/// и пойман RED-тестом, а не размазан по коду).
pub fn on_cancel_ahead(q: QueueState, cancelled_qty: i64) -> QueueState {
    let _ = cancelled_qty;
    let _ = q;
    todo!("engine-dev: M-04 task 2")
}

/// Taker: проедает ВИДИМУЮ книгу от top-of-book до лимит-цены (SM-I-4: только
/// уже видимый объём, без lookahead). Возвращает (цена, объём) по уровням.
/// Недостаточно глубины → частичное исполнение; остаток НЕ додумывается.
pub fn taker_fills(
    visible_book: &OrderBook,
    side: Side,
    qty: i64,
    limit_price: i64,
) -> Vec<(i64, i64)> {
    let _ = (visible_book, side, qty, limit_price);
    todo!("engine-dev: M-04 task 2")
}
