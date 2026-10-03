//! RED M-22 GW-I-1 (sacred, architect-only) — Read Gateway READ-ONLY (VB-I-3, Граница A).
//!
//! Gateway НЕ пишет журнал. Функциональная проверка: байты ВСЕХ сегментов журнал-каталога
//! до и после `snapshot`/`frames_since`/`replay` — идентичны. (Grep-канарейка «нет
//! journal-writer импорта в gateway/src» — была в гейте M-22, сданном в архив по норме Р-2
//! (`docs/archive/verify_M-22.sh`); ЖИВЫМ сторожем инварианта остаётся этот тест.)
//!
//! Анти-плацебо: impl, который тронул бы журнал (append/ротация/компакция), изменит байты →
//! падение. RED сейчас: `snapshot`/`frames_since`/`replay` = `unimplemented!()` (engine-dev).

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

fn build() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: 4096,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    let lvls = |base: f64| -> Vec<Level> {
        (0..10)
            .map(|k| Level {
                price: to_fixed(base + k as f64),
                size: to_fixed(1.0 + k as f64),
            })
            .collect()
    };
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for i in 0..40i64 {
            let ts = 1_752_000_000_000 + i * 100;
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::L2Snapshot {
                    bids: lvls(64_990.0),
                    asks: lvls(65_010.0),
                    ts_exch_ms: ts,
                },
            ))
            .expect("append");
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0),
                    size: to_fixed(1.0),
                    side: [Side::Buy, Side::Sell][(i % 2) as usize],
                    ts_exch_ms: ts + 5,
                },
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

/// Снимок байт всех сегментов (path → bytes), детерминированный порядок.
fn segment_bytes(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut segs = journal::list_segments(dir).expect("segments");
    segs.sort_by_key(|s| s.path.clone());
    segs.into_iter()
        .map(|s| {
            let name = s.path.file_name().unwrap().to_string_lossy().to_string();
            (name, std::fs::read(&s.path).expect("read seg"))
        })
        .collect()
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

#[test]
fn gateway_reads_do_not_mutate_journal() {
    let dir = build();
    let s = sel();
    let before = segment_bytes(dir.path());

    let _ = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot");
    let _ = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        usize::MAX,
    )
    .expect("frames_since");
    let _ = gateway::replay(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        Cursor::LATEST,
    )
    .expect("replay");

    let after = segment_bytes(dir.path());
    assert_eq!(
        before, after,
        "GW-I-1: чтение через gateway ИЗМЕНИЛО журнал (read-only нарушен)"
    );
}
