//! M-06 RED (sacred, architect) — TD-014 v2: futures runner НЕ эмитил BinanceFutures
//! L2Snapshot + Funding в live-журнал (§8 REJECT #4 reland-2: 0 L2 + 0 Funding, вечный
//! "gap / snapshot stale / 429 backoff"). Первая версия оракула была СЛИШКОМ СЛАБА:
//! тестировала happy-path bootstrap-sync (один reconcilable снапшот) и прошла на дефектном
//! коде. Эта версия воспроизводит ПОЛНЫЙ live-lifecycle и падает на af7725f.
//!
//! КОРЕНЬ (воспроизведён): когда recovery-снапшот приходит ВПЕРЕДИ буфера (все buffered
//! diff'ы `u_final <= lastUpdateId` → ДРОПАЮТСЯ, ни один не applied), книга "синкнута", но
//! `last_event_time_ms == 0` → `tick()` НЕ эмитит L2 (гейт на биржевое время) → вечный 0 L2.
//! Live: после gap+backoff снапшот всегда впереди буфера → книга без event-time → 0 L2, а
//! resync-churn (CPU/REST hammering) деградирует WS-консюмера → markPrice тоже теряется (0 Funding).
//!
//! Оракул детерминирован (FuturesSession синхронна, без сети/таймеров). Прод-масштаб дисциплина
//! (.claude/rules/testing.md): моделирует адверсарный gap/stale/backoff/markPrice, не happy-path.

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

const D_101: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":1000,"T":1000,"s":"BTCUSDT","U":101,"u":105,"pu":100,"b":[["64000.0","1.0"]],"a":[["64001.0","1.0"]]}}"#;
const D_106: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":1001,"T":1001,"s":"BTCUSDT","U":106,"u":110,"pu":105,"b":[["63999.0","2.0"]],"a":[["64002.0","1.5"]]}}"#;
// GAP: U=200 != last(110)+1 → continuity gap → resync.
const D_200: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":1002,"T":1002,"s":"BTCUSDT","U":200,"u":205,"pu":150,"b":[["63998.0","3.0"]],"a":[["64003.0","1.0"]]}}"#;
const MP: &str = r#"{"stream":"!markPrice@arr","data":[{"e":"markPriceUpdate","E":1003,"s":"BTCUSDT","p":"64000.5","i":"64000.0","P":"64001.0","r":"0.00010000","T":9999}]}"#;
const S100: &str =
    r#"{"lastUpdateId":100,"E":999,"T":999,"bids":[["64000.0","1.0"]],"asks":[["64001.0","1.0"]]}"#;
// STALE recovery snapshot: L=150 < buffer front u_first(200)-1 → stale → backoff refetch.
const S150: &str = r#"{"lastUpdateId":150,"E":1500,"T":1500,"bids":[["64000.0","1.0"]],"asks":[["64001.0","1.0"]]}"#;
// GOOD recovery snapshot: L=205 >= all buffered → БУФЕР ПОЛНОСТЬЮ ДРОПАЕТСЯ (ни один diff не applied).
const S205: &str = r#"{"lastUpdateId":205,"E":2050,"T":2050,"bids":[["63998.0","3.0"]],"asks":[["64003.0","1.0"]]}"#;

#[test]
fn td014_futures_runner_emits_l2_and_funding_through_gap_stale_backoff_lifecycle() {
    let mut s = FuturesSession::new(&["BTCUSDT".to_string()]);

    // 1. Bootstrap: diff до снапшота → буфер + fetch.
    let e = s.on_ws_text(D_101);
    assert!(
        fetch_after(&e).is_some(),
        "bootstrap: первый diff → snapshot-fetch"
    );

    // 2-3. Успешный снапшот (L=100, diff U=101 applied) → sync; tick → L2.
    s.on_snapshot_result("BTCUSDT", Ok(S100.to_string()));
    assert!(
        has_l2(&s.tick()),
        "начальный sync обязан эмитить L2Snapshot"
    );

    // 4-5. Steady-state contiguous diff → apply (last_update_id двигается) → всё ещё synced.
    s.on_ws_text(D_106);
    assert!(
        has_l2(&s.tick()),
        "steady-state contiguous diff: книга остаётся synced, L2 эмитится"
    );

    // 6. GAP → resync (книга инвалидируется, fetch).
    let e = s.on_ws_text(D_200);
    assert!(fetch_after(&e).is_some(), "continuity gap → resync fetch");

    // 7. markPrice ВО ВРЕМЯ resync → Funding (не starve).
    assert!(
        has_funding(&s.on_ws_text(MP)),
        "TD-014: Funding из markPrice во время depth-resync (live: 0 Funding)"
    );

    // 8. STALE recovery-снапшот (L=150 позади буфера) → backoff refetch (не hot-loop).
    let e = s.on_snapshot_result("BTCUSDT", Ok(S150.to_string()));
    assert!(
        fetch_after(&e).is_some_and(|d| d >= Duration::from_millis(100)),
        "stale снапшот → refetch с backoff (не hot-loop)"
    );

    // 9-10. GOOD снапшот (L=205 впереди буфера → все diff'ы дропаются, ни один не applied) →
    // книга "синкнута". tick ОБЯЗАН эмитить L2 (live-баг: last_event_time_ms=0 → 0 L2).
    s.on_snapshot_result("BTCUSDT", Ok(S205.to_string()));
    assert!(
        has_l2(&s.tick()),
        "TD-014 КОРЕНЬ: после gap→stale→recovery книга synced, но L2 НЕ эмитится \
         (recovery-снапшот дропнул все buffered diff'ы → last_event_time_ms=0 → 0 L2 в live)"
    );
}
