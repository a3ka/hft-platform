//! RED M-08 (sacred, architect-only) — ПРОД-МАСШТАБНЫЙ bounded-memory стрим (E5).
//!
//! Прямой наследник `red_open_bounded.rs` (TD-011), но этажом выше: там ломался recorder
//! на `open()`, здесь ломается ВЕСЬ research-путь. Сегодня журнал читается через
//! `read_all() -> Vec<Event>` (`research-cli/src/main.rs:54`) — на боевом журнале
//! (8.3 GB на 2026-07-13, +2.8 GB/сут, плюс докупаемая история) это не запускается.
//! То есть «инфраструктура для создания альф» физически не работает на своих же данных.
//!
//! Оракул фиксирует ИНВАРИАНТ, а не инстанс:
//!  (1) корректность: стрим отдаёт ВСЕ события по порядку;
//!  (2) абсолютный бюджет: пик аллокаций < 8 MiB на 64 MiB журнале (ловит `read_all`);
//!  (3) НЕЗАВИСИМОСТЬ ОТ РАЗМЕРА: пик(64 MiB) − пик(16 MiB) < 1 MiB → O(1) память
//!      (ловит «читаем долю файла» — на 8.3 GB такая доля снова убьёт машину).
//! Анти-плацебо: контрольное полное чтение файла обязано превышать бюджет.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{DataSource, EventKind, Level, MdPayload, Venue};
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

fn snapshot(i: u64) -> EventKind {
    let mk = |base: i64| -> Vec<Level> {
        (0..100)
            .map(|k| Level {
                price: base + k as i64 * 100,
                size: 1_000 + k as i64,
            })
            .collect()
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: mk(6_400_000_000_000 + i as i64),
            asks: mk(6_400_100_000_000 + i as i64),
            ts_exch_ms: 1_752_000_000_000 + i as i64,
        },
    )
}

/// Журнал ≥ `target` байт, с ротацией (несколько сегментов — как в проде после M-08).
fn build(target: u64) -> (tempfile::TempDir, u64) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: target / 4, // ≥4 сегмента — стрим обязан их сшить
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
            j.append(snapshot(n)).expect("append");
            // ~2.4 KiB на снапшот из 200 уровней — оценка достаточна для остановки цикла.
            written += 2_400;
            n += 1;
        }
        j.flush().expect("flush");
    }
    (dir, n)
}

#[test]
fn stream_is_bounded_memory_and_size_independent() {
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
        "контроль: полное чтение сегмента ({peak_full} B) слишком мало — увеличь фикстуру, \
         иначе бюджет ничего не доказывает"
    );

    let count_stream = |dir: &std::path::Path| -> u64 {
        let mut n = 0u64;
        let mut prev: Option<u64> = None;
        for e in journal::stream(dir, EpochFilter::OwnCaptureOnly).expect("stream") {
            let e = e.expect("event");
            if let Some(p) = prev {
                assert_eq!(e.seq, p + 1, "стрим обязан отдавать события ПО ПОРЯДКУ");
            }
            prev = Some(e.seq);
            n += 1;
        }
        n
    };

    let (n_big, peak_big) = peak_delta(|| count_stream(big.path()));
    let (n_small, peak_small) = peak_delta(|| count_stream(small.path()));

    // (1) Корректность: все события дошли.
    assert_eq!(
        n_big, big_n,
        "стрим обязан отдать ВСЕ события большого журнала"
    );
    assert_eq!(n_small, small_n);

    // (2) Абсолютный бюджет — ловит read_all/Vec<Event>.
    assert!(
        peak_big < THRESHOLD,
        "стрим выделил {peak_big} B (> {THRESHOLD}) — журнал грузится в память целиком \
         (на боевых 8.3 GB это не запустится; класс TD-011)"
    );

    // (3) O(1) по размеру журнала — ловит «читаем долю файла».
    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < INDEP_DELTA,
        "память стрима РАСТЁТ с размером журнала (16→64 MiB: +{growth} B) — не O(1)"
    );
}
