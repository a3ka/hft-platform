//! CT-RFC-01 фикстуры/тесты (sacred, architect). Гейт C-003 PASS (research/critiques/C-003.md).
//! Покрывает CT-I-1 (single-definition) и CT-I-3 (старый журнал читается новым кодом).

use std::path::Path;

use contracts::*;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

// --- СТАТИЧЕСКИЕ pre-change postcard-байты (сгенерированы кодом ДО добавления вариантов,
//     C-003 §2). Доказывают: старые дискриминанты Trade/L2Snapshot/Funding не сдвинуты. ---
const TRADE_PRECHANGE: &str =
    "012a80a0abfef962010007425443555344540080e2de92adfa0280dac40900f6a1abfef962";
const L2_PRECHANGE: &str = "022b82a0abfef96201010342544301018080d0dbc3f40280c6868f01018084ffbac4f4028088debe01bca3abfef962";
const FUNDING_PRECHANGE: &str = "032c84a0abfef96201010342544302f2c0019aa5abfef962";

/// CT-I-3 — старый (pre-change) журнал декодируется новым кодом бит-в-бит в те же события.
#[test]
fn ct_i_3_old_fixtures_still_decode() {
    let trade: Event = postcard::from_bytes(&unhex(TRADE_PRECHANGE)).unwrap();
    assert_eq!(
        trade,
        Event {
            seq: 1,
            ts_mono_ns: 42,
            ts_wall_ms: 1_700_000_000_000,
            kind: EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65000.5),
                    size: to_fixed(0.1),
                    side: Side::Buy,
                    ts_exch_ms: 1_700_000_000_123,
                }
            ),
        }
    );

    let l2: Event = postcard::from_bytes(&unhex(L2_PRECHANGE)).unwrap();
    assert_eq!(
        l2,
        Event {
            seq: 2,
            ts_mono_ns: 43,
            ts_wall_ms: 1_700_000_000_001,
            kind: EventKind::md(
                Venue::Hyperliquid,
                "BTC",
                MdPayload::L2Snapshot {
                    bids: vec![Level {
                        price: to_fixed(64000.0),
                        size: to_fixed(1.5)
                    }],
                    asks: vec![Level {
                        price: to_fixed(64001.0),
                        size: to_fixed(2.0)
                    }],
                    ts_exch_ms: 1_700_000_000_222,
                }
            ),
        }
    );

    let funding: Event = postcard::from_bytes(&unhex(FUNDING_PRECHANGE)).unwrap();
    assert_eq!(
        funding,
        Event {
            seq: 3,
            ts_mono_ns: 44,
            ts_wall_ms: 1_700_000_000_002,
            kind: EventKind::md(
                Venue::Hyperliquid,
                "BTC",
                MdPayload::Funding {
                    rate_e8: 12345,
                    ts_exch_ms: 1_700_000_000_333
                }
            ),
        }
    );
}

/// Новые варианты + Venue::BinanceFutures — serde_json И postcard roundtrip бит-идентичны.
#[test]
fn ct_rfc01_new_variants_roundtrip() {
    let events = vec![
        Event {
            seq: 10,
            ts_mono_ns: 1,
            ts_wall_ms: 1_700_000_100_000,
            kind: EventKind::md(
                Venue::BinanceFutures,
                "BTCUSDT",
                MdPayload::OpenInterest {
                    oi_e8: to_fixed(12345.678),
                    ts_exch_ms: 1_700_000_100_100,
                },
            ),
        },
        Event {
            seq: 11,
            ts_mono_ns: 2,
            ts_wall_ms: 1_700_000_100_001,
            kind: EventKind::md(
                Venue::BinanceFutures,
                "ETHUSDT",
                MdPayload::Liquidation {
                    price: to_fixed(1800.5),
                    size: to_fixed(3.25),
                    side: Side::Sell,
                    ts_exch_ms: 1_700_000_100_200,
                },
            ),
        },
        Event {
            seq: 12,
            ts_mono_ns: 3,
            ts_wall_ms: 1_700_000_100_002,
            kind: EventKind::md(
                Venue::Binance,
                "USDT",
                MdPayload::MarginRate {
                    rate_e8: to_fixed(0.0001),
                    ts_exch_ms: 1_700_000_100_300,
                },
            ),
        },
    ];
    for e in &events {
        let pc = postcard::to_stdvec(e).unwrap();
        let back_pc: Event = postcard::from_bytes(&pc).unwrap();
        assert_eq!(*e, back_pc, "postcard roundtrip");
        let js = serde_json::to_string(e).unwrap();
        let back_js: Event = serde_json::from_str(&js).unwrap();
        assert_eq!(*e, back_js, "serde_json roundtrip");
    }
}

/// CT-I-1 — `enum Venue` и `enum MdPayload` определены РОВНО в одном крейте (contracts).
#[test]
fn ct_i_1_single_definition_canary() {
    // Needle собран из частей, чтобы исходник ЭТОГО теста сам не матчился.
    let venue_needle = format!("enum {} {{", "Venue");
    let payload_needle = format!("enum {} {{", "MdPayload");
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(".."); // workspace crates/
    let mut venue_hits = Vec::new();
    let mut payload_hits = Vec::new();
    walk_rs(&crates_dir, &mut |path, body| {
        if body.contains(&venue_needle) {
            venue_hits.push(path.to_path_buf());
        }
        if body.contains(&payload_needle) {
            payload_hits.push(path.to_path_buf());
        }
    });
    assert_eq!(
        venue_hits.len(),
        1,
        "enum Venue определён не в одном месте: {venue_hits:?}"
    );
    assert_eq!(
        payload_hits.len(),
        1,
        "enum MdPayload определён не в одном месте: {payload_hits:?}"
    );
    assert!(venue_hits[0].ends_with("contracts/src/lib.rs"));
    assert!(payload_hits[0].ends_with("contracts/src/lib.rs"));
}

fn walk_rs(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk_rs(&p, f);
        } else if p.extension().is_some_and(|x| x == "rs") {
            if let Ok(body) = std::fs::read_to_string(&p) {
                f(&p, &body);
            }
        }
    }
}
