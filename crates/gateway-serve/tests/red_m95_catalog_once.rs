//! RED `M-95` (sacred, architect-only) — **одна подписка обходит каталог журнала ОДИН раз**
//! (`TD-229`; спека `docs/archive/M-95-catalog-once.md`).
//!
//! # Дефект — замер (`R-211` §4.2 на проде; клон architect'а 2026-10-05 на фикстуре)
//!
//! На пути ОДНОЙ подписки каталог сегментов строится ТРИЖДЫ — каждый раз с открытием каждого
//! сегмента и распаковкой первого блока каждого сжатого (`.zst`): (1) `validate_lineage` при
//! чтении слепка (`LiveReducer::resume`), (2) `first_visible_seq` в
//! `history_provenance_for_serve`, (3) `SegmentCatalog::open` в первом `pump` сессии. На проде
//! это 3 × ~500 сегментов × 128 КиБ ≈ 192 МБ на подписку, растущие с ДЛИНОЙ ИСТОРИИ.
//!
//! # Как судится — граница процесса, без новых символов (урок `A-033`)
//!
//! Прод-бинарь `gateway-serve` (окружение задано явно, режим без профиля — он сохраняется после
//! `M-94`); открытия файлов каталога журнала считает `inotify` (`IN_OPEN`) через Python `ctypes`
//! — `strace` не нужен. Мера — открытия КАЖДОГО `.zst` за одну подписку: построение каталога
//! открывает каждый сжатый сегмент ровно один раз (`classify_compacted_segment`), а чтение хвоста
//! сжатых сегментов не касается (хвост — в сыром активном). Равенство, а не «≤»: ноль открытий
//! значит, что наблюдатель слеп, а не что каталог не строился.
//!
//! Setup-страж первого шага чтения: после снимка тест дописывает события и ЖДЁТ кадр — значит
//! `pump` (третий обход сегодня) состоялся внутри окна наблюдения.
//!
//! # Сегодня (`origin/main` `6ee3f1a`) — красен по предмету: 3 открытия на `.zst` вместо 1.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const BASE_MS: i64 = 1_784_116_800_000;
const SEVEN: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6];
const SECRET: &str = "m95-secret";

const WATCHER: &str = r#"
import ctypes, os, struct, sys, select
libc = ctypes.CDLL("libc.so.6", use_errno=True)
d, out, ready = sys.argv[1], sys.argv[2], sys.argv[3]
fd = libc.inotify_init1(0)
if fd < 0: sys.exit("inotify_init1 failed")
if libc.inotify_add_watch(fd, d.encode(), 0x20) < 0: sys.exit("inotify_add_watch failed")
f = open(out, "w", buffering=1)
open(ready, "w").close()
while True:
    r, _, _ = select.select([fd], [], [], 0.1)
    if not r:
        if os.path.exists(ready + ".stop"): break
        continue
    buf = os.read(fd, 65536); i = 0
    while i < len(buf):
        wd, mask, cookie, ln = struct.unpack_from("iIII", buf, i)
        f.write(buf[i+16:i+16+ln].rstrip(b"\0").decode() + "\n"); i += 16 + ln
"#;

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

fn append(dir: &Path, from: u64, n: u64) {
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

/// Журнал: несколько СЖАТЫХ исторических сегментов + сырой активный; тёплый слепок селектора.
fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Vec<String>) {
    let dir = tempfile::tempdir().expect("journal");
    append(dir.path(), 0, 3_000);
    journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact_closed_segments");
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    append(dir.path(), 3_000, 40);
    let mut zst: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".zst"))
        .collect();
    zst.sort();
    assert!(
        zst.len() >= 4,
        "SETUP НЕ СОСТОЯЛСЯ: сжатых сегментов {} (нужно ≥ 4) — мера открытий ни о чём",
        zst.len()
    );
    (dir, ckpt, zst)
}

fn bin_path() -> PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    exe.parent()
        .and_then(Path::parent)
        .expect("target/<profile>")
        .join("gateway-serve")
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_serve(journal: &Path, ckpt: &Path, port: u16, grace_ms: u64) -> Child {
    let bin = bin_path();
    assert!(
        bin.exists(),
        "SETUP-СТРАЖ: прод-бинарь не собран ({})",
        bin.display()
    );
    let env: BTreeMap<&str, String> = BTreeMap::from([
        ("GATEWAY_JWT_SECRET", SECRET.to_string()),
        ("GATEWAY_ADDR", format!("127.0.0.1:{port}")),
        ("GATEWAY_JOURNAL_DIR", journal.display().to_string()),
        ("GATEWAY_CHECKPOINT_DIR", ckpt.display().to_string()),
        ("GATEWAY_VENUE", "Binance".to_string()),
        ("GATEWAY_SYMBOL", "BTCUSDT".to_string()),
        ("GATEWAY_TIMEFRAME_MS", "1000".to_string()),
        (
            "GATEWAY_BANDS",
            SEVEN
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        ("GATEWAY_WINDOW_MS", "60000".to_string()),
        ("GATEWAY_DEPTH_CADENCE_MS", "1000".to_string()),
        ("GATEWAY_ALLOWED_SYMBOLS", "BTCUSDT".to_string()),
        ("GATEWAY_ALLOWED_PROFILES", "1000/60000/1000".to_string()),
        ("GATEWAY_MAX_CONCURRENT_SERVES", "4".to_string()),
        ("GATEWAY_MAX_TAIL_EVENTS", "1000000".to_string()),
        ("GATEWAY_EXPECTED_WARMUP_EVENTS", "1000".to_string()),
        ("GATEWAY_INITIAL_SUBSCRIBE_GRACE_MS", grace_ms.to_string()),
    ]);
    let mut cmd = Command::new(&bin);
    cmd.env_clear().env("PATH", "/usr/bin:/bin");
    for (k, v) in &env {
        cmd.env(k, v);
    }
    Child(
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("запуск прод-бинаря"),
    )
}

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn token() -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m95".to_string(),
            exp: 4_000_000_000,
        },
        &EncodingKey::from_secret(&gateway_serve::auth::key_material(SECRET)),
    )
    .expect("jwt")
}

/// Наблюдатель открытий каталога журнала (`inotify IN_OPEN`).
struct Watcher {
    child: std::process::Child,
    stopped: bool,
    out: PathBuf,
    ready: PathBuf,
    _tmp: tempfile::TempDir,
}
impl Watcher {
    fn start(dir: &Path) -> Self {
        let tmp = tempfile::tempdir().expect("watcher tmp");
        let script = tmp.path().join("w.py");
        std::fs::write(&script, WATCHER).unwrap();
        let (out, ready) = (tmp.path().join("opens"), tmp.path().join("ready"));
        let child = Command::new("python3")
            .arg(&script)
            .arg(dir)
            .arg(&out)
            .arg(&ready)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("SETUP: python3 недоступен");
        let t0 = std::time::Instant::now();
        while !ready.exists() {
            assert!(
                t0.elapsed() < Duration::from_secs(10),
                "SETUP НЕ СОСТОЯЛСЯ: наблюдатель inotify не поднялся"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        Watcher {
            child,
            stopped: false,
            out,
            ready,
            _tmp: tmp,
        }
    }
    /// Остановить и вернуть число открытий по имени файла.
    fn stop(mut self) -> BTreeMap<String, usize> {
        std::thread::sleep(Duration::from_millis(300)); // дочитать очередь событий ядра
        std::fs::write(PathBuf::from(format!("{}.stop", self.ready.display())), b"").unwrap();
        let _ = self.child.wait();
        self.stopped = true;
        let mut m = BTreeMap::new();
        for l in std::fs::read_to_string(&self.out)
            .unwrap_or_default()
            .lines()
        {
            *m.entry(l.to_string()).or_insert(0) += 1;
        }
        m
    }
}

/// Упавший тест не оставляет наблюдателя сиротой (он держал бы вывод тестового процесса).
impl Drop for Watcher {
    fn drop(&mut self) {
        if !self.stopped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(port: u16) -> Ws {
    let url = format!("ws://127.0.0.1:{port}/?token={}", token());
    let t0 = std::time::Instant::now();
    loop {
        match tokio::time::timeout(
            Duration::from_secs(5),
            tokio_tungstenite::connect_async(&url),
        )
        .await
        {
            Ok(Ok((ws, _))) => return ws,
            _ if t0.elapsed() < Duration::from_secs(15) => {
                tokio::time::sleep(Duration::from_millis(100)).await
            }
            _ => panic!("SETUP НЕ СОСТОЯЛСЯ: прод-бинарь не принимает соединения"),
        }
    }
}

/// Ждать сообщение типа `ty` (до `secs`); остальное пропускать. Понимает обе формы провода:
/// v1 `{"type": "snapshot"|"frame"}` и старую legacy-пути `{"Snapshot": …}` / `{"Frame": …}`.
async fn wait_type(ws: &mut Ws, ty: &str, secs: u64) -> Option<Value> {
    let old = {
        let mut c = ty.chars();
        c.next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return None;
        }
        match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(m))) => {
                if let Ok(v) = serde_json::from_slice::<Value>(m.into_data().as_ref()) {
                    if v.get("type").and_then(Value::as_str) == Some(ty) || v.get(&old).is_some() {
                        return Some(v);
                    }
                }
            }
            _ => return None,
        }
    }
}

fn subscribe_msg(id: &str) -> String {
    json!({"op":"subscribe","v":1,"id":id,"selector":{
        "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
        "bands":SEVEN.to_vec(),"window_ms":60000,"depth_cadence_ms":1000}})
    .to_string()
}

/// Подписка → снимок → дописать события → дождаться кадра (первый `pump` состоялся).
async fn one_v1_subscription(ws: &mut Ws, journal: &Path, id: &str, tail_from: u64) {
    ws.send(Message::Text(subscribe_msg(id)))
        .await
        .expect("send");
    assert!(
        wait_type(ws, "snapshot", 20).await.is_some(),
        "SETUP НЕ СОСТОЯЛСЯ: подписка `{id}` не получила snapshot — мера открытий ни о чём"
    );
    append(journal, tail_from, 30);
    assert!(
        wait_type(ws, "frame", 20).await.is_some(),
        "SETUP НЕ СОСТОЯЛСЯ: после дописи событий кадр не пришёл — первый pump не наблюдён"
    );
}

fn assert_opens(opens: &BTreeMap<String, usize>, zst: &[String], want: usize, what: &str) {
    let got: Vec<(String, usize)> = zst
        .iter()
        .map(|n| (n.clone(), *opens.get(n).unwrap_or(&0)))
        .collect();
    assert!(
        got.iter().all(|(_, c)| *c == want),
        "TD-229 / I-1: {what}: открытий каждого сжатого сегмента ожидалось РОВНО {want}, получено \
         {got:?}. Каждое открытие `.zst` — распаковка первого блока (64–128 КиБ); лишний обход \
         каталога на подписку умножает чтение на число сегментов, растущее с историей"
    );
}

/// **`k1` — v1-подписка: один обход каталога на подписку.**
#[tokio::test(flavor = "multi_thread")]
async fn k1_v1_subscription_walks_catalog_once() {
    let (journal, ckpt, zst) = fixture();
    let port = free_port();
    let _srv = spawn_serve(journal.path(), ckpt.path(), port, 5_000);
    let mut ws = connect(port).await;
    let w = Watcher::start(journal.path());
    one_v1_subscription(&mut ws, journal.path(), "s1", 3_040).await;
    let opens = w.stop();
    assert_opens(&opens, &zst, 1, "одна v1-подписка");
}

/// **`k2` — вторая подписка на том же соединении платит СВОЙ один обход, не три.**
#[tokio::test(flavor = "multi_thread")]
async fn k2_second_subscription_walks_catalog_once_more() {
    let (journal, ckpt, zst) = fixture();
    let port = free_port();
    let _srv = spawn_serve(journal.path(), ckpt.path(), port, 5_000);
    let mut ws = connect(port).await;
    let w = Watcher::start(journal.path());
    one_v1_subscription(&mut ws, journal.path(), "s1", 3_040).await;
    one_v1_subscription(&mut ws, journal.path(), "s2", 3_070).await;
    let opens = w.stop();
    assert_opens(&opens, &zst, 2, "две v1-подписки подряд");
}

/// **`k3` — legacy-путь (клиент молчит окно ожидания, селектор сервера): тоже один обход.**
#[tokio::test(flavor = "multi_thread")]
async fn k3_legacy_path_walks_catalog_once() {
    let (journal, ckpt, zst) = fixture();
    let port = free_port();
    let _srv = spawn_serve(journal.path(), ckpt.path(), port, 200);
    let w = Watcher::start(journal.path());
    let mut ws = connect(port).await;
    assert!(
        wait_type(&mut ws, "snapshot", 20).await.is_some(),
        "SETUP НЕ СОСТОЯЛСЯ: legacy-путь не отдал snapshot"
    );
    append(journal.path(), 3_040, 30);
    assert!(
        wait_type(&mut ws, "frame", 20).await.is_some(),
        "SETUP НЕ СОСТОЯЛСЯ: legacy-путь — кадр после дописи не пришёл"
    );
    let opens = w.stop();
    assert_opens(&opens, &zst, 1, "legacy-подписка");
}

/// **`k4` — переподписка с ТЕМ ЖЕ `id` (ветка замены подписки в `handle_v1_message`) — тоже
/// один обход на подписку.**
#[tokio::test(flavor = "multi_thread")]
async fn k4_resubscribe_same_id_walks_catalog_once_more() {
    let (journal, ckpt, zst) = fixture();
    let port = free_port();
    let _srv = spawn_serve(journal.path(), ckpt.path(), port, 5_000);
    let mut ws = connect(port).await;
    let w = Watcher::start(journal.path());
    one_v1_subscription(&mut ws, journal.path(), "s1", 3_040).await;
    one_v1_subscription(&mut ws, journal.path(), "s1", 3_070).await;
    let opens = w.stop();
    assert_opens(&opens, &zst, 2, "подписка и переподписка с тем же id");
}
