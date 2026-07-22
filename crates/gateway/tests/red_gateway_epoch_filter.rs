//! RED M-22 GW-I-7 (sacred, architect-only) — EpochFilter соблюдён (CT-RFC02-2, C-022 B1).
//!
//! Gateway ОБЯЗАН сворачивать ТОЛЬКО эпохи, прошедшие переданный `EpochFilter`. Own-захват,
//! Vendor-история и Synthetic не смешиваются МОЛЧА — иначе серии кокпита/AI учатся на данных,
//! которых у нас не было (тот же класс, что TD-015, но дороже). Прежний набор был слеп: все
//! фикстуры были `OwnCapture`, поэтому impl, игнорирующий фильтр (хардкод `All`), зеленел.
//!
//! Фикстура: СМЕШАННЫЙ журнал в одном каталоге — OwnCapture (чистые ПОКУПКИ), Vendor (чистые
//! ПРОДАЖИ), Synthetic (ПРОДАЖИ). Идиома reopen-dir-с-новым-cfg — как `journal::red_segments_epochs`.
//! CVD (знаковая агрессия) делает выбор эпох НАБЛЮДАЕМЫМ в серии:
//!   own-only CVD  = +N (только покупки) > 0;
//!   all CVD       = +N − M − K (продажи вычитают) < own.
//! Анти-плацебо: impl, игнорирующий фильтр → own == all → падение. RED сейчас: `snapshot`
//! = `unimplemented!()` (engine-dev task #3).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

fn cfg(source: DataSource, epoch: &str) -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source,
        provenance: "test".to_string(),
        epoch_id: epoch.to_string(),
    }
}

fn trade(size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(65_000.0),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

/// Смешанный журнал: 10 own-BUY, 10 vendor-SELL, 5 synth-SELL (все в одном бакете B0).
fn build() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let ts = 1_752_000_000_000; // один бакет
    {
        let mut j = Journal::open_with(dir.path(), cfg(DataSource::OwnCapture, "own-2026-07"))
            .expect("own");
        for _ in 0..10 {
            j.append(trade(1.0, Side::Buy, ts)).expect("append own");
        }
        j.flush().expect("flush own");
    }
    {
        let mut j =
            Journal::open_with(dir.path(), cfg(DataSource::Vendor, "vendor-2024")).expect("vendor");
        for _ in 0..10 {
            j.append(trade(1.0, Side::Sell, ts)).expect("append vendor");
        }
        j.flush().expect("flush vendor");
    }
    {
        let mut j = Journal::open_with(dir.path(), cfg(DataSource::Synthetic, "synth-x"))
            .expect("synth");
        for _ in 0..5 {
            j.append(trade(1.0, Side::Sell, ts)).expect("append synth");
        }
        j.flush().expect("flush synth");
    }
    dir
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
    }
}

fn cvd_last(series: &gateway::SeriesBundle) -> i64 {
    series
        .cumulative_delta
        .last()
        .map(|&(_, v)| v)
        .expect("cumulative_delta непуст")
}

#[test]
fn epoch_filter_is_honored_own_differs_from_all() {
    let dir = build();
    let s = sel();

    let own = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot own");
    let all = gateway::snapshot(dir.path(), EpochFilter::All, &s, Cursor::LATEST)
        .expect("snapshot all");

    // OwnCaptureOnly = только 10 покупок → CVD строго положителен.
    assert!(
        cvd_last(&own.series) > 0,
        "OwnCaptureOnly обязан свернуть ТОЛЬКО own-покупки (CVD>0), получил {}",
        cvd_last(&own.series)
    );
    // All = own+vendor+synth → продажи перевешивают → CVD строго меньше own (и отрицателен здесь).
    assert!(
        cvd_last(&all.series) < cvd_last(&own.series),
        "All обязан включить vendor/synth-продажи → CVD({}) < own CVD({}); \
         равенство = фильтр ПРОИГНОРИРОВАН (эпохи смешаны молча)",
        cvd_last(&all.series),
        cvd_last(&own.series)
    );
}

#[test]
fn explicit_epoch_selection_is_distinct() {
    let dir = build();
    let s = sel();

    let own = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot own");
    // Explicit(own+vendor) БЕЗ synth → между own и all.
    let own_vendor = gateway::snapshot(
        dir.path(),
        EpochFilter::Explicit(vec!["own-2026-07".to_string(), "vendor-2024".to_string()]),
        &s,
        Cursor::LATEST,
    )
    .expect("snapshot own+vendor");

    // own+vendor = +10 −10 = 0; отличается и от own (+10).
    assert!(
        cvd_last(&own_vendor.series) != cvd_last(&own.series),
        "Explicit([own,vendor]) обязан отличаться от OwnCaptureOnly (vendor-продажи учтены)"
    );
}
