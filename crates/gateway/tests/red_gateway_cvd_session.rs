//! RED M-38a (sacred, architect-only) — CVD session-anchored ledger (TD-043).
//!
//! Founder-решение 2026-07-27 (подписано): CVD сбрасывается на границе 00:00 UTC, per-session
//! ledger ЗЕРКАЛЬНО Volume Profile (`utc_session_id`, VB-I-6). Текущий gateway (M-37) держит CVD
//! как ЕДИНУЮ running-сумму через все дни (`Reducer::finish`, `crates/gateway/src/lib.rs` ~843-856:
//! "running считается по удержанным бакетам" БЕЗ session-reset) — это баг TD-043.
//!
//! ЭТОТ ФАЙЛ — RUNTIME-RED анти-плацебо: тесты НЕ ссылаются на новые поля формы v7
//! (`cvd_session_base: Vec<..>`, `CvdSession`), поэтому КОМПИЛИРУЮТСЯ против текущего single-running
//! кода и ПАДАЮТ по значению (running несёт сумму прошлой сессии через 00:00 UTC). Форма v7
//! (per-session base Vec) — compile-RED в `red_gateway_window.rs` (`cvd_two_sessions_*`,
//! обновлённые `cvd_base_survives_*`/`windowed_live_eq_replay_overlap_multistep`).
//!
//! testing.md чек-лист: п.1 асимметрия (`cvd_asymmetric_imbalance_*`), п.2 множественность
//! (`cvd_multiple_trades_per_boundary_bucket_*`), п.3 отсутствие (S2 НЕ наследует S1 — во всех),
//! п.4 границы (переход через 00:00 UTC — во всех). Прод-масштаб (п.5) — N/A: CVD — чистый compute
//! без I/O-границы ресурса (оракул ресурса — M-38b checkpoint).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const DAY_S: i64 = 86_400;

/// UTC-сессия из бакет-`time_s` (= `ts_ms/1000`). Зеркалит `gateway::utc_session_id(ts_ms)`
/// в секундном пространстве: `time_s.div_euclid(86_400) == ts_ms.div_euclid(86_400_000)`.
fn session_of(time_s: i64) -> i64 {
    time_s.div_euclid(DAY_S)
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(price: f64, size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn sel(window_ms: Option<i64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms,
        depth_cadence_ms: None,
    }
}

fn snap(dir: &std::path::Path, window_ms: Option<i64>, at: Cursor) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(window_ms), at).expect("snapshot")
}

/// Кумулятив CVD к КОНЦУ сессии `sid` = running ПОСЛЕДНЕГО (max `time_s`) бакета сессии.
/// `cumulative_delta` отсортирован по `time_s` возр. → последний matching = максимальный.
fn session_running(cd: &[(i64, i64)], sid: i64) -> i64 {
    cd.iter()
        .rev()
        .find(|(t, _)| session_of(*t) == sid)
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("нет бакета сессии {sid} в cumulative_delta {cd:?}"))
}

#[test]
fn cvd_resets_at_utc_session_boundary() {
    // ЧИСТЫЙ session-reset (window=None → без эвикции). Две UTC-сессии, по одной сделке.
    // Founder 2026-07-27: CVD зеркалит VP → running обнуляется на 00:00 UTC.
    let d1 = 20_278 * DAY_MS;
    let d2 = 20_279 * DAY_MS; // следующий UTC-день = новая сессия
    let dir = journal_of(vec![
        trade(100.0, 10.0, Side::Buy, d1 + 10_000), // S1: +10
        trade(100.0, 3.0, Side::Buy, d2 + 10_000),  // S2: должно быть +3 (сессия обнулилась)
    ]);
    let full = snap(dir.path(), None, Cursor::LATEST);
    let cd = &full.series.cumulative_delta;

    let s1 = session_of(d1 / 1000);
    let s2 = session_of(d2 / 1000);

    assert_eq!(
        session_running(cd, s2),
        to_fixed(3.0),
        "TD-043: CVD обязан обнулиться на 00:00 UTC. S2 running={} != session-local {}. \
         Текущий single-running несёт S1 (+10) → выдаёт +13e8 (баг).",
        session_running(cd, s2),
        to_fixed(3.0),
    );
    assert_eq!(
        session_running(cd, s1),
        to_fixed(10.0),
        "S1 running должен остаться +10 (не тронут сессией S2)"
    );
}

#[test]
fn cvd_asymmetric_imbalance_does_not_leak_across_boundary() {
    // testing.md п.1 (асимметрия): S1 сильно buy-перекошена (+65), S2 — ТОЛЬКО одна sell-сделка.
    // Session-local S2 = -7, НЕ (S1_running - 7). Ловит утечку базы прошлой сессии через 00:00 UTC.
    let d1 = 20_278 * DAY_MS;
    let d2 = 20_279 * DAY_MS;
    let dir = journal_of(vec![
        trade(100.0, 40.0, Side::Buy, d1 + 10_000),
        trade(100.0, 25.0, Side::Buy, d1 + 20_000), // S1 running = +65
        trade(100.0, 7.0, Side::Sell, d2 + 10_000), // S2 = -7 (свежая сессия)
    ]);
    let full = snap(dir.path(), None, Cursor::LATEST);
    let cd = &full.series.cumulative_delta;

    let s1 = session_of(d1 / 1000);
    let s2 = session_of(d2 / 1000);

    assert_eq!(
        session_running(cd, s1),
        to_fixed(65.0),
        "S1 running = +40+25 = +65 (агрегат сессии)"
    );
    assert_eq!(
        session_running(cd, s2),
        -to_fixed(7.0),
        "S2 (sell-only) обязан быть -7 session-local. Текущий код: +65-7 = +58e8 (утечка S1 через границу)"
    );
}

#[test]
fn cvd_multiple_trades_per_boundary_bucket_reset() {
    // testing.md п.2 (множественность): 2+ сделки в последнем бакете S1 И 2+ в первом бакете S2
    // (все в одном 1000ms-бакете каждой стороны). Агрегация внутри сессии + reset на границе.
    let d1 = 20_278 * DAY_MS;
    let d2 = 20_279 * DAY_MS;
    let dir = journal_of(vec![
        trade(100.0, 4.0, Side::Buy, d1 + 10_000),
        trade(100.0, 6.0, Side::Buy, d1 + 10_500), // тот же бакет S1 → +10
        trade(100.0, 2.0, Side::Buy, d2 + 500),
        trade(100.0, 5.0, Side::Sell, d2 + 700), // тот же бакет S2 → +2-5 = -3
    ]);
    let full = snap(dir.path(), None, Cursor::LATEST);
    let cd = &full.series.cumulative_delta;

    let s1 = session_of(d1 / 1000);
    let s2 = session_of(d2 / 1000);

    assert_eq!(
        session_running(cd, s1),
        to_fixed(10.0),
        "S1 бакет агрегирует два филла 4+6=10"
    );
    assert_eq!(
        session_running(cd, s2),
        to_fixed(2.0) - to_fixed(5.0),
        "S2 первый бакет = +2-5 = -3 session-local (multi-fill в сессии, reset на 00:00 UTC). \
         Текущий код: +10-3 = +7e8 (несёт S1)"
    );
}

#[test]
fn cvd_three_sessions_each_reset_independently() {
    // testing.md п.4 (границы, переход через несколько 00:00 UTC): 3 дня подряд, каждая сессия
    // имеет СВОЙ running с нуля. Ни одна не наследует предыдущую (отсутствие — п.3).
    let d1 = 20_278 * DAY_MS;
    let d2 = 20_279 * DAY_MS;
    let d3 = 20_280 * DAY_MS;
    let dir = journal_of(vec![
        trade(100.0, 11.0, Side::Buy, d1 + 5_000), // S1: +11
        trade(100.0, 4.0, Side::Sell, d2 + 5_000), // S2: -4
        trade(100.0, 9.0, Side::Buy, d3 + 5_000),  // S3: +9
    ]);
    let full = snap(dir.path(), None, Cursor::LATEST);
    let cd = &full.series.cumulative_delta;

    assert_eq!(
        session_running(cd, session_of(d1 / 1000)),
        to_fixed(11.0),
        "S1=+11"
    );
    assert_eq!(
        session_running(cd, session_of(d2 / 1000)),
        -to_fixed(4.0),
        "S2=-4 session-local (не +11-4=+7)"
    );
    assert_eq!(
        session_running(cd, session_of(d3 / 1000)),
        to_fixed(9.0),
        "S3=+9 session-local (не +11-4+9=+16)"
    );
}
