//! RED OPS-I-1 LIVE-РЕЖИМ (sacred, architect-only) — recon не флудит на ЗДОРОВОМ рынке.
//!
//! §8 провал (2026-07-17): recon эмитил ReconDivergence на КАЖДОМ цикле здорового рынка
//! (divergence_bps=2436..7754, best_price_diverged=true) без инъекции. Мои прежние юниты
//! (`red_recon_sink::normal_book_is_silent`, `red_recon_wiring`) сравнивали книгу С СОБОЙ →
//! GREEN → live-режим не моделировали (green-unit/live-flood, класс TD-011/TD-016). Мой wiring-RED
//! сам был «фикстурой счастливого пути» — ровно тот дефект, что мы весь день ловим.
//!
//! Два корня (reviewer): (1) `best_price_diverged = best != best` срабатывает на timing-skew
//! (best двигается тик-в-тик между WS и REST); (2) local — сбакетированный эмит (0.02%), reference
//! — сырой REST → суммы полос не совпадают по построению. Плюс возможная РЕАЛЬНАЯ фантомная
//! ликвидность (TD-016 within-band), которую recon ВЕРНО ловит.
//!
//! Эти оракулы РАЗВОДЯТ шум-представления (обязан замолчать после фикса) от реальной порчи
//! (обязана ловиться). Оракулы 1 падают на текущей семантике (форсируют фикс толерантности);
//! 2/3/4 обязаны остаться GREEN (анти-плацебо: фикс не глушит детектор).

use book::OrderBook;
use contracts::Level;
use ops::recon::{reconcile, EPS_TEST_BPS};

const MID: i64 = 65_000_000_000_000; // $65k ×1e8
const TICK: i64 = 1_000_000; // $0.01 ×1e8; на $65k = 0.00154 bps (timing-skew ≪ 1 bps)

fn book_from(bids: Vec<Level>, asks: Vec<Level>) -> OrderBook {
    let mut b = OrderBook::new();
    b.apply_snapshot(&bids, &asks);
    b
}

/// Плотная книга ±N тиков, объём 5.0 на уровень.
fn dense(n: i64) -> (Vec<Level>, Vec<Level>) {
    let bids = (1..=n)
        .map(|k| Level {
            price: MID - k * TICK,
            size: 5 * 100_000_000,
        })
        .collect();
    let asks = (1..=n)
        .map(|k| Level {
            price: MID + k * TICK,
            size: 5 * 100_000_000,
        })
        .collect();
    (bids, asks)
}

/// (1, ПАДАЕТ СЕЙЧАС) TIMING-SKEW: reference — та же книга, но best сдвинут на несколько тиков
/// (обновился между WS и REST). Это НОРМА (sub-bps), НЕ порча. `best_price_diverged` обязан быть
/// false — иначе recon флудит на каждом такте. Текущая точная `!=` → true → падает.
#[test]
fn best_price_timing_skew_is_tolerated() {
    let (lb, la) = dense(100);
    let local = book_from(lb, la);
    // reference: best bid/ask сдвинуты на 3 тика (цена чуть ушла за миллисекунды до REST).
    let rb = (1..=100)
        .map(|k| Level {
            price: MID - 3 * TICK - k * TICK,
            size: 5 * 100_000_000,
        })
        .collect();
    let ra = (1..=100)
        .map(|k| Level {
            price: MID + 3 * TICK + k * TICK,
            size: 5 * 100_000_000,
        })
        .collect();
    let reference = book_from(rb, ra);

    let out = reconcile(&local, &reference);
    assert!(
        !out.best_price_diverged,
        "best сдвинут на 3 тика (timing-skew, {:.4} bps ≪ 1) и recon счёл это порчей — на живом \
         рынке best двигается каждый такт, recon будет флудить ReconDivergence (§8-провал)",
        (3.0 * TICK as f64 / MID as f64) * 10_000.0
    );
    assert!(
        !out.exceeds_test(),
        "timing-skew поднял ε_test-алерт — recon не отличает движение цены от порчи данных"
    );
}

/// (2, GREEN) STALE-книга: best ушёл на МНОГО bps (наша книга протухла/десинк). Это РЕАЛЬНАЯ
/// проблема — толерантность НЕ смеет её глушить.
#[test]
fn stale_book_best_far_off_still_diverges() {
    let (lb, la) = dense(100);
    let local = book_from(lb, la);
    // reference: рынок ушёл на +2% (наша книга протухла на 2% — это десинк, не timing).
    let shift = (MID as f64 * 0.02) as i64;
    let rb = (1..=100)
        .map(|k| Level {
            price: MID + shift - k * TICK,
            size: 5 * 100_000_000,
        })
        .collect();
    let ra = (1..=100)
        .map(|k| Level {
            price: MID + shift + k * TICK,
            size: 5 * 100_000_000,
        })
        .collect();
    let reference = book_from(rb, ra);

    let out = reconcile(&local, &reference);
    assert!(
        out.best_price_diverged || out.divergence_bps >= EPS_TEST_BPS,
        "best разошёлся на 2% (протухшая книга/десинк) и recon смолчал — толерантность к skew \
         НЕ смеет глушить реальный десинк"
    );
}

/// (3, GREEN, C1-детектор) Внутри-полосный уровень УДАЛЁН (эвикция C1): best почти не двигается
/// (плотная книга), но СУММА ПОЛОСЫ теряет объём → divergence_bps ≥ ε_test. Ловится полосами, НЕ
/// best_price. Толерантность best НЕ смеет спрятать это.
#[test]
fn removed_in_band_level_diverges_by_band_sum() {
    let (rb, ra) = dense(100);
    let reference = book_from(rb.clone(), ra.clone());
    // local: удалены 40 внутри-полосных бид-уровней (2..=41) — как эвикция C1 within-band.
    let lb: Vec<Level> = rb
        .into_iter()
        .filter(|l| l.price < MID - 41 * TICK || l.price >= MID - TICK)
        .collect();
    let local = book_from(lb, ra);

    let out = reconcile(&local, &reference);
    assert!(
        out.divergence_bps >= EPS_TEST_BPS,
        "удалены 40 within-band bid-уровней (C1-класс порчи), а divergence_bps={} < ε_test={} — \
         recon пропустил реальную порчу книги (полосы обязаны её ловить)",
        out.divergence_bps,
        EPS_TEST_BPS
    );
}

/// (4, GREEN, TD-016 детектор) local несёт ФАНТОМНУЮ ликвидность within-band, которой нет в REST
/// (уровни, из-под которых цена ушла, не обнулены — TD-016). Суммы полос расходятся → recon ВЕРНО
/// ловит. Это РЕАЛЬНАЯ порча, а не шум представления — фикс НЕ смеет её глушить. Если после фикса
/// (1)+(feed-raw) recon всё ещё это видит на проде — фантом реален, предусловие = TD-016.
#[test]
fn phantom_volume_within_band_diverges() {
    let (rb, ra) = dense(100);
    let reference = book_from(rb.clone(), ra.clone());
    // local: те же уровни + удвоенный объём на within-band бидах (фантомная ликвидность).
    let lb: Vec<Level> = rb
        .into_iter()
        .map(|l| Level {
            price: l.price,
            size: l.size * 2,
        })
        .collect();
    let local = book_from(lb, ra);

    let out = reconcile(&local, &reference);
    assert!(
        out.divergence_bps >= EPS_TEST_BPS,
        "локальная книга несёт 2× within-band объём (фантомная ликвидность TD-016), а \
         divergence_bps={} < ε_test={} — recon обязан ловить фантом (это его назначение)",
        out.divergence_bps,
        EPS_TEST_BPS
    );
}
