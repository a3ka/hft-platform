//! RED `M-95` (`C-284` B2; sacred, architect-only) — **один каталог на подписку не смеет
//! устареть для провенанса истории: ретеншен, удаливший ранний сегмент ПОСЛЕ построения каталога
//! и ДО расчёта провенанса, обязан дать в снимке `history_truncated = true`** (`VB-I-11`).
//!
//! # Почему
//!
//! Сегодня провенанс считается СВЕЖИМ обходом каталога (`first_visible_seq` →
//! `journal::list_segments`) прямо перед снимком. `M-95` переиспользует каталог, построенный
//! раньше (родословная слепка). Если между ними `retention-prune` удалит самый ранний сегмент,
//! устаревший каталог скажет «история полная» — ложь, против которой заведён `VB-I-11`.
//!
//! # Как судится — детерминированно, точкой остановки (`test_sync::rendezvous`, M-65/M-87)
//!
//! Контракт точки (спека M-95 §3, задачи 1 и 3): после того как подписка ПОСТРОИЛА свой каталог
//! и ДО расчёта провенанса транспорт зовёт `rendezvous::pump_signal_and_wait(<канал>)`:
//! `m95-catalog:<id подписки>` на v1-пути новой подписки, `m95-catalog:legacy` на legacy-пути.
//! Точка есть только в тестовой сборке (`feature = "testing"`), как у M-87. Тест в остановке
//! удаляет самый ранний сегмент (слепок его покрывает — родословная законна, как после
//! `retention-prune`) и отпускает.
//!
//! Парный мир (`f0`) обязателен: без удаления та же подписка НЕ объявляет историю усечённой —
//! иначе прошла бы реализация «всегда `truncated = true`».
//!
//! # Предел, названный честно
//!
//! CI гоняет `cargo test --all` БЕЗ `--features testing`, поэтому этот файл, как и оракулы точек
//! остановки M-87, исполняется только гейтом приёмки (`verify_M-95.sh` — с флагом).
//!
//! # Сегодня — красен: точки `m95-catalog:*` нет, остановка не наступает (по предмету — у
//! реализации нет места, где каталог уже построен, а провенанс ещё не посчитан).
#![cfg(feature = "testing")]

use std::time::Duration;

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use gateway_serve::admission::{AdmissionPolicy, LiveProfile};
use gateway_serve::server::{bind_with_policy, ServeConfig};
use gateway_serve::test_sync::rendezvous;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const SECRET: &[u8] = b"m95-provenance-secret";
const SEVEN: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6];
const BASE_MS: i64 = 1_784_116_800_000;
const BUDGET: Duration = Duration::from_secs(20);

/// Окно ожидания legacy-пути — глобальная настройка процесса (`set_effective_grace_ms`); тесты
/// файла сериализованы, и каждый задаёт окно сам, иначе v1-подписка соседа ушла бы legacy-путём.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1000,
        bands: SEVEN.to_vec(),
        window_ms: Some(60_000),
        depth_cadence_ms: Some(1000),
    }
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 16 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m95".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn append(dir: &std::path::Path, from: u64, n: u64) {
    let mut j = Journal::open_with(dir, writer_cfg()).expect("open_with");
    for i in from..from + n {
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

/// Журнал со сжатыми историческими сегментами, слепок покрывает ВСЁ записанное до него.
fn fixture() -> (tempfile::TempDir, tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("journal");
    append(dir.path(), 0, 3_000);
    journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact");
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    append(dir.path(), 3_000, 40);
    let earliest = dir.path().join("segment-00000000.jrnl.zst");
    assert!(
        earliest.exists(),
        "SETUP НЕ СОСТОЯЛСЯ: самого раннего сжатого сегмента нет — удалять в остановке нечего"
    );
    (dir, ckpt, earliest)
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        allowed_symbols: vec!["BTCUSDT".to_string()],
        canonical_bands: SEVEN.to_vec(),
        allowed_profiles: vec![LiveProfile {
            timeframe_ms: 1000,
            window_ms: 60_000,
            depth_cadence_ms: Some(1000),
        }],
        max_concurrent_serves: 2,
        max_tail_events: 1_000_000,
        expected_warmup_events: 1,
    }
}

fn config(dir: &std::path::Path, ckpt: &std::path::Path) -> ServeConfig {
    ServeConfig {
        addr: "127.0.0.1:0".to_string(),
        journal_dir: dir.to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
        selector: sel(),
        decoding_key: DecodingKey::from_secret(SECRET),
        checkpoint_dir: Some(ckpt.to_path_buf()),
    }
}

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m95".to_string(),
            exp: 4_000_000_000,
        },
        &EncodingKey::from_secret(SECRET),
    )
    .expect("jwt")
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn start(dir: &std::path::Path, ckpt: &std::path::Path) -> String {
    let server = bind_with_policy(config(dir, ckpt), policy())
        .await
        .expect("bind_with_policy");
    let addr = server.local_addr().to_string();
    tokio::spawn(async move {
        let _ = server.serve().await;
    });
    addr
}

async fn connect(addr: &str) -> Ws {
    let url = format!("ws://{addr}/?token={}", sign());
    tokio::time::timeout(BUDGET, tokio_tungstenite::connect_async(url))
        .await
        .expect("connect timeout")
        .expect("connect")
        .0
}

/// Первое сообщение-снимок (v1 `{"type":"snapshot","data":…}` или legacy `{"Snapshot":…}`) →
/// его тело.
async fn snapshot_body(ws: &mut Ws) -> Value {
    let deadline = tokio::time::Instant::now() + BUDGET;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let m = match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(m))) => m,
            other => panic!("SETUP НЕ СОСТОЯЛСЯ: снимок не пришёл ({other:?})"),
        };
        let Ok(v) = serde_json::from_slice::<Value>(m.into_data().as_ref()) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) == Some("snapshot") {
            return v["data"].clone();
        }
        if let Some(b) = v.get("Snapshot") {
            return b.clone();
        }
        if v.get("type").and_then(Value::as_str) == Some("error") {
            panic!("SETUP НЕ СОСТОЯЛСЯ: подписка отвергнута: {v}");
        }
    }
}

fn truncated(body: &Value) -> bool {
    body.get("history_truncated")
        .and_then(Value::as_bool)
        .expect("снимок обязан нести history_truncated (VB-I-11)")
}

struct Guard(String);
impl Drop for Guard {
    fn drop(&mut self) {
        rendezvous::test_release(&self.0);
        rendezvous::test_remove(&self.0);
    }
}

/// Остановиться в точке канала, выполнить `act`, отпустить. Точка обязана наступить.
fn pause_and(ch: &str, act: impl FnOnce()) {
    assert!(
        rendezvous::test_wait_for_pump(ch, BUDGET),
        "M-95 / C-284 B2: точка `{ch}` не наступила за {BUDGET:?} — у реализации нет места «каталог \
         подписки построен, провенанс ещё не посчитан» (контракт точки — спека M-95 §3). Без неё \
         удаление сегмента между двумя наблюдениями не воспроизводится детерминированно"
    );
    act();
    rendezvous::test_release(ch);
}

/// **`f0` — парный мир: без удаления v1-подписка НЕ объявляет историю усечённой.**
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn f0_v1_without_retention_history_is_complete() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, _earliest) = fixture();
    let ch = "m95-catalog:p0".to_string();
    rendezvous::arm(&ch);
    let _g = Guard(ch.clone());
    let addr = start(dir.path(), ckpt.path()).await;
    let mut ws = connect(&addr).await;
    ws.send(Message::Text(
        json!({"op":"subscribe","v":1,"id":"p0","selector":{
            "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
            "bands":SEVEN.to_vec(),"window_ms":60000,"depth_cadence_ms":1000}})
        .to_string(),
    ))
    .await
    .expect("send");
    let chc = ch.clone();
    tokio::task::spawn_blocking(move || pause_and(&chc, || {}))
        .await
        .expect("pause");
    let body = snapshot_body(&mut ws).await;
    assert!(
        !truncated(&body),
        "M-95 I-4: на ЦЕЛОМ журнале снимок объявил историю усечённой — реализация «всегда \
         truncated=true» прошла бы f1 без проверки свежести: {body}"
    );
}

/// **`f1` — v1: ретеншен удалил самый ранний сегмент между каталогом и провенансом ⇒ снимок
/// объявляет историю усечённой, `history_start_seq > 0`.**
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn f1_v1_retention_between_catalog_and_provenance_is_honest() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, earliest) = fixture();
    let ch = "m95-catalog:s1".to_string();
    rendezvous::arm(&ch);
    let _g = Guard(ch.clone());
    let addr = start(dir.path(), ckpt.path()).await;
    let mut ws = connect(&addr).await;
    ws.send(Message::Text(
        json!({"op":"subscribe","v":1,"id":"s1","selector":{
            "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
            "bands":SEVEN.to_vec(),"window_ms":60000,"depth_cadence_ms":1000}})
        .to_string(),
    ))
    .await
    .expect("send");
    let chc = ch.clone();
    let e = earliest.clone();
    tokio::task::spawn_blocking(move || {
        pause_and(&chc, move || {
            std::fs::remove_file(&e).expect("удаление раннего сегмента (как retention-prune)")
        })
    })
    .await
    .expect("pause");
    let body = snapshot_body(&mut ws).await;
    assert!(
        truncated(&body),
        "M-95 / C-284 B2 / VB-I-11: ранний сегмент удалён ПОСЛЕ построения каталога подписки и ДО \
         провенанса, а снимок говорит «история полная» — провенанс посчитан по устаревшему \
         каталогу. Обязан: проверка свежести (is_fresh) и пересчёт, либо (frozen, true): {body}"
    );
    let start_seq = body
        .get("history_start_seq")
        .and_then(Value::as_u64)
        .expect("history_start_seq");
    assert!(
        start_seq > 0,
        "M-95 / VB-I-11: history_start_seq = 0 после удаления префикса: {body}"
    );
}

/// **`f2` — legacy-путь (клиент молчит окно ожидания): то же требование, канал
/// `m95-catalog:legacy`.**
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn f2_legacy_retention_between_catalog_and_provenance_is_honest() {
    let _serial = SERIAL.lock().await;
    let (dir, ckpt, earliest) = fixture();
    gateway_serve::server::set_effective_grace_ms(200);
    let ch = "m95-catalog:legacy".to_string();
    rendezvous::arm(&ch);
    let _g = Guard(ch.clone());
    let addr = start(dir.path(), ckpt.path()).await;
    let mut ws = connect(&addr).await;
    let chc = ch.clone();
    let e = earliest.clone();
    tokio::task::spawn_blocking(move || {
        pause_and(&chc, move || {
            std::fs::remove_file(&e).expect("удаление раннего сегмента (как retention-prune)")
        })
    })
    .await
    .expect("pause");
    let body = snapshot_body(&mut ws).await;
    assert!(
        truncated(&body),
        "M-95 / C-284 B2 / VB-I-11: legacy-путь — ранний сегмент удалён между каталогом и \
         провенансом, снимок говорит «история полная»: {body}"
    );
}
