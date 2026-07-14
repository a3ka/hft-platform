//! SACRED (architect-only) — TD-022: компакция ЗАКРЫТЫХ сегментов (M-08 task 15).
//!
//! **Повод (замеры на боевом проде, 2026-07-14):** журнал растёт **8.83 GB/сут** (в документах
//! стояло 2.8 — цифра до включения фьючерсов в M-06; по ней же принималось решение «Storage Box
//! через 30 дней»). Свободно 118.7 GB, disk-guard при 10 GiB ⇒ **12 дней**, а не 40.
//! zstd на боевом сегменте: **-3 → 9.1×** ⇒ рост на диске 8.83 → ~1 GB/сут, запас → 100+ дней.
//!
//! **Почему безопасно:** закрытый сегмент неизменяем (recorder пишет только в активный).
//!
//! Инвариант, ради которого всё: **сжатие НИКОГДА не теряет данные.** Оригинал удаляется
//! только после того, как сжатая копия РАСПАКОВАНА и сверена (тот же принцип, что
//! `ColdCopyProof`). Данные незаменимы; «сжали и удалили, а там мусор» недопустимо.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{DataSource, EventKind, Level, MdPayload, Venue};
use journal::{EpochFilter, Journal, WriterConfig, DEFAULT_COMPACT_LEVEL};

static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), SeqCst) + l.size();
            PEAK.fetch_max(c, SeqCst);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        CUR.fetch_sub(l.size(), SeqCst);
    }
}
#[global_allocator]
static GA: Counting = Counting;

fn peak_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = CUR.load(SeqCst);
    PEAK.store(base, SeqCst);
    let r = f();
    (r, PEAK.load(SeqCst).saturating_sub(base))
}

fn snapshot(i: u64) -> EventKind {
    let lvl = |k: i64| Level {
        price: 6_400_000_000_000 + k * 100 + i as i64,
        size: 1_000 + k,
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: (0..60).map(lvl).collect(),
            asks: (0..60).map(lvl).collect(),
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

fn cfg(max_seg: u64) -> WriterConfig {
    WriterConfig {
        max_segment_bytes: max_seg,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "compaction fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn build(dir: &std::path::Path, n: u64, max_seg: u64) {
    let mut j = Journal::open_with(dir, cfg(max_seg)).expect("open_with");
    for i in 0..n {
        j.append(snapshot(i)).expect("append");
    }
    j.flush().expect("flush");
}

fn all_events(dir: &std::path::Path) -> Vec<contracts::Event> {
    journal::stream(dir, EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event"))
        .collect()
}

/// C1 (ГЛАВНОЕ): сжатие НЕ ТЕРЯЕТ и НЕ ИСКАЖАЕТ данные — поток событий бит-в-бит тот же
/// (DET-I-1: `replay(journal) == реальность` не может зависеть от того, сжат сегмент или нет).
#[test]
fn c1_compaction_preserves_events_exactly() {
    let dir = tempfile::tempdir().expect("dir");
    build(dir.path(), 3_000, 128 * 1024);
    let before = all_events(dir.path());
    assert!(before.len() == 3_000, "предусловие: все события на месте");

    let segs = journal::list_segments(dir.path()).expect("segs");
    assert!(segs.len() > 2, "предусловие: несколько сегментов");

    let reports = journal::compact_closed_segments(dir.path(), 0, DEFAULT_COMPACT_LEVEL)
        .expect("compact_closed_segments");
    assert!(!reports.is_empty(), "закрытые сегменты обязаны сжаться");
    for r in &reports {
        assert!(
            r.bytes_after < r.bytes_before,
            "сжатие не уменьшило сегмент: {} → {}",
            r.bytes_before,
            r.bytes_after
        );
        assert!(
            !r.source.exists(),
            "оригинал обязан быть удалён ПОСЛЕ сверки"
        );
        assert!(r.compacted.exists(), "сжатого файла нет");
    }

    let after = all_events(dir.path());
    assert_eq!(
        after, before,
        "после компакции поток событий ИЗМЕНИЛСЯ — это потеря/порча первичных данных, \
         а журнал бессмертен (DET-I-1)"
    );
}

/// C2: АКТИВНЫЙ сегмент никогда не сжимается — в него пишут прямо сейчас.
#[test]
fn c2_active_segment_is_never_compacted() {
    let dir = tempfile::tempdir().expect("dir");
    let mut j = Journal::open_with(dir.path(), cfg(128 * 1024)).expect("open_with");
    for i in 0..2_000 {
        j.append(snapshot(i)).expect("append");
    }
    j.flush().expect("flush");

    let segs = journal::list_segments(dir.path()).expect("segs");
    let active = segs.last().expect("active").clone();

    assert!(
        journal::compact_segment(&active, DEFAULT_COMPACT_LEVEL).is_err(),
        "АКТИВНЫЙ сегмент сжат — запись в него оборвётся, свежие данные потеряются"
    );
    assert!(active.path.exists(), "активный сегмент исчез");

    // Запись продолжается как ни в чём не бывало.
    j.append(snapshot(9_999))
        .expect("запись после попытки компакции обязана идти");
    j.flush().expect("flush");
}

/// C3: `keep_raw` последних закрытых сегментов остаются НЕсжатыми (свежее читают чаще).
#[test]
fn c3_keep_raw_segments_are_not_compacted() {
    let dir = tempfile::tempdir().expect("dir");
    build(dir.path(), 4_000, 96 * 1024);
    let segs = journal::list_segments(dir.path()).expect("segs");
    let keep = 2usize;
    let protected: Vec<_> = segs
        .iter()
        .rev()
        .take(keep + 1) // + активный
        .map(|s| s.path.clone())
        .collect();

    journal::compact_closed_segments(dir.path(), keep as u32, DEFAULT_COMPACT_LEVEL)
        .expect("compact");

    for p in &protected {
        assert!(
            p.exists(),
            "сегмент из keep_raw (или активный) был сжат: {p:?}"
        );
    }
}

/// C4: порченый сжатый сегмент НЕ читается молча — `stream` возвращает `Err`.
/// Тихо «прочитать половину» = отдать research неполные данные и не сказать об этом.
#[test]
fn c4_corrupted_compacted_segment_errors_not_truncates() {
    let dir = tempfile::tempdir().expect("dir");
    build(dir.path(), 3_000, 128 * 1024);
    journal::compact_closed_segments(dir.path(), 0, DEFAULT_COMPACT_LEVEL).expect("compact");

    // Портим середину сжатого файла.
    let zst = std::fs::read_dir(dir.path())
        .expect("rd")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with(".zst"))
        .expect("сжатый сегмент");
    let mut bytes = std::fs::read(&zst).expect("read");
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    std::fs::write(&zst, &bytes).expect("write");

    let mut saw_err = false;
    match journal::stream(dir.path(), EpochFilter::OwnCaptureOnly) {
        Err(_) => saw_err = true,
        Ok(s) => {
            for e in s {
                if e.is_err() {
                    saw_err = true;
                    break;
                }
            }
        }
    }
    assert!(
        saw_err,
        "порченый сжатый сегмент прочитался БЕЗ ошибки — research молча получит неполные данные"
    );
}

/// C5: чтение сжатого сегмента прод-масштаба — bounded memory (стрим, не распаковка в RAM).
/// Наивная реализация («распакуем сегмент целиком в Vec») на боевых 1 GiB сегментах = OOM.
///
/// ⚠ Оракул обязан мерить ПАМЯТЬ РАСПАКОВКИ, а не размер результата (урок TD-021: метрика,
/// на которой стоит гейт, сама подлежит валидации). Первая редакция C5 мерила
/// `stream(..).collect::<Vec<Event>>()` и падала на 123 MB — но эти 123 MB были САМИМ вектором
/// (60k событий × ~2 KB), а не буфером zstd: стрим уже был bounded. Оракул валил ЧЕСТНУЮ
/// реализацию за чужой аллокатор. Поэтому здесь поток ПОТРЕБЛЯЕТСЯ, но НЕ материализуется:
/// считаем события и хэшируем их, держа в руках ровно одно событие за раз — ровно так журнал
/// читает прод-путь (`stream`), для которого бюджет и заявлен.
#[test]
fn c5_streaming_compacted_segment_is_bounded_memory() {
    let dir = tempfile::tempdir().expect("dir");
    build(dir.path(), 60_000, 32 * 1024 * 1024); // ~несколько десятков MiB
    journal::compact_closed_segments(dir.path(), 0, DEFAULT_COMPACT_LEVEL).expect("compact");

    // Стрим потребляется поэлементно: ни `collect`, ни `Vec` — иначе меряем результат, не память.
    let (n, peak) = peak_delta(|| {
        let mut n = 0usize;
        for e in journal::stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream") {
            // Событие обязано декодироваться (иначе стрим «уложился в бюджет», ничего не прочитав),
            // но НЕ удерживается: живёт ровно один такт цикла.
            let _ = e.expect("event");
            n += 1;
        }
        n
    });
    assert_eq!(n, 60_000, "все события обязаны дойти из сжатых сегментов");
    assert!(
        peak < 16 * 1024 * 1024,
        "чтение сжатых сегментов выделило {peak} B при потреблении БЕЗ материализации — \
         сегмент распаковывается в память целиком; на боевом 1 GiB сегменте это OOM (класс TD-011)"
    );
}

/// C6: смешанный каталог (raw + .zst) читается в порядке `seq`, без дыр.
#[test]
fn c6_mixed_raw_and_compacted_streams_in_seq_order() {
    let dir = tempfile::tempdir().expect("dir");
    build(dir.path(), 4_000, 96 * 1024);
    journal::compact_closed_segments(dir.path(), 2, DEFAULT_COMPACT_LEVEL).expect("compact");

    let raw = std::fs::read_dir(dir.path())
        .expect("rd")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jrnl"))
        .count();
    let zst = std::fs::read_dir(dir.path())
        .expect("rd")
        .flatten()
        .filter(|e| e.path().to_string_lossy().ends_with(".zst"))
        .count();
    assert!(raw > 0 && zst > 0, "предусловие: смешанный каталог");

    let evs = all_events(dir.path());
    assert_eq!(evs.len(), 4_000);
    for (k, e) in evs.iter().enumerate() {
        assert_eq!(e.seq, k as u64, "seq сквозной через raw и сжатые сегменты");
    }
}
