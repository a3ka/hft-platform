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

use contracts::{DataSource, EventKind, MdPayload, Side, Venue, LEGACY_EPOCH_ID};
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

/// CT-RFC02-1: боевой сегмент БЕЗ заголовка читается навсегда, с ВМЕНЁННОЙ эпохой
/// `OwnCapture` — ни одно событие не теряется. Переписывать 8.3 GB боевых данных запрещено,
/// поэтому legacy-путь обязан существовать вечно (CT-I-3).
#[test]
fn legacy_segment_without_header_is_read_with_implied_own_capture_epoch() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_legacy_segment(dir.path(), 500);

    let segs = journal::list_segments(dir.path()).expect("segments");
    assert_eq!(segs.len(), 1);
    assert_eq!(
        segs[0].header.source,
        DataSource::OwnCapture,
        "legacy-сегмент — НАШ захват (вменение, а не молчание)"
    );
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
    assert_eq!(evs[0].seq, 0);
    assert_eq!(evs[499].seq, 499);
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
