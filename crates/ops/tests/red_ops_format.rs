//! RED-спека форматирования (задача §3: "человекочитаемое, с фактами: что сломалось, когда,
//! какие цифры, что делать"). Контракт: `format_alert` несёт уровень, код инцидента, текст
//! детектора (уже с цифрами), host, обнаружение-время (детерминированный UTC-рендер) и ссылку
//! на runbook.

use ops::format::{format_alert, format_utc_ts};
use ops::watchdog::{Alert, Incident, Level};

fn sample_alert() -> Alert {
    Alert {
        incident: Incident::HeartbeatStale,
        level: Level::Critical,
        message: "recorder heartbeat не обновлялся 200000 мс (порог CRITICAL 180000 мс)"
            .to_string(),
        target: None,
    }
}

#[test]
fn message_carries_level_and_incident_code() {
    let out = format_alert(&sample_alert(), "hft-recorder@vps", 1_785_539_455_840);
    assert!(out.starts_with("[CRITICAL] WD-HB-STALE"), "got: {out}");
}

#[test]
fn message_carries_original_detector_text_with_numbers() {
    let out = format_alert(&sample_alert(), "hft-recorder@vps", 0);
    assert!(
        out.contains("200000 мс"),
        "must carry the concrete figure: {out}"
    );
}

#[test]
fn message_carries_host_label() {
    let out = format_alert(&sample_alert(), "hft-recorder@vps", 0);
    assert!(out.contains("hft-recorder@vps"), "got: {out}");
}

#[test]
fn message_carries_runbook_reference() {
    let out = format_alert(&sample_alert(), "h", 0);
    assert!(out.contains("docs/runbooks/alerting.md"), "got: {out}");
    assert!(
        out.contains("wd-hb-stale"),
        "anchor should be lowercase incident code: {out}"
    );
}

#[test]
fn warning_level_label_renders_as_warning() {
    let mut a = sample_alert();
    a.level = Level::Warning;
    let out = format_alert(&a, "h", 0);
    assert!(out.starts_with("[WARNING]"), "got: {out}");
}

// ─────────────────────────── format_utc_ts — прод-замеры 2026-07-31 ───────────────────────────

#[test]
fn utc_ts_epoch_zero() {
    assert_eq!(format_utc_ts(0), "1970-01-01T00:00:00Z");
}

#[test]
fn utc_ts_matches_real_prod_heartbeat_sample() {
    // Реальный `recorder.heartbeat.ts_wall_ms`, снятый со VPS 2026-07-31 23:10:55 UTC
    // (см. отчёт агента); проверено независимо через `date -u -d @1785539455`.
    assert_eq!(format_utc_ts(1_785_539_455_840), "2026-07-31T23:10:55Z");
}

#[test]
fn utc_ts_arbitrary_known_timestamp() {
    assert_eq!(format_utc_ts(1_700_000_000_000), "2023-11-14T22:13:20Z");
}

#[test]
fn utc_ts_deterministic_across_calls() {
    // Анти-плацебо: тот же вход → тот же выход, без часов внутри (testing.md, детерминизм).
    let a = format_utc_ts(1_785_539_455_840);
    let b = format_utc_ts(1_785_539_455_840);
    assert_eq!(a, b);
}
