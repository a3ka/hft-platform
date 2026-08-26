//! RED `PL-I-5` (sacred, architect-only) — **НАСТРОЙКА ПРЕДЕЛА УПРАВЛЯЕТ ПОВЕДЕНИЕМ.**
//!
//! Милестоун `milestones/M-71-egress-cap.md`. Исполнение вердикта
//! `research/reviews/R-133-M-71-egress-cap-impl.md`, блокер **B-1**.
//!
//! # Предмет — «built-not-wired» в чистом виде
//!
//! Ручка `GATEWAY_MAX_RESPONSE_BYTES` разбирается, валидируется, кладётся в память — и
//! НИКЕМ НЕ ЧИТАЕТСЯ. Замер `R-133`: `grep effective_max_response_bytes` даёт три
//! совпадения — геттер (`lib.rs:243`), сеттер (`:248`), один вызов сеттера (`:2166`).
//! Читателей ноль; все десять точек предела в `crates/gateway` передают КОНСТАНТУ
//! `DEFAULT_MAX_RESPONSE_BYTES`. Оператор пишет в конфиге «предел 1000 байт» — сервис
//! отдаёт 224 854.
//!
//! Десять оракулов `red_egress_cap_startup.rs` этого не видят ПО ПОСТРОЕНИЮ: все судят
//! `Ok`/`Err` разбора значения, ни один — что разобранное значение чем-то УПРАВЛЯЕТ. Шаг `D`
//! гейта проверяет строку в `docker-compose.yml`, то есть ДОСТАВКУ объявления, а не действие.
//! Класс назван в `gates.md` §4 (DoD «Механизм на пути») и в `testing.md`
//! §«Механизм несущего пути обязан иметь оракул точки входа».
//!
//! # COMPILE-RED, и отдельным файлом — НАМЕРЕННО
//!
//! Оракул ссылается на поле `ServeConfig::max_response_bytes`, которого ЕЩЁ НЕТ. Оставленный
//! в общем наборе, он ронял бы КОМПИЛЯЦИЮ соседей, и те нельзя было бы предъявить красными:
//! «не собралось» и «упало на ассерте» — разные вещи, а RED-first требует второго. Тот же
//! приём и по той же причине, что в `red_egress_cap_boundary.rs`.
//!
//! # Решение architect'а, которое оракул пиннит: предел — ЗНАЧЕНИЕ КОНФИГА, не процессный глобал
//!
//! 1. Глобал невидим системе типов: «никто не читает» не ловится компилятором — именно так
//!    `B-1` и прожил мимо зелёного гейта.
//! 2. При разделяемой проекции (`П-010`, `DESIGN` §16.2 шаг 5) один процесс обслуживает
//!    несколько конфигураций; глобал это закрывает.
//! 3. Значение в конфиге проверяемо без мутации глобального состояния — тесты перестают
//!    влиять друг на друга.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;
const SECRET: &[u8] = b"m71-egress-secret";
const PROD_BAND: f64 = 0.001;
const SHORT_SUB: &str = "s1";

/// Предложенная величина предела (спека §5.1). Она **founder-owned**; оракулы судят
/// ПОВЕДЕНИЕ (сообщение сверх предела наружу не уходит), а число здесь — рабочая отсечка,
/// разведённая с фикстурами на порядки, чтобы набор не зависел от её точного значения.
const PROPOSED_CAP: usize = 2_000_000;

/// Плотный НЕ-heatmap ресурс: 25 000 сделок, ни одного L2-события. Именно он предъявлен
/// критиком как 2 804 666 Б на формах, которых прежние оракулы не покрывали.
const DENSE_TRADES: usize = 25_000;

/// Щедрый бюджет ожидания. Истечение = НЕСОСТОЯВШИЙСЯ SETUP, не вердикт о пределе.
const BUDGET: Duration = Duration::from_secs(20);

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn sign() -> String {
    let claims = Claims {
        sub: "m71".to_string(),
        exp: 9_999_999_999,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET),
    )
    .expect("encode")
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "PL-I-5 wire-level fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![PROD_BAND],
        window_ms: None,
    }
}

fn trade(i: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(MID + i as f64 * 0.01),
            size: to_fixed(1.0),
            side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
            ts_exch_ms: T0 + i,
        },
    )
}

fn journal_of_trades(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..n as i64 {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn config(dir: &Path) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: None,
    }
}

async fn serve_on(dir: &Path) -> String {
    let server = bind(config(dir)).await.expect("bind");
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    addr
}

async fn connect(addr: &str) -> Ws {
    let url = format!("ws://{addr}/?token={}", sign());
    let (ws, _r) = tokio::time::timeout(BUDGET, tokio_tungstenite::connect_async(url))
        .await
        .expect("SETUP: бюджет коннекта истёк")
        .expect("SETUP: connect");
    ws
}

fn subscribe(sub: &str, venue: &str) -> Value {
    json!({
        "op": "subscribe",
        "v": 1,
        "id": sub,
        "selector": {
            "venue": venue,
            "symbol": "BTCUSDT",
            "timeframe_ms": 1000,
            "bands": [PROD_BAND],
        }
    })
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string()))
        .await
        .expect("SETUP: send");
}

/// ОДНО сообщение с сокета: его РАЗМЕР В БАЙТАХ и разобранное тело.
/// `None` — сервер промолчал в пределах бюджета (несостоявшийся setup, не вердикт).
async fn recv(ws: &mut Ws) -> Option<(usize, Value)> {
    match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => {
            let data = m.into_data();
            let n = data.len();
            serde_json::from_slice::<Value>(data.as_ref())
                .ok()
                .map(|v| (n, v))
        }
        _ => None,
    }
}

/// Подписка, ГАРАНТИРОВАННО попавшая в grace-окно, и первый v1-снапшот по ней.
///
/// Повтор обязателен: `subscribe`, опоздавший в grace-окно, сервер игнорирует и уходит в
/// env-режим (`CT-RFC-09` §2.8). Без повтора оракул молча не получал бы v1-сообщений и был
/// бы ЗЕЛЁН ПО ОТСУТСТВИЮ ПРЕДМЕТА — ровно тот класс, который закрывает `A-022`. Тот же
/// приём применён в `red_ws_session.rs:383-405`.
async fn subscribed_snapshot(addr: &str, venue: &str) -> (Ws, usize, Value) {
    let mut last: Option<Value> = None;
    for attempt in 1..=8u64 {
        let mut ws = connect(addr).await;
        send(&mut ws, subscribe(SHORT_SUB, venue)).await;
        match recv(&mut ws).await {
            Some((n, v)) if type_of(&v) == Some("snapshot") => return (ws, n, v),
            Some((_, v)) => {
                last = Some(v);
                let _ = ws.close(None).await;
            }
            None => {
                let _ = ws.close(None).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(10 * attempt)).await;
    }
    setup_failed(&format!(
        "v1-снапшот не получен за 8 попыток попасть в grace-окно; последнее сообщение: {last:?}"
    ))
}

fn type_of(v: &Value) -> Option<&str> {
    v.get("type").and_then(|t| t.as_str())
}

/// Диагностика несостоявшегося setup'а — отдельно от вердикта.
fn setup_failed(what: &str) -> ! {
    panic!("SETUP НЕ СОСТОЯЛСЯ: {what}. Это НЕ вердикт о пределе.")
}

/// Предел, заведомо МЕНЬШЕ честного ответа: если он управляет, ответ обязан быть отвергнут.
/// Замер `R-133`: при `effective = 1000` сервис отдал 224 854 Б.
const TINY_CAP: usize = 1_000;

/// Конфиг с ЯВНО заданным пределом. **COMPILE-RED:** поля `max_response_bytes` в
/// `ServeConfig` ещё нет — в этом и предмет оракула.
fn config_with_cap(dir: &Path, cap: usize) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: None,
        max_response_bytes: cap,
    }
}

async fn serve_with_cap(dir: &Path, cap: usize) -> String {
    let server = bind(config_with_cap(dir, cap))
        .await
        .unwrap_or_else(|e| setup_failed(&format!("bind: {e}")));
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    addr
}

/// **N1 (`R-133` B-1) — заданный в конфиге предел ДЕЙСТВУЕТ.**
///
/// Оператор задаёт крошечный предел; ответ, который при дефолтном пределе обслуживается,
/// обязан быть отвергнут. Оракул судит НАБЛЮДАЕМОЕ ПОВЕДЕНИЕ через сокет, а не наличие
/// геттера: «ручка разобрана» и «ручка управляет» — разные утверждения, и первое уже
/// доказано десятью оракулами старта, которые дефект пропустили.
#[tokio::test]
async fn pl_i_5_n1_configured_cap_actually_governs() {
    let dir = journal_of_trades(200);

    // Контроль внутри оракула: при ЩЕДРОМ пределе тот же запрос обслуживается.
    let generous = serve_with_cap(dir.path(), PROPOSED_CAP).await;
    let (_ws, n_ok, _v) = subscribed_snapshot(&generous, "Binance").await;
    assert!(
        n_ok > TINY_CAP,
        "PL-I-5 N1 SETUP: ответ при щедром пределе весит {n_ok} Б — не больше крошечного \
         предела {TINY_CAP} Б, и различить «предел действует» от «ответ и так мал» нельзя"
    );

    // Предмет: тот же запрос при КРОШЕЧНОМ пределе обязан быть отвергнут.
    let tiny = serve_with_cap(dir.path(), TINY_CAP).await;
    let mut ws = connect(&tiny).await;
    send(&mut ws, subscribe(SHORT_SUB, "Binance")).await;
    let mut served: Option<usize> = None;
    for _ in 0..4 {
        match recv(&mut ws).await {
            Some((n, v)) if type_of(&v) == Some("snapshot") => {
                served = Some(n);
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    assert!(
        served.is_none(),
        "PL-I-5 B-1: при заданном пределе {TINY_CAP} Б клиент получил снапшот на {} Б. \
         Настройка разобрана и не читается никем — работает зашитая в код константа. \
         Оператор, задавший предел, получает поведение, о котором не просил",
        served.unwrap_or(0)
    );
}
