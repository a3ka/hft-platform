//! RED DV-I-6 — order-flow faithfulness (sacred, architect-only) — M-32 Q2б.
//!
//! ЦЕЛЬ (M-32 Q2б): поток diff'а ВЕРЕН? Сделка на цене P объёмом S ДОЛЖНА сопровождаться убыванием
//! книги на P (дельта, уменьшающая/снимающая P) в seq-окне. Если поток верно отражает исполненный
//! flow около touch — это подтверждает семантику отмен и глубже (звено к доверию дальним полосам Q2а).
//!
//! Контракт (research-dev impl, `crates/research-cli/src/orderflow.rs` — расширение существующего):
//!   `research_cli::orderflow::consistency(events: &[FaithEvent], window_ms: i64) -> FaithReport`
//!   - `events` упорядочены по ts (как в журнале); чистый редьюсер;
//!   - поддерживает running price→size книгу из `Delta` (size=0 = remove);
//!   - на каждый `Trade` @P,S: consistent, если в (ts, ts+window_ms] приходит `Delta`, уменьшающая
//!     размер на P минимум на S (или снимающая P); иначе inconsistent (поток НЕ отразил филл);
//!   - `checked = consistent + inconsistent`; детерминировано.
//!
//! Анти-плацебо (ОБЕ стороны): заглушка «всегда consistent» → падает на mismatch-фикстуре;
//! «всегда inconsistent» → падает на match-фикстуре. compile-RED против отсутствия символа.

use contracts::{Level, Side};
use research_cli::orderflow::{consistency, FaithEvent};

const UNIT: i64 = 100_000_000;
const P: i64 = 64_000 * UNIT; // цена уровня/сделки

fn delta(ts_ms: i64, price: i64, size_units: i64) -> FaithEvent {
    FaithEvent::Delta {
        ts_ms,
        bids: vec![Level {
            price,
            size: size_units * UNIT,
        }],
        asks: vec![],
    }
}
fn trade(ts_ms: i64, price: i64, size_units: i64) -> FaithEvent {
    FaithEvent::Trade {
        ts_ms,
        price,
        side: Side::Sell, // taker sell бьёт по bid @P
        size: size_units * UNIT,
    }
}

// ── DV-I-6a: сделка сопровождается убыванием книги на P ⇒ consistent ─────────────────────────────
#[test]
fn dv_i_6_trade_with_book_decrement_is_consistent() {
    let events = vec![
        delta(1_000, P, 10), // книга: P=10
        trade(2_000, P, 4),  // исполнено 4 на P
        delta(2_500, P, 6),  // книга P: 10→6 (убыло 4) в окне ⇒ consistent
    ];
    let r = consistency(&events, 1_000);
    assert_eq!(r.checked, 1, "одна сделка проверена");
    assert_eq!(r.consistent, 1, "book-decrement на P в окне ⇒ consistent");
    assert_eq!(r.inconsistent, 0);
}

// ── DV-I-6b: сделка БЕЗ убывания книги на P ⇒ inconsistent (поток соврал) ────────────────────────
#[test]
fn dv_i_6_trade_without_book_decrement_is_inconsistent() {
    let events = vec![
        delta(1_000, P, 10), // книга: P=10
        trade(2_000, P, 4),  // исполнено 4 на P
        delta(2_500, P, 10), // P не убыл (10→10) — поток НЕ отразил филл
        delta(2_800, P + UNIT, 5), // другая цена — нерелевантно
    ];
    let r = consistency(&events, 1_000);
    assert_eq!(r.checked, 1);
    assert_eq!(
        r.inconsistent, 1,
        "нет book-decrement на P ⇒ inconsistent (заглушка 'всегда consistent' падает здесь)"
    );
    assert_eq!(r.consistent, 0);
}

// ── DV-I-6 множественность: одна согласована, другая нет ─────────────────────────────────────────
#[test]
fn dv_i_6_multiplicity_mixed() {
    let events = vec![
        delta(1_000, P, 10),
        trade(1_500, P, 4),
        delta(1_800, P, 6), // consistent
        delta(3_000, P, 8), // восстановили ликвидность
        trade(3_500, P, 5),
        delta(3_800, P, 8), // не убыл ⇒ inconsistent
    ];
    let r = consistency(&events, 1_000);
    assert_eq!(r.checked, 2, "две сделки проверены");
    assert_eq!(r.consistent, 1);
    assert_eq!(r.inconsistent, 1);
}

// ── DV-I-6 детерминизм ──────────────────────────────────────────────────────────────────────────
#[test]
fn dv_i_6_determinism() {
    let mk = || {
        vec![
            delta(1_000, P, 10),
            trade(2_000, P, 4),
            delta(2_500, P, 6),
        ]
    };
    assert_eq!(consistency(&mk(), 1_000), consistency(&mk(), 1_000));
}
