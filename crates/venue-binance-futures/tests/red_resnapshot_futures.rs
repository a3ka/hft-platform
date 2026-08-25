//! M-06 RED (sacred, architect) — N2/INV-N2: gap-ресинк futures-книги ПОЛНОСТЬЮ заменяет
//! стакан из свежего снапшота, БЕЗ переноса stale дальних уровней (no phantom liquidity).
//! Унифицирует B1 (deep-book quality). compile-RED против architect-designed seam
//! `FuturesDepthBook` (venue-dev выставляет maintainer под этот интерфейс — граница §4).
//! Анти-плацебо: merge-семантика (add вместо replace) оставит фантом → RED.

use contracts::{Level, Side};
use venue_binance_futures::FuturesDepthBook;

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: contracts::to_fixed(price),
        size: contracts::to_fixed(size),
    }
}

#[test]
fn n2_gap_resnapshot_evicts_stale_far_level() {
    let mut b = FuturesDepthBook::new();

    // Seed: узкая книга вокруг mid≈64000.
    b.apply_snapshot(&[lvl(64_000.0, 1.0)], &[lvl(64_001.0, 1.0)]);

    // Diff добавляет ДАЛЬНИЙ bid на ~37% ниже mid (в полосе 50%).
    b.apply_diff(&[lvl(40_000.0, 5.0)], &[]);
    let with_far = b.notional_within(Side::Buy, 0.50);
    assert!(
        with_far > 200_000.0,
        "предпосылка: дальний уровень 40000×5=$200k попал в полосу 50% (got {with_far})"
    );

    // GAP непрерывности → свежий REST-снапшот БЕЗ дальнего уровня → ПОЛНАЯ замена.
    b.apply_snapshot(&[lvl(64_000.0, 1.0)], &[lvl(64_001.0, 1.0)]);

    // INV-N2: дальний уровень УДАЛЁН (replace, не merge). Полоса 50% = только узкая книга ($64k).
    let after = b.notional_within(Side::Buy, 0.50);
    assert!(
        (after - 64_000.0).abs() < 1.0,
        "фантом: gap-ресинк НЕ заменил книгу — stale уровень 40000 остался (got ${after}, ожидалось ~$64000)"
    );
}
