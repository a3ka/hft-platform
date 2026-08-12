//! M-65 wire protocol v1 (`CT-RFC-09` §2.2/§2.3).
//!
//! Клиент → сервер (`CT-RFC-09` §2.2):
//! ```jsonc
//! {"op":"subscribe",   "v":1, "id":"<sub-id>", "selector":{...}}
//! {"op":"unsubscribe", "v":1, "id":"<sub-id>"}
//! ```
//!
//! Сервер → клиент (новая форма):
//! ```jsonc
//! {"type":"snapshot","v":1,"sub":"<id>","data":{...}}   // Snapshot как сегодня
//! {"type":"frame",   "v":1,"sub":"<id>","data":{...}}   // Frame как сегодня
//! {"type":"error",   "v":1,"sub":"<id>|null","code":"...","message":"..."}
//! ```
//!
//! Совместимость с legacy-клиентами (без `subscribe` в grace-окне, §2.5): `ServeMsg::Snapshot` /
//! `ServeMsg::Frame` в СТАРОЙ форме через `wire::ServeMsg` — отдельный путь, не здесь.
//!
//! Таксономия машиночитаемых `code` (для `error`-сообщений):
//! - `unknown_version` (`O-3`): неизвестная `v` клиента. Спека §2.2 требует явный отказ.
//! - `unknown_op` (`O-3`): неизвестная операция. Расширение `O-3` по `M-65-ws-session.md` §3.1
//!   (N-1: молчание на неизвестной операции — тот же класс, что молчание на неизвестной версии).
//! - `unknown_venue` (`O-6`/§2.7): неизвестная площадка в `selector.venue`.
//! - `invalid_selector` (`O-7`/§2.7): пустой `symbol`, `bands` вне `(0, 1)` или с дублями,
//!   `timeframe_ms <= 0` или не выравнен по UTC-суткам, отсутствующие обязательные поля.
//! - `subscription_cap_exceeded` (`O-4`/§2.6): превышение `max_subscriptions_per_connection`.
//! - `unknown_id` (`O-10`/§4.2): `unsubscribe` уже снятого или никогда не существовавшего id.

use contracts::Venue;
use gateway::{Frame, Selector, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Клиентские сообщения (CT-RFC-09 §2.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum ClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe {
        v: u64,
        id: String,
        #[serde(default)]
        selector: Option<Value>,
    },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { v: u64, id: String },
}

impl ClientMessage {
    /// Извлечь версию протокола (`v`). Используется для диагностики в `error`-сообщениях.
    pub fn version(&self) -> u64 {
        match self {
            Self::Subscribe { v, .. } => *v,
            Self::Unsubscribe { v, .. } => *v,
        }
    }

    /// Извлечь id подписки из subscribe/unsubscribe.
    pub fn id(&self) -> &str {
        match self {
            Self::Subscribe { id, .. } => id,
            Self::Unsubscribe { id, .. } => id,
        }
    }
}

/// Ошибка разбора клиентского сообщения (разбирается внутри `parse_message`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Сообщение пришло не как `Text`/`Binary` (например, `Ping`/`Close`/`Pong`).
    /// Вызывающий решает, считать ли это событием или просто игнорировать; здесь мы
    /// не считаем это ошибкой протокола — `not_text_payload` его для удобства.
    NotTextPayload,
    /// Невалидный JSON — клиент прислал мусор.
    InvalidJson(String),
    /// Структура JSON не соответствует ни subscribe, ни unsubscribe — `unknown_op`
    /// по `CT-RFC-09` §2.2 и §3.1 (N-1). Заметим: `unknown_op` — это код ошибки, а
    /// `ParseError::UnknownShape` — внутренний тип.
    UnknownShape(String),
    /// `v` отсутствует, или не число, или не равно 1 (текущая поддерживаемая версия).
    /// Это `unknown_version` (`O-3`).
    UnknownVersion { found: Option<Value> },
    /// `selector` отсутствует в subscribe (обязательное поле, §2.2).
    MissingSelector,
    /// `selector` JSON присутствует, но не десеризуется в `gateway::Selector`
    /// (неизвестная `venue`, плохие типы полей и т.п.). Это `unknown_venue` или
    /// `invalid_selector` в зависимости от причины (вызывающий разбирает).
    MalformedSelector(String),
}

/// Парсинг клиентского сообщения из сырых байт WS-сообщения (Text или Binary).
pub fn parse_message(bytes: &[u8]) -> Result<ClientMessage, ParseError> {
    // Step 1: JSON-парсинг.
    let v: Value =
        serde_json::from_slice(bytes).map_err(|e| ParseError::InvalidJson(e.to_string()))?;
    // Step 2: достаём `op` и `v`.
    let op = v.get("op").and_then(|x| x.as_str());
    let ver = v.get("v").cloned();
    let ver_u64 = match &ver {
        Some(x) => x.as_u64().ok_or_else(|| ParseError::UnknownVersion {
            found: Some(x.clone()),
        })?,
        None => return Err(ParseError::UnknownVersion { found: None }),
    };
    if ver_u64 != 1 {
        return Err(ParseError::UnknownVersion { found: ver });
    }
    // Step 3: парсим структуру по `op`.
    match op {
        Some("subscribe") => {
            // Парсим через serde для строгой структуры.
            let parsed: ClientMessage = serde_json::from_value(v.clone())
                .map_err(|e| ParseError::UnknownShape(format!("subscribe: {e}")))?;
            if let ClientMessage::Subscribe { ref selector, .. } = parsed {
                if selector.is_none() {
                    return Err(ParseError::MissingSelector);
                }
            }
            Ok(parsed)
        }
        Some("unsubscribe") => {
            let parsed: ClientMessage = serde_json::from_value(v.clone())
                .map_err(|e| ParseError::UnknownShape(format!("unsubscribe: {e}")))?;
            Ok(parsed)
        }
        Some(other) => Err(ParseError::UnknownShape(format!("unknown op: {other:?}"))),
        None => Err(ParseError::UnknownShape("missing op".to_string())),
    }
}

/// Парсинг `selector`-поля (Value → `gateway::Selector`) со специфичными кодами ошибок.
/// `unknown_venue` vs `invalid_selector` различается здесь.
pub fn parse_selector(value: &Value) -> Result<Selector, SelectorError> {
    // Venue: строго проверяем (CT-RFC-09 §2.7: неизвестная площадка — `unknown_venue`).
    let venue_str = value
        .get("venue")
        .and_then(|x| x.as_str())
        .ok_or_else(|| SelectorError::Invalid("missing venue".to_string()))?;
    let venue = match venue_str {
        "Binance" => Venue::Binance,
        "Hyperliquid" => Venue::Hyperliquid,
        "BinanceFutures" => Venue::BinanceFutures,
        other => return Err(SelectorError::UnknownVenue(other.to_string())),
    };
    // Дальше делегируем в serde — `gateway::Selector` уже валидируется нами в `session.rs`.
    let sel: Selector = serde_json::from_value(value.clone())
        .map_err(|e| SelectorError::Invalid(format!("selector deserialize: {e}")))?;
    // Проверяем, что venue в десериализованном совпадает с тем, что мы прочитали (защита от
    // прохождения через serde мусора в виде Enum-тега).
    if sel.venue != venue {
        return Err(SelectorError::Invalid(format!(
            "venue mismatch: declared {venue:?}, deserialized {:?}",
            sel.venue
        )));
    }
    Ok(sel)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorError {
    UnknownVenue(String),
    Invalid(String),
}

impl SelectorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownVenue(_) => "unknown_venue",
            Self::Invalid(_) => "invalid_selector",
        }
    }
}

/// Сериализация snapshot-сообщения в NEW wire-форму.
pub fn snapshot_msg(sub: &str, snap: &Snapshot) -> Value {
    let data = serde_json::to_value(snap).expect("Snapshot Serialize");
    json!({"type":"snapshot","v":1,"sub":sub,"data":data})
}

/// Сериализация frame-сообщения в NEW wire-форму.
pub fn frame_msg(sub: &str, frame: &Frame) -> Value {
    let data = serde_json::to_value(frame).expect("Frame Serialize");
    json!({"type":"frame","v":1,"sub":sub,"data":data})
}

/// Сериализация error-сообщения в NEW wire-форму.
/// `sub` — id подписки, к которой относится ошибка (если применимо), `None` — для
/// session-level ошибок (неизвестная op/version).
pub fn error_msg(sub: Option<&str>, code: &str, message: &str) -> Value {
    json!({
        "type":"error",
        "v":1,
        "sub":sub.unwrap_or("null"),
        "code":code,
        "message":message,
    })
}
