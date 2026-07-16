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
use contracts::{EventKind, Venue};

use crate::metrics::Metrics;
use crate::recon::{reconcile, ReconThresholds};

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
    _local: &OrderBook,
    _reference: &OrderBook,
    _thr: &ReconThresholds,
    _venue: Venue,
    _symbol: &str,
    _metrics: &Metrics,
    mut _emit: impl FnMut(EventKind),
) -> bool {
    // Подсказка impl (engine-dev): let out = reconcile(local, reference);
    //   if out.exceeds_test() || out.exceeds_prod(thr) {
    //       let audit = out.to_audit(venue, symbol, ReconAction::Resynced);
    //       emit(EventKind::Sys(SysEvent::ReconDivergence(audit)));
    //       metrics.set_gauge("book_divergence_bps", &[("venue",..),("symbol",symbol)], out.divergence_bps);
    //       metrics.inc_counter("book_resync_total", &[("venue",..),("symbol",symbol)], 1);
    //       true
    //   } else { false }
    let _ = reconcile;
    todo!(
        "OPS-I-1 sink: divergence → emit Sys(ReconDivergence) + метрики; норма → false, без эмита"
    )
}
