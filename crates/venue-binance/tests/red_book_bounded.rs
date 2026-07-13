//! RED M-08 (sacred, architect-only) — **корень TD-016**: локальная full-book книга
//! venue-адаптера ОБЯЗАНА быть ограничена по памяти при бесконечном потоке diff-ов.
//!
//! Замеры reviewer'а (ОДИН контейнер, restarts=0): 8.4 MiB (1 мин) → 21.6 MiB (2 ч) →
//! 48.3 MiB (5 ч), ≈ **+6.5 MiB/час**, при этом healthy/heartbeat свежий (healthcheck
//! такое не ловит — класс TD-011). Recorder-writer-цикл проверен отдельным оракулом
//! (`recorder/tests/red_rss_bounded.rs`) и памятью НЕ течёт → лик выше по потоку.
//!
//! **Механизм (найден по коду, `venue-binance/src/lib.rs:306` `apply_diff_to_book`):**
//! diff вставляет уровни в `BTreeMap` и удаляет только при `size == 0`. Уровень, из
//! которого цена УШЛА, апдейтов больше не получает — и остаётся в книге НАВСЕГДА.
//! `MAX_REL_DIST = 0.60` применяется только к ЭМИССИИ снапшота, не к поддержанию книги.
//! За сутки дрейфа цены книга набирает десятки тысяч мёртвых уровней.
//!
//! **Контракт (architect, уточнён по факту прогона):** ±60%-полоса ограничителем НЕ является
//! (за час цена столько не проходит, а книга всё равно растёт линейно — измерено: 100k → 400k
//! уровней). Настоящее ограничение — **КАП уровней на сторону** (`MAX_BOOK_LEVELS_PER_SIDE =
//! 5000`, ровно глубина REST-снапшота, из которого книга и бутстрапится): при апдейте книга
//! эвиктит уровни, самые дальние от mid, сверх капа. Всё, что за пределами 5000 уровней от
//! середины, в эмиссию (±60%, bucketed) не влияет и не восстановимо из REST — хранить его
//! бессмысленно, а стоит оно 6.5 MiB/час.
//!
//! Анти-плацебо: текущая реализация (без эвикции) падает по обоим ассертам.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use venue_binance::{apply_diff_to_book, DepthDiff, OrderBook};

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

const E8: i64 = 100_000_000;

fn empty_book() -> OrderBook {
    OrderBook {
        bids: Default::default(),
        asks: Default::default(),
        last_update_id: 0,
        last_event_time_ms: 0,
    }
}

/// Поток diff-ов: цена дрейфует вверх, каждый апдейт трогает 20 уровней вокруг текущего mid.
/// Уровни, оставшиеся позади, апдейтов БОЛЬШЕ НЕ ПОЛУЧАЮТ (биржа не шлёт size=0 для того,
/// что давно ушло из окна) — ровно прод-сценарий.
fn pump(book: &mut OrderBook, updates: u64) {
    for u in 0..updates {
        let mid = 65_000_000_000_000i64 + (u as i64) * 1_000_000; // дрейф ~0.01$/апдейт
        let bids: Vec<(i64, i64)> = (1..=10).map(|k| (mid - k * 1_000_000, 5 * E8)).collect();
        let asks: Vec<(i64, i64)> = (1..=10).map(|k| (mid + k * 1_000_000, 5 * E8)).collect();
        let diff = DepthDiff {
            event_time_ms: 1_752_000_000_000 + u as i64,
            u_first: u + 1,
            u_final: u + 1,
            bids,
            asks,
        };
        apply_diff_to_book(book, &diff);
    }
}

fn levels(book: &OrderBook) -> usize {
    book.bids.len() + book.asks.len()
}

/// Кап уровней на сторону — та же глубина, что даёт REST-снапшот (`limit=5000`).
const MAX_BOOK_LEVELS_PER_SIDE: usize = 5_000;

/// (1) КАП: сколько бы diff-ов ни пришло, книга держит не больше `MAX_BOOK_LEVELS_PER_SIDE`
/// уровней на сторону — самые дальние от mid эвиктятся. Лучшие уровни при этом обязаны
/// сохраниться (эвикция не смеет ломать топ книги — из него считается сигнал).
#[test]
fn td016_book_levels_are_capped_per_side() {
    let mut book = empty_book();
    pump(&mut book, 50_000);

    assert!(
        book.bids.len() <= MAX_BOOK_LEVELS_PER_SIDE,
        "bids: {} уровней (кап {MAX_BOOK_LEVELS_PER_SIDE}) — книга копит мёртвые уровни, \
         которые биржа больше НИКОГДА не обнулит: это и есть утечка TD-016 (+6.5 MiB/час)",
        book.bids.len()
    );
    assert!(
        book.asks.len() <= MAX_BOOK_LEVELS_PER_SIDE,
        "asks: {} уровней (кап {MAX_BOOK_LEVELS_PER_SIDE})",
        book.asks.len()
    );

    // Эвикция режет ДАЛЬНИЕ уровни, а не топ: лучший bid/ask обязаны остаться свежими.
    let last_mid = 65_000_000_000_000i64 + 49_999 * 1_000_000;
    let best_bid = *book.bids.keys().next_back().expect("bids");
    let best_ask = *book.asks.keys().next().expect("asks");
    assert!(
        best_bid < last_mid && last_mid - best_bid <= 10 * 1_000_000,
        "лучший bid обязан быть у текущего mid — эвикция срезала ТОП вместо хвоста"
    );
    assert!(
        best_ask > last_mid && best_ask - last_mid <= 10 * 1_000_000,
        "лучший ask обязан быть у текущего mid"
    );
}

/// (2) ГРАНИЦА РЕСУРСА: память книги НЕ растёт с числом обработанных апдейтов.
/// 200k апдейтов обязаны стоить примерно столько же, сколько 50k (O(1) по времени работы,
/// а не O(число диффов)). Наивная реализация (только upsert) растёт линейно.
#[test]
fn td016_book_memory_is_independent_of_update_count() {
    let measure = |updates: u64| -> (usize, usize) {
        let base = CUR.load(SeqCst);
        PEAK.store(base, SeqCst);
        let mut book = empty_book();
        pump(&mut book, updates);
        let live = CUR.load(SeqCst).saturating_sub(base); // удержанная память книги
        let n = levels(&book);
        (live, n)
    };

    let (mem_small, lv_small) = measure(50_000);
    let (mem_big, lv_big) = measure(200_000);

    assert!(
        lv_big < lv_small * 2,
        "число уровней растёт с числом апдейтов ({lv_small} → {lv_big}) — книга копит \
         мёртвые уровни вместо эвикции"
    );
    let growth = mem_big.saturating_sub(mem_small);
    assert!(
        growth < 512 * 1024,
        "память книги выросла на {growth} B при увеличении числа апдейтов 50k→200k — \
         это линейный лик (TD-016). Книга обязана быть O(глубина окна), а не O(история)"
    );
}
