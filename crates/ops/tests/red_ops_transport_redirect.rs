//! RED-оракул на находку **F-11 (BLOCKER)** PR-гейта R-009
//! (`research/reviews/R-009-alerting-rev3.md`): `TELEGRAM_BOT_TOKEN` уезжает на ПРОИЗВОЛЬНЫЙ
//! хост в заголовке `Referer` при HTTP-редиректе.
//!
//! На момент коммита этого файла защиты НЕ существует — файл обязан быть КРАСНЫМ (RED-first,
//! `.claude/rules/gates.md` §2). Реализация — engine-dev.
//!
//! # Механика утечки (воспроизведена reviewer'ом на живом зонде)
//!
//! `reqwest::blocking::Client` строится без указания redirect-политики, то есть с дефолтной
//! `Policy::limited(10)`. Токен по требованию Telegram Bot API вшит в ПУТЬ URL. Сервер A
//! (адрес из `TELEGRAM_API_BASE`) отвечает `307` с `Location` на чужой хост B — и `reqwest`
//! сам отправляет на B новый запрос, проставив `Referer` с ПОЛНЫМ исходным URL:
//!
//! ```text
//! REQUEST-LINE: POST /redirected HTTP/1.1
//! referer: http://127.0.0.1:42097/bot7891234567:AAG-REDIRECT-PROBE-SECRET/sendMessage
//! host: 127.0.0.1:33307
//! BODY: chat_id=-100777&text=%5BCRITICAL%5D+WD-HB-STALE+...
//! ```
//!
//! На `307` переносится метод и тело — чужой хост получает И секрет, И содержимое алерта.
//! **Вывод бинаря при этом чист**: локальная редакция (F-2/F-9) работает идеально, и именно
//! поэтому эта утечка невидима из логов. F-2 была признана блокером за утечку токена в
//! ЛОКАЛЬНЫЙ файл лога; здесь секрет уходит ЗА ПРЕДЕЛЫ МАШИНЫ, третьей стороне.
//!
//! # Контракт, который задаёт этот файл (спецификация для engine-dev)
//!
//! Это оракул на **КОНФИГУРАЦИЮ клиента (политику редиректов)**, а не на форматирование
//! строк: чинить формулировки сообщений здесь нечего — утечку совершает сам HTTP-клиент.
//!
//! - **INV-F11-1.** Ни один запрос, порождённый `TelegramTransport::send`, не приходит ни на
//!   какой хост, кроме сконфигурированного в `TELEGRAM_API_BASE`. Редирект — указание
//!   УДАЛЁННОЙ стороны, то есть недоверенный вход той же категории, что тело ответа (F-9):
//!   ему нельзя позволять переписывать адрес назначения секрета.
//! - **INV-F11-2.** Заголовок `Referer` не отправляется НИКОГДА — ни на чужой хост, ни на тот
//!   же самый. Решение «этот редирект ведёт на тот же хост, значит можно» принимается по
//!   данным от удалённой стороны (`Location` бывает относительным, схемо-относительным
//!   `//host/path`, цепочечным) — надёжной эта проверка не бывает, а Telegram Bot API штатно
//!   не редиректит вовсе. Значит редирект не следуется, и `Referer` неоткуда взяться.
//! - **INV-F11-3.** Заблокированный редирект — ЧЕСТНЫЙ провал доставки (`Err`), а не тихий
//!   успех: сообщение никуда не доставлено, и watchdog обязан об этом сказать. Текст ошибки
//!   при этом подчиняется F-2/F-9 (никакого секрета, никакого содержимого ответа).
//!
//! Ожидаемая форма реализации (не предписание, а ориентир): `reqwest::blocking::Client::builder()`
//! `.redirect(reqwest::redirect::Policy::none())`. При ней 3xx возвращается как обычный ответ,
//! попадает в ветку «не-2xx» и редактируется уже проверенным путём F-9.
//!
//! # Чек-лист деградированного входа (`.claude/rules/testing.md`)
//! - **Асимметрия**: деградирован ТОЛЬКО ответ (сервер A в остальном ведёт себя штатно);
//!   `307` переносит метод+тело, `302/303` — переписывают на GET и тело роняют, но `Referer`
//!   течёт в обоих случаях.
//! - **Множественность**: всё семейство 301/302/303/307/308; цепочка A→B→C; несколько алертов
//!   за один прогон бинаря.
//! - **Отсутствие**: `3xx` БЕЗ заголовка `Location` — не паника и не «успех».
//! - **Границы**: схемо-относительный `Location: //host:port/path` (выглядит относительным,
//!   уводит на ЧУЖОЙ хост) и обычный относительный `/path` на том же хосте.
//! - **Прод-масштаб**: сквозной прогон настоящего бинаря `ops-watchdog` — именно он уезжает
//!   на VPS, и именно у него `TELEGRAM_API_BASE` будет выставлен founder'ом руками.

use ops::transport::{TelegramTransport, Transport};

mod common;
use common::{header_value, raw_response, ProbeServer};

/// Реалистичная форма токена Telegram (`<bot_id>:<base64url>`) с маркером внутри.
const TOKEN: &str = "7891234567:AAG-REDIRECT-ORACLE-SECRET-DO-NOT-SEND";
const TOKEN_SECRET_PART: &str = "AAG-REDIRECT-ORACLE-SECRET-DO-NOT-SEND";
const CHAT_ID: &str = "-1002233445566";
const ALERT_TEXT: &str = "[CRITICAL] WD-HB-STALE — оракул F-11";

fn transport_to(api_base: &str) -> TelegramTransport {
    TelegramTransport::with_credentials_and_endpoint(
        Some((TOKEN.to_string(), CHAT_ID.to_string())),
        api_base,
    )
}

/// Ответ «сервера-приёмника», который в норме не должен получить НИЧЕГО.
fn ok_response(_req: &str) -> String {
    raw_response("200 OK", &[], "{\"ok\":true}")
}

/// Сервер, до которого запрос дойти НЕ ИМЕЕТ ПРАВА (INV-F11-1). Проверяется и факт
/// обращения, и — на случай, если оно всё же было, — содержимое: так в отчёте видно, ЧТО
/// именно уехало.
fn assert_untouched(name: &str, server: &ProbeServer) {
    server.settle();
    let requests = server.requests();
    for raw in &requests {
        assert!(
            !raw.contains(TOKEN) && !raw.contains(TOKEN_SECRET_PART),
            "{name} получил TELEGRAM_BOT_TOKEN (R-009 F-11):\n{raw}"
        );
        assert!(
            !raw.contains(CHAT_ID),
            "{name} получил TELEGRAM_CHAT_ID (R-009 F-11):\n{raw}"
        );
    }
    assert_eq!(
        requests.len(),
        0,
        "{name} — ЧУЖОЙ хост, а запрос до него доехал ({} шт.); секрет и/или содержимое алерта \
         покинули машину (R-009 F-11):\n{}",
        requests.len(),
        requests.join("\n---\n")
    );
}

/// INV-F11-2 на стороне отправителя: ни один исходящий запрос не несёт `Referer`.
fn assert_no_referer(name: &str, server: &ProbeServer) {
    for raw in server.requests() {
        if let Some(referer) = header_value(&raw, "referer") {
            panic!("{name} получил запрос с заголовком Referer: {referer}\n(R-009 F-11: Referer с исходным URL несёт TELEGRAM_BOT_TOKEN — заголовок не должен отправляться вовсе)");
        }
    }
}

fn assert_error_carries_no_secret(where_: &str, text: &str) {
    assert!(
        !text.contains(TOKEN) && !text.contains(TOKEN_SECRET_PART),
        "{where_} содержит TELEGRAM_BOT_TOKEN: {text}"
    );
    assert!(
        !text.contains(CHAT_ID),
        "{where_} содержит TELEGRAM_CHAT_ID: {text}"
    );
}

/// ГЛАВНЫЙ оракул F-11: `307` на чужой хост. Ровно проба reviewer'а из R-009.
#[test]
fn f11_cross_host_redirect_never_delivers_the_token_to_the_other_host() {
    let host_b = ProbeServer::start(ok_response);
    let b_url = format!("{}/redirected", host_b.url());
    let host_a = ProbeServer::start(move |_| {
        raw_response("307 Temporary Redirect", &[("Location", &b_url)], "")
    });

    let result = transport_to(&host_a.url()).send(ALERT_TEXT);

    // (1) Анти-плацебо: путь пройден — запрос дошёл до СВОЕГО хоста и тот ответил редиректом.
    assert_eq!(
        host_a.hits(),
        1,
        "запрос не дошёл до сконфигурированного эндпоинта — оракул проверяет не тот путь"
    );
    // (2) INV-F11-1: чужой хост не получил ничего.
    assert_untouched("хост B (цель редиректа)", &host_b);
    // (3) INV-F11-2: Referer не отправляется вообще.
    assert_no_referer("хост A", &host_a);
    // (4) INV-F11-3: недоставленное сообщение не выдаётся за доставленное, и ошибка чиста.
    let err = result.expect_err(
        "редирект заблокирован, значит сообщение НЕ доставлено — это провал доставки, а не успех",
    );
    assert_error_carries_no_secret("TransportError::Display (редирект)", &err.to_string());
    assert_error_carries_no_secret("TransportError::Debug (редирект)", &format!("{err:?}"));
}

/// Множественность (`testing.md` п.2): всё семейство редирект-статусов. `302/303` роняют тело
/// и переписывают метод на GET, но `Referer` с токеном уезжает точно так же.
#[test]
fn f11_every_redirect_status_is_blocked_not_just_307() {
    for status in [
        "301 Moved Permanently",
        "302 Found",
        "303 See Other",
        "308 Permanent Redirect",
    ] {
        let host_b = ProbeServer::start(ok_response);
        let b_url = format!("{}/redirected", host_b.url());
        let status_owned = status.to_string();
        let host_a =
            ProbeServer::start(move |_| raw_response(&status_owned, &[("Location", &b_url)], ""));

        let _ = transport_to(&host_a.url()).send(ALERT_TEXT);

        assert_eq!(host_a.hits(), 1, "{status}: запрос не дошёл до эндпоинта");
        assert_untouched(&format!("хост B при статусе {status}"), &host_b);
        assert_no_referer(&format!("хост A при статусе {status}"), &host_a);
    }
}

/// Множественность №2: цепочка редиректов A → B → C. Обрыв обязан произойти на первом шаге —
/// «мы дошли до второго хоста, но не до третьего» утечкой быть не перестаёт.
#[test]
fn f11_redirect_chain_stops_at_the_configured_host() {
    let host_c = ProbeServer::start(ok_response);
    let c_url = format!("{}/final", host_c.url());
    let host_b = ProbeServer::start(move |_| {
        raw_response("307 Temporary Redirect", &[("Location", &c_url)], "")
    });
    let b_url = format!("{}/hop", host_b.url());
    let host_a = ProbeServer::start(move |_| {
        raw_response("307 Temporary Redirect", &[("Location", &b_url)], "")
    });

    let _ = transport_to(&host_a.url()).send(ALERT_TEXT);

    assert_eq!(host_a.hits(), 1, "запрос не дошёл до эндпоинта");
    assert_untouched("хост B (первый переход цепочки)", &host_b);
    assert_untouched("хост C (второй переход цепочки)", &host_c);
    assert_no_referer("хост A", &host_a);
}

/// Границы (`testing.md` п.4): схемо-относительный `Location: //host:port/path`. Выглядит
/// как относительный путь, а уводит на ЧУЖОЙ хост — ровно та проверка «это же наш хост»,
/// на которую полагаться нельзя (INV-F11-2).
#[test]
fn f11_scheme_relative_location_does_not_smuggle_the_token_to_another_host() {
    let host_b = ProbeServer::start(ok_response);
    let location = format!("//{}/redirected", host_b.addr());
    let host_a = ProbeServer::start(move |_| {
        raw_response("307 Temporary Redirect", &[("Location", &location)], "")
    });

    let _ = transport_to(&host_a.url()).send(ALERT_TEXT);

    assert_eq!(host_a.hits(), 1, "запрос не дошёл до эндпоинта");
    assert_untouched("хост B (схемо-относительный Location)", &host_b);
    assert_no_referer("хост A", &host_a);
}

/// Границы №2: относительный `Location` на ТОТ ЖЕ хост. Редирект не следуется (INV-F11-2:
/// «тот же хост» — вывод из данных удалённой стороны, а Telegram Bot API не редиректит),
/// поэтому второго запроса нет и `Referer` с токеном не появляется.
#[test]
fn f11_same_host_relative_redirect_is_not_followed_and_sends_no_referer() {
    let host_a = ProbeServer::start(|req| {
        if req.starts_with("POST /redirected") || req.starts_with("GET /redirected") {
            raw_response("200 OK", &[], "{\"ok\":true}")
        } else {
            raw_response("307 Temporary Redirect", &[("Location", "/redirected")], "")
        }
    });

    let result = transport_to(&host_a.url()).send(ALERT_TEXT);

    assert_no_referer("хост A (редирект на самого себя)", &host_a);
    assert_eq!(
        host_a.hits(),
        1,
        "редирект был проследован ({} запросов вместо 1) — адрес назначения секрета переписан \
         удалённой стороной (R-009 F-11):\n{}",
        host_a.hits(),
        host_a.requests().join("\n---\n")
    );
    let err = result.expect_err("непроследованный редирект = сообщение не доставлено");
    assert_error_carries_no_secret("TransportError (same-host редирект)", &err.to_string());
}

/// Отсутствие (`testing.md` п.3): `3xx` БЕЗ `Location`. Не паника, не «успех» — честный провал
/// доставки без секрета в тексте.
#[test]
fn f11_redirect_without_location_header_is_an_honest_failure() {
    let host_a = ProbeServer::start(|_| raw_response("302 Found", &[], ""));

    let err = transport_to(&host_a.url())
        .send(ALERT_TEXT)
        .expect_err("3xx без Location — сообщение никуда не доставлено");

    assert_eq!(host_a.hits(), 1, "запрос не дошёл до эндпоинта");
    assert!(
        err.to_string().contains("302"),
        "статус потерян, разбирать нечего: {err}"
    );
    assert_error_carries_no_secret("TransportError (3xx без Location)", &err.to_string());
}

/// ПАРНЫЙ VANTAGE: без редиректа доставка обязана работать как раньше — запрос уходит по
/// каноническому пути Telegram Bot API и возвращает `Ok`. Ловит «фикс», который выключает
/// доставку целиком.
#[test]
fn f11_delivery_without_redirect_still_succeeds_on_the_canonical_path() {
    let host_a = ProbeServer::start(ok_response);

    transport_to(&host_a.url())
        .send(ALERT_TEXT)
        .expect("без редиректа доставка обязана оставаться успешной");

    let requests = host_a.requests();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0].starts_with(&format!("POST /bot{TOKEN}/sendMessage")),
        "запрос ушёл не по каноническому пути Telegram Bot API: {}",
        requests[0].lines().next().unwrap_or("")
    );
    assert!(
        requests[0].contains("chat_id"),
        "в теле запроса нет chat_id — доставка сломана"
    );
}

/// СКВОЗНОЙ оракул (прод-масштаб): НАСТОЯЩИЙ бинарь `ops-watchdog` — тот артефакт, который
/// уезжает на VPS и которому founder руками выставит `TELEGRAM_API_BASE`. За один прогон он
/// шлёт несколько алертов, каждый обязан упереться в тот же барьер.
#[test]
fn f11_watchdog_binary_never_sends_the_token_to_a_redirect_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    let heartbeat_path = dir.path().join("recorder.heartbeat");
    // Прод-форма heartbeat'а (замер VPS 2026-07-31); ts_wall_ms — эпоха, т.е. заведомо протух.
    std::fs::write(
        &heartbeat_path,
        r#"{"events":3456495,"free_bytes":83116052480,"min_free_bytes":10737418240,"next_seq":140762639,"segment_index":145,"ts_wall_ms":1,"writable":true}"#,
    )
    .expect("write heartbeat");

    let host_b = ProbeServer::start(ok_response);
    let b_url = format!("{}/redirected", host_b.url());
    let host_a = ProbeServer::start(move |_| {
        raw_response("307 Temporary Redirect", &[("Location", &b_url)], "")
    });

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ops-watchdog"))
        .env("WATCHDOG_HEARTBEAT_PATH", &heartbeat_path)
        .env("WATCHDOG_CRON_DIR", dir.path())
        .env(
            "WATCHDOG_STATE_PATH",
            dir.path().join("watchdog.state.json"),
        )
        .env("WATCHDOG_CONTAINERS", "")
        .env("WATCHDOG_HOST_LABEL", "oracle-f11")
        .env("TELEGRAM_BOT_TOKEN", TOKEN)
        .env("TELEGRAM_CHAT_ID", CHAT_ID)
        .env("TELEGRAM_API_BASE", host_a.url())
        .output()
        .expect("бинарь ops-watchdog обязан запускаться");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Анти-плацебо: алерт сформирован и доставка была реально предпринята.
    assert!(
        combined.contains("WD-HB-STALE"),
        "протухший heartbeat не дал алерта — сквозной путь не пройден:\n{combined}"
    );
    assert!(
        host_a.hits() >= 1,
        "бинарь не обратился к сконфигурированному эндпоинту:\n{combined}"
    );

    assert_untouched("хост B (цель редиректа, сквозной прогон бинаря)", &host_b);
    assert_no_referer("хост A (сквозной прогон бинаря)", &host_a);
}
