//! RED `M-90` (sacred, architect-only) — **прогреватель, запущенный ТЕМ ВЫЗОВОМ, каким его
//! зовёт прод (cron → `deploy/bin/gateway-checkpoint-cron.sh` → `docker compose run`), пишет
//! слепок, который НАХОДИТ сервер выдачи** (`TD-227`).
//!
//! # Инцидент — замер, а не гипотеза (architect, 2026-10-01)
//!
//! Прод-выдача восемь суток отвечала КАЖДОМУ клиенту `not_ready` при зелёных liveness-сигналах.
//! Сервер открывал `/ckpt/ckpt-8f69809dd707e8c9.bin` (последняя запись 2026-09-23 12:45,
//! схема 10), прогреватель каждые 15 минут писал `/ckpt/ckpt-b0f1ed89ec2ec142.bin`. Имена
//! посчитаны тем же кодом (`gateway::checkpoint::ckpt_path_for_pub`) для прод-селектора:
//!
//! ```text
//! семь полос 0.015..0.6 → /ckpt/ckpt-8f69809dd707e8c9.bin   (читает сервер)
//! одна полоса 0.001     → /ckpt/ckpt-b0f1ed89ec2ec142.bin   (пишет прогреватель)
//! ```
//!
//! Причина — у ОДНОЙ величины ДВА источника. Сервер берёт полосы из `GATEWAY_BANDS` (host
//! `.env` → compose-интерполяция). Прогреватель — из `CHECKPOINT_BANDS` скрипта cron'а
//! (дефолт `0.001`), а `docker compose run SERVICE ARGS` ЗАМЕНЯЕТ `command:` сервиса целиком,
//! так что `--bands=${GATEWAY_BANDS}` из compose до прод-прогревателя НЕ доходит. Семь полос
//! жили ручной строкой `CHECKPOINT_BANDS=…` в `/etc/cron.d/hft-journal-retention`; деплой
//! ставит `deploy/cron.d/*` поверх (`deploy.yml` `install_cron`) и стёр её деплоем
//! `0f9a980` (2026-09-23 12:45 — та же минута, что mtime последнего «правильного» слепка).
//!
//! # Почему соседний оракул этого не поймал
//!
//! `red_checkpoint_bin_prod_argv::c3ter_writer_and_reader_agree_on_checkpoint` судит
//! писателя с argv из `command:` compose — то есть вызов, которым прод прогреватель НЕ зовёт.
//! `testing.md` §«Целостность гейта» св-во 1: гейт, проверенный не тем вызовом, каким его
//! зовёт прод, не проверен. Здесь писатель запускается через НАСТОЯЩИЙ скрипт cron'а с
//! окружением из НАСТОЯЩЕГО `deploy/cron.d/journal-retention`, а шов `CHECKPOINT_RUNNER`
//! (контракт скрипта, M-48) заменяет только `docker compose` — записывающей заглушкой.
//!
//! # Что моделируется и где предел
//!
//! Заглушка записывает argv; эффективный argv прогревателя выводится по семантике
//! `docker compose run SERVICE [ARGS]`: ARGS непусты ⇒ они и есть argv (замена `command:`);
//! пусты ⇒ `command:` сервиса, интерполированный host `.env`. Интерполяция `${VAR:-d}`
//! и `${VAR}` воспроизведена здесь, а не взята из `docker compose config` (в CI докера нет)
//! — это НАЗВАННЫЙ предел. Затем исполняется НАСТОЯЩИЙ бинарь `gateway-checkpoint` с этим
//! argv (пути перенацелены на фикстуру), а читатель — публичный `LiveReducer::resume` с
//! селектором, собранным из окружения `gateway-serve` (compose, интерполированный тем же
//! `.env`). Свидетель — `ReadStats::events_decoded`: слепок найден ⇒ 0, не найден ⇒ N.
//!
//! Форма `.env` — прод (ssh 2026-10-01, `cut -d= -f1 .env`): ключи `GATEWAY_JWT_SECRET`,
//! `GATEWAY_BANDS`. Значение полос — подписанный набор `П-014` п.4 / `П-029`.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{LiveReducer, Selector};
use journal::{EpochFilter, Journal, WriterConfig};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_gateway-checkpoint");
const N: u64 = 300;
const SEVEN: &str = "0.015,0.03,0.05,0.08,0.15,0.3,0.6";
const SERVICE: &str = "gateway-checkpoint";
/// Переменные селектора, которые сегодня несёт скрипт cron'а СВОИМИ копиями.
const SCRIPT_SELECTOR_VARS: [&str; 5] = [
    "CHECKPOINT_VENUE",
    "CHECKPOINT_SYMBOL",
    "CHECKPOINT_TIMEFRAME_MS",
    "CHECKPOINT_BANDS",
    "CHECKPOINT_WINDOW_MS",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ───────────────────────── compose: разбор + интерполяция ─────────────────────────

/// `${VAR:-d}` → значение из `env`, если непусто, иначе `d`; `${VAR}` → значение или пусто.
fn interpolate(s: &str, env: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find('}').expect("незакрытая ${...}") + start;
        let inner = &rest[start + 2..end];
        let (name, default) = match inner.split_once(":-") {
            Some((n, d)) => (n, d),
            None => (inner, ""),
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

/// Строки блока `<block>:` сервиса (список `- x` для `command:`, `k: v` для `environment:`).
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
    let items: Vec<String> = compose_block(service, "command")
        .iter()
        .filter_map(|l| l.strip_prefix("- "))
        .map(|i| interpolate(i.trim().trim_matches('"'), env))
        .collect();
    assert!(
        !items.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: нет command-блока сервиса `{service}` в docker-compose.yml"
    );
    items
}

fn compose_env(service: &str, env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    compose_block(service, "environment")
        .iter()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), interpolate(v.trim().trim_matches('"'), env)))
        .collect()
}

// ───────────────────────── cron: прод-окружение ─────────────────────────

/// `KEY=VALUE` строки НАСТОЯЩЕГО `deploy/cron.d/journal-retention` — окружение, в котором
/// cron зовёт скрипт на проде (деплой ставит этот файл как есть).
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
        "SETUP НЕ СОСТОЯЛСЯ: в deploy/cron.d/journal-retention нет PATH — окружение cron'а не разобрано"
    );
    assert!(
        text.contains("gateway-checkpoint-cron.sh"),
        "SETUP НЕ СОСТОЯЛСЯ: прод-cron больше не зовёт gateway-checkpoint-cron.sh — оракул судил бы не тот путь"
    );
    env
}

// ───────────────────────── фикстура журнала ─────────────────────────

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
            side: if i.is_multiple_of(2) { Side::Buy } else { Side::Sell },
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

// ───────────────────────── прогон пути cron → runner ─────────────────────────

struct CronRun {
    code: Option<i32>,
    stderr: String,
    log: String,
    alert: String,
    /// argv, с которым скрипт позвал `docker compose` (None — runner не звался).
    runner_argv: Option<Vec<String>>,
}

/// Запустить НАСТОЯЩИЙ скрипт cron'а в прод-окружении cron'а; `docker compose` заменён
/// записывающей заглушкой через шов `CHECKPOINT_RUNNER`. `extra` — добавочные переменные
/// окружения cron'а (оператор вписал строку в `/etc/cron.d`).
fn run_cron(root: &Path, extra: &[(&str, &str)]) -> CronRun {
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let shim = work.join("docker");
    let out = work.join("runner.argv");
    std::fs::write(
        &shim,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$M90_RUNNER_OUT\"\necho 'gateway-checkpoint: ok (shim)'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (log, alert, last) = (work.join("ckpt.log"), work.join("ckpt.alert"), work.join("ckpt.last"));
    let mut cmd = Command::new("bash");
    cmd.arg(repo_root().join("deploy/bin/gateway-checkpoint-cron.sh"))
        .env_clear()
        .envs(cron_env())
        .env("HFT_ROOT", root)
        .env("CHECKPOINT_RUNNER", format!("{} compose run --rm {SERVICE}", shim.display()))
        .env("CHECKPOINT_LOG", &log)
        .env("CHECKPOINT_ALERT_FILE", &alert)
        .env("CHECKPOINT_LAST_SUCCESS", &last)
        .env("M90_RUNNER_OUT", &out);
    for (k, v) in extra {
        cmd.env(k, v);
    }
    let o = cmd.output().expect("запуск gateway-checkpoint-cron.sh");
    let runner_argv = std::fs::read_to_string(&out)
        .ok()
        .map(|s| s.lines().map(str::to_string).collect());
    CronRun {
        code: o.status.code(),
        stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        log: std::fs::read_to_string(&log).unwrap_or_default(),
        alert: std::fs::read_to_string(&alert).unwrap_or_default(),
        runner_argv,
    }
}

/// Эффективный argv прогревателя по семантике `docker compose run [opts] SERVICE [ARGS]`.
fn effective_warmer_argv(runner_argv: &[String], dotenv: &BTreeMap<String, String>) -> Vec<String> {
    let pos = runner_argv.iter().position(|a| a == SERVICE).unwrap_or_else(|| {
        panic!("SETUP НЕ СОСТОЯЛСЯ: runner позван без имени сервиса `{SERVICE}`: {runner_argv:?}")
    });
    assert_eq!(
        runner_argv.first().map(String::as_str),
        Some("compose"),
        "SETUP НЕ СОСТОЯЛСЯ: заглушка получила не `compose …`: {runner_argv:?}"
    );
    let args = &runner_argv[pos + 1..];
    if args.is_empty() {
        compose_command(SERVICE, dotenv)
    } else {
        args.to_vec()
    }
}

/// Перенацелить пути на фикстуру в обеих формах (`--f=v` и `--f v`).
fn retarget(args: &[String], journal: &Path, ckpt: &Path, cov: &Path) -> Vec<String> {
    let map = [("--dir", journal), ("--ckpt-dir", ckpt), ("--coverage-out", cov)];
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let hit = map.iter().find(|(f, _)| a == f || a.starts_with(&format!("{f}=")));
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

/// Селектор СЕРВЕРА выдачи — из окружения `gateway-serve` в compose, интерполированного `.env`.
/// Разбор полос/чисел — та же форма, что `serve_config_from_env` (`split(',')`, `parse`):
/// предмет оракула — КОМПОЗИЦИЯ источников, а не разбор числа (названный предел).
fn server_selector(dotenv: &BTreeMap<String, String>) -> Selector {
    let env = compose_env("gateway-serve", dotenv);
    let get = |k: &str| -> String {
        env.get(k)
            .cloned()
            .unwrap_or_else(|| panic!("SETUP НЕ СОСТОЯЛСЯ: у gateway-serve в compose нет `{k}`"))
    };
    assert_eq!(get("GATEWAY_VENUE"), "Binance", "SETUP НЕ СОСТОЯЛСЯ: фикстура умеет только Binance");
    let ms = |k: &str| get(k).parse::<i64>().unwrap_or_else(|e| panic!("SETUP: {k} не число: {e}"));
    Selector {
        venue: Venue::Binance,
        symbol: get("GATEWAY_SYMBOL"),
        timeframe_ms: ms("GATEWAY_TIMEFRAME_MS"),
        bands: get("GATEWAY_BANDS")
            .split(',')
            .map(|b| b.trim().parse::<f64>().expect("полоса — число"))
            .collect(),
        window_ms: Some(ms("GATEWAY_WINDOW_MS")),
        depth_cadence_ms: Some(ms("GATEWAY_DEPTH_CADENCE_MS")),
    }
}

/// Полный путь прода: host `.env` → cron → скрипт → compose run → НАСТОЯЩИЙ прогреватель →
/// слепок → `LiveReducer::resume` с селектором сервера. Возвращает `events_decoded`
/// читателя (0 ⇔ слепок найден) и контроль с ЧУЖИМИ полосами (обязан быть > 0).
fn prod_path(dotenv_text: &str) -> (u64, u64, Vec<String>) {
    let root = tempfile::tempdir().expect("root");
    std::fs::write(root.path().join(".env"), dotenv_text).unwrap();
    std::fs::copy(repo_root().join("docker-compose.yml"), root.path().join("docker-compose.yml")).unwrap();
    let dotenv: BTreeMap<String, String> = dotenv_text
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let cr = run_cron(root.path(), &[]);
    let runner = cr.runner_argv.clone().unwrap_or_else(|| {
        panic!(
            "SETUP НЕ СОСТОЯЛСЯ: скрипт cron'а не позвал runner (exit {:?}); stderr: {}; log: {}; alert: {}",
            cr.code, cr.stderr, cr.log, cr.alert
        )
    });
    assert_eq!(cr.code, Some(0), "SETUP НЕ СОСТОЯЛСЯ: скрипт cron'а с заглушкой вышел {:?}: {}", cr.code, cr.stderr);

    let journal = journal_of(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = ckpt.path().join("covered_through_seq");
    let argv = retarget(&effective_warmer_argv(&runner, &dotenv), journal.path(), ckpt.path(), &cov);
    let w = Command::new(BIN)
        .args(&argv)
        .env_clear()
        .envs(compose_env(SERVICE, &dotenv))
        .output()
        .expect("запуск gateway-checkpoint");
    assert_eq!(
        w.status.code(),
        Some(0),
        "SETUP НЕ СОСТОЯЛСЯ: прод-прогреватель с эффективным argv {argv:?} вышел {:?}: {}",
        w.status.code(),
        String::from_utf8_lossy(&w.stderr)
    );
    let written = std::fs::read_dir(ckpt.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("ckpt-"))
        .count();
    assert_eq!(written, 1, "SETUP НЕ СОСТОЯЛСЯ: прогреватель записал {written} слепков, ожидался один");

    let sel = server_selector(&dotenv);
    let (_r, found) = LiveReducer::resume(journal.path(), EpochFilter::OwnCaptureOnly, &sel, ckpt.path())
        .expect("resume сервера");
    let mut other = sel.clone();
    other.bands = vec![0.002];
    let (_r2, ctrl) = LiveReducer::resume(journal.path(), EpochFilter::OwnCaptureOnly, &other, ckpt.path())
        .expect("resume контроля");
    (found.events_decoded, ctrl.events_decoded, argv)
}

// ───────────────────────── оракулы ─────────────────────────

/// **`w1` — прод-форма `.env` (семь полос): сервер находит слепок прогревателя.**
/// Сегодня RED: скрипт подаёт `--bands 0.001` из своей копии, слепок ненаходим.
#[test]
fn w1_cron_warmer_snapshot_is_found_by_server_with_prod_dotenv() {
    let (found, ctrl, argv) = prod_path(&format!("GATEWAY_JWT_SECRET=x\nGATEWAY_BANDS={SEVEN}\n"));
    assert!(ctrl > 0, "КОНТРОЛЬ НЕ СОСТОЯЛСЯ: читатель с чужими полосами тоже нашёл слепок — свидетель не различает");
    assert_eq!(
        found, 0,
        "TD-227: прогреватель, запущенный ТЕМ ВЫЗОВОМ, каким его зовёт прод (cron → скрипт → \
         compose run), записал слепок, которого сервер выдачи НЕ НАХОДИТ: читатель декодировал \
         {found} событий вместо 0. Значит селектор прогревателя взят не из того источника, что у \
         сервера (`GATEWAY_BANDS` host `.env`), и на проде каждый клиент получает `not_ready`. \
         Эффективный argv прогревателя: {argv:?}"
    );
}

/// **`w2` — позитивный контроль: `.env` без полос (оба на дефолте compose) — слепок найден.**
/// Зелен и сегодня; держит оракул от красного по неверной причине (поломка фикстуры).
#[test]
fn w2_default_dotenv_snapshot_is_found_positive_control() {
    let (found, ctrl, argv) = prod_path("GATEWAY_JWT_SECRET=x\n");
    assert!(ctrl > 0, "КОНТРОЛЬ НЕ СОСТОЯЛСЯ: свидетель не различает полосы");
    assert_eq!(found, 0, "позитивный контроль: на дефолтах слепок обязан находиться; argv {argv:?}");
}

/// **`w3` — у скрипта cron'а НЕТ собственного источника селектора: строка `CHECKPOINT_*`
/// селектора в окружении cron'а — отказ с именем переменной, runner не зовётся.**
///
/// Тихо игнорировать операторскую строку нельзя (оператор думает, что задал полосы), молча
/// применять — значит вернуть второй источник, из-за которого случился `TD-227`.
#[test]
fn w3_script_refuses_own_selector_copy_and_names_it() {
    for var in SCRIPT_SELECTOR_VARS {
        let root = tempfile::tempdir().expect("root");
        std::fs::write(root.path().join(".env"), format!("GATEWAY_BANDS={SEVEN}\n")).unwrap();
        std::fs::copy(repo_root().join("docker-compose.yml"), root.path().join("docker-compose.yml")).unwrap();
        let cr = run_cron(root.path(), &[(var, "0.001")]);
        assert!(
            cr.runner_argv.is_none(),
            "TD-227: при `{var}` в окружении cron'а скрипт позвал прогреватель ({:?}) — у селектора \
             снова ДВА источника",
            cr.runner_argv
        );
        assert_ne!(cr.code, Some(0), "TD-227: при `{var}` скрипт вышел 0 — отказ обязан быть виден");
        let said = format!("{}{}{}", cr.stderr, cr.log, cr.alert);
        assert!(
            said.contains(var),
            "TD-227: отказ при `{var}` не называет переменную (stderr/log/alert: {said:?})"
        );
    }
}
