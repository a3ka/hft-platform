//! SACRED (architect-only) — M-49 / **TD-049: честность `next_seq`**.
//!
//! ## Дефект (замерен architect'ом на main @ ddc105c, ПОСЛЕ мержа M-40)
//!
//! `resolve_next_seq_with` берёт `tail_last_seq_of(последний сегмент)` и при `Ok(None)`
//! молча падает на `meta_seq`. Но `tail_last_seq_of` возвращает `Ok(None)` в ВОСЬМИ
//! случаях, из которых легитимны только два (файла нет; файл нулевой длины). Остальные —
//! порча: битый заголовок сжатого сегмента, обрыв zstd-потока, полностью невалидный хвост
//! сырого сегмента. Все они трактуются вызывающим как «сегментов нет, начинай с меты».
//!
//! **При restore из холодного хранилища `journal.meta` ОТСУТСТВУЕТ** (ретеншен выгружает
//! сегменты, не мету — замерено в M-40), поэтому `meta_seq = 0`. Замер:
//!
//! ```text
//! история 0..341 в сжатых сегментах
//! усечённый .zst      → open_with = Ok, новые seq [0, 1]  ← перекрытие 342 событий
//! битый v2-заголовок  → open_with = Ok, новый seq  [0]    ← то же
//! сырой битый хвост   → open_with = Ok, seq 342           ← КОРРЕКТНО (ресинк нашёл 341)
//! ```
//!
//! Это ровно катастрофа R2b, которую M-40 закрыл для ЧИТАЕМОГО `.zst` и которая осталась
//! жить на пути порчи. Триггер — restore-drill R1, запланированный founder'ом (~2026-08-10):
//! частичная выкачка или bit-rot даёт испорченный журнал БЕЗ ЕДИНОЙ ОШИБКИ в логе.
//!
//! **Уточнение к TD-049.** Reviewer описал дефект как «recorder не стартует» (асимметрия
//! терпимости raw/zst). Замер это ОПРОВЕРГ: recorder стартует в обоих случаях порчи.
//! Реальное последствие хуже отказа — молчаливый seq-reuse. Severity: MINOR → **CRITICAL
//! по последствию** (необратимая порча журнала, необнаружимая в моменте).
//!
//! ## Контракт (architect, RED-first)
//!
//! **JR-I-8 (честность `next_seq`).** `open_with` не имеет права начать запись с `seq`,
//! который меньше либо равен максимальному `seq`, уже присутствующему в журнале. Если
//! сегмент с максимальным индексом СУЩЕСТВУЕТ и НЕ ПУСТ, но его последний `seq` установить
//! НЕ УДАЛОСЬ — `open_with` обязан вернуть `Err` с диагностикой (имя файла + причина),
//! а не стартовать с `meta_seq`.
//!
//! Легитимных `Ok(None)` ровно два: (1) файла нет; (2) файл нулевой длины.
//!
//! **Почему fail-closed, а не терпимость.** Терпимость означает запись поверх существующего
//! диапазона `seq` — необратимую порчу append-only журнала, которую реплей воспроизведёт
//! бит-в-бит. Отказ старта громкий и исправимый: оператор перекачивает сегмент из холодного
//! хранилища либо (крайний случай) объявляет `next_seq` явно — см.
//! `red_tail_integrity_operator.rs`. Сбор данных стоит минуты, а не теряется навсегда.
//!
//! **Что НЕ ужесточается.** Ресинхронизация сырого сегмента (побайтовый поиск последнего
//! ВАЛИДНОГО фрейма) сохраняется: она возвращает конкретный `seq`, то есть позиция
//! известна и reuse невозможен. Ломать работающее поведение не нужно — оракул `ti_4`
//! это пиннит парным vantage.

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

const T0: i64 = 1_752_000_000_000;
const N: u64 = 400;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 8 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "tail-integrity fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + i as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: T0 + i as i64,
        },
    )
}

fn ls(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

/// Каталог ровно в том виде, в каком он приезжает из холодного хранилища: ТОЛЬКО сжатые
/// сегменты, БЕЗ `journal.meta` (ретеншен выгружает сегменты, не мету — замер M-40).
/// Возвращает каталог и максимальный `seq` восстановленной истории.
fn restored_from_cold() -> (tempfile::TempDir, u64) {
    let src = tempfile::tempdir().expect("src");
    {
        let mut j = Journal::open_with(src.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(src.path(), 0, 3).expect("compact");

    let dst = tempfile::tempdir().expect("dst");
    for n in ls(src.path()) {
        if n.ends_with(".zst") {
            std::fs::copy(src.path().join(&n), dst.path().join(&n)).expect("copy");
        }
    }
    let max_seq = journal::stream(dst.path(), EpochFilter::All)
        .expect("stream")
        .filter_map(|e| e.ok())
        .map(|e| e.seq)
        .max()
        .expect("непустая восстановленная история");
    assert!(
        !dst.path().join("journal.meta").exists(),
        "фикстура не состоялась: journal.meta не должна существовать (прод-режим restore)"
    );
    (dst, max_seq)
}

fn zst_names(dir: &std::path::Path) -> Vec<String> {
    ls(dir)
        .into_iter()
        .filter(|n| n.ends_with(".zst"))
        .collect()
}

/// Максимальный `seq`, который вообще можно прочитать из каталога (по ЧИТАЕМЫМ сегментам).
/// Нужен, чтобы доказать перекрытие: новый seq не должен быть ≤ этого значения.
fn readable_max_seq(dir: &std::path::Path) -> Option<u64> {
    journal::stream(dir, EpochFilter::All)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.seq)
        .max()
}

/// Диагностика для сообщений об ошибке: что реально лежит в каталоге.
fn layout(dir: &std::path::Path) -> String {
    format!("{:?}", ls(dir))
}

// ═════════════════════════════════════════════════════════════════════════════════════
// TI-1 — ГЛАВНЫЙ: усечённый сжатый хвост НЕ ИМЕЕТ ПРАВА дать старт с meta_seq
// ═════════════════════════════════════════════════════════════════════════════════════

/// Прод-триггер: восстановление из холодного хранилища с ОБРЫВОМ ВЫКАЧКИ. Сегмент с
/// максимальным индексом усечён; сырого двойника нет (в крах-окне компакции сырой жив и
/// побеждает по D-COMP-1, поэтому там этот путь не задействуется).
///
/// Замер до фикса: `open_with = Ok`, новые seq `[0, 1]` при истории `0..341`.
#[test]
fn ti_1_truncated_compacted_tail_must_not_start_from_meta() {
    let (dir, history_max) = restored_from_cold();
    let victim = zst_names(dir.path())
        .pop()
        .expect("нужен хотя бы один сжатый сегмент");
    let p = dir.path().join(&victim);
    let bytes = std::fs::read(&p).expect("read");
    let cut = bytes.len() * 2 / 3;
    std::fs::write(&p, &bytes[..cut]).expect("truncate");
    assert!(
        cut > 0 && cut < bytes.len(),
        "фикстура не состоялась: усечение не произошло ({} → {cut})",
        bytes.len()
    );

    let result = Journal::open_with(dir.path(), cfg());

    // Если реализация всё же стартовала — доказываем перекрытие поимённо, а не «на глаз».
    if let Ok(mut j) = result {
        j.append(trade(9_999)).expect("append");
        j.flush().expect("flush");
        drop(j);
        // Битый .zst убираем, чтобы прочитать ТОЛЬКО то, что записал писатель.
        let q = dir.path().join("quarantine");
        std::fs::create_dir_all(&q).expect("mkdir");
        for n in zst_names(dir.path()) {
            std::fs::rename(dir.path().join(&n), q.join(&n)).expect("move");
        }
        let written: Vec<u64> = journal::stream(dir.path(), EpochFilter::All)
            .map(|s| s.filter_map(|e| e.ok()).map(|e| e.seq).collect())
            .unwrap_or_default();
        panic!(
            "JR-I-8 НАРУШЕН: усечённый сжатый хвост дал СТАРТ вместо отказа.\n\
             ДОЛЖНО БЫТЬ: open_with = Err (хвост {victim} нечитаем ⇒ последний seq не \
             установлен ⇒ стартовать нельзя)\n\
             ПОЛУЧЕНО: open_with = Ok, писатель записал seq {written:?} при истории 0..{history_max}\n\
             Перекрытие: {} событий переписаны поверх существующих.\n\
             Причина: tail_last_seq_of вернул Ok(None) на порче, resolve_next_seq_with принял \
             это за «сегментов нет» и взял meta_seq; journal.meta при restore ОТСУТСТВУЕТ ⇒ 0.\n\
             Журнал append-only: реплей воспроизведёт порчу бит-в-бит, откатить нечем.",
            written
                .iter()
                .filter(|s| **s <= history_max)
                .count()
                .max(1)
        );
    }

    let err = result.err().expect("проверено выше: Ok обработан panic'ом");
    let msg = err.to_string();
    assert!(
        msg.contains(&victim) || msg.contains("segment"),
        "отказ обязан НАЗЫВАТЬ проблемный файл — иначе оператор не знает, что перекачивать.\n\
         ДОЛЖНО БЫТЬ: сообщение содержит «{victim}»\nПОЛУЧЕНО: «{msg}»\nКаталог: {}",
        layout(dir.path())
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// TI-2 — битый v2-заголовок внутри валидного zstd: та же дыра, другой вход
// ═════════════════════════════════════════════════════════════════════════════════════

/// Отдельный класс от TI-1: zstd-поток ЦЕЛ (распаковывается), но внутри не наш сегмент —
/// bit-rot в заголовке, подмена файла, обрыв на этапе компакции. В коде это ветка
/// `skip_v2_header_forward(...).is_err() → Ok(None)`, то есть ТИХИЙ путь к meta_seq на
/// строке ВЫШЕ той, что описал reviewer.
///
/// Замер до фикса: `open_with = Ok`, новый seq `[0]`.
#[test]
fn ti_2_corrupt_v2_header_in_valid_zstd_must_not_start_from_meta() {
    let (dir, history_max) = restored_from_cold();
    let victim = zst_names(dir.path()).pop().expect("нужен .zst");
    let p = dir.path().join(&victim);
    let payload = b"valid zstd stream, but definitely not a v2 journal segment: no magic";
    std::fs::write(&p, zstd::encode_all(&payload[..], 3).expect("encode")).expect("write");

    // Setup-guard: zstd обязан оставаться ВАЛИДНЫМ (иначе это TI-1, другой класс).
    let f = std::fs::File::open(&p).expect("open");
    assert!(
        zstd::stream::decode_all(f).is_ok(),
        "фикстура не состоялась: zstd-поток должен распаковываться — проверяется порча \
         ЗАГОЛОВКА, а не потока"
    );

    match Journal::open_with(dir.path(), cfg()) {
        Ok(mut j) => {
            j.append(trade(9_999)).expect("append");
            j.flush().expect("flush");
            panic!(
                "JR-I-8 НАРУШЕН: сегмент с нечитаемым v2-заголовком дал СТАРТ.\n\
                 ДОЛЖНО БЫТЬ: open_with = Err (заголовок {victim} не распознан ⇒ последний \
                 seq неизвестен)\n\
                 ПОЛУЧЕНО: open_with = Ok при истории 0..{history_max} ⇒ запись пошла с \
                 meta_seq (при restore = 0) поверх существующего диапазона.\n\
                 Ветка `skip_v2_header_forward(..).is_err() → Ok(None)` возвращает «сегментов \
                 нет» вместо «сегмент есть, но нечитаем» — это разные утверждения."
            );
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(&victim) || msg.contains("header") || msg.contains("segment"),
                "отказ обязан называть файл или причину.\nПОЛУЧЕНО: «{msg}»"
            );
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// TI-3 — сырой сегмент, где НИ ОДИН фрейм не валиден
// ═════════════════════════════════════════════════════════════════════════════════════

/// Симметрия с TI-1/TI-2 для СЫРОГО формата: ресинхронизация не нашла ни одного валидного
/// фрейма (`last_valid_seq = None`) при НЕПУСТОМ файле — это «нечитаем», а не «пусто».
/// Мета удалена (прод-режим restore), поэтому старт означал бы запись с нуля.
#[test]
fn ti_3_raw_segment_with_no_valid_frame_must_not_start_from_meta() {
    let dir = tempfile::tempdir().expect("dir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let _ = std::fs::remove_file(dir.path().join("journal.meta"));
    let history_max = readable_max_seq(dir.path()).expect("история читается");

    // Портим ВСЁ содержимое последнего сегмента после магии+заголовка: ни одного валидного
    // фрейма не остаётся, но файл непуст и заголовок цел.
    let last = ls(dir.path())
        .into_iter()
        .rfind(|n| n.ends_with(".jrnl"))
        .expect("есть сырой сегмент");
    let p = dir.path().join(&last);
    let mut bytes = std::fs::read(&p).expect("read");
    // Граница заголовка вычисляется из САМОГО файла: magic(8) + len(4) + payload(h_len) +
    // crc(4). Иначе (как в первой редакции — порча с фиксированного 256-го байта) события
    // между заголовком и точкой порчи остаются валидными, ресинк их находит, и оракул
    // проверяет НЕ ТО, что заявлено. Поймано прогоном против прототипа.
    let h_len = u32::from_le_bytes(bytes[8..12].try_into().expect("header len")) as usize;
    let header_end = 8 + 4 + h_len + 4;
    assert!(
        header_end < 2048 && header_end < bytes.len(),
        "фикстура не состоялась: граница заголовка {header_end} неправдоподобна (файл {} B) — \
         формат сегмента изменился, оракул нужно переспецифицировать",
        bytes.len()
    );
    for b in bytes[header_end..].iter_mut() {
        *b = 0x5A; // ни одного валидного фрейма после заголовка
    }
    std::fs::write(&p, &bytes).expect("write");
    assert!(
        std::fs::metadata(&p).expect("meta").len() > header_end as u64,
        "фикстура: после заголовка обязаны быть байты (иначе это случай «нет событий», ti_4д)"
    );

    match Journal::open_with(dir.path(), cfg()) {
        Ok(mut j) => {
            j.append(trade(9_999)).expect("append");
            j.flush().expect("flush");
            drop(j);
            let written = readable_max_seq(dir.path()).unwrap_or(0);
            assert!(
                written > history_max,
                "JR-I-8 НАРУШЕН: сырой сегмент без единого валидного фрейма дал старт с \
                 meta_seq.\nДОЛЖНО БЫТЬ: open_with = Err (файл {last} непуст, но последний \
                 seq не установлен)\nПОЛУЧЕНО: старт прошёл, максимальный seq после записи \
                 {written} при истории до {history_max} — запись легла в уже занятый диапазон."
            );
        }
        Err(e) => {
            assert!(
                e.to_string().contains(&last) || e.to_string().contains("segment"),
                "отказ обязан называть файл: «{}»",
                e
            );
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// TI-4 — ПАРНЫЙ vantage: легитимные пути НЕ ломаются (гвард не переширок)
// ═════════════════════════════════════════════════════════════════════════════════════

/// Ужесточение обязано быть точным. Четыре штатных случая должны продолжать работать,
/// иначе fail-closed превратится в «recorder не стартует никогда»:
///   (а) пустой каталог — первый запуск;
///   (б) файл сегмента нулевой длины — сегмент только создан;
///   (в) валидный сжатый хвост — сценарий, закрытый M-40 (продолжение истории);
///   (г) сырой сегмент с ЧАСТИЧНО битым хвостом — ресинк находит последний валидный seq.
#[test]
fn ti_4_legitimate_paths_still_start() {
    // (а) пустой каталог
    {
        let dir = tempfile::tempdir().expect("dir");
        Journal::open_with(dir.path(), cfg()).expect("(а) пустой каталог обязан стартовать");
    }

    // (б) сегмент нулевой длины
    {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("segment-00000000.jrnl"), b"").expect("write empty");
        Journal::open_with(dir.path(), cfg())
            .expect("(б) сегмент нулевой длины — легитимный Ok(None), старт обязан пройти");
    }

    // (в) валидный сжатый хвост: история продолжается, seq НЕ переиспользуется
    {
        let (dir, history_max) = restored_from_cold();
        let mut j = Journal::open_with(dir.path(), cfg())
            .expect("(в) валидный .zst-хвост обязан стартовать (сценарий M-40)");
        j.append(trade(9_999)).expect("append");
        j.flush().expect("flush");
        drop(j);
        let after = readable_max_seq(dir.path()).expect("читается");
        assert!(
            after > history_max,
            "(в) seq обязан продолжать историю: было {history_max}, стало {after}"
        );
    }

    // (д) сегмент с ВАЛИДНЫМ заголовком и НУЛЁМ событий — recorder создал сегмент и упал
    //     до первой записи. Файл НЕПУСТ (магия + заголовок), но событий нет: это
    //     легитимное «последнего seq нет», а не «хвост нечитаем». Случай найден
    //     architect'ом при вычитке собственного оракула: без него ужесточение TI-3
    //     сломало бы штатный перезапуск recorder'а.
    {
        let dir = tempfile::tempdir().expect("dir");
        {
            let j = Journal::open_with(dir.path(), cfg()).expect("создать сегмент");
            drop(j); // ни одного append
        }
        let _ = std::fs::remove_file(dir.path().join("journal.meta"));
        let seg = ls(dir.path())
            .into_iter()
            .find(|n| n.ends_with(".jrnl"))
            .expect("сегмент создан");
        let size = std::fs::metadata(dir.path().join(&seg))
            .expect("meta")
            .len();
        assert!(
            size > 0,
            "фикстура (д) не состоялась: сегмент {seg} должен содержать заголовок (size>0)"
        );
        Journal::open_with(dir.path(), cfg()).unwrap_or_else(|e| {
            panic!(
                "(д) сегмент с валидным заголовком и нулём событий обязан стартовать: \
                 «нет событий» ≠ «хвост нечитаем». Файл {seg} ({size} B). Ошибка: {e}"
            )
        });
    }

    // (г) сырой сегмент с частично битым хвостом: ресинк даёт валидный seq
    {
        let dir = tempfile::tempdir().expect("dir");
        {
            let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
            for i in 0..N {
                j.append(trade(i)).expect("append");
            }
            j.flush().expect("flush");
        }
        let _ = std::fs::remove_file(dir.path().join("journal.meta"));
        let last = ls(dir.path())
            .into_iter()
            .rfind(|n| n.ends_with(".jrnl"))
            .expect("сырой сегмент");
        let p = dir.path().join(&last);
        let mut bytes = std::fs::read(&p).expect("read");
        let n = bytes.len();
        // Портим ТОЛЬКО последние байты — валидные фреймы перед ними остаются.
        for b in bytes[n.saturating_sub(40)..].iter_mut() {
            *b ^= 0xFF;
        }
        std::fs::write(&p, &bytes).expect("write");

        let mut j = Journal::open_with(dir.path(), cfg()).expect(
            "(г) частично битый сырой хвост НЕ должен блокировать старт: ресинхронизация \
             находит последний валидный seq, позиция известна, reuse невозможен",
        );
        j.append(trade(9_999)).expect("append");
        j.flush().expect("flush");
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// TI-5 — отказ обязан быть ДИАГНОСТИЧНЫМ, иначе fail-closed бесполезен
// ═════════════════════════════════════════════════════════════════════════════════════

/// Оператор видит только «recorder не поднялся». Если ошибка не называет файл и причину,
/// он не знает, что перекачивать, и единственный доступный ему шаг — удалить каталог.
/// Требование: сообщение содержит (1) имя проблемного файла, (2) внятную причину,
/// (3) указание, что делать (или ссылку на runbook).
#[test]
fn ti_5_refusal_is_diagnostic() {
    let (dir, _) = restored_from_cold();
    let victim = zst_names(dir.path()).pop().expect("нужен .zst");
    let p = dir.path().join(&victim);
    let bytes = std::fs::read(&p).expect("read");
    std::fs::write(&p, &bytes[..bytes.len() / 2]).expect("truncate");

    let err = Journal::open_with(dir.path(), cfg())
        .err()
        .expect("предусловие TI-1: усечённый сжатый хвост обязан давать Err");
    let msg = err.to_string();

    assert!(
        msg.contains(&victim),
        "сообщение обязано называть ФАЙЛ.\nДОЛЖНО содержать: «{victim}»\nПОЛУЧЕНО: «{msg}»"
    );
    let has_reason = ["seq", "tail", "хвост", "нечитаем", "corrupt", "truncat"]
        .iter()
        .any(|k| msg.to_lowercase().contains(k));
    assert!(
        has_reason,
        "сообщение обязано называть ПРИЧИНУ (последний seq не установлен / хвост нечитаем).\n\
         ПОЛУЧЕНО: «{msg}»"
    );
    let has_action = [
        "runbook",
        "перекач",
        "восстанов",
        "declare",
        "force",
        "объяв",
    ]
    .iter()
    .any(|k| msg.to_lowercase().contains(k));
    assert!(
        has_action,
        "сообщение обязано подсказывать ДЕЙСТВИЕ (перекачать сегмент / объявить next_seq / \
         см. runbook) — иначе оператор в тупике и удалит каталог.\nПОЛУЧЕНО: «{msg}»"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// TI-6 (rev2) — СТРОГИЙ ПУТЬ ЧТЕНИЯ ОСТАЁТСЯ СТРОГИМ
// ═════════════════════════════════════════════════════════════════════════════════════

/// **Добавлено в rev2 после REJECT'а PR-гейта M-49.** Реализация rev1 прошла все оракулы,
/// ослабив `segments()`: повреждённый хвостовой сегмент стал молча пропускаться. Формально
/// оракулы были зелёными — но ценой соседнего инварианта, и поймал это только человеческий
/// взгляд reviewer'а. Теперь ловит тест.
///
/// **Почему это недопустимо.** `segments()` питает `list_segments` и `stream` — прод-путь
/// чтения для research, бэктеста и кокпита. Молчаливый пропуск означает, что потребитель
/// получает журнал С ДЫРОЙ и не узнаёт об этом: ни ошибки, ни метки. Это тот самый класс
/// «тихой лжи в данных», против которого построена вся дисциплина проекта (JR-I: отказ
/// вместо правдоподобного ответа).
///
/// **Разделение путей (уже существует в крейте, изобретать не нужно):**
/// - `segments()` / `stream()` — СТРОГИЕ, fail-closed: для потребителей данных;
/// - `iter_segments_sorted()` / `recover()` — ТЕРПИМЫЕ, диагностические: для офлайн-работы.
///
/// Внутренняя нужда M-49 («каков максимальный читаемый seq» для валидации операторской
/// декларации) обслуживается ТЕРПИМЫМ путём внутри крейта, а не послаблением строгого.
#[test]
fn ti_6_strict_read_path_stays_strict_on_corrupt_segment() {
    let (dir, _) = restored_from_cold();
    let victim = zst_names(dir.path()).pop().expect("нужен .zst");
    let p = dir.path().join(&victim);
    let bytes = std::fs::read(&p).expect("read");
    std::fs::write(&p, &bytes[..bytes.len() * 2 / 3]).expect("truncate");

    // Setup-guard: каталог обязан быть ИМЕННО повреждённым, иначе оракул проверяет не то.
    assert!(
        std::fs::metadata(&p).expect("meta").len() < bytes.len() as u64,
        "фикстура не состоялась: усечение не произошло"
    );

    // (а) list_segments — строгий путь: обязан ОТКАЗАТЬ, а не вернуть неполный список.
    let listed = journal::list_segments(dir.path());
    assert!(
        listed.is_err(),
        "СТРОГИЙ ПУТЬ ОСЛАБЛЕН: list_segments вернул Ok при повреждённом сегменте {victim}.\n\
         ДОЛЖНО БЫТЬ: Err (потребитель обязан узнать о повреждении)\n\
         ПОЛУЧЕНО: Ok со списком из {} сегментов — повреждённый молча пропущен.\n\
         Это «тихая ложь в данных»: research и бэктест получат журнал с дырой и не заметят. \
         Внутренняя нужда M-49 (максимальный читаемый seq для валидации декларации) \
         обслуживается ТЕРПИМЫМ путём внутри крейта (iter_segments_sorted-класс), а НЕ \
         послаблением строгого. Именно за это PR-гейт завернул rev1.",
        listed.as_ref().map(|v| v.len()).unwrap_or(0)
    );

    // (б) stream — тот же контракт: отказ, а не молчаливо укороченный поток.
    let streamed = journal::stream(dir.path(), EpochFilter::All);
    let silently_truncated = match streamed {
        Err(_) => false,
        Ok(it) => {
            // Поток открылся — значит либо он честно упадёт на повреждённом сегменте
            // (Err в итерации — допустимо), либо молча его пропустит (недопустимо).
            let mut saw_err = false;
            for e in it {
                if e.is_err() {
                    saw_err = true;
                    break;
                }
            }
            !saw_err
        }
    };
    assert!(
        !silently_truncated,
        "СТРОГИЙ ПУТЬ ОСЛАБЛЕН: stream дочитал журнал до конца БЕЗ ошибки, хотя сегмент \
         {victim} повреждён.\nДОЛЖНО БЫТЬ: Err при открытии ИЛИ Err в итерации на \
         повреждённом сегменте\nПОЛУЧЕНО: полный проход без единой ошибки — потребитель \
         считает, что прочитал всё, а часть истории отсутствует."
    );
}
