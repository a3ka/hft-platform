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

use venue_binance::{apply_diff_to_book, DepthDiff, OrderBook, BACKSTOP_LEVELS_PER_SIDE};

// TD-023: прежняя редакция мерила память через `#[global_allocator]` + глобальный счётчик `CUR`.
// Две беды сразу: (1) ФЛАК — под параллельным `cargo test --all` соседние тесты ТОГО ЖЕ процесса
// alloc/free'ят и загрязняют глобальный счётчик, поэтому «рост» скакал (reviewer: 6.56 MB на
// 2 ядрах против <4 MiB при -j1) → main краснел на ровном месте; (2) МЕРИЛИ НЕ ТУ ВЕЛИЧИНУ —
// аллокатор считает аллокации ВСЕГО процесса, а нам нужна память КНИГИ. Память книги —
// детерминированно O(числа уровней) (`BTreeMap<i64,i64>`), поэтому меряем число уровней напрямую.
// Глобальный аллокатор удалён целиком — он и был источником гонки.

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

/// Непрерывный поток апдейтов НЕ уводит книгу в неограниченный рост: число уровней **насыщается**
/// и держится потолком `BACKSTOP_LEVELS_PER_SIDE`, а не растёт с числом апдейтов.
///
/// ⚠ TD-023 — вторая правка этого теста (устаревшая метрика, 8-й случай класса за сессию):
/// прежний ассерт был `growth = mem_big − mem_small < 4 MiB`, где `mem_*` — глобальный
/// аллокатор-счётчик. Помимо флака (гонка счётчика под параллельным `cargo test`), сам ПОРОГ
/// 4 MiB был ФИКЦИЕЙ: он остался с rev1, когда `BACKSTOP` был 5000/сторону. rev6 поднял backstop
/// до **200 000/сторону** («точность данных > экономия памяти», TD-021) ⇒ книга законно держит до
/// 400 000 уровней ≈ 25 MB, и 4 MiB недостижимы. Замер «проходил» лишь потому, что `growth` —
/// РАЗНОСТЬ двух больших шумных чисел, которая под шумом иногда падала < 4 MiB (green), иногда нет
/// (red). Настоящая гарантия ограниченности здесь — не `MAX_REL_DIST` (в drift-сценарии окно
/// растёт вместе с mid и почти ничего не эвиктит), а АВАРИЙНЫЙ backstop. Его и проверяем.
#[test]
fn td016_book_saturates_at_backstop_not_grows_with_updates() {
    let pump = |updates: i64| -> usize {
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
        book.bids.len() + book.asks.len()
    };

    let levels_small = pump(100_000);
    let levels_big = pump(200_000); // вдвое больше апдейтов

    // (1) Насыщение: удвоение числа апдейтов НЕ удваивает число уровней — книга ограничена,
    //     иначе это неограниченный рост (лик). Замер детерминирован (гонки нет).
    assert!(
        levels_big < levels_small * 2,
        "число уровней растёт пропорционально числу апдейтов ({levels_small} → {levels_big}): \
         книга не ограничена ничем — это лик"
    );
    // (2) Жёсткий потолок — backstop на обе стороны. Это ФАКТИЧЕСКАЯ гарантия ограниченности
    //     памяти книги (память = O(levels)); порог берётся из контракта, а не из воздуха.
    let cap = 2 * BACKSTOP_LEVELS_PER_SIDE;
    assert!(
        levels_big <= cap,
        "число уровней {levels_big} превысило backstop-потолок {cap} (2×{BACKSTOP_LEVELS_PER_SIDE}) \
         — аварийный кап не держит, память книги не ограничена"
    );
}

/// Аварийный backstop: патологически плотная книга ВНУТРИ окна всё равно ограничена;
/// эвиктится самое дальнее от mid, топ не трогается.
#[test]
fn td016_backstop_cap_evicts_farthest_never_top() {
    let mut book = empty_book();
    let n = BACKSTOP_LEVELS_PER_SIDE as i64 + 5_000; // чуть больше капа
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
