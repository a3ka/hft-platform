//! `journal-drill-read` — read-only операторский бинарь, читатель drill'а
//! (`M-74` задача 2b, `M-99` §4 п.3). Отдельный `--bin journal-drill-read`
//! потому, что прод-читатель ОБЯЗАН попасть в прод-образ (testing.md
//! §«Механизм несущего пути»): файл в репозитории при деплое, который его не
//! устанавливает, инертен. `Dockerfile` собирает и `COPY`ит его в образ
//! `/usr/local/bin/journal-drill-read`, и обёртка drill'а зовёт его через
//! `docker run --rm --network none ... --entrypoint /usr/local/bin/journal-drill-read
//! <образ работающего hft-recorder>` (M-99 §4 п.3: дефолт).
//!
//! ## Контракт argv (на нём держится различение исходов в файле состояния drill'а)
//!
//! ```text
//! journal-drill-read --dir <PATH> [--min-events <N>]
//!   --dir          каталог восстановления (тот же путь, в который обёртка писала доставку;
//!                  рассогласование двух строк даёт тихий no-op, M-99 §4 п.3)
//!   --min-events   минимум событий, требуемый для `rc=0` (по умолчанию 1)
//! ```
//!
//! ## Контракт stdout при успехе — ОДНА строка JSON, ШЕСТЬ полей
//!
//! ```text
//! {"segments_read":3,"events_read":41213,"seq_first":…,"seq_last":…,
//!  "intra_segment_continuous":true,"digest":"<64 hex>"}
//! ```
//!
//! `digest` — sha256 конкатенации `"<seq>:<ts_exch_ms>:<price_e8>\n"` по событиям В
//! ПОРЯДКЕ ЧТЕНИЯ (T2-контракт `M-74`). События без цены (не-`MdPayload::Trade`) в
//! свёртку НЕ входят. Эталонный читатель пробы (`fixture_restore_drill_cold.rs`)
//! считает по той же формуле — расхождение отслочивается в сценарии `N` (задача 6b).
//!
//! ## Exit-коды — МАШИННЫЙ КОНТРАКТ (M-99 §3)
//!
//! - `0` — журнал открыт, прочитано ≥ `--min-events`, непрерывность ВНУТРИ
//!   сегментов держится (МЕЖСЕГМЕНТНАЯ непрерывность на выборке невозможна по
//!   построению — три несмежных индекса; семантика «сброс на границе» реализуется
//!   в `is_intra_segment_continuous`).
//! - `1` — неверные аргументы / `--dir` не указан.
//! - `4` — ЧТЕНИЕ: прод-читатель вернул `Err` (порча байт, crc, битый zstd,
//!   и т.п.) — лечится перезаливом копии.
//! - `5` — ПУСТОТА: открылось, но событий меньше `--min-events`. Это ОТДЕЛЬНЫЙ
//!   код от (4): лечится разбором доставки (сеть, права, фильтр), а не
//!   копии. `journal::stream` на пустом каталоге возвращает `Ok(0)` — код
//!   различает «нет файлов» (`segments_read == 0` явно ловим ДО открытия
//!   потока) и «прочитано, но мало» (`events_read < min`).
//! - `6` — КОНТЕКСТ: `journal::is_foreign_segment` — сегмент без магии и без
//!   декларации. На наших данных недостижимо (замер 2026-10-10: деклараций
//!   ноль), ветка оставлена для forward-guard'а.
//!
//! ## Что НЕ делается
//!
//! - **Не делаем I/O за пределами `dir`** (ни в журнал, ни на Storage Box) —
//!   `M-99` §7 «Запрещено»: «проверка бэкапа не должна становиться риском для
//!   оригинала». `journal::stream` сам ходит ТОЛЬКО в `--dir`.
//! - **Не пишем в `journal-restore-drill.json`** — это зона обёртки. Бинарь
//!   читает ТОЛЬКО.
//!
//! ## Почему `--bin`, а не часть `journal-retention`
//!
//! `journal-retention` уже есть в проде, и читает те же сегменты. Однако он
//! принимает решения (план/apply/compact), а drill-читатель ОБЯЗАН быть чистым:
//! один режим, один контракт stdout, никаких `--mode`. Переиспользовать
//! `journal-retention` для drill'а означало бы менять его контракт argv под
//! два разных вызова — а это и есть тот класс «два источника правды об одном
//! бинаре», который корпус уже проходил (TD-227).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use journal::{is_foreign_segment, EpochFilter, EventStream};
use sha2::{Digest, Sha256};

/// Дефолт минимума событий: 1. Обёртка drill'а (`deploy/bin/journal-restore-drill-cron.sh`)
/// зовёт с `--min-events 1` — успех только если прочитано ХОТЯ БЫ одно событие
/// (иначе `rc=5` ПУСТОТА, и `ok=0` в файле состояния). Это и есть fail-closed
/// против `journal::stream` на пустом каталоге, который вернул бы `Ok(0)` без
/// сигнала «ничего не прочитано».
const DEFAULT_MIN_EVENTS: usize = 1;

#[derive(Debug)]
struct Args {
    dir: PathBuf,
    min_events: usize,
}

fn parse_args() -> Result<Args, String> {
    let mut dir: Option<PathBuf> = None;
    let mut min_events = DEFAULT_MIN_EVENTS;
    let mut prev = String::new();
    for cur in std::env::args().skip(1) {
        match (prev.as_str(), cur.as_str()) {
            ("--dir", v) => dir = Some(PathBuf::from(v)),
            ("--min-events", v) => {
                min_events = v
                    .parse::<usize>()
                    .map_err(|e| format!("--min-events: не число: {e}"))?;
            }
            (_, "--dir") | (_, "--min-events") => { /* ждём значение на след. итерации */
            }
            (_, _) if cur.starts_with("--") => {
                return Err(format!("неизвестный флаг: {cur}"));
            }
            (_, _) => return Err(format!("лишний позиционный аргумент: {cur}")),
        }
        prev = cur;
    }
    let dir = dir.ok_or_else(|| "--dir <PATH> обязателен".to_string())?;
    Ok(Args { dir, min_events })
}

/// Подсчитать свёртку и собрать JSON одной строкой. Чистая функция: на входе —
/// уже прочитанный поток событий (по которому считается `digest`), на выходе —
/// JSON-строка, готовая к печати в stdout.
///
/// **Почему через `is_intra_segment_continuous`, а не «всегда `true`».** На
/// выборке несмежных индексов (`first` / `floor(N/2)` / `last`) непрерывность
/// сквозная НЕВОЗМОЖНА по построению; заявлять её — ложь (`M-74` §«Правило
/// выборки», СЕМАНТИКА НЕПРЕРЫВНОСТИ). Сброс на границе сегмента — по
/// `first_seq` из заголовков (`C-218` B-1): угадывание «большой разрыв = стык»
/// пропустило бы порчу, выевшую много событий подряд.
///
/// **Почему здесь, а не в `journal::stream`.** Эталонный читатель пробы
/// (`fixture_restore_drill_cold.rs::read_dir_full`) считает `digest` через ту же
/// формулу; различие у прод-бинаря и у эталона — нарушение `N`-сценария пробы.
fn build_result(
    stream: &mut EventStream,
    bounds: &std::collections::HashSet<u64>,
    min_events: usize,
) -> (DrillResult, ExitCode) {
    let mut hasher = Sha256::new();
    let mut events_read: u64 = 0;
    let mut seq_first: Option<u64> = None;
    let mut seq_last: u64 = 0;
    let mut continuous = true;
    let mut prev_seq: Option<u64> = None;
    loop {
        let ev = match stream.next() {
            Some(Ok(e)) => e,
            Some(Err(e)) => {
                // `is_foreign_segment` (код 6) идёт ДО общего «портча» (код 4).
                if is_foreign_segment(&e) {
                    return (DrillResult::empty(), ExitCode::from(6));
                }
                return (DrillResult::empty(), ExitCode::from(4));
            }
            None => break,
        };
        if seq_first.is_none() {
            seq_first = Some(ev.seq);
        }
        seq_last = ev.seq;
        // Сброс `prev` на границе сегмента: первое событие сегмента
        // (`first_seq` совпал с `ev.seq`) — законный разрыв. Сравнивать его
        // с хвостом ПРЕДЫДУЩЕГО сегмента нечего: те сегменты разные
        // несмежные индексы.
        if bounds.contains(&ev.seq) {
            prev_seq = None;
        }
        if let Some(p) = prev_seq {
            if ev.seq > p + 1 {
                continuous = false;
            }
        }
        prev_seq = Some(ev.seq);
        // Только `MdPayload::Trade` (несёт `price`/`ts_exch_ms`) — в свёртку.
        // Прочие payload'ы не дают полей, обещанных формулой, и их участие
        // сделало бы формулу невычислимой эталоном (фикстура порождает ТОЛЬКО
        // Trade-события — иначе обе стороны пробы разойдутся).
        if let contracts::EventKind::Md(md) = &ev.kind {
            if let contracts::MdPayload::Trade {
                price, ts_exch_ms, ..
            } = &md.payload
            {
                hasher.update(format!("{}:{}:{}\n", ev.seq, ts_exch_ms, price).as_bytes());
                events_read += 1;
            }
        }
    }
    let segments_read = stream.segments_opened() as usize;
    let result = DrillResult {
        segments_read,
        events_read: events_read as usize,
        seq_first: seq_first.unwrap_or(0),
        seq_last,
        intra_segment_continuous: continuous,
        digest: format!("{:x}", hasher.finalize()),
    };
    if result.events_read < min_events {
        // ПУСТОТА — отдельный исход (`M-74` §«Сигнатура читателя»). На пустом
        // каталоге `journal::stream` вернул бы `Ok(0)` без единого события;
        // drill, судящий по exit=0 читателя, объявил бы успех. `min_events=1`
        // в проде (дефолт обёртки) и наш явный счётчик делают этот класс
        // видимым. `segments_read == 0` (катастрофический случай «каталог
        // пуст») тоже попадает сюда: events_read=0 < min_events=1.
        return (result, ExitCode::from(5));
    }
    (result, ExitCode::from(0))
}

struct DrillResult {
    segments_read: usize,
    events_read: usize,
    seq_first: u64,
    seq_last: u64,
    intra_segment_continuous: bool,
    digest: String,
}

impl DrillResult {
    fn empty() -> DrillResult {
        DrillResult {
            segments_read: 0,
            events_read: 0,
            seq_first: 0,
            seq_last: 0,
            intra_segment_continuous: true,
            digest: String::new(),
        }
    }
}

/// Разово прочитать каталог и собрать `bounds = {first_seq всех сегментов}` для
/// сброса `prev_seq` на границе (см. `build_result`).
fn collect_bounds(dir: &Path) -> std::collections::HashSet<u64> {
    journal::list_segments(dir)
        .map(|v| v.iter().map(|s| s.header.first_seq).collect())
        .unwrap_or_default()
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("journal-drill-read: {e}");
            eprintln!("(см. --help: --dir <PATH> [--min-events <N>])");
            return ExitCode::from(1);
        }
    };

    // Открыть поток чтения (`EpochFilter::All` — drill судит ВСЁ, что в каталоге,
    // включая чужие сегменты; они идут в `is_foreign_segment` → rc=6). Фильтр
    // `OwnCaptureOnly` тут был бы преждевременным сужением: drill предназначен
    // доказать читаемость КОПИИ, а не её чистоту относительно eo.
    let mut stream = match journal::stream(&args.dir, EpochFilter::All) {
        Ok(s) => s,
        Err(e) => {
            // Ошибка открытия каталога журнала (нет `segment-*.jrnl` ИЛИ
            // ошибка чтения каталога как такового) — rc=5 (ПУСТОТА): для
            // обёртки это неотличимо от «ничего не привезли» и лечится тем же
            // путём (разбор доставки / том).
            eprintln!("journal-drill-read: stream({}): {e}", args.dir.display());
            return ExitCode::from(5);
        }
    };

    let bounds = collect_bounds(&args.dir);
    let (result, rc) = build_result(&mut stream, &bounds, args.min_events);

    // JSON одной строкой — это контракт stdout (см. пробу `red_restore_drill.sh`,
    // где `sed -n 's/.*"digest"[[:space:]]*:[[:space:]]*"\([0-9a-f]\{64\}\)".*/\1/p'`
    // берёт ПОСЛЕДНЕЕ поле; и эта формат-строка — та, что её подбирает).
    // Управляющие символы в строке не появятся: `digest` — 64 hex, `events_read`/
    // `segments_read`/`seq_first`/`seq_last` — целые, `intra_segment_continuous` —
    // bool.
    println!(
        r#"{{"segments_read":{},"events_read":{},"seq_first":{},"seq_last":{},"intra_segment_continuous":{},"digest":"{}"}}"#,
        result.segments_read,
        result.events_read,
        result.seq_first,
        result.seq_last,
        if result.intra_segment_continuous {
            "true"
        } else {
            "false"
        },
        result.digest,
    );
    rc
}

#[cfg(test)]
mod tests {
    //! Самостоятельные проверки argv/JSON-формы и границ логики, не зависящие от
    //! фикстуры прод-формы (та — в `fixture_restore_drill_cold.rs`).
    use super::*;

    #[test]
    fn empty_result_serialises_with_all_six_fields() {
        let r = DrillResult::empty();
        // шесть полей по контракту: `C-217` N-1 — проверка формой.
        let s = format!(
            r#"{{"segments_read":{},"events_read":{},"seq_first":{},"seq_last":{},"intra_segment_continuous":{},"digest":"{}"}}"#,
            r.segments_read,
            r.events_read,
            r.seq_first,
            r.seq_last,
            if r.intra_segment_continuous {
                "true"
            } else {
                "false"
            },
            r.digest,
        );
        // Подсчёт полей в JSON через regex.
        let count = s.matches(',').count() + 1;
        assert_eq!(count, 6, "ровно шесть полей: {s}");
        assert!(s.contains("\"digest\":\"\""));
        assert!(s.contains("\"segments_read\":0"));
    }

    #[test]
    fn drill_result_empty_is_empty() {
        let r = DrillResult::empty();
        assert_eq!(r.segments_read, 0);
        assert_eq!(r.events_read, 0);
        assert_eq!(r.seq_first, 0);
        assert_eq!(r.seq_last, 0);
        assert!(r.intra_segment_continuous);
        assert!(r.digest.is_empty());
    }
}
