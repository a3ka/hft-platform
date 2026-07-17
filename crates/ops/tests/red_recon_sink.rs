//! RED OPS-I-1 sink (sacred, architect-only) — оркестрация recon через ОКОННЫЙ детектор: churn на
//! здоровом рынке → ТИШИНА (канал не шумит); персистентная порча / best-десинк → доменное событие
//! `EventKind::Sys(ReconDivergence)` в канал рекордера + метрики. Оркестраторный контракт (engine-dev impl).
//!
//! Тестируется БЕЗ живого `Recorder`: `emit`-замыкание собирает `EventKind` в `Vec`. Доказывает
//! `JR-I-1` (единственный путь — `EventKind::Sys(ReconDivergence)` через emit, не journal.append) и
//! `OPS-I-6` (ops журнал не трогает: sink принимает emit-замыкание + `&mut ReconDetector`, не journal-handle).
//!
//! ВТОРОЙ §8-провал (2026-07-17): прежний `normal_book_is_silent` сравнивал книгу С СОБОЙ (local==reference)
//! → всегда GREEN на юните, а на проде sink флудил `ReconDivergence` на КАЖДОМ цикле здорового рынка
//! (near-touch объём churn'ит между WS-книгой и async REST). Теперь sink STATEFUL (`ReconDetector` держит
//! окно per venue/symbol), а «норма» моделируется ПОСЛЕДОВАТЕЛЬНОСТЬЮ churn-циклов (два источника,
//! разные моменты), НЕ книгой-с-собой — `.claude/rules/testing.md` «RED двух источников → live-режим».
//!
//! Анти-плацебо: против `todo!()` — все падают; «всегда эмитить» валит churn-тишину;
//! «никогда не эмитить» валит persistent-emit И best-emit.

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
/// best-price расхождение → immediate emit (per-cycle, без окна).
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

/// CHURN на здоровом рынке → НИЧЕГО не эмитится за ВСЮ последовательность. Прежний дефект: sink
/// шумел на каждом такте. Знак per-cycle чередуется → окно усредняет в 0 → тишина.
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
        "sink эмитил {} recon-событий на ЗДОРОВОМ churn'ащем рынке (near-touch объём мигает знаком) — \
         ровно §8-флуд, который поймал reviewer. Оконный детектор обязан гасить churn в тишину",
        emitted.len()
    );
}

/// ПЕРСИСТЕНТНАЯ порча объёма (local держит −15% near-book каждый цикл) → РОВНО одно событие после
/// заполнения окна (best цел → `best_price_diverged=false`, `divergence_bps>0` = оконная магнитуда).
#[test]
fn persistent_divergence_emits_recondivergence() {
    let mut det = detector();
    let reference = reference();
    let metrics = Metrics::new();
    let mut emitted: Vec<EventKind> = Vec::new();

    for _ in 0..RECON_WINDOW {
        let local = scaled_book(0.85, 1.0); // персистентный дефицит
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
        !emitted.is_empty(),
        "персистентный дефицит объёма (−15% ВСЕ {RECON_WINDOW} циклов) не эмитировал событие — \
         C1-класс порчи не аудируется"
    );
    match emitted.last().expect("есть событие") {
        EventKind::Sys(SysEvent::ReconDivergence(audit)) => {
            assert_eq!(audit.venue, Venue::Binance);
            assert_eq!(audit.symbol, "BTCUSDT");
            assert!(
                !audit.best_price_diverged,
                "объёмная персистентная порча помечена как best-расхождение — офлайн спутает класс \
                 (best цел, разошёлся ОБЪЁМ)"
            );
            assert!(
                audit.divergence_bps > 0,
                "аудит оконной порчи не несёт магнитуду (divergence_bps=0) — офлайн не оценит тяжесть"
            );
        }
        other => panic!("не Sys(ReconDivergence): {other:?}"),
    }
}

/// Best-price десинк (пропал best bid) → immediate emit УЖЕ на первом цикле (окно не нужно для best).
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
         обязан быть per-cycle, без ожидания окна",
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

/// Персистентная порча → метрики реально обновлены: `book_divergence_bps` set, `book_resync_total`
/// инкрементирован (обе с labels venue/symbol). No-op метрики валят тест.
#[test]
fn divergence_updates_metrics() {
    let mut det = detector();
    let reference = reference();
    let metrics = Metrics::new();

    for _ in 0..RECON_WINDOW {
        let local = scaled_book(0.85, 1.0);
        handle_recon_snapshot(
            &mut det,
            &local,
            &reference,
            Venue::Binance,
            "BTCUSDT",
            &metrics,
            |_| {},
        );
    }

    let text = metrics.prometheus_text();
    let div = text
        .lines()
        .find(|l| l.starts_with("book_divergence_bps") && l.contains("symbol=\"BTCUSDT\""))
        .expect("book_divergence_bps{symbol=BTCUSDT} не выведена");
    let resync = text
        .lines()
        .find(|l| l.starts_with("book_resync_total") && l.contains("symbol=\"BTCUSDT\""))
        .expect("book_resync_total{symbol=BTCUSDT} не выведена после ресинка");
    let val = |l: &str| {
        l.split_whitespace()
            .last()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
    };
    assert!(
        val(div) > 0,
        "book_divergence_bps не обновлён (0) после персистентного расхождения"
    );
    assert!(
        val(resync) >= 1,
        "book_resync_total не инкрементирован после ресинка"
    );
}
