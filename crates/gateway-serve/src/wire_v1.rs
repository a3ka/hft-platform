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
use gateway::{view::ViewSpec, Frame, Selector, Snapshot};
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
        /// M-100 (§4.2, CT-RFC-09 §2.10.2): проекция отображения (выбор строк глубины
        /// из готового расчёта). `Option<Value>` — на проводе это JSON-объект
        /// `{"depth_rows":[{"band_pct_e8":...,"side":"bid"}, ...]}`, отсутствующий —
        /// «всё показывать». Парсится в `gateway::view::ViewSpec` позже (после
        /// резолва `selector.bands`, чтобы валидировать `band_pct_e8` против ключа).
        #[serde(default)]
        view: Option<Value>,
    },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { v: u64, id: String },
}

impl ClientMessage {
    // Задача 13 §12 N-6: методы `version()` и `id()` удалены — ноль вызовов в крэйтe
    // (`grep -rnE 'msg\.version|msg\.id|parsed\.version|parsed\.id' crates/gateway-serve`
    // пуст). Метод `version()` заявлялся «для диагностики в error-сообщениях», но
    // `parse_error_code`/`parse_error_message` работают с `ParseError`, а не с
    // `ClientMessage` — клиент прислал мусор, и версия неизвестна. Метод `id()` —
    // то же: разбор идёт через `match` на конкретный вариант (`Subscribe { id, .. }`).
    //
    // Мёртвый публичный API — источник ложной уверенности: читающий код видит метод,
    // которого на пути нет, и достраивает несуществующую гарантию. Ровно тот же класс,
    // что `Sub::generation` до §10 (R-086), — дешевле.
}

/// Ошибка разбора клиентского сообщения (разбирается внутри `parse_message`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
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
///
/// M-100 (§4.1, CT-RFC-09 §2.10.1): `selector.bands` НЕОБЯЗАТЕЛЕН. Отсутствует ⇒
/// полосы ключа = `canonical_bands` (задаются политикой допуска). Присутствует и
/// РАВЕН `canonical_bands` (порядок И значения) ⇒ принимается. Присутствует и НЕ
/// равен ⇒ `SelectorError::UnsupportedBands` (отказ `unsupported` на проводе,
/// §3 п.1 запрет подмены).
pub fn parse_selector(value: &Value, canonical_bands: &[f64]) -> Result<Selector, SelectorError> {
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
    // Извлекаем `bands` ДО serde-десериализации (она требует `Vec<f64>` нончего):
    //   - None → подставим `canonical_bands` ниже;
    //   - Some и равен canonical → примем как есть;
    //   - Some и НЕ равен → отказ `UnsupportedBands` (`unsupported` на проводе).
    let bands = value.get("bands").map(|v| match v.as_array() {
        Some(arr) => arr
            .iter()
            .map(|x| {
                x.as_f64().ok_or_else(|| {
                    SelectorError::Invalid(format!("bands entry {x:?} not a number"))
                })
            })
            .collect::<Result<Vec<f64>, _>>()
            .map(Some),
        None => Err(SelectorError::Invalid("bands is not an array".to_string())),
    });
    let bands: Option<Vec<f64>> = match bands {
        Some(Ok(Some(b))) => Some(b),
        Some(Ok(None)) => Some(vec![]),
        Some(Err(e)) => return Err(e),
        None => None, // ключ `bands` отсутствует в JSON
    };
    let resolved_bands: Vec<f64> = match bands {
        None => {
            // M-100 (§4.1, CT-RFC-09 §2.10.1): полосы отсутствуют — подставляем
            // канонический набор ТОЛЬКО при наличии политики (`canonical_bands` не
            // пустой). Legacy-режим (без политики, `canonical_bands == &[]`)
            // ОБЯЗАН прислать полосы — иначе перейти на профильный режим.
            // Прямой отказ (`invalid_selector`) — ниже в коде через пустой резолв.
            if canonical_bands.is_empty() {
                return Err(SelectorError::Invalid(
                    "selector.bands is required (legacy mode without policy)".to_string(),
                ));
            }
            canonical_bands.to_vec()
        }
        Some(client_bands) if bands_equal(&client_bands, canonical_bands) => {
            // Равен (порядок + значения) ⇒ принимается как есть. Это переходная
            // совместимость v1-клиентов (CT-RFC-09 §2.2 rev2 присылают `bands`).
            client_bands
        }
        Some(client_bands) if canonical_bands.is_empty() => {
            // Legacy-режим (нет политики, `canonical_bands == &[]`): полосы
            // обязательны и принимаются как есть (прежняя семантика M-65/M-87).
            client_bands
        }
        Some(_) => {
            // Подмножество / надмножество / перестановка / чужие полосы ⇒ отказ
            // (`П-030`: параметры не подменяются). Сообщение называет `view.depth_rows`
            // как способ выбрать отображение.
            return Err(SelectorError::UnsupportedBands);
        }
    };
    // Строим `Selector` напрямую (минуя serde для `bands`) — `bands` уже резолвлены.
    let mut sel_value = value.clone();
    sel_value["bands"] = serde_json::to_value(&resolved_bands).expect("bands serialize");
    let sel: Selector = serde_json::from_value(sel_value)
        .map_err(|e| SelectorError::Invalid(format!("selector deserialize: {e}")))?;
    if sel.venue != venue {
        return Err(SelectorError::Invalid(format!(
            "venue mismatch: declared {venue:?}, deserialized {:?}",
            sel.venue
        )));
    }
    Ok(sel)
}

/// Сравнение двух списков `band_pct_e8` (f64, через `(b*1e8).round() as i64`) — порядок
/// и значения. `bands_subset`-семантика здесь НЕ подходит: ровно-равенство.
fn bands_equal(a: &[f64], b: &[f64]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (x, y) in a.iter().zip(b.iter()) {
        let xi = (x * 1e8).round() as i64;
        let yi = (y * 1e8).round() as i64;
        if xi != yi {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorError {
    UnknownVenue(String),
    Invalid(String),
    /// M-100 (§4.1, CT-RFC-09 §2.10.1, П-030): клиентские `bands` ≠ каноническому
    /// набору (подмножество, надмножество, перестановка, чужие полосы). На проводе — код
    /// `unsupported` (сообщение называет `view.depth_rows` как способ выбрать отображение).
    UnsupportedBands,
}

impl SelectorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownVenue(_) => "unknown_venue",
            Self::Invalid(_) => "invalid_selector",
            Self::UnsupportedBands => "unsupported",
        }
    }
}

/// Сериализация snapshot-сообщения в NEW wire-форму.
///
/// M-100 (§4.2, CT-RFC-09 §2.10.2): конверт снимка повторяет принятый выбор —
/// `"view":{...}|null` после `sub`. `view == None` — клиент НЕ прислал выбора (или
/// прислдал пустой), показывать все строки расчёта.
pub fn snapshot_msg(sub: &str, snap: &Snapshot, view: Option<&ViewSpec>) -> Value {
    let view_inner = view.cloned();
    let data = serde_json::to_value(snap).expect("Snapshot Serialize");
    // Форма `view`-поля в конверте: JSON-объект, если задан; `null`, если нет.
    // Зеркалит `sub` (тоже `string|null`).
    let view_v = match &view_inner {
        Some(v) => match &v.depth_rows {
            // `depth_rows == None` (или пустой Vec) — тождество; на проводе тоже `null`,
            // чтобы клиент не различал «всё» и «явно все».
            None => Value::Null,
            Some(rows) if rows.is_empty() => Value::Null,
            Some(rows) => {
                serde_json::json!({"depth_rows": serde_json::to_value(rows).expect("depth_rows serialize")})
            }
        },
        None => Value::Null,
    };
    json!({"type":"snapshot","v":1,"sub":sub,"view":view_v,"data":data})
}

/// Сериализация frame-сообщения в NEW wire-форму. Проекция `view` уже применена
/// к `frame.delta.depth_series` вызывающей стороной (`project_view`) ДО сериализации
/// (`CT-RFC-09` §2.10.2: проекция применяется транспортом к снимку И к каждому кадру
/// подписки ПОСЛЕ проверки предела объёма).
pub fn frame_msg(sub: &str, frame: &Frame) -> Value {
    let data = serde_json::to_value(frame).expect("Frame Serialize");
    json!({"type":"frame","v":1,"sub":sub,"data":data})
}

/// Сериализация error-сообщения в NEW wire-форму.
/// `sub` — id подписки, к которой относится ошибка (если применимо), `None` — для
/// session-level ошибок (неизвестная op/version).
///
/// CT-RFC-09 §2.3: тип поля `sub` — `"<id>|null"`, то есть JSON-литерал `null` (НЕ строка
/// `"null"`). Строка `"null"` делает подписку с `id == "null"` неотличимой от session-level
/// ошибки, и клиент, разбирающий кадр по ТИПУ узла, не может их развести (задача 13 N-4).
pub fn error_msg(sub: Option<&str>, code: &str, message: &str) -> Value {
    // `Value::Null` — JSON `null`; `Value::String(..)` — строка. Совпадение ТИПОВ двух
    // разных кадров и есть дефект N-4, который не ловил ни один старый оракул (хелпер
    // `sub_of` читал `.as_str()`, и `null`/`"null"` давали ему `None`/`Some("null")`).
    let sub_v = match sub {
        Some(id) => Value::String(id.to_string()),
        None => Value::Null,
    };
    json!({
        "type":"error",
        "v":1,
        "sub":sub_v,
        "code":code,
        "message":message,
    })
}
