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
use crate::recon::{reconcile, ReconThresholds};

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

/// Обработать один recon-снапшот.
///
/// - сверяет `local` с REST-`reference` (`ops::recon::reconcile`);
/// - при расхождении выше порога (`exceeds_test()` ИЛИ `exceeds_prod(thr)`): выполняется
///   принудительный ресинк (`ReconAction::Resynced`, FA §4), эмитится
///   `EventKind::Sys(SysEvent::ReconDivergence(audit))` через `emit`, обновляются метрики
///   (`book_divergence_bps{venue,symbol}` set, `book_resync_total{venue,symbol}` inc);
/// - в НОРМЕ (нет расхождения): НИЧЕГО не эмитится (alert only on divergence — канал не шумит).
///
/// Возвращает `true`, если событие эмитировано. `emit` — замыкание рекордера, шлющее в его
/// mpsc-канал (`JR-I-1`). Функция журнал НЕ трогает.
pub fn handle_recon_snapshot(
    local: &OrderBook,
    reference: &OrderBook,
    thr: &ReconThresholds,
    venue: Venue,
    symbol: &str,
    metrics: &Metrics,
    mut emit: impl FnMut(EventKind),
) -> bool {
    let out = reconcile(local, reference);

    // `exceeds_test()` (ε_test, не калибруется) ИЛИ `exceeds_prod(thr)` (ε_prod).
    // Любое из них — расхождение, требующее ресинка и аудита (`FA §4`: алерт + ресинк +
    // `Sys(ReconDivergence)`). `ReconAction::Resynced` — ресинк уже произведён на уровне
    // venue-фетчера (book.apply_snapshot(reference)); см. handoff M-09 task 2.
    if out.exceeds_test() || out.exceeds_prod(thr) {
        let audit = out.to_audit(venue, symbol, ReconAction::Resynced);
        emit(EventKind::Sys(SysEvent::ReconDivergence(audit)));

        let vlabel = venue_label(venue);
        metrics.set_gauge(
            "book_divergence_bps",
            &[("venue", vlabel), ("symbol", symbol)],
            out.divergence_bps,
        );
        metrics.inc_counter(
            "book_resync_total",
            &[("venue", vlabel), ("symbol", symbol)],
            1,
        );
        true
    } else {
        // НОРМА: алерт ТОЛЬКО on divergence. Канал не шумит (alert only on divergence;
        // `red_recon_sink::normal_book_is_silent` ловит no-op-impl).
        false
    }
}
