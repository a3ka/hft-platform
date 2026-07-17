//! Recon-loop в рекордере (M-09) — ИЗОЛЯЦИЯ. Скелет (architect: сигнатуры + `todo!()`).
//!
//! Recon добавляет REST-трафик и сравнение книг — это может паниковать (parse, книга, бан). Но
//! рекордер — единственный писатель журнала (`JR-I-1`) и работает 24/7: **recon-сбой НЕ смеет
//! останавливать append**. Поэтому recon исполняется ИЗОЛИРОВАННЫМ таском (по образцу venue-
//! супервизора): паника внутри поймана, не пробрасывается в writer-стек.
//!
//! Оркестраторная логика (сверка → эмит `Sys(ReconDivergence)` через канал рекордера + метрики) —
//! в `ops::sink::handle_recon_snapshot` (чистая, без journal, `OPS-I-6`). Здесь — только запуск и
//! изоляция; событие идёт в тот же `mpsc::Sender<EventKind>`, что и всё остальное (`JR-I-1`).

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use book::OrderBook;
use contracts::{EventKind, Venue};
use tokio::sync::Mutex;

/// Live-книги recon per (venue, symbol) — локальное состояние для сверки с REST-reference.
pub type ReconBooks = Arc<Mutex<HashMap<(Venue, String), OrderBook>>>;

/// Обновить live-книгу recon из потока событий (BOOKS-FEEDER): `Md(L2Snapshot)` заменяет книгу
/// `(venue, symbol)`; прочее (`Trade`/`Funding`/`OpenInterest`/…) ИГНОРИРУЕТСЯ.
///
/// **Без этого feeder'а local-книга всегда ПУСТА** → `reconcile(пустая, reference)` даёт
/// `exceeds_test() == true` на ЛЮБОЙ непустой reference ⇒ recon флудит ложным `ReconDivergence`
/// на каждый снапшот. Сверка становится бессмысленной (сравниваем реальность с пустотой).
/// RED `red_recon_wiring.rs` ловит именно это: книга, собранная feeder'ом из того же снапшота,
/// что и reference, НЕ смеет расходиться.
pub async fn apply_md_to_books(_books: &ReconBooks, _ev: &EventKind) {
    todo!(
        "M-09 books-feeder: Md(L2Snapshot) → books[(venue,symbol)].apply_snapshot(bids,asks); \
         не-L2 игнорировать. Без этого recon сравнивает reference с ПУСТОЙ книгой (ложное расхождение)"
    )
}

/// Запустить recon-итерацию ИЗОЛИРОВАННО. Паника/ошибка внутри `f` поймана и НЕ пробрасывается
/// наружу — append-цикл рекордера не затрагивается (`JR-I-1`, 24/7). Возвращает `JoinHandle`;
/// `.await` на нём отдаёт `Err(JoinError)` при панике, но НЕ разворачивает стек вызывающего.
pub fn spawn_recon_isolated<F, Fut>(f: F) -> tokio::task::JoinHandle<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    // `tokio::spawn` гарантирует, что паника внутри `async`-блока поймана рантаймом:
    // задача завершается с `JoinError`, а НЕ разворачивает стек вызывающего. Это и есть
    // изоляция `JR-I-1` (writer живёт 24/7, recon-сбой не смеет его задеть). См. RED
    // `red_recon_loop::recon_panic_does_not_stop_append`.
    tokio::spawn(async move {
        f().await;
    })
}
