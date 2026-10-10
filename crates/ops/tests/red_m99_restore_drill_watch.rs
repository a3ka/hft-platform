//! RED `M-99` (sacred, architect-only) — **сторож `ops-watchdog` видит провал, просрочку и
//! ОТСУТСТВИЕ учебного восстановления** (`OPS-I-3`, `OPS-I-10`, `PL-I-8`; спека
//! `milestones/M-99-restore-drill-automation.md` §5).
//!
//! # Почему не хватает метрики `backup_restore_drill_ok`
//!
//! `M-74` вёл наблюдаемость drill'а до gauge в `/metrics` recorder'а и правила `OPS-BKP`
//! (`crates/ops/src/alerts.rs`) — то есть до Prometheus. Prometheus/Alertmanager на проде НЕТ
//! (`docs/ROADMAP.md` строка 3, аудит 2026-10-01); тревогу наружу поднимает ТОЛЬКО `ops-watchdog`
//! (cron на хосте, `M-93`). Gauge без потребителя — тот же «built-not-wired», что и правило без
//! продюсера. Поэтому сторож читает ФАЙЛ СОСТОЯНИЯ drill'а сам — ТЕМ ЖЕ отображением, что и
//! recorder (`ops::restore_drill::restore_drill_verdict`, одна функция на обоих потребителей).
//!
//! # Объявленная форма (спека §5 — дословно, против неё и написан оракул)
//!
//! ```ignore
//! // crates/ops/src/restore_drill.rs
//! pub const RESTORE_DRILL_FRESH_WINDOW_MS: i64 = 40 * 24 * 60 * 60 * 1000;
//! pub const DEFAULT_RESTORE_DRILL_STATE: &str = "/var/lib/hft/journal-restore-drill.json";
//! pub enum RestoreDrillVerdict { Ok, Missing, Unparseable, Failed, Empty, Stale, FromFuture }
//! impl RestoreDrillVerdict { pub fn is_ok(&self) -> bool }
//! pub fn restore_drill_verdict(state_path: &Path, now_wall_ms: i64) -> RestoreDrillVerdict;
//! // crates/ops/src/watchdog.rs
//! Incident::RestoreDrill            // код "WD-RESTORE-DRILL", уровень Critical
//! pub fn check_restore_drill(v: &RestoreDrillVerdict) -> Option<Alert>;
//! // crates/ops/src/watchdog_cycle.rs
//! pub fn run_cycle_with_drill(inputs: &CycleInputs, serving: &ServingInputs,
//!     drill: Option<&RestoreDrillVerdict>, now_ms: i64, thr: &Thresholds,
//!     dedup_window_ms: i64, state: &mut WatchdogState) -> CycleOutcome;
//! // run_cycle_full(..) ≡ run_cycle_with_drill(.., None, ..) — аддитивно
//! // бинарь: env WATCHDOG_RESTORE_DRILL_STATE (дефолт DEFAULT_RESTORE_DRILL_STATE)
//! ```
//!
//! # Состояние
//!
//! COMPILE-RED: модуля `ops::restore_drill`, варианта `Incident::RestoreDrill` и
//! `run_cycle_with_drill` нет — их вносит engine-dev задачей 6. Санкционировано так же, как
//! COMPILE-RED задачи 3 `M-74` (`A-028` §1: послабление касается КОМПИЛИРУЕМОСТИ оракула, не
//! его существования).

use std::path::Path;

use ops::restore_drill::{
    restore_drill_verdict, RestoreDrillVerdict, DEFAULT_RESTORE_DRILL_STATE,
    RESTORE_DRILL_FRESH_WINDOW_MS,
};
use ops::state::WatchdogState;
use ops::watchdog::{check_restore_drill, Incident, Level, Thresholds};
use ops::watchdog_cycle::{
    run_cycle_full, run_cycle_with_drill, CycleInputs, CycleOutcome, ServingInputs,
};

/// Тот же «сейчас», что у оракула продюсера (`red_restore_drill_metric.rs`).
const NOW_MS: i64 = 1_788_169_968_723;
const DIGEST: &str = "96a231ce97316518856045415a7fdf089ce5106f427c239cdc07fb3dbbdb9c81";

fn body(ok: u8, ts_wall_ms: i64, events_read: u64, digest: Option<&str>) -> String {
    let d = digest
        .map(|d| format!(r#","digest":"{d}""#))
        .unwrap_or_default();
    format!(
        r#"{{"ok":{ok},"ts_wall_ms":{ts_wall_ms},"checked":3,"events_read":{events_read}{d},"reason":""}}"#
    )
}

fn verdict_of(dir: &Path, text: Option<&str>, now: i64) -> RestoreDrillVerdict {
    let p = dir.join("journal-restore-drill.json");
    if let Some(t) = text {
        std::fs::write(&p, t).expect("write state");
    }
    restore_drill_verdict(&p, now)
}

/// **`v1` — отображение «файл состояния → вердикт», fail-closed по каждому классу входа.**
/// Классы — те же, что пиннит оракул продюсера (`M-74` §«Сигнатура продюсера»), плюс
/// обязательность `digest` при `ok=1` (T2-контракт `M-74`: «обязательно при `ok=1`»). Каждый
/// класс отдельным вердиктом: оператор лечит «просрочено» и «копия не читается» по-разному.
#[test]
fn v1_verdict_mapping_is_fail_closed_per_class() {
    let d = tempfile::tempdir().expect("tmp");
    let fresh = NOW_MS - 60_000;
    let cases: Vec<(&str, Option<String>, RestoreDrillVerdict)> = vec![
        ("файла нет", None, RestoreDrillVerdict::Missing),
        (
            "свежий успех с отпечатком",
            Some(body(1, fresh, 41_213, Some(DIGEST))),
            RestoreDrillVerdict::Ok,
        ),
        (
            "ok=1 БЕЗ отпечатка — состояние не доказывает чтения",
            Some(body(1, fresh, 41_213, None)),
            RestoreDrillVerdict::Unparseable,
        ),
        (
            "ok=1, отпечаток не 64 hex",
            Some(body(1, fresh, 41_213, Some("abc"))),
            RestoreDrillVerdict::Unparseable,
        ),
        (
            "явный провал",
            Some(body(0, fresh, 0, None)),
            RestoreDrillVerdict::Failed,
        ),
        (
            "успех, ничего не прочитавший",
            Some(body(1, fresh, 0, Some(DIGEST))),
            RestoreDrillVerdict::Empty,
        ),
        (
            "оборванная запись",
            Some(r#"{"ok":1,"ts_wall_ms":1788169"#.to_string()),
            RestoreDrillVerdict::Unparseable,
        ),
        (
            "метка из будущего",
            Some(body(1, NOW_MS + 1, 41_213, Some(DIGEST))),
            RestoreDrillVerdict::FromFuture,
        ),
    ];
    for (name, text, want) in cases {
        let _ = std::fs::remove_file(d.path().join("journal-restore-drill.json"));
        let got = verdict_of(d.path(), text.as_deref(), NOW_MS);
        assert_eq!(
            got, want,
            "M-99 / OPS-I-3: класс входа «{name}» дал {got:?}, ожидалось {want:?}"
        );
        assert_eq!(
            got.is_ok(),
            want == RestoreDrillVerdict::Ok,
            "is_ok() расходится с вердиктом на «{name}»"
        );
    }
    // Каталог на месте файла — тоже «нет состояния», без паники.
    let dd = tempfile::tempdir().expect("tmp");
    std::fs::create_dir(dd.path().join("journal-restore-drill.json")).expect("dir");
    assert_eq!(
        verdict_of(dd.path(), None, NOW_MS),
        RestoreDrillVerdict::Missing
    );
}

/// **`v2` — окно свежести с ОБЕИХ сторон границы** (`M-74` §«Окно свежести — 40 суток»).
/// `RESTORE_DRILL_FRESH_WINDOW_MS` — единственное место числа; recorder его реэкспортирует.
#[test]
fn v2_freshness_window_is_forty_days_both_sides() {
    assert_eq!(RESTORE_DRILL_FRESH_WINDOW_MS, 40 * 24 * 60 * 60 * 1000);
    let d = tempfile::tempdir().expect("tmp");
    let at_edge = NOW_MS - RESTORE_DRILL_FRESH_WINDOW_MS;
    assert_eq!(
        verdict_of(
            d.path(),
            Some(&body(1, at_edge, 41_213, Some(DIGEST))),
            NOW_MS
        ),
        RestoreDrillVerdict::Ok,
        "ровно на границе окна drill ещё свеж"
    );
    assert_eq!(
        verdict_of(
            d.path(),
            Some(&body(1, at_edge - 1, 41_213, Some(DIGEST))),
            NOW_MS
        ),
        RestoreDrillVerdict::Stale,
        "на 1 мс за границей окна drill просрочен — молчание не считается успехом"
    );
}

/// **`v3` — путь состояния у сторожа и у обёртки ОДИН** (композиция, `testing.md`: путь записи
/// продюсера = путь чтения потребителя). Обёртка пишет в `JOURNAL_DRILL_STATE` с этим дефолтом
/// (`M-74` §«Контракт обёртки»); гейт `verify_M-99.sh` сверяет дефолт обёртки с этой константой.
#[test]
fn v3_default_state_path_matches_wrapper_contract() {
    assert_eq!(
        DEFAULT_RESTORE_DRILL_STATE,
        "/var/lib/hft/journal-restore-drill.json"
    );
}

fn inputs() -> CycleInputs {
    CycleInputs {
        heartbeat: None,
        containers: vec![],
        cron_jobs: vec![],
    }
}

fn fired(out: &CycleOutcome, incident: Incident) -> Vec<Level> {
    out.fired
        .iter()
        .filter(|a| a.incident == incident)
        .map(|a| a.level)
        .collect()
}

fn delivered(out: &CycleOutcome, incident: Incident) -> usize {
    out.delivered
        .iter()
        .filter(|a| a.incident == incident)
        .count()
}

/// **`w1` — каждый не-`Ok` вердикт поднимает `WD-RESTORE-DRILL` уровня Critical, `Ok` — молчит.**
/// Сторож обязан видеть и ОТСУТСТВИЕ: `Missing` — тревога, а не «нечего судить»
/// (`testing.md` «Целостность гейта» св. 4).
#[test]
fn w1_each_bad_verdict_fires_critical_and_ok_is_silent() {
    let thr = Thresholds::default();
    for v in [
        RestoreDrillVerdict::Missing,
        RestoreDrillVerdict::Unparseable,
        RestoreDrillVerdict::Failed,
        RestoreDrillVerdict::Empty,
        RestoreDrillVerdict::Stale,
        RestoreDrillVerdict::FromFuture,
    ] {
        let a = check_restore_drill(&v).unwrap_or_else(|| {
            panic!("M-99: вердикт {v:?} не поднял тревогу — сторож слеп к этому классу")
        });
        assert_eq!(a.incident, Incident::RestoreDrill);
        assert_eq!(
            a.level,
            Level::Critical,
            "{v:?}: уровень обязан быть Critical"
        );
        assert_eq!(a.incident.code(), "WD-RESTORE-DRILL");

        let mut st = WatchdogState::default();
        let out = run_cycle_with_drill(
            &inputs(),
            &ServingInputs::default(),
            Some(&v),
            NOW_MS,
            &thr,
            60_000,
            &mut st,
        );
        assert_eq!(
            fired(&out, Incident::RestoreDrill),
            vec![Level::Critical],
            "M-99: цикл сторожа не поднял WD-RESTORE-DRILL на {v:?}"
        );
    }
    assert!(check_restore_drill(&RestoreDrillVerdict::Ok).is_none());
    let mut st = WatchdogState::default();
    let out = run_cycle_with_drill(
        &inputs(),
        &ServingInputs::default(),
        Some(&RestoreDrillVerdict::Ok),
        NOW_MS,
        &thr,
        60_000,
        &mut st,
    );
    assert!(fired(&out, Incident::RestoreDrill).is_empty());
}

/// **`w2` — дедупликация та же, что у остальных условий:** вторая тревога в окне подавлена,
/// здоровый такт снимает подавление, следующая тревога доставляется сразу.
#[test]
fn w2_restore_drill_alert_is_deduped_and_cleared_by_healthy_tick() {
    let thr = Thresholds::default();
    let dedup = 30 * 60_000;
    let mut st = WatchdogState::default();
    let tick = |st: &mut WatchdogState, v: &RestoreDrillVerdict, t: i64| {
        let o = run_cycle_with_drill(
            &inputs(),
            &ServingInputs::default(),
            Some(v),
            t,
            &thr,
            dedup,
            st,
        );
        let s = serde_json::to_string(&*st).expect("state → json");
        *st = serde_json::from_str(&s).expect("json → state");
        o
    };
    let o1 = tick(&mut st, &RestoreDrillVerdict::Missing, NOW_MS);
    assert_eq!(
        delivered(&o1, Incident::RestoreDrill),
        1,
        "первая тревога доставлена"
    );
    let o2 = tick(&mut st, &RestoreDrillVerdict::Missing, NOW_MS + 60_000);
    assert_eq!(
        delivered(&o2, Incident::RestoreDrill),
        0,
        "повтор в окне подавлен"
    );
    let o3 = tick(&mut st, &RestoreDrillVerdict::Ok, NOW_MS + 120_000);
    assert_eq!(delivered(&o3, Incident::RestoreDrill), 0);
    let o4 = tick(&mut st, &RestoreDrillVerdict::Stale, NOW_MS + 180_000);
    assert_eq!(
        delivered(&o4, Incident::RestoreDrill),
        1,
        "после здорового такта новая тревога доставляется сразу"
    );
}

/// **`w3` — аддитивность:** `run_cycle_full` ≡ `run_cycle_with_drill(.., None, ..)` (ни одной
/// тревоги drill'а, остальные исходы равны). Без этого прежние вызыватели цикла получили бы
/// тревогу, которой не заказывали.
#[test]
fn w3_run_cycle_full_equals_cycle_without_drill_input() {
    let thr = Thresholds::default();
    let mut a = WatchdogState::default();
    let mut b = WatchdogState::default();
    let oa = run_cycle_full(
        &inputs(),
        &ServingInputs::default(),
        NOW_MS,
        &thr,
        60_000,
        &mut a,
    );
    let ob = run_cycle_with_drill(
        &inputs(),
        &ServingInputs::default(),
        None,
        NOW_MS,
        &thr,
        60_000,
        &mut b,
    );
    let codes = |o: &CycleOutcome| {
        let mut v: Vec<&str> = o.fired.iter().map(|a| a.incident.code()).collect();
        v.sort();
        v
    };
    assert_eq!(codes(&oa), codes(&ob));
    assert!(fired(&ob, Incident::RestoreDrill).is_empty());
}

fn run_watchdog(drill_state: &Path, dir: &Path) -> String {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ops-watchdog"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WATCHDOG_RESTORE_DRILL_STATE", drill_state)
        .env(
            "WATCHDOG_SERVING_HEARTBEAT_PATH",
            dir.join("gateway-serve.heartbeat"),
        )
        .env("WATCHDOG_HEARTBEAT_PATH", dir.join("recorder.heartbeat"))
        .env("WATCHDOG_STATE_PATH", dir.join("watchdog.state.json"))
        .env("WATCHDOG_CRON_DIR", dir)
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

/// **`b1` — точка входа бинаря:** сторож, запущенный прод-формой (env, как в
/// `deploy/cron.d/watchdog`), читает файл состояния drill'а по `WATCHDOG_RESTORE_DRILL_STATE`
/// и сообщает `WD-RESTORE-DRILL`, когда файла нет; на свежем успехе — не сообщает. Часы бинаря —
/// настоящие, поэтому «свежий» здесь — `ts_wall_ms` текущего момента.
#[test]
fn b1_binary_reads_drill_state_from_env_and_reports_absence() {
    let dir = tempfile::tempdir().expect("tmp");
    let state = dir.path().join("journal-restore-drill.json");
    let out = run_watchdog(&state, dir.path());
    assert!(
        out.contains("WD-RESTORE-DRILL"),
        "M-99 / OPS-I-3: файла состояния drill'а НЕТ, а бинарь сторожа не сообщил \
         WD-RESTORE-DRILL — либо переменную не читает, либо отсутствие не судит. Вывод:\n{out}"
    );

    let dir2 = tempfile::tempdir().expect("tmp");
    let state2 = dir2.path().join("journal-restore-drill.json");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;
    std::fs::write(&state2, body(1, now - 60_000, 41_213, Some(DIGEST))).expect("write");
    let out2 = run_watchdog(&state2, dir2.path());
    assert!(
        !out2.contains("WD-RESTORE-DRILL"),
        "M-99: свежий успешный drill, а сторож сообщил WD-RESTORE-DRILL — тревога, звенящая на \
         норме, так же бесполезна, как молчащая. Вывод:\n{out2}"
    );
}
