//! RED M-28 GS-I-4 / GS-I-5 (sacred, architect-only) — wire-roundtrip + passthrough-fidelity.
//!
//! Serve-adapter — ТОНКАЯ оболочка над `gateway::{snapshot,frames_since}`: обязан отдавать РОВНО те же
//! Snapshot/Frame, что библиотека (GS-I-5 → live==replay цел), в JSON-конверте `ServeMsg` (GS-I-4,
//! JS-декодируемо). Анти-плацебо: любая перекодировка/фильтрация серий в транспорте → расхождение с
//! библиотекой. RED сейчас: `snapshot_msg`/`frames_msgs` = `unimplemented!()` (тела — engine-dev task #3).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use gateway_serve::serve::{frames_msgs, snapshot_msg};
use gateway_serve::wire::ServeMsg;
use journal::{EpochFilter, Journal, WriterConfig};

fn journal_of() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    let t = 1_752_000_010_000;
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for i in 0..6i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + i as f64),
                    size: to_fixed(1.0),
                    side: [Side::Buy, Side::Sell][(i % 2) as usize],
                    ts_exch_ms: t + i,
                },
            ))
            .expect("append");
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
fn frames_msgs_passthrough_equals_library() {
    // GS-I-5: serve-adapter отдаёт РОВНО кадры gateway::frames_since (без трансформации).
    let dir = journal_of();
    let s = sel();
    let (msgs, cur) = frames_msgs(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        usize::MAX,
    )
    .expect("frames_msgs");
    let (frames, cur2) = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        usize::MAX,
    )
    .expect("frames_since");

    let served: Vec<gateway::Frame> = msgs
        .iter()
        .filter_map(|m| match m {
            ServeMsg::Frame(f) => Some(f.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        served, frames,
        "GS-I-5: serve-adapter обязан отдать РОВНО кадры библиотеки (без трансформации)"
    );
    assert_eq!(cur, cur2, "курсор serve-adapter == библиотеки");
}

#[test]
fn snapshot_msg_roundtrips_and_matches_library() {
    let dir = journal_of();
    let s = sel();
    let msg = snapshot_msg(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot_msg");

    // GS-I-4: JSON wire-roundtrip.
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: ServeMsg = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, back, "GS-I-4: ServeMsg wire-roundtrip (JSON)");
    assert!(
        json.contains("schema_version"),
        "GS-I-4: конверт несёт schema_version (GW-I-5)"
    );

    // GS-I-5: обёртка == библиотека.
    let ServeMsg::Snapshot(inner) = back else {
        panic!("ожидался ServeMsg::Snapshot");
    };
    let lib = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot");
    assert_eq!(
        inner, lib,
        "GS-I-5: snapshot_msg обязан обернуть РОВНО gateway::snapshot"
    );
}
