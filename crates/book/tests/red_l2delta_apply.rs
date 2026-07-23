//! RED M-29 BL-I-1..6 (sacred, architect-only) — L2Delta-применение в replay/reducer-книге.
//!
//! `OrderBook::apply_delta` (инкрементальный diff) + `Books::apply(MdPayload::L2Delta)`. Семантика зеркалит
//! live-захват (`venue-binance::apply_diff_to_book`): `size==0` удаляет, `size>0` upsert, неупомянутое
//! неизменно, пустая сторона = no-op (НЕ очистка). Основа heatmap (M-23).
//!
//! COMPILE-RED: метод `apply_delta` ещё НЕ существует → тест не компилируется. engine-dev добавляет apply_delta
//! + ветку Books::apply(L2Delta) → GREEN. Анти-плацебо: пустая-сторона→очистка / неупомянутое→удаление →
//! падение BL-I-3; текущий Books::apply игнорирует L2Delta → книга неизменна → падение BL-I-6.

use book::{Books, OrderBook};
use contracts::{Level, MdEvent, MdPayload, Side, Venue};

fn lvl(price: i64, size: i64) -> Level {
    Level { price, size }
}

/// Книга со снапшотом bids/asks (price,size).
fn seeded(bids: &[(i64, i64)], asks: &[(i64, i64)]) -> OrderBook {
    let mut b = OrderBook::new();
    let bl: Vec<Level> = bids.iter().map(|&(p, s)| lvl(p, s)).collect();
    let al: Vec<Level> = asks.iter().map(|&(p, s)| lvl(p, s)).collect();
    b.apply_snapshot(&bl, &al);
    b
}

#[test]
fn set_and_remove() {
    // BL-I-1: size>0 upsert (new/update), size==0 remove.
    let mut b = seeded(&[(100, 5), (99, 3)], &[(101, 4)]);
    b.apply_delta(
        &[lvl(98, 2), lvl(100, 7), lvl(99, 0)], // +98, 100→7, удалить 99
        &[],
    );
    assert_eq!(b.size_at(Side::Buy, 100), 7, "обновление уровня 100→7");
    assert_eq!(b.size_at(Side::Buy, 98), 2, "новый уровень 98");
    assert_eq!(b.size_at(Side::Buy, 99), 0, "size=0 → уровень 99 удалён");
}

#[test]
fn asymmetry_one_side() {
    // BL-I-2: дельта только по bid → ask нетронут.
    let mut b = seeded(&[(100, 5)], &[(101, 4), (102, 2)]);
    b.apply_delta(&[lvl(100, 9)], &[]);
    assert_eq!(b.size_at(Side::Buy, 100), 9, "bid обновлён");
    assert_eq!(b.size_at(Side::Sell, 101), 4, "ask 101 не тронут");
    assert_eq!(b.size_at(Side::Sell, 102), 2, "ask 102 не тронут");
}

#[test]
fn empty_side_and_unmentioned_preserved() {
    // BL-I-3 (класс TD-016): пустая сторона ≠ очистка; неупомянутое неизменно.
    let mut b = seeded(&[(100, 5), (99, 3)], &[(101, 4)]);
    b.apply_delta(&[], &[lvl(102, 1)]); // пустая bid-дельта; ask +102
    assert_eq!(
        b.size_at(Side::Buy, 100),
        5,
        "пустая bid-дельта НЕ очищает bid-сторону"
    );
    assert_eq!(b.size_at(Side::Buy, 99), 3, "неупомянутый bid 99 неизменен");
    assert_eq!(
        b.size_at(Side::Sell, 101),
        4,
        "неупомянутый ask 101 неизменен"
    );
    assert_eq!(b.size_at(Side::Sell, 102), 1, "новый ask 102");
}

#[test]
fn determinism() {
    // BL-I-4: тот же снапшот+дельты на двух книгах → идентичные levels().
    let deltas: &[(&[(i64, i64)], &[(i64, i64)])] = &[
        (&[(98, 2), (99, 0)], &[(103, 1)]),
        (&[(100, 8)], &[(101, 0)]),
    ];
    let run = || {
        let mut b = seeded(&[(100, 5), (99, 3)], &[(101, 4), (102, 2)]);
        for (bd, ad) in deltas {
            let bl: Vec<Level> = bd.iter().map(|&(p, s)| lvl(p, s)).collect();
            let al: Vec<Level> = ad.iter().map(|&(p, s)| lvl(p, s)).collect();
            b.apply_delta(&bl, &al);
        }
        b
    };
    let a = run();
    let b = run();
    assert_eq!(a.levels(Side::Buy), b.levels(Side::Buy), "bid детерминизм");
    assert_eq!(
        a.levels(Side::Sell),
        b.levels(Side::Sell),
        "ask детерминизм"
    );
}

#[test]
fn multi_level_and_scale() {
    // BL-I-5 (множественность+масштаб): много уровней в одной дельте (set+remove вперемешку).
    let mut b = OrderBook::new();
    let big: Vec<Level> = (0..500).map(|i| lvl(1_000 + i, 10 + i)).collect();
    b.apply_snapshot(&big, &[]);
    // одна дельта: обновить чётные, удалить кратные 5, добавить новый уровень.
    let mut delta: Vec<Level> = (0..500)
        .filter(|i| i % 2 == 0)
        .map(|i| lvl(1_000 + i, 999))
        .collect();
    delta.extend((0..500).filter(|i| i % 5 == 0).map(|i| lvl(1_000 + i, 0)));
    delta.push(lvl(9_999, 42));
    b.apply_delta(&delta, &[]);
    assert_eq!(b.size_at(Side::Buy, 9_999), 42, "новый дальний уровень");
    assert_eq!(b.size_at(Side::Buy, 1_000), 0, "кратный 5 (0) удалён");
    assert_eq!(
        b.size_at(Side::Buy, 1_002),
        999,
        "чётный не-кратный-5 обновлён"
    );
    assert_eq!(b.size_at(Side::Buy, 1_001), 11, "нечётный неизменен (10+1)");
}

#[test]
fn books_apply_routes_l2delta() {
    // BL-I-6: Books::apply(MdPayload::L2Delta) двигает книгу (сейчас L2Delta игнорируется → RED).
    let mut books = Books::new();
    let snap = MdEvent {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        payload: MdPayload::L2Snapshot {
            bids: vec![lvl(100, 5)],
            asks: vec![lvl(101, 4)],
            ts_exch_ms: 1_752_000_000_000,
        },
    };
    books.apply(&snap);
    let delta = MdEvent {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        payload: MdPayload::L2Delta {
            bids: vec![lvl(100, 9), lvl(99, 2)], // 100→9, +99
            asks: vec![lvl(101, 0)],             // удалить 101
            first_update_id: 1,
            final_update_id: 2,
            prev_final_update_id: None,
            ts_exch_ms: 1_752_000_000_001,
        },
    };
    books.apply(&delta);
    let bk = books.get(Venue::Binance, "BTCUSDT").expect("книга есть");
    assert_eq!(bk.size_at(Side::Buy, 100), 9, "L2Delta обновил bid 100");
    assert_eq!(bk.size_at(Side::Buy, 99), 2, "L2Delta добавил bid 99");
    assert_eq!(
        bk.size_at(Side::Sell, 101),
        0,
        "L2Delta удалил ask 101 (size=0)"
    );
}
