//! SACRED (architect-only) — M-40 / риск **R2b**: **писатель обязан ВИДЕТЬ сжатую историю.**
//!
//! ## Тот же корень, что R2, но цена выше: тихая потеря восстановленной истории
//!
//! `latest_segment_index()` (segments.rs:1130) — ТРЕТИЙ независимый энумератор каталога, с
//! тем же слепым фильтром `extension == "jrnl"`. Через него работают `decide_open_segment`
//! (какой сегмент открыть на запись), `resolve_next_seq_with` (с какого `seq` продолжать) и
//! защита «не сжимать активный» в `compact_segment`.
//!
//! ## Замер architect'а на `origin/main` @ 30f5ab0 (не гипотеза)
//!
//! Сценарий — **restore-drill R1**, который founder планирует ~2026-08-10: журнал восстановлен
//! из холодного хранилища (там лежат СЖАТЫЕ сегменты — их и выгружает ретеншен), `journal.meta`
//! в холодном хранилище нет (ретеншен копирует только сегменты), recorder стартует поверх.
//!
//! ```text
//! восстановлено : 5 × .zst, 849 событий, seq 0..848
//! recorder start: создаёт segment-00000000.jrnl  ← КОЛЛИЗИЯ с segment-00000000.jrnl.zst
//! прочитано ПОСЛЕ: 681 событие  (ожидалось 849 + 5 = 854)
//! ```
//!
//! Механика потери: `latest_segment_index` не видит `.zst` ⇒ возвращает `None` ⇒
//! «каталог пуст» ⇒ создаётся `segment-00000000.jrnl`, а `resolve_next_seq_with` берёт
//! `meta_seq = 0` ⇒ новые события получают `seq = 0..4`, ДУБЛИРУЯ уже существующие. Дальше
//! включается D-COMP-1 (при коллизии индекса побеждает СЫРОЙ) — и восстановленный
//! `segment-00000000.jrnl.zst` со 173 событиями становится невидимым для read-пути НАВСЕГДА.
//!
//! Ни одной ошибки при этом не возникает: `open_with` вернул `Ok`, recorder пишет, healthcheck
//! зелёный. Потеря обнаруживается только сверкой количества событий — то есть никогда.
//!
//! ## Почему это обязано лечь ДО R1
//!
//! R1 без restore-drill'а — не бэкап, а надежда. А restore-drill на сегодняшнем коде
//! ЗАКАНЧИВАЕТСЯ ПОРЧЕЙ ровно того, что восстанавливали: первый же старт recorder'а поверх
//! восстановленного каталога затирает начало истории и ломает монотонность `seq`
//! (DET-I-1: `replay(journal) == реальность`).
//!
//! ## Контракт (architect)
//!
//! - **RS-1** Старт recorder'а поверх восстановленной сжатой истории НЕ ТЕРЯЕТ ни одного
//!   события; после старта читается вся история + новые записи.
//! - **RS-2** `seq` остаётся строго возрастающим без дубликатов: новые события продолжают
//!   историю, а не начинают нумерацию заново.
//! - **RS-3** Писатель НЕ СОЗДАЁТ коллизию `segment-N.jrnl` + `segment-N.jrnl.zst`: новый
//!   сегмент получает индекс `max + 1` по ПОЛНОМУ перечислению. (D-COMP-1 — правило чтения
//!   для уже возникшей коллизии, а не лицензия её создавать.)
//! - **RS-4** ПАРНЫЙ vantage: в смешанном каталоге поведение не меняется — recorder
//!   по-прежнему дописывает (reuse) в последний СЫРОЙ сегмент своей эпохи. Фикс не имеет
//!   права «на всякий случай» плодить новые сегменты на каждом рестарте.

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

const T0: i64 = 1_752_000_000_000;
const N: u64 = 900;
const SEG_BYTES: u64 = 8 * 1024;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "restore fixture".to_string(),
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

fn seqs(dir: &std::path::Path) -> Vec<u64> {
    journal::stream(dir, EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event").seq)
        .collect()
}

/// Каталог ровно в том виде, в каком он приезжает из холодного хранилища: ТОЛЬКО сжатые
/// сегменты, без `journal.meta` (ретеншен выгружает сегменты, не мету).
fn restored_from_cold() -> tempfile::TempDir {
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
    for name in ls(src.path()) {
        if name.ends_with(".zst") {
            std::fs::copy(src.path().join(&name), dst.path().join(&name)).expect("copy");
        }
    }
    dst
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RS-1 — ГЛАВНЫЙ: старт поверх восстановленной сжатой истории ничего не теряет
// ═════════════════════════════════════════════════════════════════════════════════════

#[test]
fn rs_1_recorder_start_over_restored_cold_journal_loses_nothing() {
    let dir = restored_from_cold();
    let dir = dir.path();

    // Setup-guard (свойство 3 «целостности гейта»): фикстура обязана ДОКАЗАТЬ, что
    // восстановила сжатую многосегментную историю — иначе тест зелен по пустоте.
    let zst = ls(dir).iter().filter(|n| n.ends_with(".zst")).count();
    let raw = ls(dir).iter().filter(|n| n.ends_with(".jrnl")).count();
    assert!(
        zst >= 4 && raw == 0,
        "фикстура restore не состоялась: .zst={zst} raw={raw} (нужно ≥4 сжатых и НИ ОДНОГО \
         сырого — так выглядит каталог, приехавший из холодного хранилища). Каталог: {:?}",
        ls(dir)
    );
    let before = seqs(dir);
    assert!(
        before.len() >= 800,
        "фикстура restore не состоялась: восстановлено {} событий (ожидалось ~849)",
        before.len()
    );

    // Recorder стартует поверх восстановленного каталога — ровно как на VPS после drill'а.
    const NEW: u64 = 5;
    {
        let mut j = Journal::open_with(dir, cfg()).expect("open_with поверх restore");
        for i in 0..NEW {
            j.append(trade(10_000 + i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let after = seqs(dir);
    assert_eq!(
        after.len(),
        before.len() + NEW as usize,
        "R2b НАРУШЕН: старт recorder'а поверх восстановленной сжатой истории ПОТЕРЯЛ данные.\n\
         ДОЛЖНО БЫТЬ: {} событий (восстановлено {} + дописано {NEW})\n\
         ПОЛУЧЕНО:    {} событий\n\
         Каталог после старта: {:?}\n\
         Писатель не увидел .zst-сегменты, создал segment-00000000.jrnl и по правилу D-COMP-1 \
         (raw побеждает) вытеснил восстановленный сжатый сегмент из read-пути. Ошибки не было \
         НИ ОДНОЙ: open_with вернул Ok, healthcheck зелёный, история молча укорочена.",
        before.len() + NEW as usize,
        before.len(),
        after.len(),
        ls(dir)
    );

    // Вся прежняя история обязана присутствовать поимённо, а не «по количеству».
    for s in &before {
        assert!(
            after.contains(s),
            "seq={s} из восстановленной истории исчез после старта recorder'а"
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RS-2 — seq продолжает историю, а не начинается заново
// ═════════════════════════════════════════════════════════════════════════════════════

/// Дубликат `seq` — это нарушение DET-I-1 и одновременно поломка гейта покрытия чекпоинтом
/// (`covered_through_seq` перестаёт быть точкой на монотонной шкале). Проверяется отдельно
/// от RS-1: реализация может «ничего не потерять», но начать нумерацию с нуля.
#[test]
fn rs_2_seq_continues_history_without_duplicates() {
    let dir = restored_from_cold();
    let dir = dir.path();
    let before = seqs(dir);
    let max_before = *before.iter().max().expect("непустая история");

    {
        let mut j = Journal::open_with(dir, cfg()).expect("open_with");
        for i in 0..5 {
            j.append(trade(10_000 + i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let after = seqs(dir);
    let mut sorted = after.clone();
    sorted.sort_unstable();
    let mut dedup = sorted.clone();
    dedup.dedup();
    assert_eq!(
        sorted.len(),
        dedup.len(),
        "seq ДУБЛИРУЮТСЯ после старта поверх восстановленной истории.\n\
         ДОЛЖНО БЫТЬ: {} уникальных seq\nПОЛУЧЕНО: {} уникальных из {} событий\n\
         Новые записи получили seq, уже занятые восстановленной историей: next_seq считался \
         по journal.meta (которого в холодной копии нет), а сжатые сегменты писатель не увидел.",
        sorted.len(),
        dedup.len(),
        after.len()
    );

    let new_min = after
        .iter()
        .filter(|s| !before.contains(s))
        .min()
        .copied()
        .expect("новые события обязаны существовать");
    assert!(
        new_min > max_before,
        "новые события начались с seq={new_min}, а восстановленная история кончается на \
         seq={max_before}.\nДОЛЖНО БЫТЬ: новый seq > {max_before}\nПОЛУЧЕНО: {new_min}"
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RS-3 — писатель не создаёт коллизию raw + .zst одного индекса
// ═════════════════════════════════════════════════════════════════════════════════════

/// D-COMP-1 — правило ЧТЕНИЯ уже возникшей коллизии (крах-окно компакции между `rename` и
/// `remove`), а не разрешение её создавать. Писатель, порождающий коллизию штатно, делает
/// потерю данных постоянным свойством журнала: состояние не самоизлечивается.
#[test]
fn rs_3_writer_never_creates_raw_zst_index_collision() {
    let dir = restored_from_cold();
    let dir = dir.path();
    let zst_indices: Vec<String> = ls(dir)
        .iter()
        .filter(|n| n.ends_with(".zst"))
        .map(|n| n.trim_end_matches(".zst").to_string())
        .collect();
    assert!(!zst_indices.is_empty(), "фикстура: нужны сжатые сегменты");

    {
        let mut j = Journal::open_with(dir, cfg()).expect("open_with");
        j.append(trade(10_000)).expect("append");
        j.flush().expect("flush");
    }

    let after = ls(dir);
    let collisions: Vec<&String> = zst_indices
        .iter()
        .filter(|base| after.contains(base))
        .collect();
    assert!(
        collisions.is_empty(),
        "писатель создал коллизию индексов raw + .zst: {collisions:?}\n\
         ДОЛЖНО БЫТЬ: новый сегмент получает индекс max+1 по ПОЛНОМУ перечислению каталога\n\
         ПОЛУЧЕНО: каталог {after:?}\n\
         Коллизия постоянна (компакция её не разрешит: сжимать активный запрещено), поэтому \
         сжатый сегмент выпадает из чтения навсегда."
    );
}

// ═════════════════════════════════════════════════════════════════════════════════════
// RS-4 — ПАРНЫЙ vantage: в смешанном каталоге reuse сохраняется
// ═════════════════════════════════════════════════════════════════════════════════════

/// Гвард не имеет права быть переширок. Реализация «увидел .zst — всегда открываю новый
/// сегмент» прошла бы RS-1..RS-3, но плодила бы сегмент на каждый рестарт recorder'а
/// (на проде — рестарт при каждом деплое), дробя журнал и раздувая число файлов.
#[test]
fn rs_4_mixed_directory_still_reuses_last_raw_segment() {
    let dir = tempfile::tempdir().expect("dir");
    let dir = dir.path();
    {
        let mut j = Journal::open_with(dir, cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    // Смесь: старые сжаты, активный + один свежий остались сырыми.
    journal::compact_closed_segments(dir, 2, 3).expect("compact");
    let files_before = ls(dir);
    let segs_before = journal::list_segments(dir).expect("segments").len();
    let events_before = seqs(dir).len();

    // Рестарт recorder'а той же эпохи: дописать в существующий активный сегмент.
    {
        let mut j = Journal::open_with(dir, cfg()).expect("open_with");
        j.append(trade(10_000)).expect("append");
        j.flush().expect("flush");
    }

    assert_eq!(
        journal::list_segments(dir).expect("segments").len(),
        segs_before,
        "рестарт recorder'а в смешанном каталоге создал НОВЫЙ сегмент вместо reuse активного.\n\
         ДОЛЖНО БЫТЬ: {segs_before} сегментов (дописали в активный)\nПОЛУЧЕНО: {}\n\
         Было: {files_before:?}\nСтало: {:?}",
        journal::list_segments(dir).expect("segments").len(),
        ls(dir)
    );
    assert_eq!(
        seqs(dir).len(),
        events_before + 1,
        "смешанный каталог: после дописи ожидалось {} событий, получено {}",
        events_before + 1,
        seqs(dir).len()
    );
}
