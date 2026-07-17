//! OPS-I-1 (оркестраторная часть) — обработка одного recon-снапшота: сверка → эмит доменного
//! события + метрики. `docs/fa/ops.md` §4.
//!
//! **Почему здесь, а не в recorder:** логика чистая (`reconcile` + эмит + метрики) и НЕ трогает
//! журнал — она принимает `emit`-замыкание, а не journal-handle. `OPS-I-6` сохранён (ops не
//! зависит от `journal`), а рекордер лишь передаёт замыкание, шлющее в СВОЙ mpsc-канал
//! (`JR-I-1`: журнал пишет только рекордер). Юнит-тестируется без живого `Recorder`.
//!
//! Единственный путь события в журнал — `EventKind::Sys(SysEvent::ReconDivergence)` через `emit`
//! (тот же `EventKind`-конверт, что у всех событий; никакого спец-пути).

use book::OrderBook;
use contracts::{EventKind, ReconAction, SysEvent, Venue};

use crate::metrics::Metrics;
use crate::recon::ReconDetector;

/// Канонические `venue`-labels для метрик §3 (согласованы с `venue_binance::VENUE_LABEL`,
/// `venue_binance_futures::VENUE_LABEL`). `hl` отдельным RED'ом; пока нет recon-фетчера
/// на HL (M-09 task 2 — binance/futures), `match` с `_ => "unknown"` страхует.
fn venue_label(v: Venue) -> &'static str {
    match v {
        Venue::Binance => "binance",
        Venue::BinanceFutures => "binance_futures",
        Venue::Hyperliquid => "hyperliquid",
    }
}

/// Обработать один recon-снапшот через ОКОННЫЙ детектор (`ops.md` §4.3, второй §8-провал).
///
/// STATEFUL: `detector` держит окно персистентности per (venue,symbol) (передаётся `&mut`, живёт в
/// оркестраторе рядом с `ReconBudget`). Логика:
/// - `detector.observe(local, reference)` → best-price (per-cycle, immediate) + объём near-touch в
///   окно; вердикт `alert` = best разошёлся ИЛИ заполненное окно держит `|signed_mean|` над порогом
///   (персистентная порча; churn mean→0 → тишина);
/// - гейдж `book_divergence_bps{venue,symbol}` обновляется КАЖДЫЙ цикл (наблюдаемость §3, не эмиссия);
/// - при `alert`: принудительный ресинк (`ReconAction::Resynced`), эмит
///   `EventKind::Sys(SysEvent::ReconDivergence(audit))`, `book_resync_total{venue,symbol}`++;
/// - иначе (churn/норма): событие НЕ эмитится (канал не шумит на здоровом рынке — §8-тишина).
///
/// Возвращает `true`, если событие эмитировано. `emit` — замыкание рекордера в его mpsc-канал
/// (`JR-I-1`). Функция журнал НЕ трогает (`OPS-I-6`).
pub fn handle_recon_snapshot(
    detector: &mut ReconDetector,
    local: &OrderBook,
    reference: &OrderBook,
    venue: Venue,
    symbol: &str,
    metrics: &Metrics,
    mut emit: impl FnMut(EventKind),
) -> bool {
    // (1) Вердикт детектора (best-price per-cycle + оконный объём).
    let verdict = detector.observe(local, reference);

    // (2) Гейдж `book_divergence_bps{venue,symbol}` — КАЖДЫЙ цикл (§3, наблюдаемость, не эмиссия).
    //     Это per-cycle магнитуда: на здоровом churn'е она будет скакать, и это ОК (мы хотим
    //     видеть churn); окно берёт на себя решение об эмиссии.
    metrics.set_gauge(
        "book_divergence_bps",
        &[("venue", venue_label(venue)), ("symbol", symbol)],
        verdict.gauge_divergence_bps,
    );

    // (3) Алерт → принудительный ресинк + Sys(ReconDivergence) + book_resync_total++.
    if verdict.alert {
        let audit = ReconDetector::verdict_to_audit(&verdict, venue, symbol, ReconAction::Resynced);
        emit(EventKind::Sys(SysEvent::ReconDivergence(audit)));
        metrics.inc_counter(
            "book_resync_total",
            &[("venue", venue_label(venue)), ("symbol", symbol)],
            1,
        );
        return true;
    }

    // (4) Тишина: churn (mean→0) или норма — НЕ шуметь (канал рекордера, §8).
    false
}
