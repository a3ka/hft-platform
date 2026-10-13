//! RED M-41 task #1 (sacred, architect-only): битый КОНВЕРТ сообщения (VN-I-7) —
//! обрезанный JSON, отсутствующие/нетипизированные поля, чужие каналы. Ожидание везде:
//! ПУСТОЙ результат, ноль фабрикации. Характеризация текущего кода (он уже дропает эти
//! формы) — но каждый оракул проверен мутацией: фабрикация события в malformed-ветке
//! валит суиту (анти-плацебо фактически, не рассуждением).

use contracts::EventKind;
use venue_hyperliquid::parse_message;

fn assert_dropped(input: &str, why: &str) {
    let events = parse_message(input);
    assert!(
        events.is_empty(),
        "{why}; вход: {input}; получено: {events:?}"
    );
}

/// Обрезанный посреди передачи JSON (реальный сценарий рвущегося WS).
#[test]
fn truncated_json_dropped() {
    assert_dropped(
        r#"{"channel":"trades","data":[{"coin":"BT"#,
        "обрезанный JSON обязан дропаться без паники",
    );
    assert_dropped("", "пустая строка");
    assert_dropped("не json вовсе", "мусор вместо JSON");
}

/// Конверт без поля channel / с channel неверного типа.
#[test]
fn missing_or_mistyped_channel_dropped() {
    assert_dropped(r#"{"data":[{"coin":"BTC"}]}"#, "нет channel");
    assert_dropped(r#"{"channel":123,"data":[]}"#, "channel — число, не строка");
    assert_dropped(r#"{"channel":null,"data":[]}"#, "channel null");
}

/// Неизвестный канал — не наш формат: дроп, не попытка «угадать» смысл.
#[test]
fn unknown_channel_dropped() {
    assert_dropped(
        r#"{"channel":"orderUpdates","data":[{"coin":"BTC","px":"1.0"}]}"#,
        "чужой канал (даже с похожими полями) дропается",
    );
}

/// Служебные каналы (pong, subscriptionResponse) — штатно игнорируются (не malformed,
/// но и не события: ноль эмиссии).
#[test]
fn control_channels_ignored() {
    assert_dropped(r#"{"channel":"pong"}"#, "pong — служебный");
    assert_dropped(
        r#"{"channel":"subscriptionResponse","data":{"method":"subscribe","subscription":{"type":"trades","coin":"BTC"}}}"#,
        "subscriptionResponse — служебный",
    );
}

/// trades: data отсутствует / не массив / null.
#[test]
fn trades_data_wrong_shape_dropped() {
    assert_dropped(r#"{"channel":"trades"}"#, "trades без data");
    assert_dropped(r#"{"channel":"trades","data":null}"#, "trades data=null");
    assert_dropped(
        r#"{"channel":"trades","data":{"coin":"BTC","side":"B","px":"1.0","sz":"1.0","time":1753000000100}}"#,
        "trades data-объект вместо массива",
    );
}

/// l2Book: data отсутствует / null / levels не массив.
#[test]
fn l2book_data_wrong_shape_dropped() {
    assert_dropped(r#"{"channel":"l2Book"}"#, "l2Book без data");
    assert_dropped(r#"{"channel":"l2Book","data":null}"#, "l2Book data=null");
    assert_dropped(
        r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":"пусто"}}"#,
        "levels — строка, не массив",
    );
}

/// Число вместо строки в px/sz (HL шлёт СТРОКИ; число = чужой формат/смена wire-формата —
/// сигнал integrity, не повод молча съесть).
#[test]
fn numeric_px_sz_instead_of_string_dropped() {
    assert_dropped(
        r#"{"channel":"trades","data":[{"coin":"BTC","side":"B","px":118250.0,"sz":"0.001","time":1753000000100}]}"#,
        "px-число вместо строки",
    );
    assert_dropped(
        r#"{"channel":"l2Book","data":{"coin":"BTC","time":1753000000100,"levels":[[{"px":118250.0,"sz":1.5,"n":1}],[{"px":"118250.5","sz":"0.5","n":1}]]}}"#,
        "числовые px/sz в уровне",
    );
}

/// time строкой ("1753000000100") вместо числа → дроп (не парсим «на удачу»).
#[test]
fn string_time_dropped() {
    assert_dropped(
        r#"{"channel":"trades","data":[{"coin":"BTC","side":"B","px":"118250.0","sz":"0.001","time":"1753000000100"}]}"#,
        "time-строка вместо числа",
    );
}

/// coin неверного типа / отсутствует.
#[test]
fn missing_or_mistyped_coin_dropped() {
    assert_dropped(
        r#"{"channel":"trades","data":[{"side":"B","px":"118250.0","sz":"0.001","time":1753000000100}]}"#,
        "трейд без coin",
    );
    assert_dropped(
        r#"{"channel":"trades","data":[{"coin":42,"side":"B","px":"118250.0","sz":"0.001","time":1753000000100}]}"#,
        "coin-число",
    );
}

/// Пустой массив trades — ноль событий (граница «пусто»), без паники и без фабрикации.
#[test]
fn empty_trades_array_yields_nothing() {
    assert_dropped(r#"{"channel":"trades","data":[]}"#, "пустой массив трейдов");
}

/// Лишние ТОП-уровневые поля конверта терпятся (аддитивность биржи ≠ malformed):
/// валидное сообщение с неизвестным полем рядом с channel/data обязано разобраться.
#[test]
fn extra_envelope_fields_tolerated() {
    let msg = r#"{"channel":"trades","seq":42,"data":[{"coin":"BTC","side":"A","px":"118250.0","sz":"0.001","time":1753000000100}]}"#;
    let events = parse_message(msg);
    assert_eq!(
        events.len(),
        1,
        "неизвестное поле конверта не делает сообщение битым: {events:?}"
    );
    assert!(matches!(events[0], EventKind::Md(_)));
}
