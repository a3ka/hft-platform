//! RED задачи 13 (sacred, architect-only) — **`CT-RFC-09` §2.3: тип поля `sub`.**
//!
//! Предмет: `R-086` N-4, решение — `milestones/M-65-ws-session.md` §11. Суть и основания
//! только там.
//!
//! ЧТО ПИННИТ. §2.3 задаёт форму дословно: `{"type":"error","v":1,"sub":"<id>|null",…}` —
//! то есть либо СТРОКА с id, либо JSON `null`. Реализация отдаёт `sub.unwrap_or("null")` —
//! строку из четырёх букв. Следствие не косметическое: подписка с `id == "null"` становится
//! НЕОТЛИЧИМА от session-level ошибки. Клиент, разбирающий кадр, не может решить, к какой
//! подписке относится отказ, — а §2.3 вводился ровно затем, чтобы он мог.
//!
//! ПОЧЕМУ ЭТОГО НЕ ЛОВИТ НИ ОДИН СУЩЕСТВУЮЩИЙ ОРАКУЛ (`R-086` N-4). Хелпер `sub_of` в
//! `red_ws_session.rs` читает поле через `.as_str()`: и JSON `null`, и строка `"null"` дают
//! ему `None`/`Some("null")` — разница стирается на входе в проверку. Оракул, читающий
//! значение тем же способом, каким его портит код, слеп по построению
//! (`CT-RFC-09` §5.1 — эталон обязан идти НЕЗАВИСИМЫМ путём).
//! Поэтому здесь проверяется ТИП узла (`Value::is_null` / `is_string`), а не его строковое
//! представление.
//!
//! RUNTIME-RED СЕЙЧАС: `wire_v1.rs` — `"sub":sub.unwrap_or("null")`.
//! `session_error_sub_is_json_null` обязан падать против текущего кода.
//!
//! ПАРНЫЙ VANTAGE: `sub_scoped_error_keeps_string_id` не даёт «починить» дефект, отдав
//! `null` ВСЕГДА — тогда потеряется адресация подписки, и станет хуже, чем было.

use gateway_serve::wire_v1::error_msg;

/// ЯДРО RED: session-level ошибка несёт JSON `null`, а не строку.
#[test]
fn session_error_sub_is_json_null() {
    let v = error_msg(None, "unknown_op", "no such op");
    let sub = &v["sub"];
    assert!(
        sub.is_null(),
        "session-level error обязана нести ТИП JSON null (CT-RFC-09 §2.3 «sub»:«<id>|null»), \
         иначе подписка с id=\"null\" неотличима от сессионной ошибки. Получено: {sub}"
    );
}

/// ПАРНЫЙ VANTAGE: адресная ошибка сохраняет строковый id — «починка» через всегда-`null`
/// валит именно этот тест.
#[test]
fn sub_scoped_error_keeps_string_id() {
    let v = error_msg(Some("w1"), "invalid_selector", "bad bands");
    assert_eq!(
        v["sub"].as_str(),
        Some("w1"),
        "ошибка, относящаяся к подписке, обязана нести её id строкой — иначе клиент не знает, \
         какой виджет сломался"
    );
}

/// ГРАНИЦА, ради которой §2.3 и различает типы: подписка, буквально названная `null`.
/// При строковой форме этот кадр неотличим от session-level ошибки.
#[test]
fn subscription_literally_named_null_is_distinguishable() {
    let session = error_msg(None, "unknown_op", "session-level");
    let scoped = error_msg(Some("null"), "invalid_selector", "подписка с id=null");
    assert!(
        session["sub"].is_null() && scoped["sub"].is_string(),
        "кадры обязаны различаться ПО ТИПУ узла: session={} scoped={}. Совпадение типов и есть \
         дефект N-4 — клиент не может их развести",
        session["sub"],
        scoped["sub"]
    );
}
