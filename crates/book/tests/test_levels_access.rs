//! RED-тесты levels()/size_at() (sacred, architect; M-04 SVR-резолюция для
//! sim::taker_fills + корректной queue-ahead семантики FA sim §5).

use book::OrderBook;
use contracts::{to_fixed, Level, Side};

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

#[test]
fn test_levels_best_first_order() {
    let mut b = OrderBook::new();
    b.apply_snapshot(
        &[lvl(99.0, 2.0), lvl(100.0, 1.0), lvl(90.0, 5.0)],
        &[lvl(102.0, 4.0), lvl(101.0, 3.0)],
    );
    // bid: лучший = наибольшая цена
    assert_eq!(
        b.levels(Side::Buy),
        vec![
            (to_fixed(100.0), to_fixed(1.0)),
            (to_fixed(99.0), to_fixed(2.0)),
            (to_fixed(90.0), to_fixed(5.0)),
        ]
    );
    // ask: лучший = наименьшая цена
    assert_eq!(
        b.levels(Side::Sell),
        vec![
            (to_fixed(101.0), to_fixed(3.0)),
            (to_fixed(102.0), to_fixed(4.0)),
        ]
    );
    assert!(OrderBook::new().levels(Side::Buy).is_empty());
}

#[test]
fn test_size_at_exact_level() {
    let mut b = OrderBook::new();
    b.apply_snapshot(&[lvl(100.0, 1.5), lvl(99.0, 2.0)], &[lvl(101.0, 3.0)]);
    assert_eq!(b.size_at(Side::Buy, to_fixed(99.0)), to_fixed(2.0));
    assert_eq!(b.size_at(Side::Buy, to_fixed(100.0)), to_fixed(1.5));
    assert_eq!(b.size_at(Side::Sell, to_fixed(101.0)), to_fixed(3.0));
    // нет уровня → 0 (в т.ч. цена другой стороны)
    assert_eq!(b.size_at(Side::Buy, to_fixed(98.0)), 0);
    assert_eq!(b.size_at(Side::Sell, to_fixed(100.0)), 0);
}
