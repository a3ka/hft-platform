//! `ops-watchdog` — одноразовый cron-бинарь (founder-задача 2026-07-31, мост к M-43 task#3
//! "минимальный cron-watchdog→Telegram"). Читает `recorder.heartbeat` + cron-маркеры
//! (`/var/lib/hft/*.last-success`) + `docker ps`/`docker inspect`, прогоняет их через чистые
//! детекторы `ops::watchdog`, дедуплицирует через `ops::state::WatchdogState`, форматирует
//! (`ops::format`) и шлёт в оба транспорта (`StdoutTransport` — всегда, лог cron'а;
//! `TelegramTransport` — если сконфигурирован).
//!
//! ВСЁ I/O (файлы, `docker`, HTTP) — ЗДЕСЬ. Библиотечные модули `ops::watchdog`/`ops::format`
//! остаются чистыми и юнит-тестируемыми без реальной машины (см. их doc-комментарии).
//!
//! Установка в cron/`/var/lib/hft` — задача reviewer/founder после ревью (границы задачи);
//! этот бинарь и `scripts/watchdog_cron.sh` — готовый к установке артефакт, но НЕ
//! устанавливается мной на прод.
//!
//! Конфигурация — через env (значения по умолчанию соответствуют прод-топологии VPS,
//! `docs/SESSION-HANDOFF.md §7`):
//!   - `WATCHDOG_HEARTBEAT_PATH` (default
//!     `/var/lib/docker/volumes/hft-platform_journal-data/_data/recorder.heartbeat`)
//!   - `WATCHDOG_CRON_DIR` (default `/var/lib/hft`) — сканирует
//!     `compaction.last-success`, `gateway-checkpoint.last-success`, `retention.last-success`
//!   - `WATCHDOG_STATE_PATH` (default `/var/lib/hft/watchdog.state.json`)
//!   - `WATCHDOG_CONTAINERS` (default `hft-recorder,hft-gateway-serve`, запятая-разделитель)
//!   - `WATCHDOG_HOST_LABEL` (default `hft-platform-vps`)
//!   - `WATCHDOG_DEDUP_WINDOW_MS` (default `ops::state::DEFAULT_DEDUP_WINDOW_MS` = 30 мин)
//!   - `TELEGRAM_BOT_TOKEN` / `TELEGRAM_CHAT_ID` — см. `ops::transport::TelegramTransport`.
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
use ops::watchdog::{
    check_container_missing, check_container_restarted, check_container_unhealthy,
    check_cron_marker_missing, check_cron_marker_stale, check_disk, check_heartbeat_missing,
    check_heartbeat_stale, check_seq_stalled, check_writable, parse_docker_status_healthy, Alert,
    ContainerStatus, CronMarker, HeartbeatSample, Incident, Thresholds,
};

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
    let mut alerts: Vec<Alert> = Vec::new();

    run_heartbeat_checks(&heartbeat_path, now_ms, &thr, &mut state, &mut alerts);
    run_container_checks(&container_names, &mut state, &mut alerts);
    run_cron_marker_checks(&cron_dir, now_ms, &thr, &mut state, &mut alerts);

    let stdout_transport = StdoutTransport;
    let telegram_transport = TelegramTransport::from_env();
    if !telegram_transport.is_configured() {
        eprintln!(
            "[ops-watchdog] TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID не заданы — алерты идут только \
             в stdout (лог cron'а). Как только founder добавит токен в окружение, доставка в \
             Telegram включится без правок кода."
        );
    }

    let mut sent = 0usize;
    let mut suppressed = 0usize;
    for alert in &alerts {
        let key = dedup_key(alert);
        if !state.should_fire(&key, now_ms, dedup_window_ms) {
            suppressed += 1;
            continue;
        }
        let message = format_alert(alert, &host_label, now_ms);
        // stdout — всегда (лог cron'а — свидетельство работы watchdog'а само по себе).
        let _ = stdout_transport.send(&message);
        if let Err(e) = telegram_transport.send(&message) {
            eprintln!("[ops-watchdog] TelegramTransport::send failed: {e}");
        }
        sent += 1;
    }

    if alerts.is_empty() {
        println!("[ops-watchdog] {now_ms} — норма, ни одно условие не сработало");
    } else {
        println!(
            "[ops-watchdog] {now_ms} — обнаружено {} алертов ({} отправлено, {} подавлено \
             дедупликацией)",
            alerts.len(),
            sent,
            suppressed
        );
    }

    state
        .save(&state_path)
        .map_err(|e| anyhow::anyhow!("не удалось сохранить {}: {e}", state_path.display()))?;

    Ok(())
}

/// Ключ дедупликации/дедуп-состояния: код инцидента (+ `:<цель>` для условий с
/// множественностью — несколько контейнеров/cron-маркеров могут одновременно ловить один и
/// тот же `Incident`, и это РАЗНЫЕ инциденты — см. `watchdog::Alert::target`). ОДНА функция
/// для "слать" (`dedup_key`, из готового `Alert`) и "снять дедуп-запись, раз условие
/// здорово" (`push_or_clear`, до того как `Alert` мог не случиться) — иначе они бы могли
/// разойтись по формату ключа незаметно.
fn state_key(incident: Incident, target: Option<&str>) -> String {
    match target {
        Some(t) => format!("{}:{}", incident.code(), t),
        None => incident.code().to_string(),
    }
}

fn dedup_key(alert: &Alert) -> String {
    state_key(alert.incident, alert.target.as_deref())
}

fn run_heartbeat_checks(
    heartbeat_path: &Path,
    now_ms: i64,
    thr: &Thresholds,
    state: &mut WatchdogState,
    alerts: &mut Vec<Alert>,
) {
    let hb = read_heartbeat(heartbeat_path);

    if let Some(a) = check_heartbeat_missing(hb.as_ref()) {
        alerts.push(a);
    } else {
        state.clear(Incident::HeartbeatMissing.code());
    }

    let Some(hb) = hb else {
        // Без сэмпла остальные heartbeat-производные проверки бессмысленны в этом цикле;
        // prev_* НЕ обновляем (следующий цикл сравнит с последним ИЗВЕСТНЫМ хорошим сэмплом).
        return;
    };

    push_or_clear(
        alerts,
        state,
        Incident::HeartbeatStale,
        None,
        check_heartbeat_stale(now_ms, &hb, thr),
    );
    push_or_clear(
        alerts,
        state,
        Incident::NotWritable,
        None,
        check_writable(&hb),
    );

    match (state.prev_heartbeat, state.prev_check_ms) {
        (Some(prev_hb), Some(prev_ms)) => {
            push_or_clear(
                alerts,
                state,
                Incident::SeqStalled,
                None,
                check_seq_stalled(&prev_hb, prev_ms, &hb, now_ms, thr),
            );
            push_or_clear(
                alerts,
                state,
                Incident::DiskLow,
                None,
                check_disk(&hb, Some((&prev_hb, prev_ms)), now_ms, thr),
            );
        }
        _ => {
            push_or_clear(
                alerts,
                state,
                Incident::DiskLow,
                None,
                check_disk(&hb, None, now_ms, thr),
            );
        }
    }

    state.prev_heartbeat = Some(hb);
    state.prev_check_ms = Some(now_ms);
}

/// `target` — цель для дедуп-ключа (см. `state_key`); ДОЛЖЕН совпадать с тем, что кладёт в
/// `Alert::target` соответствующий `check_*` (проверено параллельно `Alert::with_target` в
/// `watchdog.rs` — если они разойдутся, `should_fire`/`clear` перестанут совпадать по ключу,
/// но тест `red_ops_watchdog.rs` этого не поймает — это wiring-риск, покрыт комментарием).
fn push_or_clear(
    alerts: &mut Vec<Alert>,
    state: &mut WatchdogState,
    incident: Incident,
    target: Option<&str>,
    outcome: Option<Alert>,
) {
    match outcome {
        Some(a) => alerts.push(a),
        None => state.clear(&state_key(incident, target)),
    }
}

fn run_container_checks(
    container_names: &[String],
    state: &mut WatchdogState,
    alerts: &mut Vec<Alert>,
) {
    let ps_output = run_docker_ps();
    for name in container_names {
        let status = container_status(name, ps_output.as_deref());

        push_or_clear(
            alerts,
            state,
            Incident::ContainerMissing,
            Some(name),
            check_container_missing(&status),
        );
        push_or_clear(
            alerts,
            state,
            Incident::ContainerUnhealthy,
            Some(name),
            check_container_unhealthy(&status),
        );

        let prev_restart = state.prev_restart_counts.get(name).copied();
        if let Some(a) = check_container_restarted(&status, prev_restart) {
            alerts.push(a);
        }
        if let Some(cur) = status.restart_count {
            state.prev_restart_counts.insert(name.clone(), cur);
        }
    }
}

fn run_cron_marker_checks(
    cron_dir: &Path,
    now_ms: i64,
    thr: &Thresholds,
    state: &mut WatchdogState,
    alerts: &mut Vec<Alert>,
) {
    const MARKERS: &[&str] = &[
        "compaction.last-success",
        "gateway-checkpoint.last-success",
        "retention.last-success",
    ];
    for name in MARKERS {
        let last_success_ms = read_cron_marker(&cron_dir.join(name));
        let marker = CronMarker {
            name,
            last_success_ms,
        };
        push_or_clear(
            alerts,
            state,
            Incident::CronMarkerMissing,
            Some(name),
            check_cron_marker_missing(&marker),
        );
        push_or_clear(
            alerts,
            state,
            Incident::CronMarkerStale,
            Some(name),
            check_cron_marker_stale(&marker, now_ms, thr),
        );
    }
}

fn read_heartbeat(path: &Path) -> Option<HeartbeatSample> {
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
    fn state_key_matches_dedup_key_for_targeted_alert() {
        // wiring-инвариант, упомянутый в doc-комментарии `push_or_clear`: ключ, под которым
        // клирится дедуп-запись, ОБЯЗАН совпадать с ключом, под которым она была установлена
        // через `dedup_key(&alert)`.
        use ops::watchdog::{Alert, Incident, Level};
        let alert = Alert {
            incident: Incident::ContainerUnhealthy,
            level: Level::Critical,
            message: "x".to_string(),
            target: Some("hft-recorder".to_string()),
        };
        assert_eq!(
            dedup_key(&alert),
            state_key(Incident::ContainerUnhealthy, Some("hft-recorder"))
        );
    }
}
