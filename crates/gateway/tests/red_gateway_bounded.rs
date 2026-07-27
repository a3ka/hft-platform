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
//! МНОГИМ time-бакетам (20000 бакетов) и требует: память snapshot ограничена окном `[at−W, at]`,
//! а НЕ числом бакетов истории. Окновой режим — `Selector.window_ms = Some(W)` (задача #1 M-37).
//! Анти-плацебо: текущая unbounded-реализация удерживает ВСЕ бакеты → память растёт с числом
//! бакетов → бюджет/size-independence нарушены. Деградированный вход (testing.md): L2Snapshot +
//! Trade на КАЖДЫЙ бакет → давятся ВСЕ per-bucket серии (heatmap/ohlcv/depth/cvd/vp/bubbles).
//!
//! Статус: GREEN против M-37-impl (Selector.window_ms реализован). TD-040: замер сериализован
//! `MEASURE_LOCK` + размер сегмента КОНСТАНТЕН → гейт мерит инвариант, не планировщик/окружение
//! (counting-allocator процесс-глобален; параллельные тесты загрязняли PEAK: dev PASS / CI FAIL).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Mutex;

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
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

/// TD-040: counting-allocator `CUR/PEAK` — ПРОЦЕСС-ГЛОБАЛЬНЫЙ. cargo гоняет тесты одного бинаря
/// ПАРАЛЛЕЛЬНО → одновременный `peak_delta` в двух тестах видит чужие аллокации → недетерминизм
/// (dev PASS / CI FAIL). Оба замеряющих теста берут этот лок на ВСЁ время → замер отражает только
/// СВОИ аллокации (мерим инвариант, не планировщик — testing.md §Целостность гейта п.2).
static MEASURE_LOCK: Mutex<()> = Mutex::new(());

fn peak_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = CUR.load(SeqCst);
    PEAK.store(base, SeqCst);
    let r = f();
    (r, PEAK.load(SeqCst).saturating_sub(base))
}

const THRESHOLD: usize = 8 * 1024 * 1024;
// Допуск size-независимости. TD-040: сегменты КОНСТАНТНОГО размера (SEG_BYTES), поэтому рост между
// SMALL/BIG отражает ТОЛЬКО число сегментов (стрим vs аккумуляция), не размер сегмента. 2 MiB
// поглощает шум интерливинга аллокаций dev↔CI, но << роста при материализации (МиБ × число сегментов).
const INDEP_DELTA: usize = 2 * 1024 * 1024;

// ---- Свойство 1: stream working-set (single-bucket, TD-011/TD-040) ----
// РАЗМЕР СЕГМЕНТА КОНСТАНТЕН — журнал растёт ЧИСЛОМ сегментов, не размером сегмента. Иначе
// per-segment буфер коррелирует с размером журнала → гейт мерит ОКРУЖЕНИЕ (allocator-интерливинг),
// а не инвариант (testing.md §Целостность гейта п.2; инцидент TD-040: dev PASS / CI FAIL на 1 MiB-дельте).
const SEG_BYTES: u64 = 1024 * 1024;
const SMALL: u64 = 16 * 1024 * 1024; // 16 сегментов
const BIG: u64 = 64 * 1024 * 1024; // 64 сегмента (4× count при том же размере сегмента)
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
        max_segment_bytes: SEG_BYTES, // КОНСТАНТА (TD-040): журнал растёт числом сегментов
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
    let _measure = MEASURE_LOCK.lock().unwrap(); // сериализация замера (TD-040)
    let (small, small_n) = build_single_bucket(SMALL);
    let (big, big_n) = build_single_bucket(BIG);
    assert!(
        big_n > small_n && small_n > 1_000,
        "предусловие: {small_n} < {big_n}"
    );

    // Анти-плацебо контраст: read_all МАТЕРИАЛИЗУЕТ весь журнал (peak ≥ размер журнала) — доказывает,
    // что бюджет СПОСОБЕН поймать материализацию. stream ниже обязан быть НАМНОГО меньше. Контраст
    // ~64 MiB vs <THRESHOLD устойчив к MiB-шуму интерливинга (мерим инвариант, не окружение — TD-040).
    let (_, peak_readall) = peak_delta(|| journal::read_all(big.path()).expect("read_all"));
    assert!(
        peak_readall > THRESHOLD,
        "контроль: read_all журнала дал peak {peak_readall} ≤ {THRESHOLD} — увеличь фикстуру"
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
    // O(1) по ЧИСЛУ сегментов (стрим держит один reader за раз, не аккумулирует). При том же
    // размере сегмента 4× сегментов → та же память; иначе stream копит историю (read_all-класс).
    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < INDEP_DELTA,
        "память растёт с числом сегментов журнала (+{growth} B) — stream аккумулирует, не O(1)"
    );
}

// ---- Свойство 2: память ограничена ОКНОМ, не числом бакетов (M-37, TD-039) ----
const BASE_TS: i64 = 1_752_000_000_000;
const WINDOW_MS: i64 = 60_000; // окно 60с → ~60 удержанных бакетов при timeframe 1000ms
const FEW_BUCKETS: u64 = 2_000;
const MANY_BUCKETS: u64 = 20_000; // 10× бакетов few (давит рост per-bucket состояния, не days)

fn trade_at(i: u64, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(64_000.0), // ≈ mid книги (bids base 6.4e12)
            size: to_fixed(1.0),
            side: [Side::Buy, Side::Sell][(i % 2) as usize],
            ts_exch_ms: ts,
        },
    )
}

/// На КАЖДЫЙ бакет: L2Snapshot (depth/heatmap/cob) + Trade (ohlcv/cvd/vwap/vp/volume_bubbles),
/// чтобы давить рост ВСЕХ per-bucket серий. ts растёт на 1000ms/бакет → num_buckets = num событий/2.
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
            let ts = BASE_TS + (b as i64) * 1_000; // отдельный 1с-бакет на пару событий
            j.append(book(6_400_000_000_000, 6_400_100_000_000, b as i64, ts))
                .expect("append book");
            j.append(trade_at(b, ts)).expect("append trade");
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
    let _measure = MEASURE_LOCK.lock().unwrap(); // сериализация замера (TD-040)
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

    // (1) Корректность: окновые серии непусты (trade→ohlcv, L2→heatmap/depth), но ОБРЕЗАНЫ окном.
    assert!(
        !snap_many.series.ohlcv.is_empty()
            && !snap_many.series.heatmap.is_empty()
            && !snap_many.series.depth_series.is_empty(),
        "окновой snapshot обязан отдать непустые ohlcv/heatmap/depth в окне"
    );
    let window_buckets = (WINDOW_MS / 1_000) as usize + 2; // допуск на граничный бакет
    let w_s = WINDOW_MS / 1_000; // окно в единицах time_s (timeframe 1000ms → шаг 1/бакет)
    let max_ts = snap_many.series.ohlcv.last().expect("ohlcv непуст").time_s;
    let lo = max_ts - w_s; // нижняя граница окна [lo, max_ts]

    // ВНИМАНИЕ (C-027 K3): per-bucket точки depth лежат ВНУТРИ `DepthRow.series`, а НЕ во внешнем
    // `Vec<DepthRow>` (внешний = число side×band). Бюджет памяти (2) покрывает heatmap (много ячеек).
    assert!(
        snap_many.series.ohlcv.len() <= window_buckets
            && snap_many
                .series
                .ohlcv
                .iter()
                .all(|r| r.time_s >= lo && r.time_s <= max_ts),
        "ohlcv вне окна (len={}, окно={window_buckets}, [{lo},{max_ts}])",
        snap_many.series.ohlcv.len()
    );
    assert!(
        !snap_many.series.depth_series.is_empty(),
        "depth_series (строки side×band) непуст"
    );
    for row in &snap_many.series.depth_series {
        assert!(
            !row.series.is_empty(),
            "DepthRow.series пуст (side={}, band={})",
            row.side,
            row.band_pct_e8
        );
        assert!(
            row.series.len() <= window_buckets,
            "DepthRow.series удерживает историю: {} > {window_buckets} бакетов (side={})",
            row.series.len(),
            row.side
        );
        assert!(
            row.series.iter().all(|&(t, _)| t >= lo && t <= max_ts),
            "DepthRow.series содержит точки вне окна [{lo}, {max_ts}] (side={})",
            row.side
        );
    }
    assert!(
        !snap_many.series.volume_bubbles.is_empty(),
        "volume_bubbles пуст — Trade-фикстура обязана дать пузыри"
    );
    assert!(
        snap_many.series.volume_bubbles.len() <= window_buckets
            && snap_many
                .series
                .volume_bubbles
                .iter()
                .all(|c| c.time_s >= lo && c.time_s <= max_ts),
        "volume_bubbles вне окна (len={}, окно={window_buckets}, [{lo},{max_ts}])",
        snap_many.series.volume_bubbles.len()
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
