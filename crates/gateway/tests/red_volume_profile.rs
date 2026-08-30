//! RED M-24 VP-I-1..4 (sacred, architect-only) — Session Volume Profile (SVP) в gateway SeriesBundle.
//!
//! Гистограмма цена→объём per сессия (UTC-день, VB-I-6 `utc_session_id`) → POC/VAH/VAL. Новая серия
//! `SeriesBundle.volume_profile: Vec<VolumeProfileRow>` (поля см. milestones/M-24 §Контракт-форма).
//!
//! COMPILE-RED: поле `volume_profile` и тип `VolumeProfileRow` ещё НЕ существуют → тест не компилируется
//! (как M-17 `red_depth_series`). engine-dev создаёт тип+аккумулятор+поле+bump schema→4 → GREEN.
//! Анти-плацебо: POC не-argmax → VP-I-1; неверный VA-обход → VP-I-2; без session-сброса → VP-I-3;
//! выдуманные пустые цены → VP-I-4.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(price: f64, size: f64, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side: Side::Buy, // VP объём-взвешен по цене, сторона не влияет на гистограмму
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

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
        depth_cadence_ms: None,
    }
}

fn snap(dir: &std::path::Path) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST).expect("snapshot")
}

const T: i64 = 1_752_000_010_000; // один UTC-день

#[test]
fn vp_poc() {
    // VP-I-1: POC = цена макс. объёма. 100:1, 101:2, 102:6, 103:1 → POC=102.
    let dir = journal_of(vec![
        trade(100.0, 1.0, T),
        trade(101.0, 2.0, T + 1),
        trade(102.0, 6.0, T + 2),
        trade(103.0, 1.0, T + 3),
    ]);
    let a = snap(dir.path());
    let row = a
        .series
        .volume_profile
        .first()
        .expect("volume_profile непуст");
    assert_eq!(
        row.poc_e8,
        to_fixed(102.0),
        "POC обязан быть ценой макс. объёма (102)"
    );
    // Детерминизм.
    let b = snap(dir.path());
    assert_eq!(
        a.series.volume_profile, b.series.volume_profile,
        "volume_profile недетерминирован"
    );
}

#[test]
fn vp_value_area() {
    // VP-I-2: 100:1,101:3,102:6,103:2,104:1. total=13, target=ceil(13·0.7)=10.
    // POC=102(6); +101(3)→9; +103(2)→11≥10. VA=[101,103]. VAH=103, VAL=101, va_pct=11/13=84.6%.
    let dir = journal_of(vec![
        trade(100.0, 1.0, T),
        trade(101.0, 3.0, T + 1),
        trade(102.0, 6.0, T + 2),
        trade(103.0, 2.0, T + 3),
        trade(104.0, 1.0, T + 4),
    ]);
    let a = snap(dir.path());
    let row = a
        .series
        .volume_profile
        .first()
        .expect("volume_profile непуст");
    assert_eq!(row.poc_e8, to_fixed(102.0), "POC=102");
    assert_eq!(
        row.val_e8,
        to_fixed(101.0),
        "VAL=101 (нижняя граница 70%-зоны)"
    );
    assert_eq!(
        row.vah_e8,
        to_fixed(103.0),
        "VAH=103 (верхняя граница 70%-зоны)"
    );
    assert!(
        row.va_pct_e8 >= 70_000_000,
        "VA обязана покрывать ≥70% объёма, получила {}e-8",
        row.va_pct_e8
    );
}

#[test]
fn vp_session_reset() {
    // VP-I-3: сделки по разные стороны UTC-полуночи → РАЗНЫЕ VolumeProfileRow (по session_id).
    let midnight = 20_278 * DAY_MS;
    let dir = journal_of(vec![
        trade(100.0, 5.0, midnight - 30_000), // сессия D (день 20277)
        trade(200.0, 3.0, midnight + 30_000), // сессия D+1 (день 20278)
    ]);
    let a = snap(dir.path());
    assert!(
        a.series.volume_profile.len() >= 2,
        "две сессии → ≥2 VolumeProfileRow, получила {}",
        a.series.volume_profile.len()
    );
    let pocs: Vec<i64> = a.series.volume_profile.iter().map(|r| r.poc_e8).collect();
    assert!(
        pocs.contains(&to_fixed(100.0)) && pocs.contains(&to_fixed(200.0)),
        "POC каждой сессии раздельны (100 и 200); объёмы дней не смешаны"
    );
    // session_id-ы различны (VB-I-6 UTC-день).
    let sids: Vec<i64> = a
        .series
        .volume_profile
        .iter()
        .map(|r| r.session_id)
        .collect();
    assert!(
        sids.contains(&20_277) && sids.contains(&20_278),
        "session_id = utc_session_id (20277 и 20278)"
    );
}

#[test]
fn vp_prices_not_invented() {
    // VP-I-4: сделки только на 100 и 105 (разрыв) → bins содержит ТОЛЬКО эти цены, не 101..104.
    let dir = journal_of(vec![trade(100.0, 1.0, T), trade(105.0, 1.0, T + 1)]);
    let a = snap(dir.path());
    let row = a
        .series
        .volume_profile
        .first()
        .expect("volume_profile непуст");
    assert_eq!(row.bins.len(), 2, "ровно 2 торгованные цены");
    assert!(
        row.bins.iter().any(|&(p, _)| p == to_fixed(100.0))
            && row.bins.iter().any(|&(p, _)| p == to_fixed(105.0)),
        "bins = торгованные цены 100 и 105"
    );
    assert!(
        !row.bins.iter().any(|&(p, _)| p == to_fixed(102.0)),
        "VP-I-4: цена без сделок (102) НЕ выдумывается"
    );
}

// === RN-19 (тай-брейки — testing.md #4): прежние фикстуры их обходили ⇒ инвариант держался
// реализацией, не тестом. Пинуем ОБА правила §Design: POC тай→НИЗШАЯ, VA тай→ВЕРХНИЙ. ===

#[test]
fn vp_poc_tie_goes_to_lowest_price() {
    // VP-I-1 тай: 100:5, 101:1, 102:5 — макс. объём (5) на ДВУХ ценах. POC = НИЗШАЯ (100).
    // Анти-плацебо: impl с тай→высшая дал бы 102.
    let dir = journal_of(vec![
        trade(100.0, 5.0, T),
        trade(101.0, 1.0, T + 1),
        trade(102.0, 5.0, T + 2),
    ]);
    let a = snap(dir.path());
    let row = a
        .series
        .volume_profile
        .first()
        .expect("volume_profile непуст");
    assert_eq!(
        row.poc_e8,
        to_fixed(100.0),
        "POC-тай (100 и 102 по 5) обязан разрешиться в НИЗШУЮ цену (100), не 102"
    );
}

#[test]
fn vp_value_area_tie_expands_upward() {
    // VP-I-2 тай: 101:2, 102:6, 103:2. total=10, target=ceil(10·0.7)=7. POC=102(6), acc=6.
    // Шаг: above(103)=2 == below(101)=2 → ТАЙ → берём ВЕРХНИЙ (103). acc=8≥7. VA=[102,103].
    // ⇒ VAH=103, VAL=102. Анти-плацебо: impl с тай→нижний дал бы VAL=101, VAH=102.
    let dir = journal_of(vec![
        trade(101.0, 2.0, T),
        trade(102.0, 6.0, T + 1),
        trade(103.0, 2.0, T + 2),
    ]);
    let a = snap(dir.path());
    let row = a
        .series
        .volume_profile
        .first()
        .expect("volume_profile непуст");
    assert_eq!(row.poc_e8, to_fixed(102.0), "POC=102");
    assert_eq!(
        row.vah_e8,
        to_fixed(103.0),
        "VA-тай (above==below) → расширение ВВЕРХ: VAH=103"
    );
    assert_eq!(
        row.val_e8,
        to_fixed(102.0),
        "VA-тай → нижняя граница остаётся POC=102 (не спускается к 101)"
    );
}
