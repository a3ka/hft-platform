//! RED M-41 task #1 (sacred, architect-only): нормализация канала `l2Book` Hyperliquid →
//! `MdPayload::L2Snapshot` (VN-I-7 fail-closed; снапшот-семантика).
//!
//! КРИТИЧНО (recon §B, помечено в самом src): уровни HL — ОБЪЕКТЫ `{"px","sz","n"}`,
//! НЕ массивы `[px,sz]` (binance-стиль). `levels[0]` = bids, `levels[1]` = asks.
//!
//! Снапшот-семантика: l2Book — ПОЛНЫЙ снапшот книги на момент времени. Пустая сторона в
//! снапшоте = «на этой стороне уровней НЕТ» (правда о тонкой книге) — в отличие от
//! L2Delta, где пустая сторона = «не менялось». Адаптер НЕ додумывает уровни из прошлых
//! сообщений (stateless) и НЕ конвертирует снапшот в дифф.
//!
//! RED-дефект D1 (task #4): текущий код фабрикует `ts_exch_ms = 0` при отсутствии `time`
//! (`unwrap_or(0)`) — нарушение VN-I-7 («никогда не фабрикуется правдоподобное значение»)
//! и порча возрастного фильтра ретеншена. Ожидание: сообщение без time ДРОПАЕТСЯ.
//!
//! Гранулярность fail-closed для l2Book — ВСЁ СООБЩЕНИЕ: битый уровень/поле ставит под
//! сомнение целостность всего снапшота (в отличие от независимых трейдов).
//!
//! Чек-лист testing.md: асимметрия (3 бида / 1 аск; пустая сторона), множественность,
//! отсутствие (нет time; нет стороны), границы (пустые массивы), прод-режим значений.

use contracts::{EventKind, Level, MdPayload, Venue};
use venue_hyperliquid::parse_message;

/// Прод-форма l2Book: объектные уровни с полем n (число ордеров — адаптером не переносится,
/// но обязано ТЕРПЕТЬСЯ как штатное поле).
const PROD_L2BOOK: &str = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000123,"levels":[
    [{"px":"118250.0","sz":"1.5","n":3},{"px":"118249.5","sz":"0.75","n":1},{"px":"118249.0","sz":"2.0","n":7}],
    [{"px":"118250.5","sz":"0.5","n":2}]]}}"#;

fn only_snapshot(events: &[EventKind]) -> (String, Vec<Level>, Vec<Level>, i64) {
    assert_eq!(
        events.len(),
        1,
        "ожидалось ровно одно событие, получено: {events:?}"
    );
    let EventKind::Md(md) = &events[0] else {
        panic!("ожидался EventKind::Md, получено: {:?}", events[0]);
    };
    assert_eq!(md.venue, Venue::Hyperliquid);
    let MdPayload::L2Snapshot {
        bids,
        asks,
        ts_exch_ms,
    } = &md.payload
    else {
        panic!(
            "l2Book обязан нормализоваться в L2Snapshot (не Delta), получено: {:?}",
            md.payload
        );
    };
    (md.symbol.clone(), bids.clone(), asks.clone(), *ts_exch_ms)
}

/// Прод-форма + асимметрия (3 бида / 1 аск): объектные уровни разобраны точно,
/// стороны не перепутаны, глубина не обрезана и не допридумана.
#[test]
fn l2book_prod_shape_object_levels_asymmetric_depth() {
    let (symbol, bids, asks, ts) = only_snapshot(&parse_message(PROD_L2BOOK));
    assert_eq!(symbol, "BTC");
    assert_eq!(
        ts, 1_753_000_000_123,
        "ts_exch_ms — время биржи из data.time"
    );
    assert_eq!(
        bids.len(),
        3,
        "все 3 бид-уровня сохранены (асимметрия не выравнивается)"
    );
    assert_eq!(asks.len(), 1);
    assert_eq!(
        (bids[0].price, bids[0].size),
        (11_825_000_000_000, 150_000_000),
        "levels[0][0] = лучший bid"
    );
    assert_eq!(
        (bids[1].price, bids[1].size),
        (11_824_950_000_000, 75_000_000)
    );
    assert_eq!(
        (bids[2].price, bids[2].size),
        (11_824_900_000_000, 200_000_000)
    );
    assert_eq!(
        (asks[0].price, asks[0].size),
        (11_825_050_000_000, 50_000_000),
        "levels[1][0] = лучший ask (стороны НЕ перепутаны)"
    );
    assert!(bids[0].price < asks[0].price, "sanity: bid < ask");
}

/// Пустая сторона снапшота — ПРАВДА о тонкой книге, не ошибка и не «не менялось».
#[test]
fn empty_bid_side_is_truth_not_error() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"NEWCOIN","time":1753000000200,"levels":[[],[{"px":"0.5","sz":"100.0","n":1}]]}}"#;
    let (_, bids, asks, _) = only_snapshot(&parse_message(msg));
    assert!(bids.is_empty(), "пустая сторона остаётся пустой");
    assert_eq!(asks.len(), 1);
}

/// Снапшот-семантика + statelessness: каждое сообщение — независимая полная правда.
/// Уровень, ПРОПАВШИЙ из следующего снапшота, отсутствует и в событии — адаптер не
/// «дозаполняет» из предыдущего сообщения (характеризация: parse_message stateless;
/// канарейка против будущего появления скрытого состояния в парсере).
#[test]
fn each_snapshot_independent_no_carryover() {
    let full = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[
        [{"px":"118250.0","sz":"1.5","n":1},{"px":"118249.0","sz":"1.0","n":1}],
        [{"px":"118251.0","sz":"1.0","n":1}]]}}"#;
    let thinner = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000200,"levels":[
        [{"px":"118250.0","sz":"1.5","n":1}],
        [{"px":"118251.0","sz":"1.0","n":1}]]}}"#;
    let _ = parse_message(full);
    let (_, bids2, _, _) = only_snapshot(&parse_message(thinner));
    assert_eq!(
        bids2.len(),
        1,
        "уровень 118249.0 отсутствует во втором снапшоте → отсутствует в событии (без carry-over)"
    );
    assert_eq!(bids2[0].price, 11_825_000_000_000);
}

/// D1 / RED: сообщение БЕЗ поля time дропается целиком — НЕ эмитится с ts_exch_ms=0.
/// Текущий код: `unwrap_or(0)` — фабрикация значения (VN-I-7) + отравление возрастного
/// фильтра ретеншена (событие с ts=0 «старше всех» навсегда).
#[test]
fn missing_time_drops_message_not_fabricates_zero() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"BTC","levels":[
        [{"px":"118250.0","sz":"1.5","n":1}],
        [{"px":"118250.5","sz":"0.5","n":1}]]}}"#;
    let events = parse_message(msg);
    assert!(
        events.is_empty(),
        "l2Book без time обязан дропаться (VN-I-7: не фабрикуй ts=0), получено: {events:?}"
    );
}

/// Отсутствие целой стороны (levels из одного массива) → дроп всего сообщения.
#[test]
fn missing_ask_side_drops_whole_message() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[
        [{"px":"118250.0","sz":"1.5","n":1}]]}}"#;
    assert!(
        parse_message(msg).is_empty(),
        "снапшот без одной из сторон неполон → дроп"
    );
}

/// Битый уровень (нет px) → дроп ВСЕГО сообщения: целостность снапшота под сомнением,
/// частичный снапшот в журнале хуже отсутствующего (реплей воспроизведёт дыру бит-в-бит).
#[test]
fn level_missing_px_drops_whole_message() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[
        [{"px":"118250.0","sz":"1.5","n":1},{"sz":"0.9","n":1}],
        [{"px":"118250.5","sz":"0.5","n":1}]]}}"#;
    assert!(
        parse_message(msg).is_empty(),
        "уровень без px = битый снапшот → дроп целиком, не «пропустить уровень»"
    );
}

/// Массивный (binance-стиль) формат уровней `[px,sz]` вместо объектов `{px,sz,n}` —
/// чужой wire-формат → дроп (КРИТИЧНО-помеченное место кода; регресс на смену формата).
#[test]
fn array_style_levels_rejected() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[
        [["118250.0","1.5"]],
        [["118250.5","0.5"]]]}}"#;
    assert!(
        parse_message(msg).is_empty(),
        "уровни-массивы — не формат HL → дроп"
    );
}

/// Уровень без поля `n` терпится: `n` адаптером не используется, его отсутствие не делает
/// px/sz недостоверными (лениентность к неиспользуемым полям ≠ фабрикация).
#[test]
fn level_without_n_field_tolerated() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[
        [{"px":"118250.0","sz":"1.5"}],
        [{"px":"118250.5","sz":"0.5"}]]}}"#;
    let (_, bids, asks, _) = only_snapshot(&parse_message(msg));
    assert_eq!(bids.len(), 1);
    assert_eq!(asks.len(), 1);
}

/// MID-фильтр действует и на l2Book (характеризация, паритетно trades).
#[test]
fn mid_instrument_filtered_out_l2book() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"MID","time":1753000000100,"levels":[
        [{"px":"1.0","sz":"1.0","n":1}],
        [{"px":"1.1","sz":"1.0","n":1}]]}}"#;
    assert!(parse_message(msg).is_empty());
}
