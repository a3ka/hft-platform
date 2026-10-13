//! Recon-loop в рекордере (M-09) — ИЗОЛЯЦИЯ + BOOKS-FEEDER.
//!
//! Recon добавляет REST-трафик и сравнение книг — это может паниковать (parse, книга, бан). Но
//! рекордер — единственный писатель журнала (`JR-I-1`) и работает 24/7: **recon-сбой НЕ смеет
//! останавливать append**. Поэтому recon исполняется ИЗОЛИРОВАННЫМ таском (по образцу venue-
//! супервизора): паника внутри поймана, не пробрасывается в writer-стек.
//!
//! Books-feeder (`apply_md_to_books`) перекладывает `Md(L2Snapshot)` в `ReconBooks` — без него
//! live-книга ПУСТА и `reconcile(пустая, reference)` флудит ложным `ReconDivergence` (класс
//! TD-011/TD-016, выловлен reviewer'ом). Trade/Funding/OpenInterest/… — не-L2 события —
//! ИГНОРИРУЮТСЯ: книгу двигает только полный снимок стакана (Binance @depth, HL l2Book).
//!
//! Оркестраторная логика (сверка → эмит `Sys(ReconDivergence)` через канал рекордера + метрики) —
//! в `ops::sink::handle_recon_snapshot` (чистая, без journal, `OPS-I-6`). Здесь — только запуск и
//! изоляция + наполнение live-книги; событие идёт в тот же `mpsc::Sender<EventKind>`, что и всё
//! остальное (`JR-I-1`).

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use book::OrderBook;
use contracts::{EventKind, MdPayload, Venue};
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
///
/// **Контракт:**
/// - `EventKind::Md(MdEvent { payload: L2Snapshot { bids, asks, .. }, venue, symbol, .. })` →
///   `books.lock().await.entry((venue, symbol)).or_default().apply_snapshot(bids, asks)`;
/// - всё остальное (`Sys`, `Ord`, `Risk`, `Ctl`, `Md(Trade)`, `Md(Funding)`, `Md(OpenInterest)`,
///   `Md(MarginRate)`, `Md(Liquidation)`, `Md(…другое)`) → no-op, книги не трогаем.
///
/// Детерминизм: `apply_snapshot` — детерминированная функция (BTreeMap-replace по (price→size));
/// при одинаковой последовательности `L2Snapshot`-ов `books` приходит в одно и то же состояние
/// (DET-I-1 на уровне feeder'а). Порядок применения — порядок поступления событий в feeder-таск.
///
/// Не-async по природе (lock + map-mutation), но `async` для единообразия с оркестратором
/// (lock.await под `tokio::sync::Mutex`). При contention (orchestrator читает параллельно) — lock
/// удерживается ОДНОЙ строкой кода (insert+apply), contention в наносекундах; на живом потоке
/// L2 @100-1000ms lock-окно незначимо vs. оркестратор-цикл (5 мин REST cadence).
pub async fn apply_md_to_books(books: &ReconBooks, ev: &EventKind) {
    // (1) Pattern-match ТОЛЬКО на L2Snapshot (sacred contract — RED `red_recon_wiring`).
    //     Trade/Funding/OI/… — НЕ двигают книгу (Binance @depth и HL l2Book шлют ПОЛНЫЙ снимок;
    //     trade-by-trade книгу не строим, это out-of-scope M-09).
    let (venue, symbol, bids, asks) = match ev {
        EventKind::Md(md) => match &md.payload {
            MdPayload::L2Snapshot { bids, asks, .. } => (md.venue, md.symbol.clone(), bids, asks),
            _ => return, // не-L2 → игнор (Trade/Funding/OI/Liquidation/MarginRate)
        },
        _ => return, // не-Md → игнор (Sys/Ord/Risk/Ctl)
    };

    // (2) Apply. Лок-корткий: entry + одно дерево-replace; не держим lock через await-точки.
    let mut map = books.lock().await;
    map.entry((venue, symbol))
        .or_insert_with(OrderBook::new)
        .apply_snapshot(bids, asks);
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
