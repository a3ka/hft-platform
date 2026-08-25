//! RED M-30 GD-I-1..6 (sacred, architect-only) — gap-detection по update-id chaining, fail-closed.
//!
//! `OrderBook::apply_l2delta(...) -> ContinuityStatus`: дельты ЧЕЙНЯТСЯ (спот `U==prev.u+1`, фьючерс
//! `pu==prev.u`); разрыв → `Gap` + книга `stale` + дельта НЕ применена (fail-closed, как риск-слой);
//! выход из stale — только `apply_snapshot` (ресинк). Закрывает дыру M-29 (sequencing не валидировался).
//!
//! COMPILE-RED: `apply_l2delta`/`ContinuityStatus`/`is_stale` ещё НЕ существуют. engine-dev добавляет →
//! GREEN. Анти-плацебо: impl, применяющий gap-дельту → книга двинулась → падение GD-I-2/4.

use book::{Books, ContinuityStatus, OrderBook};
use contracts::{Level, MdEvent, MdPayload, Side, Venue};

fn lvl(price: i64, size: i64) -> Level {
    Level { price, size }
}

/// Свежая книга со снапшотом (last_final сброшен, не stale).
fn seeded() -> OrderBook {
    let mut b = OrderBook::new();
    b.apply_snapshot(&[lvl(100, 5)], &[lvl(101, 4)]);
    b
}

#[test]
fn bootstrap_first_delta() {
    // GD-I-5: первая дельта после снапшота (last_final==None) → Applied, чейн заведён.
    let mut b = seeded();
    let st = b.apply_l2delta(&[lvl(100, 9)], &[], 5, 5, None);
    assert_eq!(
        st,
        ContinuityStatus::Applied,
        "bootstrap-дельта применяется"
    );
    assert_eq!(b.size_at(Side::Buy, 100), 9, "книга обновлена");
    assert!(!b.is_stale(), "не stale");
}

#[test]
fn spot_contiguous_applies() {
    // GD-I-1: спот U==prev.u+1 → Applied.
    let mut b = seeded();
    assert_eq!(
        b.apply_l2delta(&[lvl(100, 9)], &[], 5, 5, None),
        ContinuityStatus::Applied
    );
    let st = b.apply_l2delta(&[lvl(99, 2)], &[], 6, 6, None); // U=6 == last(5)+1
    assert_eq!(st, ContinuityStatus::Applied, "спот-чейн непрерывен");
    assert_eq!(b.size_at(Side::Buy, 99), 2, "вторая дельта применена");
    assert!(!b.is_stale());
}

#[test]
fn spot_gap_fail_closed() {
    // GD-I-2: спот U != prev.u+1 → Gap, книга НЕ тронута разорванной дельтой, stale.
    let mut b = seeded();
    b.apply_l2delta(&[lvl(100, 9)], &[], 5, 5, None); // last_final=5
    let st = b.apply_l2delta(&[lvl(50, 999)], &[], 8, 8, None); // U=8 != 6 → GAP
    assert_eq!(st, ContinuityStatus::Gap, "разрыв спот-чейна детектирован");
    assert_eq!(
        b.size_at(Side::Buy, 50),
        0,
        "fail-closed: gap-дельта НЕ применена (нет 50)"
    );
    assert_eq!(
        b.size_at(Side::Buy, 100),
        9,
        "книга осталась в pre-gap состоянии"
    );
    assert!(b.is_stale(), "книга помечена stale");
}

#[test]
fn futures_contiguous_applies() {
    // GD-I-3: фьючерс pu==prev.u → Applied.
    let mut b = seeded();
    b.apply_l2delta(&[lvl(100, 9)], &[], 5, 5, None); // last_final=5
    let st = b.apply_l2delta(&[lvl(99, 3)], &[], 6, 7, Some(5)); // pu=5 == last(5)
    assert_eq!(st, ContinuityStatus::Applied, "фьючерс-чейн непрерывен");
    assert_eq!(b.size_at(Side::Buy, 99), 3);
}

#[test]
fn futures_gap_fail_closed() {
    // GD-I-4: фьючерс pu != prev.u → Gap, stale, не применено.
    let mut b = seeded();
    b.apply_l2delta(&[lvl(100, 9)], &[], 5, 5, None); // last_final=5
    let st = b.apply_l2delta(&[lvl(50, 999)], &[], 9, 9, Some(6)); // pu=6 != last(5) → GAP
    assert_eq!(st, ContinuityStatus::Gap);
    assert_eq!(b.size_at(Side::Buy, 50), 0, "fail-closed");
    assert!(b.is_stale());
}

#[test]
fn snapshot_resync_clears_stale() {
    // GD-I-6: после gap → дельты Gap; apply_snapshot → stale снят, чейн заново → Applied.
    let mut b = seeded();
    b.apply_l2delta(&[lvl(100, 9)], &[], 5, 5, None);
    b.apply_l2delta(&[lvl(50, 999)], &[], 8, 8, None); // GAP → stale
    assert!(b.is_stale());
    // stale-книга отвергает дальнейшие дельты (даже «валидные» по виду).
    assert_eq!(
        b.apply_l2delta(&[lvl(100, 1)], &[], 9, 9, None),
        ContinuityStatus::Gap,
        "stale-книга отвергает дельты до ресинка"
    );
    // ресинк снапшотом.
    b.apply_snapshot(&[lvl(200, 7)], &[lvl(201, 6)]);
    assert!(!b.is_stale(), "снапшот снял stale");
    let st = b.apply_l2delta(&[lvl(200, 10)], &[], 100, 100, None); // bootstrap заново
    assert_eq!(
        st,
        ContinuityStatus::Applied,
        "после ресинка чейн заводится заново"
    );
    assert_eq!(b.size_at(Side::Buy, 200), 10);
}

#[test]
fn books_apply_routes_and_flags_gap() {
    // Task #3: Books::apply(L2Delta) → apply_l2delta; gap помечает книгу stale (queryable).
    let mut books = Books::new();
    let venue = Venue::Binance;
    let sym = "BTCUSDT";
    let snap = MdEvent {
        venue,
        symbol: sym.to_string(),
        payload: MdPayload::L2Snapshot {
            bids: vec![lvl(100, 5)],
            asks: vec![lvl(101, 4)],
            ts_exch_ms: 1_752_000_000_000,
        },
    };
    let delta = |u0: u64, u1: u64| MdEvent {
        venue,
        symbol: sym.to_string(),
        payload: MdPayload::L2Delta {
            bids: vec![lvl(100, 9)],
            asks: vec![],
            first_update_id: u0,
            final_update_id: u1,
            prev_final_update_id: None,
            ts_exch_ms: 1_752_000_000_001,
        },
    };
    books.apply(&snap);
    books.apply(&delta(5, 5)); // bootstrap
    books.apply(&delta(8, 8)); // GAP (8 != 6)
    let bk = books.get(venue, sym).expect("книга есть");
    assert!(
        bk.is_stale(),
        "Books::apply(L2Delta) обязан прогнать чейнинг и пометить книгу stale на gap"
    );
}
