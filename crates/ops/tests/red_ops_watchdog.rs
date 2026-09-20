//! RED-спека watchdog-детекторов (founder-задача 2026-07-31, мост к M-43 task#3).
//!
//! Контракт из задачи: "по тесту на условие, с фикстурой" + "НЕ срабатывает на здоровом
//! состоянии" (парный vantage — иначе получим генератор ложных тревог) + "дедупликация" +
//! "TelegramTransport без токена не паникует" (дедуп/транспорт — в отдельных файлах).
//!
//! Анти-плацебо: КАЖДЫЙ тест на "срабатывает" имеет напарника на "не срабатывает" с ТЕМ ЖЕ
//! детектором на здоровом входе — заглушка-детектор, который всегда молчит, ловится
//! половиной этих тестов; заглушка, которая всегда алертит, ловится второй половиной.

use ops::watchdog::{
    check_container_missing, check_container_restarted, check_container_unhealthy,
    check_cron_marker_missing, check_cron_marker_stale, check_disk, check_heartbeat_missing,
    check_heartbeat_stale, check_seq_stalled, check_writable, parse_docker_status_healthy,
    ContainerStatus, CronMarker, HeartbeatSample, Incident, Level, Thresholds,
};

fn healthy_hb(ts_wall_ms: i64) -> HeartbeatSample {
    HeartbeatSample {
        ts_wall_ms,
        next_seq: 140_000_000,
        segment_index: 145,
        events: 3_000_000,
        free_bytes: Some(83_326_705_664),
        min_free_bytes: Some(10_737_418_240),
        writable: Some(true),
    }
}

// ─────────────────────────── heartbeat missing ───────────────────────────

#[test]
fn heartbeat_missing_fires_when_no_sample() {
    let alert = check_heartbeat_missing(None).expect("None должен алертить");
    assert_eq!(alert.incident, Incident::HeartbeatMissing);
    assert_eq!(alert.level, Level::Critical);
}

#[test]
fn heartbeat_missing_silent_when_sample_present() {
    let hb = healthy_hb(1_000);
    assert!(check_heartbeat_missing(Some(&hb)).is_none());
}

// ─────────────────────────── heartbeat stale ───────────────────────────

#[test]
fn heartbeat_stale_silent_on_fresh_tick() {
    let thr = Thresholds::default();
    let hb = healthy_hb(100_000);
    // 10с спустя тика — норма (recorder тикает каждые 10с).
    assert!(check_heartbeat_stale(110_000, &hb, &thr).is_none());
}

#[test]
fn heartbeat_stale_warns_past_warn_threshold() {
    let thr = Thresholds::default();
    let hb = healthy_hb(0);
    let now = thr.heartbeat_warn_ms + 1;
    let alert = check_heartbeat_stale(now, &hb, &thr).expect("должно WARNING-алертить");
    assert_eq!(alert.incident, Incident::HeartbeatStale);
    assert_eq!(alert.level, Level::Warning);
}

#[test]
fn heartbeat_stale_critical_past_crit_threshold() {
    let thr = Thresholds::default();
    let hb = healthy_hb(0);
    let now = thr.heartbeat_crit_ms + 1;
    let alert = check_heartbeat_stale(now, &hb, &thr).expect("должно CRITICAL-алертить");
    assert_eq!(alert.incident, Incident::HeartbeatStale);
    assert_eq!(alert.level, Level::Critical);
}

#[test]
fn heartbeat_stale_exact_warn_boundary_is_not_yet_stale() {
    // Строгое `>`, не `>=` — граничный кейс (testing.md п.4 "границы").
    let thr = Thresholds::default();
    let hb = healthy_hb(0);
    assert!(check_heartbeat_stale(thr.heartbeat_warn_ms, &hb, &thr).is_none());
}

// ─────────────────────────── writable ───────────────────────────

#[test]
fn not_writable_fires_on_explicit_false() {
    let mut hb = healthy_hb(0);
    hb.writable = Some(false);
    let alert = check_writable(&hb).expect("writable=false должен алертить");
    assert_eq!(alert.incident, Incident::NotWritable);
    assert_eq!(alert.level, Level::Critical);
}

#[test]
fn not_writable_silent_on_true() {
    let hb = healthy_hb(0);
    assert!(check_writable(&hb).is_none());
}

#[test]
fn not_writable_silent_on_unknown_none() {
    // testing.md п.3 "отсутствие ≠ сигнал к удалению/обнулению" — recorder сам не знает,
    // watchdog не додумывает.
    let mut hb = healthy_hb(0);
    hb.writable = None;
    assert!(check_writable(&hb).is_none());
}

// ─────────────────────────── seq stalled (самый опасный класс) ───────────────────────────

#[test]
fn seq_stalled_fires_when_flat_across_checks() {
    let thr = Thresholds::default();
    let prev = healthy_hb(0);
    let mut cur = healthy_hb(300_000);
    cur.next_seq = prev.next_seq; // ни одного нового события за 5 минут
    let alert = check_seq_stalled(&prev, 0, &cur, 300_000, &thr).expect("должен алертить");
    assert_eq!(alert.incident, Incident::SeqStalled);
    assert_eq!(alert.level, Level::Critical);
}

#[test]
fn seq_stalled_silent_when_growing() {
    let thr = Thresholds::default();
    let prev = healthy_hb(0);
    let mut cur = healthy_hb(300_000);
    cur.next_seq = prev.next_seq + 28_000; // ~96 events/s × 300с (замер на проде)
    assert!(check_seq_stalled(&prev, 0, &cur, 300_000, &thr).is_none());
}

#[test]
fn seq_stalled_ignored_when_checks_too_close_together() {
    // Анти-флап: два прогона в одной пачке (< seq_stall_min_gap_ms) не должны читаться как
    // "recorder встал" — next_seq ещё физически не успел бы вырасти заметно.
    let thr = Thresholds::default();
    let prev = healthy_hb(0);
    let mut cur = healthy_hb(5_000);
    cur.next_seq = prev.next_seq; // тот же seq, но gap мал
    assert!(check_seq_stalled(&prev, 0, &cur, 5_000, &thr).is_none());
}

// ─────────────────────────── disk ───────────────────────────

#[test]
fn disk_silent_with_ample_free_space_no_trend() {
    let thr = Thresholds::default();
    let hb = healthy_hb(0); // free≈77.6 GiB, min_free=10 GiB — далеко от 3×min_free
    assert!(check_disk(&hb, None, 0, &thr).is_none());
}

#[test]
fn disk_critical_at_or_below_min_free_bytes() {
    let thr = Thresholds::default();
    let mut hb = healthy_hb(0);
    hb.free_bytes = Some(hb.min_free_bytes.unwrap()); // ровно на полу
    let alert = check_disk(&hb, None, 0, &thr).expect("должен алертить");
    assert_eq!(alert.incident, Incident::DiskLow);
    assert_eq!(alert.level, Level::Critical);
}

#[test]
fn disk_warns_when_absolute_backstop_under_three_times_min_free_no_trend() {
    let thr = Thresholds::default();
    let mut hb = healthy_hb(0);
    let min_free = hb.min_free_bytes.unwrap();
    hb.free_bytes = Some(min_free * 2); // < 3×min_free, но > min_free
    let alert = check_disk(&hb, None, 0, &thr).expect("должен WARNING-алертить (backstop)");
    assert_eq!(alert.level, Level::Warning);
}

#[test]
fn disk_projects_critical_hours_from_measured_decline_rate() {
    // Замер на проде 2026-07-31: 30с дало убыль 3_514_368 байт (~117 КБ/с). На этом темпе
    // при небольшом запасе сверх min_free прогноз должен уйти в CRITICAL (< disk_crit_hours).
    let thr = Thresholds::default();
    let min_free = 10_737_418_240_i64;
    let decline_bps = 3_514_368_f64 / 30.0;
    // Хотим hours_left чуть меньше disk_crit_hours (24ч): подбираем запас сверх min_free.
    let target_seconds = (thr.disk_crit_hours - 1.0) * 3600.0;
    let headroom = (decline_bps * target_seconds) as i64;
    let prev = HeartbeatSample {
        free_bytes: Some(min_free + headroom + 3_514_368),
        min_free_bytes: Some(min_free),
        ..healthy_hb(0)
    };
    let cur = HeartbeatSample {
        ts_wall_ms: 30_000,
        free_bytes: Some(min_free + headroom),
        min_free_bytes: Some(min_free),
        ..healthy_hb(30_000)
    };
    let alert = check_disk(&cur, Some((&prev, 0)), 30_000, &thr).expect("должен алертить");
    assert_eq!(alert.incident, Incident::DiskLow);
    assert_eq!(alert.level, Level::Critical);
}

#[test]
fn disk_silent_when_trend_is_growing_free_space() {
    // Ретеншен освободил место между сэмплами — падение НЕ должно читаться как убыль.
    let thr = Thresholds::default();
    let min_free = 10_737_418_240_i64;
    let prev = HeartbeatSample {
        free_bytes: Some(min_free * 4),
        min_free_bytes: Some(min_free),
        ..healthy_hb(0)
    };
    let cur = HeartbeatSample {
        ts_wall_ms: 30_000,
        free_bytes: Some(min_free * 5), // выросло
        min_free_bytes: Some(min_free),
        ..healthy_hb(30_000)
    };
    assert!(check_disk(&cur, Some((&prev, 0)), 30_000, &thr).is_none());
}

#[test]
fn disk_silent_when_disk_fields_unknown() {
    let thr = Thresholds::default();
    let mut hb = healthy_hb(0);
    hb.free_bytes = None;
    hb.min_free_bytes = None;
    assert!(check_disk(&hb, None, 0, &thr).is_none());
}

// ─────────────────────────── container ───────────────────────────

fn healthy_container(name: &str) -> ContainerStatus {
    ContainerStatus {
        name: name.to_string(),
        healthy: Some(true),
        restart_count: Some(0),
    }
}

#[test]
fn container_missing_fires_when_not_seen_in_docker_ps() {
    let status = ContainerStatus {
        name: "hft-recorder".to_string(),
        healthy: None,
        restart_count: None,
    };
    let alert = check_container_missing(&status).expect("должен алертить");
    assert_eq!(alert.incident, Incident::ContainerMissing);
    assert_eq!(alert.level, Level::Critical);
    assert_eq!(alert.target.as_deref(), Some("hft-recorder"));
}

#[test]
fn container_missing_silent_when_present() {
    let status = healthy_container("hft-recorder");
    assert!(check_container_missing(&status).is_none());
}

#[test]
fn container_unhealthy_fires_on_explicit_false() {
    let status = ContainerStatus {
        name: "hft-recorder".to_string(),
        healthy: Some(false),
        restart_count: Some(1),
    };
    let alert = check_container_unhealthy(&status).expect("должен алертить");
    assert_eq!(alert.incident, Incident::ContainerUnhealthy);
    assert_eq!(alert.level, Level::Critical);
}

#[test]
fn container_unhealthy_silent_when_healthy() {
    let status = healthy_container("hft-recorder");
    assert!(check_container_unhealthy(&status).is_none());
}

#[test]
fn container_unhealthy_silent_when_missing_not_double_counted() {
    // Missing и Unhealthy — РАЗНЫЕ инциденты; missing НЕ должен также светить unhealthy.
    let status = ContainerStatus {
        name: "hft-recorder".to_string(),
        healthy: None,
        restart_count: None,
    };
    assert!(check_container_unhealthy(&status).is_none());
}

#[test]
fn container_restarted_fires_when_count_increased() {
    let status = ContainerStatus {
        name: "hft-recorder".to_string(),
        healthy: Some(true),
        restart_count: Some(3),
    };
    let alert = check_container_restarted(&status, Some(2)).expect("должен алертить");
    assert_eq!(alert.incident, Incident::ContainerRestarted);
    assert_eq!(alert.level, Level::Warning);
}

#[test]
fn container_restarted_silent_when_unchanged() {
    let status = ContainerStatus {
        name: "hft-recorder".to_string(),
        healthy: Some(true),
        restart_count: Some(3),
    };
    assert!(check_container_restarted(&status, Some(3)).is_none());
}

#[test]
fn container_restarted_silent_without_baseline() {
    // Первый прогон — нет prev, нельзя утверждать "выросло".
    let status = healthy_container("hft-recorder");
    assert!(check_container_restarted(&status, None).is_none());
}

#[test]
fn parse_docker_status_healthy_known_bad_statuses() {
    assert!(!parse_docker_status_healthy("Up 2 minutes (unhealthy)"));
    assert!(!parse_docker_status_healthy("Restarting (1) 5 seconds ago"));
    assert!(!parse_docker_status_healthy("Exited (137) 3 minutes ago"));
}

#[test]
fn parse_docker_status_healthy_known_good_statuses() {
    assert!(parse_docker_status_healthy("Up 9 hours (healthy)"));
    assert!(parse_docker_status_healthy(
        "Up 5 seconds (health: starting)"
    ));
    assert!(parse_docker_status_healthy("Up 3 hours"));
}

// ─────────────────────────── cron marker ───────────────────────────

#[test]
fn cron_marker_missing_fires_when_absent() {
    let marker = CronMarker {
        name: "retention.last-success",
        last_success_ms: None,
    };
    let alert = check_cron_marker_missing(&marker).expect("должен алертить");
    assert_eq!(alert.incident, Incident::CronMarkerMissing);
    assert_eq!(alert.target.as_deref(), Some("retention.last-success"));
}

#[test]
fn cron_marker_missing_silent_when_present() {
    let marker = CronMarker {
        name: "retention.last-success",
        last_success_ms: Some(0),
    };
    assert!(check_cron_marker_missing(&marker).is_none());
}

#[test]
fn cron_marker_stale_silent_within_a_day() {
    let thr = Thresholds::default();
    let marker = CronMarker {
        name: "retention.last-success",
        last_success_ms: Some(0),
    };
    let now = 20 * 3_600_000; // 20ч спустя — суточная задача, ещё не пора беспокоиться
    assert!(check_cron_marker_stale(&marker, now, &thr).is_none());
}

#[test]
fn cron_marker_stale_warns_past_26h() {
    let thr = Thresholds::default();
    let marker = CronMarker {
        name: "retention.last-success",
        last_success_ms: Some(0),
    };
    let alert = check_cron_marker_stale(&marker, thr.cron_warn_age_ms + 1, &thr).expect("WARNING");
    assert_eq!(alert.level, Level::Warning);
}

#[test]
fn cron_marker_stale_critical_past_48h() {
    let thr = Thresholds::default();
    let marker = CronMarker {
        name: "retention.last-success",
        last_success_ms: Some(0),
    };
    let alert = check_cron_marker_stale(&marker, thr.cron_crit_age_ms + 1, &thr).expect("CRITICAL");
    assert_eq!(alert.level, Level::Critical);
}

// ───────────────── anti-plaebo: полностью здоровое состояние → НОЛЬ алертов ─────────────────

#[test]
fn fully_healthy_snapshot_yields_zero_alerts_across_every_check() {
    // Парный vantage требования задачи: "не срабатывает на здоровом состоянии" — прогоняем
    // ВСЕ детекторы разом на заведомо здоровых фикстурах. Если бы детектор был заглушкой
    // "всегда Some(...)", этот тест бы упал.
    let thr = Thresholds::default();
    let prev = healthy_hb(0);
    let mut cur = healthy_hb(30_000);
    cur.next_seq = prev.next_seq + 2_881; // растёт (замер на проде за 30с)

    assert!(check_heartbeat_missing(Some(&cur)).is_none());
    assert!(check_heartbeat_stale(30_000, &cur, &thr).is_none());
    assert!(check_writable(&cur).is_none());
    assert!(check_seq_stalled(&prev, 0, &cur, 30_000, &thr).is_none()); // gap < min_gap, но и растёт
    assert!(check_disk(&cur, Some((&prev, 0)), 30_000, &thr).is_none());

    let container = healthy_container("hft-recorder");
    assert!(check_container_missing(&container).is_none());
    assert!(check_container_unhealthy(&container).is_none());
    assert!(check_container_restarted(&container, Some(0)).is_none());

    let marker = CronMarker {
        name: "retention.last-success",
        last_success_ms: Some(0),
    };
    assert!(check_cron_marker_missing(&marker).is_none());
    assert!(check_cron_marker_stale(&marker, 3_600_000, &thr).is_none());
}
