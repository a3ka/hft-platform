//! RED M-23 HM-I-4 (sacred, architect-only) — Volume Bubbles (торгованный объём time×price).
//!
//! Пузыри исполнений: `(time_s, price) → {buy_vol, sell_vol}` из `Trade` (side→buy/sell раздельно).
//! Цены НЕ выдумываются (только торгованные, класс footprint C-016). COMPILE-RED: поле
//! `snap.series.volume_bubbles` и тип `BubbleCell` ещё НЕ существуют (engine-dev task #4).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_752_000_010_000;

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
    let cfg = WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
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
    }
}

#[test]
fn bubbles_buy_sell_and_not_invented() {
    // HM-I-4: buy 65000×2, sell 65000×1, buy 65005×3 (один бакет). Цена 65001 — БЕЗ сделок.
    let dir = journal_of(vec![
        trade(65_000.0, 2.0, Side::Buy, T),
        trade(65_000.0, 1.0, Side::Sell, T + 1),
        trade(65_005.0, 3.0, Side::Buy, T + 2),
    ]);
    let a = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("snapshot");

    let at = |p: f64| {
        a.series
            .volume_bubbles
            .iter()
            .find(|c| c.price_e8 == to_fixed(p))
    };
    let c65000 = at(65_000.0).expect("пузырь на 65000 есть");
    assert_eq!(c65000.buy_vol_e8, to_fixed(2.0), "buy на 65000 = 2");
    assert_eq!(
        c65000.sell_vol_e8,
        to_fixed(1.0),
        "sell на 65000 = 1 (раздельно)"
    );

    let c65005 = at(65_005.0).expect("пузырь на 65005 есть");
    assert_eq!(c65005.buy_vol_e8, to_fixed(3.0), "buy на 65005 = 3");
    assert_eq!(c65005.sell_vol_e8, 0, "sell на 65005 = 0");

    assert!(
        at(65_001.0).is_none(),
        "HM-I-4: цена без сделок (65001) НЕ выдумывается"
    );
}
