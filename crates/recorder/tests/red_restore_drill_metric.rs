//! SACRED (architect-only) — `M-74` задачи 3 и 5: **`backup_restore_drill_ok` реально
//! эмитится в РЕНДЕРЕ `/metrics`, и просроченность есть ОТКАЗ.**
//!
//! ## СОСТОЯНИЕ: COMPILE-RED, И ЭТО ЗАЯВЛЕНО, А НЕ СЛУЧИЛОСЬ
//!
//! Оракул написан ПРОТИВ СИГНАТУРЫ, объявленной спекой `M-74` дословно
//! (§«Сигнатура продюсера»), и до внесения этой сигнатуры engine-dev'ом он НЕ СОБИРАЕТСЯ.
//! Это санкционированное состояние: `A-028` §1 — «послабление касается только
//! КОМПИЛИРУЕМОСТИ оракула, никогда его СУЩЕСТВОВАНИЯ». Прецедент формы — `M-72` задача 2
//! (`2a701eb`: 155 строк оракула закоммичены, задача dev'а ⏳ OPEN).
//!
//! Гейт `scripts/verify_M-74.sh` различает ТРИ исхода на этих шагах — вакуум (фильтр не нашёл
//! ни одного теста) / COMPILE-RED / исполнено-и-упало, — потому что `cargo test` возвращает 0
//! при НУЛЕ исполненных тестов, и зелёное здесь означало бы пустоту, а не закрытую задачу.
//!
//! ## ЧТО ИМЕННО ПИННИТСЯ И ПОЧЕМУ ИМЕННО ЭТО
//!
//! `OPS-I-10` («объявлена ⟹ эмитится») уже стоил нам `TD-027`: реестр имён был полон,
//! 13 из 15 метрик не эмитились, а правила P0/P1 ссылались на МЁРТВЫЕ метрики. Сегодня
//! `backup_restore_drill_ok` — ровно такая: объявлена в `crates/ops/src/metrics.rs:109-113`,
//! правило `OPS-BKP` на неё ссылается (`crates/ops/src/alerts.rs:117`), продюсера нет
//! (`crates/recorder/src/metric_emit.rs:22` — «deferred»).
//!
//! Поэтому ассерты идут по SAMPLE-СЕРИИ рендера (`name value`), а НЕ по `# HELP`/`# TYPE`:
//! registry-only и есть тот класс, который `OPS-I-10` запрещает. Метрика безлейблова
//! (`labels: &[]`, `metrics.rs:112`), значит серия рендерится как `backup_restore_drill_ok N`.
//!
//! ## ПОЧЕМУ ВРЕМЯ ПРИХОДИТ ПАРАМЕТРОМ
//!
//! `now_wall_ms` — аргумент, а не `SystemTime::now()` внутри: иначе оракул просроченности
//! недетерминирован и его пришлось бы «ждать 40 суток». Тот же приём уже применён в
//! `journal-retention` (`--now-wall-ms`, «часы снаружи — детерминизм, ТОЛЬКО для тестов»).
//!
//! ## ЧЕГО ЭТОТ ОРАКУЛ НЕ ЛОВИТ — названо
//!
//! Он не доказывает, что sampler recorder'а ЗОВЁТ `sample_restore_drill` раз в секунду:
//! это композиция, и её проверяет отдельный шаг гейта (канарейка вызова в `metric_emit.rs`
//! + `red_metrics_emission.rs`-класс). Здесь пиннится ОТОБРАЖЕНИЕ «файл состояния → gauge».

use std::path::Path;

use ops::metrics::Metrics;
use recorder::metric_emit::{sample_restore_drill, RESTORE_DRILL_FRESH_WINDOW_MS};

/// Произвольный «сейчас» — ровно замер heartbeat'а прода 2026-08-31T09:52:48Z.
/// Конкретное число не важно, важно что оно ОДНО на все сценарии.
const NOW_MS: i64 = 1_788_169_968_723;

/// Значение SAMPLE-серии безлейбловой метрики. `None` — серии НЕТ вовсе
/// (registry-only: есть `# HELP`/`# TYPE`, нет строки со значением).
fn sample(text: &str, name: &str) -> Option<i64> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix(name)?.strip_prefix(' ')?.trim().parse().ok())
}

/// Записать файл состояния drill'а и вернуть путь.
fn write_state(dir: &Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("journal-restore-drill.json");
    std::fs::write(&p, body).expect("write state");
    p
}

/// Тело состояния по T2-контракту спеки `M-74`.
fn state(ok: u8, ts_wall_ms: i64, events_read: u64) -> String {
    format!(
        r#"{{"ok":{ok},"ts_wall_ms":{ts_wall_ms},"ts":"2026-08-31T09:52:48Z","checked":3,"events_read":{events_read},"reason":""}}"#
    )
}

fn emit(state_path: &Path) -> Option<i64> {
    let m = Metrics::new();
    sample_restore_drill(&m, state_path, NOW_MS);
    sample(&m.prometheus_text(), "backup_restore_drill_ok")
}

// ── Отсутствие drill'а — это ОТКАЗ, а не «нет данных» ────────────────────────────────────

/// **Главный fail-closed случай.** Файла нет — drill не проводился НИ РАЗУ (наше состояние
/// на 2026-08-31, `TD-020` OPEN). Серия обязана СУЩЕСТВОВАТЬ и быть нулём: правило `OPS-BKP`
/// стреляет на `== 0`, и отсутствие серии его НЕ взведёт — молчание стало бы «всё хорошо».
#[test]
fn missing_state_file_emits_explicit_zero_not_absence() {
    let d = tempfile::tempdir().expect("tempdir");
    let path = d.path().join("нет-такого-файла.json");
    assert!(!path.exists(), "предусловие: файла действительно нет");

    assert_eq!(
        emit(&path),
        Some(0),
        "нет файла состояния ⇒ серия обязана присутствовать со значением 0. `None` здесь \
         означало бы registry-only (класс TD-027): правило OPS-BKP ждёт метрику, которой нет"
    );
}

/// Позитивный контроль. Без него все остальные сценарии зелены и у продюсера, который
/// ВСЕГДА ставит 0: «на битом состоянии ноль» верно и для метрики, не работающей никогда.
#[test]
fn fresh_success_emits_one() {
    let d = tempfile::tempdir().expect("tempdir");
    let p = write_state(d.path(), &state(1, NOW_MS - 60_000, 41_213));
    assert_eq!(
        emit(&p),
        Some(1),
        "свежий успешный drill ⇒ 1; иначе продюсер не различает успех и провал"
    );
}

#[test]
fn explicit_failure_emits_zero() {
    let d = tempfile::tempdir().expect("tempdir");
    let p = write_state(d.path(), &state(0, NOW_MS - 60_000, 0));
    assert_eq!(emit(&p), Some(0), "ok=0 ⇒ 0");
}

// ── ПРОСРОЧЕННОСТЬ (задача 5). Фильтр гейта — `stale`; имена обоих тестов его несут. ─────

/// Молчание не есть успех: последний УСПЕШНЫЙ drill старше окна ⇒ метрика падает в 0.
/// Без этого достаточно один раз пройти drill, и метрика будет зелёной вечно, даже если
/// расписание сломалось на следующий день — ровно класс `OPS-SILENCE` («жив, но не работает»).
#[test]
fn stale_success_beyond_window_emits_zero() {
    let d = tempfile::tempdir().expect("tempdir");
    // На час СТАРШЕ окна — не «ровно на границе»: фикстура, стоящая на границе, падает
    // от округления по неверной причине (`testing.md` §«Границы»).
    let ts = NOW_MS - RESTORE_DRILL_FRESH_WINDOW_MS - 3_600_000;
    let p = write_state(d.path(), &state(1, ts, 41_213));
    assert_eq!(emit(&p), Some(0), "успешный drill старше окна свежести ⇒ 0");
}

/// Обратная сторона границы. Без неё «просроченность ⇒ 0» проходит и у продюсера, который
/// всегда возвращает 0 — окно оказалось бы не запиннено ни с одной стороны, и любое его
/// значение (час, век) удовлетворяло бы набору.
#[test]
fn stale_check_does_not_fire_inside_window() {
    let d = tempfile::tempdir().expect("tempdir");
    let ts = NOW_MS - RESTORE_DRILL_FRESH_WINDOW_MS + 3_600_000;
    let p = write_state(d.path(), &state(1, ts, 41_213));
    assert_eq!(
        emit(&p),
        Some(1),
        "внутри окна свежести успешный drill остаётся 1 — иначе метрика краснеет на исправном \
         расписании, и её выключат"
    );
}

/// Окно объявлено спекой числом (40 суток) и живёт В ОДНОМ месте — в константе продюсера.
/// Проверка здесь не тавтология: она запрещает «уточнить» окно молча, не тронув спеку.
#[test]
fn stale_window_is_forty_days() {
    assert_eq!(
        RESTORE_DRILL_FRESH_WINDOW_MS,
        40 * 24 * 60 * 60 * 1000,
        "окно свежести объявлено спекой M-74 как 40 суток (месячное расписание + запас на \
         один пропущенный прогон); менять его — правка спеки, а не константы"
    );
}

// ── Деградированный вход: частичная запись, будущее, пустое чтение ───────────────────────

/// Партнёр атомарности из shell-пробы: оборванная запись НЕ ЧИТАЕТСЯ как успех.
/// Обёртка пишет `tmp` → `rename`, но крах между ними физически возможен, и потребитель
/// обязан быть fail-closed сам по себе, а не полагаться на дисциплину писателя.
#[test]
fn truncated_state_emits_zero() {
    let d = tempfile::tempdir().expect("tempdir");
    let p = write_state(d.path(), r#"{"ok":1,"ts_wall_ms":1788169"#);
    assert_eq!(
        emit(&p),
        Some(0),
        "неразобранный JSON ⇒ 0 (неизвестный вход → reject, класс RK-I-3)"
    );
}

/// Метка из будущего — признак сбитых часов или подложенного файла, а не свежести.
/// Fail-closed: «слишком свежо» тоже отказ.
#[test]
fn future_timestamp_emits_zero() {
    let d = tempfile::tempdir().expect("tempdir");
    let p = write_state(d.path(), &state(1, NOW_MS + 7 * 24 * 3_600_000, 41_213));
    assert_eq!(
        emit(&p),
        Some(0),
        "ts в будущем ⇒ 0: часы сбиты либо файл подложен, и то и другое не есть успешный drill"
    );
}

/// `R-157` `Б-5` на уровне метрики: обёртка уже умела рапортовать успех, ничего не сделав.
/// `ok=1` при НУЛЕ прочитанных событий — ровно этот случай, и продюсер обязан его отвергнуть.
#[test]
fn success_with_zero_events_read_emits_zero() {
    let d = tempfile::tempdir().expect("tempdir");
    let p = write_state(d.path(), &state(1, NOW_MS - 60_000, 0));
    assert_eq!(
        emit(&p),
        Some(0),
        "«успех» без единого прочитанного события успехом не является — копия не доказана \
         читаемой, доказано лишь, что процедура завершилась"
    );
}

/// Продюсер не смеет ронять sampler: файл нечитаем по правам — это отказ, а не паника.
/// Sampler крутится в recorder'е раз в секунду; паника здесь останавливает СБОР ДАННЫХ.
#[test]
fn unreadable_state_emits_zero_without_panic() {
    let d = tempfile::tempdir().expect("tempdir");
    // Каталог вместо файла — гарантированно нечитаемый как файл на любой ОС,
    // не требует прав root и не зависит от umask.
    let p = d.path().join("state-as-dir.json");
    std::fs::create_dir(&p).expect("mkdir");
    assert_eq!(
        emit(&p),
        Some(0),
        "нечитаемое состояние ⇒ 0 и НИ В КОЕМ СЛУЧАЕ не паника: sampler живёт внутри \
         recorder'а, единственного писателя журнала"
    );
}
