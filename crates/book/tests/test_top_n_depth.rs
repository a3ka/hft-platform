//! RED-тест top_n_depth (sacred, architect; M-04 fix per critic C-001 C1).
//! Примитив Трека A OBI: сумма размеров N лучших уровней стороны.

use book::OrderBook;
use contracts::{to_fixed, Level, Side};

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

#[test]
fn test_top_n_depth_sums_best_levels() {
    let mut b = OrderBook::new();
    b.apply_snapshot(
        &[lvl(100.0, 1.0), lvl(99.0, 2.0), lvl(90.0, 5.0)],
        &[lvl(101.0, 3.0), lvl(102.0, 4.0), lvl(110.0, 7.0)],
    );
    // bid: лучшие = наибольшие цены → 100(1.0), 99(2.0)
    assert_eq!(b.top_n_depth(Side::Buy, 2), to_fixed(3.0));
    // ask: лучшие = наименьшие цены → 101(3.0), 102(4.0)
    assert_eq!(b.top_n_depth(Side::Sell, 2), to_fixed(7.0));
    // n=1 — строго top-of-book
    assert_eq!(b.top_n_depth(Side::Buy, 1), to_fixed(1.0));
    assert_eq!(b.top_n_depth(Side::Sell, 1), to_fixed(3.0));
}

#[test]
fn test_top_n_depth_n_exceeds_levels_and_edges() {
    let mut b = OrderBook::new();
    b.apply_snapshot(&[lvl(100.0, 1.0), lvl(99.0, 2.0)], &[]);
    // N больше числа уровней — суммируем что есть, не выдумываем
    assert_eq!(b.top_n_depth(Side::Buy, 10), to_fixed(3.0));
    // пустая сторона → 0
    assert_eq!(b.top_n_depth(Side::Sell, 5), 0);
    // n=0 → 0
    assert_eq!(b.top_n_depth(Side::Buy, 0), 0);
}
