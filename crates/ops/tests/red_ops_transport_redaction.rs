//! RED-оракул на находку **F-2 (BLOCKER)** PR-гейта R-005 (`research/reviews/R-005-alerting.md`):
//! `TELEGRAM_BOT_TOKEN` утекает в лог cron'а при любой сетевой ошибке доставки.
//!
//! Механика утечки (воспроизведена reviewer'ом на живом зонде): токен вшит в URL (требование
//! Telegram Bot API, само по себе нормально), `reqwest::Error::to_string()` ДОПИСЫВАЕТ URL
//! целиком, строка уезжает в `TransportError::Http`, бинарь печатает её в stderr, а cron-обёртка
//! перенаправляет stderr в `/var/log/hft/watchdog.log`:
//!
//! ```text
//! PROBE Display = error sending request for url
//!   (https://api.telegram.org.invalid/bot1234567890:AAHsuperSECRETtokenVALUE/sendMessage)
//! ```
//!
//! Триггер — не экзотика: DNS, TLS-хендшейк, таймаут, обрыв сети. Сторож работает круглосуточно,
//! т.е. это произойдёт — и именно тогда, когда логи будут читать внимательно (инцидент).
//!
//! # Контракт, который задаёт этот файл (спецификация для engine-dev)
//!
//! 1. `TransportError`, порождаемый `TelegramTransport::send`, НЕ содержит секрета — ни в
//!    `Display`, ни в `Debug` (любой путь печати безопасен, а не только тот, что сегодня в
//!    бинаре).
//! 2. Сообщение остаётся ДИАГНОСТИЧНЫМ: по нему видно, какой транспорт упал. Редакция — это
//!    вырезание секрета, а не превращение ошибки в «что-то пошло не так».
//! 3. Появляется конструктор с подменяемым эндпоинтом + константа дефолта:
//!    ```ignore
//!    pub const DEFAULT_TELEGRAM_API_BASE: &str = "https://api.telegram.org";
//!    pub fn with_credentials_and_endpoint(
//!        credentials: Option<(String, String)>, api_base: &str) -> Self;
//!    ```
//!    `with_credentials`/`from_env` остаются и работают через дефолт. Без подменяемого
//!    эндпоинта путь доставки НЕВОЗМОЖНО проверить офлайн — а непроверяемый путь и есть тот,
//!    в котором нашли блокер (R-005 F-4/F-10).
//! 4. Бинарь `ops-watchdog` читает необязательный `TELEGRAM_API_BASE` (по умолчанию —
//!    та же константа), чтобы сквозной оракул ниже проверял РЕАЛЬНЫЙ артефакт, который
//!    уезжает на прод, а не его библиотечную половину.
//!
//! # Чек-лист деградированного входа
//! - **Отсутствие**: эндпоинт недоступен (connection refused) — путь ошибки, где и течёт.
//! - **Границы**: реалистичный токен вида `<digits>:<base64url>`, а не `"fake-token"`.
//! - **Множественность**: проверяются ОБА представления ошибки (`Display` и `Debug`) и оба
//!   потока бинаря (stdout+stderr — cron сливает их в один файл).
//! - **Прод-масштаб**: сквозной прогон настоящего бинаря с прод-формой heartbeat'а, как его
//!   запускает `scripts/watchdog_cron.sh`.

use std::io::{Read, Write};
use std::net::TcpListener;

use ops::transport::{TelegramTransport, Transport, TransportError, DEFAULT_TELEGRAM_API_BASE};

mod common;
use common::{raw_response, request_line, ProbeServer};

/// Реалистичная форма токена Telegram (`<bot_id>:<base64url>`), с маркером внутри — чтобы
/// «редакция», спрятавшая только числовой bot_id, не прошла.
const TOKEN: &str = "7891234567:AAG-ORACLE-MARKER-SECRET-DO-NOT-LOG";
const TOKEN_SECRET_PART: &str = "AAG-ORACLE-MARKER-SECRET-DO-NOT-LOG";
const CHAT_ID: &str = "-1002233445566";

/// Эндпоинт, на котором гарантированно никто не слушает (порт 1 на петле). Никакой сети
/// наружу — connection refused мгновенно, тест детерминирован и работает в CI без DNS.
const DEAD_ENDPOINT: &str = "http://127.0.0.1:1";

fn dead_transport() -> TelegramTransport {
    TelegramTransport::with_credentials_and_endpoint(
        Some((TOKEN.to_string(), CHAT_ID.to_string())),
        DEAD_ENDPOINT,
    )
}

fn assert_no_secret(where_: &str, text: &str) {
    assert!(
        !text.contains(TOKEN),
        "{where_} содержит TELEGRAM_BOT_TOKEN целиком (R-005 F-2): {text}"
    );
    assert!(
        !text.contains(TOKEN_SECRET_PART),
        "{where_} содержит секретную часть токена (R-005 F-2): {text}"
    );
    assert!(
        !text.contains(CHAT_ID),
        "{where_} содержит TELEGRAM_CHAT_ID (R-005 F-2): {text}"
    );
}

#[test]
fn f2_transport_error_display_carries_no_secret() {
    let transport = dead_transport();
    let err = transport
        .send("[CRITICAL] WD-HB-STALE — оракул F-2")
        .expect_err("недоступный эндпоинт обязан дать ошибку — иначе оракул ничего не проверяет");
    assert_no_secret("TransportError::Display", &err.to_string());
}

#[test]
fn f2_transport_error_debug_carries_no_secret() {
    // Любой путь печати обязан быть безопасен: сегодня бинарь печатает `{e}`, завтра кто-то
    // напишет `{e:?}` в новом месте — секрет не должен жить внутри значения ошибки вообще.
    let transport = dead_transport();
    let err = transport
        .send("[CRITICAL] WD-HB-STALE — оракул F-2")
        .unwrap_err();
    assert_no_secret("TransportError::Debug", &format!("{err:?}"));
}

/// ПАРНЫЙ VANTAGE к редакции: ошибка обязана остаться пригодной для разбора. Редакция —
/// вырезание секрета, а не «что-то пошло не так».
#[test]
fn f2_redacted_error_stays_diagnosable() {
    let transport = dead_transport();
    let err = transport.send("msg").unwrap_err();
    let text = err.to_string();
    assert!(
        text.to_lowercase().contains("telegram"),
        "по тексту ошибки не видно, какой транспорт упал: {text}"
    );
    assert!(
        text.len() >= 20,
        "ошибка выродилась в заглушку, разбирать нечего: {text}"
    );
    // Тип ошибки сохранён — вариант транспорта, а не паника/подмена.
    assert!(matches!(err, TransportError::Http(_)));
}

/// Дефолт эндпоинта — прод-адрес Telegram. Подменяемость нужна тестам, но прод обязан
/// остаться прежним (иначе «фикс» тихо уведёт доставку в никуда).
#[test]
fn f2_default_endpoint_is_production_telegram() {
    assert_eq!(DEFAULT_TELEGRAM_API_BASE, "https://api.telegram.org");
}

/// ПАРНЫЙ VANTAGE №2: подмена эндпоинта РАБОТАЕТ и доставка не сломана — запрос уходит по
/// каноническому пути Telegram Bot API `/bot<token>/sendMessage` с `chat_id` и текстом в теле.
/// Ловит «фикс», который прячет секрет ценой поломки самой отправки.
#[test]
fn f2_successful_delivery_still_hits_telegram_bot_api_path() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("accept");
        let mut received = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let n = sock.read(&mut chunk).unwrap_or(0);
            if n == 0 {
                break;
            }
            received.extend_from_slice(&chunk[..n]);
            if request_is_complete(&received) {
                break;
            }
        }
        sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}")
            .expect("write response");
        sock.flush().ok();
        String::from_utf8_lossy(&received).into_owned()
    });

    let transport = TelegramTransport::with_credentials_and_endpoint(
        Some((TOKEN.to_string(), CHAT_ID.to_string())),
        &format!("http://{addr}"),
    );
    assert!(transport.is_configured());
    transport
        .send("[CRITICAL] WD-SEQ-STALLED — оракул F-2, успешная доставка")
        .expect("успешная доставка обязана возвращать Ok");

    let request = server.join().expect("server thread");
    assert!(
        request.contains(&format!("POST /bot{TOKEN}/sendMessage")),
        "запрос ушёл не по каноническому пути Telegram Bot API: {}",
        request.lines().next().unwrap_or("")
    );
    assert!(
        request.contains("chat_id"),
        "в теле запроса нет chat_id — доставка сломана"
    );
}

/// Заголовки закончились и тело получено целиком (иначе закрытие сокета сервером даст RST и
/// клиент увидит ошибку вместо ответа — тест стал бы флаки).
fn request_is_complete(buf: &[u8]) -> bool {
    let text = String::from_utf8_lossy(buf);
    let Some(header_end) = text.find("\r\n\r\n") else {
        return false;
    };
    let body_start = header_end + 4;
    let content_length = text
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    buf.len() >= body_start + content_length
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// Сквозной оракул: РЕАЛЬНЫЙ бинарь, оба потока, как их сливает cron-обёртка
// ══════════════════════════════════════════════════════════════════════════════════════════

/// Библиотечная редакция ничего не стоит, если секрет печатает сам бинарь. `scripts/watchdog_cron.sh`
/// делает `"${WATCHDOG_BIN}" >>"${LOG}" 2>&1` — stdout и stderr сливаются в один файл, который
/// человек читает во время инцидента. Оракул запускает НАСТОЯЩИЙ артефакт с недоступным
/// эндпоинтом Telegram и проверяет весь его вывод.
///
/// Анти-плацебо: сначала проверяется, что путь доставки вообще был пройден (алерт сформирован
/// И попытка отправки провалилась) — отсутствие токена в пустом выводе ничего не доказывает.
#[test]
fn f2_watchdog_binary_never_prints_the_token_into_the_cron_log() {
    let dir = tempfile::tempdir().expect("tempdir");
    let heartbeat_path = dir.path().join("recorder.heartbeat");
    // Прод-форма heartbeat'а (замер VPS 2026-07-31), но ts_wall_ms — эпоха: heartbeat
    // заведомо протух → CRITICAL → доставка → сетевая ошибка.
    std::fs::write(
        &heartbeat_path,
        r#"{"events":3456495,"free_bytes":83116052480,"min_free_bytes":10737418240,"next_seq":140762639,"segment_index":145,"ts_wall_ms":1,"writable":true}"#,
    )
    .expect("write heartbeat");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ops-watchdog"))
        .env("WATCHDOG_HEARTBEAT_PATH", &heartbeat_path)
        .env("WATCHDOG_CRON_DIR", dir.path())
        .env(
            "WATCHDOG_STATE_PATH",
            dir.path().join("watchdog.state.json"),
        )
        .env("WATCHDOG_CONTAINERS", "")
        .env("WATCHDOG_HOST_LABEL", "oracle-f2")
        .env("TELEGRAM_BOT_TOKEN", TOKEN)
        .env("TELEGRAM_CHAT_ID", CHAT_ID)
        .env("TELEGRAM_API_BASE", DEAD_ENDPOINT)
        .output()
        .expect("бинарь ops-watchdog обязан запускаться");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // (1) Путь действительно пройден — иначе оракул проверяет пустоту.
    assert!(
        combined.contains("WD-HB-STALE"),
        "протухший heartbeat не дал алерта — сквозной путь не пройден, оракул недействителен:\n{combined}"
    );
    assert!(
        combined.to_lowercase().contains("telegram"),
        "в выводе нет следа неудачной доставки в Telegram — путь ошибки не пройден, оракул \
         недействителен:\n{combined}"
    );
    // (2) Собственно инвариант.
    assert_no_secret(
        "вывод бинаря ops-watchdog (stdout+stderr, cron-лог)",
        &combined,
    );
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// F-9 (BLOCKER, R-009) — редакция ТЕЛА неуспешного HTTP-ответа
// ══════════════════════════════════════════════════════════════════════════════════════════
//
// Находка R-008 («токен течёт в теле не-2xx ответа») была починена коммитом `5d55914`, но
// пришла БЕЗ оракула: reviewer в R-009 откатил фикс (мутация A — вернул `{status}: {body}` в
// `TransportError`) и получил `passed=146 failed=0`, `verify → VERDICT: PASS`. То есть блокер
// переоткрывается, и весь набор гейтов к этому слеп. Ниже — оракулы, закрывающие ровно это.
//
// # Почему прошлый набор промахнулся
//
// Все шесть оракулов выше бьют либо в МЁРТВЫЙ эндпоинт (connection refused → путь
// `redact_reqwest_error`), либо в сервер, отдающий `200 OK`. Целое семейство «сервер ответил,
// но не 2xx» — вне фикстуры. Это дословно «фикстура счастливого пути — дефект оракула»
// (`.claude/rules/testing.md`): покрыт счастливый путь и ОДИН путь ошибки, а тот, в котором
// блокер находили ДВАЖДЫ, — не покрыт.
//
// # Инвариант, который задают оракулы (спецификация)
//
// Ничто из полученного от удалённой стороны (тело ответа, его заголовки) не попадает в
// `TransportError` ни в каком виде — ни целиком, ни усечённо. Наружу идёт ТОЛЬКО статус
// (код + каноническая reason-phrase генерируются локально крейтом `http` по таблице RFC —
// это не эхо удалённого текста) и статические строки. Причина: `TELEGRAM_BOT_TOKEN` вшит в
// путь URL (требование Telegram Bot API), а прокси/CDN/captive-portal штатно возвращают эхо
// строки запроса в теле ошибки — секрет живёт в ЗНАЧЕНИИ, а не в конкретном форматтере.
//
// # Чек-лист деградированного входа (`.claude/rules/testing.md`)
// - **Множественность**: три кода (401/429/500) × два представления (`Display`/`Debug`);
//   маркер И в теле, И в заголовке ответа; сквозной прогон бинаря шлёт несколько алертов.
// - **Асимметрия**: маркер приходит ТОЛЬКО в теле/заголовке — статус при этом безобиден и
//   обязан остаться в сообщении (иначе «редакция» = потеря диагностики).
// - **Отсутствие**: не-2xx с ПУСТЫМ телом — не повод считать доставку удавшейся.
// - **Границы**: тело в 1 МиБ, где эхо URL стоит В НАЧАЛЕ, а маркер — В КОНЦЕ: усечение с
//   любой стороны («возьмём первые 200 байт для диагностики») всё равно течёт.
// - **Прод-масштаб**: сквозной прогон настоящего бинаря `ops-watchdog` (stdout+stderr, как их
//   сливает `scripts/watchdog_cron.sh`) + проверка файла состояния на диске.

/// Маркер, который «удалённая сторона» кладёт в тело ответа.
const BODY_MARKER: &str = "BODY-MARKER-FROM-REMOTE-DO-NOT-LOG";
/// Маркер в ЗАГОЛОВКЕ ответа — заголовки тоже недоверенный вход.
const HEADER_MARKER: &str = "HEADER-MARKER-FROM-REMOTE-DO-NOT-LOG";

/// Тело, которое реально отдают прокси/captive-portal: собственный маркер + ЭХО строки
/// запроса, в которой вшит `TELEGRAM_BOT_TOKEN` (`POST /bot<token>/sendMessage HTTP/1.1`).
fn hostile_body(raw_request: &str) -> String {
    format!(
        "{{\"ok\":false,\"error_code\":401,\"description\":\"{BODY_MARKER}; request was: {}\"}}",
        request_line(raw_request)
    )
}

fn assert_no_remote_content(where_: &str, text: &str) {
    assert_no_secret(where_, text);
    assert!(
        !text.contains(BODY_MARKER),
        "{where_} содержит содержимое ТЕЛА ответа удалённой стороны (R-009 F-9): {text}"
    );
    assert!(
        !text.contains(HEADER_MARKER),
        "{where_} содержит содержимое ЗАГОЛОВКА ответа удалённой стороны (R-009 F-9): {text}"
    );
}

/// ГЛАВНЫЙ оракул F-9. Мутация A из R-009 (`Err(Http(format!("...{status}: {body}")))`)
/// обязана его валить.
#[test]
fn f9_non_2xx_response_body_never_reaches_the_transport_error() {
    for (code, reason) in [
        (401, "Unauthorized"),
        (429, "Too Many Requests"),
        (500, "Internal Server Error"),
    ] {
        let server = ProbeServer::start(move |req| {
            raw_response(
                &format!("{code} {reason}"),
                &[("X-Probe-Marker", HEADER_MARKER)],
                &hostile_body(req),
            )
        });
        let transport = TelegramTransport::with_credentials_and_endpoint(
            Some((TOKEN.to_string(), CHAT_ID.to_string())),
            &server.url(),
        );

        let err = transport
            .send("[CRITICAL] WD-HB-STALE — оракул F-9")
            .expect_err("не-2xx ответ обязан быть ошибкой доставки, а не тихим успехом");

        // (1) Анти-плацебо: пройден именно путь «сервер ОТВЕТИЛ не-2xx», а не connect-error.
        assert_eq!(
            server.hits(),
            1,
            "запрос не доехал до сервера — оракул проверяет не тот путь"
        );
        let display = err.to_string();
        assert!(
            display.contains(&code.to_string()),
            "в ошибке нет статуса {code} — значит сработала другая ветка, оракул недействителен: {display}"
        );

        // (2) Собственно инвариант — оба представления ошибки.
        assert_no_remote_content(
            &format!("TransportError::Display (статус {code})"),
            &display,
        );
        assert_no_remote_content(
            &format!("TransportError::Debug (статус {code})"),
            &format!("{err:?}"),
        );
    }
}

/// Границы + прод-масштаб: тело в 1 МиБ, эхо URL в НАЧАЛЕ, маркер в КОНЦЕ. «Редакция»,
/// сводящаяся к усечению («первые/последние N байт для диагностики»), течёт в обе стороны.
/// Плюс ограничение размера самого сообщения об ошибке — иначе мегабайт недоверенного текста
/// уезжает в cron-лог даже без секрета.
#[test]
fn f9_huge_hostile_body_is_not_read_into_the_error_even_truncated() {
    let server = ProbeServer::start(|req| {
        let head = format!("request was: {}", request_line(req));
        let body = format!("{head}{}{BODY_MARKER}", "x".repeat(1_000_000));
        raw_response("502 Bad Gateway", &[], &body)
    });
    let transport = TelegramTransport::with_credentials_and_endpoint(
        Some((TOKEN.to_string(), CHAT_ID.to_string())),
        &server.url(),
    );

    let err = transport
        .send("[CRITICAL] WD-HB-STALE — оракул F-9")
        .unwrap_err();
    assert_eq!(server.hits(), 1, "запрос не доехал — оракул недействителен");
    let display = err.to_string();
    assert!(display.contains("502"), "не та ветка ошибки: {display}");
    assert_no_remote_content("TransportError::Display (тело 1 МиБ)", &display);
    assert_no_remote_content("TransportError::Debug (тело 1 МиБ)", &format!("{err:?}"));
    assert!(
        display.len() < 500,
        "сообщение об ошибке несёт {} байт — в него затянуто тело ответа удалённой стороны",
        display.len()
    );
}

/// Отсутствие (`testing.md` п.3): у не-2xx ответа НЕТ тела. Это не повод считать доставку
/// удавшейся и не повод выродить ошибку в заглушку — статус обязан остаться в сообщении.
#[test]
fn f9_non_2xx_with_empty_body_is_still_a_diagnosable_failure() {
    let server = ProbeServer::start(|_| raw_response("403 Forbidden", &[], ""));
    let transport = TelegramTransport::with_credentials_and_endpoint(
        Some((TOKEN.to_string(), CHAT_ID.to_string())),
        &server.url(),
    );

    let err = transport
        .send("[CRITICAL] WD-HB-STALE — оракул F-9")
        .expect_err("403 с пустым телом — всё ещё провал доставки");
    assert_eq!(server.hits(), 1);
    let display = err.to_string();
    assert!(display.contains("403"), "статус потерян: {display}");
    assert!(
        display.to_lowercase().contains("telegram"),
        "по ошибке не видно, какой транспорт упал: {display}"
    );
    assert_no_remote_content("TransportError (пустое тело)", &display);
}

/// ПАРНЫЙ VANTAGE: тот же враждебный контент в теле, но статус 2xx — доставка удалась,
/// ошибки нет. Ловит «фикс» вида «считать любой ответ провалом».
#[test]
fn f9_hostile_body_on_success_status_is_still_a_successful_delivery() {
    let server = ProbeServer::start(|req| {
        raw_response(
            "200 OK",
            &[("X-Probe-Marker", HEADER_MARKER)],
            &hostile_body(req),
        )
    });
    let transport = TelegramTransport::with_credentials_and_endpoint(
        Some((TOKEN.to_string(), CHAT_ID.to_string())),
        &server.url(),
    );
    transport
        .send("[CRITICAL] WD-SEQ-STALLED — оракул F-9, успешная доставка")
        .expect("2xx обязан оставаться успехом, что бы ни лежало в теле");
    assert_eq!(server.hits(), 1);
}

/// СКВОЗНОЙ оракул F-9 — дыра R-008 была именно в том, что библиотечная половина проходила,
/// а путь целиком — нет. Гоняется НАСТОЯЩИЙ бинарь `ops-watchdog` против сервера, который
/// отдаёт 401 с враждебным телом и заголовком; проверяются ОБА потока (cron сливает их в один
/// файл) И файл состояния на диске.
#[test]
fn f9_watchdog_binary_never_logs_the_non_2xx_response_body() {
    let dir = tempfile::tempdir().expect("tempdir");
    let heartbeat_path = dir.path().join("recorder.heartbeat");
    let state_path = dir.path().join("watchdog.state.json");
    // Прод-форма heartbeat'а (замер VPS 2026-07-31); ts_wall_ms — эпоха, т.е. заведомо протух.
    std::fs::write(
        &heartbeat_path,
        r#"{"events":3456495,"free_bytes":83116052480,"min_free_bytes":10737418240,"next_seq":140762639,"segment_index":145,"ts_wall_ms":1,"writable":true}"#,
    )
    .expect("write heartbeat");

    let server = ProbeServer::start(move |req| {
        raw_response(
            "401 Unauthorized",
            &[("X-Probe-Marker", HEADER_MARKER)],
            &hostile_body(req),
        )
    });

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ops-watchdog"))
        .env("WATCHDOG_HEARTBEAT_PATH", &heartbeat_path)
        .env("WATCHDOG_CRON_DIR", dir.path())
        .env("WATCHDOG_STATE_PATH", &state_path)
        .env("WATCHDOG_CONTAINERS", "")
        .env("WATCHDOG_HOST_LABEL", "oracle-f9")
        .env("TELEGRAM_BOT_TOKEN", TOKEN)
        .env("TELEGRAM_CHAT_ID", CHAT_ID)
        .env("TELEGRAM_API_BASE", server.url())
        .output()
        .expect("бинарь ops-watchdog обязан запускаться");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server.settle();

    // (1) Анти-плацебо: алерт сформирован, запрос ушёл, сервер ответил НЕ-2xx и это видно.
    assert!(
        combined.contains("WD-HB-STALE"),
        "протухший heartbeat не дал алерта — сквозной путь не пройден:\n{combined}"
    );
    assert!(
        server.hits() >= 1,
        "бинарь не обратился к Telegram API — путь доставки не пройден:\n{combined}"
    );
    assert!(
        combined.contains("401"),
        "в выводе нет следа неуспешного СТАТУСА — пройдена другая ветка ошибки (connect?), \
         оракул недействителен:\n{combined}"
    );

    // (2) Инвариант — вывод бинаря.
    assert_no_remote_content(
        "вывод бинаря ops-watchdog (stdout+stderr, cron-лог)",
        &combined,
    );
    // (3) Инвариант — файл состояния (живёт на диске рядом с cron-маркерами и переживает прогон).
    let state = std::fs::read_to_string(&state_path).expect("файл состояния обязан быть записан");
    assert_no_remote_content("watchdog.state.json", &state);
}
