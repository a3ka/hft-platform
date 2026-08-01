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
