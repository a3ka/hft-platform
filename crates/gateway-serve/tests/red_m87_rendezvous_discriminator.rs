//! RED M-87 круг `C-245` (sacred, architect-only) — **РАЗЛИЧИТЕЛЬ ПОДМЕННОГО ПУТИ.**
//!
//! Гейт `C-245` R2 нашёл дыру, из-за которой `C4` мог позеленеть против ЧУЖОГО механизма.
//! `C4` держит ADD-работу точкой рандеву; но точно такая же механика уже живёт в дереве —
//! периодический v1-pump из M-65 (`lib.rs:1495`, `:2038`), и первая редакция `C4` ждала
//! РОВНО ТОТ ЖЕ голый идентификатор подписки. Дев, добавив один аддитивный доступ, получил
//! бы зелёный `C4`, не поставив требуемой точки на ADD-пути вовсе: сигнал пришёл бы от
//! старого pump'а, а занятый слот обеспечился бы другим путём.
//!
//! Закрыто ДВУМЯ разными средствами, и ни одного из них по отдельности не хватает:
//!  · **именем** — канал ADD-пути несёт префикс `m87-add:` и не может быть разбужен
//!    pump'ом, который пишет в канал `<sub>` (структурно, `red_m87_entrypoint.rs`);
//!  · **наблюдением** — ЭТОТ файл.
//!
//! **Почему отдельный бинарь, а не ассерт внутри `C4`.** `C4` красен по компиляции до
//! задачи 13 (`slots_handle` ещё нет), и пока он красен, любое утверждение ВНУТРИ него
//! непроверяемо. В Rust каждый файл `tests/` — отдельный бинарник, поэтому здешний
//! контроль работает УЖЕ СЕГОДНЯ и сторожит разницу двух механизмов всё время, пока
//! задача 13 не сделана, — и после того, как она сделана.
//!
//! **Этот контроль обязан быть ЗЕЛЁНЫМ.** Он краснеет ровно тогда, когда удержание
//! ADD-работы завязано на чужой канал.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::auth::Claims;
use gateway_serve::server::{bind_with_policy, ServeConfig};
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m87-entrypoint-secret";
const BASE_MS: i64 = 1_784_116_800_000;
const BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
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

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Снять канал на любом выходе, включая панику.
struct RendezvousGuard(String);

impl Drop for RendezvousGuard {
    fn drop(&mut self) {
        gateway_serve::test_sync::rendezvous::test_release(&self.0);
        gateway_serve::test_sync::rendezvous::test_remove(&self.0);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn periodic_pump_rendezvous_does_not_hold_the_add_work() {
    use gateway_serve::test_sync::rendezvous;

    let dir = journal_busy(300);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(
        dir.path(),
        ckpt.path(),
        &canonical_sel(),
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");

    // ВЗВОДИМ ГОЛЫЙ идентификатор — канал ПЕРИОДИЧЕСКОГО pump'а M-65, НЕ канал ADD-пути.
    rendezvous::arm("a");
    let _rv = RendezvousGuard("a".to_string());

    let server = bind_with_policy(
        config(dir.path(), Some(ckpt.path().to_path_buf())),
        policy(),
    )
    .await
    .expect("bind_with_policy");
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ckpt_guard = ckpt;
        let _ = server.serve().await;
    });

    let mut a = connect(&addr).await;
    send(&mut a, subscribe("a", canonical_selector_json())).await;

    let first = recv(&mut a)
        .await
        .expect("первый клиент не получил ответа вовсе — выдача встала на ЧУЖОМ канале");
    assert_eq!(
        first.get("type").and_then(|t| t.as_str()),
        Some("snapshot"),
        "взведён канал ПЕРИОДИЧЕСКОГО pump'а (M-65), а первая выдача встала: {first}. \
         Значит удержание ADD-работы завязано на чужой механизм, и C4 зеленел бы, не \
         получив требуемой точки на ADD-пути вовсе (C-245 R2)"
    );
}
