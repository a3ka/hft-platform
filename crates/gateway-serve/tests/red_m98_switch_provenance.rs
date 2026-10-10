//! RED `M-98` (`TD-251`; sacred, architect-only) — **переподписка с ТЕМ ЖЕ `id` обязана
//! объявлять провенанс истории так же честно, как новая подписка** (`VB-I-11`).
//!
//! # Дефект (по коду, `crates/gateway-serve/src/lib.rs` на `a660b3a7`)
//!
//! `handle_v1_message`, ветка `inner.subs.contains_key(id)` (SWITCH, `:1379`): после
//! `LiveReducer::resume` снимок уходит клиенту как есть — `live.snapshot_checked()`, и его
//! `history_start_seq` / `history_truncated` — значения СЛЕПКА. Ветка новой подписки (ADD,
//! `:1527-1622`) и legacy-путь (`:2400-2462`) после `M-95` пересчитывают провенанс по каталогу
//! редуктора, проверив его свежесть (`is_fresh` → `refresh`), и на любой `Err` дают
//! `(frozen, true)`. На SWITCH этого нет вовсе: ретеншен, удаливший префикс ПОСЛЕ записи слепка,
//! клиенту, переподписавшемуся с тем же `id`, не объявляется — «история полная» там, где её нет.
//! Пред-существующее; `M-95` поведение ветки не менял (`A-050` NOTE 1, спека `M-95` §5).
//!
//! # Сценарии (спека `milestones/M-98-switch-provenance.md` §4)
//!
//! | # | мир | требование | против какой реализации |
//! |---|---|---|---|
//! | `s0` | переподписка, журнал цел, точка переподписки пройдена без действия | `truncated == false` | «всегда `true`» |
//! | `s1` | ранний сегмент удалён МЕЖДУ первым снимком и переподпиской | `truncated == true`, `history_start_seq > 0` | сегодняшний код (значения слепка) |
//! | `s2` | удаление В ТОЧКЕ переподписки: каталог построен, провенанс ещё не посчитан | то же + свидетель места | провенанс без проверки свежести |
//! | `s3` | то же + каталог журнала не перечисляется (`0300`) | `truncated == true` | `is_fresh(..)` с проглоченным `Err` |
//! | `s4` | то же + `is_fresh → Ok(false)`, затем `refresh → Err` | `truncated == true` | проглоченный `Err` у `refresh` |
//!
//! # Точка остановки — новый контракт ветки SWITCH
//!
//! Канал `m98-switch-catalog:<id>`: на ветке SWITCH, ПОСЛЕ `resume` (каталог подписки построен —
//! доказывает сторож inotify) и НЕПОСРЕДСТВЕННО перед расчётом провенанса (между ними нет
//! `pump`/`is_fresh`/`refresh`/`SegmentCatalog::open`). Только `feature = "testing"`, как у
//! `m95-catalog:*`. Канал ОТЛИЧЕН от ADD-канала `m95-catalog:<id>` намеренно: точка доказывает
//! заодно, что повторный `subscribe` исполнила именно ветка SWITCH, а не ADD (реализация,
//! уводящая повторную подписку в ADD, нарушила бы `CT-RFC-09` §2.4 и была бы поймана по
//! «точка не наступила»).
//!
//! # Предел, названный честно
//!
//! `cargo test --all` файл не исполняет: без `--features testing` он не компилируется. Исполняет
//! шаг `cargo test -p gateway-serve --features testing` джоба `build-test` (`TD-253`); форму шага
//! держит `scripts/check_feature_oracles.sh`. Помощники (фикстура, наблюдатель, миры отказа)
//! скопированы из `red_m95_provenance_fresh.rs` ДОСЛОВНО: у тест-таргетов нет общего модуля, и
//! заводить его ради одного файла — правка чужого набора.
//!
//! # Состояние
//!
//! Заведён красным на `a660b3a7` (5 из 5): `s1` — по ДЕФЕКТУ (снимок переподписки несёт
//! `history_truncated = false` — провенанс слепка); `s0`, `s2`…`s4` — по отсутствию точки
//! `m98-switch-catalog:*`. `s0` — парный мир: на реализации он обязан стать зелёным, и его
//! требование (`truncated == false`) сегодняшний код выполняет — красен он только по точке.
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

const SECRET: &[u8] = b"m98-switch-provenance-secret";
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
        provenance: "m98".to_string(),
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
            sub: "m98".to_string(),
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
         подписки построен, провенанс ещё не посчитан» (контракт точки — спека M-95 §3, на ветке SWITCH — M-98 §3). Без неё \
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

// ───────────────────────────── сценарии M-98 ─────────────────────────────

const EARLIEST: &str = "segment-00000000.jrnl.zst";

async fn subscribe(ws: &mut Ws, id: &str) {
    ws.send(Message::Text(
        json!({"op":"subscribe","v":1,"id":id,"selector":{
            "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
            "bands":SEVEN.to_vec(),"window_ms":60000,"depth_cadence_ms":1000}})
        .to_string(),
    ))
    .await
    .expect("send subscribe");
}

/// Первый снимок подписки `id` (ветка ADD) — и setup-страж: на целом журнале он НЕ объявляет
/// историю усечённой. Иначе мир построен не тот, и вердикт по второму снимку ничего не значит.
async fn first_snapshot_complete(ws: &mut Ws, id: &str) {
    subscribe(ws, id).await;
    let body = snapshot_body(ws).await;
    assert!(
        !truncated(&body),
        "SETUP НЕ СОСТОЯЛСЯ: ПЕРВЫЙ снимок подписки `{id}` (ветка ADD, журнал цел) объявил историю \
         усечённой — мир сценария не построен: {body}"
    );
}

/// Переподписка `id` с остановкой в точке `m98-switch-catalog:<id>` и действием `act` в ней.
/// Отсчёт открытий раннего сегмента берётся ПОСЛЕ первого снимка: открытие, сделанное ADD-веткой,
/// порядка переподписки не доказывает.
async fn resubscribe_with_pause<F>(ws: &mut Ws, w: std::sync::Arc<Watcher>, id: &str, act: F)
where
    F: FnOnce() + Send + 'static,
{
    let ch = format!("m98-switch-catalog:{id}");
    rendezvous::arm(&ch);
    let baseline = w.opens_of(EARLIEST);
    subscribe(ws, id).await;
    let chc = ch.clone();
    tokio::task::spawn_blocking(move || pause_and(&chc, &w, EARLIEST, baseline, act))
        .await
        .expect("pause");
    rendezvous::test_remove(&ch);
}

/// **`s0` — парный мир.** Переподписка на целом журнале НЕ объявляет историю усечённой. Без него
/// `s1`…`s4` прошла бы реализация «на SWITCH всегда `truncated = true`».
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s0_switch_without_retention_history_is_complete() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, _earliest) = fixture();
    let addr = start(dir.path(), ckpt.path()).await;
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let mut ws = connect(&addr).await;
    first_snapshot_complete(&mut ws, "s0").await;
    let _g = Guard("m98-switch-catalog:s0".to_string());
    resubscribe_with_pause(&mut ws, w, "s0", || {}).await;
    let body = snapshot_body(&mut ws).await;
    assert!(
        !truncated(&body),
        "M-98 / VB-I-11: на ЦЕЛОМ журнале переподписка объявила историю усечённой — реализация \
         «всегда truncated=true» прошла бы s1…s4 без расчёта провенанса: {body}"
    );
}

/// **`s1` — ретеншен удалил ранний сегмент МЕЖДУ первым снимком и переподпиской.** Снимок
/// переподписки обязан объявить историю усечённой. Красен на сегодняшнем коде БЕЗ всякой точки
/// остановки: ветка SWITCH отдаёт значения слепка (`history_truncated = false`). Сценарий судит
/// ТРЕБОВАНИЕ, а не путь: он остаётся верным при любом месте точки.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s1_switch_after_retention_is_honest() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, earliest) = fixture();
    let addr = start(dir.path(), ckpt.path()).await;
    let mut ws = connect(&addr).await;
    first_snapshot_complete(&mut ws, "s1").await;
    std::fs::remove_file(&earliest).expect("удаление раннего сегмента (как retention-prune)");
    assert!(
        !earliest.exists(),
        "SETUP НЕ СОСТОЯЛСЯ: ранний сегмент не удалён — ретеншен не смоделирован"
    );
    subscribe(&mut ws, "s1").await;
    let body = snapshot_body(&mut ws).await;
    assert!(
        truncated(&body),
        "M-98 / TD-251 / VB-I-11: ранний сегмент удалён ДО переподписки с тем же id, а её снимок \
         говорит «история полная» — ветка SWITCH отдала провенанс СЛЕПКА, не посчитав его по \
         журналу. Обязан: тот же расчёт, что у новой подписки (M-95 §3): {body}"
    );
    let start_seq = body
        .get("history_start_seq")
        .and_then(Value::as_u64)
        .expect("history_start_seq");
    assert!(
        start_seq > 0,
        "M-98 / VB-I-11: history_start_seq = 0 после удаления префикса — переподписка объявила \
         началом истории seq, которого в журнале больше нет: {body}"
    );
}

/// **`s2` — удаление В ТОЧКЕ переподписки** (каталог SWITCH построен, провенанс не посчитан).
/// Провенанс, посчитанный по каталогу без проверки свежести, скажет «история полная». Свидетель
/// места (`A-050` A2): события, дописанные в остановке, в снимок не попадают.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s2_switch_retention_between_catalog_and_provenance_is_honest() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, earliest) = fixture();
    let addr = start(dir.path(), ckpt.path()).await;
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let mut ws = connect(&addr).await;
    first_snapshot_complete(&mut ws, "s2").await;
    let _g = Guard("m98-switch-catalog:s2".to_string());
    let jd = dir.path().to_path_buf();
    resubscribe_with_pause(&mut ws, w, "s2", move || {
        std::fs::remove_file(&earliest).expect("удаление раннего сегмента (как retention-prune)");
        append(&jd, TAIL_END, AT_PAUSE); // свидетель места (`A-050` A2)
    })
    .await;
    let body = snapshot_body(&mut ws).await;
    assert_point_right_before_provenance(&body, "switch", false);
    assert!(
        truncated(&body),
        "M-98 / TD-251 / VB-I-11: ранний сегмент удалён ПОСЛЕ построения каталога переподписки и ДО \
         провенанса, а снимок говорит «история полная» — провенанс посчитан по устаревшему каталогу \
         либо не посчитан вовсе. Обязан: is_fresh → refresh → расчёт, либо (frozen, true): {body}"
    );
}

/// **`s3` — то же, и проверка свежести ОТКАЗАЛА** (`read_dir` каталога журнала → `Err`). Исход
/// обязан быть `(frozen_start_seq, true)`. Мутант «`is_fresh(..).unwrap_or_default()`» ⇒
/// провенанс по устаревшему каталогу ⇒ «история полная» ⇒ FAILED.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s3_switch_catalog_unlistable_is_fail_closed() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, earliest) = fixture();
    let addr = start(dir.path(), ckpt.path()).await;
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let mut ws = connect(&addr).await;
    first_snapshot_complete(&mut ws, "s3").await;
    let _g = Guard("m98-switch-catalog:s3".to_string());
    let jd = dir.path().to_path_buf();
    let sealed = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sealed_in = sealed.clone();
    resubscribe_with_pause(&mut ws, w, "s3", move || {
        std::fs::remove_file(&earliest).expect("удаление раннего сегмента (как retention-prune)");
        append(&jd, TAIL_END, AT_PAUSE); // свидетель места (`A-050` A2)
        *sealed_in.lock().unwrap() = Some(UnlistableDir::seal(&jd));
    })
    .await;
    let body = snapshot_body(&mut ws).await;
    drop(sealed.lock().unwrap().take());
    assert_point_right_before_provenance(&body, "switch", false);
    assert!(
        truncated(&body),
        "M-98 / R-249 Б-1 / VB-I-11: переподписка — префикс удалён, проверка свежести каталога \
         ОТКАЗАЛА, а снимок говорит «история полная» — Err проглочен. Обязан: (frozen, true): {body}"
    );
}

/// **`s4` — то же, `is_fresh → Ok(false)`, затем `refresh → Err`** (мир `R-250` Б-1). Мутант
/// «`Err` у `refresh` проглочен, провенанс по каталогу» ⇒ FAILED.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s4_switch_refresh_failure_after_stale_is_fail_closed() {
    let _serial = SERIAL.lock().await;
    gateway_serve::server::set_effective_grace_ms(5_000);
    let (dir, ckpt, _earliest) = fixture();
    let addr = start(dir.path(), ckpt.path()).await;
    let w = std::sync::Arc::new(Watcher::start(dir.path()));
    let mut ws = connect(&addr).await;
    first_snapshot_complete(&mut ws, "s4").await;
    let _g = Guard("m98-switch-catalog:s4".to_string());
    let jd = dir.path().to_path_buf();
    let sealed = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sealed_in = sealed.clone();
    resubscribe_with_pause(&mut ws, w, "s4", move || {
        *sealed_in.lock().unwrap() = Some(act_refresh_fails(&jd));
    })
    .await;
    let body = snapshot_body(&mut ws).await;
    drop(sealed.lock().unwrap().take());
    assert_point_right_before_provenance(&body, "switch", false);
    assert!(
        truncated(&body),
        "M-98 / R-250 Б-1 / VB-I-11: переподписка — is_fresh «устарел», refresh ОТКАЗАЛ, а снимок \
         говорит «история полная» — Err refresh проглочен. Обязан: (frozen, true): {body}"
    );
}
