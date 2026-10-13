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
//! Контракт точки (спека M-95 §3, задачи 1 и 3; `A-050` A1): после того как подписка ПОСТРОИЛА
//! свой каталог, НЕПОСРЕДСТВЕННО перед расчётом провенанса (на legacy — ПОСЛЕ цикла догона; между
//! точкой и провенансом нет `pump`/`is_fresh`/`refresh`/`SegmentCatalog::open`) транспорт зовёт `rendezvous::pump_signal_and_wait(<канал>)`:
//! `m95-catalog:<id подписки>` на v1-пути новой подписки, `m95-catalog:legacy` на legacy-пути.
//! Точка есть только в тестовой сборке (`feature = "testing"`), как у M-87. Тест в остановке
//! удаляет самый ранний сегмент (слепок его покрывает — родословная законна, как после
//! `retention-prune`) и отпускает.
//!
//! Место точки доказывается двумя свидетелями: сторож inotify — каталог построен ДО точки
//! (`C-285`); дописанные в остановке события не попадают в снимок — догон не прошёл ПОСЛЕ точки
//! (`A-050` A2; мутант E5 «точка до догона + провенанс без проверки свежести» иначе зеленел `f2`).
//!
//! Парный мир (`f0`) обязателен: без удаления та же подписка НЕ объявляет историю усечённой —
//! иначе прошла бы реализация «всегда `truncated = true`».
//!
//! # Предел, названный честно
//!
//! `cargo test --all` его НЕ исполняет: без `--features testing` файл не компилируется вовсе. Исполняет
//! отдельный шаг джоба `build-test` — `cargo test -p gateway-serve --features testing` (`TD-253`);
//! его форму держит барьер `scripts/check_feature_oracles.sh`. Гейт приёмки M-95 — в архиве
//! (`docs/archive/verify_M-95.sh`) и не исполняется.
//!
//! # Состояние
//!
//! Заведён красным (точки `m95-catalog:*` не было: у реализации не было места, где каталог уже
//! построен, а провенанс ещё не посчитан). Зелёный с `M-95` (PR #333): f0…f6 — 7 passed.
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

/// Хвост фикстуры, дописанный ПОСЛЕ слепка и ДО подключения: `[TAIL_FROM, TAIL_END)`.
const TAIL_FROM: u64 = 3_000;
const TAIL_END: u64 = TAIL_FROM + 40;
/// Сколько событий дописывается В ОСТАНОВКЕ — свидетель места точки (`A-050` A2).
const AT_PAUSE: u64 = 7;

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
    append(dir.path(), 0, TAIL_FROM);
    journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact");
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    append(dir.path(), TAIL_FROM, TAIL_END - TAIL_FROM);
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

/// **Свидетель места (`A-050` A2).** В остановке, кроме удаления, дописываются `AT_PAUSE` событий с
/// `seq >= TAIL_END`. Снимок, построенный ДО точки, их не содержит; если они в нём — между точкой и
/// провенансом прошёл догон (`pump`, а с ним `is_fresh`/`refresh`), и проверку свежести сделал
/// ЧУЖОЙ код: провенанс без собственной проверки оказался бы честен случайно (`A-050` E5).
///
/// Нижняя граница (`caught_up`) — setup-страж ТОЛЬКО для legacy: там снимок строится после догона
/// до хвоста, и без неё верхняя граница была бы вакуумной (снимок, не догнавший даже хвост фикстуры,
/// прошёл бы её всегда). На v1 догона нет — снимок отдаётся из слепка (`upto_seq` = позиция слепка,
/// замер 2999), там действует только верхняя граница: она ловит догон, если реализация его введёт.
fn assert_point_right_before_provenance(body: &Value, path: &str, caught_up: bool) {
    let upto = body
        .get("cursor")
        .and_then(|c| c.get("upto_seq"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("SETUP НЕ СОСТОЯЛСЯ: в снимке нет cursor.upto_seq: {body}"));
    assert!(
        !caught_up || upto >= TAIL_END - 1,
        "SETUP НЕ СОСТОЯЛСЯ: снимок не догнал хвост фикстуры (upto_seq={upto}, ждали ≥ {}): {body}",
        TAIL_END - 1
    );
    assert!(
        upto < TAIL_END,
        "M-95 / A-050 E5/W2: {path} — снимок содержит события, дописанные В ОСТАНОВКЕ \
         (upto_seq={upto} ≥ {TAIL_END}): точка стоит ДО догона, каталог тронут между точкой и \
         провенансом. Точка обязана стоять непосредственно перед расчётом провенанса (спека §3)"
    );
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

/// Наблюдатель открытий файлов каталога журнала (`inotify IN_OPEN`, Python `ctypes`) — тот же,
/// что в `red_m95_catalog_once`. Здесь он ДОКАЗЫВАЕТ ПОРЯДОК (`C-285`): к моменту остановки
/// подписка уже открыла самый ранний сжатый сегмент, то есть её каталог построен.
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
    r, _, _ = select.select([fd], [], [], 0.05)
    if not r:
        if os.path.exists(ready + ".stop"): break
        continue
    buf = os.read(fd, 65536); i = 0
    while i < len(buf):
        wd, mask, cookie, ln = struct.unpack_from("iIII", buf, i)
        f.write(buf[i+16:i+16+ln].rstrip(b"\0").decode() + "\n"); i += 16 + ln
"#;

struct Watcher {
    child: std::process::Child,
    out: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}
impl Watcher {
    fn start(dir: &std::path::Path) -> Self {
        let tmp = tempfile::tempdir().expect("watcher tmp");
        let script = tmp.path().join("w.py");
        std::fs::write(&script, WATCHER).unwrap();
        let (out, ready) = (tmp.path().join("opens"), tmp.path().join("ready"));
        let child = std::process::Command::new("python3")
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
            out,
            _tmp: tmp,
        }
    }
    fn opens_of(&self, name: &str) -> usize {
        std::fs::read_to_string(&self.out)
            .unwrap_or_default()
            .lines()
            .filter(|l| *l == name)
            .count()
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Остановиться в точке канала; ДОКАЗАТЬ ПОРЯДОК — к остановке подписка уже открыла `earliest`
/// (каталог построен; `C-285`); выполнить `act`; отпустить.
fn pause_and(ch: &str, w: &Watcher, earliest: &str, baseline: usize, act: impl FnOnce()) {
    assert!(
        rendezvous::test_wait_for_pump(ch, BUDGET),
        "M-95 / C-284 B2: точка `{ch}` не наступила за {BUDGET:?} — у реализации нет места «каталог \
         подписки построен, провенанс ещё не посчитан» (контракт точки — спека M-95 §3). Без неё \
         удаление сегмента между двумя наблюдениями не воспроизводится детерминированно"
    );
    // События ядра доставляются асинхронно: ждём появления открытия до 2 с.
    let t0 = std::time::Instant::now();
    while w.opens_of(earliest) <= baseline && t0.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let n = w.opens_of(earliest);
    if n <= baseline {
        rendezvous::test_release(ch);
        panic!(
            "M-95 / C-285: в точке `{ch}` самый ранний сжатый сегмент `{earliest}` не открыт ПОСЛЕ \
             начала подписки (до неё: {baseline}, сейчас: {n}) — точка стоит ДО построения каталога подписки. Удаление в такой точке произошло \
             бы раньше обхода, и реализация без проверки свежести прошла бы f1/f2. Точка обязана \
             стоять ПОСЛЕ построения каталога (спека M-95 §3)"
        );
    }
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
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let earliest_name = "segment-00000000.jrnl.zst".to_string();
    // Отсчёт ДО подписки/соединения: открытия до неё (старт сервера и т.п.) порядка не доказывают.
    let baseline = w.opens_of(&earliest_name);
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
    let wc = w.clone();
    let en = earliest_name.clone();
    tokio::task::spawn_blocking(move || pause_and(&chc, &wc, &en, baseline, || {}))
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
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let earliest_name = "segment-00000000.jrnl.zst".to_string();
    // Отсчёт ДО подписки/соединения: открытия до неё (старт сервера и т.п.) порядка не доказывают.
    let baseline = w.opens_of(&earliest_name);
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
    let wc = w.clone();
    let en = earliest_name.clone();
    let jd = dir.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        pause_and(&chc, &wc, &en, baseline, move || {
            std::fs::remove_file(&e).expect("удаление раннего сегмента (как retention-prune)");
            append(&jd, TAIL_END, AT_PAUSE); // свидетель места (`A-050` A2)
        })
    })
    .await
    .expect("pause");
    let body = snapshot_body(&mut ws).await;
    assert_point_right_before_provenance(&body, "v1", false);
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
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let earliest_name = "segment-00000000.jrnl.zst".to_string();
    // Отсчёт ДО подписки/соединения: открытия до неё (старт сервера и т.п.) порядка не доказывают.
    let baseline = w.opens_of(&earliest_name);
    let mut ws = connect(&addr).await;
    let chc = ch.clone();
    let e = earliest.clone();
    let wc = w.clone();
    let en = earliest_name.clone();
    let jd = dir.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        pause_and(&chc, &wc, &en, baseline, move || {
            std::fs::remove_file(&e).expect("удаление раннего сегмента (как retention-prune)");
            append(&jd, TAIL_END, AT_PAUSE); // свидетель места (`A-050` A2)
        })
    })
    .await
    .expect("pause");
    let body = snapshot_body(&mut ws).await;
    assert_point_right_before_provenance(&body, "legacy", true);
    assert!(
        truncated(&body),
        "M-95 / C-284 B2 / VB-I-11: legacy-путь — ранний сегмент удалён между каталогом и \
         провенансом, снимок говорит «история полная»: {body}"
    );
}

/// Каталог журнала, закрытый на ЧТЕНИЕ списка (`0300`: `stat`/открытие по пути работают,
/// `read_dir` — нет). Права возвращаются при выходе из области — иначе `TempDir` не уберёт каталог,
/// а сервер следующего мира не прочтёт журнал.
/// Хранит ИСХОДНЫЙ режим каталога и возвращает именно его (`C-292` примечание).
struct UnlistableDir(std::path::PathBuf, u32);
impl UnlistableDir {
    fn seal(dir: &std::path::Path) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let orig = std::fs::metadata(dir).expect("stat").permissions().mode() & 0o7777;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o300)).expect("chmod 0300");
        // Setup-страж: под root `chmod` не запрещает чтение — отказ обновления каталога не
        // смоделирован, и мир молча мерил бы f1.
        assert!(
            std::fs::read_dir(dir).is_err(),
            "SETUP НЕ СОСТОЯЛСЯ: read_dir каталога журнала удался после chmod 0300 (прогон под \
             root?) — отказ обновления каталога не смоделирован"
        );
        UnlistableDir(dir.to_path_buf(), orig)
    }
}
impl Drop for UnlistableDir {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(self.1));
    }
}

/// **`f3` — v1: ретеншен удалил префикс И проверка свежести/обновление каталога ОТКАЗАЛИ
/// (`R-249` Б-1).** Исход обязан быть `(frozen_start_seq, true)` — «не знаем, не обещаем»
/// (`VB-I-11`, M-87 задача 20, оракул `red_m87_history_provenance_failclosed` `h3`/`h4` — на
/// функции, которую путь выдачи после M-95 не зовёт). Мутант «отбросить ошибку `refresh`
/// (`let _ =`) / `is_fresh(..).unwrap_or_default()`» ⇒ провенанс по устаревшему каталогу ⇒
/// «история полная» ⇒ FAILED.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn f3_v1_catalog_refresh_failure_is_fail_closed() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, earliest) = fixture();
    let ch = "m95-catalog:s3".to_string();
    rendezvous::arm(&ch);
    let _g = Guard(ch.clone());
    let addr = start(dir.path(), ckpt.path()).await;
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let earliest_name = "segment-00000000.jrnl.zst".to_string();
    let baseline = w.opens_of(&earliest_name);
    let mut ws = connect(&addr).await;
    ws.send(Message::Text(
        json!({"op":"subscribe","v":1,"id":"s3","selector":{
            "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
            "bands":SEVEN.to_vec(),"window_ms":60000,"depth_cadence_ms":1000}})
        .to_string(),
    ))
    .await
    .expect("send");
    let chc = ch.clone();
    let e = earliest.clone();
    let wc = w.clone();
    let en = earliest_name.clone();
    let jd = dir.path().to_path_buf();
    let sealed = tokio::task::spawn_blocking(move || {
        let mut sealed = None;
        pause_and(&chc, &wc, &en, baseline, || {
            std::fs::remove_file(&e).expect("удаление раннего сегмента (как retention-prune)");
            append(&jd, TAIL_END, AT_PAUSE); // свидетель места (`A-050` A2)
            sealed = Some(UnlistableDir::seal(&jd)); // R-249 Б-1: обновление каталога откажет
        });
        sealed
    })
    .await
    .expect("pause");
    let body = snapshot_body(&mut ws).await;
    drop(sealed);
    assert_point_right_before_provenance(&body, "v1", false);
    assert!(
        truncated(&body),
        "M-95 / R-249 Б-1 / VB-I-11: префикс удалён, обновление каталога ОТКАЗАЛО, а снимок говорит \
         «история полная» — ошибка проверки свежести проглочена, провенанс посчитан по устаревшему \
         каталогу. Обязан: на любой Err проверки свежести/обновления — (frozen_start_seq, true): {body}"
    );
}

/// **`f4` — legacy: то же требование, что `f3`, канал `m95-catalog:legacy`.**
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn f4_legacy_catalog_refresh_failure_is_fail_closed() {
    let _serial = SERIAL.lock().await;
    let (dir, ckpt, earliest) = fixture();
    gateway_serve::server::set_effective_grace_ms(200);
    let ch = "m95-catalog:legacy".to_string();
    rendezvous::arm(&ch);
    let _g = Guard(ch.clone());
    let addr = start(dir.path(), ckpt.path()).await;
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let earliest_name = "segment-00000000.jrnl.zst".to_string();
    let baseline = w.opens_of(&earliest_name);
    let mut ws = connect(&addr).await;
    let chc = ch.clone();
    let e = earliest.clone();
    let wc = w.clone();
    let en = earliest_name.clone();
    let jd = dir.path().to_path_buf();
    let sealed = tokio::task::spawn_blocking(move || {
        let mut sealed = None;
        pause_and(&chc, &wc, &en, baseline, || {
            std::fs::remove_file(&e).expect("удаление раннего сегмента (как retention-prune)");
            append(&jd, TAIL_END, AT_PAUSE); // свидетель места (`A-050` A2)
            sealed = Some(UnlistableDir::seal(&jd));
        });
        sealed
    })
    .await
    .expect("pause");
    let body = snapshot_body(&mut ws).await;
    drop(sealed);
    assert_point_right_before_provenance(&body, "legacy", true);
    assert!(
        truncated(&body),
        "M-95 / R-249 Б-1 / VB-I-11: legacy-путь — префикс удалён, обновление каталога ОТКАЗАЛО, \
         снимок говорит «история полная»: {body}"
    );
}

/// Файл, закрытый на чтение (`000`); исходный режим возвращается при выходе из области.
struct UnreadableFile(std::path::PathBuf, u32);
impl UnreadableFile {
    fn seal(path: &std::path::Path) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let orig = std::fs::metadata(path).expect("stat").permissions().mode() & 0o7777;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).expect("chmod 000");
        UnreadableFile(path.to_path_buf(), orig)
    }
}
impl Drop for UnreadableFile {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(self.1));
    }
}

/// Сжатые сегменты каталога по возрастанию индекса.
fn zst_segments(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".jrnl.zst"))
        .collect();
    v.sort();
    v
}

/// Мир `R-250` Б-1: в остановке удалить ТРИ самых ранних `.zst` (diff каталога > 2 ⇒
/// `is_fresh` обязан ответить `Ok(false)`, а не обновиться инкрементально), дописать свидетеля
/// места, закрыть на чтение один из оставшихся `.zst` (⇒ полный `refresh` классифицирует его и
/// получает `Err`). Каталог журнала остаётся читаемым — `is_fresh` (список имён + `stat`) успешен.
///
/// **Setup-страж исполнения ИМЕННО этой ветки:** каталог, построенный тестом ДО изменений тем же
/// публичным `journal::SegmentCatalog`, после них обязан дать `is_fresh == Ok(false)` и
/// `refresh == Err`. Иначе мир не построен (например, `is_fresh` вернул `Err` — это `f3`/`f4`, а не
/// `R-250`), и тест падает как `SETUP НЕ СОСТОЯЛСЯ`, а не как вердикт.
fn act_refresh_fails(jd: &std::path::Path) -> UnreadableFile {
    let (mut guard_cat, _) = journal::SegmentCatalog::open(jd).expect("guard catalog");
    let zst = zst_segments(jd);
    assert!(
        zst.len() >= 4,
        "SETUP НЕ СОСТОЯЛСЯ: сжатых сегментов {} — нужно ≥ 4 (три удалить, один закрыть)",
        zst.len()
    );
    for p in &zst[..3] {
        std::fs::remove_file(p).expect("удаление ранних сегментов (как retention-prune)");
    }
    append(jd, TAIL_END, AT_PAUSE); // свидетель места (`A-050` A2)
    let sealed = UnreadableFile::seal(&zst[3]);
    assert!(
        std::fs::File::open(&zst[3]).is_err(),
        "SETUP НЕ СОСТОЯЛСЯ: закрытый на чтение сегмент открывается (прогон под root?)"
    );
    match guard_cat.is_fresh(jd) {
        Ok((false, _)) => {}
        other => panic!(
            "SETUP НЕ СОСТОЯЛСЯ: is_fresh после изменений = {other:?}, ожидалось Ok(false) — ветка \
             «is_fresh → Ok(false), refresh → Err» (R-250 Б-1) не построена"
        ),
    }
    assert!(
        guard_cat.refresh(jd).is_err(),
        "SETUP НЕ СОСТОЯЛСЯ: refresh после изменений успешен — ветка «refresh → Err» не построена"
    );
    sealed
}

/// **`f5` — v1: `is_fresh` → `Ok(false)`, затем `refresh` → `Err` (`R-250` Б-1).** Исход обязан
/// быть `(frozen_start_seq, true)`. Мутант «`Err` у `refresh` проглочен, провенанс по каталогу»
/// (`R-250` MV2) ⇒ каталог устарел ⇒ «история полная» ⇒ FAILED.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn f5_v1_refresh_failure_after_stale_is_fail_closed() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, earliest) = fixture();
    let ch = "m95-catalog:s5".to_string();
    rendezvous::arm(&ch);
    let _g = Guard(ch.clone());
    let addr = start(dir.path(), ckpt.path()).await;
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let earliest_name = earliest.file_name().unwrap().to_string_lossy().into_owned();
    let baseline = w.opens_of(&earliest_name);
    let mut ws = connect(&addr).await;
    ws.send(Message::Text(
        json!({"op":"subscribe","v":1,"id":"s5","selector":{
            "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
            "bands":SEVEN.to_vec(),"window_ms":60000,"depth_cadence_ms":1000}})
        .to_string(),
    ))
    .await
    .expect("send");
    let chc = ch.clone();
    let wc = w.clone();
    let en = earliest_name.clone();
    let jd = dir.path().to_path_buf();
    let sealed = tokio::task::spawn_blocking(move || {
        let mut sealed = None;
        pause_and(&chc, &wc, &en, baseline, || {
            sealed = Some(act_refresh_fails(&jd))
        });
        sealed
    })
    .await
    .expect("pause");
    let body = snapshot_body(&mut ws).await;
    drop(sealed);
    assert_point_right_before_provenance(&body, "v1", false);
    assert!(
        truncated(&body),
        "M-95 / R-250 Б-1 / VB-I-11: префикс удалён, is_fresh ответил «устарел», refresh ОТКАЗАЛ, а \
         снимок говорит «история полная» — ошибка refresh проглочена, провенанс посчитан по \
         устаревшему каталогу. Обязан: (frozen_start_seq, true): {body}"
    );
}

/// **`f6` — legacy: то же, что `f5`, канал `m95-catalog:legacy`.**
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn f6_legacy_refresh_failure_after_stale_is_fail_closed() {
    let _serial = SERIAL.lock().await;
    let (dir, ckpt, earliest) = fixture();
    gateway_serve::server::set_effective_grace_ms(200);
    let ch = "m95-catalog:legacy".to_string();
    rendezvous::arm(&ch);
    let _g = Guard(ch.clone());
    let addr = start(dir.path(), ckpt.path()).await;
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let earliest_name = earliest.file_name().unwrap().to_string_lossy().into_owned();
    let baseline = w.opens_of(&earliest_name);
    let mut ws = connect(&addr).await;
    let chc = ch.clone();
    let wc = w.clone();
    let en = earliest_name.clone();
    let jd = dir.path().to_path_buf();
    let sealed = tokio::task::spawn_blocking(move || {
        let mut sealed = None;
        pause_and(&chc, &wc, &en, baseline, || {
            sealed = Some(act_refresh_fails(&jd))
        });
        sealed
    })
    .await
    .expect("pause");
    let body = snapshot_body(&mut ws).await;
    drop(sealed);
    assert_point_right_before_provenance(&body, "legacy", true);
    assert!(
        truncated(&body),
        "M-95 / R-250 Б-1 / VB-I-11: legacy-путь — is_fresh «устарел», refresh ОТКАЗАЛ, снимок \
         говорит «история полная»: {body}"
    );
}
