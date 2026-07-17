//! RED OPS-I-1 sink (sacred, architect-only) — обработка recon-снапшота: расхождение → доменное
//! событие в канал рекордера + метрики; норма → тишина. Оркестраторный контракт (engine-dev impl).
//!
//! Тестируется БЕЗ живого `Recorder`: `emit`-замыкание собирает `EventKind` в `Vec`. Это доказывает
//! `JR-I-1` (единственный путь — `EventKind::Sys(ReconDivergence)` через emit, не journal.append) и
//! `OPS-I-6` (ops журнал не трогает). Чек-лист testing.md: деградированный вход (порча best bid),
//! норма (нет шума), метрики реально меняются.
//!
//! Анти-плацебо: против `todo!()` — все падают; «всегда эмитить» валит `normal_book_is_silent`;
//! «никогда не эмитить» валит `divergence_emits_recondivergence`.

use book::OrderBook;
use contracts::{EventKind, Level, SysEvent, Venue};
use ops::metrics::Metrics;
use ops::recon::{ReconThresholds, EPS_PROD_DEFAULT_BPS};
use ops::sink::handle_recon_snapshot;

const MID: i64 = 65_000_000_000_000;
const UNIT: i64 = 100_000_000; // 1.0 ×1e8

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

/// Уровни на 0.05..0.55% от mid (near-book redesign 2026-07-17): достают до полос recon (0.1/0.3/0.5%),
/// reach≈0.55%. НЕ ±100 тиков (±0.0015%) — те были бы целиком ВНУТРИ первой полосы и с новым
/// skip-за-reach правилом дали бы ложную тишину.
const PCTS: [f64; 6] = [0.0005, 0.0015, 0.0025, 0.0035, 0.0045, 0.0055];

fn full_book() -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = PCTS.iter().map(|&p| bid_at(p, 5)).collect();
    let asks: Vec<Level> = PCTS.iter().map(|&p| ask_at(p, 5)).collect();
    b.apply_snapshot(&bids, &asks);
    b
}

/// local без best bid (эвикция C1) — 0.05% уровень удалён, ask цел. best уходит на 0.15% (>skew),
/// near-touch полоса теряет объём → расхождение.
fn book_missing_best_bid() -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = PCTS.iter().skip(1).map(|&p| bid_at(p, 5)).collect();
    let asks: Vec<Level> = PCTS.iter().map(|&p| ask_at(p, 5)).collect();
    b.apply_snapshot(&bids, &asks);
    b
}

/// Расхождение → РОВНО одно `EventKind::Sys(ReconDivergence(audit))` в канал (emit), с пометкой
/// порчи best. Тот же `EventKind`-конверт, что у всех событий (JR-I-1, единственный путь).
#[test]
fn divergence_emits_recondivergence() {
    let local = book_missing_best_bid();
    let reference = full_book();
    let thr = ReconThresholds::new(EPS_PROD_DEFAULT_BPS).expect("thr");
    let metrics = Metrics::new();

    let mut emitted: Vec<EventKind> = Vec::new();
    let did = handle_recon_snapshot(
        &local,
        &reference,
        &thr,
        Venue::Binance,
        "BTCUSDT",
        &metrics,
        |ev| emitted.push(ev),
    );

    assert!(
        did,
        "расхождение (пропал best bid) не эмитировало событие — C1-класс не аудируется"
    );
    assert_eq!(
        emitted.len(),
        1,
        "ожидалось РОВНО одно recon-событие, получено {}",
        emitted.len()
    );
    match &emitted[0] {
        EventKind::Sys(SysEvent::ReconDivergence(audit)) => {
            assert_eq!(audit.venue, Venue::Binance);
            assert_eq!(audit.symbol, "BTCUSDT");
            assert!(
                audit.best_price_diverged,
                "аудит не пометил порчу best — офлайн не отличит порчу от шума дальних полос"
            );
        }
        other => {
            panic!("не Sys(ReconDivergence): {other:?} — recon обязан идти тем же EventKind-путём")
        }
    }
}

/// Норма (local == reference) → НИЧЕГО не эмитится (alert only on divergence; канал не шумит).
#[test]
fn normal_book_is_silent() {
    let local = full_book();
    let reference = full_book();
    let thr = ReconThresholds::new(EPS_PROD_DEFAULT_BPS).expect("thr");
    let metrics = Metrics::new();

    let mut emitted: Vec<EventKind> = Vec::new();
    let did = handle_recon_snapshot(
        &local,
        &reference,
        &thr,
        Venue::Binance,
        "BTCUSDT",
        &metrics,
        |ev| emitted.push(ev),
    );
    assert!(
        !did,
        "здоровые книги подняли recon-событие — журнал засоряется ложными ReconDivergence"
    );
    assert!(
        emitted.is_empty(),
        "в норме канал обязан быть пуст, получено {}",
        emitted.len()
    );
}

/// Расхождение → метрики реально обновлены: `book_divergence_bps` установлен, `book_resync_total`
/// инкрементирован (обе с labels venue/symbol). No-op метрики валят тест (значения не появятся).
#[test]
fn divergence_updates_metrics() {
    let local = book_missing_best_bid();
    let reference = full_book();
    let thr = ReconThresholds::new(EPS_PROD_DEFAULT_BPS).expect("thr");
    let metrics = Metrics::new();

    handle_recon_snapshot(
        &local,
        &reference,
        &thr,
        Venue::Binance,
        "BTCUSDT",
        &metrics,
        |_| {},
    );

    let text = metrics.prometheus_text();
    let div = text
        .lines()
        .find(|l| l.starts_with("book_divergence_bps") && l.contains("symbol=\"BTCUSDT\""))
        .expect("book_divergence_bps{symbol=BTCUSDT} не выведена после расхождения");
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
        "book_divergence_bps не обновлён (0) после реального расхождения"
    );
    assert!(
        val(resync) >= 1,
        "book_resync_total не инкрементирован после ресинка"
    );
}
