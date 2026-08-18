//! RED M-41 task #1 (sacred, architect-only): валидация ЗНАЧЕНИЙ — ядро VN-I-7
//! («malformed дропается + логируется; НИКОГДА не фабрикуется правдоподобное значение»).
//! Тот же принцип, что RK-I-3: неизвестный/битый вход → reject, не догадка.
//!
//! RED-дефекты D2/D3 (task #5) против текущего кода:
//!  - `"NaN".parse::<f64>()` в Rust УСПЕШЕН → `to_fixed(NaN)` = `(NaN).round() as i64` = 0
//!    (saturating cast) → в журнал уходит трейд с ценой 0 — буквально «событие с нулями».
//!  - `"inf"` тоже парсится → `to_fixed(inf)` = i64::MAX → цена 92233720368.54775807.
//!  - Отрицательные/нулевые px/sz и отрицательный time принимаются как есть.
//!
//! Ожидание (спека): px и sz — конечные и СТРОГО > 0; time > 0. Нарушение → дроп
//! (для trades — элемента, для l2Book — всего сообщения; см. red_parse_l2book.rs).
//!
//! Анти-плацебо: оракулы падают на текущем коде (проверено фактическим прогоном) —
//! фикстуры давят ровно на пути фабрикации.

use contracts::{EventKind, MdPayload};
use venue_hyperliquid::parse_message;

/// Ни при каком входе из этого файла не должно родиться событие с нулевым/отрицательным/
/// сатурированным price или size — общий инвариант «нет фабрикации».
fn assert_no_fabricated_values(events: &[EventKind], input: &str) {
    for e in events {
        let EventKind::Md(md) = e else { continue };
        let (p, s) = match &md.payload {
            MdPayload::Trade { price, size, .. } => (*price, *size),
            MdPayload::L2Snapshot { bids, asks, .. } => {
                for l in bids.iter().chain(asks) {
                    assert!(
                        l.price > 0 && l.price < i64::MAX && l.size > 0 && l.size < i64::MAX,
                        "фабрикованный уровень {l:?} из входа: {input}"
                    );
                }
                continue;
            }
            _ => continue,
        };
        assert!(
            p > 0 && p < i64::MAX && s > 0 && s < i64::MAX,
            "фабрикованные price={p}/size={s} из входа: {input}"
        );
    }
}

fn trade_msg(px: &str, sz: &str, time: i64) -> String {
    format!(
        r#"{{"channel":"trades","data":[{{"coin":"BTC","side":"B","px":"{px}","sz":"{sz}","time":{time}}}]}}"#
    )
}

/// D2 / RED: строка "NaN" в px — Rust парсит её в f64::NAN, каст даёт цену 0.
#[test]
fn nan_price_dropped_not_zero() {
    let msg = trade_msg("NaN", "0.001", 1_753_000_000_100);
    let events = parse_message(&msg);
    assert_no_fabricated_values(&events, &msg);
    assert!(
        events.is_empty(),
        "px=NaN обязан дропаться, получено: {events:?}"
    );
}

/// D2 / RED: "inf"/"-inf" в sz — сатурация в i64::MAX / фабрикация.
#[test]
fn infinite_size_dropped_not_saturated() {
    for sz in ["inf", "infinity", "-inf"] {
        let msg = trade_msg("118250.0", sz, 1_753_000_000_100);
        let events = parse_message(&msg);
        assert_no_fabricated_values(&events, &msg);
        assert!(
            events.is_empty(),
            "sz={sz} обязан дропаться, получено: {events:?}"
        );
    }
}

/// D3 / RED: отрицательная цена — не существует на бирже; принять = записать мусор навсегда.
#[test]
fn negative_price_dropped() {
    let msg = trade_msg("-118250.0", "0.001", 1_753_000_000_100);
    let events = parse_message(&msg);
    assert_no_fabricated_values(&events, &msg);
    assert!(events.is_empty(), "отрицательный px обязан дропаться");
}

/// D3 / RED: нулевые и отрицательные размеры трейда.
#[test]
fn zero_or_negative_size_dropped() {
    for sz in ["0", "0.0", "-0.5"] {
        let msg = trade_msg("118250.0", sz, 1_753_000_000_100);
        let events = parse_message(&msg);
        assert_no_fabricated_values(&events, &msg);
        assert!(
            events.is_empty(),
            "sz={sz}: трейд нулевого/отрицательного размера — дроп"
        );
    }
}

/// D3 / RED: нулевая цена трейда.
#[test]
fn zero_price_dropped() {
    let msg = trade_msg("0", "0.001", 1_753_000_000_100);
    let events = parse_message(&msg);
    assert_no_fabricated_values(&events, &msg);
    assert!(events.is_empty(), "px=0 — не цена; дроп");
}

/// D3 / RED: неположительный time (0 или отрицательный) — не epoch-ms; отравляет
/// возрастной фильтр ретеншена и порядок реплея.
#[test]
fn non_positive_time_dropped() {
    for t in [0i64, -1_753_000_000_100] {
        let msg = trade_msg("118250.0", "0.001", t);
        let events = parse_message(&msg);
        assert!(events.is_empty(), "time={t} — не валидный epoch-ms; дроп");
    }
}

/// Множественность + отрава: NaN-элемент дропается, валидный сосед в том же сообщении живёт
/// (гранулярность fail-closed для trades — элемент).
#[test]
fn poisoned_item_dropped_valid_sibling_kept() {
    let msg = r#"{"channel":"trades","data":[
        {"coin":"BTC","side":"B","px":"NaN","sz":"0.001","time":1753000000100},
        {"coin":"BTC","side":"A","px":"118250.0","sz":"0.002","time":1753000000101}]}"#;
    let events = parse_message(msg);
    assert_no_fabricated_values(&events, msg);
    assert_eq!(events.len(), 1, "валидный трейд выжил, отравленный дропнут");
}

/// D2 / RED (l2Book): NaN в размере уровня → дроп ВСЕГО снапшота (целостность под
/// сомнением), не «уровень с size=0».
#[test]
fn nan_level_size_drops_whole_snapshot() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[
        [{"px":"118250.0","sz":"NaN","n":1}],
        [{"px":"118250.5","sz":"0.5","n":1}]]}}"#;
    let events = parse_message(msg);
    assert_no_fabricated_values(&events, msg);
    assert!(
        events.is_empty(),
        "NaN в уровне = битый снапшот → дроп целиком"
    );
}

/// D3 / RED (l2Book): отрицательный размер уровня → дроп всего снапшота.
#[test]
fn negative_level_size_drops_whole_snapshot() {
    let msg = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[
        [{"px":"118250.0","sz":"-1.5","n":1}],
        [{"px":"118250.5","sz":"0.5","n":1}]]}}"#;
    let events = parse_message(msg);
    assert_no_fabricated_values(&events, msg);
    assert!(
        events.is_empty(),
        "отрицательный sz уровня = битый снапшот → дроп целиком"
    );
}
