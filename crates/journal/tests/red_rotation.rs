//! RED M-08 (sacred, architect-only) — ротация сегментов (E2) + заголовок (CT-I-6).
//!
//! Сегодня имя сегмента ЗАХАРДКОЖЕНО (`journal/src/lib.rs:24`), файл растёт бесконечно:
//! при 2.8 GB/сут (замер VPS 2026-07-13) свободные 120 GB кончатся за ~43 дня — сбор
//! данных ОСТАНОВИТСЯ сам собой. Это прямое нарушение приоритета founder'а №1.
//!
//! Анти-плацебо: реализация, которая «ротирует», но теряет/дублирует `seq` на границе
//! или не пишет заголовок в новый сегмент, здесь падает.

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
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

/// Маленький порог ротации → много сегментов на небольшом потоке.
fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 16 * 1024, // 16 KiB — десятки сегментов на 2000 событий
        min_free_bytes: 0,            // диск-гейт проверяется отдельно (red_retention)
        source: DataSource::OwnCapture,
        provenance: "test recorder v0 (git:deadbeef)".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

const N: u64 = 2_000;

#[test]
fn rotation_produces_multiple_segments_each_with_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let segs = journal::list_segments(dir.path()).expect("segments");
    assert!(
        segs.len() > 1,
        "порог 16 KiB на {N} событиях обязан дать НЕСКОЛЬКО сегментов, получено: {}",
        segs.len()
    );

    // Индексы монотонны и без дыр; каждый сегмент несёт заголовок (CT-I-6).
    for (k, s) in segs.iter().enumerate() {
        assert_eq!(s.index as usize, k, "индексы сегментов по порядку, без дыр");
        assert_eq!(s.header.schema_version, contracts::SCHEMA_VERSION);
        assert_eq!(s.header.source, DataSource::OwnCapture);
        assert_eq!(s.header.epoch_id, "own-test");
        assert!(
            !s.header.provenance.is_empty(),
            "provenance обязан быть записан (CT-RFC-02)"
        );
    }

    // first_seq заголовков строго возрастает — сшивка сегментов честная.
    for w in segs.windows(2) {
        assert!(
            w[1].header.first_seq > w[0].header.first_seq,
            "first_seq следующего сегмента обязан быть больше предыдущего"
        );
    }
}

/// `seq` — тотальный порядок ЖУРНАЛА, не сегмента: сквозной, монотонный, без дыр и дублей
/// на границах сегментов. Потеря/дубль события на ротации = порванный реплей (DET-I-1).
#[test]
fn seq_is_continuous_across_segment_boundaries() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let evs: Vec<_> = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event"))
        .collect();

    assert_eq!(
        evs.len() as u64,
        N,
        "ни одно событие не потеряно и не продублировано на границах сегментов"
    );
    for (k, e) in evs.iter().enumerate() {
        assert_eq!(
            e.seq, k as u64,
            "seq сквозной и монотонный через все сегменты"
        );
    }
}

/// Рестарт писателя: новый сегмент, но `seq` НЕ переиспользуется (JR-I-1 / TD-011-семантика).
#[test]
fn restart_continues_seq_and_opens_fresh_segment() {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..100 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    let next_after_first = {
        let j = Journal::open_with(dir.path(), cfg()).expect("reopen");
        j.next_seq()
    };
    assert_eq!(next_after_first, 100, "next_seq переживает рестарт");

    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("reopen");
        for i in 100..200 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let evs: Vec<_> = journal::stream(dir.path(), EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event"))
        .collect();
    assert_eq!(evs.len(), 200);
    let seqs: Vec<u64> = evs.iter().map(|e| e.seq).collect();
    let mut uniq = seqs.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(
        uniq.len(),
        seqs.len(),
        "seq НЕ переиспользуется после рестарта (дублей быть не может)"
    );
}
