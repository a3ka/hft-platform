//! M-05 RED (sacred, architect) — TD-011: `Journal::open()` выводит next_seq из сегмента
//! с O(1) ПАМЯТЬЮ (ХВОСТОВОЙ скан фикс. K), НЕ загружая файл (или его ДОЛЮ) в RAM.
//!
//! Прод-инцидент 2026-07-11: откаченный `scan_next_seq` делал `read_to_end` ВСЕГО сегмента
//! (прод=2.65 GiB) в Vec на КАЖДОМ `open()` → recorder переставал писать (101% CPU,
//! 2.48 GiB RAM, OOM-риск). Юнит-J2 на крошечных фикстурах не поймал.
//!
//! Перманентный guard фиксирует ИНВАРИАНТ, а не инстанс:
//!  (1) корректность: next_seq из СЕГМЕНТА (не отстающей меты) — семантика J2 на масштабе;
//!  (2) абсолютный бюджет: пик аллокации open() < 8 MiB на 64 MiB сегменте (ловит full-read);
//!  (3) НЕЗАВИСИМОСТЬ ОТ РАЗМЕРА: пик(64 MiB) − пик(16 MiB) < 1 MiB → O(1) память. Ловит и
//!      full-read (растёт линейно), и «читать ДОЛЮ файла» (напр. seek(len/10)) — что абсолютный
//!      бюджет на одном размере пропустил бы (6.4 MiB на 64 MiB, но 265 MiB на 2.65 GiB).
//! RED на current main (meta-only → next_seq≠N). Анти-плацебо: контрольный `fs::read` > бюджет.
//! Решение (architect, 2026-07-11): двух-размерная независимость — достаточный перманентный
//! guard; отдельный ≥2.65 GiB стрим-кейс НЕ нужен (медленно на CI; sparse-нули парсятся как
//! пустые фреймы; независимость генерализует на любой размер). §8 прод-проверка reviewer'а —
//! merge-time, не юнит.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{Event, EventKind, Level, MdPayload, Venue};
use tempfile::TempDir;

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

/// Пиковая аллокация (дельта) во время `f`.
fn peak_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = CUR.load(SeqCst);
    PEAK.store(base, SeqCst);
    let r = f();
    (r, PEAK.load(SeqCst).saturating_sub(base))
}

const SEG: &str = "segment-00000000.jrnl";

fn frame(seq: u64) -> Vec<u8> {
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

/// Сегмент из валидных фреймов seq=0..N-1 размером ≥ `target` + ОТСТАЮЩАЯ мета (=5).
fn build_fixture(target: usize) -> (TempDir, u64) {
    let dir = tempfile::tempdir().unwrap();
    let mut written = 0usize;
    let mut n: u64 = 0;
    {
        let f = std::fs::File::create(dir.path().join(SEG)).unwrap();
        let mut w = std::io::BufWriter::new(f);
        while written < target {
            let b = frame(n);
            w.write_all(&b).unwrap();
            written += b.len();
            n += 1;
        }
        w.flush().unwrap();
    }
    std::fs::write(dir.path().join("journal.meta"), 5u64.to_le_bytes()).unwrap();
    (dir, n)
}

const THRESHOLD: usize = 8 * 1024 * 1024; // абсолютный бюджет памяти open()
const INDEP_DELTA: usize = 1024 * 1024; // допустимый рост памяти между размерами (O(1))
const SMALL: usize = 16 * 1024 * 1024;
const BIG: usize = 64 * 1024 * 1024;

#[test]
fn open_next_seq_from_segment_bounded_and_size_independent_memory() {
    let (small, small_n) = build_fixture(SMALL);
    let (big, big_n) = build_fixture(BIG);
    assert!(
        big_n > small_n && small_n > 1000,
        "предусловие: {small_n} < {big_n}"
    );

    // Анти-плацебо: полное чтение (как откаченный read_to_end) превышает бюджет → замер реален.
    let (_, peak_full) = peak_delta(|| std::fs::read(big.path().join(SEG)).unwrap());
    assert!(
        peak_full > THRESHOLD,
        "контроль: полное чтение ({peak_full} B) обязано превышать бюджет — иначе замер слеп"
    );

    let (jb, peak_big) = peak_delta(|| journal::Journal::open(big.path()).unwrap());
    let (js, peak_small) = peak_delta(|| journal::Journal::open(small.path()).unwrap());

    // (1) Корректность на масштабе (J2): next_seq из СЕГМЕНТА, не из отстающей меты (=5).
    assert_eq!(
        jb.next_seq(),
        big_n,
        "next_seq из СЕГМЕНТА (={big_n}), не мета(=5)"
    );
    assert_eq!(
        js.next_seq(),
        small_n,
        "next_seq из СЕГМЕНТА (={small_n}), не мета(=5)"
    );

    // (2) Абсолютный бюджет — ловит full-read.
    assert!(
        peak_big < THRESHOLD,
        "open() выделил {peak_big} B (> {THRESHOLD}) — грузит сегмент целиком (TD-011)"
    );

    // (3) Независимость от размера → O(1) память. Ловит и full-read, и чтение ДОЛИ файла.
    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < INDEP_DELTA,
        "память open() РАСТЁТ с размером файла (16→64 MiB: +{growth} B) — не O(1); \
         ХВОСТОВОЙ скан обязан читать ФИКС. K байт (seek к концу), НЕ долю файла"
    );
}
