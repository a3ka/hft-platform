//! RED `M-94` (sacred, architect-only) — **профиль расчётов: прогреватель берёт ВСЁ определение
//! расчёта из ОДНОГО версионированного файла репозитория** (`П-032`, спека
//! `docs/archive/M-94-calc-profile.md`).
//!
//! # Почему
//!
//! У величины «полосы» на `045fef9` ТРИ носителя: host `.env` прода (`GATEWAY_BANDS=…`), дефолт
//! compose `${GATEWAY_BANDS:-0.001}` и дефолт в коде допуска (`GATEWAY_CANONICAL_BANDS`). Восемь
//! суток отказа выдачи (`TD-227`) — расхождение двух из них. `П-032` подписал носитель:
//! `config/calc-profile/active.env`, читаемый ОБОИМИ процессами, считающими выдачу.
//!
//! # Как судится — RUNTIME-RED (урок `A-033`)
//!
//! Ни одного несуществующего символа: несобирающийся тестовый бинарь глушит ВЕСЬ корпус крейта.
//! Судятся границы процессов — НАСТОЯЩИЙ `gateway-checkpoint`, НАСТОЯЩИЙ скрипт cron'а, тексты
//! `docker-compose.yml` / `Dockerfile` / `deploy/cron.d/*`. Профиль разбирается здесь
//! НЕЗАВИСИМО от загрузчика dev'а (эталон из независимого пути); `sha256` — командой `sha256sum`.
//!
//! # Моделируется и где предел (как `M-90`)
//!
//! Интерполяция compose `${VAR:-d}` и семантика `docker compose run [opts] SERVICE [ARGS]`
//! (`-e K=V` до имени сервиса — окружение; ARGS непусты ⇒ замена `command:`) воспроизведены, а не
//! взяты из `docker compose config` — в CI докера нет. Путь профиля внутри контейнера
//! отображается на файл репозитория по строке `COPY config/calc-profile/ <dst>` ФИНАЛЬНОЙ стадии
//! `Dockerfile` — это и есть доставка (`testing.md` §«канарейка» п.3).
//!
//! # Сегодня (`045fef9`) — красны по предмету
//!
//! `c1`/`p1`/`p2` — compose не объявляет `GATEWAY_CALC_PROFILE`; `p3`…`p7`, `p9` — прогреватель
//! игнорирует профиль (env и флаги принимаются, `.profile` нет, `--print-ckpt-name` неизвестен);
//! `p8` — cron не делает второго прогона. `c2` — зелёный сторож.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{LiveReducer, Selector};
use journal::{EpochFilter, Journal, WriterConfig};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_gateway-checkpoint");
const N: u64 = 300;
const SERVICE: &str = "gateway-checkpoint";
/// Имя слепка, который ищет прод-выдача и пишет прод-прогреватель (`ls` тома 2026-10-05; оно же
/// посчитано `M-90` тем же кодом для прод-селектора семи полос).
const PROD_CKPT_NAME: &str = "ckpt-8f69809dd707e8c9.bin";
/// Семь величин профиля под прежними именами (`П-032` п.2).
const PROFILE_KEYS: [&str; 7] = [
    "GATEWAY_BANDS",
    "GATEWAY_DEPTH_CADENCE_MS",
    "GATEWAY_TIMEFRAME_MS",
    "GATEWAY_WINDOW_MS",
    "GATEWAY_ALLOWED_PROFILES",
    "GATEWAY_VP_BIN_WIDTH_E8",
    "GATEWAY_HEATMAP_WINDOW",
];
/// Третий носитель полос (дефолт допуска) — выводится из оборота.
const CANONICAL_KEY: &str = "GATEWAY_CANONICAL_BANDS";
const AXIS_FLAGS: [(&str, &str); 4] = [
    ("--bands", "0.015,0.03,0.05,0.08,0.15,0.3,0.6"),
    ("--timeframe-ms", "1000"),
    ("--window-ms", "60000"),
    ("--depth-cadence-ms", "1000"),
];
const CHECKPOINT_SELECTOR_VARS: [&str; 6] = [
    "CHECKPOINT_VENUE",
    "CHECKPOINT_SYMBOL",
    "CHECKPOINT_TIMEFRAME_MS",
    "CHECKPOINT_BANDS",
    "CHECKPOINT_WINDOW_MS",
    "CHECKPOINT_DEPTH_CADENCE_MS",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn active_profile_path() -> PathBuf {
    repo_root().join("config/calc-profile/active.env")
}

// ───────────────────────── профиль: НЕЗАВИСИМЫЙ разбор ─────────────────────────

/// `KEY=VALUE` без комментариев и пустых строк. Повтор ключа здесь — дефект ФИКСТУРЫ.
fn parse_profile_text(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for l in text.lines() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (k, v) = t
            .split_once('=')
            .unwrap_or_else(|| panic!("SETUP: строка профиля без `=`: {t}"));
        assert!(
            out.insert(k.trim().to_string(), v.trim().to_string())
                .is_none(),
            "SETUP: ключ {k} повторён в фикстуре профиля"
        );
    }
    out
}

fn active_profile() -> (String, BTreeMap<String, String>) {
    let p = active_profile_path();
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "I-2 / задача 7: нет файла профиля {} ({e}) — носитель определения расчёта не заведён",
            p.display()
        )
    });
    let map = parse_profile_text(&text);
    (text, map)
}

fn sha256_of(path: &Path) -> String {
    let o = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("SETUP: sha256sum недоступен");
    assert!(
        o.status.success(),
        "SETUP: sha256sum {} упал",
        path.display()
    );
    String::from_utf8_lossy(&o.stdout)
        .split_whitespace()
        .next()
        .expect("SETUP: пустой вывод sha256sum")
        .to_string()
}

/// Селектор СЕГОДНЯШНЕГО прод-окружения (замер `docker inspect hft-gateway-serve` 2026-10-05) —
/// эталон `П-032` п.8 «имя при тех же значениях не меняется». Собран ЛИТЕРАЛАМИ, не из профиля.
fn legacy_prod_selector() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1000,
        bands: vec![0.015, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6],
        window_ms: Some(60_000),
        depth_cadence_ms: Some(1000),
    }
}

/// Селектор, разобранный ИЗ ПРОФИЛЯ — независимо от загрузчика dev'а.
fn selector_from_profile(p: &BTreeMap<String, String>) -> Selector {
    let get = |k: &str| {
        p.get(k)
            .cloned()
            .unwrap_or_else(|| panic!("SETUP: в профиле нет {k}"))
    };
    let ms = |k: &str| {
        get(k)
            .parse::<i64>()
            .unwrap_or_else(|e| panic!("SETUP: {k} не число: {e}"))
    };
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: ms("GATEWAY_TIMEFRAME_MS"),
        bands: get("GATEWAY_BANDS")
            .split(',')
            .map(|b| b.trim().parse::<f64>().expect("SETUP: полоса — число"))
            .collect(),
        window_ms: Some(ms("GATEWAY_WINDOW_MS")),
        depth_cadence_ms: Some(ms("GATEWAY_DEPTH_CADENCE_MS")),
    }
}

// ───────────────────────── compose / Dockerfile ─────────────────────────

fn interpolate(s: &str, env: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find('}').expect("незакрытая ${...}") + start;
        let inner = &rest[start + 2..end];
        let (name, default) = match inner.split_once(":-") {
            Some((n, d)) => (n, d),
            None => match inner.split_once(":?") {
                Some((n, _)) => (n, "probe-value"),
                None => (inner, ""),
            },
        };
        match env.get(name).filter(|v| !v.is_empty()) {
            Some(v) => out.push_str(v),
            None => out.push_str(default),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn compose_text() -> String {
    std::fs::read_to_string(repo_root().join("docker-compose.yml")).expect("docker-compose.yml")
}

/// Имена сервисов верхнего уровня `services:`.
fn compose_services() -> Vec<String> {
    let text = compose_text();
    let mut out = Vec::new();
    let mut in_services = false;
    for line in text.lines() {
        if !line.starts_with(' ') && !line.trim().is_empty() && !line.starts_with('#') {
            in_services = line.trim_end() == "services:";
            continue;
        }
        if in_services
            && line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim_start().starts_with('#')
        {
            out.push(line.trim().trim_end_matches(':').to_string());
        }
    }
    assert!(
        out.len() >= 3,
        "SETUP НЕ СОСТОЯЛСЯ: сервисы compose не разобраны ({out:?})"
    );
    out
}

/// Строки блока `<block>:` сервиса (без комментариев).
fn compose_block(service: &str, block: &str) -> Vec<String> {
    let text = compose_text();
    let mut out = Vec::new();
    let (mut in_service, mut in_block) = (false, false);
    for line in text.lines() {
        if line.starts_with("  ") && !line.starts_with("    ") && line.trim_end().ends_with(':') {
            in_service = line.trim().trim_end_matches(':') == service;
            in_block = false;
            continue;
        }
        if !in_service {
            continue;
        }
        let t = line.trim();
        if line.starts_with("    ") && !line.starts_with("      ") && t.ends_with(':') {
            in_block = t == format!("{block}:");
            continue;
        }
        if in_block && !t.is_empty() && !t.starts_with('#') {
            if line.starts_with("    ") && !line.starts_with("      ") {
                in_block = false;
                continue;
            }
            out.push(t.to_string());
        }
    }
    out
}

fn compose_command(service: &str, env: &BTreeMap<String, String>) -> Vec<String> {
    compose_block(service, "command")
        .iter()
        .filter_map(|l| l.strip_prefix("- "))
        .map(|i| interpolate(i.trim().trim_matches('"'), env))
        .collect()
}

fn compose_env(service: &str, env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    compose_block(service, "environment")
        .iter()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| {
            (
                k.trim().to_string(),
                interpolate(v.trim().trim_matches('"'), env),
            )
        })
        .collect()
}

/// Каталог ВНУТРИ образа, куда финальная стадия `Dockerfile` кладёт `config/calc-profile/`.
fn dockerfile_profile_dst() -> String {
    let text = std::fs::read_to_string(repo_root().join("Dockerfile")).expect("Dockerfile");
    let lines: Vec<&str> = text.lines().collect();
    let last_from = lines
        .iter()
        .rposition(|l| l.trim_start().to_ascii_uppercase().starts_with("FROM "))
        .expect("SETUP НЕ СОСТОЯЛСЯ: в Dockerfile нет FROM");
    let copy = lines[last_from..]
        .iter()
        .map(|l| l.trim())
        .filter(|l| l.to_ascii_uppercase().starts_with("COPY "))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|t| {
            t.iter()
                .any(|x| x.trim_end_matches('/') == "config/calc-profile")
        });
    let toks = copy.unwrap_or_else(|| {
        panic!(
            "I-5 / задача 4: финальная стадия Dockerfile не доставляет `config/calc-profile/` в \
             образ — профиль, лежащий в репозитории, процессу недоступен (файл в репо при деплое, \
             который его не устанавливает, инертен — `testing.md` §«канарейка» п.3)"
        )
    });
    toks.last()
        .expect("COPY без назначения")
        .trim_end_matches('/')
        .to_string()
}

/// Путь профиля внутри контейнера (из compose) → файл репозитория (через COPY `Dockerfile`).
fn map_container_profile(container_path: &str) -> PathBuf {
    let dst = dockerfile_profile_dst();
    let rel = container_path
        .strip_prefix(&format!("{dst}/"))
        .unwrap_or_else(|| {
            panic!(
                "I-5: GATEWAY_CALC_PROFILE={container_path} не лежит в каталоге `{dst}`, куда \
                 Dockerfile кладёт профиль — процесс прочтёт файл, которого в образе нет"
            )
        });
    let p = repo_root().join("config/calc-profile").join(rel);
    assert!(
        p.exists(),
        "I-5: GATEWAY_CALC_PROFILE={container_path} отображается на {} — такого файла в \
         репозитории нет",
        p.display()
    );
    p
}

/// Путь профиля, объявленный сервисом compose (host `.env` = только секрет — прод-форма после M-94).
fn declared_profile(service: &str) -> String {
    let env = compose_env(service, &secret_only_dotenv());
    env.get("GATEWAY_CALC_PROFILE").cloned().unwrap_or_else(|| {
        panic!(
            "I-5 / задача 4: docker-compose.yml не объявляет GATEWAY_CALC_PROFILE у сервиса \
             `{service}` — процесс не в режиме профиля и берёт полосы из env/дефолтов (класс TD-227)"
        )
    })
}

fn secret_only_dotenv() -> BTreeMap<String, String> {
    BTreeMap::from([("GATEWAY_JWT_SECRET".to_string(), "x".to_string())])
}

// ───────────────────────── журнал ─────────────────────────

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 8 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(100.0 + (i % 5) as f64),
            size: to_fixed(1.0),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: 1_752_000_000_000 + i as i64 * 100,
        },
    )
}

fn journal_of(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..n {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    dir
}

fn slepok_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("ckpt-") && n.ends_with(".bin"))
        .collect();
    v.sort();
    v
}

// ───────────────────────── прогон прогревателя напрямую ─────────────────────────

struct WarmerRun {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Прогреватель в режиме профиля: окружение — только `PATH` + `GATEWAY_CALC_PROFILE` + `extra_env`.
fn run_warmer(
    profile: &Path,
    journal: &Path,
    ckpt: &Path,
    extra_args: &[&str],
    extra_env: &[(&str, &str)],
) -> WarmerRun {
    let mut cmd = Command::new(BIN);
    cmd.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GATEWAY_CALC_PROFILE", profile)
        .arg(format!("--dir={}", journal.display()))
        .arg(format!("--ckpt-dir={}", ckpt.display()))
        .arg(format!(
            "--coverage-out={}",
            ckpt.join("covered_through_seq").display()
        ));
    for a in extra_args {
        cmd.arg(a);
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let o = cmd.output().expect("запуск gateway-checkpoint");
    WarmerRun {
        code: o.status.code(),
        stdout: String::from_utf8_lossy(&o.stdout).to_string(),
        stderr: String::from_utf8_lossy(&o.stderr).to_string(),
    }
}

fn write_profile_variant(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

/// Активный профиль с заменой/удалением/добавлением строк — для испорченных и иных миров.
fn variant(edit: impl Fn(&mut Vec<String>)) -> String {
    let (text, _) = active_profile();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
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

// ───────────────────────── прод-путь cron → runner ─────────────────────────

fn cron_env() -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(repo_root().join("deploy/cron.d/journal-retention"))
        .expect("deploy/cron.d/journal-retention");
    let env: BTreeMap<String, String> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    assert!(
        env.contains_key("PATH"),
        "SETUP НЕ СОСТОЯЛСЯ: окружение cron'а не разобрано"
    );
    assert!(
        text.contains("gateway-checkpoint-cron.sh"),
        "SETUP НЕ СОСТОЯЛСЯ: прод-cron больше не зовёт gateway-checkpoint-cron.sh"
    );
    env
}

/// Настоящий скрипт cron'а; `docker` — заглушка, ДОПИСЫВАЮЩАЯ argv каждого вызова.
/// Возвращает argv всех вызовов runner'а в порядке вызова.
fn run_cron_calls(root: &Path) -> (Option<i32>, String, Vec<Vec<String>>) {
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let shim = work.join("docker");
    let out = work.join("runner.calls");
    std::fs::write(
        &shim,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$M94_RUNNER_OUT\"\necho '--M94-CALL-END--' >> \"$M94_RUNNER_OUT\"\necho 'gateway-checkpoint: ok (shim)'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut cmd = Command::new("bash");
    cmd.arg(repo_root().join("deploy/bin/gateway-checkpoint-cron.sh"))
        .env_clear()
        .envs(cron_env())
        .env("HFT_ROOT", root)
        .env(
            "CHECKPOINT_RUNNER",
            format!("{} compose run --rm {SERVICE}", shim.display()),
        )
        .env("CHECKPOINT_LOG", work.join("ckpt.log"))
        .env("CHECKPOINT_ALERT_FILE", work.join("ckpt.alert"))
        .env("CHECKPOINT_LAST_SUCCESS", work.join("ckpt.last"))
        .env("M94_RUNNER_OUT", &out);
    let o = cmd.output().expect("запуск gateway-checkpoint-cron.sh");
    let calls: Vec<Vec<String>> = std::fs::read_to_string(&out)
        .unwrap_or_default()
        .split("--M94-CALL-END--\n")
        .filter(|c| !c.trim().is_empty())
        .map(|c| c.lines().map(str::to_string).collect())
        .collect();
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stderr).to_string(),
        calls,
    )
}

/// Разбор вызова `compose run [opts] SERVICE [ARGS]`: (окружение из `-e`, эффективный argv).
fn split_compose_run(call: &[String]) -> (BTreeMap<String, String>, Vec<String>) {
    assert_eq!(
        call.first().map(String::as_str),
        Some("compose"),
        "SETUP НЕ СОСТОЯЛСЯ: заглушка получила не `compose …`: {call:?}"
    );
    let pos = call.iter().position(|a| a == SERVICE).unwrap_or_else(|| {
        panic!("SETUP НЕ СОСТОЯЛСЯ: runner позван без имени сервиса `{SERVICE}`: {call:?}")
    });
    let mut env = BTreeMap::new();
    let opts = &call[..pos];
    let mut i = 0;
    while i < opts.len() {
        let a = &opts[i];
        let kv = if a == "-e" || a == "--env" {
            i += 1;
            opts.get(i).cloned()
        } else {
            a.strip_prefix("--env=")
                .or_else(|| a.strip_prefix("-e="))
                .map(str::to_string)
        };
        if let Some(kv) = kv {
            if let Some((k, v)) = kv.split_once('=') {
                env.insert(k.to_string(), v.to_string());
            }
        }
        i += 1;
    }
    let args = call[pos + 1..].to_vec();
    let argv = if args.is_empty() {
        compose_command(SERVICE, &secret_only_dotenv())
    } else {
        args
    };
    (env, argv)
}

fn coverage_of(argv: &[String]) -> Option<String> {
    argv.iter().enumerate().find_map(|(i, a)| {
        a.strip_prefix("--coverage-out=")
            .map(str::to_string)
            .or_else(|| {
                (a == "--coverage-out")
                    .then(|| argv.get(i + 1).cloned())
                    .flatten()
            })
    })
}

fn retarget(args: &[String], journal: &Path, ckpt: &Path, cov: &Path) -> Vec<String> {
    let map = [
        ("--dir", journal),
        ("--ckpt-dir", ckpt),
        ("--coverage-out", cov),
    ];
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let hit = map
            .iter()
            .find(|(f, _)| a == f || a.starts_with(&format!("{f}=")));
        match hit {
            Some((f, p)) if a == f => {
                out.push(a.clone());
                out.push(p.display().to_string());
                i += 2;
                continue;
            }
            Some((f, p)) => out.push(format!("{f}={}", p.display())),
            None => out.push(a.clone()),
        }
        i += 1;
    }
    out
}

/// Прод-путь: host `.env` (только секрет) → cron → скрипт → `compose run` → НАСТОЯЩИЙ прогреватель
/// с окружением compose, где путь профиля отображён на файл репозитория.
fn warm_via_prod_path() -> (tempfile::TempDir, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("root");
    std::fs::write(root.path().join(".env"), "GATEWAY_JWT_SECRET=x\n").unwrap();
    let (code, stderr, calls) = run_cron_calls(root.path());
    assert_eq!(
        code,
        Some(0),
        "SETUP НЕ СОСТОЯЛСЯ: скрипт cron'а вышел {code:?}: {stderr}"
    );
    assert!(
        !calls.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: скрипт cron'а не позвал runner"
    );
    let (opt_env, argv) = split_compose_run(&calls[0]);
    let mut env = compose_env(SERVICE, &secret_only_dotenv());
    env.extend(opt_env);
    let container_profile = env.get("GATEWAY_CALC_PROFILE").cloned().unwrap_or_else(|| {
        panic!(
            "I-5 / задача 4: прогреватель прод-пути запущен БЕЗ GATEWAY_CALC_PROFILE (ни compose, \
             ни runner его не задают) — профиль не применяется"
        )
    });
    env.insert(
        "GATEWAY_CALC_PROFILE".to_string(),
        map_container_profile(&container_profile)
            .display()
            .to_string(),
    );
    env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());

    let journal = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = ckpt.path().join("covered_through_seq");
    let argv = retarget(&argv, journal.path(), ckpt.path(), &cov);
    let w = Command::new(BIN)
        .args(&argv)
        .env_clear()
        .envs(&env)
        .output()
        .expect("запуск gateway-checkpoint");
    assert_eq!(
        w.status.code(),
        Some(0),
        "I-1 / задача 2: прод-прогреватель (argv {argv:?}) вышел {:?}: {}",
        w.status.code(),
        String::from_utf8_lossy(&w.stderr)
    );
    (journal, ckpt)
}

// ───────────────────────── оракулы ─────────────────────────

/// **`c1` — композиция доставки.** Оба сервиса объявляют ОДИН путь профиля; финальная стадия
/// `Dockerfile` кладёт `config/calc-profile/` в его каталог; ни один сервис compose не несёт ключа
/// профиля в `environment:`/`command:` (второго носителя нет).
#[test]
fn c1_compose_declares_one_profile_path_delivered_by_image_and_no_profile_keys() {
    let serve = declared_profile("gateway-serve");
    let warm = declared_profile(SERVICE);
    assert_eq!(
        serve, warm,
        "I-5: выдача и прогреватель читают РАЗНЫЕ профили ({serve} против {warm}) — ровно класс TD-227"
    );
    let mapped = map_container_profile(&serve);
    assert_eq!(
        std::fs::canonicalize(&mapped).unwrap(),
        std::fs::canonicalize(active_profile_path()).unwrap(),
        "I-5: путь профиля в compose ({serve}) отображается не на active.env"
    );
    let mut hits = Vec::new();
    for svc in compose_services() {
        for line in compose_block(&svc, "environment")
            .into_iter()
            .chain(compose_block(&svc, "command"))
        {
            for k in PROFILE_KEYS.iter().chain(std::iter::once(&CANONICAL_KEY)) {
                if line.contains(k) {
                    hits.push(format!("{svc}: {line}"));
                }
            }
            for (f, _) in AXIS_FLAGS {
                if line
                    .trim_start_matches("- ")
                    .trim_matches('"')
                    .starts_with(f)
                {
                    hits.push(format!("{svc}: {line}"));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "I-1 / I-5: compose несёт величину профиля ВТОРЫМ носителем (П-032 п.1): {hits:#?}"
    );
}

/// **`c2` — сторож: `deploy/cron.d/*` не несут ни ключей профиля, ни `CHECKPOINT_*` селектора.**
#[test]
fn c2_cron_files_carry_no_profile_value() {
    let dir = repo_root().join("deploy/cron.d");
    let mut seen = 0usize;
    let mut hits = Vec::new();
    for e in std::fs::read_dir(&dir).expect("deploy/cron.d") {
        let p = e.unwrap().path();
        if !p.is_file() {
            continue;
        }
        seen += 1;
        let text = std::fs::read_to_string(&p).unwrap();
        for l in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
            for k in PROFILE_KEYS
                .iter()
                .chain(std::iter::once(&CANONICAL_KEY))
                .chain(CHECKPOINT_SELECTOR_VARS.iter())
            {
                if l.contains(&format!("{k}=")) {
                    hits.push(format!("{}: {l}", p.display()));
                }
            }
        }
    }
    assert!(seen > 0, "SETUP НЕ СОСТОЯЛСЯ: в deploy/cron.d нет файлов");
    assert!(
        hits.is_empty(),
        "I-5: cron несёт величину профиля (деплой ставит deploy/cron.d/* в /etc/cron.d): {hits:#?}"
    );
}

/// **`p1` — имя слепка при тех же значениях не меняется (`П-032` п.8).** Прод-путь cron'а с
/// профилем репозитория пишет РОВНО то имя, что вычисляется из сегодняшнего прод-окружения.
#[test]
fn p1_prod_path_writes_the_same_slepok_name_as_todays_prod_env() {
    let (_journal, ckpt) = warm_via_prod_path();
    let names = slepok_names(ckpt.path());
    let legacy = gateway::checkpoint::ckpt_path_for_pub(ckpt.path(), &legacy_prod_selector());
    let legacy_name = legacy.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(
        legacy_name, PROD_CKPT_NAME,
        "SETUP: эталон разошёлся с прод-именем — сменился toolchain или отпечаток (вне предмета)"
    );
    assert_eq!(
        names,
        vec![PROD_CKPT_NAME.to_string()],
        "I-2: прогреватель прод-пути с профилем записал {names:?} вместо {PROD_CKPT_NAME} — выдача \
         на проде после выкатки не найдёт слепок (холодная пересборка ≈ 23 мин, П-032 п.8)"
    );
}

/// **`p2` — сервер, читающий тот же файл, находит слепок прод-прогревателя.**
#[test]
fn p2_selector_from_the_same_profile_finds_the_warmer_slepok() {
    let (journal, ckpt) = warm_via_prod_path();
    let (_, prof) = active_profile();
    let sel = selector_from_profile(&prof);
    let (_r, found) = LiveReducer::resume(
        journal.path(),
        EpochFilter::OwnCaptureOnly,
        &sel,
        ckpt.path(),
    )
    .expect("resume по профилю");
    let mut other = sel.clone();
    other.bands = vec![0.002];
    let (_r2, ctrl) = LiveReducer::resume(
        journal.path(),
        EpochFilter::OwnCaptureOnly,
        &other,
        ckpt.path(),
    )
    .expect("resume контроля");
    assert!(
        ctrl.events_decoded > 0,
        "SETUP: свидетель не различает полосы (контроль декодировал {} событий)",
        ctrl.events_decoded
    );
    assert_eq!(
        found.events_decoded, 0,
        "I-2: селектор из профиля НЕ находит слепок прогревателя (декодировано {}) — у писателя и \
         читателя разные определения расчёта",
        found.events_decoded
    );
}

/// **`p3` — «наличие — отказ»:** каждый из восьми ключей в окружении при заданном профиле, со
/// значением, РАВНЫМ профилю, ⇒ отказ с именем ключа и `GATEWAY_CALC_PROFILE`; слепка нет.
#[test]
fn p3_profile_key_in_env_is_refused_even_when_equal() {
    let (_, prof) = active_profile();
    let journal = journal_of(N);
    let mut failures = Vec::new();
    for k in PROFILE_KEYS.iter().chain(std::iter::once(&CANONICAL_KEY)) {
        let v = if *k == CANONICAL_KEY {
            prof["GATEWAY_BANDS"].clone()
        } else {
            prof[*k].clone()
        };
        let ckpt = tempfile::tempdir().unwrap();
        let r = run_warmer(
            &active_profile_path(),
            journal.path(),
            ckpt.path(),
            &[],
            &[(k, &v)],
        );
        let named = r.stderr.contains(k) && r.stderr.contains("GATEWAY_CALC_PROFILE");
        if r.code == Some(0) || !named || !slepok_names(ckpt.path()).is_empty() {
            failures.push(format!(
                "{k}={v}: exit {:?}, слепков {:?}, stderr: {}",
                r.code,
                slepok_names(ckpt.path()),
                r.stderr.lines().last().unwrap_or("")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "I-1: ключ профиля в окружении прогревателя НЕ отвергнут (П-032 п.1: наличие — отказ):\n{}",
        failures.join("\n")
    );
}

/// **`p4` — флаг оси поверх профиля — отказ.**
#[test]
fn p4_axis_flag_over_profile_is_refused() {
    let journal = journal_of(N);
    let mut failures = Vec::new();
    for (f, v) in AXIS_FLAGS {
        let ckpt = tempfile::tempdir().unwrap();
        let arg = format!("{f}={v}");
        let r = run_warmer(
            &active_profile_path(),
            journal.path(),
            ckpt.path(),
            &[&arg],
            &[],
        );
        if r.code == Some(0) || !r.stderr.contains(f) || !slepok_names(ckpt.path()).is_empty() {
            failures.push(format!(
                "{arg}: exit {:?}, слепков {:?}",
                r.code,
                slepok_names(ckpt.path())
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "I-1: флаг оси принят поверх профиля — второй носитель (поведение M-90):\n{}",
        failures.join("\n")
    );
}

/// **`p5` — профиль замкнут и полон.** Десять испорченных профилей отвергаются с именем ключа;
/// валидная копия даёт прод-имя слепка (контроль).
#[test]
fn p5_profile_is_closed_and_complete() {
    let journal = journal_of(N);
    let tmp = tempfile::tempdir().unwrap();
    type Edit = Box<dyn Fn(&mut Vec<String>)>;
    let cases: Vec<(&str, &str, Edit)> = vec![
        (
            "неизвестный ключ",
            "GATEWAY_FOO",
            Box::new(|l: &mut Vec<String>| l.push("GATEWAY_FOO=1".into())),
        ),
        (
            "нет ключа",
            "GATEWAY_HEATMAP_WINDOW",
            Box::new(|l: &mut Vec<String>| {
                l.retain(|x| !x.trim_start().starts_with("GATEWAY_HEATMAP_WINDOW="))
            }),
        ),
        (
            "дубль",
            "GATEWAY_BANDS",
            Box::new(|l: &mut Vec<String>| l.push("GATEWAY_BANDS=0.015".into())),
        ),
        (
            "версия 0",
            "CALC_PROFILE_VERSION",
            Box::new(|l: &mut Vec<String>| replace_key(l, "CALC_PROFILE_VERSION", "0")),
        ),
        (
            "версия не число",
            "CALC_PROFILE_VERSION",
            Box::new(|l: &mut Vec<String>| replace_key(l, "CALC_PROFILE_VERSION", "v1")),
        ),
        (
            "каденция 999",
            "GATEWAY_DEPTH_CADENCE_MS",
            Box::new(|l: &mut Vec<String>| replace_key(l, "GATEWAY_DEPTH_CADENCE_MS", "999")),
        ),
        (
            "полосы-мусор",
            "GATEWAY_BANDS",
            Box::new(|l: &mut Vec<String>| replace_key(l, "GATEWAY_BANDS", "0.015,abc")),
        ),
        (
            "окно heatmap 1.5",
            "GATEWAY_HEATMAP_WINDOW",
            Box::new(|l: &mut Vec<String>| replace_key(l, "GATEWAY_HEATMAP_WINDOW", "1.5")),
        ),
        (
            "шаг VP 0",
            "GATEWAY_VP_BIN_WIDTH_E8",
            Box::new(|l: &mut Vec<String>| replace_key(l, "GATEWAY_VP_BIN_WIDTH_E8", "0")),
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
        let p = write_profile_variant(tmp.path(), &format!("bad{i}.env"), &variant(edit));
        let ckpt = tempfile::tempdir().unwrap();
        let r = run_warmer(&p, journal.path(), ckpt.path(), &[], &[]);
        if r.code == Some(0) || !r.stderr.contains(key) || !slepok_names(ckpt.path()).is_empty() {
            failures.push(format!(
                "{what}: exit {:?}, ключ `{key}` назван: {}, слепков {:?}",
                r.code,
                r.stderr.contains(key),
                slepok_names(ckpt.path())
            ));
        }
    }
    // Контроль: валидная копия принимается и даёт прод-имя.
    let good = write_profile_variant(tmp.path(), "good.env", &variant(|_| {}));
    let ckpt = tempfile::tempdir().unwrap();
    let r = run_warmer(&good, journal.path(), ckpt.path(), &[], &[]);
    let ctrl_ok =
        r.code == Some(0) && slepok_names(ckpt.path()) == vec![PROD_CKPT_NAME.to_string()];
    assert!(
        failures.is_empty() && ctrl_ok,
        "I-3: профиль не замкнут/не полон.\nиспорченные приняты:\n{}\nконтроль (валидная копия): \
         exit {:?}, слепки {:?} (ожидался {PROD_CKPT_NAME}); stderr: {}",
        failures.join("\n"),
        r.code,
        slepok_names(ckpt.path()),
        r.stderr.lines().last().unwrap_or("")
    );
}

/// **`p6` — применённая версия видна у слепка (`П-032` п.4 (б)).** После успеха —
/// `<слепок>.profile` с версией и `sha256` байтов файла; неуспешный прогон `.profile` не создаёт.
#[test]
fn p6_slepok_carries_applied_profile_version_and_sha() {
    let (_, prof) = active_profile();
    let journal = journal_of(N);
    let ckpt = tempfile::tempdir().unwrap();
    let r = run_warmer(
        &active_profile_path(),
        journal.path(),
        ckpt.path(),
        &[],
        &[],
    );
    let side = ckpt.path().join(format!("{PROD_CKPT_NAME}.profile"));
    let body = std::fs::read_to_string(&side).unwrap_or_else(|e| {
        panic!(
            "I-4 (б): после прогона (exit {:?}) нет {} ({e}) — по какому определению посчитан \
             слепок, восстановить нельзя (план §15.5)",
            r.code,
            side.display()
        )
    });
    let v: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("I-4 (б): `.profile` не JSON ({e}): {body}"));
    let want_ver: u64 = prof["CALC_PROFILE_VERSION"].parse().expect("версия");
    assert_eq!(
        v["version"].as_u64(),
        Some(want_ver),
        "I-4 (б): версия в `.profile` ≠ CALC_PROFILE_VERSION файла: {body}"
    );
    assert_eq!(
        v["sha256"].as_str(),
        Some(sha256_of(&active_profile_path()).as_str()),
        "I-4 (б): sha256 в `.profile` ≠ sha256 БАЙТОВ файла профиля: {body}"
    );
    // Неуспешный прогон: журнал — не каталог.
    let bad_journal = tempfile::NamedTempFile::new().unwrap();
    let ckpt2 = tempfile::tempdir().unwrap();
    let r2 = run_warmer(
        &active_profile_path(),
        bad_journal.path(),
        ckpt2.path(),
        &[],
        &[],
    );
    assert_ne!(
        r2.code,
        Some(0),
        "SETUP: прогон на не-каталоге журнала не упал"
    );
    let leaked: Vec<String> = std::fs::read_dir(ckpt2.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".profile"))
        .collect();
    assert!(
        leaked.is_empty(),
        "I-4 (б): неуспешный прогон оставил {leaked:?} — `.profile` обязан писаться ПОСЛЕ слепка"
    );
}

/// **`p7` — `--print-ckpt-name` печатает имя без журнала** (опора гейта деплоя).
#[test]
fn p7_print_ckpt_name_without_journal() {
    let tmp = tempfile::tempdir().unwrap();
    let r = run_warmer(
        &active_profile_path(),
        &tmp.path().join("нет-такого-журнала"),
        tmp.path(),
        &["--print-ckpt-name"],
        &[],
    );
    assert_eq!(
        (r.code, r.stdout.trim()),
        (Some(0), PROD_CKPT_NAME),
        "I-8: `--print-ckpt-name` не напечатал имя слепка профиля (stderr: {})",
        r.stderr.lines().last().unwrap_or("")
    );
    assert!(
        slepok_names(tmp.path()).is_empty(),
        "I-8: `--print-ckpt-name` записал слепок — печать имени обязана быть без прогона"
    );
    // ГОЛАЯ форма — так её зовёт гейт деплоя (`compose run … gateway-checkpoint --print-ckpt-name`:
    // ARGS непусты ⇒ `command:` заменён целиком, путей нет; `A-049` примечание к `p7`).
    let o = Command::new(BIN)
        .arg("--print-ckpt-name")
        .current_dir(tmp.path())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GATEWAY_CALC_PROFILE", active_profile_path())
        .output()
        .expect("запуск gateway-checkpoint");
    assert_eq!(
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        ),
        (Some(0), PROD_CKPT_NAME.to_string()),
        "I-8: голая форма `--print-ckpt-name` (как её зовёт гейт деплоя) не напечатала имя: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

/// **`p8` — прогрев `next` заранее (`П-032` п.5).** С `next.env` cron делает второй прогон с
/// `GATEWAY_CALC_PROFILE` на `next.env` и покрытием НЕ в путь ретеншена; без `next.env` — один прогон.
#[test]
fn p8_cron_prewarms_next_profile_with_separate_coverage() {
    // Без next.env — ровно один вызов (поведение сегодняшнего дня сохранено).
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".env"), "GATEWAY_JWT_SECRET=x\n").unwrap();
    let (code, err, calls) = run_cron_calls(root.path());
    assert_eq!(code, Some(0), "SETUP: cron без next вышел {code:?}: {err}");
    assert_eq!(
        calls.len(),
        1,
        "I-7: без next.env runner позван {} раз, ожидался один",
        calls.len()
    );
    let (_, active_argv) = split_compose_run(&calls[0]);
    let active_cov = coverage_of(&active_argv).expect("SETUP: у прогона нет --coverage-out");

    // С next.env в каталоге профиля прод-чекаута.
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".env"), "GATEWAY_JWT_SECRET=x\n").unwrap();
    let pdir = root.path().join("config/calc-profile");
    std::fs::create_dir_all(&pdir).unwrap();
    std::fs::copy(active_profile_path(), pdir.join("active.env")).unwrap();
    std::fs::write(
        pdir.join("next.env"),
        variant(|l| {
            replace_key(l, "CALC_PROFILE_VERSION", "2");
            replace_key(l, "GATEWAY_BANDS", "0.015,0.03,0.05,0.08,0.15,0.3");
        }),
    )
    .unwrap();
    let (code, err, calls) = run_cron_calls(root.path());
    assert_eq!(code, Some(0), "SETUP: cron с next вышел {code:?}: {err}");
    let parsed: Vec<(BTreeMap<String, String>, Vec<String>)> =
        calls.iter().map(|c| split_compose_run(c)).collect();
    let next_calls: Vec<&(BTreeMap<String, String>, Vec<String>)> = parsed
        .iter()
        .filter(|(env, _)| {
            env.get("GATEWAY_CALC_PROFILE")
                .is_some_and(|p| p.ends_with("/next.env"))
        })
        .collect();
    assert_eq!(
        (calls.len(), next_calls.len()),
        (2, 1),
        "I-7: с next.env ожидались два вызова runner'а, из них ровно один с \
         `-e GATEWAY_CALC_PROFILE=<каталог>/next.env`; получено {calls:?}"
    );
    let (env, argv) = next_calls[0];
    let next_container = env["GATEWAY_CALC_PROFILE"].clone();
    let declared = declared_profile(SERVICE);
    assert_eq!(
        Path::new(&next_container).parent(),
        Path::new(&declared).parent(),
        "I-7: next.env прогрева лежит не в каталоге профиля образа"
    );
    let next_cov = coverage_of(argv);
    assert!(
        next_cov.as_deref().is_some_and(|c| c != active_cov),
        "I-7: прогон next пишет покрытие {next_cov:?} — оно обязано существовать и отличаться от \
         пути ретеншена `{active_cov}`: курсор next дальше курсора active даст ретеншену право \
         удалить хвост, нужный выдаче"
    );
    let active_now: Vec<&(BTreeMap<String, String>, Vec<String>)> = parsed
        .iter()
        .filter(|(env, _)| !env.contains_key("GATEWAY_CALC_PROFILE"))
        .collect();
    assert_eq!(
        active_now.len(),
        1,
        "I-7: обычный прогон по active.env пропал или раздвоился при наличии next.env"
    );
    assert_eq!(
        coverage_of(&active_now[0].1).as_deref(),
        Some(active_cov.as_str()),
        "I-7: прогон active сменил путь покрытия ретеншена"
    );
}

/// **`p9` — слепки двух профилей сосуществуют; первый не затирается.**
#[test]
fn p9_two_profiles_coexist_first_slepok_untouched() {
    let journal = journal_of(N);
    let ckpt = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let r1 = run_warmer(
        &active_profile_path(),
        journal.path(),
        ckpt.path(),
        &[],
        &[],
    );
    assert_eq!(r1.code, Some(0), "SETUP: прогон active упал: {}", r1.stderr);
    let first = ckpt.path().join(PROD_CKPT_NAME);
    let before = std::fs::read(&first).unwrap_or_else(|e| {
        panic!(
            "I-2: прогон active не записал {} ({e}); есть {:?}",
            PROD_CKPT_NAME,
            slepok_names(ckpt.path())
        )
    });
    let next = write_profile_variant(
        tmp.path(),
        "next.env",
        &variant(|l| {
            replace_key(l, "CALC_PROFILE_VERSION", "2");
            replace_key(l, "GATEWAY_BANDS", "0.015,0.03,0.05,0.08,0.15,0.3");
        }),
    );
    let r2 = run_warmer(&next, journal.path(), ckpt.path(), &[], &[]);
    assert_eq!(r2.code, Some(0), "SETUP: прогон next упал: {}", r2.stderr);
    let names = slepok_names(ckpt.path());
    assert_eq!(
        names.len(),
        2,
        "I-7: после прогрева второго профиля слепков {names:?} — новый не собран рядом со старым"
    );
    assert_eq!(
        std::fs::read(&first).unwrap(),
        before,
        "I-7: слепок active изменён прогоном другого профиля — откат лишился готового слепка"
    );
}

// ───────────────────────── p8b: ЭФФЕКТ на цели ретеншена (C-280 R2) ─────────────────────────

/// Монтирования сервиса compose: `(том, путь в контейнере)`.
/// Монтирования сервиса compose: `(том, путь в контейнере, только-чтение)` — режим НЕСЁТСЯ
/// (`A-049` Р-5, чек-лист `testing.md` п.6: фикстура повторяет режим монтирования прода).
fn compose_volumes(service: &str) -> Vec<(String, String, bool)> {
    compose_block(service, "volumes")
        .iter()
        .filter_map(|l| l.strip_prefix("- "))
        .map(|spec| spec.trim().trim_matches('"'))
        .filter_map(|spec| {
            let mut it = spec.split(':');
            let vol = it.next()?.to_string();
            let dst = it.next()?.to_string();
            let ro = it.next().is_some_and(|m| m.split(',').any(|o| o == "ro"));
            Some((vol, dst, ro))
        })
        .collect()
}

/// Том сервиса, на котором лежит путь контейнера: `(том, только-чтение)`.
fn volume_of(service: &str, p: &str) -> (String, bool) {
    let p = normalize_container(p);
    compose_volumes(service)
        .into_iter()
        .filter(|(_, dst, _)| p == *dst || p.starts_with(&format!("{dst}/")))
        .max_by_key(|(_, dst, _)| dst.len())
        .map(|(v, _, ro)| (v, ro))
        .unwrap_or_else(|| panic!("SETUP НЕ СОСТОЯЛСЯ: путь `{p}` сервиса `{service}` не на томе"))
}

/// Тома `:ro` сервиса в фикстуре делаются НЕДОСТУПНЫМИ ДЛЯ ЗАПИСИ на время прогона; возврат
/// прав — при выходе из области (иначе `TempDir` не уберёт каталог).
struct ReadOnlySeal(Vec<PathBuf>);
impl ReadOnlySeal {
    fn seal(service: &str, vols: &Path) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let mut sealed = Vec::new();
        for (vol, _, ro) in compose_volumes(service) {
            if !ro {
                continue;
            }
            let base = vols.join(&vol);
            std::fs::create_dir_all(&base).unwrap();
            for e in walk(&base) {
                let mode = if e.is_dir() { 0o555 } else { 0o444 };
                std::fs::set_permissions(&e, std::fs::Permissions::from_mode(mode)).unwrap();
                sealed.push(e);
            }
            // Setup-страж: под root `chmod` не запрещает запись — модель режима не состоялась бы.
            let probe = base.join(".m94-ro-probe");
            assert!(
                std::fs::write(&probe, b"x").is_err(),
                "SETUP НЕ СОСТОЯЛСЯ: запись в `:ro`-том `{vol}` фикстуры удалась (прогон под root?) — \
                 режим монтирования прода не смоделирован"
            );
        }
        ReadOnlySeal(sealed)
    }
}
impl Drop for ReadOnlySeal {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        for e in &self.0 {
            let mode = if e.is_dir() { 0o755 } else { 0o644 };
            let _ = std::fs::set_permissions(e, std::fs::Permissions::from_mode(mode));
        }
    }
}

fn walk(d: &Path) -> Vec<PathBuf> {
    let mut out = vec![d.to_path_buf()];
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// Путь ВНУТРИ контейнера сервиса → путь фикстуры: каждый том — свой каталог в `vols`.
/// Лексическая нормализация пути ВНУТРИ контейнера (`.`/`..`/`//`) — так его разрешит ядро
/// контейнера. Без неё `/ckpt/../ckpt/x` отобразился бы мимо тома и псевдоним цели прошёл бы.
fn normalize_container(p: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    format!("/{}", parts.join("/"))
}

fn map_container_path(service: &str, p: &str, vols: &Path) -> PathBuf {
    let p = normalize_container(p);
    let p = p.as_str();
    let (vol, dst, _) = compose_volumes(service)
        .into_iter()
        .filter(|(_, dst, _)| p == dst || p.starts_with(&format!("{dst}/")))
        .max_by_key(|(_, dst, _)| dst.len())
        .unwrap_or_else(|| {
            panic!("SETUP НЕ СОСТОЯЛСЯ: путь `{p}` сервиса `{service}` не лежит ни на одном томе")
        });
    let base = vols.join(&vol);
    std::fs::create_dir_all(&base).unwrap();
    base.join(p[dst.len()..].trim_start_matches('/'))
}

/// Аргумент `--flag=v` / `--flag v`.
fn arg_of(argv: &[String], flag: &str) -> Option<String> {
    argv.iter().enumerate().find_map(|(i, a)| {
        a.strip_prefix(&format!("{flag}="))
            .map(str::to_string)
            .or_else(|| (a == flag).then(|| argv.get(i + 1).cloned()).flatten())
    })
}

/// Путь покрытия, который ЧИТАЕТ ретеншен: НАСТОЯЩИЙ скрипт cron'а ретеншена в режиме печати
/// argv (контракт M-48 `HFT_CRON_PRINT_ARGV=1`), окружение — прод-cron.
fn retention_coverage_container_path() -> String {
    let o = Command::new("bash")
        .arg(repo_root().join("deploy/bin/journal-retention-cron.sh"))
        .env_clear()
        .envs(cron_env())
        .env("HFT_CRON_PRINT_ARGV", "1")
        .output()
        .expect("запуск journal-retention-cron.sh");
    let argv: Vec<String> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    arg_of(&argv, "--checkpoint-coverage").unwrap_or_else(|| {
        panic!(
            "SETUP НЕ СОСТОЯЛСЯ: ретеншен не печатает --checkpoint-coverage (exit {:?}): {argv:?}",
            o.status.code()
        )
    })
}

fn append_trades(dir: &Path, from: u64, n: u64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in from..from + n {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
}

/// Исполнить вызов runner'а НАСТОЯЩИМ прогревателем: пути контейнера — на тома-фикстуры.
/// Образ собирается из ЧЕКАУТА (`HFT_ROOT`): путь профиля в образе → файл `checkout/config/…`.
fn exec_call(call: &[String], vols: &Path, checkout: &Path) -> (Option<i32>, String) {
    let (opt_env, argv) = split_compose_run(call);
    let mut env = compose_env(SERVICE, &secret_only_dotenv());
    env.extend(opt_env);
    if let Some(c) = env.get("GATEWAY_CALC_PROFILE").cloned() {
        let dst = dockerfile_profile_dst();
        let rel = c
            .strip_prefix(&format!("{dst}/"))
            .unwrap_or_else(|| panic!("I-5: GATEWAY_CALC_PROFILE={c} вне каталога образа `{dst}`"));
        let file = checkout.join("config/calc-profile").join(rel);
        assert!(
            file.exists(),
            "I-7: прогон указывает на профиль {c}, а в чекауте его нет ({})",
            file.display()
        );
        env.insert(
            "GATEWAY_CALC_PROFILE".to_string(),
            file.display().to_string(),
        );
    }
    env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
    let argv: Vec<String> = argv
        .iter()
        .map(|a| {
            for f in ["--dir", "--ckpt-dir", "--coverage-out"] {
                if let Some(v) = a.strip_prefix(&format!("{f}=")) {
                    return format!("{f}={}", map_container_path(SERVICE, v, vols).display());
                }
            }
            a.clone()
        })
        .collect();
    // Раздельная форма `--flag value` для путей: перенацелить значение следующего токена.
    let mut out = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if ["--dir", "--ckpt-dir", "--coverage-out"].contains(&a.as_str()) {
            out.push(a.clone());
            if let Some(v) = argv.get(i + 1) {
                out.push(map_container_path(SERVICE, v, vols).display().to_string());
            }
            i += 2;
            continue;
        }
        out.push(a.clone());
        i += 1;
    }
    let o = Command::new(BIN)
        .args(&out)
        .env_clear()
        .envs(&env)
        .output()
        .expect("запуск gateway-checkpoint");
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// **`p8b` — ЭФФЕКТ, а не текст (`C-280` R2).** Оба прогона cron'а (active и next) исполняются
/// НАСТОЯЩИМ прогревателем на ОБЩИХ томах-фикстурах (пути контейнера → тома compose); журнал
/// между прогонами растёт, так что курсор next ДАЛЬШЕ курсора active. Файл, который ЧИТАЕТ
/// ретеншен (путь снят с его настоящего скрипта), обязан после прогона next остаться РАВНЫМ
/// курсору active: только курсор active разрешает удаление хвоста active. Пути сравниваются
/// после лексической нормализации пути контейнера и `canonicalize` — синтаксические псевдонимы
/// одной цели (`./`, `../`, `//`) здесь не проходят (мутанты предъявлены). ПРЕДЕЛ, названный по
/// `C-281` (примечание к R2): фикстура НЕ создаёт симлинк-псевдоним, а исполняются только ВЫЗОВЫ
/// runner'а, записанные заглушкой, — побочные действия самой cron-обёртки сверх этих вызовов
/// здесь не наблюдаются. Их запрещает спека (§5: обёртка не трогает файлы тома сама).
#[test]
fn p8b_next_run_cannot_move_the_retention_cursor() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".env"), "GATEWAY_JWT_SECRET=x\n").unwrap();
    let pdir = root.path().join("config/calc-profile");
    std::fs::create_dir_all(&pdir).unwrap();
    std::fs::copy(active_profile_path(), pdir.join("active.env")).unwrap();
    std::fs::write(
        pdir.join("next.env"),
        variant(|l| {
            replace_key(l, "CALC_PROFILE_VERSION", "2");
            replace_key(l, "GATEWAY_BANDS", "0.015,0.03,0.05,0.08,0.15,0.3");
        }),
    )
    .unwrap();
    let (code, err, calls) = run_cron_calls(root.path());
    assert_eq!(code, Some(0), "SETUP: cron с next вышел {code:?}: {err}");
    let is_next = |c: &Vec<String>| {
        split_compose_run(c)
            .0
            .get("GATEWAY_CALC_PROFILE")
            .is_some_and(|p| p.ends_with("/next.env"))
    };
    let active: Vec<&Vec<String>> = calls.iter().filter(|c| !is_next(c)).collect();
    let next: Vec<&Vec<String>> = calls.iter().filter(|c| is_next(c)).collect();
    assert_eq!(
        (active.len(), next.len()),
        (1, 1),
        "I-7: с next.env ожидался ровно один прогон active и один next; вызовы: {calls:?}"
    );

    // Тома-фикстуры; журнал — на томе журнала по пути `--dir` прогона active.
    let vols = tempfile::tempdir().unwrap();
    let (_, active_argv) = split_compose_run(active[0]);
    let jdir = map_container_path(
        SERVICE,
        &arg_of(&active_argv, "--dir").expect("SETUP: у прогона нет --dir"),
        vols.path(),
    );
    std::fs::create_dir_all(&jdir).unwrap();
    append_trades(&jdir, 0, N);

    // Цель ретеншена: путь его скрипта, разрешённый через ЕГО монтирования.
    let ret_container = retention_coverage_container_path();
    let ret_target = map_container_path("journal-retention", &ret_container, vols.path());

    let (c1, e1) = {
        let _ro = ReadOnlySeal::seal(SERVICE, vols.path()); // `A-049` Р-5: режим тома как на проде
        exec_call(active[0], vols.path(), root.path())
    };
    assert_eq!(c1, Some(0), "SETUP: прогон active упал: {e1}");
    let cursor_active = std::fs::read_to_string(&ret_target).unwrap_or_else(|e| {
        panic!(
            "I-7: прогон active не записал покрытие туда, откуда читает ретеншен ({}): {e}",
            ret_target.display()
        )
    });

    append_trades(&jdir, N, 200); // next увидит больше журнала, чем active
    let (c2, e2) = {
        let _ro = ReadOnlySeal::seal(SERVICE, vols.path());
        exec_call(next[0], vols.path(), root.path())
    };
    assert_eq!(
        c2,
        Some(0),
        "I-7: прогон next упал (на проде так же; например, запись на `:ro`-том — EROFS): {e2}"
    );

    // Setup-страж: next действительно продвинулся дальше active — иначе равенство ниже ничего
    // не доказывает.
    let (_, next_argv) = split_compose_run(next[0]);
    let next_cov = map_container_path(
        SERVICE,
        &arg_of(&next_argv, "--coverage-out").expect("I-7: у прогона next нет --coverage-out"),
        vols.path(),
    );
    let next_val = std::fs::read_to_string(&next_cov).unwrap_or_else(|e| {
        panic!(
            "I-7: прогон next не записал своё покрытие {}: {e}",
            next_cov.display()
        )
    });
    let parse = |s: &str| s.trim().parse::<u64>().expect("покрытие — число");
    assert!(
        parse(&next_val) > parse(&cursor_active),
        "SETUP НЕ СОСТОЯЛСЯ: курсор next ({}) не дальше курсора active ({}) — подмена цели \
         ретеншена была бы неотличима",
        next_val.trim(),
        cursor_active.trim()
    );
    assert_ne!(
        std::fs::canonicalize(&next_cov).unwrap(),
        std::fs::canonicalize(&ret_target).unwrap(),
        "I-7: покрытие next и покрытие ретеншена — ОДИН файл после разрешения путей"
    );
    // `A-049` Р-5: пути записи прогона `next` лежат на RW-томе прогревателя.
    for flag in ["--coverage-out", "--ckpt-dir"] {
        let p =
            arg_of(&next_argv, flag).unwrap_or_else(|| panic!("I-7: у прогона next нет {flag}"));
        let (vol, ro) = volume_of(SERVICE, &p);
        assert!(
            !ro,
            "I-7: прогон next пишет {flag}={p} на том `{vol}`, смонтированный `:ro` — на проде EROFS"
        );
    }
    // `A-049` Р-4: КОМПОЗИЦИЯ С ГЕЙТОМ — гейт ищет слепок нового профиля в КОРНЕ тома слепков
    // (имя от `--print-ckpt-name`), значит прогон `next` обязан писать туда же, куда `active`.
    let (_, act_argv) = split_compose_run(active[0]);
    let act_dir =
        normalize_container(&arg_of(&act_argv, "--ckpt-dir").expect("у active нет --ckpt-dir"));
    let nxt_dir_c =
        normalize_container(&arg_of(&next_argv, "--ckpt-dir").expect("у next нет --ckpt-dir"));
    assert_eq!(
        nxt_dir_c, act_dir,
        "I-7: прогон next пишет слепки в `{nxt_dir_c}`, а active и гейт деплоя — в `{act_dir}`: \
         переключение на next отвергалось бы вечно («слепка нет»)"
    );
    let next_prof = parse_profile_text(&std::fs::read_to_string(pdir.join("next.env")).unwrap());
    let next_dir = map_container_path(SERVICE, &nxt_dir_c, vols.path());
    let want =
        gateway::checkpoint::ckpt_path_for_pub(&next_dir, &selector_from_profile(&next_prof));
    assert!(
        want.exists(),
        "I-7: после прогона next в каталоге слепков нет {} — слепка селектора next.env (имя по \
         независимому разбору профиля); есть {:?}",
        want.display(),
        slepok_names(&next_dir)
    );
    let after = std::fs::read_to_string(&ret_target).expect("покрытие ретеншена исчезло");
    assert_eq!(
        after.trim(),
        cursor_active.trim(),
        "I-7: после прогона next файл, который читает ретеншен, сдвинулся с {} на {} — курсор \
         next разрешил бы удаление хвоста, нужного выдаче для докрутки active (данные, не ресурс)",
        cursor_active.trim(),
        after.trim()
    );
}
