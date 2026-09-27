//! RED `M-89` (sacred, architect-only) — **счётчики выдачи и свежесть ВЫХОДЯТ НАРУЖУ
//! ПРОЦЕССА: прод-бинарь на окружении из compose пишет JSON-сердцебиение** (`I-6`,
//! `TD-220`, `PL-I-8`).
//!
//! # Что не так сегодня
//!
//! У `gateway-serve` наблюдаемости нет: heartbeat удалён в `M-65` round 2, scrape-эндпоинта
//! нет; `docker logs hft-gateway-serve` — одна строка старта (`R-205`, §8-гейт 2026-09-26).
//! Предохранитель `M-87`, чьи показания никто не видит, срабатывает МОЛЧА: §8-взгляд видит
//! `healthy` и рост журнала — и был бы тем же, если бы выдача отказывала каждому второму.
//! Прод-факт, снятый ssh 2026-09-27: у контейнера `hft-gateway-serve` ДВА монтирования и оба
//! `rw=false` (`/journal`, `/ckpt`) — писать сердцебиение ФИЗИЧЕСКИ некуда (`testing.md`,
//! п. 6 чек-листа: права носителя воспроизводятся, а не воображаются).
//!
//! # Форма (спека `M-89` §4.2, задана ДОСЛОВНО)
//!
//! · env `GATEWAY_HEARTBEAT_PATH` — путь файла ВНУТРИ контейнера, на ЗАПИСЫВАЕМОМ томе
//!   (не `/journal`, не `/ckpt`); `GATEWAY_HEARTBEAT_PERIOD_MS` — период (дефолт 10 000);
//! · запись АТОМАРНАЯ: `<path>.tmp` → `rename` (читатель никогда не видит частичный файл;
//!   образец `recorder::write_heartbeat` здесь НЕ образец — он пишет `fs::write` напрямую,
//!   и это названо в спеке как расхождение с мандатом);
//! · JSON-объект с полями `schema` (=1), `ts_wall_ms`, `attempts`, `successes`,
//!   `refusals_supported`, `refusals_unsupported`, `journal_payload_bytes_read`,
//!   `slots_in_flight`, `freshness{source_ms,projection_ms,published_ms,snapshot_ms}`.
//!   Значения — из счётчиков ЭКЗЕМПЛЯРА сервера (`I-5`).
//!
//! # Что здесь пиннится
//!
//! · `h0` — КОМПОЗИЦИЯ в compose: переменная объявлена и её каталог лежит на монтировании
//!   `rw`, отличном от журнала и слепков (`testing.md` §«канарейка», п. 2: путь producer'а);
//! · `h1` — ТОЧКА ВХОДА: прод-бинарь на окружении ИЗ compose (образец
//!   `red_m87_prod_entrypoint_argv.rs`) пишет файл в течение периода; JSON целый при
//!   каждом чтении (атомарность наблюдается опросом); после обслуженного запроса значения
//!   РАСТУТ (`OPS-I-10`: продюсер, а не объявление).
//!
//! # Чего оракул НЕ ловит
//!
//! · путь читателя (ops) — `crates/ops/tests/red_m89_serving_silence.rs`; композиция
//!   «куда пишет compose ↔ откуда читает cron» — шаг `scripts/verify_M-89.sh`;
//! · установку cron на VPS — это §8-гейт и founder ★ (`deploy/README.md`);
//! · правдивость `freshness` — её позиции судит `C9` `M-87`.
//!
//! RUNTIME-RED на ревизии набора: `h0` — переменной нет в compose; `h1` — файл не появляется.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use futures_util::{SinkExt, StreamExt};
use gateway::Selector;
use journal::{EpochFilter, Journal, WriterConfig};
use jsonwebtoken::{EncodingKey, Header};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

const BASE_MS: i64 = 1_784_116_800_000;
const CANONICAL: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];
const HB_PERIOD_MS: u64 = 200;
const REQUIRED_KEYS: [&str; 9] = [
    "schema",
    "ts_wall_ms",
    "attempts",
    "successes",
    "refusals_supported",
    "refusals_unsupported",
    "journal_payload_bytes_read",
    "slots_in_flight",
    "freshness",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("корень репозитория")
        .to_path_buf()
}

fn compose_text() -> String {
    let p = repo_root().join("docker-compose.yml");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("чтение {}: {e}", p.display()))
}

/// `${VAR:-default}` → `default`; `${VAR:?msg}` → проба.
fn subst_default(raw: &str) -> String {
    let s = raw.trim().trim_matches('"');
    if let Some(inner) = s.strip_prefix("${").and_then(|x| x.strip_suffix('}')) {
        if let Some((_, d)) = inner.split_once(":-") {
            return d.to_string();
        }
        if inner.contains(":?") {
            return "probe-value".to_string();
        }
    }
    s.to_string()
}

/// Строки сервиса `gateway-serve` из compose (до следующего сервиса).
fn service_block() -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in compose_text().lines() {
        if line.starts_with("  gateway-serve:") {
            inside = true;
            continue;
        }
        if inside && line.starts_with("  ") && !line.starts_with("    ") {
            break;
        }
        if inside {
            out.push(line.to_string());
        }
    }
    assert!(
        !out.is_empty(),
        "SETUP-СТРАЖ: сервис gateway-serve не найден в docker-compose.yml"
    );
    out
}

fn compose_env() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut in_env = false;
    for line in service_block() {
        if line.trim_start().starts_with("environment:") {
            in_env = true;
            continue;
        }
        if in_env && line.starts_with("    ") && !line.starts_with("      ") {
            in_env = false;
        }
        if !in_env {
            continue;
        }
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some((k, v)) = t.split_once(':') else {
            continue;
        };
        if k.trim().starts_with("GATEWAY_") {
            out.insert(k.trim().to_string(), subst_default(v.trim()));
        }
    }
    out
}

/// Монтирования сервиса: `(источник, назначение, rw)`.
fn compose_mounts() -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    let mut in_vol = false;
    for line in service_block() {
        if line.trim_start().starts_with("volumes:") {
            in_vol = true;
            continue;
        }
        if in_vol && line.starts_with("    ") && !line.starts_with("      ") {
            in_vol = false;
        }
        if !in_vol {
            continue;
        }
        let t = line.trim();
        let Some(spec) = t.strip_prefix("- ") else {
            continue;
        };
        let spec = spec.trim_matches('"');
        let parts: Vec<&str> = spec.split(':').collect();
        if parts.len() < 2 {
            continue;
        }
        let rw = parts.get(2).is_none_or(|m| *m != "ro");
        out.push((parts[0].to_string(), parts[1].to_string(), rw));
    }
    out
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
        .expect("bind 0")
        .local_addr()
        .expect("addr")
        .port()
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

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 30,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m89".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

/// Журнал + тёплый слепок ДЛЯ СЕЛЕКТОРА ПОДПИСКИ + хвост: запрос допускается и обслуживается.
fn journal_with_warm_ckpt() -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let append = |from: u64, n: u64| {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
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
    };
    append(0, 300);
    let ckpt = tempfile::tempdir().expect("ckpt");
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    append(300, 40);
    (dir, ckpt)
}

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

/// Токен подписывается ТОЙ ЖЕ трактовкой секрета, что у сервера (`auth::key_material`,
/// задача 11 `M-87`), из значения compose (`${GATEWAY_JWT_SECRET:?…}` → проба).
fn sign(secret: &str) -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m89-hb".to_string(),
            exp: 4_000_000_000,
        },
        &EncodingKey::from_secret(&gateway_serve::auth::key_material(secret)),
    )
    .expect("jwt")
}

struct Child {
    inner: std::process::Child,
}
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.inner.kill();
        let _ = self.inner.wait();
    }
}

fn spawn_prod_binary(
    env: &BTreeMap<String, String>,
    journal: &Path,
    ckpt: &Path,
    port: u16,
    hb_path: &Path,
) -> Child {
    let bin = bin_path();
    assert!(
        bin.exists(),
        "SETUP-СТРАЖ: прод-бинарь не собран ({}) — оракул ИСПОЛНЯЕТ границу процесса",
        bin.display()
    );
    let mut cmd = Command::new(&bin);
    cmd.env_clear().env("PATH", "/usr/bin:/bin");
    for (k, v) in env {
        let v = match k.as_str() {
            "GATEWAY_JOURNAL_DIR" => journal.display().to_string(),
            "GATEWAY_CHECKPOINT_DIR" => ckpt.display().to_string(),
            "GATEWAY_ADDR" => format!("127.0.0.1:{port}"),
            _ => v.clone(),
        };
        cmd.env(k, v);
    }
    // Переменные сердцебиения ставятся ПОВЕРХ compose: если compose их не объявляет, `h0`
    // краснеет отдельно, а `h1` всё равно судит бинарь.
    cmd.env("GATEWAY_HEARTBEAT_PATH", hb_path.display().to_string());
    cmd.env("GATEWAY_HEARTBEAT_PERIOD_MS", HB_PERIOD_MS.to_string());
    let inner = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("запуск прод-бинаря");
    Child { inner }
}

/// Читать файл сердцебиения, различая «нет файла», «есть и целый», «есть и НЕ разбирается».
enum Hb {
    Missing,
    Whole(Value),
    Broken(String),
}
fn read_hb(path: &Path) -> Hb {
    match std::fs::read_to_string(path) {
        Err(_) => Hb::Missing,
        Ok(s) => match serde_json::from_str::<Value>(&s) {
            Ok(v) if v.is_object() => Hb::Whole(v),
            _ => Hb::Broken(s),
        },
    }
}

/// **h0 — compose объявляет путь сердцебиения на ЗАПИСЫВАЕМОМ монтировании.**
#[test]
fn h0_compose_declares_heartbeat_path_on_a_writable_mount() {
    let env = compose_env();
    let path = env
        .get("GATEWAY_HEARTBEAT_PATH")
        .cloned()
        .unwrap_or_default();
    assert!(
        !path.is_empty(),
        "I-6 / TD-220: docker-compose.yml не объявляет GATEWAY_HEARTBEAT_PATH у сервиса \
         gateway-serve — сердцебиению некуда писаться, и предохранитель остаётся невидимым \
         снаружи процесса (PL-I-8: алерт живёт вне наблюдаемой машины)"
    );
    let mounts = compose_mounts();
    let hit = mounts
        .iter()
        .find(|(_, dst, _)| path.starts_with(&format!("{dst}/")) || path == *dst);
    let Some((src, dst, rw)) = hit else {
        panic!(
            "I-6: GATEWAY_HEARTBEAT_PATH={path} не лежит ни на одном монтировании сервиса \
             gateway-serve ({mounts:?}) — файл жил бы в слое контейнера и умирал с ним, а \
             читатель на хосте его бы не видел"
        );
    };
    assert!(
        *rw,
        "I-6: монтирование {src}:{dst} для сердцебиения — `:ro`; прод-факт 2026-09-27: у \
         hft-gateway-serve оба монтирования rw=false, писать некуда"
    );
    assert!(
        dst != "/journal" && dst != "/ckpt",
        "I-6: сердцебиение положено на {dst} — это том журнала/слепков, у выдачи он \
         ТОЛЬКО ДЛЯ ЧТЕНИЯ по инварианту (GS-I-3, единственный писатель слепка — прогреватель)"
    );
}

/// **h1 — прод-бинарь пишет сердцебиение, JSON всегда целый, значения растут после выдачи.**
#[tokio::test]
async fn h1_prod_binary_writes_heartbeat_and_values_grow_after_serving() {
    let env = compose_env();
    let (journal, ckpt) = journal_with_warm_ckpt();
    let state = tempfile::tempdir().expect("state dir");
    let hb_path = state.path().join("gateway-serve.heartbeat");
    let port = free_port();
    let mut child = spawn_prod_binary(&env, journal.path(), ckpt.path(), port, &hb_path);

    // (1) Файл обязан появиться в течение нескольких периодов; бинарь при этом жив.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(HB_PERIOD_MS * 25);
    let mut first: Option<Value> = None;
    let mut broken = 0usize;
    while std::time::Instant::now() < deadline {
        if let Some(st) = child.inner.try_wait().expect("try_wait") {
            panic!(
                "SETUP НЕ СОСТОЯЛСЯ: прод-бинарь завершился с {st:?} ещё до первого \
                 сердцебиения — окружение из compose не поднимает сервер"
            );
        }
        match read_hb(&hb_path) {
            Hb::Whole(v) => {
                first = Some(v);
                break;
            }
            Hb::Broken(_) => broken += 1,
            Hb::Missing => {}
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let first = first.unwrap_or_else(|| {
        panic!(
            "I-6 / TD-220: прод-бинарь на окружении из compose НЕ НАПИСАЛ сердцебиение в {} за \
             {} мс (период {HB_PERIOD_MS} мс). Счётчики M-87 существуют только внутри процесса; \
             снаружи предохранитель срабатывает молча",
            hb_path.display(),
            HB_PERIOD_MS * 25
        )
    });
    for k in REQUIRED_KEYS {
        assert!(
            first.get(k).is_some(),
            "I-6: в сердцебиении нет поля `{k}` — форма §4.2 не соблюдена: {first}"
        );
    }
    assert_eq!(first["schema"], json!(1), "схема сердцебиения ≠ 1");
    let a0 = first["attempts"].as_u64().expect("attempts");
    let s0 = first["successes"].as_u64().expect("successes");

    // (2) Обслуженный запрос по прод-форме — селектор в пределах политики compose.
    let secret = env
        .get("GATEWAY_JWT_SECRET")
        .cloned()
        .unwrap_or_else(|| "probe-value".to_string());
    let url = format!("ws://127.0.0.1:{port}/?token={}", sign(&secret));
    let (mut ws, _) = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio_tungstenite::connect_async(url),
    )
    .await
    .expect("connect timeout")
    .expect("connect");
    let sub = json!({"op":"subscribe","v":1,"id":"s1","selector":{
        "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":1000,
        "bands":CANONICAL.to_vec(),"window_ms":60000}});
    ws.send(Message::Text(sub.to_string())).await.expect("send");
    let first_msg: Option<Value> =
        match tokio::time::timeout(std::time::Duration::from_secs(10), ws.next()).await {
            Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref()).ok(),
            _ => None,
        };
    match first_msg
        .as_ref()
        .and_then(|v| v.get("type"))
        .and_then(Value::as_str)
    {
        Some("snapshot") => {}
        Some(other) => {
            panic!("SETUP НЕ СОСТОЯЛСЯ: вместо snapshot пришло `{other}`: {first_msg:?}")
        }
        None => panic!("SETUP НЕ СОСТОЯЛСЯ: запрос не обслужен — рост счётчиков не наблюдаем"),
    }

    // (3) Ждём ≥ 2 периодов, наблюдая целостность файла при каждом чтении.
    let mut last: Option<Value> = None;
    for _ in 0..((HB_PERIOD_MS * 3) / 20) {
        match read_hb(&hb_path) {
            Hb::Whole(v) => last = Some(v),
            Hb::Broken(s) => {
                broken += 1;
                eprintln!("частичное чтение: {} байт", s.len());
            }
            Hb::Missing => broken += 1, // после первой записи файл обязан существовать всегда (rename)
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(
        broken, 0,
        "I-6: {broken} чтений застали файл частичным или отсутствующим — запись не атомарна \
         (обязана быть `<path>.tmp` → rename); читатель на хосте получит обрывок JSON"
    );
    let last = last.expect("хотя бы одно целое чтение после запроса");
    let a1 = last["attempts"].as_u64().expect("attempts");
    let s1 = last["successes"].as_u64().expect("successes");
    let b1 = last["journal_payload_bytes_read"].as_u64().expect("bytes");
    assert!(
        a1 > a0 && s1 > s0,
        "I-6 / OPS-I-10: после обслуженного запроса сердцебиение не выросло \
         (attempts {a0}→{a1}, successes {s0}→{s1}) — файл пишется, но не продюсером выдачи"
    );
    assert!(
        b1 > 0,
        "I-6: байты журнала в сердцебиении = 0 после обслуженного запроса"
    );
    drop(ws);
}
