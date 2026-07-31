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

/// Учётные данные Telegram Bot API. `None` — транспорт не сконфигурирован (нет токена).
pub struct TelegramTransport {
    credentials: Option<(String, String)>,
    client: reqwest::blocking::Client,
}

impl TelegramTransport {
    /// Читает `TELEGRAM_BOT_TOKEN`/`TELEGRAM_CHAT_ID` из окружения процесса. Обе переменные
    /// обязаны быть непустыми, иначе транспорт считается НЕ сконфигурированным.
    pub fn from_env() -> Self {
        Self::with_credentials(read_env_credentials())
    }

    /// Явный конструктор — основной путь тестирования (не зависит от того, стоит ли токен
    /// в env машины, на которой гоняются тесты; `from_env` — тонкая обёртка над этим).
    pub fn with_credentials(credentials: Option<(String, String)>) -> Self {
        Self {
            credentials,
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
        let url = format!("https://api.telegram.org/bot{token}/sendMessage");
        let resp = self
            .client
            .post(&url)
            .form(&[("chat_id", chat_id.as_str()), ("text", message)])
            .send()
            .map_err(|e| TransportError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(TransportError::Http(format!(
                "Telegram API вернул {status}: {body}"
            )));
        }
        Ok(())
    }
}
