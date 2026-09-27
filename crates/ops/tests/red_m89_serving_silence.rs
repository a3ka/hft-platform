//! RED `M-89` (sacred, architect-only) — **watchdog читает сердцебиение ВЫДАЧИ и алертит
//! на молчание и на несвежесть** (`I-6`, `TD-220`, `OPS-I-8` распространён на выдачу,
//! `PL-I-8`).
//!
//! # Что закрывается
//!
//! `M-87` построил правило тревоги `metrics::silence_alarm` ВНУТРИ процесса выдачи и не
//! вывел его наружу (`M-87` §11 п. 5, `A-037` У-7; `TD-220` MAJOR). Алерт, живущий на
//! наблюдаемой машине в наблюдаемом процессе, не алерт: если процесс молчит, молчит и он.
//! Единственный внешний наблюдатель в корпусе — `ops-watchdog` (cron на хосте, читает
//! `recorder.heartbeat` из тома docker'а). Он получает ВТОРОЙ вход — сердцебиение выдачи —
//! и ДВА класса условий: (а) свежесть файла (наблюдать ОТСУТСТВИЕ, `testing.md` св. 4);
//! (б) молчание выдачи: за окно попытки ЕСТЬ, поддержанные отказы ЕСТЬ, успехов НЕТ;
//! поток заведомо неподдержанных тревогу не держит (`M-87` §7).
//!
//! # Три состояния входа, а не два (`C-260` R1)
//!
//! Первая редакция кодировала вход как `Option<ServingHeartbeatSample>`, и `None` означал
//! ОДНОВРЕМЕННО «интеграции нет» (legacy `run_cycle`, тревоги быть не должно) и «файл не
//! прочитан» (тревога ОБЯЗАНА быть) — g1/g4 и g4b требовали противоположного от одного
//! значения. Круг 2: вход — перечисление из ТРЁХ состояний:
//!
//! ```ignore
//! // crates/ops/src/watchdog.rs
//! #[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
//! pub struct ServingHeartbeatSample {
//!     pub ts_wall_ms: i64, pub attempts: u64, pub successes: u64,
//!     pub refusals_supported: u64, pub refusals_unsupported: u64,
//!     pub journal_payload_bytes_read: u64, pub slots_in_flight: u64,
//! }
//! /// Вход «сердцебиение выдачи» одного такта.
//! #[derive(Debug, Clone, Copy, PartialEq, Default)]
//! pub enum ServingHeartbeat {
//!     /// Интеграции нет (legacy `run_cycle`): serving-проверки НЕ исполняются, состояние
//!     /// serving-якоря НЕ трогается. Это `Default`.
//!     #[default]
//!     Disabled,
//!     /// Путь задан, файл не прочитан/не разобран ⇒ `WD-SERVING-HB-MISSING`; якорь
//!     /// молчания НЕ сбрасывается (нечитаемый такт не стирает историю, R-005 F-1).
//!     ConfiguredMissing,
//!     Present(ServingHeartbeatSample),
//! }
//! pub enum Incident { …, ServingHeartbeatMissing, ServingHeartbeatStale, ServingSilence }
//! //   коды: WD-SERVING-HB-MISSING · WD-SERVING-HB-STALE · WD-SERVING-SILENCE
//! pub struct Thresholds { …, pub serving_heartbeat_warn_ms: i64, pub serving_heartbeat_crit_ms: i64 }
//! pub fn check_serving_heartbeat_missing(hb: &ServingHeartbeat) -> Option<Alert>;
//! pub fn check_serving_heartbeat_stale(now_ms: i64, hb: &ServingHeartbeatSample, thr: &Thresholds) -> Option<Alert>;
//! pub fn check_serving_silence(prev: &ServingHeartbeatSample, cur: &ServingHeartbeatSample) -> Option<Alert>;
//!
//! // crates/ops/src/watchdog_cycle.rs — `CycleInputs` НЕ меняется (12 литералов в
//! // `red_ops_watchdog_cycle.rs`); второй вход идёт отдельной структурой:
//! #[derive(Debug, Clone, Default)]
//! pub struct ServingInputs { pub heartbeat: ServingHeartbeat }   // Default = Disabled
//! pub fn run_cycle_full(inputs: &CycleInputs, serving: &ServingInputs, now_ms: i64,
//!                       thr: &Thresholds, dedup_window_ms: i64, state: &mut WatchdogState) -> CycleOutcome;
//! // `run_cycle(..)` ≡ `run_cycle_full(.., &ServingInputs::default(), ..)` — `Disabled`.
//! // crates/ops/src/state.rs: `#[serde(default)] pub prev_serving_heartbeat: Option<ServingHeartbeatSample>`
//! // crates/ops/src/bin/ops-watchdog.rs: env `WATCHDOG_SERVING_HEARTBEAT_PATH`
//! //   (default `/var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat`)
//! //   — бинарь зовёт ТОЛЬКО `run_cycle_full`; путь у него ЕСТЬ ВСЕГДА (env либо дефолт),
//! //   поэтому он НИКОГДА не передаёт `Disabled`: нечитаемый файл ⇒ `ConfiguredMissing`.
//! ```
//!
//! # Что здесь пиннится
//!
//! · `g1` — три состояния входа: `Disabled` молчит, `ConfiguredMissing` алертит CRITICAL,
//!   `Present` молчит; `g2` — несвежесть (WARNING → CRITICAL, свежий молчит);
//! · `g3` — правило молчания на дельтах: три «молчит» и одно «звенит»;
//! · `g4` — склейка: дедуп и якорь `prev_serving_heartbeat` живут в СОСТОЯНИИ между
//!   тактами; `ConfiguredMissing` звенит и якорь не стирает; `Disabled` не трогает ничего;
//! · `g4b` — legacy `run_cycle` ≡ `run_cycle_full(.., Disabled)` ИСПОЛНЕНИЕМ (равные исходы),
//!   ни одного serving-инцидента; `g4c` — парный vantage: `ConfiguredMissing` на том же
//!   входе даёт `WD-SERVING-HB-MISSING`;
//! · `g5` — ТОЧКА ВХОДА бинаря: путь задан, файла нет ⇒ `WD-SERVING-HB-MISSING`
//!   (исполняемый случай «configured-missing»); старый файл ⇒ STALE; свежий ⇒ тишина.
//!
//! COMPILE-RED на ревизии набора: типов и функций не существует. `g5` — RUNTIME-RED и без
//! них (бинарь переменную не читает), но собирается только вместе с файлом.

use ops::state::WatchdogState;
use ops::watchdog::{
    check_serving_heartbeat_missing, check_serving_heartbeat_stale, check_serving_silence,
    Incident, Level, ServingHeartbeat, ServingHeartbeatSample, Thresholds,
};
use ops::watchdog_cycle::{run_cycle_full, CycleInputs, CycleOutcome, ServingInputs};

fn hb(
    ts_wall_ms: i64,
    attempts: u64,
    successes: u64,
    sup: u64,
    unsup: u64,
) -> ServingHeartbeatSample {
    ServingHeartbeatSample {
        ts_wall_ms,
        attempts,
        successes,
        refusals_supported: sup,
        refusals_unsupported: unsup,
        journal_payload_bytes_read: 1_000,
        slots_in_flight: 0,
    }
}

// ─────────────────────────── g1 — три состояния входа ───────────────────────────

#[test]
fn g1_missing_fires_only_when_configured_disabled_and_present_are_silent() {
    assert!(
        matches!(ServingHeartbeat::default(), ServingHeartbeat::Disabled),
        "Default обязан быть Disabled — иначе legacy run_cycle получает тревогу (C-260 R1)"
    );
    assert!(
        check_serving_heartbeat_missing(&ServingHeartbeat::Disabled).is_none(),
        "Disabled — интеграции нет, тревоги нет"
    );
    let a = check_serving_heartbeat_missing(&ServingHeartbeat::ConfiguredMissing)
        .expect("ConfiguredMissing обязан алертить");
    assert_eq!(a.incident, Incident::ServingHeartbeatMissing);
    assert_eq!(a.level, Level::Critical);
    assert_eq!(a.incident.code(), "WD-SERVING-HB-MISSING");
    let s = hb(1_000, 0, 0, 0, 0);
    assert!(check_serving_heartbeat_missing(&ServingHeartbeat::Present(s)).is_none());
}

// ─────────────────────────── g2 — несвежесть ───────────────────────────

#[test]
fn g2_stale_serving_heartbeat_warns_then_criticals_and_fresh_is_silent() {
    let thr = Thresholds::default();
    assert!(
        thr.serving_heartbeat_warn_ms > 0
            && thr.serving_heartbeat_crit_ms > thr.serving_heartbeat_warn_ms,
        "пороги свежести выдачи обязаны быть положительными и упорядоченными"
    );
    let s = hb(0, 0, 0, 0, 0);
    assert!(check_serving_heartbeat_stale(thr.serving_heartbeat_warn_ms / 2, &s, &thr).is_none());
    let w = check_serving_heartbeat_stale(thr.serving_heartbeat_warn_ms + 1, &s, &thr)
        .expect("WARNING");
    assert_eq!(
        (w.incident, w.level),
        (Incident::ServingHeartbeatStale, Level::Warning)
    );
    let c = check_serving_heartbeat_stale(thr.serving_heartbeat_crit_ms + 1, &s, &thr)
        .expect("CRITICAL");
    assert_eq!(
        (c.incident, c.level),
        (Incident::ServingHeartbeatStale, Level::Critical)
    );
    assert_eq!(c.incident.code(), "WD-SERVING-HB-STALE");
}

// ─────────────────────────── g3 — молчание по дельтам ───────────────────────────

/// Правило `M-87` §7 на ДЕЛЬТАХ: попытки выросли, поддержанные отказы выросли, успехи — нет.
#[test]
fn g3_silence_rule_on_deltas_fires_only_on_starved_supported_flow() {
    let prev = hb(0, 100, 50, 30, 20);
    // (а) звенит: +10 попыток, +10 поддержанных отказов, +0 успехов.
    let starved = hb(60_000, 110, 50, 40, 20);
    let a = check_serving_silence(&prev, &starved).expect("голодание обязано алертить");
    assert_eq!(a.incident, Incident::ServingSilence);
    assert_eq!(a.incident.code(), "WD-SERVING-SILENCE");
    // (б) молчит: успех был.
    let ok = hb(60_000, 110, 51, 39, 20);
    assert!(
        check_serving_silence(&prev, &ok).is_none(),
        "успех за окно — тревоги нет"
    );
    // (в) молчит: только мусор (неподдержанные) — поток мусора не держит тревогу.
    let junk = hb(60_000, 110, 50, 30, 30);
    assert!(
        check_serving_silence(&prev, &junk).is_none(),
        "неподдержанный поток — тревоги нет"
    );
    // (г) молчит: попыток не было.
    let idle = hb(60_000, 100, 50, 30, 20);
    assert!(
        check_serving_silence(&prev, &idle).is_none(),
        "никто не просил ≠ просили, не получили"
    );
    // (д) fail-closed на регрессе счётчиков (рестарт процесса): дельты отрицательные — не
    // «голодание», а новый якорь; тревоги нет, ложного срабатывания на рестарте нет.
    let restarted = hb(60_000, 3, 0, 3, 0);
    assert!(
        check_serving_silence(&prev, &restarted).is_none(),
        "рестарт процесса — не голодание"
    );
}

// ─────────────────────────── g4 — склейка и состояние ───────────────────────────

fn inputs() -> CycleInputs {
    CycleInputs {
        heartbeat: None,
        containers: vec![],
        cron_jobs: vec![],
    }
}

fn present(s: ServingHeartbeatSample) -> ServingInputs {
    ServingInputs {
        heartbeat: ServingHeartbeat::Present(s),
    }
}

fn fired(out: &CycleOutcome, incident: Incident) -> usize {
    out.fired.iter().filter(|a| a.incident == incident).count()
}
fn delivered(out: &CycleOutcome, incident: Incident) -> usize {
    out.delivered
        .iter()
        .filter(|a| a.incident == incident)
        .count()
}

/// Такты cron'а — отдельные процессы: состояние проходит через JSON между тактами
/// (образец `red_ops_watchdog_cycle.rs::CronSim`).
fn roundtrip(state: &WatchdogState) -> WatchdogState {
    let s = serde_json::to_string(state).expect("state → json");
    serde_json::from_str(&s).expect("json → state")
}

#[test]
fn g4_cycle_keeps_serving_anchor_in_state_and_dedups_silence() {
    let thr = Thresholds::default();
    let dedup = 30 * 60_000;
    let mut state = WatchdogState::default();

    // Такт 1: первый сэмпл — якоря нет, молчание судить не с чем; несвежести нет.
    let t1 = 1_000_000;
    let o1 = run_cycle_full(
        &inputs(),
        &present(hb(t1, 100, 50, 30, 20)),
        t1,
        &thr,
        dedup,
        &mut state,
    );
    assert_eq!(
        fired(&o1, Incident::ServingSilence),
        0,
        "без якоря молчание не судится"
    );
    assert_eq!(fired(&o1, Incident::ServingHeartbeatMissing), 0);
    let mut state = roundtrip(&state);

    // Такт 2: голодание относительно якоря — звенит и доставляется.
    let t2 = t1 + 60_000;
    let o2 = run_cycle_full(
        &inputs(),
        &present(hb(t2, 110, 50, 40, 20)),
        t2,
        &thr,
        dedup,
        &mut state,
    );
    assert_eq!(
        fired(&o2, Incident::ServingSilence),
        1,
        "голодание не обнаружено на такте 2"
    );
    assert_eq!(delivered(&o2, Incident::ServingSilence), 1);
    let mut state = roundtrip(&state);

    // Такт 3: то же голодание — сработало, но подавлено дедупом.
    let t3 = t2 + 60_000;
    let o3 = run_cycle_full(
        &inputs(),
        &present(hb(t3, 120, 50, 50, 20)),
        t3,
        &thr,
        dedup,
        &mut state,
    );
    assert_eq!(fired(&o3, Incident::ServingSilence), 1);
    assert_eq!(
        delivered(&o3, Incident::ServingSilence),
        0,
        "дедуп-окно не применено"
    );
    let mut state = roundtrip(&state);

    // Такт 4: файл исчез при ЗАДАННОМ пути — отсутствие звенит; якорь НЕ сбрасывается
    // (нечитаемый такт не стирает историю — та же дисциплина, что `f1_unreadable_heartbeat_tick…`).
    let t4 = t3 + 60_000;
    let o4 = run_cycle_full(
        &inputs(),
        &ServingInputs {
            heartbeat: ServingHeartbeat::ConfiguredMissing,
        },
        t4,
        &thr,
        dedup,
        &mut state,
    );
    assert_eq!(fired(&o4, Incident::ServingHeartbeatMissing), 1);
    let anchor_after_missing = state.prev_serving_heartbeat;
    assert!(
        anchor_after_missing.is_some(),
        "нечитаемый такт сбросил якорь молчания"
    );
    let mut state = roundtrip(&state);

    // Такт 5: интеграция выключена (Disabled) — ни одной serving-тревоги, состояние
    // serving-якоря НЕ тронуто (ни сброса, ни обновления).
    let t5 = t4 + 60_000;
    let o5 = run_cycle_full(
        &inputs(),
        &ServingInputs::default(),
        t5,
        &thr,
        dedup,
        &mut state,
    );
    assert_eq!(
        fired(&o5, Incident::ServingHeartbeatMissing),
        0,
        "Disabled дал MISSING"
    );
    assert_eq!(
        fired(&o5, Incident::ServingSilence),
        0,
        "Disabled дал SILENCE"
    );
    assert_eq!(
        fired(&o5, Incident::ServingHeartbeatStale),
        0,
        "Disabled дал STALE"
    );
    assert_eq!(
        state.prev_serving_heartbeat, anchor_after_missing,
        "Disabled тронул serving-якорь"
    );
}

/// **g4b — legacy `run_cycle` ≡ `run_cycle_full(.., Disabled)` ИСПОЛНЕНИЕМ.** Аддитивность:
/// прежний вызов не получает ни одного serving-инцидента, и его исход поэлементно равен
/// исходу полного цикла с `Default` входом на том же состоянии.
#[test]
fn g4b_legacy_run_cycle_equals_full_cycle_with_disabled_input() {
    let thr = Thresholds::default();
    let mut s_legacy = WatchdogState::default();
    let mut s_full = WatchdogState::default();
    let out = ops::watchdog_cycle::run_cycle(&inputs(), 1_000, &thr, 60_000, &mut s_legacy);
    let full = run_cycle_full(
        &inputs(),
        &ServingInputs::default(),
        1_000,
        &thr,
        60_000,
        &mut s_full,
    );
    assert_eq!(fired(&out, Incident::ServingHeartbeatMissing), 0);
    assert_eq!(fired(&out, Incident::ServingSilence), 0);
    assert_eq!(fired(&out, Incident::ServingHeartbeatStale), 0);
    assert_eq!(
        out.fired, full.fired,
        "run_cycle ≠ run_cycle_full(Disabled): fired"
    );
    assert_eq!(
        out.delivered, full.delivered,
        "run_cycle ≠ run_cycle_full(Disabled): delivered"
    );
    assert_eq!(out.suppressed, full.suppressed);
    assert_eq!(
        s_legacy, s_full,
        "run_cycle ≠ run_cycle_full(Disabled): состояние"
    );
}

/// **g4c — парный vantage к g4b:** тот же вход, но `ConfiguredMissing` — тревога ЕСТЬ.
/// Без этой пары «Default молчит» неотличим от «serving-проверки не подключены вовсе».
#[test]
fn g4c_configured_missing_on_the_same_input_fires_missing() {
    let thr = Thresholds::default();
    let mut state = WatchdogState::default();
    let out = run_cycle_full(
        &inputs(),
        &ServingInputs {
            heartbeat: ServingHeartbeat::ConfiguredMissing,
        },
        1_000,
        &thr,
        60_000,
        &mut state,
    );
    assert_eq!(
        fired(&out, Incident::ServingHeartbeatMissing),
        1,
        "ConfiguredMissing не дал MISSING"
    );
    assert_eq!(delivered(&out, Incident::ServingHeartbeatMissing), 1);
}

// ─────────────────────────── g5 — точка входа бинаря ───────────────────────────

fn run_watchdog(serving_path: &std::path::Path, state_dir: &std::path::Path) -> String {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ops-watchdog"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WATCHDOG_SERVING_HEARTBEAT_PATH", serving_path)
        .env(
            "WATCHDOG_HEARTBEAT_PATH",
            state_dir.join("recorder.heartbeat"),
        )
        .env("WATCHDOG_STATE_PATH", state_dir.join("watchdog.state.json"))
        .env("WATCHDOG_CRON_DIR", state_dir)
        .output()
        .expect("запуск ops-watchdog");
    assert!(
        out.status.success(),
        "ops-watchdog завершился с {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn g5_watchdog_binary_reads_serving_heartbeat_from_env_path() {
    let state = tempfile::tempdir().expect("state");
    let serving = state.path().join("gateway-serve.heartbeat");

    // (1) Путь ЗАДАН, файла нет — исполняемый случай `ConfiguredMissing`: код отсутствия.
    // Это единственный прод-вход, и он не имеет права передавать `Disabled`.
    let out = run_watchdog(&serving, state.path());
    assert!(
        out.contains("WD-SERVING-HB-MISSING"),
        "I-6 / TD-220 / C-260 R1: бинарь ops-watchdog при ЗАДАННОМ WATCHDOG_SERVING_HEARTBEAT_PATH \
         и отсутствующем файле не сообщил WD-SERVING-HB-MISSING — либо переменную не читает, либо \
         передаёт в цикл Disabled вместо ConfiguredMissing; молчание выдачи снаружи невидимо. \
         Вывод:\n{out}"
    );

    // (2) Файл есть, но старый — код несвежести (наблюдать ОТСУТСТВИЕ обновлений).
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let stale = serde_json::json!({
        "schema": 1, "ts_wall_ms": now_ms - 3_600_000, "attempts": 5, "successes": 5,
        "refusals_supported": 0, "refusals_unsupported": 0,
        "journal_payload_bytes_read": 10, "slots_in_flight": 0,
        "freshness": {"source_ms": 0, "projection_ms": 0, "published_ms": 0, "snapshot_ms": 0}
    });
    std::fs::write(&serving, stale.to_string()).expect("write stale");
    let out = run_watchdog(&serving, state.path());
    assert!(
        out.contains("WD-SERVING-HB-STALE"),
        "I-6: файл сердцебиения часовой давности не дал кода несвежести. Вывод:\n{out}"
    );
    assert!(
        !out.contains("WD-SERVING-HB-MISSING"),
        "читаемый файл дал MISSING — бинарь не разобрал форму §4.2. Вывод:\n{out}"
    );

    // (3) Свежий здоровый файл — ни отсутствия, ни несвежести.
    let fresh = serde_json::json!({
        "schema": 1, "ts_wall_ms": now_ms, "attempts": 5, "successes": 5,
        "refusals_supported": 0, "refusals_unsupported": 0,
        "journal_payload_bytes_read": 10, "slots_in_flight": 0,
        "freshness": {"source_ms": 0, "projection_ms": 0, "published_ms": 0, "snapshot_ms": 0}
    });
    std::fs::write(&serving, fresh.to_string()).expect("write fresh");
    let out = run_watchdog(&serving, state.path());
    assert!(
        !out.contains("WD-SERVING-HB-MISSING") && !out.contains("WD-SERVING-HB-STALE"),
        "парный vantage: свежий файл дал ложную тревогу. Вывод:\n{out}"
    );
}
