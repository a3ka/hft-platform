//! `SM-7` — guard `hint.pos` против длины файла и границы кадра (M-62 §7, `R-040` NOTE-1).
//!
//! ЗАЧЕМ ОТДЕЛЬНЫМ ФАЙЛОМ. Эти оракулы проверяют задачу 4 и от счётчика `segment_meta_ops`
//! (задача 1) не зависят — значит обязаны быть запускаемыми СЕГОДНЯ, до всякой реализации
//! кеша. В одном файле с оракулами счётчика они бы не компилировались и молчали, то есть
//! проверка задачи 4 оказалась бы заблокирована выполнением задачи 1.
//!
//! ПОЧЕМУ GUARD ВХОДИТ В M-62, А НЕ ЖИВЁТ ДОЛГОМ. `resolve_active_start_offset`
//! (`crates/journal/src/segments.rs:1102-1136`) валидирует hint ЧЕТЫРЬМЯ условиями
//! (`seg_idx` совпал · `pos >= header_end` · `after >= last_seq` · есть `after_seq`) и при
//! любом сомнении возвращает `header_end` — конструкция fail-safe, и она верна. Но `hint.pos`
//! НЕ проверяется против длины файла и границы кадра. Сегодня это недостижимо: сегмент
//! append-only, ротацию ловит `seg_idx`. Механизм M-62 — кеш метаданных, ЖИВУЩИЙ МЕЖДУ
//! ТИКАМИ, — расширяет окно, в котором сессия действует по устаревшему представлению о
//! сегменте: компакция или усечение активного при неизменном индексе дадут seek за EOF
//! (ТИШИНА — сессия молча перестаёт отдавать события) либо в середину кадра (МУСОР).
//! Класс «тихая деградация при зелёном healthcheck» в проекте уже реализовывался: `TD-031`
//! поймал только глазной sanity свежих событий, ни один гейт его не увидел.
//!
//! ЗАМЕР ФОРМАТА (11.08). Отдельной функции чтения заголовка кадра НЕТ и per-frame magic НЕ
//! СУЩЕСТВУЕТ: единственный валидатор границы — CRC. Формат (`segments.rs:17-19`):
//! `SEGMENT_MAGIC = b"HFTJRN02"` (8 B) → `[u32 LE len][postcard(SegmentHeader)][u32 LE crc32]`
//! → далее подряд `[u32 LE len][postcard(Event)][u32 LE crc32]`; читатель — `read_frame_payload`
//! (`:472-509`), санити-кап длины 64 MiB (`:461`). Значит guard обязан ПРОБОВАТЬ ЧТЕНИЕ кадра,
//! а не только сравнивать числа.
//!
//! ЭТАЛОН — НЕЗАВИСИМЫЙ. Выдача сверяется с полным проходом `stream_from` без hint'а, а не с
//! тем же seek-путём: сравнение источника с самим собой есть тавтология (`testing.md`).
//!
//! СОСТОЯНИЕ: RED. `sm7_a` и `sm7_b` обязаны падать на сегодняшнем коде (guard'а нет);
//! `sm7_c` обязан быть ЗЕЛЁНЫМ уже сейчас и остаться зелёным после правки — он пиннит
//! легитимный хвост, который наивное «pos >= len ⇒ откат» сломало бы.

use std::fs;
use std::path::Path;

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, TailHint, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;
const SEG_BYTES: u64 = 1024 * 1024 * 1024;
const INCREMENT: u64 = 3;
const BUDGET_SCANNED: u64 = INCREMENT * 4;

/// Манифест этого файла; сверку с таблицей §4.2 спеки делает `scripts/verify_M-62.sh`
/// по ОБОИМ файлам набора.
const MANIFEST: &[(&str, u8, &str, char)] = &[
    ("sm7_a", 4, "сегмент исчез, а кеш живёт", 'V'),
    ("sm7_b", 4, "сегмент исчез, а кеш живёт", 'V'),
    ("sm7_c", 3, "первый тик сессии платит полную цену", 'L'),
];

fn claims(id: &str, axis: u8, value: &str) {
    assert!(
        MANIFEST
            .iter()
            .any(|(i, a, v, _)| *i == id && *a == axis && *v == value),
        "МАНИФЕСТ НЕ СОДЕРЖИТ заявленного покрытия: {id} / ось {axis} / «{value}»"
    );
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m62-guard".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(100.0 + (i % 5) as f64),
            size: to_fixed(1.0 + (i % 3) as f64),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: D2_MS + (i as i64 * 100),
        },
    )
}

fn append_range(dir: &Path, from: u64, to: u64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in from..to {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
}

fn reference_seqs(dir: &Path, after: Option<u64>) -> Vec<u64> {
    let mut out = Vec::new();
    let mut s = journal::stream_from(dir, EpochFilter::OwnCaptureOnly, after).expect("stream_from");
    for ev in s.by_ref() {
        out.push(ev.expect("event").seq);
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// SM-7 — guard `hint.pos`: усечение, середина кадра и ЛЕГИТИМНЫЙ хвост
//
// ⚠️ КОНСТРУКЦИЯ ФИКСТУРЫ (переписана после первого прогона — он дал ЛОЖНОЕ ЗЕЛЁНОЕ).
// Первая ветка `resolve_active_start_offset` — `after_seq: None ⇒ return header_end`
// (`segments.rs:1110`). Значит вызов с `after=None` до проверки `hint.pos` НЕ ДОХОДИТ, и
// оракул, построенный так, зеленеет всегда — «guard сработал» и «hint не рассматривался»
// дают побайтно одинаковую выдачу. Проба, молча тестирующая не тот сценарий, есть плацебо
// самой себя (`testing.md` §«Целостность гейта», свойство 3).
//
// Поэтому каждый оракул ниже сперва предъявляет ПОЗИТИВНЫЙ КОНТРОЛЬ: с ВАЛИДНЫМ hint'ом
// тик читает мало (hint-путь жив в этой фикстуре), и только затем портит позицию.
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Активный сегмент = максимальный индекс.
fn active_segment(dir: &Path) -> journal::SegmentInfo {
    let mut all = journal::list_segments(dir).expect("list_segments");
    all.sort_by_key(|s| s.index);
    all.pop().expect("хотя бы один сегмент")
}

/// Выдача ТЕРПИМА к ошибке потока: сырой `io::Error` из `.expect()` съел бы объяснение
/// оракула и оставил читателя с текстом вроде «frame length absurd», из которого не следует
/// НИ ЧТО сломано, НИ почему это дефект. Ошибка возвращается третьим элементом и попадает в
/// сообщение ассерта.
fn yield_with_hint(
    dir: &Path,
    hint: Option<TailHint>,
    after: Option<u64>,
) -> (Vec<u64>, u64, Option<String>) {
    let mut s = match journal::stream_from_at(dir, EpochFilter::OwnCaptureOnly, after, hint) {
        Ok(s) => s,
        Err(e) => return (Vec::new(), 0, Some(format!("stream_from_at: {e}"))),
    };
    let mut seqs = Vec::new();
    let mut err = None;
    for ev in s.by_ref() {
        match ev {
            Ok(e) => seqs.push(e.seq),
            Err(e) => {
                err = Some(format!("{e}"));
                break;
            }
        }
    }
    (seqs, s.events_scanned(), err)
}

/// Фикстура: журнал, догнанный до хвоста, ВАЛИДНЫЙ hint от прошлого прохода и приращение.
/// Возвращает `(dir, valid_hint, after_seq, ожидаемые seq приращения)`.
fn caught_up_with_valid_hint() -> (tempfile::TempDir, TailHint, u64, Vec<u64>) {
    let dir = tempfile::tempdir().expect("tempdir");
    append_range(dir.path(), 0, 400);
    let all = reference_seqs(dir.path(), None);
    let last = *all.last().expect("события есть");

    // Проход 1 — так же, как это делает сессия: hint берётся у самого потока.
    let mut s = journal::stream_from_at(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("stream_from_at");
    for ev in s.by_ref() {
        ev.expect("event");
    }
    let hint = s.tail_hint().expect(
        "SETUP НЕ СОСТОЯЛСЯ: поток не вернул tail_hint — механизм M-57 не работает, и \
         сценарий guard'а не воспроизводится",
    );

    append_range(dir.path(), 400, 400 + INCREMENT);
    let expect: Vec<u64> = reference_seqs(dir.path(), Some(last));
    assert_eq!(
        expect.len() as u64,
        INCREMENT,
        "SETUP НЕ СОСТОЯЛСЯ: приращение {} вместо {INCREMENT}",
        expect.len()
    );
    (dir, hint, last, expect)
}

/// Позитивный контроль: с ВАЛИДНЫМ hint'ом тик обязан быть дешёвым. Если он дорог, значит
/// hint-путь в этой фикстуре не задействован вовсе, и любой вывод ниже недействителен.
fn assert_hint_path_is_live(dir: &Path, hint: TailHint, after: u64, expect: &[u64]) {
    let (got, scanned, err) = yield_with_hint(dir, Some(hint), Some(after));
    assert!(
        err.is_none(),
        "ПОЗИТИВНЫЙ КОНТРОЛЬ НЕ ПРОШЁЛ: поток сломался: {err:?}"
    );
    assert_eq!(
        got, expect,
        "ПОЗИТИВНЫЙ КОНТРОЛЬ НЕ ПРОШЁЛ: с валидным hint'ом выдача разошлась с эталоном"
    );
    assert!(
        scanned <= BUDGET_SCANNED,
        "ПОЗИТИВНЫЙ КОНТРОЛЬ НЕ ПРОШЁЛ: с ВАЛИДНЫМ hint'ом тик прочитал {scanned} событий \
         при бюджете {BUDGET_SCANNED}. Значит hint не применяется в этой фикстуре вообще, и \
         оракулы порчи позиции ниже проверяли бы не тот сценарий — плацебо самих себя."
    );
}

#[test]
fn sm7_a_hint_beyond_eof_falls_back_and_yields_correctly() {
    claims("sm7_a", 4, "сегмент исчез, а кеш живёт");
    let (dir, hint, after, expect) = caught_up_with_valid_hint();
    assert_hint_path_is_live(dir.path(), hint, after, &expect);

    let act = active_segment(dir.path());
    let len = fs::metadata(&act.path).expect("metadata").len();
    assert!(
        hint.pos <= len,
        "SETUP НЕ СОСТОЯЛСЯ: валидный hint уже за концом файла ({} > {len})",
        hint.pos
    );

    // Позиция ЗА концом файла — достижима при усечении или компакции активного сегмента
    // при неизменном индексе, то есть ровно в окне, которое РАСШИРЯЕТ кеш M-62.
    let bad = TailHint {
        pos: len + 4_096,
        ..hint
    };
    let (got, _scanned, err) = yield_with_hint(dir.path(), Some(bad), Some(after));
    assert!(
        err.is_none(),
        "SM-7a: поток вернул ОШИБКУ «{}» вместо fail-safe отката. Недостоверный hint обязан \
         приводить к `header_end`, а не к отказу наружу.",
        err.clone().unwrap_or_default()
    );
    assert_eq!(
        got,
        expect,
        "SM-7a: при `hint.pos` ЗА концом файла (pos={}, len={len}) выдача разошлась с \
         независимым эталоном. `resolve_active_start_offset` проверяет четыре условия и НЕ \
         проверяет длину файла (`segments.rs:1102-1136`): seek за EOF даёт ТИШИНУ — сессия \
         молча перестаёт отдавать события при зелёном healthcheck. Guard обязан отвергнуть \
         недостоверный hint и откатиться в `header_end`, как в прочих четырёх ветках.",
        len + 4_096
    );
}

#[test]
fn sm7_b_hint_inside_frame_falls_back_and_yields_correctly() {
    claims("sm7_b", 4, "сегмент исчез, а кеш живёт");
    let (dir, hint, after, expect) = caught_up_with_valid_hint();
    assert_hint_path_is_live(dir.path(), hint, after, &expect);

    // Позиция ВНУТРИ кадра. Per-frame magic не существует — единственный валидатор границы
    // это CRC (`read_frame_payload`, `segments.rs:472-509`), поэтому guard обязан ПРОБОВАТЬ
    // чтение кадра, а не только сравнивать числа.
    let bad = TailHint {
        pos: hint.pos + 3,
        ..hint
    };
    let (got, _scanned, err) = yield_with_hint(dir.path(), Some(bad), Some(after));
    assert!(
        err.is_none(),
        "SM-7b: позиция в середине кадра дала ОШИБКУ «{}». Именно этот исход и предсказан \
         §7: без валидации границы читатель берёт мусорные байты за длину кадра и падает на \
         санити-капе 64 MiB. Guard обязан отвергнуть позицию и откатиться в `header_end`.",
        err.clone().unwrap_or_default()
    );
    assert_eq!(
        got, expect,
        "SM-7b: при `hint.pos` в СЕРЕДИНЕ кадра (валидная позиция + 3) выдача разошлась с \
         эталоном. Мусорная позиция даёт либо абсурдную длину (cap 64 MiB), либо \
         несовпадение CRC — оба исхода обязаны приводить к fail-safe откату в `header_end`, \
         а не к порче выдачи или к ошибке наружу."
    );
}

#[test]
fn sm7_c_hint_exactly_at_eof_stays_green_without_fallback() {
    claims("sm7_c", 3, "первый тик сессии платит полную цену");

    let dir = tempfile::tempdir().expect("tempdir");
    append_range(dir.path(), 0, 400);
    let all = reference_seqs(dir.path(), None);
    let last = *all.last().expect("события есть");

    let mut s = journal::stream_from_at(dir.path(), EpochFilter::OwnCaptureOnly, None, None)
        .expect("stream_from_at");
    for ev in s.by_ref() {
        ev.expect("event");
    }
    let hint = s.tail_hint().expect("SETUP НЕ СОСТОЯЛСЯ: нет tail_hint");
    let act = active_segment(dir.path());
    let len = fs::metadata(&act.path).expect("metadata").len();
    assert_eq!(
        hint.pos, len,
        "SETUP НЕ СОСТОЯЛСЯ: после полного прохода hint не стоит РОВНО в конце файла \
         ({} против {len}) — легитимный сценарий «догнали хвост» не воспроизведён",
        hint.pos
    );

    // ЛЕГИТИМНЫЙ случай: приращения нет, `pos == len`. Это норма между тиками, а не порча.
    // Наивное `pos >= len ⇒ откат` сломало бы её, и каждый пустой тик платил бы
    // O(сегмента) — M-62 отменил бы сам себя.
    let (got, scanned, err) = yield_with_hint(dir.path(), Some(hint), Some(last));
    assert!(err.is_none(), "SM-7c: легитимный хвост дал ошибку {err:?}");
    assert!(
        got.is_empty(),
        "SM-7c: приращения нет, а выдача не пуста ({} событий)",
        got.len()
    );
    assert!(
        scanned <= BUDGET_SCANNED,
        "SM-7c: пустой тик у хвоста прочитал {scanned} событий при бюджете {BUDGET_SCANNED}. \
         Guard откатился в `header_end` на ЛЕГИТИМНОЙ позиции `pos == len`: каждый пустой \
         тик платит O(сегмента), и milestone отменяет сам себя. Условие обязано быть \
         `pos > len` ⇒ откат, `pos == len` ⇒ законный конец."
    );
}
