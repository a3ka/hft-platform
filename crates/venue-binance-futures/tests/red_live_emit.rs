//! M-06 RED (sacred, architect) — TD-014: futures runner НЕ эмитил L2Snapshot и Funding
//! в live-журнал (§8: 0 BinanceFutures L2 + 0 Funding, вечный "stale vs buffered diffs").
//!
//! Живой дефект был НЕВИДИМ юнит-тестам, т.к. `handle_diff/handle_snapshot/emit_book_snapshots`
//! ходят в сеть (`reqwest::Client`) и шлют в `tx` напрямую. Нужен ТЕСТИРУЕМЫЙ seam
//! `FuturesSession`: sync-state-машина БЕЗ сети/каналов; `run()` — тонкая I/O-оболочка.
//!
//! Оракул моделирует mock fstream+REST (прод-масштаб дисциплина, .claude/rules/testing.md):
//!  • НЕСКОЛЬКО contiguous depth-diff'ов буферизуются до снапшота;
//!  • REST snapshot сначала 418, потом успешный (консистентный с буфером);
//!  • !markPrice@arr присутствует в потоке;
//!  • после resync runner ОБЯЗАН эмитить L2Snapshot (live: 0 — вероятно last_update_id не
//!    двигается при apply → 2-й diff вечно "stale");
//!  • Funding из markPrice ОБЯЗАН эмититься даже во время depth-resync (не starvation);
//!  • нет hot-loop (418 → backoff).
//! Падает compile-RED (seam `FuturesSession` не существует) → venue-dev реализует seam + фикс
//! (run() делегирует в FuturesSession — live == tested) → GREEN. Анти-плацебо: верный рефактор
//! ТЕКУЩЕЙ логики оставляет multi-diff-stale → оракул RED, форсит фикс.

use std::time::Duration;

use contracts::{MdPayload, Venue};
use venue_binance_futures::{FuturesSession, SessionEffect};

// combined-stream fstream формат: {"stream":..., "data":...}.
const DIFF_A: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":1000,"T":1000,"s":"BTCUSDT","U":101,"u":105,"pu":100,"b":[["64000.0","1.0"]],"a":[["64001.0","1.0"]]}}"#;
const DIFF_B: &str = r#"{"stream":"btcusdt@depth","data":{"e":"depthUpdate","E":1001,"T":1001,"s":"BTCUSDT","U":106,"u":110,"pu":105,"b":[["63999.0","2.0"]],"a":[["64002.0","1.5"]]}}"#;
const MARKPRICE: &str = r#"{"stream":"!markPrice@arr","data":[{"e":"markPriceUpdate","E":1002,"s":"BTCUSDT","p":"64000.5","i":"64000.0","P":"64001.0","r":"0.00010000","T":9999999}]}"#;
// lastUpdateId=100: буфер DIFF_A(U=101=L+1) + DIFF_B(U=106) должны ОБА примениться → sync.
const SNAPSHOT_L100: &str = r#"{"lastUpdateId":100,"E":999,"T":999,"bids":[["64000.0","1.0"],["63999.0","2.0"]],"asks":[["64001.0","1.0"]]}"#;

fn emitted(effs: &[SessionEffect]) -> Vec<&contracts::MdEvent> {
    effs.iter()
        .filter_map(|e| match e {
            SessionEffect::Emit(md) => Some(md),
            _ => None,
        })
        .collect()
}
fn fetch_after(effs: &[SessionEffect], sym: &str) -> Option<Duration> {
    effs.iter().find_map(|e| match e {
        SessionEffect::FetchSnapshot { symbol, after } if symbol == sym => Some(*after),
        _ => None,
    })
}

#[test]
fn td014_futures_runner_emits_l2_and_funding_after_resync_no_starvation() {
    let mut s = FuturesSession::new(&["BTCUSDT".to_string()]);

    // Два contiguous diff'а до снапшота → буферизуются; первый триггерит bootstrap-fetch.
    let e1 = s.on_ws_text(DIFF_A);
    assert!(
        fetch_after(&e1, "BTCUSDT").is_some(),
        "первый depth-diff (книга не синкнута) обязан запросить snapshot-fetch"
    );
    let _ = s.on_ws_text(DIFF_B);

    // markPrice ПОКА depth ресинкается → Funding ОБЯЗАН эмититься (не starvation).
    let em = s.on_ws_text(MARKPRICE);
    assert!(
        emitted(&em).iter().any(|md| md.venue == Venue::BinanceFutures
            && matches!(md.payload, MdPayload::Funding { .. })),
        "TD-014: !markPrice@arr → Funding даже во время depth-resync (live: 0 Funding = starvation)"
    );

    // Snapshot 418 → backoff refetch (задержка > 0, не hot-loop).
    let e418 = s.on_snapshot_result("BTCUSDT", Err(418));
    assert!(
        fetch_after(&e418, "BTCUSDT").is_some_and(|d| d >= Duration::from_millis(100)),
        "418 → refetch с backoff (не hot-loop)"
    );

    // Успешный snapshot (L=100). Буфер DIFF_A(101-105)+DIFF_B(106-110) ОБА contiguous →
    // ОБА обязаны примениться → книга синкнута (не "stale" на 2-м diff'е).
    let _ = s.on_snapshot_result("BTCUSDT", Ok(SNAPSHOT_L100.to_string()));

    // Тик → bounded L2Snapshot ОБЯЗАН эмититься (книга синкнута, есть биржевое время).
    let et = s.tick();
    assert!(
        emitted(&et).iter().any(|md| md.venue == Venue::BinanceFutures
            && matches!(md.payload, MdPayload::L2Snapshot { .. })),
        "TD-014: после snapshot-sync с НЕСКОЛЬКИМИ contiguous diff'ами обязан эмитить L2Snapshot \
         (live: 0 L2 — вероятно last_update_id не двигается при apply → 2-й diff вечно stale)"
    );
}
