//! RED M-08 (sacred, architect-only) — legacy-сегмент (CT-RFC02-1) и фильтр эпох
//! (CT-RFC02-2/3/4).
//!
//! Цена ошибки: founder планирует ДОКУПИТЬ историю. Если вендорский сегмент молча
//! подмешается к собственному захвату, альфа обучится на данных, которых у нас никогда
//! не было (чужая глубина книги, чужие часы, чужие гэпы) — и мы узнаем об этом в live.
//! Тот же класс, что TD-015 (несопоставимые эпохи ledger'а), но дороже: там метрики,
//! здесь обучающая выборка.
//!
//! Анти-плацебо: реализация, которая просто читает все сегменты подряд, падает на
//! `default_filter_excludes_vendor_and_synthetic`.

use std::io::Write;

use contracts::{
    DataSource, EventKind, LegacyManifest, LegacySegmentDecl, MdPayload, Side, Venue,
    LEGACY_EPOCH_ID,
};
use journal::{EpochFilter, Journal, WriterConfig};

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + i as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

fn cfg(source: DataSource, epoch: &str) -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1024 * 1024 * 1024,
        min_free_bytes: 0,
        source,
        provenance: format!("fixture {epoch}"),
        epoch_id: epoch.to_string(),
    }
}

/// Дописать события в конец существующего legacy-сегмента (эмуляция живой записи).
fn append_legacy_events(dir: &std::path::Path, from_seq: u64, n: u64) {
    let f = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("segment-00000000.jrnl"))
        .expect("open append");
    let mut w = std::io::BufWriter::new(f);
    for seq in from_seq..from_seq + n {
        let ev = contracts::Event {
            seq,
            ts_mono_ns: seq,
            ts_wall_ms: 1_752_000_000_000 + seq as i64,
            kind: trade(seq),
        };
        let payload = postcard::to_stdvec(&ev).expect("ser");
        w.write_all(&(payload.len() as u32).to_le_bytes()).unwrap();
        w.write_all(&payload).unwrap();
        w.write_all(&crc32fast::hash(&payload).to_le_bytes())
            .unwrap();
    }
    w.flush().unwrap();
}

/// Вариант с другим содержимым (для проверки отпечатка).
fn write_legacy_segment_with_offset(dir: &std::path::Path, n: u64, offset: u64) {
    let path = dir.join("segment-00000000.jrnl");
    let f = std::fs::File::create(path).expect("create");
    let mut w = std::io::BufWriter::new(f);
    for seq in 0..n {
        let ev = contracts::Event {
            seq,
            ts_mono_ns: seq + offset,
            ts_wall_ms: 1_752_000_000_000 + (seq + offset) as i64,
            kind: trade(seq + offset),
        };
        let payload = postcard::to_stdvec(&ev).expect("ser");
        w.write_all(&(payload.len() as u32).to_le_bytes()).unwrap();
        w.write_all(&payload).unwrap();
        w.write_all(&crc32fast::hash(&payload).to_le_bytes())
            .unwrap();
    }
    w.flush().unwrap();
}

/// Записать сегмент СТАРОГО формата (без заголовка) — байт-в-байт как боевой
/// `segment-00000000.jrnl`, который сейчас лежит на VPS (8.3 GB).
fn write_legacy_segment(dir: &std::path::Path, n: u64) {
    let path = dir.join("segment-00000000.jrnl");
    let f = std::fs::File::create(path).expect("create");
    let mut w = std::io::BufWriter::new(f);
    for seq in 0..n {
        let ev = contracts::Event {
            seq,
            ts_mono_ns: seq,
            ts_wall_ms: 1_752_000_000_000 + seq as i64,
            kind: trade(seq),
        };
        let payload = postcard::to_stdvec(&ev).expect("ser");
        w.write_all(&(payload.len() as u32).to_le_bytes()).unwrap();
        w.write_all(&payload).unwrap();
        w.write_all(&crc32fast::hash(&payload).to_le_bytes())
            .unwrap();
    }
    w.flush().unwrap();
    std::fs::write(dir.join("journal.meta"), n.to_le_bytes()).expect("meta");
}

/// Задекларировать legacy-сегмент в манифесте (операторская процедура).
fn declare(dir: &std::path::Path, file: &str, source: DataSource, epoch: &str) {
    let fp = journal::fingerprint(&dir.join(file)).expect("fingerprint");
    let size = std::fs::metadata(dir.join(file)).expect("meta").len();
    let m = LegacyManifest {
        declarations: vec![LegacySegmentDecl {
            file_name: file.to_string(),
            fingerprint_sha256: fp,
            size_bytes_at_decl: size,
            source,
            provenance: format!("declared fixture {epoch}"),
            epoch_id: epoch.to_string(),
        }],
    };
    std::fs::write(
        dir.join(journal::LEGACY_MANIFEST),
        serde_json::to_vec_pretty(&m).expect("ser"),
    )
    .expect("write manifest");
}

/// CT-RFC02-1 (rev 2): боевой сегмент БЕЗ заголовка читается вечно — но ТОЛЬКО будучи ЯВНО
/// задекларированным (магии нет → происхождение берётся из манифеста после сверки отпечатка).
#[test]
fn declared_legacy_segment_is_read_with_declared_epoch() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_legacy_segment(dir.path(), 500);
    declare(
        dir.path(),
        "segment-00000000.jrnl",
        DataSource::OwnCapture,
        LEGACY_EPOCH_ID,
    );

    let segs = journal::list_segments(dir.path()).expect("segments");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].header.source, DataSource::OwnCapture);
    assert_eq!(segs[0].header.epoch_id, LEGACY_EPOCH_ID);
    assert_eq!(
        segs[0].header.schema_version,
        contracts::SCHEMA_VERSION_PRE_HEADER
    );

    let evs: Vec<_> = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event"))
        .collect();
    assert_eq!(evs.len(), 500, "ни одно legacy-событие не потеряно");
    assert_eq!(evs[499].seq, 499);
}

/// **C-005 C2 (fail-closed):** НЕзадекларированный безголовый сегмент НЕ получает нашего
/// происхождения — чтение обязано вернуть Err. Прежнее правило («не разобрался заголовок →
/// OwnCapture») здесь молча приписало бы чужим данным наш захват.
#[test]
fn undeclared_headerless_segment_is_rejected_not_imputed() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_legacy_segment(dir.path(), 100); // манифеста НЕТ

    match journal::list_segments(dir.path()) {
        Err(e) => assert!(
            journal::is_foreign_segment(&e),
            "ожидалась ошибка ForeignSegment, получено: {e}"
        ),
        Ok(segs) => panic!(
            "незадекларированный безголовый сегмент прочитан как {:?} — это ТИХАЯ ПРИПИСКА \
             происхождения (fail-open), ровно то, что CT-RFC-02 обязан исключить",
            segs.first().map(|s| s.header.source)
        ),
    }
}

/// Подмена файла под знакомым именем: декларация есть, но отпечаток НЕ совпадает → Err.
#[test]
fn declared_segment_with_wrong_fingerprint_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_legacy_segment(dir.path(), 100);
    declare(
        dir.path(),
        "segment-00000000.jrnl",
        DataSource::OwnCapture,
        LEGACY_EPOCH_ID,
    );

    // Тот же путь, ДРУГИЕ байты (вендорский дамп подсунут под нашим именем).
    write_legacy_segment_with_offset(dir.path(), 100, 777);

    assert!(
        journal::list_segments(dir.path()).is_err(),
        "отпечаток не совпал → сегмент обязан быть отвергнут (иначе декларация ничего не \
         гарантирует: подменил файл — получил наше происхождение)"
    );
}

/// **R4 (C-005 re-audit):** тот же первый MiB, но файл УСЕЧЁН ниже `size_bytes_at_decl` → Err.
///
/// Отпечатка префикса недостаточно: реализация, сверяющая только первый MiB, примет
/// укороченный (или подменённый с сохранением префикса) файл как наш задекларированный
/// сегмент — и мы молча потеряем хвост ЕДИНСТВЕННОЙ копии 8.3 GB боевых данных.
/// Правило: рост хвоста после декларации — норма; СЖАТИЕ — fail-closed.
#[test]
fn declared_segment_truncated_below_declared_size_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_legacy_segment(dir.path(), 4_000); // достаточно длинный файл
    declare(
        dir.path(),
        "segment-00000000.jrnl",
        DataSource::OwnCapture,
        LEGACY_EPOCH_ID,
    );

    let path = dir.path().join("segment-00000000.jrnl");
    let full = std::fs::metadata(&path).expect("meta").len();

    // Усекаем ХВОСТ, не трогая префикс (отпечаток первого MiB останется тем же).
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open");
    f.set_len(full / 2).expect("truncate");
    drop(f);

    match journal::list_segments(dir.path()) {
        Err(_) => {}
        Ok(segs) => panic!(
            "усечённый сегмент ({} B вместо {} B при декларации) принят как {:?}/{} — \
             реализация сверяет только префикс и молча теряет хвост единственной копии \
             боевых данных",
            full / 2,
            full,
            segs[0].header.source,
            segs[0].header.epoch_id
        ),
    }
}

/// Рост файла ПОСЛЕ декларации — норма (боевой сегмент пишется 24/7): сегмент читается.
#[test]
fn declared_segment_that_grew_after_declaration_is_accepted() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_legacy_segment(dir.path(), 200);
    declare(
        dir.path(),
        "segment-00000000.jrnl",
        DataSource::OwnCapture,
        LEGACY_EPOCH_ID,
    );

    // Дописываем ещё события в тот же файл (recorder продолжает работать).
    append_legacy_events(dir.path(), 200, 100);

    let segs = journal::list_segments(dir.path()).expect(
        "рост хвоста после декларации ДОЛЖЕН приниматься — иначе живой сегмент перестанет \
         читаться сразу после декларации",
    );
    assert_eq!(segs[0].header.source, DataSource::OwnCapture);
    assert_eq!(segs[0].header.epoch_id, LEGACY_EPOCH_ID);
}

/// Битые байты в начале сегмента: ни магии, ни валидных фреймов → Err, а не «наш захват».
#[test]
fn corrupt_segment_is_rejected_not_imputed() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("segment-00000000.jrnl"),
        b"\x00\x01\x02 garbage not a journal at all",
    )
    .expect("write");

    assert!(
        journal::list_segments(dir.path()).is_err(),
        "мусорный файл обязан быть отвергнут, а не классифицирован как OwnCapture"
    );
}

/// Задекларированный ВЕНДОРСКИЙ безголовый дамп остаётся Vendor и не попадает в дефолтную
/// выборку (иначе купленная история молча войдёт в обучение).
#[test]
fn declared_headerless_vendor_stays_vendor() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_legacy_segment(dir.path(), 50);
    declare(
        dir.path(),
        "segment-00000000.jrnl",
        DataSource::Vendor,
        "tardis-2024",
    );

    let segs = journal::list_segments(dir.path()).expect("segments");
    assert_eq!(segs[0].header.source, DataSource::Vendor);

    let n = {
        let mut n = 0usize;
        for e in journal::stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream") {
            e.expect("event");
            n += 1;
        }
        n
    };
    assert_eq!(
        n, 0,
        "вендорский сегмент не смеет попасть в OwnCaptureOnly-выборку"
    );
}

/// CT-RFC02-3/4: по умолчанию (`OwnCaptureOnly`) вендорские и синтетические сегменты
/// В ВЫБОРКУ НЕ ПОПАДАЮТ. Наивная реализация «читаем все сегменты каталога» падает здесь.
#[test]
fn default_filter_excludes_vendor_and_synthetic() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Три сегмента разных эпох в ОДНОМ каталоге (реальный сценарий после докупки истории).
    {
        let mut j = Journal::open_with(dir.path(), cfg(DataSource::OwnCapture, "own-2026-07"))
            .expect("own");
        for i in 0..10 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    {
        let mut j =
            Journal::open_with(dir.path(), cfg(DataSource::Vendor, "vendor-2024")).expect("vendor");
        for i in 0..20 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    {
        let mut j =
            Journal::open_with(dir.path(), cfg(DataSource::Synthetic, "synth-x")).expect("synth");
        for i in 0..30 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let count = |f: EpochFilter| -> usize {
        let mut n = 0usize;
        for e in journal::stream(dir.path(), f).expect("stream") {
            e.expect("event");
            n += 1;
        }
        n
    };

    assert_eq!(
        count(EpochFilter::OwnCaptureOnly),
        10,
        "дефолт обязан отдать ТОЛЬКО собственный захват — вендор/синтетика молча в \
         обучение не попадают"
    );
    assert_eq!(
        count(EpochFilter::Explicit(vec![
            "own-2026-07".to_string(),
            "vendor-2024".to_string()
        ])),
        30,
        "смешение эпох возможно ТОЛЬКО явным перечислением (осознанное решение)"
    );
    assert_eq!(count(EpochFilter::All), 60, "All — для дампов/диагностики");
}

/// CT-RFC02-2: эпоху нельзя не заметить — стрим отдаёт заголовки выбранных сегментов
/// (они обязаны попасть в отчёт: «на каких данных это посчитано»).
#[test]
fn stream_exposes_headers_of_selected_segments() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg(DataSource::OwnCapture, "own-2026-07"))
            .expect("own");
        for i in 0..5 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let s = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream");
    let heads = s.headers();
    assert_eq!(heads.len(), 1);
    assert_eq!(heads[0].epoch_id, "own-2026-07");
    assert!(
        !heads[0].provenance.is_empty(),
        "provenance обязан дойти до потребителя — иначе отчёт не воспроизводим"
    );
}
