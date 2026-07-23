//! RED M-23 HM-I-1/2/3/5 (sacred, architect-only) — heatmap (L2Delta-книга) + COB.
//!
//! Heatmap = матрица `(bucket, price, side) → размер` из L2Delta-реконструированной книги (M-29
//! apply_delta), окно `[mid·(1−W), mid·(1+W)]`, W=max(bands). Ячейка глубже 1.3% несёт провенанс
//! (VB-I-5, честность diff-реконструкции). COB = финальный стакан в окне.
//!
//! COMPILE-RED: поля `snap.series.{heatmap,cob}` и типы `HeatmapCell/CobLevel` ещё НЕ существуют.
//! engine-dev добавляет book-dep + аккумуляторы + поля + bump schema→5 → GREEN. Анти-плацебо:
//! heatmap игнорирует L2Delta → HM-I-1; уровень вне окна/без провенанса → HM-I-2.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_752_000_010_000; // один бакет (timeframe 1000)

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvls(v: &[(f64, f64)]) -> Vec<Level> {
    v.iter()
        .map(|&(p, s)| Level {
            price: to_fixed(p),
            size: to_fixed(s),
        })
        .collect()
}

fn snapshot_ev(bids: &[(f64, f64)], asks: &[(f64, f64)], ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lvls(bids),
            asks: lvls(asks),
            ts_exch_ms: ts,
        },
    )
}

fn delta_ev(bids: &[(f64, f64)], asks: &[(f64, f64)], u0: u64, u1: u64, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: lvls(bids),
            asks: lvls(asks),
            first_update_id: u0,
            final_update_id: u1,
            prev_final_update_id: None,
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

fn sel(bands: Vec<f64>) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands,
    }
}

fn snap(dir: &std::path::Path, s: &Selector) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, s, Cursor::LATEST).expect("snapshot")
}

#[test]
fn heatmap_reflects_l2delta_book() {
    // HM-I-1: снапшот + дельта (update+remove в ТОМ ЖЕ бакете) → heatmap показывает пост-дельта книгу.
    let dir = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0), (64_980.0, 3.0)], &[(65_010.0, 4.0)], T),
        delta_ev(&[(64_990.0, 9.0)], &[(65_010.0, 0.0)], 1, 2, T + 1), // 64990→9, удалить ask65010
    ]);
    let a = snap(dir.path(), &sel(vec![0.001]));
    let bid_at = |p: f64| {
        a.series
            .heatmap
            .iter()
            .find(|c| c.side == "bid" && c.price_e8 == to_fixed(p))
            .map(|c| c.size_e8)
    };
    assert_eq!(
        bid_at(64_990.0),
        Some(to_fixed(9.0)),
        "HM-I-1: L2Delta обновил bid 64990 → 9"
    );
    assert_eq!(
        bid_at(64_980.0),
        Some(to_fixed(3.0)),
        "неупомянутый bid неизменен"
    );
    assert!(
        !a.series
            .heatmap
            .iter()
            .any(|c| c.side == "ask" && c.price_e8 == to_fixed(65_010.0)),
        "HM-I-1: ask 65010 удалён дельтой (size=0) → нет ячейки"
    );
}

#[test]
fn heatmap_windowed_and_provenance() {
    // HM-I-2: окно ±W=3%; уровень глубже 1.3% несёт провенанс; уровень вне окна не эмитится.
    // mid = (64990+65010)/2 = 65000; окно [63050, 66950].
    let dir = journal_of(vec![snapshot_ev(
        &[(64_990.0, 5.0), (63_500.0, 2.0), (60_000.0, 1.0)], // 63500 (2.3%>1.3%), 60000 вне окна
        &[(65_010.0, 4.0)],
        T,
    )]);
    let a = snap(dir.path(), &sel(vec![0.03]));
    let cell = |p: f64| {
        a.series
            .heatmap
            .iter()
            .find(|c| c.side == "bid" && c.price_e8 == to_fixed(p))
    };
    // вне окна (60000 < 63050) — не эмитится.
    assert!(
        cell(60_000.0).is_none(),
        "HM-I-2: уровень вне окна ±3% не эмитится"
    );
    // глубокий (63500, 2.3% от mid) — провенанс обязателен.
    let deep = cell(63_500.0).expect("63500 в окне — ячейка есть");
    assert!(
        !deep
            .depth_band_provenance
            .as_deref()
            .unwrap_or("")
            .is_empty(),
        "HM-I-2: ячейка глубже 1.3% ОБЯЗАНА нести depth_band_provenance"
    );
    // близкий (64990, 0.015%) — провенанс не требуется.
    assert!(
        cell(64_990.0).is_some(),
        "близкий уровень в окне присутствует"
    );
}

#[test]
fn cob_is_final_book() {
    // HM-I-3: COB = финальный стакан (после дельты) в окне.
    let dir = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0), (65_020.0, 2.0)], T),
        delta_ev(&[(64_990.0, 9.0)], &[(65_010.0, 0.0)], 1, 2, T + 1),
    ]);
    let a = snap(dir.path(), &sel(vec![0.001]));
    let cob_size = |side: &str, p: f64| {
        a.series
            .cob
            .iter()
            .find(|l| l.side == side && l.price_e8 == to_fixed(p))
            .map(|l| l.size_e8)
    };
    assert_eq!(
        cob_size("bid", 64_990.0),
        Some(to_fixed(9.0)),
        "COB bid финальный"
    );
    assert_eq!(
        cob_size("ask", 65_020.0),
        Some(to_fixed(2.0)),
        "COB ask 65020"
    );
    assert!(cob_size("ask", 65_010.0).is_none(), "COB: ask 65010 удалён");
}

#[test]
fn determinism() {
    // HM-I-5: heatmap/cob байт-идентичны при повторе.
    let dir = journal_of(vec![
        snapshot_ev(&[(64_990.0, 5.0)], &[(65_010.0, 4.0)], T),
        delta_ev(&[(64_990.0, 7.0)], &[], 1, 2, T + 1),
    ]);
    let s = sel(vec![0.001]);
    let a = snap(dir.path(), &s);
    let b = snap(dir.path(), &s);
    assert_eq!(
        a.series.heatmap, b.series.heatmap,
        "heatmap недетерминирован"
    );
    assert_eq!(a.series.cob, b.series.cob, "cob недетерминирован");
}
