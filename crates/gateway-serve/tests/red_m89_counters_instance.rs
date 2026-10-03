//! RED `M-89` (sacred, architect-only) — **счётчики выдачи принадлежат ЭКЗЕМПЛЯРУ сервера,
//! а не процессу** (`I-5`, `TD-224`).
//!
//! # Дефект, который это закрывает
//!
//! `journal_payload_bytes_read` и прочие счётчики живут в процессных атомиках
//! (`JOURNAL_BYTES_GLOBAL` и соседи, `crates/gateway-serve/src/metrics.rs`). Тесты одного
//! файла — один процесс: сценарий, законно читающий журнал, двигает счётчик в окне ЧУЖОГО
//! утверждения. `c1`/`c5` («до == после») тогда краснеют ложно (CI `main` после merge #227,
//! `R-206`), а `c6` и позитивный контроль («после > до») ложно ЗЕЛЕНЕЮТ на заимствованном
//! приросте — и этого не видно никаким прогоном. Лечение симптома — `SERIAL` в
//! `red_m87_entrypoint.rs` (14 тестов идут по очереди, 2.36 с → 6.0 с): оракул получил
//! требование к ПОРЯДКУ ИСПОЛНЕНИЯ, которого у правильного оракула быть не должно
//! (`testing.md`, св. 2 целостности гейта).
//!
//! # Форма (спека §4.1 `M-89`, задана ДОСЛОВНО)
//!
//! ```ignore
//! impl Server {
//!     /// АДДИТИВНО, по образцу `slots_handle` (задача 13 `M-87`): ручка счётчиков ЭТОГО
//!     /// экземпляра. `None` ⇒ сервер поднят без политики.
//!     pub fn counters_handle(&self) -> Option<std::sync::Arc<ServingCountersHandle>>;
//! }
//! pub struct ServingCountersHandle { /* атомики экземпляра */ }
//! impl ServingCountersHandle { pub fn snapshot(&self) -> ServingCounters; }
//! ```
//!
//! Процессный `serving_counters()` МОЖЕТ остаться агрегатом по всем экземплярам — здесь он
//! не судится и не запрещается.
//!
//! # Что здесь пиннится
//!
//! · `i1` — два сервера в одном процессе: запросы к A не двигают счётчики B (изоляция);
//! · `i2` — мутационный контроль формы: ручка, возвращающая ПРОЦЕССНЫЙ счётчик под видом
//!   экземплярного, краснеет на `i1` (B увидел бы прирост A). Это и есть условие, при
//!   котором `SERIAL` в `red_m87_entrypoint.rs` снимается (спека §Tasks, задача 7).
//!
//! COMPILE-RED на ревизии набора: `Server::counters_handle` не существует.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::server::{bind_with_policy, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m89-counters-instance";
const FUTURE: usize = 4_000_000_000;
const BASE_MS: i64 = 1_784_116_800_000;
const BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
const CANONICAL: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m89".to_string(),
            exp: FUTURE,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt")
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 30,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m89".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
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
        max_concurrent_serves: 2,
        max_tail_events: 1_000,
        expected_warmup_events: 100,
    }
}

/// Журнал + тёплый слепок + хвост: запрос к серверу ДОПУСКАЕТСЯ и ЧИТАЕТ хвост, то есть
/// двигает все счётчики, включая байты.
fn warm_fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        for i in 0..300u64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + (i % 20) as f64),
                    size: to_fixed(0.5),
                    side: if i.is_multiple_of(2) {
                        Side::Buy
                    } else {
                        Side::Sell
                    },
                    ts_exch_ms: BASE_MS + i as i64 * 100,
                },
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("reopen");
        for i in 300..340u64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + (i % 20) as f64),
                    size: to_fixed(0.5),
                    side: Side::Buy,
                    ts_exch_ms: BASE_MS + i as i64 * 100,
                },
            ))
            .expect("append tail");
        }
        j.flush().expect("flush");
    }
    (dir, ckpt)
}

async fn serve(
    dir: &std::path::Path,
    ckpt: &std::path::Path,
) -> (
    String,
    std::sync::Arc<gateway_serve::metrics::ServingCountersHandle>,
) {
    let cfg = ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(ckpt.to_path_buf()),
    };
    let server = bind_with_policy(cfg, policy())
        .await
        .expect("bind_with_policy");
    let addr = server.local_addr().to_string();
    let counters = server
        .counters_handle()
        .expect("bind_with_policy обязан дать ручку счётчиков экземпляра");
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    (addr, counters)
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn subscribe_and_wait_frame(addr: &str) {
    let url = format!("ws://{addr}/?token={}", sign());
    let (mut ws, _): (Ws, _) = tokio::time::timeout(BUDGET, tokio_tungstenite::connect_async(url))
        .await
        .expect("connect timeout")
        .expect("connect");
    let sub = json!({"op":"subscribe","v":1,"id":"s1","selector":{
        "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
        "bands":CANONICAL.to_vec(),"window_ms":60000}});
    ws.send(Message::Text(sub.to_string())).await.expect("send");
    let mut seen_snapshot = false;
    for _ in 0..8 {
        let m: Option<Value> = match tokio::time::timeout(BUDGET, ws.next()).await {
            Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref()).ok(),
            _ => None,
        };
        match m
            .as_ref()
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str)
        {
            Some("snapshot") => seen_snapshot = true,
            Some("frame") if seen_snapshot => return,
            Some(_) => continue,
            None => break,
        }
    }
    panic!("SETUP НЕ СОСТОЯЛСЯ: snapshot и первый frame не получены — запрос не обслужен");
}

/// **i1 — изоляция экземпляров.** Два сервера в одном процессе; запросы идут ТОЛЬКО к A.
/// Счётчики B обязаны остаться на нуле — иначе ручка отдаёт процессное состояние.
#[tokio::test]
async fn i1_requests_to_one_server_do_not_move_counters_of_another() {
    let (dir_a, ckpt_a) = warm_fixture();
    let (dir_b, ckpt_b) = warm_fixture();
    let (addr_a, counters_a) = serve(dir_a.path(), ckpt_a.path()).await;
    let (_addr_b, counters_b) = serve(dir_b.path(), ckpt_b.path()).await;

    let a0 = counters_a.snapshot();
    let b0 = counters_b.snapshot();
    assert_eq!(a0.attempts, 0, "свежий экземпляр A уже несёт попытки");
    assert_eq!(b0.attempts, 0, "свежий экземпляр B уже несёт попытки");

    subscribe_and_wait_frame(&addr_a).await;
    subscribe_and_wait_frame(&addr_a).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let a1 = counters_a.snapshot();
    let b1 = counters_b.snapshot();
    assert!(
        a1.attempts >= 2 && a1.successes >= 2,
        "A обслужил два запроса, а счётчики A: attempts={} successes={}",
        a1.attempts,
        a1.successes
    );
    assert!(
        a1.journal_payload_bytes_read > 0,
        "A прочитал слепок и хвост, а байты экземпляра A = 0"
    );
    assert_eq!(
        (b1.attempts, b1.successes, b1.journal_payload_bytes_read),
        (0, 0, 0),
        "I-5 / TD-224: запросы к A сдвинули счётчики B (attempts={} successes={} bytes={}) — \
         ручка отдаёт ПРОЦЕССНОЕ состояние под видом экземплярного. Именно это делает \
         оракулы `red_m87_entrypoint.rs` зависимыми от порядка исполнения и заставляет \
         держать `SERIAL`.",
        b1.attempts,
        b1.successes,
        b1.journal_payload_bytes_read
    );
}

/// **i2 — экземплярная ручка — тот же продюсер, что и путь выдачи, а не копия.** Значения
/// растут монотонно между двумя снимками одной ручки после запроса; `slots_in_flight`
/// возвращается к нулю по завершении работы (парный vantage: ручка, всегда отдающая нули,
/// проходит `i1` для B — и падает здесь для A).
#[tokio::test]
async fn i2_instance_handle_is_fed_by_the_real_serving_path() {
    let (dir, ckpt) = warm_fixture();
    let (addr, counters) = serve(dir.path(), ckpt.path()).await;
    let before = counters.snapshot();
    subscribe_and_wait_frame(&addr).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let after = counters.snapshot();
    assert!(after.attempts > before.attempts, "попытки не выросли");
    assert!(after.successes > before.successes, "успехи не выросли");
    assert!(
        after.journal_payload_bytes_read > before.journal_payload_bytes_read,
        "байты не выросли — экземплярная ручка не кормится путём выдачи"
    );
    assert_eq!(after.slots_in_flight, 0, "слот не освобождён после работы");
}
