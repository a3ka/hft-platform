//! RED OPS-I-1 sink ПОД B2 (sacred, architect-only) — оркестрация recon: рантайм эмитит `Sys` ТОЛЬКО
//! на best-price расхождении; персистентный ОБЪЁМ (даже ≫ ε_max) в рантайме МОЛЧИТ. Оркестраторный
//! контракт (engine-dev impl).
//!
//! B2 ПРИНЯТ founder ★ 2026-07-18 (`docs/fa/ops.md` §4.3.2). ТРЕТИЙ §8-провал показал: оконное знаковое
//! среднее near-touch ОБЪЁМА НЕ сходится к 0 на живом рынке (систематический WS(T1)-vs-REST(T2) bias,
//! 103..747 bps, в т.ч. на нетронутом BinanceFutures). Объёмная сверка снята из рантайма → офлайн-трек.
//! Рантайм-alert ⟺ best_price_diverged (best-price §8-подтверждён: healthy 0, injection 6× best=true).
//!
//! Тестируется БЕЗ живого `Recorder`: `emit`-замыкание собирает `EventKind` в `Vec`. Доказывает
//! `JR-I-1` (единственный путь — `EventKind::Sys(ReconDivergence)` через emit, не journal.append) и
//! `OPS-I-6` (ops журнал не трогает: sink принимает emit-замыкание + `&mut ReconDetector`, не journal-handle).
//!
//! Анти-плацебо В ОБЕ СТОРОНЫ: против `todo!()` — все падают; персистентный-объём→тишина ВАЛИТ
//! window-active impl (текущий прод-код эмитил на персистентном объёме — §8-флуд B); best-desync→эмит
//! ВАЛИТ always-silent impl («заглушить всё» запрещено).

use book::OrderBook;
use contracts::{EventKind, Level, SysEvent, Venue};
use ops::metrics::Metrics;
use ops::recon::{ReconDetector, ReconThresholds, EPS_PROD_DEFAULT_BPS, RECON_WINDOW};
use ops::sink::handle_recon_snapshot;

const MID: i64 = 65_000_000_000_000;
const UNIT: i64 = 100_000_000; // 1.0 ×1e8
const BASE: f64 = 100.0;

/// Уровни на 0.05..0.55% от mid → reach≈0.55%, покрывает полосы recon (0.1/0.3/0.5%).
const PCTS: [f64; 6] = [0.0005, 0.0015, 0.0025, 0.0035, 0.0045, 0.0055];

/// Книга с масштабом объёма (цены не меняются → best-price идентичен: изолируем ОБЪЁМ).
fn scaled_book(bid_scale: f64, ask_scale: f64) -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = PCTS
        .iter()
        .map(|&p| Level {
            price: (MID as f64 * (1.0 - p)) as i64,
            size: (BASE * bid_scale).round() as i64 * UNIT,
        })
        .collect();
    let asks: Vec<Level> = PCTS
        .iter()
        .map(|&p| Level {
            price: (MID as f64 * (1.0 + p)) as i64,
            size: (BASE * ask_scale).round() as i64 * UNIT,
        })
        .collect();
    b.apply_snapshot(&bids, &asks);
    b
}

fn reference() -> OrderBook {
    scaled_book(1.0, 1.0)
}

/// local без best bid (эвикция C1) — 0.05% уровень удалён, ask цел. best уходит на 0.15% (>skew) →
/// best-price расхождение → immediate emit (per-cycle, рантайм-путь под B2).
fn book_missing_best_bid() -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = PCTS
        .iter()
        .skip(1)
        .map(|&p| Level {
            price: (MID as f64 * (1.0 - p)) as i64,
            size: BASE as i64 * UNIT,
        })
        .collect();
    let asks: Vec<Level> = PCTS
        .iter()
        .map(|&p| Level {
            price: (MID as f64 * (1.0 + p)) as i64,
            size: BASE as i64 * UNIT,
        })
        .collect();
    b.apply_snapshot(&bids, &asks);
    b
}

fn detector() -> ReconDetector {
    ReconDetector::new(ReconThresholds::new(EPS_PROD_DEFAULT_BPS).expect("thr"))
}

/// CHURN на здоровом рынке → НИЧЕГО не эмитится. Best-цена цела (масштаб — по объёму) → под B2 путь
/// объёма не эмитит вовсе → тишина. Прежний дефект: sink шумел на каждом такте.
#[test]
fn churn_sequence_is_silent() {
    let mut det = detector();
    let reference = reference();
    let metrics = Metrics::new();
    let mut emitted: Vec<EventKind> = Vec::new();

    for i in 0..RECON_WINDOW * 2 {
        let bid_scale = if i % 2 == 0 { 1.15 } else { 0.85 };
        let local = scaled_book(bid_scale, 1.0);
        handle_recon_snapshot(
            &mut det,
            &local,
            &reference,
            Venue::Binance,
            "BTCUSDT",
            &metrics,
            |ev| emitted.push(ev),
        );
    }
    assert!(
        emitted.is_empty(),
        "sink эмитил {} recon-событий на ЗДОРОВОМ churn'ащем рынке — под B2 объёмный путь не эмитит вовсе",
        emitted.len()
    );
}

/// (B2, ПАДАЕТ против window-active impl) ПЕРСИСТЕНТНЫЙ объёмный дефицит (−15% ≫ ε_max) 2×`RECON_WINDOW`
/// циклов — прод-класс, флудивший §8 (best=false). Под B2 sink МОЛЧИТ: объёмная сверка снята из рантайма
/// → офлайн. Инверсия снятого `persistent_divergence_emits_recondivergence` (тот требовал эмит).
#[test]
fn persistent_volume_sequence_does_not_emit() {
    let mut det = detector();
    let reference = reference();
    let metrics = Metrics::new();
    let mut emitted: Vec<EventKind> = Vec::new();

    for _ in 0..RECON_WINDOW * 2 {
        let local = scaled_book(0.85, 1.0); // персистентный дефицит, best цел
        handle_recon_snapshot(
            &mut det,
            &local,
            &reference,
            Venue::Binance,
            "BTCUSDT",
            &metrics,
            |ev| emitted.push(ev),
        );
    }
    assert!(
        emitted.is_empty(),
        "sink эмитил {} событий на ПЕРСИСТЕНТНОМ объёмном дефиците (−1500 bps ≫ ε_max, 2×{RECON_WINDOW} \
         циклов, best цел) — это §8-флуд B, который B2 удаляет. Под B2 рантайм эмитит ⟺ best-расхождение; \
         объёмная порча аудируется офлайн-треком над записанной книгой, не рантайм-`Sys` (window-active \
         impl не удалён)",
        emitted.len()
    );
}

/// Best-price десинк (пропал best bid) → immediate emit УЖЕ на первом цикле (рантайм-путь под B2).
#[test]
fn best_desync_emits_immediately() {
    let mut det = detector();
    let reference = reference();
    let metrics = Metrics::new();
    let mut emitted: Vec<EventKind> = Vec::new();

    let local = book_missing_best_bid();
    let did = handle_recon_snapshot(
        &mut det,
        &local,
        &reference,
        Venue::Binance,
        "BTCUSDT",
        &metrics,
        |ev| emitted.push(ev),
    );
    assert!(
        did && emitted.len() == 1,
        "пропавший best bid не дал immediate emit на первом цикле (did={did}, emitted={}) — best-путь \
         обязан быть per-cycle (рантайм-триггер под B2)",
        emitted.len()
    );
    match &emitted[0] {
        EventKind::Sys(SysEvent::ReconDivergence(audit)) => assert!(
            audit.best_price_diverged,
            "аудит не пометил порчу best — офлайн не отличит best-десинк от объёмного дрейфа"
        ),
        other => panic!("не Sys(ReconDivergence): {other:?}"),
    }
}

/// Best-десинк → метрики реально обновлены: `book_resync_total` инкрементирован (recon-эмиссия) +
/// `book_divergence_bps` гейдж выведен (обе с labels venue/symbol). No-op метрики валят тест. Под B2
/// счётчик ресинков растёт на BEST-расхождении (объём не эмитит), а гейдж обновляется каждый цикл.
#[test]
fn metrics_updated_on_best_resync() {
    let mut det = detector();
    let reference = reference();
    let metrics = Metrics::new();

    // best-десинк (пропал best bid) → эмиссия + ресинк; гейдж обновляется в этом же цикле.
    let local = book_missing_best_bid();
    let did = handle_recon_snapshot(
        &mut det,
        &local,
        &reference,
        Venue::Binance,
        "BTCUSDT",
        &metrics,
        |_| {},
    );
    assert!(
        did,
        "best-десинк не эмитировал — метрика ресинка не сможет обновиться"
    );

    let text = metrics.prometheus_text();
    let div = text
        .lines()
        .find(|l| l.starts_with("book_divergence_bps") && l.contains("symbol=\"BTCUSDT\""))
        .expect("book_divergence_bps{symbol=BTCUSDT} не выведена (гейдж наблюдаемости, §3)");
    let resync = text
        .lines()
        .find(|l| l.starts_with("book_resync_total") && l.contains("symbol=\"BTCUSDT\""))
        .expect("book_resync_total{symbol=BTCUSDT} не выведена после best-ресинка");
    let val = |l: &str| {
        l.split_whitespace()
            .last()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(-1)
    };
    assert!(
        val(div) >= 0,
        "book_divergence_bps не выведен как число (гейдж наблюдаемости обязан присутствовать)"
    );
    assert!(
        val(resync) >= 1,
        "book_resync_total не инкрементирован после best-ресинка (C1-класс P0-метрика §7.1)"
    );
}
