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
const TICK: i64 = 1_000_000;

fn lvl(p: i64, s: i64) -> Level {
    Level { price: p, size: s }
}

fn full_book() -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = (1..=100)
        .map(|k| lvl(MID - k * TICK, 5 * 100_000_000))
        .collect();
    let asks: Vec<Level> = (1..=100)
        .map(|k| lvl(MID + k * TICK, 5 * 100_000_000))
        .collect();
    b.apply_snapshot(&bids, &asks);
    b
}

/// local без best bid (эвикция C1) — ask цел.
fn book_missing_best_bid() -> OrderBook {
    let mut b = OrderBook::new();
    let bids: Vec<Level> = (2..=100)
        .map(|k| lvl(MID - k * TICK, 5 * 100_000_000))
        .collect();
    let asks: Vec<Level> = (1..=100)
        .map(|k| lvl(MID + k * TICK, 5 * 100_000_000))
        .collect();
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
