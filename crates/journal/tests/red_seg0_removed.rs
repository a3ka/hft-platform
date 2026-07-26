//! RED/guard M-36 (sacred, architect-only) — journal ТЕРПИТ удаление нижнего сегмента.
//!
//! Контекст TD-038: на проде legacy `segment-00000000.jrnl` (15GB, headerless, задекларирован в
//! `journal.legacy.json`) содержит битый фрейм (~193.7 MiB, crc-поле обрывок) → strict-чтение
//! (`read_all`/`stream`) падает `frame crc mismatch`, gateway::snapshot нерабочий. Founder-решение
//! M-36: физически удалить legacy (файл + запись манифеста). ПЕРЕД необратимой операцией на проде
//! этот guard доказывает, что enumeration/seq-континуити журнала терпят ОТСУТСТВИЕ нижнего сегмента
//! (индексация стартует не с 0, дыра в начале).
//!
//! После purge на проде все оставшиеся сегменты headered (v2: компактированные .zst + активные raw),
//! headerless-файла нет → путь «undeclared legacy» (segments.rs) не активируется. Эта фикстура
//! моделирует ПОСТ-purge состояние: все сегменты headered own-capture, нижний удалён.
//!
//! Анти-плацебо: если enumeration предполагает contiguous-from-0 ИЛИ meta.next_seq ломает чтение
//! при дыре в начале — тест ПАДАЕТ (что и нужно поймать ДО прода). Ожидание: read_all + stream
//! возвращают РОВНО события выживших сегментов, в порядке, без ошибки.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use journal::{read_all, stream, EpochFilter, Journal, WriterConfig};

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 10, // 1 KiB → частая ротация, гарантированно ≥3 сегмента
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(100.0 + i as f64),
            size: to_fixed(1.0),
            side: Side::Buy,
            ts_exch_ms: 1_752_000_000_000 + i * 1000,
        },
    )
}

#[test]
fn seg0_removal_tolerated_remaining_segments_read_clean() {
    let dir = tempfile::tempdir().expect("tempdir");
    const N: i64 = 200;

    // 1) записать N событий с частой ротацией → несколько сегментов (0,1,2,...).
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    // 2) убедиться, что ротация действительно создала ≥3 сегмента и seg0 существует.
    let seg0 = dir.path().join("segment-00000000.jrnl");
    let seg_count = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("segment-") && n.contains(".jrnl"))
                .unwrap_or(false)
        })
        .count();
    assert!(
        seg_count >= 3,
        "фикстура должна дать ≥3 сегмента (получили {seg_count}); уменьши max_segment_bytes"
    );
    assert!(seg0.exists(), "segment-00000000.jrnl должен существовать до удаления");

    // baseline: полный журнал читается, N событий.
    let full = read_all(dir.path()).expect("read_all baseline");
    assert_eq!(full.len(), N as usize, "baseline: все N событий");

    // 3) МОДЕЛЬ ПРОД-PURGE: удалить нижний сегмент (аналог legacy seg0). Манифеста нет
    //    (fresh own-capture), так что чистим только файл.
    std::fs::remove_file(&seg0).expect("remove seg0");

    // 4) ТРЕБОВАНИЕ: read_all и stream(OwnCaptureOnly) читают ОСТАВШИЕСЯ сегменты БЕЗ ошибки.
    let after = read_all(dir.path())
        .expect("read_all после удаления seg0 обязан работать (дыра в начале индексации)");
    assert!(
        !after.is_empty() && after.len() < N as usize,
        "должны выжить события сегментов ≥1 (0 < {} < {N})",
        after.len()
    );

    let mut streamed = 0usize;
    for r in stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream открылся") {
        r.expect("stream после удаления seg0 обязан читать без crc/continuity ошибки");
        streamed += 1;
    }
    assert_eq!(
        streamed,
        after.len(),
        "stream и read_all обязаны видеть одинаковое число выживших событий"
    );
}
