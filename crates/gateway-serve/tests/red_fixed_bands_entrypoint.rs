//! RED `M-84` (sacred, architect-only) — **НАСТОЯЩИЙ ВХОД ПРОВОДА, а не внутренняя функция.**
//!
//! RUNTIME-RED (`A-033`): файл СОБИРАЕТСЯ и падает ПОВЕДЕНИЕМ, не сборкой.
//!
//! ## ЗАЧЕМ ОТДЕЛЬНЫЙ ФАЙЛ, ЕСЛИ ЕСТЬ `red_fixed_bands_wire.rs`
//!
//! Прежний набор звал `session::validate_selector` НАПРЯМУЮ и объявлял это достаточным.
//! Критик собрал жульническую реализацию: она честно отвергает чужой набор в прямом
//! валидаторе и МОЛЧА подменяет `sel.bands` каноническим в `wire_v1::parse_selector` —
//! то есть ДО валидатора. **Все шестнадцать RED-тестов остались зелёными.**
//!
//! Это ровно `Р-1` (`oracle-blindness-class-2026-08-28.md` §5): мера снимается на границе
//! ПОТРЕБИТЕЛЯ, а не с внутреннего участника. Потребитель здесь — байты от клиента, а
//! `session::validate_selector` есть внутренний участник, стоящий ПОСЛЕ записываемого шва.
//!
//! ## ГРАНИЦА, НА КОТОРОЙ СНИМАЕТСЯ МЕРА
//!
//! Вход — БАЙТЫ кадра `subscribe`. Путь — `parse_message` → `parse_selector` → валидатор.
//! Это тот же путь, которым идёт прод (`crates/gateway-serve/src/lib.rs:742` разбор, `:808`
//! валидация). Оракул, зовущий валидатор напрямую, шов между ними НЕ ПРОВЕРЯЕТ — а именно
//! в этот шов и садится подмена.
//!
//! ## `testing.md` чек-лист
//! - п.3 **отсутствие** — кадр БЕЗ поля `bands` обязан проходить (новая норма);
//! - п.4 **границы** — пустой массив, чужой набор, канонический набор от клиента;
//! - п.7 **ПАРНЫЙ vantage** — «чужое отвергнуто» И «доставленное не подменено».
//!   Второе — то, чего не хватало: без него подмена неотличима от отказа.

use gateway_serve::session::validate_selector;
use gateway_serve::wire_v1::{parse_message, parse_selector, ClientMessage};
use serde_json::{json, Value};

/// Продуктовый набор — ЛИТЕРАЛОМ, а не ссылкой на `gateway::CANONICAL_DEPTH_BANDS`.
/// `A-033`: тест, не собирающийся из-за отсутствующего символа, глушит ВЕСЬ корпус крейта —
/// именно так три круга не увидели 209 красных из 329. Исполняемый RED строго сильнее
/// (прецедент: `red_grace_from_env.rs:12-15`, `red_heatmap_window_env.rs:19-30`).
/// Принадлежность константы крейту держит шаг гейта, а не ссылка отсюда.
const PRODUCT_BANDS: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

/// Кадр `subscribe` прод-формы (`docs/rfc/CT-RFC-09-ws-session.md` §2.7).
/// `bands` подставляется ровно так, как его прислал бы клиент.
fn subscribe_frame(bands: Option<Value>) -> Vec<u8> {
    let mut sel = json!({
        "venue": "Binance",
        "symbol": "BTCUSDT",
        "timeframe_ms": 1000,
        "window_ms": 60000
    });
    if let Some(b) = bands {
        sel["bands"] = b;
    }
    serde_json::to_vec(&json!({"op":"subscribe","v":1,"id":"c-1","selector":sel})).unwrap()
}

/// Сквозной путь входа: байты → разбор кадра → разбор селектора → валидация.
/// Возвращает либо готовый селектор, либо текст отказа — как это увидит клиент.
fn through_entrypoint(bytes: &[u8]) -> Result<gateway::Selector, String> {
    let msg = parse_message(bytes).map_err(|e| format!("parse_message: {e:?}"))?;
    let sel_value = match msg {
        ClientMessage::Subscribe { selector, .. } => {
            selector.ok_or_else(|| "selector absent".to_string())?
        }
        ClientMessage::Unsubscribe { .. } => return Err("not a subscribe".to_string()),
    };
    let sel = parse_selector(&sel_value).map_err(|e| format!("{}: {e:?}", e.code()))?;
    validate_selector(&sel)?;
    Ok(sel)
}

#[test]
fn foreign_bands_rejected_at_the_real_entrypoint() {
    // ГЛАВНЫЙ ФОРСИНГ. Жульническая реализация подменяет набор В РАЗБОРЕ, до валидатора.
    // Прямой вызов валидатора её не видит; этот путь — видит.
    let err = through_entrypoint(&subscribe_frame(Some(json!([0.001, 0.002])))).expect_err(
        "чужой набор, пришедший НАСТОЯЩИМ кадром, обязан быть отвергнут. Если здесь успех — \
         значит набор подменён где-то на пути разбора, и клиент об этом не узнает (C-220 B-1)",
    );
    assert!(
        err.contains("bands") || err.contains("полос"),
        "отказ обязан называть причину. Получено: {err}"
    );
}

#[test]
fn foreign_bands_are_not_silently_canonicalized() {
    // ПАРНЫЙ vantage к предыдущему: мало отвергнуть — надо, чтобы НЕ ПОДМЕНИЛИ. Если
    // реализация вернёт Ok с каноническим набором на чужой вход, тест ловит именно это.
    match through_entrypoint(&subscribe_frame(Some(json!([0.001, 0.002])))) {
        Err(_) => {}
        Ok(sel) => panic!(
            "вход с чужим набором ПРИНЯТ, и на выходе полосы {:?}. Если они канонические — \
             это МОЛЧАЛИВАЯ ПОДМЕНА, прямо запрещённая спекой M-84: клиент просил одно, \
             получил другое и не знает (PL-I-7)",
            sel.bands
        ),
    }
}

#[test]
fn absent_bands_yield_canonical_selector_at_the_entrypoint() {
    // НОВАЯ НОРМА и позитивный контроль: без него реализация, отвергающая всё, зелена.
    let sel = through_entrypoint(&subscribe_frame(None))
        .expect("кадр БЕЗ поля bands — новая норма по П-029: сетку задаёт сервер");
    assert_eq!(
        sel.bands,
        PRODUCT_BANDS.to_vec(),
        "селектор, собранный из кадра без полос, обязан нести КАНОНИЧЕСКИЙ набор — иначе \
         сервер не задал сетку, а просто оставил её пустой"
    );
}

#[test]
fn empty_band_array_is_rejected_not_treated_as_absent() {
    // Граница: пустой массив — это НЕ «поле отсутствует». Трактовать его как отсутствие
    // значило бы принимать вход, которого контракт не предусматривает.
    through_entrypoint(&subscribe_frame(Some(json!([])))).expect_err(
        "пустой массив bands обязан быть отвергнут отдельно: он отличается от ОТСУТСТВИЯ поля",
    );
}

#[test]
fn even_canonical_bands_from_client_rejected_at_entrypoint() {
    // Принять — значит оставить путь «клиент знает сетку». Следующая смена набора сломает
    // такого клиента молча.
    through_entrypoint(&subscribe_frame(Some(json!(PRODUCT_BANDS))))
        .expect_err("даже канонический набор от клиента обязан отвергаться: сетку задаёт сервер");
}

#[test]
fn two_clients_converge_to_one_selector() {
    // ЯДРО решения `П-029`, и наблюдаемо оно ТОЛЬКО здесь — на пути клиента. В крейте
    // `gateway` тот же тест сравнивал бы два селектора, которые сам же и построил
    // одинаковыми (тавтология, пойманная `C-220` B-1).
    //
    // Сегодня: Аня просит [0.15], Боря — [0.03, 0.15]; оба кадра разбираются, дают РАЗНЫЕ
    // селекторы, разные отпечатки, разные расчёты и разные слепки одного и того же стакана.
    // После M-84 оба кадра обязаны быть отвергнуты, а кадр БЕЗ поля — дать один и тот же
    // серверный селектор обоим.
    let anya = through_entrypoint(&subscribe_frame(Some(json!([0.15]))));
    let boris = through_entrypoint(&subscribe_frame(Some(json!([0.03, 0.15]))));
    assert!(
        anya.is_err() && boris.is_err(),
        "клиентские сетки обязаны отвергаться ОБЕ: пока хоть одна принимается, ось отпечатка \
         жива и сервер считает один стакан дважды. Аня: {anya:?}, Боря: {boris:?}"
    );

    let a = through_entrypoint(&subscribe_frame(None)).expect("кадр без полос — новая норма");
    let b = through_entrypoint(&subscribe_frame(None)).expect("кадр без полос — новая норма");
    assert_eq!(
        a.bands, b.bands,
        "два клиента на одном инструменте обязаны получить ОДИН серверный набор — это и есть \
         общий ключ и общий чекпоинт, ради которых принято П-029"
    );
    assert_eq!(
        a.bands,
        PRODUCT_BANDS.to_vec(),
        "и этот набор обязан быть продуктовым: {:?}",
        a.bands
    );
}
