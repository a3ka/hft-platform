//! M-06 RED (sacred, architect) — TD-014: futures runner live-emit. §8 REJECT'ов было 3×
//! (0 L2 → sparse L2 + 0 Funding + gap/stale/429 churn). Оракул усиливался итеративно; эта
//! версия моделирует ПЕРСИСТЕНТНЫЙ live-фейл — continuity-churn, не только recovery.
//!
//! Два корня (оба воспроизведены детерминированно на FuturesSession, без сети/таймеров):
//!  T1 (recovery, FIXED в fac7c07 — regression-guard): recovery-снапшот впереди буфера дропает
//!     все diff'ы → last_event_time_ms=0 → tick не эмитит L2.
//!  T2 (continuity CHURN, ОТКРЫТ): Binance USDT-M FUTURES чейнит diff'ы через `pu`
//!     (previous update id) == book.last_update_id — update-id'ы НЕ +1-contiguous (U прыгает).
//!     Код использует СПОТ-правило `u_first == last_update_id + 1` → валидные futures-diff'ы с
//!     прыжком U ложно детектятся как gap → вечный resync (live: 311 gap / 44 stale / 18×429,
//!     sparse L2), а churn деградирует WS-консюмера → 0 Funding downstream.

use std::time::Duration;

use contracts::{MdPayload, Venue};
use venue_binance_futures::{FuturesSession, SessionEffect};

fn emitted(e: &[SessionEffect]) -> Vec<&contracts::MdEvent> {
    e.iter()
        .filter_map(|x| match x {
            SessionEffect::Emit(m) => Some(m),
            _ => None,
        })
        .collect()
}
fn fetch_after(e: &[SessionEffect]) -> Option<Duration> {
    e.iter().find_map(|x| match x {
        SessionEffect::FetchSnapshot { after, .. } => Some(*after),
        _ => None,
    })
}
fn has_l2(e: &[SessionEffect]) -> bool {
    emitted(e).iter().any(|m| {
        m.venue == Venue::BinanceFutures && matches!(m.payload, MdPayload::L2Snapshot { .. })
    })
}
fn has_funding(e: &[SessionEffect]) -> bool {
    emitted(e)
        .iter()
        .any(|m| m.venue == Venue::BinanceFutures && matches!(m.payload, MdPayload::Funding { .. }))
}

// --- lifecycle (T1) fixtures ---
const D_101: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":1000,"T":1000,"s":"BTCUSDT","U":101,"u":105,"pu":100,"b":[["64000.0","1.0"]],"a":[["64001.0","1.0"]]}}"#;
const D_106: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":1001,"T":1001,"s":"BTCUSDT","U":106,"u":110,"pu":105,"b":[["63999.0","2.0"]],"a":[["64002.0","1.5"]]}}"#;
const D_GAP: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":1002,"T":1002,"s":"BTCUSDT","U":200,"u":205,"pu":150,"b":[["63998.0","3.0"]],"a":[["64003.0","1.0"]]}}"#;
const MP: &str = r#"{"stream":"!markPrice@arr","data":[{"e":"markPriceUpdate","E":1003,"s":"BTCUSDT","p":"64000.5","i":"64000.0","P":"64001.0","r":"0.00010000","T":9999}]}"#;
const S100: &str =
    r#"{"lastUpdateId":100,"E":999,"T":999,"bids":[["64000.0","1.0"]],"asks":[["64001.0","1.0"]]}"#;
const S150: &str = r#"{"lastUpdateId":150,"E":1500,"T":1500,"bids":[["64000.0","1.0"]],"asks":[["64001.0","1.0"]]}"#;
const S205: &str = r#"{"lastUpdateId":205,"E":2050,"T":2050,"bids":[["63998.0","3.0"]],"asks":[["64003.0","1.0"]]}"#;

/// T1 (regression-guard, зелёный на fac7c07): recovery через gap/stale/впереди-буфера → L2.
#[test]
fn td014_t1_emit_l2_and_funding_through_gap_stale_recovery() {
    let mut s = FuturesSession::new(&["BTCUSDT".to_string()]);
    let e = s.on_ws_text(D_101);
    assert!(fetch_after(&e).is_some(), "bootstrap fetch");
    s.on_snapshot_result("BTCUSDT", Ok(S100.to_string()));
    assert!(has_l2(&s.tick()), "начальный sync → L2");
    s.on_ws_text(D_106);
    assert!(has_l2(&s.tick()), "steady contiguous → L2");
    let e = s.on_ws_text(D_GAP);
    assert!(fetch_after(&e).is_some(), "gap → resync fetch");
    assert!(has_funding(&s.on_ws_text(MP)), "Funding во время resync");
    let e = s.on_snapshot_result("BTCUSDT", Ok(S150.to_string()));
    assert!(
        fetch_after(&e).is_some_and(|d| d >= Duration::from_millis(100)),
        "stale → backoff"
    );
    s.on_snapshot_result("BTCUSDT", Ok(S205.to_string()));
    assert!(
        has_l2(&s.tick()),
        "recovery впереди-буфера → L2 (last_event_time_ms bug)"
    );
}

// --- continuity churn (T2) fixtures: pu чейнится, но U прыгает (валидно для FUTURES) ---
const C_BOOT: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":2000,"T":2000,"s":"BTCUSDT","U":101,"u":105,"pu":100,"b":[["64000.0","1.0"]],"a":[["64001.0","1.0"]]}}"#;
const C_S100: &str = r#"{"lastUpdateId":100,"E":1999,"T":1999,"bids":[["64000.0","1.0"]],"asks":[["64001.0","1.0"]]}"#;
// pu(105)==last(105), но U=120 != 106 — норм для perp (id'ы не +1). НЕ должен быть gap.
const C_JUMP1: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":2001,"T":2001,"s":"BTCUSDT","U":120,"u":130,"pu":105,"b":[["63999.0","2.0"]],"a":[["64002.0","1.5"]]}}"#;
// pu(130)==last(130), U=150 != 131 — снова валидный futures-jump.
const C_JUMP2: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":2002,"T":2002,"s":"BTCUSDT","U":150,"u":160,"pu":130,"b":[["63997.0","1.0"]],"a":[["64004.0","2.0"]]}}"#;

/// T2 (ОТКРЫТ, RED на fac7c07): futures continuity = `pu == last_update_id`, НЕ `u_first == last+1`.
/// Валидные futures-diff'ы с прыжком U НЕ должны триггерить resync → книга остаётся synced,
/// L2 эмитится плотно (live: 311 ложных gap → sparse L2 + 429 churn + 0 Funding downstream).
#[test]
fn td014_t2_futures_continuity_uses_pu_not_spot_u_first() {
    let mut s = FuturesSession::new(&["BTCUSDT".to_string()]);
    s.on_ws_text(C_BOOT);
    s.on_snapshot_result("BTCUSDT", Ok(C_S100.to_string()));
    assert!(has_l2(&s.tick()), "initial sync → L2");

    // Валидный futures-jump (pu чейнится) НЕ должен вызывать resync-fetch.
    let e1 = s.on_ws_text(C_JUMP1);
    assert!(
        fetch_after(&e1).is_none(),
        "TD-014 T2: futures diff с валидным pu (==last), но прыжком U НЕ должен детектиться как gap \
         (continuity у perp = pu, не спот-правило u_first==last+1) — иначе вечный resync churn"
    );
    assert!(
        has_l2(&s.tick()),
        "после валидного pu-jump книга synced → L2"
    );

    // Второй подряд — устойчиво synced (плотный L2, без ложного gap).
    let e2 = s.on_ws_text(C_JUMP2);
    assert!(
        fetch_after(&e2).is_none(),
        "второй pu-jump тоже без ложного gap"
    );
    assert!(
        has_l2(&s.tick()),
        "устойчивый sync → плотный L2 (не sparse)"
    );
}
