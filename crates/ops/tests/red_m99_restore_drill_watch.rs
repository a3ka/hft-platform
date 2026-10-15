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
//! pub const DEFAULT_RESTORE_DRILL_STATE: &str = "/var/lib/hft/drill-state/journal-restore-drill.json";
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
///
/// **Круг 2 (`R-263` Б-4): путь живёт в ОТДЕЛЬНОМ каталоге.** Recorder — потребитель метрики —
/// работает в контейнере и видит хост только через объявленные тома; прежний путь в корне
/// `/var/lib/hft` в контейнер не монтировался, и метрика на проде была бы 0 и после успеха.
/// Монтируется КАТАЛОГ, а не файл: обёртка пишет `tmp → rename`, а bind-mount файла держит
/// прежний inode и не увидел бы ни одной новой записи. Каталог отдельный, чтобы том не тащил в
/// контейнер ничего, кроме состояния drill'а (каталог восстановления — 2.5 ГБ — туда не входит).
#[test]
fn v3_default_state_path_matches_wrapper_contract() {
    assert_eq!(
        DEFAULT_RESTORE_DRILL_STATE,
        "/var/lib/hft/drill-state/journal-restore-drill.json"
    );
}

/// Каталог файла состояния по СПЕКЕ (`M-99` §4 п.10) — то, что монтируется в контейнер
/// recorder'а (`v5`). Литерал спеки, а не производное от продуктовой константы: иначе судья
/// исключительности и его самопроверка судили бы то, что продукт сейчас объявил (`v3` отдельно
/// пиннит, что продукт объявляет то же).
const SPEC_STATE_DIR: &str = "/var/lib/hft/drill-state";

fn state_dir() -> &'static str {
    SPEC_STATE_DIR
}

/// **`v4` — поля и приоритет — ровно по таблице спеки §5 (`R-263` Н-1…Н-3).** Каждый случай —
/// отдельное отступление реализации круга 1:
/// - `checked` ОБЯЗАТЕЛЕН по T2 (`M-74`): без него — `Unparseable` (круг 1 давал `Ok` —
///   fail-open на неполном состоянии);
/// - нет поля `ok` — `Unparseable`, а не `Failed`;
/// - нет `events_read` при `ok=1` с отпечатком — `Empty`, а не `Unparseable`;
/// - `Empty` стоит в таблице ВЫШЕ `FromFuture`: первое сработавшее условие и есть вердикт;
/// - отпечаток — 64 СТРОЧНЫХ hex: заглавные — `Unparseable` (писатель отпечатка — строчный);
/// - метка `i64::MIN` — не паника и не `Ok` (разность «сейчас − метка» переполнялась бы;
///   в release `overflow-checks` выключены и вердикт стал бы `Ok`).
#[test]
fn v4_fields_and_priority_follow_spec_table() {
    let d = tempfile::tempdir().expect("tmp");
    let fresh = NOW_MS - 60_000;
    let without = |field: &str| -> String {
        let all = [
            ("ok", "1".to_string()),
            ("ts_wall_ms", fresh.to_string()),
            ("checked", "3".to_string()),
            ("events_read", "41213".to_string()),
            ("digest", format!("\"{DIGEST}\"")),
            ("reason", "\"\"".to_string()),
        ];
        let kv: Vec<String> = all
            .iter()
            .filter(|(k, _)| *k != field)
            .map(|(k, v)| format!("\"{k}\":{v}"))
            .collect();
        format!("{{{}}}", kv.join(","))
    };
    // Setup-страж: полная форма того же построителя — `Ok`; иначе случаи ниже судят построитель.
    assert_eq!(
        verdict_of(d.path(), Some(&without("—")), NOW_MS),
        RestoreDrillVerdict::Ok,
        "SETUP НЕ СОСТОЯЛСЯ: полное состояние построителя `without` обязано давать Ok"
    );
    let upper = DIGEST.to_uppercase();
    let cases: Vec<(&str, String, RestoreDrillVerdict)> = vec![
        (
            "нет checked",
            without("checked"),
            RestoreDrillVerdict::Unparseable,
        ),
        ("нет ok", without("ok"), RestoreDrillVerdict::Unparseable),
        (
            "нет ts_wall_ms",
            without("ts_wall_ms"),
            RestoreDrillVerdict::Unparseable,
        ),
        (
            "нет events_read",
            without("events_read"),
            RestoreDrillVerdict::Empty,
        ),
        (
            "заглавный отпечаток",
            body(1, fresh, 41_213, Some(&upper)),
            RestoreDrillVerdict::Unparseable,
        ),
        (
            "пусто И из будущего — первое по таблице: Empty",
            body(1, NOW_MS + 1, 0, Some(DIGEST)),
            RestoreDrillVerdict::Empty,
        ),
        (
            "провал И из будущего — Failed",
            body(0, NOW_MS + 1, 0, None),
            RestoreDrillVerdict::Failed,
        ),
        (
            "метка i64::MIN — просрочено, не паника и не Ok",
            body(1, i64::MIN, 41_213, Some(DIGEST)),
            RestoreDrillVerdict::Stale,
        ),
    ];
    for (name, text, want) in cases {
        let got = verdict_of(d.path(), Some(&text), NOW_MS);
        assert_eq!(
            got, want,
            "M-99 §5 / R-263: случай «{name}» дал {got:?}, ожидалось {want:?}; вход: {text}"
        );
    }
}

/// **`v5` — КОМПОЗИЦИЯ хост ↔ контейнер recorder'а (`R-263` Б-4, `testing.md` «Механизм несущего
/// пути» п.2).** Обёртка пишет файл состояния на ХОСТЕ; recorder читает тот же путь ВНУТРИ
/// контейнера. Совпадение строк пути ничего не доказывает, пока каталог не смонтирован в
/// контейнер ПО ТОМУ ЖЕ ПУТИ. Оракул читает `docker-compose.yml` — то, что поднимает прод.
#[test]
fn v5_state_dir_is_mounted_into_recorder_at_same_path_read_only() {
    let compose_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker-compose.yml");
    let compose = std::fs::read_to_string(&compose_path).expect("docker-compose.yml читается");
    let binds = compose_binds(&compose);
    assert!(
        binds.iter().any(|b| b.service == "recorder"),
        "SETUP НЕ СОСТОЯЛСЯ: сервис recorder в docker-compose.yml не найден разбором — \
         оракул судил бы пустоту"
    );
    let dir = state_dir();
    assert!(
        dir != "/var/lib/hft" && dir != RESTORE_DIR,
        "каталог состояния обязан быть ОТДЕЛЬНЫМ: монтирование {dir} протащит в контейнер лишнее"
    );
    if let Err(why) = judge_state_mounts(&binds, dir) {
        panic!(
            "R-263 Б-4 / C-311 Б-3 / OPS-I-10: {why}. Требуется РОВНО одно монтирование «{dir}:{dir}:ro» — \
             в recorder, и никакого другого монтирования каталога состояния, его предков или \
             каталога восстановления ни в одном сервисе"
        );
    }
}

/// Каталог восстановления drill'а (`M-99` §4 п.10): 2.5 ГБ восстановленного журнала, в
/// контейнеры не монтируется.
const RESTORE_DIR: &str = "/var/lib/hft/restore-drill";

/// Bind-монтирование сервиса compose: источник на хосте, цель в контейнере, режим.
#[derive(Debug, Clone, PartialEq)]
struct Bind {
    service: String,
    source: String,
    target: String,
    mode: String,
}

/// Все bind-монтирования ВСЕХ сервисов (`C-311` Б-3: круг 2 смотрел один блок и судил
/// существование, а не исключительность). Разбирает короткую форму `- "src:dst[:mode]"`;
/// `${VAR:-default}` в источнике раскрывается в `default` — так его видит прод без `.env`.
/// Длинная форма (`source:` / `target:`) в нашем compose не встречается; если появится, её
/// источник учитывается с целью и режимом `?` — судья такую форму не примет как допустимую,
/// но и не пропустит монтирование, видящее состояние (fail-closed, а не слепое пятно).
fn compose_binds(compose: &str) -> Vec<Bind> {
    let mut out = Vec::new();
    let mut top = String::new();
    let mut service = String::new();
    for line in compose.lines() {
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') {
            top = line.trim_end().to_string();
            service.clear();
            continue;
        }
        if top != "services:" {
            continue;
        }
        if line.starts_with("  ") && !line.starts_with("   ") && line.trim_end().ends_with(':') {
            service = line.trim().trim_end_matches(':').to_string();
            continue;
        }
        let item = line.trim();
        if let Some(src) = item.trim_start_matches("- ").strip_prefix("source:") {
            out.push(Bind {
                service: service.clone(),
                source: src.trim().trim_matches('"').to_string(),
                target: "?".into(),
                mode: "?".into(),
            });
            continue;
        }
        let Some(v) = item.strip_prefix("- ") else {
            continue;
        };
        let v = v.trim().trim_matches('"').trim_matches('\'');
        let (source, rest) = if let Some(inner) = v.strip_prefix("${") {
            let Some(close) = inner.find('}') else {
                continue;
            };
            let var = &inner[..close];
            let default = var.split_once(":-").map(|(_, d)| d).unwrap_or("");
            (default.to_string(), &inner[close + 1..])
        } else {
            match v.find(':') {
                Some(i) => (v[..i].to_string(), &v[i..]),
                None => continue,
            }
        };
        if !(source.starts_with('/') || source.starts_with('.')) {
            continue;
        }
        let Some(rest) = rest.strip_prefix(':') else {
            continue;
        };
        let (target, mode) = match rest.split_once(':') {
            Some((t, m)) => (t.to_string(), m.to_string()),
            None => (rest.to_string(), String::new()),
        };
        out.push(Bind {
            service: service.clone(),
            source,
            target,
            mode,
        });
    }
    out
}

/// `a` — тот же путь, что `b`, или его предок.
fn covers(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('/');
    a.is_empty() || b == a || b.starts_with(&format!("{a}/"))
}

/// Судья исключительности (`M-99` §7 строка о монтировании): ровно одно монтирование, видящее
/// каталог состояния (сам каталог, его предок или потомок), — `recorder`, тот же путь, `:ro`;
/// ни одного монтирования, видящего каталог восстановления; в `recorder` — никакого иного
/// источника из `/var/lib/hft`. Соседние каталоги `/var/lib/hft/*` других сервисов (сегодня —
/// `retention-work` у `retention`, `M-92`) не судятся: они не видят ни состояния, ни восстановления.
fn judge_state_mounts(binds: &[Bind], dir: &str) -> Result<(), String> {
    let sees_state: Vec<&Bind> = binds
        .iter()
        .filter(|b| covers(&b.source, dir) || covers(dir, &b.source))
        .collect();
    let want = Bind {
        service: "recorder".into(),
        source: dir.into(),
        target: dir.into(),
        mode: "ro".into(),
    };
    if sees_state.len() != 1 || *sees_state[0] != want {
        return Err(format!(
            "каталог состояния видят монтирования {sees_state:?} — ожидалось ровно одно {want:?}"
        ));
    }
    if let Some(b) = binds
        .iter()
        .find(|b| covers(&b.source, RESTORE_DIR) || covers(RESTORE_DIR, &b.source))
    {
        return Err(format!("каталог восстановления виден в контейнере: {b:?}"));
    }
    if let Some(b) = binds
        .iter()
        .find(|b| b.service == "recorder" && covers("/var/lib/hft", &b.source) && b.source != dir)
    {
        return Err(format!("recorder монтирует из /var/lib/hft лишнее: {b:?}"));
    }
    Ok(())
}

/// САМОПРОВЕРКА v5 (`C-311` Б-3, условие 5): судья исключительности принимает допустимую форму
/// и отвергает каждого мутанта. Зелёный с первого прогона ПО НАЗНАЧЕНИЮ — это проверка оракула,
/// а не продукта (как `D`/`R` пробы). Без неё «v5 зелёный» не отличим от «v5 принимает всё».
#[test]
fn v5m_state_mount_judge_rejects_every_mutant() {
    let dir = state_dir();
    let good_line = format!("      - {dir}:{dir}:ro");
    let base = |extra_rec: &str, extra_gw: &str, state_line: &str| {
        format!(
            "services:\n  recorder:\n    image: hft-platform-recorder:local\n    container_name: hft-recorder\n    volumes:\n      - journal-data:/data\n{state_line}\n{extra_rec}\n  retention:\n    volumes:\n      - \"${{RETENTION_WORK_DIR:-/var/lib/hft/retention-work}}:/work\"\n  gateway-serve:\n    volumes:\n      - gateway-state:/state\n{extra_gw}\nvolumes:\n  journal-data:\n  gateway-state:\n"
        )
    };
    let ok = base("", "", &good_line);
    let binds = compose_binds(&ok);
    assert_eq!(
        binds.iter().filter(|b| b.source.starts_with('/')).count(),
        2,
        "SETUP: разбор допустимой формы не увидел двух хост-путей (состояние и retention-work): {binds:?}"
    );
    assert_eq!(
        judge_state_mounts(&binds, dir),
        Ok(()),
        "SETUP: судья отверг ДОПУСТИМУЮ форму — пойманные мутанты ничего бы не значили"
    );
    let mutants: Vec<(&str, String)> = vec![
        (
            "broad /var/lib/hft в recorder",
            base("      - /var/lib/hft:/var/lib/hft:ro", "", &good_line),
        ),
        (
            "второй сервис монтирует состояние",
            base("", &good_line, &good_line),
        ),
        (
            "на запись",
            base("", "", &format!("      - {dir}:{dir}:rw")),
        ),
        ("без режима", base("", "", &format!("      - {dir}:{dir}"))),
        (
            "другой путь в контейнере",
            base("", "", &format!("      - {dir}:/state:ro")),
        ),
        ("не в recorder", base("", &good_line, "")),
        ("нет монтирования", base("", "", "")),
        (
            "предок /var/lib",
            base("      - /var/lib:/host-lib:ro", "", &good_line),
        ),
        (
            "сам файл, а не каталог",
            base(
                "",
                "",
                &format!(
                    "      - {dir}/journal-restore-drill.json:{dir}/journal-restore-drill.json:ro"
                ),
            ),
        ),
        (
            "каталог восстановления",
            base(
                &format!("      - {RESTORE_DIR}:{RESTORE_DIR}:ro"),
                "",
                &good_line,
            ),
        ),
        (
            "лишний источник из /var/lib/hft в recorder",
            base("      - /var/lib/hft/other:/other:ro", "", &good_line),
        ),
        (
            "длинная форма source:",
            base(
                "      - type: bind\n        source: /var/lib/hft\n        target: /hft",
                "",
                &good_line,
            ),
        ),
        (
            "кавычки и переменная не прячут broad",
            base(
                "      - \"${HFT_DIR:-/var/lib/hft}:/hft:ro\"",
                "",
                &good_line,
            ),
        ),
    ];
    let accepted: Vec<&str> = mutants
        .iter()
        .filter(|(_, text)| judge_state_mounts(&compose_binds(text), dir).is_ok())
        .map(|(name, _)| *name)
        .collect();
    assert!(
        accepted.is_empty(),
        "судья v5 ПРИНЯЛ мутантов: {accepted:?}"
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
