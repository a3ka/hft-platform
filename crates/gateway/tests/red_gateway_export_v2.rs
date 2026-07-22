//! RED M-22 GW-I-5 / GW-I-6 (sacred, architect-only) — export v2 аддитивен + провенанс глубины.
//!
//! GW-I-5 (VB-I-4): `Snapshot` несёт `schema_version = GATEWAY_SCHEMA_VERSION`; форма аддитивна —
//! потребитель, знающий только v1-подмножество полей depth-строки, читает вывод gateway без ошибки
//! (serde игнорирует незнакомые поля). Форма меняется ТОЛЬКО с bump константы.
//!
//! GW-I-6 (VB-I-5): любая depth-серия глубже 1.3% от mid несёт НЕПУСТОЙ `depth_band_provenance`;
//! полоса ≤1.3% — `None` допустим. Полосы `[0.001 (0.1%), 0.03 (3%)]`: строка 3% ОБЯЗАНА иметь
//! провенанс (diff-реконструкция, не биржевой факт), строка 0.1% — может не иметь.
//!
//! Анти-плацебо: impl без провенанса на deep-полосе → падение GW-I-6; impl, ломающий v1-подмножество
//! → падение GW-I-5. RED сейчас: `snapshot` = `unimplemented!()` → вывод недостижим (engine-dev).

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector, GATEWAY_SCHEMA_VERSION};
use journal::{EpochFilter, Journal, WriterConfig};
use serde::Deserialize;

/// v1-потребитель depth-строки: знает ТОЛЬКО базовые поля. `depth_band_provenance` (v2) —
/// незнаком и должен молча игнорироваться serde (доказательство аддитивности).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct DepthRowV1 {
    side: String,
    band_pct_e8: i64,
    series: Vec<(i64, i64)>,
}

fn build() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: 8192,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    // Уровни и близко (0.1%), и далеко (>3%) от mid ~65000 → 3%-полоса непуста.
    let bids: Vec<Level> = [64_990.0, 64_500.0, 63_000.0, 62_000.0]
        .iter()
        .map(|&p| Level {
            price: to_fixed(p),
            size: to_fixed(5.0),
        })
        .collect();
    let asks: Vec<Level> = [65_010.0, 65_500.0, 67_000.0, 68_000.0]
        .iter()
        .map(|&p| Level {
            price: to_fixed(p),
            size: to_fixed(5.0),
        })
        .collect();
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for i in 0..8i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::L2Snapshot {
                    bids: bids.clone(),
                    asks: asks.clone(),
                    ts_exch_ms: 1_752_000_000_000 + i * 1_000,
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
        bands: vec![0.001, 0.03], // 0.1% (≤1.3%) и 3% (deep → провенанс обязателен)
    }
}

#[test]
fn snapshot_carries_schema_version_and_is_v1_additive() {
    let dir = build();
    let snap = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST)
        .expect("snapshot");

    // GW-I-5: версия проставлена и равна константе gateway.
    assert_eq!(
        snap.schema_version, GATEWAY_SCHEMA_VERSION,
        "snapshot обязан нести schema_version = GATEWAY_SCHEMA_VERSION"
    );

    // Аддитивность: v1-потребитель depth-строк парсит вывод gateway, игнорируя v2-поле провенанса.
    let depth_json = serde_json::to_string(&snap.series.depth_series).expect("serialize depth");
    let v1: Vec<DepthRowV1> = serde_json::from_str(&depth_json)
        .expect("v1-потребитель обязан распарсить depth-вывод gateway (аддитивность нарушена)");
    assert!(
        !v1.is_empty(),
        "v1-потребитель обязан увидеть непустой depth_series"
    );
}

#[test]
fn deep_band_carries_provenance() {
    let dir = build();
    let snap = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST)
        .expect("snapshot");

    let deep_e8 = (0.03_f64 * 1e8) as i64;
    let mut saw_deep = false;
    for row in &snap.series.depth_series {
        if row.band_pct_e8 >= deep_e8 {
            saw_deep = true;
            let prov = row.depth_band_provenance.as_deref().unwrap_or("");
            assert!(
                !prov.is_empty(),
                "GW-I-6: полоса {}e-8 глубже 1.3% ОБЯЗАНА нести depth_band_provenance",
                row.band_pct_e8
            );
        }
    }
    assert!(
        saw_deep,
        "предусловие: snapshot обязан вернуть строку для deep-полосы 3% (обе стороны)"
    );
}
