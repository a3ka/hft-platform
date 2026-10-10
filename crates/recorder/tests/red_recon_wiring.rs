//! RED M-09 recon WIRING (sacred, architect-only) — local-книга собирается feeder'ом из потока,
//! НЕ остаётся пустой. Ловит заглушку, где `books` никогда не заполняется → recon сравнивает
//! reference с ПУСТОЙ книгой → ложное `ReconDivergence` на каждый снапшот (не для прода).
//!
//! Инвариант: книга, собранная feeder'ом ИЗ ТОГО ЖЕ снапшота, что и reference, НЕ расходится с ним.
//! Анти-плацебо: против заглушки (feeder = `todo!()` / books пусты) `reconcile(пустая, reference)`
//! даёт `exceeds_test()==true` → тест падает. Против рабочего feeder'а local==reference → тишина.
//! Это ровно тот класс «GREEN sink-логика, но wiring кормит пустоту», что sink-RED не ловит.

use book::OrderBook;
use contracts::{EventKind, Level, MdPayload, Venue};
use ops::recon::reconcile;
use recorder::recon_loop::{apply_md_to_books, ReconBooks};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const MID: i64 = 65_000_000_000_000;
const TICK: i64 = 1_000_000;

fn levels() -> (Vec<Level>, Vec<Level>) {
    let bids = (1..=50)
        .map(|k| Level {
            price: MID - k * TICK,
            size: 5 * 100_000_000,
        })
        .collect();
    let asks = (1..=50)
        .map(|k| Level {
            price: MID + k * TICK,
            size: 5 * 100_000_000,
        })
        .collect();
    (bids, asks)
}

/// Книга, собранная feeder'ом из L2Snapshot, СХОДИТСЯ с reference того же снапшота (recon молчит).
#[tokio::test]
async fn feeder_built_local_book_matches_reference() {
    let (bids, asks) = levels();
    let books: ReconBooks = Arc::new(Mutex::new(HashMap::new()));

    // Тот же снапшот, что уйдёт в reference — но через ПОТОК событий (как в проде: L2Snapshot).
    let ev = EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: bids.clone(),
            asks: asks.clone(),
            ts_exch_ms: 1_752_000_000_000,
        },
    );
    apply_md_to_books(&books, &ev).await;

    // local ОБЯЗАН быть заполнен feeder'ом (не пустая заглушка).
    let local = {
        let map = books.lock().await;
        map.get(&(Venue::Binance, "BTCUSDT".to_string()))
            .cloned()
            .expect(
            "feeder не заполнил live-книгу — recon сравнивал бы reference с ПУСТОТОЙ (заглушка)",
        )
    };

    // reference — тот же снапшот напрямую в книгу.
    let mut reference = OrderBook::new();
    reference.apply_snapshot(&bids, &asks);

    let out = reconcile(&local, &reference);
    assert!(
        !out.exceeds_test(),
        "книга из feeder'а разошлась с reference того же снапшота (divergence_bps={}, best={}) — \
         значит local пуст/неверно собран; в проде recon флудил бы ложным ReconDivergence каждые 5 мин",
        out.divergence_bps,
        out.best_price_diverged
    );
}

/// Feeder игнорирует не-L2 события (Trade) — книга не появляется/не портится от них.
#[tokio::test]
async fn feeder_ignores_non_l2_events() {
    let books: ReconBooks = Arc::new(Mutex::new(HashMap::new()));
    let trade = EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: MID,
            size: 100,
            side: contracts::Side::Buy,
            ts_exch_ms: 1_752_000_000_000,
        },
    );
    apply_md_to_books(&books, &trade).await;
    let map = books.lock().await;
    assert!(
        map.get(&(Venue::Binance, "BTCUSDT".to_string())).is_none(),
        "Trade-событие создало/изменило live-книгу — feeder обязан применять ТОЛЬКО L2Snapshot"
    );
}
