//! RED `M-84` (sacred, architect-only) — **кадр несёт РОВНО четырнадцать строк `(band, side)`
//! в каноническом порядке.**
//!
//! Заведён закрытием `C-220` B-3. COMPILE-RED: `gateway::CANONICAL_DEPTH_BANDS` ещё нет.
//!
//! ## ПОЧЕМУ ЭТОТ ОРАКУЛ ОБЯЗАТЕЛЕН ОТДЕЛЬНО
//!
//! Спека обещала четырнадцать строк в двух местах, а прежний набор проверял только ВХОД
//! (валидатор) и НЕ проверял ВЫХОД. Критик назвал последствие точно: транспорт, который
//! отвергает чужой вход, но на успешном пути отдаёт ноль, одну или часть строк, удовлетворял
//! закоммиченному набору целиком.
//!
//! Обещание в тексте спеки — не оракул. Здесь оно предъявляется ИСПОЛНЕНИЕМ: снимок берётся
//! настоящей `gateway::snapshot` на фикстуре с ДВУСТОРОННЕЙ книгой.
//!
//! ## `testing.md` чек-лист
//! - п.1 **асимметрия** — bid и ask НЕСИММЕТРИЧНЫ по объёму и глубине: симметричная книга
//!   спрятала бы путаницу сторон (`bid_sums`/`ask_sums` перепутаны местами — зелено);
//! - п.2 **множественность** — уровней больше, чем полос, и несколько уровней в одну полосу;
//! - п.4 **границы** — уровень РОВНО на пороге полосы и уровень за `MAX_REL_DIST`.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const T: i64 = 1_700_000_000_000;

/// Книга с уровнями по ОБЕ стороны и на РАЗНОЙ глубине: от вплотную к mid до 55 % —
/// чтобы наполнились и узкие, и широкие полосы канонического набора.
fn book_event(mid: f64) -> EventKind {
    let mut bids = Vec::new();
    let mut asks = Vec::new();
    // Доли от mid, покрывающие все семь полос: 1 %, 2.5 %, 4 %, 7 %, 12 %, 25 %, 55 %.
    for (i, frac) in [0.01, 0.025, 0.04, 0.07, 0.12, 0.25, 0.55]
        .iter()
        .enumerate()
    {
        // АСИММЕТРИЯ (п.1): объёмы сторон различны и различны по уровням.
        bids.push(Level {
            price: to_fixed(mid * (1.0 - frac)),
            size: to_fixed((i + 1) as f64),
        });
        asks.push(Level {
            price: to_fixed(mid * (1.0 + frac)),
            size: to_fixed((i + 1) as f64 * 2.0),
        });
    }
    // МНОЖЕСТВЕННОСТЬ (п.2): второй уровень в ту же узкую полосу.
    bids.push(Level {
        price: to_fixed(mid * 0.995),
        size: to_fixed(9.0),
    });
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids,
            asks,
            ts_exch_ms: T,
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

fn canonical_sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: gateway::CANONICAL_DEPTH_BANDS.to_vec(),
        window_ms: None,
        depth_cadence_ms: None,
    }
}

#[test]
fn frame_carries_exactly_fourteen_rows() {
    let dir = journal_of(vec![book_event(65_000.0)]);
    let snap = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &canonical_sel(),
        Cursor::LATEST,
    )
    .expect("snapshot на канонический селектор обязан строиться");
    assert_eq!(
        snap.series.depth_series.len(),
        14,
        "кадр обязан нести РОВНО 14 строк: семь полос × две стороны. Получено {}. \
         Спека обещает это в двух местах, и обещание в тексте оракулом не является \
         (C-220 B-3): транспорт, отдающий подмножество строк, проходил прежний набор",
        snap.series.depth_series.len()
    );
}

#[test]
fn rows_cover_every_canonical_band_on_both_sides() {
    let dir = journal_of(vec![book_event(65_000.0)]);
    let snap = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &canonical_sel(),
        Cursor::LATEST,
    )
    .expect("snapshot");
    // Счёт строк мог бы сойтись при семи дублях одной полосы — проверяем ТОЖДЕСТВО пар.
    for b in gateway::CANONICAL_DEPTH_BANDS {
        let want = (b * 1e8).round() as i64;
        for side in ["bid", "ask"] {
            assert!(
                snap.series
                    .depth_series
                    .iter()
                    .any(|r| r.band_pct_e8 == want && r.side == side),
                "в кадре нет строки (полоса {b}, сторона {side}). Совпадение ЧИСЛА строк \
                 не есть совпадение состава — тот же класс, что полнота доставки в M-74"
            );
        }
    }
}

#[test]
fn sides_are_not_swapped() {
    // АСИММЕТРИЯ работает форсингом: объёмы ask вдвое больше bid по построению фикстуры.
    // Реализация, перепутавшая bid_sums и ask_sums местами, отдаёт 14 строк с верными
    // полосами и зелена по обоим тестам выше.
    let dir = journal_of(vec![book_event(65_000.0)]);
    let snap = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &canonical_sel(),
        Cursor::LATEST,
    )
    .expect("snapshot");
    let widest = (gateway::CANONICAL_DEPTH_BANDS.last().unwrap() * 1e8).round() as i64;
    let last = |side: &str| -> i64 {
        snap.series
            .depth_series
            .iter()
            .find(|r| r.band_pct_e8 == widest && r.side == side)
            .and_then(|r| r.series.last().map(|(_, v)| *v))
            .unwrap_or(0)
    };
    assert!(
        last("ask") > last("bid"),
        "стороны перепутаны: по построению фикстуры объёмы ask ВДВОЕ больше bid, а получено \
         bid={} ask={}. Симметричная фикстура этот дефект спрятала бы",
        last("bid"),
        last("ask")
    );
}
