//! RED M-22 GW-I-2 (sacred, architect-only) — bounded-memory прод-масштаб (C-021 NOTE-2).
//!
//! Прямой наследник `journal::tests::red_stream_bounded` (TD-011, E5): Read Gateway ОБЯЗАН
//! читать журнал через `journal::stream` (bounded), НЕ `read_all()`/`Vec<Event>`. На боевом
//! журнале (8.3 GB+) материализация физически не запускается — весь кокпит-путь мёртв.
//!
//! Оракул фиксирует ИНВАРИАНТ, а не инстанс:
//!  (1) корректность: `snapshot` отдаёт непустую свёртку (депт-серия по L2Snapshot'ам);
//!  (2) абсолютный бюджет: пик аллокаций `snapshot` < 8 MiB на 64 MiB журнале (ловит `read_all`);
//!  (3) НЕЗАВИСИМОСТЬ ОТ РАЗМЕРА: пик(64 MiB) − пик(16 MiB) < 1 MiB → O(1) память
//!      (ловит «читаем долю файла» — на 8.3 GB такая доля снова убьёт машину).
//! Анти-плацебо: контрольное полное чтение сегмента обязано превышать бюджет — иначе замер слеп.
//!
//! Фикстура пинует ВСЕ события в ОДИН таймфрейм-бакет (общий `ts_exch_ms`), чтобы ВЫХОД был O(1)
//! независимо от размера журнала → замер изолирует рабочее множество СТРИМА, а не размер выхода.
//! RED сейчас: `snapshot` = `unimplemented!()` → паника (тело — engine-dev, task #3).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

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

const THRESHOLD: usize = 8 * 1024 * 1024;
const INDEP_DELTA: usize = 1024 * 1024;
const SMALL: u64 = 16 * 1024 * 1024;
const BIG: u64 = 64 * 1024 * 1024;
/// Все события в ОДНОМ бакете (общий ts) → выход O(1) вне зависимости от размера журнала.
const FIXED_TS: i64 = 1_752_000_000_000;

fn l2(i: u64) -> EventKind {
    let mk = |base: i64| -> Vec<Level> {
        (0..100)
            .map(|k| Level {
                price: base + k as i64 * 100,
                size: 1_000 + k as i64 + i as i64, // варьируем размер, но НЕ ts (один бакет)
            })
            .collect()
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: mk(6_400_000_000_000),
            asks: mk(6_400_100_000_000),
            ts_exch_ms: FIXED_TS,
        },
    )
}

fn build(target: u64) -> (tempfile::TempDir, u64) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: target / 4,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    let mut n: u64 = 0;
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        let mut written: u64 = 0;
        while written < target {
            j.append(l2(n)).expect("append");
            written += 2_400;
            n += 1;
        }
        j.flush().expect("flush");
    }
    (dir, n)
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
    }
}

#[test]
fn gateway_snapshot_is_bounded_memory_and_size_independent() {
    let (small, small_n) = build(SMALL);
    let (big, big_n) = build(BIG);
    assert!(
        big_n > small_n && small_n > 1_000,
        "предусловие фикстуры: {small_n} < {big_n}"
    );

    // Анти-плацебо: полное чтение первого сегмента обязано превышать бюджет — иначе замер слеп.
    let first_seg = journal::list_segments(big.path()).expect("segments")[0]
        .path
        .clone();
    let (_, peak_full) = peak_delta(|| std::fs::read(&first_seg).expect("read"));
    assert!(
        peak_full > THRESHOLD / 2,
        "контроль: полное чтение сегмента ({peak_full} B) слишком мало — увеличь фикстуру"
    );

    let s = sel();
    let (snap_big, peak_big) =
        peak_delta(|| gateway::snapshot(big.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST));
    let (snap_small, peak_small) = peak_delta(|| {
        gateway::snapshot(small.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
    });
    let snap_big = snap_big.expect("snapshot big");
    let snap_small = snap_small.expect("snapshot small");

    // (1) Корректность: свёртка непуста (депт-серия по обеим сторонам на полосе 0.1%).
    assert!(
        !snap_big.series.depth_series.is_empty() && !snap_small.series.depth_series.is_empty(),
        "snapshot обязан отдать депт-серию по L2Snapshot'ам"
    );

    // (2) Абсолютный бюджет — ловит read_all/Vec<Event>.
    assert!(
        peak_big < THRESHOLD,
        "snapshot выделил {peak_big} B (> {THRESHOLD}) — журнал грузится в память целиком \
         (на боевых 8.3 GB не запустится; класс TD-011 / C-021 NOTE-2)"
    );

    // (3) O(1) по размеру журнала — ловит «читаем долю файла».
    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < INDEP_DELTA,
        "память snapshot РАСТЁТ с размером журнала (16→64 MiB: +{growth} B) — не O(1)"
    );

    // === B3: frames_since (live-tail) ТОЖЕ bounded ===
    // «Пропуск к курсору» near-tail: корректный impl СТРИМИТ и пропускает seq<=after (O(1) память);
    // наивный `stream.collect::<Vec<Event>>().filter(...)` материализует историю → O(журнала).
    // Выход мал (≈3 события после курсора) → замер изолирует память ПРОПУСКА, а не выхода.
    let last_seq = |dir: &std::path::Path| -> u64 {
        journal::stream(dir, EpochFilter::OwnCaptureOnly)
            .expect("stream")
            .map(|e| e.expect("ev").seq)
            .last()
            .expect("журнал непуст")
    };
    let after_big = Cursor::at(last_seq(big.path()).saturating_sub(3));
    let after_small = Cursor::at(last_seq(small.path()).saturating_sub(3));

    let (fr_big, peak_fr_big) = peak_delta(|| {
        gateway::frames_since(big.path(), EpochFilter::OwnCaptureOnly, &s, after_big, usize::MAX)
    });
    let (fr_small, peak_fr_small) = peak_delta(|| {
        gateway::frames_since(small.path(), EpochFilter::OwnCaptureOnly, &s, after_small, usize::MAX)
    });
    let _ = (fr_big.expect("frames big"), fr_small.expect("frames small"));

    // Абсолютный бюджет — ловит материализацию истории при пропуске к курсору.
    assert!(
        peak_fr_big < THRESHOLD,
        "frames_since выделил {peak_fr_big} B (> {THRESHOLD}) — «пропуск к курсору» \
         материализует историю в Vec<Event>, а не стримит (C-022 B3 / TD-011)"
    );
    // O(1) по размеру журнала.
    let fr_growth = peak_fr_big.saturating_sub(peak_fr_small);
    assert!(
        fr_growth < INDEP_DELTA,
        "память frames_since РАСТЁТ с журналом (16→64 MiB: +{fr_growth} B) — пропуск к курсору не O(1)"
    );
}
