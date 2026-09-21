//! RED M-87 (sacred, architect-only) — ОРАКУЛ ТОЧКИ ВХОДА: предохранитель стоит на
//! РЕАЛЬНОМ публичном пути, а не рядом с ним.
//!
//! ## Почему этот файл существует (`C-234` R2)
//!
//! Первая редакция набора звала `gateway::LiveReducer::resume` НАПРЯМУЮ и называла это
//! «живым путём». Настоящий публичный путь другой: `handle_v1_message` после
//! `session::validate_selector` (`crates/gateway-serve/src/lib.rs:810-814`) НЕМЕДЛЕННО уходит
//! в `spawn_blocking` с `LiveReducer::resume` — на ветке SWITCH (`:842-849`) и на ветке ADD
//! (`:938-944`). Оракул, обходящий эту точку, не доказывает, что защита к ней подключена:
//! dev мог бы не подключить допуск вовсе — и набор остался бы зелёным.
//!
//! Следствие второе, названное гейтом и принятое: GREEN можно получить и НЕВЕРНО — гвардом
//! внутри общего `gateway::validate_selector`/`LiveReducer`. Это задело бы offline-путь,
//! чекпоинтер и реплей, то есть воспроизвело конструкцию `M-84`, признанную негодной
//! арбитражем `A-033` (209 красных из 329, поломка прод-формы argv). Поэтому запрет на такую
//! правку — в запретном списке спеки §10, а здесь судится ИМЕННО транспорт.
//!
//! ## Чем доказывается «журнал не читался» на точке входа
//!
//! Счётчиком ПРОЧИТАННЫХ БАЙТ, наблюдаемым СНАРУЖИ (`ServingCounters::journal_payload_bytes_read`).
//! Счётчик декодированных событий недостаточен по плану §15.1: сегмент можно открыть,
//! прочитать и распаковать, не вызвав декодер. Счётчик снимается ДО и ПОСЛЕ запроса —
//! то есть судится ПРОДЮСЕР на реальном пути, как требует `OPS-I-10`
//! (`docs/fa/ops.md:478`: «объявлена ⟹ эмитится»).
//!
//! COMPILE-RED: модулей `gateway_serve::admission` / `gateway_serve::metrics` не существует.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::auth::Claims;
use gateway_serve::metrics::{freshness, serving_counters};
use gateway_serve::server::{bind, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m87-entrypoint-secret";
const BASE_MS: i64 = 1_784_116_800_000;
const BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
/// Канонические семь полос (`П-029`).
const CANONICAL: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

fn sign() -> String {
    encode(
        &Header::default(),
        &Claims {
            sub: "m87".to_string(),
            exp: 9_999_999_999,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("encode")
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 64 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m87".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// Журнал прод-подобной длины: пересчёт по нему ДОЛЖЕН быть заметно дороже отказа.
fn journal_busy(events: i64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![lvl(65_000.0, 2.0)],
                asks: vec![lvl(65_010.0, 1.5)],
                ts_exch_ms: BASE_MS,
            },
        ))
        .expect("snap");
        for i in 0..events {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + (i % 20) as f64),
                    size: to_fixed(0.5),
                    side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    ts_exch_ms: BASE_MS + i * 100,
                },
            ))
            .expect("trade");
        }
        j.flush().expect("flush");
    }
    dir
}

fn canonical_sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: CANONICAL.to_vec(),
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        allowed_symbols: vec!["BTCUSDT".to_string()],
        canonical_bands: CANONICAL.to_vec(),
        allowed_profiles: vec![LiveProfile {
            timeframe_ms: 1_000,
            window_ms: 60_000,
            depth_cadence_ms: None,
        }],
        max_concurrent_serves: 1,
    }
}

fn config(dir: &std::path::Path, ckpt: Option<std::path::PathBuf>) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: canonical_sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: ckpt,
    }
}

async fn serve(dir: &std::path::Path, ckpt: Option<std::path::PathBuf>) -> String {
    let server = bind(config(dir, ckpt)).await.expect("bind");
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    addr
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(addr: &str) -> Ws {
    let url = format!("ws://{addr}/?token={}", sign());
    let (ws, _) = tokio::time::timeout(BUDGET, tokio_tungstenite::connect_async(url))
        .await
        .expect("connect timeout")
        .expect("connect");
    ws
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string())).await.expect("send");
}

async fn recv(ws: &mut Ws) -> Option<Value> {
    match tokio::time::timeout(BUDGET, ws.next()).await {
        Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref()).ok(),
        _ => None,
    }
}

fn subscribe(id: &str, sel: Value) -> Value {
    json!({"op":"subscribe","v":1,"id":id,"selector":sel})
}

fn canonical_selector_json() -> Value {
    json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
           "bands":CANONICAL.to_vec(),"window_ms":60000})
}

// ═════════════ C1/C2 — точка входа отвечает ИСХОДОМ, не пересчётом ═════════════

/// Холодный публичный запрос: слепка нет. Сервер обязан ответить НАЗВАННЫМ исходом,
/// и журнал при этом читаться НЕ ДОЛЖЕН — проверяется счётчиком, снятым до и после.
#[tokio::test]
async fn c1_entry_cold_request_named_outcome_without_reading_journal() {
    let dir = journal_busy(3_000);
    let addr = serve(dir.path(), None).await;

    let before = serving_counters().journal_payload_bytes_read;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал на подписку");

    assert_eq!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("error"),
        "холодный запрос обслужен вместо отказа: {msg}"
    );
    let code = msg.get("code").and_then(|c| c.as_str()).unwrap_or("");
    assert!(
        matches!(code, "not_ready" | "warming"),
        "исход обязан быть НАЗВАН ('not_ready'/'warming'), получено '{code}'"
    );

    let after = serving_counters().journal_payload_bytes_read;
    assert_eq!(
        after,
        before,
        "публичный путь прочитал {} байт журнала при неготовом состоянии — предохранитель \
         не стоит между валидацией селектора и spawn_blocking",
        after - before
    );
}

/// АНТИ-ПЛАЦЕБО: реализация «всегда отказывать» проходит тест выше. Здесь она обязана упасть.
#[tokio::test]
async fn c1_entry_ready_state_is_actually_served() {
    let dir = journal_busy(500);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");

    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");

    assert_eq!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "готовое состояние не обслужено: {msg}. 'Отказывать всегда' решением не является"
    );
}

// ═════════════ C5 — политика допуска на РЕАЛЬНОМ входе ═════════════

/// **Дыра, найденная гейтом (`C-234` R1).** `window_ms: null` — это unbounded offline-свёртка
/// (`crates/gateway/src/lib.rs:287-305`), и общий валидатор её ПРОПУСКАЕТ. Политика,
/// проверяющая только символ и полосы, такой запрос отклонить не способна.
#[tokio::test]
async fn c5_entry_unbounded_profile_is_refused() {
    let dir = journal_busy(3_000);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;

    let before = serving_counters().journal_payload_bytes_read;
    let mut ws = connect(&addr).await;
    // Символ и полосы КАНОНИЧЕСКИЕ, профиль — неограниченный.
    send(
        &mut ws,
        subscribe(
            "s1",
            json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1,
                   "bands":CANONICAL.to_vec(),"window_ms":null}),
        ),
    )
    .await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");

    assert_eq!(
        msg.get("code").and_then(|c| c.as_str()),
        Some("unsupported"),
        "неограниченный профиль принят: {msg}. Символ и полосы разрешены, но окно не \
         ограничено — это публичный путь к полной свёртке истории"
    );
    assert_eq!(
        serving_counters().journal_payload_bytes_read,
        before,
        "отказ наступил ПОСЛЕ чтения журнала — работа уже оплачена"
    );
}

/// Параметры НЕ подменяются молча на поддержанные (`CT-RFC-09` §2.7).
#[tokio::test]
async fn c5_entry_refusal_does_not_substitute_parameters() {
    let dir = journal_busy(200);
    let addr = serve(dir.path(), None).await;
    let mut ws = connect(&addr).await;
    send(
        &mut ws,
        subscribe(
            "s1",
            json!({"venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
                   "bands":[0.0123,0.4567],"window_ms":60000}),
        ),
    )
    .await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    assert_eq!(
        msg.get("code").and_then(|c| c.as_str()),
        Some("unsupported"),
        "неканонический набор полос принят: {msg}"
    );
    assert_ne!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "вместо отказа пришёл снимок — параметры подменены молча"
    );
}

/// Соседняя подписка и соединение остаются живыми (`CT-RFC-09` §2.7).
#[tokio::test]
async fn c5_entry_refusal_keeps_connection_and_neighbours_alive() {
    let dir = journal_busy(500);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;

    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("good", canonical_selector_json())).await;
    let first = recv(&mut ws).await.expect("нет ответа на годную подписку");
    assert_eq!(first.get("type").and_then(|t| t.as_str()), Some("snapshot"));

    send(
        &mut ws,
        subscribe(
            "bad",
            json!({"venue":"Binance","symbol":"DOGEUSDT","timeframe_ms":1000,
                                "bands":CANONICAL.to_vec(),"window_ms":60000}),
        ),
    )
    .await;
    let refused = recv(&mut ws).await.expect("соединение закрыто отказом");
    assert_eq!(
        refused.get("code").and_then(|c| c.as_str()),
        Some("unsupported"),
        "неразрешённый инструмент принят: {refused}"
    );
    assert_eq!(
        refused.get("sub").and_then(|x| x.as_str()),
        Some("bad"),
        "отказ обязан быть адресован СВОЕЙ подписке, а не соединению"
    );
}

// ═════════════ C4 — слот НЕ освобождается по таймауту ОЖИДАНИЯ ═════════════

/// При `max_concurrent_serves = 1` вторая одновременная работа обязана получить
/// `overloaded`, а занятый слот — оставаться занятым, пока работа не завершилась.
/// Освобождение слота по таймауту ОТВЕТА даёт неограниченное число параллельных расчётов
/// при формально ограниченном параллелизме.
#[tokio::test]
async fn c4_slot_is_held_until_work_ends_not_until_response_timeout() {
    let dir = journal_busy(20_000);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;

    let mut a = connect(&addr).await;
    let mut b = connect(&addr).await;
    send(&mut a, subscribe("a", canonical_selector_json())).await;
    // Второй клиент просит, пока первый ещё обслуживается.
    send(&mut b, subscribe("b", canonical_selector_json())).await;

    let in_flight = serving_counters().slots_in_flight;
    assert!(
        in_flight >= 1,
        "слот не занят во время работы (slots_in_flight={in_flight}) — ограничитель \
         не наблюдаем, и его наличие нечем предъявить"
    );

    let mb = recv(&mut b).await.expect("второй клиент не получил ответа");
    let code = mb.get("code").and_then(|c| c.as_str()).unwrap_or("");
    assert!(
        matches!(code, "overloaded") || mb.get("type").and_then(|t| t.as_str()) == Some("snapshot"),
        "второй запрос при занятом единственном слоте обязан получить 'overloaded' \
         либо дождаться слота, а не запустить параллельный расчёт: {mb}"
    );
    let _ = recv(&mut a).await;
}

// ═════════════ C6 — счётчики ЭМИТЯТСЯ продюсером на реальном пути ═════════════

/// `OPS-I-10` (`docs/fa/ops.md:478`): объявления мало — метрика обязана РЕАЛЬНО
/// инкрементироваться продюсером. Первая редакция набора конструировала `ServingCounters`
/// литералом и судила чистую функцию правила тревоги; гейт это отклонил (`C-234` R3).
#[tokio::test]
async fn c6_counters_are_emitted_by_the_real_serving_path() {
    let dir = journal_busy(300);
    let addr = serve(dir.path(), None).await;

    let before = serving_counters();
    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let _ = recv(&mut ws).await;
    send(
        &mut ws,
        subscribe(
            "s2",
            json!({"venue":"Binance","symbol":"DOGEUSDT","timeframe_ms":1000,
                               "bands":CANONICAL.to_vec(),"window_ms":60000}),
        ),
    )
    .await;
    let _ = recv(&mut ws).await;
    let after = serving_counters();

    assert!(
        after.attempts > before.attempts,
        "счётчик попыток не вырос после ДВУХ реальных запросов — продюсера нет, метрика мертва"
    );
    assert!(
        after.refusals_supported > before.refusals_supported,
        "отказ по неготовности не посчитан как отказ ПОДДЕРЖАННОГО запроса"
    );
    assert!(
        after.refusals_unsupported > before.refusals_unsupported,
        "отказ неразрешённому инструменту не посчитан ОТДЕЛЬНО — поток мусора будет \
         держать тревогу включённой"
    );
}

// ═════════════ C9 — четыре позиции свежести ═════════════

/// Позиции обязаны быть РАЗЛИЧИМЫ, и отставание СЛЕПКА не имеет права останавливать
/// исправную живую выдачу: слепок — кэш, а не источник истины.
#[tokio::test]
async fn c9_stale_snapshot_does_not_stop_a_healthy_live_path() {
    let dir = journal_busy(500);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    let addr = serve(dir.path(), Some(ckpt.path().to_path_buf())).await;

    let mut ws = connect(&addr).await;
    send(&mut ws, subscribe("s1", canonical_selector_json())).await;
    let msg = recv(&mut ws).await.expect("сервер промолчал");
    assert_eq!(
        msg.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "исправная выдача остановлена: {msg}"
    );

    let f = freshness();
    assert!(
        f.source_ms > 0 && f.projection_ms > 0 && f.published_ms > 0 && f.snapshot_ms > 0,
        "четыре позиции свежести обязаны быть заполнены: {:?}",
        (f.source_ms, f.projection_ms, f.published_ms, f.snapshot_ms)
    );
    assert!(
        f.published_ms >= f.snapshot_ms,
        "опубликованные данные не могут отставать от слепка: слепок пишется ИЗ них, \
         а не наоборот"
    );
}
