//! Транспорт доставки алертов — за трейтом, с двумя реализациями: `StdoutTransport`
//! (работает сейчас, cron перехватывает stdout в лог — та же конвенция, что у
//! `deploy/bin/journal-retention-cron.sh`) и `TelegramTransport` (код готов сейчас; без
//! `TELEGRAM_BOT_TOKEN`/`TELEGRAM_CHAT_ID` в env — НЕ падает, работает как no-op и логирует
//! факт отсутствия конфигурации). Когда founder добавит токен в окружение VPS — алерты
//! пойдут без единой правки кода.
//!
//! Использует `reqwest::blocking` (не `tokio`): бинарь — одноразовый cron-процесс, а не
//! долгоживущий сервис, и `ops` намеренно остаётся БЕЗ async-рантайма в библиотечной части
//! (см. мотивацию в `server.rs` — избежать скрытой зависимости от scheduler'а). Блокирующий
//! HTTP-клиент здесь используется ТОЛЬКО в этом модуле (I/O-граница, как `sink.rs`/`recon.rs`
//! REST-fetch остаются в `venue-*`, а не в чистых модулях `ops`).

use std::time::Duration;

/// Транспорт доставки текстового алерта. `&self` — реализации должны быть безопасны для
/// повторного использования на несколько алертов за один прогон cron'а.
pub trait Transport {
    fn send(&self, message: &str) -> Result<(), TransportError>;
}

#[derive(Debug)]
pub enum TransportError {
    Http(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Http(msg) => write!(f, "transport http error: {msg}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Пишет сообщение в stdout (одна строка на алерт — многострочные сообщения экранируем
/// заменой `\n` на `" | "`, чтобы cron-лог оставался grep-able построчно). Никогда не
/// возвращает `Err`.
pub struct StdoutTransport;

impl Transport for StdoutTransport {
    fn send(&self, message: &str) -> Result<(), TransportError> {
        println!("{}", message.replace('\n', " | "));
        Ok(())
    }
}

/// Дефолтный (продовый) базовый URL Telegram Bot API. Подменяемость (см.
/// `with_credentials_and_endpoint`) нужна ТОЛЬКО тестам — прод обязан остаться на этом
/// адресе (R-005 F-2, `f2_default_endpoint_is_production_telegram`).
pub const DEFAULT_TELEGRAM_API_BASE: &str = "https://api.telegram.org";

/// Учётные данные Telegram Bot API. `None` — транспорт не сконфигурирован (нет токена).
pub struct TelegramTransport {
    credentials: Option<(String, String)>,
    api_base: String,
    client: reqwest::blocking::Client,
}

impl TelegramTransport {
    /// Читает `TELEGRAM_BOT_TOKEN`/`TELEGRAM_CHAT_ID` из окружения процесса. Обе переменные
    /// обязаны быть непустыми, иначе транспорт считается НЕ сконфигурированным. Также читает
    /// необязательный `TELEGRAM_API_BASE` (по умолчанию — прод-адрес Telegram) — это то, что
    /// делает сквозной оракул F-2 (`red_ops_transport_redaction.rs`) проверяемым: он
    /// подставляет сюда мёртвый эндпоинт и гоняет РЕАЛЬНЫЙ бинарь `ops-watchdog`, а не только
    /// библиотечную половину.
    pub fn from_env() -> Self {
        let api_base = std::env::var("TELEGRAM_API_BASE")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_TELEGRAM_API_BASE.to_string());
        Self::with_credentials_and_endpoint(read_env_credentials(), &api_base)
    }

    /// Явный конструктор — основной путь тестирования (не зависит от того, стоит ли токен
    /// в env машины, на которой гоняются тесты; `from_env` — тонкая обёртка над этим).
    /// Эндпоинт — продовый (`DEFAULT_TELEGRAM_API_BASE`); для подмены см.
    /// `with_credentials_and_endpoint`.
    pub fn with_credentials(credentials: Option<(String, String)>) -> Self {
        Self::with_credentials_and_endpoint(credentials, DEFAULT_TELEGRAM_API_BASE)
    }

    /// Полный конструктор с подменяемым базовым URL Telegram Bot API. Без него путь доставки
    /// (и путь его сетевой ошибки — R-005 F-2) непроверяем офлайн.
    pub fn with_credentials_and_endpoint(
        credentials: Option<(String, String)>,
        api_base: &str,
    ) -> Self {
        Self {
            credentials,
            api_base: api_base.to_string(),
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("reqwest::blocking::Client::build — TLS backend недоступен"),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.credentials.is_some()
    }
}

fn read_env_credentials() -> Option<(String, String)> {
    let token = std::env::var("TELEGRAM_BOT_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    let chat_id = std::env::var("TELEGRAM_CHAT_ID")
        .ok()
        .filter(|s| !s.is_empty());
    match (token, chat_id) {
        (Some(t), Some(c)) => Some((t, c)),
        _ => None,
    }
}

impl Transport for TelegramTransport {
    fn send(&self, message: &str) -> Result<(), TransportError> {
        let Some((token, chat_id)) = &self.credentials else {
            eprintln!(
                "[ops::transport] TelegramTransport: транспорт не сконфигурирован (нет \
                 TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID в окружении) — сообщение НЕ отправлено \
                 в Telegram, no-op"
            );
            return Ok(());
        };
        let url = format!("{}/bot{token}/sendMessage", self.api_base);
        let resp = self
            .client
            .post(&url)
            .form(&[("chat_id", chat_id.as_str()), ("text", message)])
            .send()
            .map_err(|e| TransportError::Http(redact_reqwest_error(&e)))?;
        if !resp.status().is_success() {
            let status = resp.status();
            // R-008 F-2 (rev2 находка): тело НЕ читается в сообщение. Оно получено от
            // удалённой стороны — той же категории недоверенных данных, что и `reqwest::Error`
            // выше (`redact_reqwest_error`): прокси/CDN/captive-portal/неверно настроенный
            // `TELEGRAM_API_BASE` может эхом вернуть URI запроса, в котором вшит
            // `TELEGRAM_BOT_TOKEN`. Инвариант тот же — "секрет живёт в значении, а не в
            // конкретном форматтере", поэтому значение просто не попадает в `TransportError`.
            // `status` безопасен: код + каноническая reason-phrase генерируются локально
            // крейтом `http` по фиксированной таблице (RFC), это не эхо удалённого текста.
            return Err(TransportError::Http(format!(
                "Telegram API вернул неуспешный статус {status} (тело ответа намеренно не \
                 печатается — R-008 F-2, может нести секрет из URL запроса)"
            )));
        }
        Ok(())
    }
}

/// R-005 F-2: `reqwest::Error::to_string()`/`{:?}` дописывают URL целиком (`" for url (...)"`,
/// `reqwest-0.12.28/src/error.rs:267-269`) — а URL здесь несёт `TELEGRAM_BOT_TOKEN` вшитым в
/// путь (`/bot<token>/sendMessage`, требование самого Telegram Bot API). Значит НИКАКОЙ путь
/// печати `reqwest::Error` (ни `{e}`, ни `{e:?}`, ни любой будущий) не безопасен — секрет
/// живёт в значении ошибки, а не в конкретном форматтере.
///
/// Поэтому эта функция вообще НЕ читает содержимое `e` (ни `Display`, ни `Debug`, ни `source()`
/// — источник у `reqwest::Error` тоже может нести URL на некоторых бэкендах, доверять нельзя):
/// секрет технически не может утечь через классификатор, который построен целиком из
/// статических строк. Диагностическая ценность — категория сбоя (`timeout`/`connect`/...),
/// этого достаточно, чтобы отличить "DNS/сеть недоступны" от "бот API вернул ошибку" —
/// последнее в редакции не нуждается (`Telegram API вернул {status}: {body}` не содержит URL).
fn redact_reqwest_error(e: &reqwest::Error) -> String {
    let kind = if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_request() {
        "request"
    } else if e.is_body() {
        "body"
    } else if e.is_decode() {
        "decode"
    } else if e.is_redirect() {
        "redirect"
    } else {
        "unknown"
    };
    format!(
        "telegram transport error ({kind}): не удалось доставить сообщение в Telegram Bot API \
         (детали редактированы — R-005 F-2, секрет живёт в URL значения ошибки)"
    )
}
