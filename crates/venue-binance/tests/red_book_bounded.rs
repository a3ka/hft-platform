//! SACRED (architect-only) — TD-016: граница памяти книги venue-адаптера.
//!
//! ## Две мои ошибки (фиксирую, чтобы не повторить в третий раз)
//!
//! 1. Лик нашёл reviewer: RSS recorder'а 8.4 MiB (1 мин) → 48.3 MiB (5 ч) ≈ **+6.5 MiB/час**,
//!    контейнер всё это время healthy — healthcheck такое не ловит (класс TD-011).
//! 2. Мой первый контракт и первый оракул были НЕВЕРНЫ (блокер C1 на PR-гейте):
//!    - фикстура была СИММЕТРИЧНОЙ (дифф обновлял top-3 обеих сторон) → `diff_mid` совпадал с
//!      реальным mid, и оракул зеленел против реализации, которая на АСИММЕТРИЧНОМ диффе
//!      стирала живые топовые уровни, включая лучший bid;
//!    - «кап 5000 уровней» как граница бессмысленен: число уровней ничего не говорит о
//!      дистанции, и он режет ровно то, из чего считается сигнал.
//!
//! ## Контракт эвикции v2
//!
//! **A. Дифф ничего не говорит о том, чего в нём НЕТ.** `@depth` содержит ТОЛЬКО изменившиеся
//!    уровни; лучший bid может не меняться целое окно — это норма. Единственное санкционированное
//!    удаление по диффу — явный `size == 0` от биржи. Удалять по «mid самого диффа» неправомерно.
//!
//! **B. Граница — ДИСТАНЦИЯ от mid КНИГИ, а не число уровней.** Эмиссия в журнал — bucketed по
//!    полосам до `MAX_REL_DIST` (±60%), и в сумму полосы входят ВСЕ уровни внутри окна. Значит
//!    эвикция ВНУТРИ окна меняет суммы полос → портит и сигнал, и первичные данные (журнал
//!    бессмертен). Эвиктить безопасно только то, что ВНЕ окна: оно не эмитится и нигде не считается.
//!
//! **C. Кап — аварийный backstop (`BACKSTOP_LEVELS_PER_SIDE`), а не рабочий инструмент.**
//!    Эвиктит самое дальнее от mid, топ не трогает. Его срабатывание — тревога, не норма.
//!
//! **D. Если после (B) память всё равно растёт** — уровни внутри окна реальны, и хранить сырую
//!    книгу нельзя вовсе: правильный фикс — инкрементальные bucket-агрегаты вместо полной книги
//!    (дизайн M-09). Поэтому §8 обязан ИЗМЕРИТЬ число in-band уровней, а не только RSS — иначе
//!    мы снова чиним по догадке.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use venue_binance::{apply_diff_to_book, DepthDiff, OrderBook, BACKSTOP_LEVELS_PER_SIDE};

static CUR: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            CUR.fetch_add(l.size(), SeqCst);
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
const TICK: i64 = 1_000_000; // $0.01 ×1e8
const MID0: i64 = 65_000_000_000_000; // $65_000 ×1e8
/// Зеркало `MAX_REL_DIST` в `venue-binance/src/lib.rs` — окно эмиссии.
const MAX_REL_DIST: f64 = 0.60;

fn empty_book() -> OrderBook {
    OrderBook {
        bids: Default::default(),
        asks: Default::default(),
        last_update_id: 0,
        last_event_time_ms: 0,
    }
}

fn diff(u: u64, bids: Vec<(i64, i64)>, asks: Vec<(i64, i64)>) -> DepthDiff {
    DepthDiff {
        event_time_ms: 1_752_000_000_000 + u as i64,
        u_first: u,
        u_final: u,
        bids,
        asks,
    }
}

fn bootstrap(book: &mut OrderBook, mid: i64, n: i64) {
    let d = diff(
        1,
        (1..=n).map(|k| (mid - k * TICK, 5 * E8)).collect(),
        (1..=n).map(|k| (mid + k * TICK, 5 * E8)).collect(),
    );
    apply_diff_to_book(book, &d);
}

// ── C1 (блокер PR-гейта): АСИММЕТРИЧНЫЕ диффы ─────────────────────────────────────────

/// (1a) Дифф БЕЗ лучшего bid (обновились глубокий bid и лучший ask) — штатная ситуация.
/// Ни один живой уровень не смеет исчезнуть.
#[test]
fn c1_asymmetric_diff_must_not_delete_live_levels() {
    let mut book = empty_book();
    bootstrap(&mut book, MID0, 100);
    let (bids_before, asks_before) = (book.bids.len(), book.asks.len());
    let best_bid_before = *book.bids.keys().next_back().expect("bids");

    apply_diff_to_book(
        &mut book,
        &diff(
            2,
            vec![(MID0 - 5 * TICK, 6 * E8)],
            vec![(MID0 + TICK, 6 * E8)],
        ),
    );

    assert_eq!(
        book.bids.len(),
        bids_before,
        "асимметричный дифф УДАЛИЛ живые bid-уровни ({} → {}): эвикция берёт «середину» из \
         самого диффа, а дифф содержит только ИЗМЕНИВШИЕСЯ уровни и ничего не говорит о тех, \
         что не упомянул. Испорченный стакан уходит в журнал (L2Snapshot) навсегда, а RSS и \
         healthcheck остаются зелёными — класс TD-011",
        bids_before,
        book.bids.len()
    );
    assert_eq!(book.asks.len(), asks_before, "asks не должны пострадать");
    assert!(
        book.bids.contains_key(&best_bid_before),
        "ЛУЧШИЙ BID удалён — спред фиктивно расширён, полосы OBI перекошены"
    );
}

/// (1b) Односторонний дифф (только bids) — противоположная сторона не трогается.
#[test]
fn c1_one_sided_diff_must_not_delete_opposite_side() {
    let mut book = empty_book();
    bootstrap(&mut book, MID0, 50);
    let asks_before = book.asks.len();
    let best_ask_before = *book.asks.keys().next().expect("asks");

    apply_diff_to_book(&mut book, &diff(2, vec![(MID0 - 3 * TICK, 9 * E8)], vec![]));

    assert_eq!(
        book.asks.len(),
        asks_before,
        "односторонний дифф изменил ПРОТИВОПОЛОЖНУЮ сторону книги"
    );
    assert!(
        book.asks.contains_key(&best_ask_before),
        "лучший ask удалён"
    );
}

/// (1c) Дифф только по ДАЛЬНИМ уровням — топ книги обязан выжить.
#[test]
fn c1_far_only_diff_must_not_touch_top_of_book() {
    let mut book = empty_book();
    bootstrap(&mut book, MID0, 100);
    let best_bid = *book.bids.keys().next_back().expect("bids");
    let best_ask = *book.asks.keys().next().expect("asks");

    apply_diff_to_book(
        &mut book,
        &diff(
            2,
            vec![(MID0 - 90 * TICK, 3 * E8)],
            vec![(MID0 + 90 * TICK, 3 * E8)],
        ),
    );

    assert!(
        book.bids.contains_key(&best_bid) && book.asks.contains_key(&best_ask),
        "дифф по дальним уровням снёс топ книги"
    );
    assert_eq!(
        book.bids.len(),
        100,
        "число bid-уровней не должно измениться"
    );
    assert_eq!(
        book.asks.len(),
        100,
        "число ask-уровней не должно измениться"
    );
}

/// (1d) `size == 0` — единственное санкционированное биржей удаление.
#[test]
fn c1_only_explicit_zero_size_removes_a_level() {
    let mut book = empty_book();
    bootstrap(&mut book, MID0, 10);
    apply_diff_to_book(&mut book, &diff(2, vec![(MID0 - 2 * TICK, 0)], vec![]));
    assert!(
        !book.bids.contains_key(&(MID0 - 2 * TICK)),
        "size=0 обязан удалить уровень"
    );
    assert_eq!(book.bids.len(), 9, "и ТОЛЬКО его");
}

// ── B: граница памяти — дистанция от mid КНИГИ ────────────────────────────────────────

/// Эвиктится только то, что ВНЕ окна эмиссии; всё внутри окна (входит в полосы OBI) — живёт.
#[test]
fn td016_evicts_only_levels_outside_emission_window() {
    let mut book = empty_book();
    bootstrap(&mut book, MID0, 10);

    let far_bid = (MID0 as f64 * 0.20) as i64; // −80% от mid: вне окна
    let far_ask = (MID0 as f64 * 1.80) as i64; // +80%: вне окна
    let in_band_bid = (MID0 as f64 * 0.70) as i64; // −30%: ВНУТРИ окна (полоса 30%)

    apply_diff_to_book(
        &mut book,
        &diff(
            2,
            vec![(far_bid, 4 * E8), (in_band_bid, 4 * E8)],
            vec![(far_ask, 4 * E8)],
        ),
    );

    assert!(
        !book.bids.contains_key(&far_bid) && !book.asks.contains_key(&far_ask),
        "уровни за пределами ±{:.0}% от mid не эвиктятся — они не попадают ни в эмиссию, ни в \
         полосы OBI, биржа их никогда не обнулит, и они живут в памяти вечно (лик TD-016)",
        MAX_REL_DIST * 100.0
    );
    assert!(
        book.bids.contains_key(&in_band_bid),
        "уровень ВНУТРИ окна эмиссии удалён — суммы полос OBI испорчены, сигнал деградировал \
         тихо, а RSS при этом стабилен (худший из возможных исходов)"
    );
}

/// Дрейф цены: уровни, ВЫШЕДШИЕ за окно, эвиктятся → память не растёт с числом апдейтов.
#[test]
fn td016_memory_bounded_when_price_drifts_out_of_band() {
    let pump = |updates: i64| -> (usize, usize) {
        let base = CUR.load(SeqCst);
        let mut book = empty_book();
        for u in 0..updates {
            let mid = MID0 + u * 40 * TICK; // +$0.40 за апдейт
            let d = diff(
                u as u64 + 1,
                (1..=10).map(|k| (mid - k * TICK, 5 * E8)).collect(),
                (1..=10).map(|k| (mid + k * TICK, 5 * E8)).collect(),
            );
            apply_diff_to_book(&mut book, &d);
        }
        let held = CUR.load(SeqCst).saturating_sub(base);
        (book.bids.len() + book.asks.len(), held)
    };

    let (levels_small, mem_small) = pump(100_000); // mid +62% → уровни начинают покидать окно
    let (levels_big, mem_big) = pump(200_000); // mid +123%

    assert!(
        levels_big < levels_small * 2,
        "число уровней растёт пропорционально числу апдейтов ({levels_small} → {levels_big}): \
         уровни, из которых цена ушла, не эвиктятся — это и есть лик TD-016"
    );
    let growth = mem_big.saturating_sub(mem_small);
    assert!(
        growth < 4 * 1024 * 1024,
        "память книги выросла на {growth} B при удвоении числа апдейтов — граница не держит"
    );
}

/// Аварийный backstop: патологически плотная книга ВНУТРИ окна всё равно ограничена;
/// эвиктится самое дальнее от mid, топ не трогается.
#[test]
fn td016_backstop_cap_evicts_farthest_never_top() {
    let mut book = empty_book();
    let n = (BACKSTOP_LEVELS_PER_SIDE as i64) * 3; // втрое больше капа
    let d = diff(
        1,
        (1..=n).map(|k| (MID0 - k * TICK, E8)).collect(),
        (1..=n).map(|k| (MID0 + k * TICK, E8)).collect(),
    );
    apply_diff_to_book(&mut book, &d);

    assert!(
        book.bids.len() <= BACKSTOP_LEVELS_PER_SIDE && book.asks.len() <= BACKSTOP_LEVELS_PER_SIDE,
        "backstop-кап не сработал: bids={} asks={} (кап {BACKSTOP_LEVELS_PER_SIDE}) — книга \
         может съесть память без предела",
        book.bids.len(),
        book.asks.len()
    );
    assert!(
        book.bids.contains_key(&(MID0 - TICK)) && book.asks.contains_key(&(MID0 + TICK)),
        "backstop-кап срезал ТОП книги вместо дальнего хвоста"
    );
}
