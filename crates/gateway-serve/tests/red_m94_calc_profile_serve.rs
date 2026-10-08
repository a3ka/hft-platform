//! RED `M-94` (sacred, architect-only) — **сервер выдачи берёт определение расчёта из ТОГО ЖЕ
//! версионированного файла, что прогреватель, и сообщает применённую версию наружу процесса**
//! (`П-032` п.1, п.4 (а); спека `docs/archive/M-94-calc-profile.md`).
//!
//! # Как судится
//!
//! Прод-бинарь `gateway-serve` на окружении из `docker-compose.yml` (образец
//! `red_m89_heartbeat_entrypoint.rs`); путь профиля внутри контейнера отображается на файл
//! репозитория по строке `COPY config/calc-profile/ <dst>` финальной стадии `Dockerfile`. Профиль
//! разбирается здесь НЕЗАВИСИМО от загрузчика dev'а; `sha256` — командой `sha256sum`.
//! RUNTIME-RED: ни одного несуществующего символа (урок `A-033`).
//!
//! # Что важно знать про подписку
//!
//! Каденцию депт-серии в селекторе подписки задаёт КЛИЕНТ (`wire_v1::parse_selector` —
//! `serde` над `gateway::Selector`, поле `depth_cadence_ms` из JSON; нет поля ⇒ `None`). Слепок
//! прогревателя несёт `Some(cadence)` профиля. Поэтому `s1` подписывается С ЯВНОЙ каденцией —
//! иначе отпечаток расходится и `snapshot` недостижим при ЛЮБОЙ реализации профиля. Это факт
//! сегодняшнего контракта, а не предмет M-94 (он — в S1a «три контракта»).
//!
//! # Сегодня (`045fef9`) — красны по предмету
//!
//! `s1` — compose не объявляет `GATEWAY_CALC_PROFILE` (композиция). `s2`/`s3` задают путь профиля
//! НАПРЯМУЮ и падают по СВОЕМУ предмету: бинарь профиль игнорирует и стартует (процесс жив).

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
const HB_PERIOD_MS: u64 = 200;
const PROFILE_KEYS: [&str; 7] = [
    "GATEWAY_BANDS",
    "GATEWAY_DEPTH_CADENCE_MS",
    "GATEWAY_TIMEFRAME_MS",
    "GATEWAY_WINDOW_MS",
    "GATEWAY_ALLOWED_PROFILES",
    "GATEWAY_VP_BIN_WIDTH_E8",
    "GATEWAY_HEATMAP_WINDOW",
];
const CANONICAL_KEY: &str = "GATEWAY_CANONICAL_BANDS";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("корень репозитория")
        .to_path_buf()
}

fn active_profile_path() -> PathBuf {
    repo_root().join("config/calc-profile/active.env")
}

// ───────────────────────── профиль: НЕЗАВИСИМЫЙ разбор ─────────────────────────

fn active_profile_text() -> String {
    let p = active_profile_path();
    std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "I-2 / задача 7: нет файла профиля {} ({e}) — носитель не заведён",
            p.display()
        )
    })
}

fn parse_profile(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|t| !t.is_empty() && !t.starts_with('#'))
        .filter_map(|t| t.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn selector_from_profile(p: &BTreeMap<String, String>) -> Selector {
    let ms = |k: &str| -> i64 {
        p.get(k)
            .unwrap_or_else(|| panic!("SETUP: в профиле нет {k}"))
            .parse()
            .unwrap_or_else(|e| panic!("SETUP: {k} не число: {e}"))
    };
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: ms("GATEWAY_TIMEFRAME_MS"),
        bands: p["GATEWAY_BANDS"]
            .split(',')
            .map(|b| b.trim().parse::<f64>().expect("SETUP: полоса — число"))
            .collect(),
        window_ms: Some(ms("GATEWAY_WINDOW_MS")),
        depth_cadence_ms: Some(ms("GATEWAY_DEPTH_CADENCE_MS")),
    }
}

fn sha256_of(path: &Path) -> String {
    let o = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("SETUP: sha256sum недоступен");
    assert!(o.status.success(), "SETUP: sha256sum упал");
    String::from_utf8_lossy(&o.stdout)
        .split_whitespace()
        .next()
        .expect("SETUP: пустой вывод sha256sum")
        .to_string()
}

fn variant(edit: impl Fn(&mut Vec<String>)) -> String {
    let mut lines: Vec<String> = active_profile_text().lines().map(str::to_string).collect();
    edit(&mut lines);
    lines.join("\n") + "\n"
}

fn replace_key(lines: &mut [String], key: &str, value: &str) {
    let mut hit = false;
    for l in lines.iter_mut() {
        if l.trim_start().starts_with(&format!("{key}=")) {
            *l = format!("{key}={value}");
            hit = true;
        }
    }
    assert!(hit, "SETUP: в профиле нет строки {key}=");
}

// ───────────────────────── compose / Dockerfile ─────────────────────────

fn compose_text() -> String {
    let p = repo_root().join("docker-compose.yml");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("чтение {}: {e}", p.display()))
}

/// `${VAR:-default}` → `default`; `${VAR:?msg}` → проба (host `.env` = только секрет).
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

fn compose_env() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let (mut inside, mut in_env) = (false, false);
    for line in compose_text().lines() {
        if line.starts_with("  gateway-serve:") {
            inside = true;
            continue;
        }
        if inside && line.starts_with("  ") && !line.starts_with("    ") {
            break;
        }
        if !inside {
            continue;
        }
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
        if let Some((k, v)) = t.split_once(':') {
            if k.trim().starts_with("GATEWAY_") {
                out.insert(k.trim().to_string(), subst_default(v.trim()));
            }
        }
    }
    assert!(
        !out.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: окружение gateway-serve в compose не разобрано"
    );
    out
}

fn dockerfile_profile_dst() -> String {
    let text = std::fs::read_to_string(repo_root().join("Dockerfile")).expect("Dockerfile");
    let lines: Vec<&str> = text.lines().collect();
    let last_from = lines
        .iter()
        .rposition(|l| l.trim_start().to_ascii_uppercase().starts_with("FROM "))
        .expect("SETUP: в Dockerfile нет FROM");
    let toks = lines[last_from..]
        .iter()
        .map(|l| l.trim())
        .filter(|l| l.to_ascii_uppercase().starts_with("COPY "))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|t| {
            t.iter()
                .any(|x| x.trim_end_matches('/') == "config/calc-profile")
        })
        .unwrap_or_else(|| {
            panic!(
                "I-5 / задача 4: финальная стадия Dockerfile не доставляет config/calc-profile/ — \
                 файл профиля процессу выдачи недоступен"
            )
        });
    toks.last().unwrap().trim_end_matches('/').to_string()
}

/// Окружение выдачи из compose, где путь профиля отображён на файл репозитория (композиция, `s1`).
fn serve_env_with_profile() -> BTreeMap<String, String> {
    let mut env = compose_env();
    let container = env.get("GATEWAY_CALC_PROFILE").cloned().unwrap_or_else(|| {
        panic!(
            "I-5 / задача 4: docker-compose.yml не объявляет GATEWAY_CALC_PROFILE у gateway-serve \
             — выдача не в режиме профиля и берёт определение расчёта из env/дефолтов (TD-227)"
        )
    });
    let dst = dockerfile_profile_dst();
    let rel = container
        .strip_prefix(&format!("{dst}/"))
        .unwrap_or_else(|| {
            panic!("I-5: GATEWAY_CALC_PROFILE={container} вне каталога образа `{dst}`")
        });
    let mapped = repo_root().join("config/calc-profile").join(rel);
    assert!(
        mapped.exists(),
        "I-5: {container} отображается на {} — файла нет",
        mapped.display()
    );
    env.insert(
        "GATEWAY_CALC_PROFILE".to_string(),
        mapped.display().to_string(),
    );
    env
}

/// Окружение выдачи из compose с путём профиля, заданным НАПРЯМУЮ (без отображения через compose):
/// `s2`/`s3` судят ПОВЕДЕНИЕ бинаря в режиме профиля независимо от композиции, которую судит `s1`
/// (иначе все три падали бы по одной причине, и отказ самого бинаря оставался бы недоказанным).
fn serve_env_direct(profile: &Path) -> BTreeMap<String, String> {
    let mut env = compose_env();
    env.insert(
        "GATEWAY_CALC_PROFILE".to_string(),
        profile.display().to_string(),
    );
    env
}

// ───────────────────────── процесс ─────────────────────────

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

struct Child {
    inner: std::process::Child,
}
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.inner.kill();
        let _ = self.inner.wait();
    }
}

fn spawn(
    env: &BTreeMap<String, String>,
    journal: &Path,
    ckpt: &Path,
    port: u16,
    hb: &Path,
) -> Child {
    let bin = bin_path();
    assert!(
        bin.exists(),
        "SETUP-СТРАЖ: прод-бинарь не собран ({})",
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
    cmd.env("GATEWAY_HEARTBEAT_PATH", hb.display().to_string());
    cmd.env("GATEWAY_HEARTBEAT_PERIOD_MS", HB_PERIOD_MS.to_string());
    let inner = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("запуск прод-бинаря");
    Child { inner }
}

/// Ждать выхода до `secs`; `Some((code, stderr))` — вышел, `None` — жив (и убит).
fn wait_exit(mut child: Child, secs: u64) -> Option<(Option<i32>, String)> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        if let Some(st) = child.inner.try_wait().expect("try_wait") {
            let mut err = String::new();
            if let Some(mut e) = child.inner.stderr.take() {
                use std::io::Read;
                let _ = e.read_to_string(&mut err);
            }
            return Some((st.code(), err));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    None
}

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 30,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m94".to_string(),
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

#[derive(serde::Serialize)]
struct Claims {
    sub: String,
    exp: usize,
}

fn sign(secret: &str) -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            sub: "m94".to_string(),
            exp: 4_000_000_000,
        },
        &EncodingKey::from_secret(&gateway_serve::auth::key_material(secret)),
    )
    .expect("jwt")
}

fn read_hb(path: &Path) -> Option<Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(Value::is_object)
}

// ───────────────────────── оракулы ─────────────────────────

/// **`s1` — прод-бинарь на окружении compose стартует из профиля, сообщает его версию и `sha256`
/// в сердцебиении и обслуживает подписку на слепке, записанном по ТОМУ ЖЕ профилю.**
#[tokio::test]
async fn s1_prod_binary_serves_from_profile_and_reports_its_version() {
    let env = serve_env_with_profile();
    let prof = parse_profile(&active_profile_text());
    let sel = selector_from_profile(&prof);

    let journal = tempfile::tempdir().unwrap();
    append(journal.path(), 0, 300);
    let ckpt = tempfile::tempdir().unwrap();
    gateway::checkpoint::advance(
        journal.path(),
        ckpt.path(),
        &sel,
        EpochFilter::OwnCaptureOnly,
    )
    .expect("advance");
    append(journal.path(), 300, 40);

    let state = tempfile::tempdir().unwrap();
    let hb = state.path().join("gateway-serve.heartbeat");
    let port = free_port();
    let mut child = spawn(&env, journal.path(), ckpt.path(), port, &hb);

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(HB_PERIOD_MS * 25);
    let mut first: Option<Value> = None;
    while std::time::Instant::now() < deadline {
        if let Some(st) = child.inner.try_wait().expect("try_wait") {
            let mut err = String::new();
            if let Some(mut e) = child.inner.stderr.take() {
                use std::io::Read;
                let _ = e.read_to_string(&mut err);
            }
            panic!(
                "I-1 / задача 3: прод-бинарь с профилем из compose завершился {st:?} до первого \
                 сердцебиения — выдача не стартует из профиля: {}",
                err.lines().last().unwrap_or("")
            );
        }
        if let Some(v) = read_hb(&hb) {
            first = Some(v);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let first = first.expect("I-4 (а): сердцебиение не появилось");
    let want_ver: u64 = prof["CALC_PROFILE_VERSION"].parse().expect("версия");
    assert_eq!(
        first["calc_profile"]["version"].as_u64(),
        Some(want_ver),
        "I-4 (а): сердцебиение не сообщает применённую версию профиля: {first}"
    );
    assert_eq!(
        first["calc_profile"]["sha256"].as_str(),
        Some(sha256_of(&active_profile_path()).as_str()),
        "I-4 (а): sha256 в сердцебиении ≠ sha256 байтов файла профиля: {first}"
    );
    assert_eq!(
        first["schema"],
        json!(1),
        "I-4 (а): поле аддитивно — схема сердцебиения остаётся 1"
    );

    let url = format!("ws://127.0.0.1:{port}/?token={}", sign("probe-value"));
    let (mut ws, _) = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio_tungstenite::connect_async(url),
    )
    .await
    .expect("connect timeout")
    .expect("connect");
    let sub = json!({"op":"subscribe","v":1,"id":"s1","selector":{
        "venue":"Binance","symbol":"BTCUSDT","timeframe_ms":sel.timeframe_ms,
        "bands":sel.bands,"window_ms":sel.window_ms,"depth_cadence_ms":sel.depth_cadence_ms}});
    ws.send(Message::Text(sub.to_string())).await.expect("send");
    let msg: Option<Value> =
        match tokio::time::timeout(std::time::Duration::from_secs(10), ws.next()).await {
            Ok(Some(Ok(m))) => serde_json::from_slice(m.into_data().as_ref()).ok(),
            _ => None,
        };
    assert_eq!(
        msg.as_ref()
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str),
        Some("snapshot"),
        "I-2: подписка канонического селектора профиля на слепке, записанном по профилю, не \
         получила snapshot: {msg:?}"
    );
}

/// **`s2` — «наличие — отказ»:** каждый из восьми ключей в окружении выдачи при заданном профиле
/// (значение РАВНО профилю) ⇒ процесс завершается `≠ 0`, называя ключ и `GATEWAY_CALC_PROFILE`.
#[test]
fn s2_profile_key_in_env_refuses_start() {
    let base = serve_env_direct(&active_profile_path());
    let prof = parse_profile(&active_profile_text());
    let journal = tempfile::tempdir().unwrap();
    append(journal.path(), 0, 10);
    let mut failures = Vec::new();
    for k in PROFILE_KEYS.iter().chain(std::iter::once(&CANONICAL_KEY)) {
        let v = if *k == CANONICAL_KEY {
            prof["GATEWAY_BANDS"].clone()
        } else {
            prof[*k].clone()
        };
        let mut env = base.clone();
        env.insert(k.to_string(), v.clone());
        let ckpt = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let child = spawn(
            &env,
            journal.path(),
            ckpt.path(),
            free_port(),
            &state.path().join("hb"),
        );
        match wait_exit(child, 15) {
            None => failures.push(format!("{k}={v}: процесс ЖИВ через 15 с — ключ принят")),
            Some((code, err)) => {
                if code == Some(0) || !(err.contains(k) && err.contains("GATEWAY_CALC_PROFILE")) {
                    failures.push(format!(
                        "{k}={v}: exit {code:?}, stderr: {}",
                        err.lines().last().unwrap_or("")
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "I-1: ключ профиля в окружении выдачи НЕ отвергнут (П-032 п.1: наличие — отказ):\n{}",
        failures.join("\n")
    );
}

/// **`s3` — профиль замкнут у выдачи тем же загрузчиком:** испорченный профиль ⇒ отказ старта с
/// именем ключа.
#[test]
fn s3_broken_profile_refuses_start() {
    let tmp = tempfile::tempdir().unwrap();
    let journal = tempfile::tempdir().unwrap();
    append(journal.path(), 0, 10);
    type Edit = Box<dyn Fn(&mut Vec<String>)>;
    let cases: Vec<(&str, &str, Edit)> = vec![
        (
            "неизвестный ключ",
            "GATEWAY_FOO",
            Box::new(|l: &mut Vec<String>| l.push("GATEWAY_FOO=1".into())),
        ),
        (
            "нет ключа",
            "GATEWAY_VP_BIN_WIDTH_E8",
            Box::new(|l: &mut Vec<String>| {
                l.retain(|x| !x.trim_start().starts_with("GATEWAY_VP_BIN_WIDTH_E8="))
            }),
        ),
        (
            "дубль",
            "GATEWAY_WINDOW_MS",
            Box::new(|l: &mut Vec<String>| l.push("GATEWAY_WINDOW_MS=60000".into())),
        ),
        (
            "тройка вне ALLOWED_PROFILES",
            "GATEWAY_ALLOWED_PROFILES",
            Box::new(|l: &mut Vec<String>| {
                replace_key(l, "GATEWAY_ALLOWED_PROFILES", "1000/300000/1000")
            }),
        ),
    ];
    let mut failures = Vec::new();
    for (i, (what, key, edit)) in cases.iter().enumerate() {
        let p = tmp.path().join(format!("bad{i}.env"));
        std::fs::write(&p, variant(edit)).unwrap();
        let env = serve_env_direct(&p);
        let ckpt = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let child = spawn(
            &env,
            journal.path(),
            ckpt.path(),
            free_port(),
            &state.path().join("hb"),
        );
        match wait_exit(child, 15) {
            None => failures.push(format!("{what}: процесс ЖИВ через 15 с")),
            Some((code, err)) => {
                if code == Some(0) || !err.contains(key) {
                    failures.push(format!(
                        "{what}: exit {code:?}, `{key}` назван: {}",
                        err.contains(key)
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "I-3: испорченный профиль принят выдачей:\n{}",
        failures.join("\n")
    );
}
