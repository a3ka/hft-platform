//! RED OPS-I-1 LIVE-РЕЖИМ (sacred, architect-only) — recon валидирует БЛИЖНЮЮ книгу и НЕ флудит.
//!
//! §8 провал (2026-07-17): recon эмитил ReconDivergence на КАЖДОМ цикле здорового рынка
//! (`divergence_bps=2436..7754`, `best_price_diverged=true`) без инъекции порчи.
//!
//! КОРЕНЬ (измерено architect'ом 2026-07-17, `api.binance.com/api/v3/depth?limit=5000`, BTCUSDT):
//!   - REST-reference обрезан на 5000 уровней и достаёт **~1.1–1.7% от mid** (не глубже);
//!   - локальная книга реконструирована до **~60%** (WS-diff аккумуляция);
//!   - прежние `RECON_BANDS=[1.5%,3%,8%]` сравнивали суммы полос ТАМ, ГДЕ У REFERENCE НЕТ ДАННЫХ
//!     → local≫reference по построению → `divergence_bps` 2436..7754 = подпись АСИММЕТРИИ ГЛУБИНЫ,
//!     не бакетинга и не timing-skew;
//!   - `best_price_diverged = best != best` срабатывал на sub-bp timing-skew (best двигается тик-в-тик
//!     между WS-книгой и REST-моментом).
//!
//! Замер также опроверг «фикс сырой книги»: бакетинг (0.02%-сетка) даёт **0.0 bps** на полосах
//! ≤0.8% и `best_bid` бакет == сырой best_bid — сравнивать сырую книгу адаптера НЕ нужно.
//!
//! ДИЗАЙН (founder ★ 2026-07-17: near-book recon + отдельный трек для 6–60%):
//!   (a) `best_price_diverged` С ТОЛЕРАНТНОСТЬЮ `BEST_SKEW_BPS` (sub-bp skew — норма; много bps — десинк);
//!   (b) МЕЛКИЕ полосы `RECON_BANDS` (≤ REST-reach) + ПРОПУСК полосы, которую reference НЕ ДОСТаёт
//!       (`reference.max_reach_pct(side) < band` → полоса невалидируема, НЕ считается расхождением);
//!   (c) глубокие полосы 6–60% через REST НЕ верифицируемы (нет ground truth) — ОТДЕЛЬНЫЙ трек (TD-016).
//!
//! Эти оракулы моделируют LIVE-режим: РАЗНЫЕ представления/глубины/моменты, НЕ книгу-с-собой
//! (прежние `red_recon_sink::normal_book_is_silent`/`red_recon_wiring` сравнивали книгу С СОБОЙ →
//! green-unit/live-flood, класс TD-011/TD-016 — `.claude/rules/testing.md` «live-режим RED»).
//! Гейт-целостность (testing.md, 4 свойства): набор ПАДАЕТ и против no-skip-impl (§8-флуд, C),
//! и против over-skip-impl (пропустил near-book порчу, D/E), и против «пустой ref → тишина» (F).

use book::OrderBook;
use contracts::{Level, Side};
use ops::recon::{reconcile, RECON_BANDS};

const MID: i64 = 65_000_000_000_000; // $65k ×1e8
const UNIT: i64 = 100_000_000; // 1.0 объёма ×1e8

/// Уровень на расстоянии `pct` доли от mid (bid — ниже, ask — выше).
fn bid_at(pct: f64, size_units: i64) -> Level {
    Level {
        price: (MID as f64 * (1.0 - pct)) as i64,
        size: size_units * UNIT,
    }
}
fn ask_at(pct: f64, size_units: i64) -> Level {
    Level {
        price: (MID as f64 * (1.0 + pct)) as i64,
        size: size_units * UNIT,
    }
}
fn book_of(bids: Vec<Level>, asks: Vec<Level>) -> OrderBook {
    let mut b = OrderBook::new();
    b.apply_snapshot(&bids, &asks);
    b
}
/// Симметричная книга из списка (pct, size) на каждую сторону.
fn sym(levels: &[(f64, i64)]) -> OrderBook {
    let bids = levels.iter().map(|&(p, s)| bid_at(p, s)).collect();
    let asks = levels.iter().map(|&(p, s)| ask_at(p, s)).collect();
    book_of(bids, asks)
}

// ─────────────────────────────────────────────────────────────────────────────
// ГАРД ДИЗАЙНА: полосы recon должны быть МЕЛКИМИ (в пределах REST-reach ~1.1%).
// Регресс к 1.5/3/8% ⇒ структурный §8-флуд (reference туда не достаёт).
// ─────────────────────────────────────────────────────────────────────────────

/// `RECON_BANDS` не смеют превышать 0.8% — иначе сравниваем там, где REST-reference пуст (§8-флуд).
#[test]
fn recon_bands_are_shallow_within_rest_reach() {
    assert!(
        !RECON_BANDS.is_empty(),
        "RECON_BANDS пуст — recon не сравнивает полосы вообще"
    );
    for &band in &RECON_BANDS {
        assert!(
            band <= 0.008,
            "полоса {band} > 0.8%: REST limit=5000 достаёт лишь ~1.1–1.7% от mid, а глубокие полосы \
             сравнивают local(60%) с пустым reference → структурный флуд (§8-провал 2026-07-17)"
        );
    }
    // Должна быть хотя бы одна полоса в диапазоне [0.3%,0.5%], где REST заведомо есть данные —
    // иначе near-book recon вырождается в best-price-only и не ловит порчу ОБЪЁМА near-touch.
    assert!(
        RECON_BANDS.iter().any(|&b| (0.003..=0.005).contains(&b)),
        "нет ни одной полосы 0.3–0.5%: near-book recon обязан валидировать объём near-touch, \
         а не только цену"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (a) TIMING-SKEW ТОЛЕРАНТНОСТЬ — best двигается тик-в-тик, это НЕ порча.
// ─────────────────────────────────────────────────────────────────────────────

/// (A, ПАДАЕТ на текущей точной `!=`) best сдвинут на sub-bp (timing-skew между WS и REST) —
/// `best_price_diverged` обязан быть false, иначе recon флудит на каждом такте живого рынка.
#[test]
fn best_price_timing_skew_is_tolerated() {
    let near = [(0.0005, 5), (0.002, 5), (0.004, 5)];
    let local = sym(&near);
    // reference: тот же near-book, но best сдвинут на ~0.5 bp (несколько тиков — цена ушла за мс до REST).
    let shift = 0.00005; // 0.5 bp
    let reference = book_of(
        near.iter().map(|&(p, s)| bid_at(p + shift, s)).collect(),
        near.iter().map(|&(p, s)| ask_at(p + shift, s)).collect(),
    );
    let out = reconcile(&local, &reference);
    assert!(
        !out.best_price_diverged,
        "best сдвинут на ~0.5 bp (timing-skew) и recon счёл это порчей — на живом рынке best \
         двигается каждый такт, recon флудил бы ReconDivergence (§8-провал)"
    );
    assert!(
        !out.exceeds_test(),
        "sub-bp timing-skew поднял ε_test-алерт — recon не отличает движение цены от порчи данных"
    );
}

/// (B, GREEN) best ушёл на 0.1% (10 bp) — это НЕ мс-skew, а десинк/протухшая книга. Толерантность
/// НЕ смеет это глушить. Пиннит `BEST_SKEW_BPS < 10 bp` (иначе реальный десинк проглатывается).
#[test]
fn real_desync_best_moved_ten_bps_still_diverges() {
    let near = [(0.0005, 5), (0.002, 5), (0.004, 5)];
    let local = sym(&near);
    let shift = 0.001; // 10 bp — рынок ушёл, наша книга протухла
    let reference = book_of(
        near.iter().map(|&(p, s)| bid_at(p + shift, s)).collect(),
        near.iter().map(|&(p, s)| ask_at(p + shift, s)).collect(),
    );
    let out = reconcile(&local, &reference);
    assert!(
        out.best_price_diverged || out.exceeds_test(),
        "best разошёлся на 10 bp (десинк, не мс-skew), а recon смолчал — толерантность к skew \
         НЕ смеет глушить реальное расхождение best"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (b) DEPTH-AWARE — §8 корень: local ГЛУБОКАЯ, reference ОБРЕЗАН (REST 5000). Полоса за пределами
//     reference НЕ сравнивается (невалидируема), а НЕ считается расхождением.
// ─────────────────────────────────────────────────────────────────────────────

/// (C, ПАДАЕТ СЕЙЧАС — ЭТО §8-ФЛУД) local достаёт до 4%, reference обрезан на ~0.4% (REST limit).
/// Near-book (0–0.4%) СХОДИТСЯ. Полосы за пределами reference-reach ОБЯЗАНЫ пропускаться → recon
/// молчит. Текущий reconcile сравнивает все `RECON_BANDS` целиком → local(4%)≫reference(0.4%) на
/// глубокой полосе → `divergence_bps` огромный → флуд. Анти-плацебо: no-skip impl падает здесь.
#[test]
fn deep_local_vs_truncated_reference_does_not_flood() {
    // near-book 0–0.4% ИДЕНТИЧЕН у обоих (наша ближняя книга верна).
    let near = [(0.0005, 8), (0.002, 6), (0.004, 5)];
    // reference: только near-book, обрезан на 0.4% (как REST limit=5000 на ~1.1%, здесь масштаб теста).
    let reference = sym(&near);
    // local: тот же near-book + ГЛУБОКИЕ уровни, которых reference не видит (0.45%, 1%, 4%).
    let mut deep: Vec<(f64, i64)> = near.to_vec();
    deep.extend_from_slice(&[(0.0045, 50), (0.01, 200), (0.04, 800)]);
    let local = sym(&deep);

    let out = reconcile(&local, &reference);
    assert!(
        !out.exceeds_test(),
        "local глубже reference (REST обрезан) и recon счёл разницу ГЛУБОКИХ полос порчей \
         (divergence_bps={}, best={}) — полосы за пределами reference.max_reach_pct обязаны \
         ПРОПУСКАТЬСЯ, а не флудить (§8-корень: асимметрия глубины)",
        out.divergence_bps,
        out.best_price_diverged
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (b-анти-плацебо) В пределах reference-reach recon ОБЯЗАН ловить порчу — иначе over-skip.
// ─────────────────────────────────────────────────────────────────────────────

/// (D, GREEN, C1-детектор) within-reach уровень УДАЛЁН из local (эвикция C1 стирала живой уровень),
/// reference его имеет. Полоса 0.3% ⊂ reference-reach → сравнивается → расхождение. Толерантность/skip
/// НЕ смеют это спрятать. Анти-плацебо против «skip всё / всегда тихо».
#[test]
fn near_book_eviction_within_reach_diverges() {
    let reference = sym(&[(0.0005, 8), (0.002, 10), (0.004, 5)]);
    // local: near-book с УДАЛЁННЫМ уровнем 0.2% (в пределах reference-reach) — C1-порча.
    let local = sym(&[(0.0005, 8), (0.004, 5)]);
    let out = reconcile(&local, &reference);
    assert!(
        out.exceeds_test(),
        "within-reach уровень 0.2% удалён (C1-порча книги), а recon смолчал (divergence_bps={}) — \
         near-book полосы ⊂ reference-reach ОБЯЗАНЫ ловить порчу, skip не смеет заходить в reach",
        out.divergence_bps
    );
}

/// (E, GREEN, TD-016 near-touch детектор) local несёт ФАНТОМНЫЙ within-reach объём (уровень, из-под
/// которого цена ушла, не обнулён), reference — свежий, его нет. Полоса ⊂ reach → расхождение.
#[test]
fn near_touch_phantom_within_reach_diverges() {
    let reference = sym(&[(0.0005, 8), (0.002, 5), (0.004, 5)]);
    // local: тот же near-book, но 0.2% несёт 4× фантомного объёма (не обнулён после ухода цены).
    let local = sym(&[(0.0005, 8), (0.002, 20), (0.004, 5)]);
    let out = reconcile(&local, &reference);
    assert!(
        out.exceeds_test(),
        "local несёт 4× фантомного within-reach объёма (TD-016 near-touch), а recon смолчал \
         (divergence_bps={}) — near-book фантом обязан ловиться",
        out.divergence_bps
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (F) ПУСТОЙ reference — не тихий OK. REST вернул пустую книгу (halt/сбой) при живой local ⇒ сигнал.
// ─────────────────────────────────────────────────────────────────────────────

/// (F, GREEN) reference пуст (REST вернул пустоту), local жив. Все полосы пропускаются (reach=None),
/// но recon НЕ смеет отрапортовать «всё сошлось»: local имеет best, reference — нет ⇒ расхождение.
/// Анти-плацебо против «нет сравнимых полос → exceeds_test=false».
#[test]
fn empty_reference_is_not_silently_ok() {
    let local = sym(&[(0.0005, 8), (0.002, 5), (0.004, 5)]);
    let reference = OrderBook::new(); // пусто
    let out = reconcile(&local, &reference);
    assert!(
        out.exceeds_test(),
        "reference пуст (REST halt/сбой), local жив, а recon отрапортовал тишину — невалидируемость \
         (нет near-book у reference при живой local) обязана подниматься как расхождение best, \
         а не как «всё сошлось»"
    );
    // локальная сторона обязана быть достижимой (страховка фикстуры — reach у local есть).
    assert!(
        local.max_reach_pct(Side::Buy).is_some(),
        "фикстура-сетап не состоялась: у local нет глубины (тест бы прошёл вхолостую)"
    );
}
