//! RED M-37 GW-I-2 bounded-memory (sacred, architect-only) — ДВА независимых свойства.
//!
//! ## Свойство 1 (M-22, TD-011): stream working-set bounded
//! snapshot ОБЯЗАН читать журнал через `journal::stream` (bounded), НЕ `read_all()`/`Vec<Event>`.
//! Фикстура — все события в ОДИН бакет → выход O(1) → замер изолирует рабочее множество СТРИМА.
//!
//! ## Свойство 2 (M-37, TD-039 — ЧЕГО НЕ ХВАТАЛО): память ограничена ОКНОМ, не историей
//! Прошлый оракул был СЛЕП: он пинил ВСЕ события в один бакет (`FIXED_TS`), поэтому число
//! time-бакетов = 1 и рост per-bucket состояния (`heatmap_buckets`, ohlcv, bubbles, vwap.values)
//! НЕ давился. Именно этот рост убил прод (RSS 7.3GB, OOM). Новый тест раскидывает события по
//! МНОГИМ бакетам за МНОГО UTC-дней и требует: память snapshot ограничена окном `[at−W, at]`, а
//! НЕ числом бакетов истории. Окновой режим — `Selector.window_ms = Some(W)` (задача #1 M-37).
//! Анти-плацебо: текущая unbounded-реализация удерживает ВСЕ бакеты → память растёт с историей →
//! бюджет/size-independence нарушены. Деградированный вход (testing.md): много сессий (много дней).
//!
//! COMPILE-RED сейчас: поле `Selector.window_ms` ещё НЕ существует → файл не компилируется, пока
//! engine-dev (task #1) не добавит его. GREEN после эвикции бакетов (task #2).

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

// ---- Свойство 1: stream working-set (single-bucket, TD-011) ----
const SMALL: u64 = 16 * 1024 * 1024;
const BIG: u64 = 64 * 1024 * 1024;
const FIXED_TS: i64 = 1_752_000_000_000;

fn book(base_bid: i64, base_ask: i64, jitter: i64, ts: i64) -> EventKind {
    let mk = |base: i64| -> Vec<Level> {
        (0..100)
            .map(|k| Level {
                price: base + k * 100,
                size: 1_000 + k + jitter,
            })
            .collect()
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: mk(base_bid),
            asks: mk(base_ask),
            ts_exch_ms: ts,
        },
    )
}

fn build_single_bucket(target: u64) -> (tempfile::TempDir, u64) {
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
            j.append(book(
                6_400_000_000_000,
                6_400_100_000_000,
                n as i64,
                FIXED_TS,
            ))
            .expect("append");
            written += 2_400;
            n += 1;
        }
        j.flush().expect("flush");
    }
    (dir, n)
}

/// offline-Selector: окна нет (None), полная свёртка — режим read-side инструментов.
fn sel_unbounded() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
    }
}

#[test]
fn snapshot_stream_working_set_bounded() {
    // Свойство 1: единый бакет → выход O(1) → замер = рабочее множество СТРИМА (ловит read_all).
    let (small, small_n) = build_single_bucket(SMALL);
    let (big, big_n) = build_single_bucket(BIG);
    assert!(
        big_n > small_n && small_n > 1_000,
        "предусловие: {small_n} < {big_n}"
    );

    let first_seg = journal::list_segments(big.path()).expect("segments")[0]
        .path
        .clone();
    let (_, peak_full) = peak_delta(|| std::fs::read(&first_seg).expect("read"));
    assert!(
        peak_full > THRESHOLD / 2,
        "контроль: чтение сегмента слишком мало ({peak_full} B)"
    );

    let s = sel_unbounded();
    let (snap_big, peak_big) = peak_delta(|| {
        gateway::snapshot(big.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
    });
    let (_snap_small, peak_small) = peak_delta(|| {
        gateway::snapshot(
            small.path(),
            EpochFilter::OwnCaptureOnly,
            &s,
            Cursor::LATEST,
        )
    });
    let snap_big = snap_big.expect("snapshot big");
    assert!(
        !snap_big.series.depth_series.is_empty(),
        "депт-серия непуста"
    );
    assert!(
        peak_big < THRESHOLD,
        "snapshot выделил {peak_big} B — журнал грузится целиком (read_all, TD-011)"
    );
    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < INDEP_DELTA,
        "память растёт с РАЗМЕРОМ журнала (+{growth} B) — не O(1)"
    );
}

// ---- Свойство 2: память ограничена ОКНОМ, не числом бакетов (M-37, TD-039) ----
const BASE_TS: i64 = 1_752_000_000_000;
const WINDOW_MS: i64 = 60_000; // окно 60с → ~60 удержанных бакетов при timeframe 1000ms
const FEW_BUCKETS: u64 = 2_000;
const MANY_BUCKETS: u64 = 20_000; // 10× истории few → пересекает много UTC-дней

/// Один L2Snapshot на бакет: ts растёт на 1000ms/бакет → num_buckets = num_events.
fn build_buckets(num_buckets: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: 8 * 1024 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for b in 0..num_buckets {
            let ts = BASE_TS + (b as i64) * 1_000; // отдельный 1с-бакет на событие
            j.append(book(6_400_000_000_000, 6_400_100_000_000, b as i64, ts))
                .expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn sel_windowed() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(WINDOW_MS),
    }
}

#[test]
fn snapshot_memory_bounded_by_window_not_history() {
    let few = build_buckets(FEW_BUCKETS);
    let many = build_buckets(MANY_BUCKETS);
    let s = sel_windowed();

    let (snap_many, peak_many) = peak_delta(|| {
        gateway::snapshot(many.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
    });
    let (snap_few, peak_few) = peak_delta(|| {
        gateway::snapshot(few.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
    });
    let snap_many = snap_many.expect("snapshot many");
    let snap_few = snap_few.expect("snapshot few");

    // (1) Корректность: окновые серии непусты, но ОБРЕЗАНЫ окном (heatmap/ohlcv в пределах ~W).
    assert!(
        !snap_many.series.ohlcv.is_empty() && !snap_many.series.heatmap.is_empty(),
        "окновой snapshot обязан отдать непустые серии в окне"
    );
    let window_buckets = (WINDOW_MS / 1_000) as usize + 2; // допуск на граничный бакет
    assert!(
        snap_many.series.ohlcv.len() <= window_buckets,
        "ohlcv обязан быть ограничен окном: {} бакетов > окно {window_buckets}",
        snap_many.series.ohlcv.len()
    );

    // (2) Абсолютный бюджет на журнале с МНОГИМИ бакетами (ловит удержание всех бакетов истории).
    assert!(
        peak_many < THRESHOLD,
        "snapshot выделил {peak_many} B (> {THRESHOLD}) на {MANY_BUCKETS}-бакетном журнале — \
         удерживает ВСЕ бакеты истории, а не окно (TD-039 OOM-класс)"
    );

    // (3) НЕЗАВИСИМОСТЬ ОТ ИСТОРИИ: 10× бакетов → та же память (одно окно).
    let growth = peak_many.saturating_sub(peak_few);
    assert!(
        growth < INDEP_DELTA,
        "память РАСТЁТ с числом бакетов истории ({FEW_BUCKETS}→{MANY_BUCKETS}: +{growth} B) — \
         не ограничена окном; ровно это дало прод-OOM (TD-039)"
    );

    // Детерминизм окна (VB-I-1): повтор идентичен.
    let (snap_many2, _) = peak_delta(|| {
        gateway::snapshot(many.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
    });
    assert_eq!(
        snap_many.series.ohlcv,
        snap_many2.expect("snap2").series.ohlcv,
        "окновой snapshot недетерминирован"
    );
    let _ = snap_few;
}
