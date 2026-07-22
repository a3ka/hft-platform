//! RED M-22 GW-I-3 / GW-I-4 (sacred, architect-only) — live == replay + snapshot-completeness.
//!
//! Ядро кокпита: серия, посчитанная на live-хвосте, БАЙТ-идентична серии из replay того же окна
//! (VB-I-2). И клиент, подключившийся В СЕРЕДИНЕ (курсор C), после `snapshot(C) + frames_since(C)`
//! получает ТО ЖЕ, что полный пересчёт (GW-I-4) — без дрейфа snapshot+deltas против свёртки-с-нуля.
//!
//! Деградированные входы (`.claude/rules/testing.md` чек-лист) ВСТРОЕНЫ в фикстуру:
//!  - АСИММЕТРИЯ: L2Snapshot, где меняется ТОЛЬКО bid-сторона (ask как прежде);
//!  - МНОЖЕСТВЕННОСТЬ: две сделки в ОДНОМ бакете, РАЗРЕЗАННЫЕ курсором C (snapshot берёт первую,
//!    frames — вторую → `apply` ОБЯЗАН слить их в один бакет, а не создать дубль);
//!  - ГРАНИЦА: `max_segment_bytes` мал → окно пересекает границу сегмента (стрим обязан сшить).
//!
//! Анти-плацебо: impl, который append'ит дубль-бакет вместо merge (GW-I-4), даёт acc ≠ full →
//! падение. RED сейчас: `snapshot`/`frames_since`/`replay`/`apply` = `unimplemented!()` (engine-dev).

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Frame, Selector, Snapshot};
use journal::{EpochFilter, Journal, WriterConfig};

const T0: i64 = 1_752_000_000_000; // начало бакета B0 (timeframe 1000ms → bucket = ts/1000)

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
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

fn l2(bids: &[(f64, f64)], asks: &[(f64, f64)], ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: bids.iter().map(|&(p, s)| lvl(p, s)).collect(),
            asks: asks.iter().map(|&(p, s)| lvl(p, s)).collect(),
            ts_exch_ms: ts,
        },
    )
}

/// Детерминированная фикстура с деградациями. Возвращает (dir, seqs) — seq в порядке append.
/// `max_segment_bytes` мал → несколько сегментов (ГРАНИЦА).
fn build() -> (tempfile::TempDir, Vec<u64>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: 200, // мал + крупные L2 → ротация ~каждое событие (граница сегмента)
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    // Near-touch уровни (влияют на depth 0.1%) + DEEP-padding (>0.1%, НЕ влияют на 0.1%, но
    // раздувают событие → ротация сегментов). Асимметрия проверяется на near-touch bid.
    let pad_b: &[(f64, f64)] = &[
        (60_000.0, 9.0),
        (58_000.0, 9.0),
        (55_000.0, 9.0),
        (50_000.0, 9.0),
        (45_000.0, 9.0),
        (40_000.0, 9.0),
    ];
    let pad_a: &[(f64, f64)] = &[
        (70_000.0, 9.0),
        (72_000.0, 9.0),
        (75_000.0, 9.0),
        (80_000.0, 9.0),
        (85_000.0, 9.0),
        (90_000.0, 9.0),
    ];
    let cat = |near: &[(f64, f64)], pad: &[(f64, f64)]| -> Vec<(f64, f64)> {
        near.iter().chain(pad.iter()).copied().collect()
    };
    let both = (
        cat(&[(64_990.0, 3.0), (64_980.0, 2.0)], pad_b),
        cat(&[(65_010.0, 4.0), (65_020.0, 1.0)], pad_a),
    );
    // Порядок событий (seq = порядок). Индексы важны для выбора C.
    let events = vec![
        l2(&both.0, &both.1, T0),                      // 0: книга B0
        trade(65_000.0, 1.0, Side::Buy, T0 + 10),      // 1: сделка#1 в B0  <-- C режет ЗДЕСЬ
        trade(65_000.0, 2.0, Side::Sell, T0 + 20),     // 2: сделка#2 в B0 (МНОЖЕСТВЕННОСТЬ, тот же бакет)
        // 3: АСИММЕТРИЯ — меняется ТОЛЬКО bid (ask те же уровни), бакет B1
        l2(&[(64_995.0, 9.0), (64_980.0, 2.0)], &both.1, T0 + 1_000),
        trade(65_005.0, 1.5, Side::Buy, T0 + 1_010),   // 4: сделка в B1
        l2(&both.0, &both.1, T0 + 2_000),              // 5: книга B2
        trade(64_990.0, 0.5, Side::Sell, T0 + 2_030),  // 6: сделка в B2
        l2(&both.0, &both.1, T0 + 3_000),              // 7: книга B3 (доп. события → ≥2 сегмента)
    ];
    let mut seqs = Vec::new();
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for e in events {
            seqs.push(j.append(e).expect("append").seq);
        }
        j.flush().expect("flush");
    }
    (dir, seqs)
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
    }
}

fn json<T: serde::Serialize>(x: &T) -> String {
    serde_json::to_string(x).expect("serialize")
}

fn fold(base: Snapshot, frames: &[Frame]) -> Snapshot {
    let mut acc = base;
    for f in frames {
        acc.apply(f);
    }
    acc
}

#[test]
fn boundary_is_multi_segment() {
    // Предусловие фикстуры: окно РЕАЛЬНО пересекает границу сегмента (иначе «граница» не тестируется).
    let (dir, _seqs) = build();
    let n = journal::list_segments(dir.path()).expect("segments").len();
    assert!(n >= 2, "фикстура обязана дать ≥2 сегмента (граница), а дала {n}");
}

#[test]
fn snapshot_equals_folded_frames_from_start() {
    // Чистый live == replay: свёртка всех кадров от пустого снапшота == полный snapshot.
    let (dir, _seqs) = build();
    let s = sel();
    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot full");
    let empty = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::START)
        .expect("snapshot start");
    let frames = gateway::replay(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        Cursor::LATEST,
    )
    .expect("replay");
    let acc = fold(empty, &frames);
    assert_eq!(
        json(&acc.series),
        json(&full.series),
        "live==replay: свёртка кадров с нуля обязана совпасть с полным snapshot"
    );
}

#[test]
fn mid_stream_snapshot_completeness_merges_same_bucket() {
    // GW-I-4 + МНОЖЕСТВЕННОСТЬ: C режет между двумя сделками ОДНОГО бакета B0.
    // snapshot(C) содержит сделку#1; frames_since(C) содержит сделку#2 → apply ОБЯЗАН слить их
    // в тот же бакет B0 (volume=1+2, знаковая дельта=+1−2), а НЕ создать второй бакет B0.
    let (dir, seqs) = build();
    let s = sel();
    let c = Cursor::at(seqs[1]); // включительно до сделки#1 (индекс 1)

    let full = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot full");
    let base = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, c)
        .expect("snapshot mid");
    let (frames, _end) =
        gateway::frames_since(dir.path(), EpochFilter::OwnCaptureOnly, &s, c).expect("frames_since");
    let acc = fold(base, &frames);
    assert_eq!(
        json(&acc.series),
        json(&full.series),
        "GW-I-4: snapshot(C)+frames(C..) обязан == полной свёртке (merge бакета, не дубль)"
    );
}

#[test]
fn snapshot_and_replay_are_deterministic() {
    // Детерминизм: повторные вызовы байт-идентичны (RC-I-5 класс).
    let (dir, _seqs) = build();
    let s = sel();
    let a = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snap a");
    let b = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snap b");
    assert_eq!(json(&a), json(&b), "snapshot недетерминирован");

    let r1 = gateway::replay(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        Cursor::LATEST,
    )
    .expect("replay1");
    let r2 = gateway::replay(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        Cursor::START,
        Cursor::LATEST,
    )
    .expect("replay2");
    assert_eq!(json(&r1), json(&r2), "replay недетерминирован");
}
