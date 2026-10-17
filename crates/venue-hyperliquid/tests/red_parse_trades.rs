//! RED M-41 task #1 (sacred, architect-only): нормализация канала `trades` Hyperliquid →
//! `MdPayload::Trade` (VN-I-5: наружу только contracts-типы; VN-I-7: fail-closed).
//!
//! Compile-RED: `venue_hyperliquid::parse_message` сейчас ПРИВАТНА → суита не компилируется.
//! venue-dev (task #2) экспортирует `pub fn parse_message(text: &str) -> Vec<EventKind>`
//! (паттерн M-18 `venue_binance::l2delta_event`) БЕЗ изменения семантики.
//!
//! ГЛАВНЫЙ RED (дефект D0, task #3): текущий код маппит `"A" => Side::Buy, "B" => Side::Sell`
//! — это ИНВЕРСИЯ. Первоисточник — официальная нотация Hyperliquid
//! (hyperliquid.gitbook.io → For developers → API → Notation, сверено 2026-07-29):
//!   «Side = side of trade or book. B = Bid = Buy, A = Ask = Short.
//!    Side is aggressing side for trades.»
//! Т.е. "B" — агрессивная ПОКУПКА, "A" — агрессивная ПРОДАЖА. Оракулы ниже падают на
//! текущем коде — это подлинный RED, не характеризация.
//!
//! Чек-лист testing.md: множественность (2 трейда в одном сообщении), отсутствие (битый
//! элемент не убивает соседний и не фабрикуется), границы (точность fixed-point),
//! прод-режим значений (реальная форма сообщения с hash/tid/users, epoch-ms 2026 года).

use contracts::{EventKind, MdPayload, Side, Venue};
use venue_hyperliquid::parse_message;

/// Прод-форма элемента trades: лишние поля `hash`/`tid`/`users` ШТАТНЫ (аддитивные поля
/// биржи — не malformed; адаптер обязан их терпеть, а не отвергать).
const PROD_TRADE_B: &str = r#"{"channel":"trades","data":[{"coin":"BTC","side":"B","px":"112000.12345678","sz":"0.00842","time":1753000000123,"hash":"0x7b0d1e","tid":118250501234567,"users":["0xaa","0xbb"]}]}"#;

fn only_trade(events: &[EventKind]) -> (String, i64, i64, Side, i64) {
    assert_eq!(
        events.len(),
        1,
        "ожидалось ровно одно событие, получено: {events:?}"
    );
    let EventKind::Md(md) = &events[0] else {
        panic!("ожидался EventKind::Md, получено: {:?}", events[0]);
    };
    assert_eq!(md.venue, Venue::Hyperliquid);
    let MdPayload::Trade {
        price,
        size,
        side,
        ts_exch_ms,
    } = &md.payload
    else {
        panic!("ожидался MdPayload::Trade, получено: {:?}", md.payload);
    };
    (md.symbol.clone(), *price, *size, *side, *ts_exch_ms)
}

/// D0 / RED: side "B" = Bid = BUY (агрессор-покупатель), НЕ Sell.
#[test]
fn trade_side_b_is_buy_official_notation() {
    let (symbol, price, size, side, ts) = only_trade(&parse_message(PROD_TRADE_B));
    assert_eq!(symbol, "BTC", "нативный тикер HL как есть (не BTCUSDT)");
    assert_eq!(
        price, 11_200_012_345_678,
        "px \"112000.12345678\" → i64×1e8 без потери"
    );
    assert_eq!(size, 842_000, "sz \"0.00842\" → i64×1e8");
    assert_eq!(
        side,
        Side::Buy,
        "HL notation: \"B\" = Bid = Buy (агрессор). Текущий код инвертирует стороны — D0"
    );
    assert_eq!(
        ts, 1_753_000_000_123,
        "ts_exch_ms — время БИРЖИ из поля time"
    );
}

/// D0 / RED: side "A" = Ask = SELL (агрессор-продавец), НЕ Buy.
#[test]
fn trade_side_a_is_sell_official_notation() {
    let msg = r#"{"channel":"trades","data":[{"coin":"ETH","side":"A","px":"4310.194359","sz":"1.5","time":1753000000456}]}"#;
    let (symbol, price, size, side, ts) = only_trade(&parse_message(msg));
    assert_eq!(symbol, "ETH");
    assert_eq!(price, 431_019_435_900);
    assert_eq!(size, 150_000_000);
    assert_eq!(
        side,
        Side::Sell,
        "HL notation: \"A\" = Ask = Sell (агрессор)"
    );
    assert_eq!(ts, 1_753_000_000_456);
}

/// Множественность: 2 трейда в ОДНОМ сообщении → 2 события, порядок сохранён.
#[test]
fn two_trades_one_message_both_emitted_in_order() {
    let msg = r#"{"channel":"trades","data":[
        {"coin":"BTC","side":"B","px":"118250.5","sz":"0.001","time":1753000000100},
        {"coin":"BTC","side":"A","px":"118250.0","sz":"0.002","time":1753000000101}]}"#;
    let events = parse_message(msg);
    assert_eq!(events.len(), 2, "оба трейда сохранены (множественность)");
    let ts_of = |e: &EventKind| match e {
        EventKind::Md(md) => match &md.payload {
            MdPayload::Trade { ts_exch_ms, .. } => *ts_exch_ms,
            other => panic!("ожидался Trade, получено {other:?}"),
        },
        other => panic!("ожидался Md, получено {other:?}"),
    };
    assert_eq!(
        ts_of(&events[0]),
        1_753_000_000_100,
        "порядок из сообщения сохранён"
    );
    assert_eq!(ts_of(&events[1]), 1_753_000_000_101);
}

/// Отсутствие поля в ОДНОМ элементе: битый элемент дропается, валидный сосед живёт.
/// Гранулярность fail-closed для trades — ЭЛЕМЕНТ (трейды независимы), не всё сообщение.
#[test]
fn bad_item_dropped_good_item_kept() {
    let msg = r#"{"channel":"trades","data":[
        {"coin":"BTC","side":"B","sz":"0.001","time":1753000000100},
        {"coin":"BTC","side":"A","px":"118250.0","sz":"0.002","time":1753000000101}]}"#;
    let events = parse_message(msg);
    assert_eq!(events.len(), 1, "элемент без px дропнут, валидный сохранён");
    let EventKind::Md(md) = &events[0] else {
        panic!()
    };
    let MdPayload::Trade { price, .. } = &md.payload else {
        panic!()
    };
    assert_eq!(*price, 11_825_000_000_000, "выжил именно ВАЛИДНЫЙ элемент");
}

/// Неизвестная сторона НЕ дефолтится (ни в Buy, ни в Sell) — элемент дропается.
/// Тот же принцип, что RK-I-3: неизвестный вход → reject, не догадка.
#[test]
fn unknown_side_dropped_not_defaulted() {
    let msg = r#"{"channel":"trades","data":[{"coin":"BTC","side":"X","px":"118250.0","sz":"0.001","time":1753000000100}]}"#;
    assert!(
        parse_message(msg).is_empty(),
        "side \"X\" не мапится ни в одну сторону — событие не фабрикуется"
    );
}

/// Характеризация (текущий код зелёный; проверено мутацией — снятие фильтра валит тест):
/// инструменты с "MID" в имени отфильтровываются (синтетические mid-цены — не трейды).
/// ОТКРЫТЫЙ ВОПРОС в milestone §E: substring-матч зацепит и гипотетический листинг
/// вида "MIDAS" — сознательное допущение, зафиксировано.
#[test]
fn mid_instrument_filtered_out() {
    let msg = r#"{"channel":"trades","data":[{"coin":"MID","side":"B","px":"1.0","sz":"1.0","time":1753000000100}]}"#;
    assert!(
        parse_message(msg).is_empty(),
        "MID-инструмент не попадает в MdEvent-поток"
    );
}

/// Границы точности: min-tick 1e-8, малые размеры, много значащих цифр.
/// Ожидания посчитаны от ТОЧНОЙ десятичной арифметики (Decimal), не от f64.
#[test]
fn fixed_point_precision_boundaries() {
    let cases: &[(&str, &str, i64, i64)] = &[
        // (px, sz, ожидание px_e8, ожидание sz_e8)
        ("0.00000001", "0.00001", 1, 1_000), // min-tick / малый размер
        ("118250.5", "0.00842", 11_825_050_000_000, 842_000), // прод-режим BTC
        (
            "20999999.99999999",
            "1.0",
            2_099_999_999_999_999,
            100_000_000,
        ), // 16 значащих цифр
        ("0.00001234", "2500000000.5", 1_234, 250_000_000_050_000_000), // мем-коин: микроцена × огромный размер
    ];
    for (px, sz, want_px, want_sz) in cases {
        let msg = format!(
            r#"{{"channel":"trades","data":[{{"coin":"BTC","side":"B","px":"{px}","sz":"{sz}","time":1753000000100}}]}}"#
        );
        let (_, price, size, _, _) = only_trade(&parse_message(&msg));
        assert_eq!(price, *want_px, "px {px}: потеря точности при нормализации");
        assert_eq!(size, *want_sz, "sz {sz}: потеря точности при нормализации");
    }
}
