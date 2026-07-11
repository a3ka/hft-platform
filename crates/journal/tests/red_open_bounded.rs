//! M-05 RED (sacred, architect) — TD-011: `Journal::open()` обязан выводить next_seq из
//! сегмента ОГРАНИЧЕННОЙ памятью (ХВОСТОВОЙ скан), НЕ загружая весь сегмент в RAM.
//!
//! Прод-инцидент 2026-07-11: откаченный `scan_next_seq` делал `read_to_end` ВСЕГО сегмента
//! (прод=2.65 GiB) в Vec на КАЖДОМ `open()` → recorder переставал писать (101% CPU,
//! 2.48 GiB RAM, OOM-риск на 3-7 днях). Юнит-J2 использовал крошечные фикстуры → не поймал.
//!
//! Оракул: на БОЛЬШОМ сегменте (64 MiB) с ОТСТАЮЩЕЙ метой `open()` обязан
//!  (1) вернуть next_seq из сегмента (== число фреймов) — семантика J2 на масштабе; и
//!  (2) пиковая аллокация во время open() < 8 MiB — НЕ грузить файл целиком.
//! RED на current main (meta-only → next_seq=мета≠N). Анти-плацебо: контрольный
//! `std::fs::read` (как в откаченном read_to_end) обязан превысить бюджет.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{Event, EventKind, Level, MdPayload, Venue};

// --- считающий аллокатор: current + peak по всему тест-бинарю (файл = отдельный бинарь) ---
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

/// Замерить пиковую аллокацию (дельту) во время выполнения `f`.
fn peak_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = CUR.load(SeqCst);
    PEAK.store(base, SeqCst);
    let r = f();
    let peak = PEAK.load(SeqCst);
    (r, peak.saturating_sub(base))
}

fn frame(seq: u64) -> Vec<u8> {
    // L2-фрейм ~ несколько KB (реалистично: прод-журнал — большие снапшоты).
    let mk = |base: i64| -> Vec<Level> {
        (0..100)
            .map(|i| Level {
                price: base + i as i64 * 100,
                size: 1_000 + i as i64,
            })
            .collect()
    };
    let ev = Event {
        seq,
        ts_mono_ns: seq,
        ts_wall_ms: seq as i64,
        kind: EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: mk(6_400_000_000_000),
                asks: mk(6_400_100_000_000),
                ts_exch_ms: seq as i64,
            },
        ),
    };
    let payload = postcard::to_stdvec(&ev).unwrap();
    let mut out = (payload.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&payload);
    out.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    out
}

const THRESHOLD: usize = 8 * 1024 * 1024; // 8 MiB бюджет памяти на open()
const TARGET: usize = 64 * 1024 * 1024; // 64 MiB сегмент

#[test]
fn open_derives_next_seq_from_segment_with_bounded_memory() {
    let dir = tempfile::tempdir().unwrap();
    let seg_path = dir.path().join("segment-00000000.jrnl");

    // Пишем 64 MiB валидных фреймов seq=0..N-1 стримингом (fixture-память не считается —
    // замер только вокруг open()).
    let mut written = 0usize;
    let mut n: u64 = 0;
    {
        let f = std::fs::File::create(&seg_path).unwrap();
        let mut w = std::io::BufWriter::new(f);
        while written < TARGET {
            let b = frame(n);
            w.write_all(&b).unwrap();
            written += b.len();
            n += 1;
        }
        w.flush().unwrap();
    }
    // Мета ОТСТАЁТ от сегмента (как после SIGKILL) — заведомо не N.
    std::fs::write(dir.path().join("journal.meta"), 5u64.to_le_bytes()).unwrap();
    assert!(n > 1000, "предусловие: много фреймов ({n})");

    // Контроль (анти-плацебо): полное чтение сегмента (как откаченный read_to_end)
    // выделяет ~размер файла → обязано превышать бюджет. Доказывает: замер реален.
    let (_, peak_full) = peak_delta(|| std::fs::read(&seg_path).unwrap());
    assert!(
        peak_full > THRESHOLD,
        "контроль: полное чтение ({peak_full} B) обязано превышать бюджет — иначе замер слеп"
    );

    // ТРЕБОВАНИЕ: open() выводит next_seq из сегмента с ОГРАНИЧЕННОЙ памятью.
    let (journal, peak_open) = peak_delta(|| journal::Journal::open(dir.path()).unwrap());

    assert_eq!(
        journal.next_seq(),
        n,
        "next_seq обязан быть из СЕГМЕНТА (={n}), не из отстающей меты (=5) — семантика J2 на масштабе"
    );
    assert!(
        peak_open < THRESHOLD,
        "open() выделил {peak_open} B (> {THRESHOLD} бюджета) — грузит сегмент целиком (TD-011). \
         Требуется ХВОСТОВОЙ скан O(1) памяти."
    );
}
