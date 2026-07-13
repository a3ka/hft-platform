//! RED M-08 (sacred, architect-only) — грид на прод-масштабе (E5) + эквивалентность.
//!
//! Сегодня `research-cli/src/main.rs:54` делает `journal::read_all(&dir)` → весь журнал в
//! `Vec<Event>`. На боевом журнале (8.3 GB на 2026-07-13, +2.8 GB/сут, плюс докупаемая
//! история) грид просто не запустится: «инфраструктура для создания альф» не работает на
//! собственных данных. Это класс TD-011, но убивает не recorder, а весь research.
//!
//! Два оракула:
//!  (1) ПАМЯТЬ: грид на 16 MiB журнале укладывается в бюджет (наивный `Vec<Event>` — нет);
//!  (2) ЭКВИВАЛЕНТНОСТЬ: стрим-грид даёт РОВНО те же CellResult, что и in-memory `run_grid`
//!      на той же выборке — иначе «оптимизация» тихо меняет измеряемую логику (ровно то,
//!      чем M-07 занимался: бэктест обязан мерить то же самое).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use contracts::{DataSource, EventKind, Level, MdPayload, Venue};
use journal::{EpochFilter, Journal, WriterConfig};
use research_cli::grid::{self, JournalSource};
use research_cli::types::{CostsMode, GridSpec, SplitKind};
use research_cli::Ledger;
use sim::{FeeRates, FeeSchedule, LatencyTable};

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

/// Перекошенная книга → OBI даёт устойчивый сигнал.
fn snapshot() -> EventKind {
    let lvl = |p: f64, s: f64| Level {
        price: contracts::to_fixed(p),
        size: contracts::to_fixed(s),
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: vec![lvl(100.0, 50.0), lvl(99.0, 50.0), lvl(98.0, 50.0)],
            asks: vec![lvl(101.0, 5.0), lvl(102.0, 5.0), lvl(103.0, 5.0)],
            ts_exch_ms: 1_752_000_000_000,
        },
    )
}

fn build_journal(target_bytes: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: target_bytes / 4 + 1,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "fixture".to_string(),
        epoch_id: "own-test".to_string(),
    };
    let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
    let mut written: u64 = 0;
    while written < target_bytes {
        j.append(snapshot()).expect("append");
        written += 300; // грубая оценка размера фрейма — нужна лишь для остановки цикла
    }
    j.flush().expect("flush");
    dir
}

fn cell() -> serde_json::Value {
    serde_json::json!({
        "mode": "top_n",
        "n_levels": 3,
        "theta_e8": 10_000_000,
        "horizon_ms": 2_000,
        "venue": "Binance",
        "symbol": "BTCUSDT",
        "strategy": { "max_position_e8": 100_000_000i64 }
    })
}

fn spec() -> GridSpec {
    GridSpec {
        signal_family: "obi".to_string(),
        signal_id_prefix: "S-001".to_string(),
        cells: vec![cell()],
        costs_mode: CostsMode::Baseline,
        seed: 42,
    }
}

fn latency() -> LatencyTable {
    let mut t = LatencyTable::new();
    t.insert_samples(
        Venue::Binance,
        "BTCUSDT",
        vec![1_000_000],
        vec![1_000_000],
        vec![500_000],
        "synthetic-test-fixture",
    );
    t
}

fn fees() -> FeeSchedule {
    let mut f = FeeSchedule::new();
    f.insert_rates(
        Venue::Binance,
        FeeRates {
            maker_rate_e8: 10_000,
            taker_rate_e8: 45_000,
        },
    );
    f
}

const BUDGET: usize = 16 * 1024 * 1024; // бюджет памяти грида на 16 MiB журнале
const RANGE: (i64, i64) = (0, i64::MAX);

/// (1) Грид на журнале, который НЕ помещается в бюджет как `Vec<Event>`, обязан
/// отработать в bounded-memory. Наивный `read_all` здесь падает по бюджету.
#[test]
fn streamed_grid_runs_in_bounded_memory() {
    let dir = build_journal(16 * 1024 * 1024);
    let led = tempfile::tempdir().expect("led");
    let mut ledger = Ledger::open(led.path().join("trials.jsonl")).expect("ledger");
    let lat = latency();
    let fee = fees();

    // Контроль анти-плацебо: материализация журнала в память ВЫХОДИТ за бюджет.
    let (_, peak_read_all) = peak_delta(|| journal::read_all(dir.path()).expect("read_all"));
    assert!(
        peak_read_all > BUDGET,
        "контроль: read_all выделил {peak_read_all} B — фикстура слишком мала, оракул слеп"
    );

    let source = JournalSource {
        dir: dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
    };
    let (res, peak) = peak_delta(|| {
        let mut env = grid::GridRunEnv {
            ledger: &mut ledger,
            latency: &lat,
            fees: &fee,
        };
        grid::run_grid_streamed(&source, &spec(), SplitKind::Train, RANGE, &mut env, None)
            .expect("streamed grid")
    });

    assert_eq!(res.len(), 1, "ячейка обязана отработать");
    assert!(
        res[0].intents > 0,
        "сигнал перекошенной книги обязан породить интенты"
    );
    assert!(
        peak < BUDGET,
        "стрим-грид выделил {peak} B (> {BUDGET}) — журнал материализуется в память; \
         на боевых 8.3 GB это не запустится"
    );
}

/// (2) ЭКВИВАЛЕНТНОСТЬ: стрим-грид считает РОВНО то же, что in-memory `run_grid`.
/// Иначе переход на стрим тихо изменит измеряемую логику (та же ловушка, что ad-hoc
/// harness до M-07 — бэктест мерил не то, что торгуется).
#[test]
fn streamed_grid_equals_in_memory_grid() {
    let dir = build_journal(512 * 1024);

    let events = journal::read_all(dir.path()).expect("read_all");
    let led1 = tempfile::tempdir().expect("led1");
    let mut l1 = Ledger::open(led1.path().join("t.jsonl")).expect("ledger");
    let lat = latency();
    let fee = fees();
    let in_memory = {
        let mut env = grid::GridRunEnv {
            ledger: &mut l1,
            latency: &lat,
            fees: &fee,
        };
        grid::run_grid(&events, &spec(), SplitKind::Train, RANGE, &mut env, None).expect("run_grid")
    };

    let led2 = tempfile::tempdir().expect("led2");
    let mut l2 = Ledger::open(led2.path().join("t.jsonl")).expect("ledger");
    let streamed = {
        let mut env = grid::GridRunEnv {
            ledger: &mut l2,
            latency: &lat,
            fees: &fee,
        };
        let source = JournalSource {
            dir: dir.path().to_path_buf(),
            filter: EpochFilter::OwnCaptureOnly,
        };
        grid::run_grid_streamed(&source, &spec(), SplitKind::Train, RANGE, &mut env, None)
            .expect("streamed")
    };

    assert_eq!(
        in_memory.len(),
        streamed.len(),
        "число ячеек обязано совпасть"
    );
    for (a, b) in in_memory.iter().zip(streamed.iter()) {
        // ПОЛНАЯ семантика CellResult (C-005 M3): «оптимизация» не смеет тихо изменить
        // ни одно число, которое дальше уходит в ValidationReport → trials-ledger →
        // подпись founder'а (gates §6/§7).
        assert_eq!(a.params, b.params, "params ячейки");
        assert_eq!(a.params_hash, b.params_hash, "хэш ячейки");
        assert_eq!(a.intents, b.intents, "интенты");
        assert_eq!(a.fills, b.fills, "филлы");
        assert_eq!(a.net_pnl_e8, b.net_pnl_e8, "PnL до цента");
        assert_eq!(a.turnover_e8, b.turnover_e8, "оборот");
        assert_eq!(a.max_drawdown_e8, b.max_drawdown_e8, "max drawdown");
        assert_eq!(
            a.returns, b.returns,
            "returns-серия обязана совпасть ПОЭЛЕМЕНТНО (не только по длине) — иначе \
             стрим-путь тихо меняет σ и Sharpe отчёта"
        );
        assert_eq!(
            a.sharpe.to_bits(),
            b.sharpe.to_bits(),
            "Sharpe обязан совпасть БИТ-В-БИТ (детерминизм, DESIGN §1)"
        );
    }

    // Побочный эффект ledger'а (RC-I-9) — тот же: те же записи, тот же порядок.
    let recs_mem = l1.read_all().expect("ledger in-memory");
    let recs_str = l2.read_all().expect("ledger streamed");
    assert_eq!(
        recs_mem.len(),
        recs_str.len(),
        "оба пути обязаны записать одинаковое число trial-записей"
    );
    for (a, b) in recs_mem.iter().zip(recs_str.iter()) {
        assert_eq!(a.params_hash, b.params_hash, "ledger: хэш ячейки");
        assert_eq!(a.signal_family, b.signal_family, "ledger: семейство");
        assert_eq!(a.code_hash, b.code_hash, "ledger: code_hash эпохи (TD-015)");
        assert_eq!(
            a.sharpe.map(f64::to_bits),
            b.sharpe.map(f64::to_bits),
            "ledger: Sharpe бит-в-бит (по нему считается deflated Sharpe → подпись founder)"
        );
    }
}
