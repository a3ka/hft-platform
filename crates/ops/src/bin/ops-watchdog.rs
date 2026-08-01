//! `ops-watchdog` — одноразовый cron-бинарь (founder-задача 2026-07-31, мост к M-43 task#3
//! "минимальный cron-watchdog→Telegram"). Читает `recorder.heartbeat` + cron-маркеры
//! (`/var/lib/hft/*.last-success` + `*.alert`, R-005 F-6) + `docker ps`/`docker inspect`,
//! собирает их в `ops::watchdog_cycle::CycleInputs` и зовёт `ops::watchdog_cycle::run_cycle` —
//! ВСЯ диагностическая логика живёт там (юнит-тестируема сценарно, `red_ops_watchdog_cycle.rs`
//! — R-005 F-10: старая версия держала склейку прямо в бинаре, что делало её недостижимой
//! для `tests/`).
//!
//! ВСЁ I/O (файлы, `docker`, HTTP) — ЗДЕСЬ. Библиотечные модули `ops::watchdog`/
//! `ops::watchdog_cycle`/`ops::format` остаются чистыми и юнит-тестируемыми без реальной
//! машины (см. их doc-комментарии).
//!
//! Установка в cron/`/var/lib/hft` — задача reviewer/founder после ревью (границы задачи);
//! этот бинарь и `scripts/watchdog_cron.sh` — готовый к установке артефакт, но НЕ
//! устанавливается мной на прод.
//!
//! Конфигурация — через env (значения по умолчанию соответствуют прод-топологии VPS,
//! `docs/SESSION-HANDOFF.md §7`):
//!   - `WATCHDOG_HEARTBEAT_PATH` (default
//!     `/var/lib/docker/volumes/hft-platform_journal-data/_data/recorder.heartbeat`)
//!   - `WATCHDOG_CRON_DIR` (default `/var/lib/hft`) — сканирует пары
//!     `<job>.last-success`/`<job>.alert` для `compaction`/`gateway-checkpoint`/`retention`
//!     (R-005 F-6: оба маркера, не только позитивный).
//!   - `WATCHDOG_STATE_PATH` (default `/var/lib/hft/watchdog.state.json`)
//!   - `WATCHDOG_CONTAINERS` (default `hft-recorder,hft-gateway-serve`, запятая-разделитель)
//!   - `WATCHDOG_HOST_LABEL` (default `hft-platform-vps`)
//!   - `WATCHDOG_DEDUP_WINDOW_MS` (default `ops::state::DEFAULT_DEDUP_WINDOW_MS` = 30 мин)
//!   - `TELEGRAM_BOT_TOKEN` / `TELEGRAM_CHAT_ID` / `TELEGRAM_API_BASE` — см.
//!     `ops::transport::TelegramTransport`.
//!
//! Exit-код: 0 всегда, ЕСЛИ детекторы отработали (сам факт найденных CRITICAL-алертов —
//! это НЕ ошибка процесса watchdog'а, это его штатный результат). Ненулевой — только если
//! watchdog не смог выполнить свою работу (не смог сохранить состояние и т.п.); такой сбой
//! сам по себе не алертит (у watchdog'а нет watchdog'а — backstop — свежесть его собственных
//! логов/крон-запусков, вне scope этой задачи).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ops::format::format_alert;
use ops::state::{WatchdogState, DEFAULT_DEDUP_WINDOW_MS};
use ops::transport::{StdoutTransport, TelegramTransport, Transport};
use ops::watchdog::{parse_docker_status_healthy, ContainerStatus, Thresholds};
use ops::watchdog_cycle::{run_cycle, CronFailureMarker, CronJobObservation, CycleInputs};

/// Задачи обслуживания журнала, за которыми следит watchdog (имена БЕЗ суффикса — см.
/// `CronJobObservation::name`; у каждой два независимых маркера: `<name>.last-success` и
/// `<name>.alert`, R-005 F-6).
const CRON_JOBS: &[&str] = &["compaction", "gateway-checkpoint", "retention"];

fn main() -> anyhow::Result<()> {
    let heartbeat_path = env_path(
        "WATCHDOG_HEARTBEAT_PATH",
        "/var/lib/docker/volumes/hft-platform_journal-data/_data/recorder.heartbeat",
    );
    let cron_dir = env_path("WATCHDOG_CRON_DIR", "/var/lib/hft");
    let state_path = env_path("WATCHDOG_STATE_PATH", "/var/lib/hft/watchdog.state.json");
    let container_names = env_list(
        "WATCHDOG_CONTAINERS",
        &["hft-recorder", "hft-gateway-serve"],
    );
    let host_label =
        std::env::var("WATCHDOG_HOST_LABEL").unwrap_or_else(|_| "hft-platform-vps".to_string());
    let dedup_window_ms = env_i64("WATCHDOG_DEDUP_WINDOW_MS", DEFAULT_DEDUP_WINDOW_MS);

    let now_ms = now_ms();
    let thr = Thresholds::default();
    let mut state = WatchdogState::load_or_default(&state_path);

    let inputs = gather_inputs(&heartbeat_path, &cron_dir, &container_names);
    let outcome = run_cycle(&inputs, now_ms, &thr, dedup_window_ms, &mut state);

    let stdout_transport = StdoutTransport;
    let telegram_transport = TelegramTransport::from_env();
    if !telegram_transport.is_configured() {
        eprintln!(
            "[ops-watchdog] TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID не заданы — алерты идут только \
             в stdout (лог cron'а). Как только founder добавит токен в окружение, доставка в \
             Telegram включится без правок кода."
        );
    }

    for alert in &outcome.delivered {
        let message = format_alert(alert, &host_label, now_ms);
        // stdout — всегда (лог cron'а — свидетельство работы watchdog'а само по себе).
        let _ = stdout_transport.send(&message);
        if let Err(e) = telegram_transport.send(&message) {
            eprintln!("[ops-watchdog] TelegramTransport::send failed: {e}");
        }
    }

    if outcome.fired.is_empty() {
        println!("[ops-watchdog] {now_ms} — норма, ни одно условие не сработало");
    } else {
        println!(
            "[ops-watchdog] {now_ms} — обнаружено {} алертов ({} отправлено, {} подавлено \
             дедупликацией)",
            outcome.fired.len(),
            outcome.delivered.len(),
            outcome.suppressed
        );
    }

    state
        .save(&state_path)
        .map_err(|e| anyhow::anyhow!("не удалось сохранить {}: {e}", state_path.display()))?;

    Ok(())
}

/// Собрать `CycleInputs` из реального окружения машины — единственное место в бинаре, где
/// происходит I/O для входа цикла (отправка `CycleOutcome::delivered` — отдельно, в `main`).
fn gather_inputs(
    heartbeat_path: &Path,
    cron_dir: &Path,
    container_names: &[String],
) -> CycleInputs {
    let heartbeat = read_heartbeat(heartbeat_path);

    let ps_output = run_docker_ps();
    let containers = container_names
        .iter()
        .map(|name| container_status(name, ps_output.as_deref()))
        .collect();

    let cron_jobs = CRON_JOBS
        .iter()
        .map(|name| CronJobObservation {
            name: name.to_string(),
            last_success_ms: read_cron_marker(&cron_dir.join(format!("{name}.last-success"))),
            failure: read_cron_failure_marker(&cron_dir.join(format!("{name}.alert"))),
        })
        .collect();

    CycleInputs {
        heartbeat,
        containers,
        cron_jobs,
    }
}

fn read_heartbeat(path: &Path) -> Option<ops::watchdog::HeartbeatSample> {
    let body = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&body).ok()
}

/// Маркер — UTC ISO-8601 (`date -u +%Y-%m-%dT%H:%M:%SZ`), см. `deploy/bin/journal-retention-cron.sh`
/// (`RETENTION_LAST_SUCCESS`). Парсим вручную (без `chrono`) в epoch ms — обратная операция
/// `format::format_utc_ts`.
fn read_cron_marker(path: &Path) -> Option<i64> {
    let body = std::fs::read_to_string(path).ok()?;
    parse_utc_iso8601(body.trim())
}

/// R-005 F-6: `<job>.alert` — та же двухстрочная конвенция, что пишет `alert()` в
/// `deploy/bin/journal-retention-cron.sh`/`journal-compaction-cron.sh`/`scripts/watchdog_cron.sh`
/// сам про себя: первая строка — UTC ISO-8601, вторая+ — текст сбоя. Присутствие файла = факт
/// сбоя ДАЖЕ если первая строка не распарсилась (fail-closed, `f6_failure_marker_with_unparseable_timestamp_still_alerts`).
fn read_cron_failure_marker(path: &Path) -> Option<CronFailureMarker> {
    let body = std::fs::read_to_string(path).ok()?;
    let mut lines = body.lines();
    let first = lines.next().unwrap_or("").trim();
    let reported_at_ms = parse_utc_iso8601(first);
    let rest: String = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    let detail = if rest.is_empty() {
        first.to_string()
    } else {
        rest
    };
    Some(CronFailureMarker {
        reported_at_ms,
        detail,
    })
}

fn parse_utc_iso8601(s: &str) -> Option<i64> {
    // "YYYY-MM-DDTHH:MM:SSZ" — фиксированный формат, длина 20.
    if s.len() != 20 || !s.ends_with('Z') {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let se: i64 = s.get(17..19)?.parse().ok()?;
    let days = days_from_civil(y, mo, d);
    Some((days * 86400 + h * 3600 + mi * 60 + se) * 1000)
}

/// Обратная операция к `format::civil_from_days` (Хиннант, days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d as u64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i64 - 719_468
}

fn run_docker_ps() -> Option<String> {
    std::process::Command::new("docker")
        .args(["ps", "--format", "{{.Names}}\t{{.Status}}"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn run_docker_inspect_restart_count(name: &str) -> Option<u64> {
    std::process::Command::new("docker")
        .args(["inspect", "-f", "{{.RestartCount}}", name])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
}

fn container_status(name: &str, ps_output: Option<&str>) -> ContainerStatus {
    let line = ps_output.and_then(|out| {
        out.lines()
            .find_map(|l| l.split_once('\t').filter(|(n, _)| *n == name))
    });
    let healthy = line.map(|(_, status)| parse_docker_status_healthy(status));
    let restart_count = if healthy.is_some() {
        run_docker_inspect_restart_count(name)
    } else {
        None
    };
    ContainerStatus {
        name: name.to_string(),
        healthy,
        restart_count,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn env_path(key: &str, default: &str) -> PathBuf {
    std::env::var(key)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_list(key: &str, default: &[&str]) -> Vec<String> {
    match std::env::var(key) {
        Ok(v) => v
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Err(_) => default.iter().map(|s| s.to_string()).collect(),
    }
}

#[cfg(test)]
mod tests {
    //! `parse_utc_iso8601` — обратная операция `format::format_utc_ts` (сериализатор маркера в
    //! `deploy/bin/journal-retention-cron.sh`). Корректность здесь критична: сломанный парсер
    //! читает возраст cron-маркера неверно и либо молчит на реально протухшем маркере, либо
    //! шумит на здоровом — оба хуже отсутствия проверки. Roundtrip против `ops::format`
    //! (та же civil-calendar математика, независимо реализованная в обе стороны).
    use super::*;
    use ops::format::format_utc_ts;

    #[test]
    fn roundtrips_against_format_utc_ts_for_real_prod_sample() {
        // Тот же реальный прод-таймстамп, что и в `red_ops_format.rs`.
        let ms = 1_785_539_455_000_i64; // секундная точность — маркер не несёт миллисекунды
        let s = format_utc_ts(ms);
        assert_eq!(parse_utc_iso8601(&s), Some(ms));
    }

    #[test]
    fn roundtrips_for_epoch_zero() {
        assert_eq!(parse_utc_iso8601("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn roundtrips_across_many_days_including_month_and_year_boundaries() {
        for ms in [
            0_i64,
            86_400_000,        // 1970-01-02 — day boundary
            2_678_400_000,     // 1970-02-01 — month boundary
            31_536_000_000,    // 1971-01-01 — year boundary
            1_582_934_400_000, // 2020-02-29 — leap day
            1_785_539_455_000, // прод-сэмпл (см. выше)
        ] {
            let s = format_utc_ts(ms);
            assert_eq!(parse_utc_iso8601(&s), Some(ms), "roundtrip failed for {s}");
        }
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(parse_utc_iso8601("not-a-timestamp"), None);
        assert_eq!(parse_utc_iso8601(""), None);
        assert_eq!(parse_utc_iso8601("2026-07-31T23:10:55"), None); // no trailing Z
    }

    #[test]
    fn cron_failure_marker_splits_timestamp_and_detail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("retention.alert");
        std::fs::write(
            &path,
            "2026-07-31T23:43:00Z\ndry-run exit=2 (2=failed_cold_verify)",
        )
        .unwrap();
        let marker = read_cron_failure_marker(&path).expect("marker must be read");
        assert_eq!(
            marker.reported_at_ms,
            parse_utc_iso8601("2026-07-31T23:43:00Z")
        );
        assert!(marker.detail.contains("exit=2"));
    }

    #[test]
    fn cron_failure_marker_with_unparseable_first_line_still_returns_some() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("compaction.alert");
        std::fs::write(&path, "not-a-timestamp\ncompact exit=1").unwrap();
        let marker = read_cron_failure_marker(&path).expect("marker must be read");
        assert_eq!(marker.reported_at_ms, None);
        assert!(marker.detail.contains("exit=1"));
    }

    #[test]
    fn cron_failure_marker_missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.alert");
        assert!(read_cron_failure_marker(&path).is_none());
    }
}
