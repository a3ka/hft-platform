//! M-99 (задача 3/5/6) — единое отображение «файл состояния drill'а → класс или другая
//! сущность», на нём стоят оба потребителя метрики/тревоги: продюсер gauge
//! (`recorder::metric_emit::sample_restore_drill`, `M-99` §5) и сторож `ops-watchdog`
//! (`check_restore_drill` + `Incident::RestoreDrill`/`WD-RESTORE-DRILL`). Спека
//! `milestones/M-99-restore-drill-automation.md` §5 объявляет типы и таблицу зость
//! дословно; оракулы `red_restore_drill_metric.rs` и `red_m99_restore_drill_watch.rs`
//! написаны ПРОТИВ этого модуля, и любое отклонение от их сигнатур ловится компилятором.
//!
//! ## Что лежит здесь и почему
//!
//! **Один источник окна свежего.** `RESTORE_DRILL_FRESH_WINDOW_MS = 40 суток`
//! (`M-74` §«Окно свежести — 40 суток»): месячное расписание плюс запас на один
//! пропусленный прогон. Число живёт ТОЛЬКО в одной константе этого модуля; recorder
//! реэкспортирует (`pub use ops::restore_drill::RESTORE_DRILL_FRESH_WINDOW_MS;`),
//! и `verify_M-99.sh` отдельной строкой пиннит, что иной файл `crates/**` её не
//! дублирует. Менять — только вместе с обновлением спеки.
//!
//! **T2-контракт файла состояния** (`M-74` §«T2-контракт…»): обязательные поля
//! `ok`/`ts_wall_ms`/`checked`/`events_read`; `digest` (64 hex) обязателен при
//! `ok=1` (`M-99` §4 п.4 — без него состояние = заявление о действии, потребитель
//! обязан отвергать). Поле `reason` — обязательно при `ok=0`, классы причин
//! ОБЯЗАНЫ РАЗЛИЧАТЬСЯ. Здесь это соблюдается по таблице зости ниже.
//!
//! **Fail-closed по каждому классу.** `Missing`/`Unparseable`/`Failed`/`Empty`/
//! `Stale`/`FromFuture` — каждое условие объявлено отдельным вердиктом; первый
//! сработавший побеждает, потом общий ВЕРДИКТ уже не терпит (таблица зости
//! упорядочена по приоритету и есть порядок проверок).

use std::path::Path;

/// Окно свежести restore-drill'а. 40 суток в миллисекундах: месячное расписание плюс
/// запас на ОДИН пропусленный прогон. Единственное место, где это число живёт.
///
/// Проверка границы — двусторонняя (`M-99` §5 + спека `M-74`):
/// ровно на границе (`now - ts_wall_ms == window`) drill ещё свежий;
/// на 1 мс за границей — `Stale`. Симметрия держится оракулом `v2_freshness_window_…`
/// в `red_m99_restore_drill_watch.rs` и парными `stale_*` тестами в
/// `red_restore_drill_metric.rs`.
pub const RESTORE_DRILL_FRESH_WINDOW_MS: i64 = 40 * 24 * 60 * 60 * 1000;

/// Дефолтный путь файла состояния drill'а. Композиция (testing.md, канарейка
/// «путь продюсера = путь потребителя»): обёртка пишет сюда, сторож читает отсюда;
/// рассогласование даёт тихий no-op. Проверяется `verify_M-99.sh` (задача 6:
/// сравнение дефолта обёртки с этой константой + регексп по `DEFAULT_RESTORE_DRILL_STATE`).
///
/// Круг 2 (`R-263` Б-4): путь в ОТДЕЛЬНОМ каталоге `/var/lib/hft/drill-state/`. Recorder —
/// потребитель метрики — работает в контейнере и видит хост только через объявленные
/// тома; прежний путь в корне `/var/lib/hft` в контейнер не монтировался, и метрика на
/// проде была бы 0 и после успеха. Монтируется КАТАЛОГ, а не файл: обёртка пишет
/// `tmp → rename`, а bind-mount файла держит прежний inode и не увидел бы ни одной новой
/// записи. Каталог отдельный — том не тащит в контейнер ничего, кроме состояния drill'а
/// (каталог восстановления — 2.5 ГБ — туда не входит).
pub const DEFAULT_RESTORE_DRILL_STATE: &str = "/var/lib/hft/drill-state/journal-restore-drill.json";

/// Регулярка для отпечатка при `ok=1`: ровно 64 строчных hex-символа.
/// T2-контракт: «обязательно при `ok=1`» (`M-74` §«Поле `digest` — ЯДРО rev 5»),
/// и `M-99` §4 п.4 закрепил это отдельным требованием: «ok=1 без 64-hex digest ⇒ 0».
/// Здесь — единый валидатор; оба потребителя (recorder + watchdog) используют ту же
/// функцию. Без зависимости от `regex` — дёшево, нечего компилировать.
fn is_64_hex(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 64 && bytes.iter().all(|b| b.is_ascii_hexdigit())
}

/// Решение отображения «файл состояния → вердикт». Каждый вариант — отдельный класс
/// входа, fail-closed по построению. Порядок вариантов здесь не обязан совпадать
/// с таблицей приоритетов в спеке — он отсортирован по семантике (Ok в начале,
/// «нет файла» рядом), а приоритет проверок реализуется ВНУТРИ [`restore_drill_verdict`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreDrillVerdict {
    /// Файл свеж, `ok=1`, есть валидный 64-hex `digest`, `events_read >= 1`,
    /// метка НЕ из будущего.
    Ok,
    /// Файла нет / не читается / это каталог.
    Missing,
    /// JSON не разобран (в т.ч. обрывок); нет обязательного поля;
    /// `ok=1` без 64-hex `digest` (`M-99` §4 п.4 — `digest` обязателен при `ok=1`).
    Unparseable,
    /// `ok != 1` (явный провал).
    Failed,
    /// `ok=1`, `digest` валиден, но `events_read == 0` — `R-157` `Б-5` на уровне
    /// метрики/тревоги: «успех», ничего не прочитавший, успехом не является.
    Empty,
    /// `now - ts_wall_ms > RESTORE_DRILL_FRESH_WINDOW_MS`.
    Stale,
    /// `ts_wall_ms > now` — часы сбиты либо файл подложен.
    FromFuture,
}

impl RestoreDrillVerdict {
    /// `true` ⇔ [`RestoreDrillVerdict::Ok`]. Используется ОБОИМЯ потребителями:
    /// recorder пишет `verdict.is_ok() as 0|1` (`M-99` §5), сторож звонит
    /// `check_restore_drill(&v).is_some()` как инверсию.
    pub fn is_ok(&self) -> bool {
        matches!(self, RestoreDrillVerdict::Ok)
    }
}

/// Разобрать и оценить файл состояния. Чистая функция: часы — снаружи, файлового I/O —
/// ровно одна попытка `read_to_string` (нена модуль — ровно как и `sample_restore_drill`
/// у recorder'а, см. `samples.md` §`M-09` task 4C: «I/O, кроме разового `read_to_string`,
/// sampler 1 Гц»). Никогда не паникует: на любом отказе ввода — соответствующий
/// вариант [`RestoreDrillVerdict`].
///
/// Порядок проверок — таблица приоритетов из спеки `M-99` §5 дословно: первое
/// сработавшее условие и есть вердикт. Менять порядок = менять семантику зость.
pub fn restore_drill_verdict(state_path: &Path, now_wall_ms: i64) -> RestoreDrillVerdict {
    // (1) Файла нет / не читается / это каталог — `Missing`.
    let body = match std::fs::read_to_string(state_path) {
        Ok(s) => s,
        Err(_) => return RestoreDrillVerdict::Missing,
    };

    // (2) JSON не разобран — `Unparseable` (обрывок, мусор, не-JSON). Дальше поля не
    //     трогаем: разборка могла потерять их на полпути и оставить неконсистентное
    //     состояние с правдоподобными полями.
    let v: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return RestoreDrillVerdict::Unparseable,
    };

    // (3) Обязательные поля по T2 (`M-74`): `ok`, `ts_wall_ms`, `checked`. Любое
    //     отсутствие — `Unparseable` (контракт не выполнен). Это РАНЬШЕ `Failed`:
    //     «нет поля `ok`» — не «явный провал», а «контракт не выполнен»
    //     (`R-263` Н-1).
    let ok = match v.get("ok").and_then(serde_json::Value::as_i64) {
        Some(o) => o,
        None => return RestoreDrillVerdict::Unparseable,
    };
    let ts_wall_ms = match v.get("ts_wall_ms").and_then(serde_json::Value::as_i64) {
        Some(t) => t,
        None => return RestoreDrillVerdict::Unparseable,
    };
    if v.get("checked").is_none() {
        return RestoreDrillVerdict::Unparseable;
    }

    // (4) `ok != 1` — `Failed` (явный провал). Проверяем ДО `digest`, потому что
    //     `ok=0` снимает требование наличия `digest`: `digest` обязателен ТОЛЬКО при
    //     `ok=1` (T2-контракт `M-74`).
    if ok != 1 {
        return RestoreDrillVerdict::Failed;
    }

    // (5) `digest` обязателен при `ok=1` (`M-99` §4 п.4 + T2-контракт `M-74`). Без
    //     него / не 64 СТРОЧНЫХ hex — `Unparseable`: состояние не доказывает действия.
    //     Заглавные hex — `Unparseable` (писатель отпечатка — строчный; `R-263` Н-3).
    match v.get("digest").and_then(serde_json::Value::as_str) {
        Some(d)
            if is_64_hex(d)
                && d.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) => {}
        _ => return RestoreDrillVerdict::Unparseable,
    };

    // (6) `events_read == 0` ИЛИ поля нет — `Empty` (нет доказательства чтения).
    //     Поле `events_read` ОБЯЗАТЕЛЬНО по T2-контракту; отсутствие трактуется как
    //     «явный ноль» — оба пути дают `Empty`, а не `Unparseable` (`R-263` Н-1).
    match v.get("events_read") {
        Some(serde_json::Value::Number(n)) if n.as_u64() == Some(0) => {
            return RestoreDrillVerdict::Empty;
        }
        Some(serde_json::Value::Number(n)) if n.as_u64().is_some() => {}
        // Поля нет, или это не число, или не u64 — оба дают `Empty` по §5.
        _ => return RestoreDrillVerdict::Empty,
    }

    // (7) `ts_wall_ms > now` — `FromFuture` (часы сбиты либо файл подложен). Это
    //     РАНЬШЕ `Stale`: метка из будущего подозрительнее просроченной. И РАНЬШЕ
    //     `Empty`: по таблице §5 приоритет Empty выше FromFuture.
    if ts_wall_ms > now_wall_ms {
        return RestoreDrillVerdict::FromFuture;
    }

    // (8) `now - ts_wall_ms > RESTORE_DRILL_FRESH_WINDOW_MS` — `Stale`. Разность
    //     через `saturating_sub` (метка `i64::MIN` даёт `Stale`, не панику и не
    //     `Ok` — `R-263` Н-2). Ровно на границе (`== window`) ещё `Ok`; на 1 мс за
    //     границей — `Stale`. Симметрия проверяется оракулом v2.
    if now_wall_ms.saturating_sub(ts_wall_ms) > RESTORE_DRILL_FRESH_WINDOW_MS {
        return RestoreDrillVerdict::Stale;
    }

    RestoreDrillVerdict::Ok
}

#[cfg(test)]
mod tests {
    //! Самостоятельные unit-проверки отображения. Полное покрытие (мутации, прод-форма,
    //! композиция с другими) — в оракулах `red_restore_drill_metric.rs` (recorder-сторона)
    //! и `red_m99_restore_drill_watch.rs` (сторож-сторона); здесь — sanity на собственные
    //! утверждения о приоритетах и форме.
    use super::*;

    fn write(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().expect("tmpdir");
        let p = d.path().join("journal-restore-drill.json");
        std::fs::write(&p, body).expect("write");
        (d, p)
    }

    const NOW: i64 = 1_788_169_968_723;
    const DIGEST: &str = "96a231ce97316518856045415a7fdf089ce5106f427c239cdc07fb3dbbdb9c81";

    #[test]
    fn missing_file_is_missing() {
        let d = tempfile::tempdir().expect("tmpdir");
        assert_eq!(
            restore_drill_verdict(&d.path().join("nope.json"), NOW),
            RestoreDrillVerdict::Missing
        );
    }

    #[test]
    fn truncated_json_is_unparseable() {
        let (_d, p) = write(r#"{"ok":1,"ts_wall_ms":1788169"#);
        assert_eq!(
            restore_drill_verdict(&p, NOW),
            RestoreDrillVerdict::Unparseable
        );
    }

    #[test]
    fn ok_equals_one_but_no_digest_is_unparseable() {
        let (_d, p) = write(&format!(
            r#"{{"ok":1,"ts_wall_ms":{},"checked":3,"events_read":41213}}"#,
            NOW - 60_000
        ));
        assert_eq!(
            restore_drill_verdict(&p, NOW),
            RestoreDrillVerdict::Unparseable
        );
    }

    #[test]
    fn ok_equals_one_with_short_digest_is_unparseable() {
        let (_d, p) = write(&format!(
            r#"{{"ok":1,"ts_wall_ms":{},"checked":3,"events_read":41213,"digest":"abc"}}"#,
            NOW - 60_000
        ));
        assert_eq!(
            restore_drill_verdict(&p, NOW),
            RestoreDrillVerdict::Unparseable
        );
    }

    #[test]
    fn explicit_failure_is_failed() {
        let (_d, p) = write(&format!(
            r#"{{"ok":0,"ts_wall_ms":{},"checked":3,"events_read":0,"reason":"reader-rc-4"}}"#,
            NOW - 60_000
        ));
        assert_eq!(restore_drill_verdict(&p, NOW), RestoreDrillVerdict::Failed);
    }

    #[test]
    fn success_with_zero_events_is_empty() {
        let (_d, p) = write(&format!(
            r#"{{"ok":1,"ts_wall_ms":{},"checked":3,"events_read":0,"digest":"{DIGEST}"}}"#,
            NOW - 60_000
        ));
        assert_eq!(restore_drill_verdict(&p, NOW), RestoreDrillVerdict::Empty);
    }

    #[test]
    fn fresh_success_with_digest_is_ok() {
        let (_d, p) = write(&format!(
            r#"{{"ok":1,"ts_wall_ms":{},"checked":3,"events_read":41213,"digest":"{DIGEST}"}}"#,
            NOW - 60_000
        ));
        assert_eq!(restore_drill_verdict(&p, NOW), RestoreDrillVerdict::Ok);
    }

    #[test]
    fn from_future_beats_stale() {
        let (_d, p) = write(&format!(
            r#"{{"ok":1,"ts_wall_ms":{},"checked":3,"events_read":41213,"digest":"{DIGEST}"}}"#,
            NOW + 1
        ));
        assert_eq!(
            restore_drill_verdict(&p, NOW),
            RestoreDrillVerdict::FromFuture
        );
    }

    #[test]
    fn boundary_at_exactly_window_is_still_ok() {
        let (_d, p) = write(&format!(
            r#"{{"ok":1,"ts_wall_ms":{},"checked":3,"events_read":41213,"digest":"{DIGEST}"}}"#,
            NOW - RESTORE_DRILL_FRESH_WINDOW_MS
        ));
        assert_eq!(restore_drill_verdict(&p, NOW), RestoreDrillVerdict::Ok);
    }

    #[test]
    fn one_ms_past_window_is_stale() {
        let (_d, p) = write(&format!(
            r#"{{"ok":1,"ts_wall_ms":{},"checked":3,"events_read":41213,"digest":"{DIGEST}"}}"#,
            NOW - RESTORE_DRILL_FRESH_WINDOW_MS - 1
        ));
        assert_eq!(restore_drill_verdict(&p, NOW), RestoreDrillVerdict::Stale);
    }

    #[test]
    fn default_state_path_is_well_known() {
        assert_eq!(
            DEFAULT_RESTORE_DRILL_STATE,
            "/var/lib/hft/drill-state/journal-restore-drill.json"
        );
    }

    #[test]
    fn is_ok_only_on_ok_variant() {
        assert!(RestoreDrillVerdict::Ok.is_ok());
        assert!(!RestoreDrillVerdict::Missing.is_ok());
        assert!(!RestoreDrillVerdict::Unparseable.is_ok());
        assert!(!RestoreDrillVerdict::Failed.is_ok());
        assert!(!RestoreDrillVerdict::Empty.is_ok());
        assert!(!RestoreDrillVerdict::Stale.is_ok());
        assert!(!RestoreDrillVerdict::FromFuture.is_ok());
    }
}
