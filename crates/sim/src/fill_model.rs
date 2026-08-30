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
    let mut q = order.queue;

    // Тик не по нашей цене ИЛИ уже видим на момент постановки → очередь не движется.
    if tick.price != order.intent.price || tick.seq <= order.submitted_seq {
        return (q, FillDecision::NoFill);
    }

    q.cum_traded += tick.qty;

    let remaining = order.intent.qty - q.filled;
    if remaining <= 0 {
        // Уже исполнены целиком — дальнейшие тики только двигают cum_traded (выше).
        return (q, FillDecision::NoFill);
    }

    // Пессимистично: доступно для заполнения — только та часть traded-объёма, что
    // ПРЕВЫСИЛА глубину впереди нас, за вычетом уже исполненного (излишек не
    // переносится вперёд оптимистично — SM-I-1/SM-I-6).
    let excess = (q.cum_traded - q.ahead).max(0);
    let fillable_now = excess - q.filled;
    if fillable_now <= 0 {
        return (q, FillDecision::NoFill);
    }

    let fill_qty = fillable_now.min(remaining);
    q.filled += fill_qty;

    if fill_qty == remaining {
        (q, FillDecision::Full { qty: fill_qty })
    } else {
        (q, FillDecision::Partial { qty: fill_qty })
    }
}

/// SM-I-5: отмена ордера впереди нас НЕ освобождает место — функция обязана быть
/// тождеством по QueueState (существует, чтобы соблазн «улучшить» был виден в ревью
/// и пойман RED-тестом, а не размазан по коду).
pub fn on_cancel_ahead(q: QueueState, cancelled_qty: i64) -> QueueState {
    let _ = cancelled_qty;
    q
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
    // Buy проедает ask-сторону (уровни с price ≤ limit); sell — bid-сторону (price ≥ limit).
    let opposite = match side {
        Side::Buy => Side::Sell,
        Side::Sell => Side::Buy,
    };
    let mut fills = Vec::new();
    let mut remaining = qty;
    for (price, size) in visible_book.levels(opposite) {
        if remaining <= 0 {
            break;
        }
        let within_limit = match side {
            Side::Buy => price <= limit_price,
            Side::Sell => price >= limit_price,
        };
        if !within_limit {
            // Уровни идут от лучшего к худшему — дальше только хуже.
            break;
        }
        let take = remaining.min(size);
        if take > 0 {
            fills.push((price, take));
            remaining -= take;
        }
    }
    fills
}
