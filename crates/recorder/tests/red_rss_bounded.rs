//! RED M-08 (sacred, architect-only) — E7: writer-цикл recorder'а BOUNDED ПО ПАМЯТИ.
//! Заведён по TD-016 (MAJOR) — измеренная reviewer'ом траектория ОДНОГО контейнера:
//!
//!   ~1 мин →  8.40 MiB    ~2 часа → 21.63 MiB
//!   ~5 мин →  8.86 MiB    ~5 часов → 48.27 MiB      (норма 5–9 MiB, restarts=0)
//!   ≈ +6.5 MiB/час
//!
//! Контейнер всё это время healthy, heartbeat свежий, журнал пишется — **healthcheck такое
//! не поймает никогда** (класс TD-011: «зелёные гейты + Deploy success ≠ рабочий прод»).
//! Значит, оракул обязан мерить ГРАНИЦУ РЕСУРСА, а не корректность (паттерн
//! `journal/tests/red_open_bounded.rs`).
//!
//! Почему на уровне recorder'а, а не journal (C-005 M2): дрейф может жить в канале,
//! heartbeat-петле, буферах supervisor'а — то есть в коде recorder'а, которого journal-тесты
//! не исполняют вовсе.
//!
//! Анти-плацебо: тест ловит ЛЮБОЕ накопление, растущее с числом обработанных событий
//! (растущий буфер/лог/вектор): память на 200k событий обязана быть ≈ той же, что на 50k.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{EventKind, Level, MdPayload, Venue};
use journal::Journal;
use recorder::run_writer;

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

fn snapshot() -> EventKind {
    let lvl = |i: i64| Level {
        price: 6_400_000_000_000 + i * 100,
        size: 1_000 + i,
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: (0..20).map(lvl).collect(),
            asks: (0..20).map(lvl).collect(),
            ts_exch_ms: 1_752_000_000_000,
        },
    )
}

/// Прогнать writer-цикл recorder'а через N событий; вернуть пик аллокаций.
fn run_n(n: usize) -> usize {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");

    let (_, peak) = peak_delta(|| {
        rt.block_on(async move {
            let dir = tempfile::tempdir().expect("tempdir");
            let journal = Journal::open(dir.path()).expect("journal");
            let (tx, rx) = tokio::sync::mpsc::channel::<EventKind>(1024);
            let (sd_tx, sd_rx) = tokio::sync::oneshot::channel::<()>();

            // Продюсер: N событий, затем shutdown (writer дренит остаток).
            let producer = tokio::spawn(async move {
                for _ in 0..n {
                    if tx.send(snapshot()).await.is_err() {
                        break;
                    }
                }
                let _ = sd_tx.send(());
            });

            run_writer(
                rx,
                journal,
                dir.path().join("recorder.heartbeat"),
                std::sync::Arc::new(ops::metrics::Metrics::new()),
                async move {
                    let _ = sd_rx.await;
                },
            )
            .await
            .expect("run_writer");
            producer.await.expect("producer");
        })
    });
    peak
}

const SMALL: usize = 50_000;
const BIG: usize = 200_000;
/// Допустимый рост пика между 50k и 200k событий. Реальный лик (+6.5 MiB/час на потоке
/// ~1-2k событий/с) даёт на 150k дополнительных событий существенно больше.
const GROWTH_BUDGET: usize = 2 * 1024 * 1024; // 2 MiB
/// Абсолютный потолок writer-цикла (журнал буферизует батчами, книги не держит).
const ABS_BUDGET: usize = 16 * 1024 * 1024; // 16 MiB

#[test]
fn e7_writer_loop_memory_is_bounded_and_event_count_independent() {
    let peak_small = run_n(SMALL);
    let peak_big = run_n(BIG);

    assert!(
        peak_big < ABS_BUDGET,
        "writer-цикл на {BIG} событий выделил {peak_big} B (> {ABS_BUDGET}) — recorder держит \\
         в памяти то, что должен был отдать журналу (TD-016: 5→48 MiB за 5 часов, healthcheck \\
         этого не видит)"
    );

    let growth = peak_big.saturating_sub(peak_small);
    assert!(
        growth < GROWTH_BUDGET,
        "память writer-цикла РАСТЁТ с числом событий (50k→200k: +{growth} B) — это лик: \\
         на проде он даёт наблюдённые +6.5 MiB/час (TD-016). Память обязана быть O(1) по \\
         числу обработанных событий"
    );
}
