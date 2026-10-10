//! RED M-18 / CT-RFC-04 rev2 (sacred, architect-only): ROLLBACK-ISOLATION по ЭПОХЕ СХЕМЫ
//! (TD-031 — §8 нашёл, что провенанс-изоляция ВОИД в проде).
//!
//! **Прод-реальность (root cause TD-031):** recorder считает provenance через `git rev-parse` В
//! РАНТАЙМЕ (main.rs), но runtime-контейнер БЕЗ git → provenance = КОНСТАНТА
//! `"recorder v0.0.0 (git:no-git-info)"` на ВСЕХ деплоях. Прошлый оракул был зелёным лишь потому,
//! что ФИКСТУРА задавала РАЗНЫЙ provenance (P1≠P2) — он кодировал допущение, а не прод-режим
//! (`.claude/rules/testing.md`: фикстура-счастливый-путь ЭТАЖОМ ВЫШЕ). В проде provenance ОДИНАКОВ
//! → `decide_open_segment` reuse'ит pre-M18 сегмент → variant-6 (L2Delta) смешался в schema-2
//! сегмент (segment-55), quarantine стал невозможен.
//!
//! **Инвариант (машинный, git-независимый):** изоляция держится ЭПОХОЙ СХЕМЫ (`SCHEMA_VERSION`),
//! а не provenance. Сегмент schema-2 (без L2Delta) НЕ reuse'ится бинарём schema-3 ДАЖЕ при
//! ИДЕНТИЧНОМ provenance → L2Delta уходит в НОВЫЙ сегмент. Анти-плацебо: тест ПАДАЕТ на текущем
//! `decide_open_segment` (reuse по source+provenance+epoch, БЕЗ schema-гейта) — он бы reuse'нул
//! (1 сегмент); зеленеет после добавления `header.schema_version == SCHEMA_VERSION` в reuse.

use std::io::Write;

use contracts::{
    DataSource, Event, EventKind, Level, MdPayload, SegmentHeader, Side, Venue, SCHEMA_VERSION,
    SEGMENT_MAGIC,
};
use journal::{Journal, WriterConfig};

/// ПРОД-КОНСТАНТА provenance: git недоступен в контейнере → всегда одно и то же (TD-031).
const PROD_PROV: &str = "recorder v0.0.0 (git:no-git-info)";
const EPOCH: &str = "own-2026-07";

fn frame(w: &mut impl Write, payload: &[u8]) {
    w.write_all(&(payload.len() as u32).to_le_bytes()).unwrap();
    w.write_all(payload).unwrap();
    w.write_all(&crc32fast::hash(payload).to_le_bytes())
        .unwrap();
}

/// Записать v2-сегмент СТАРОЙ эпохи схемы (schema_version=2, БЕЗ L2Delta) с прод-провенансом:
/// магия + header-фрейм + N Trade-событий. Так выглядел активный сегмент ДО M-18.
fn write_schema2_segment(dir: &std::path::Path, n: u64) {
    let path = dir.join("segment-00000000.jrnl");
    let f = std::fs::File::create(path).expect("create");
    let mut w = std::io::BufWriter::new(f);
    w.write_all(&SEGMENT_MAGIC).unwrap();
    let header = SegmentHeader {
        schema_version: 2, // ПРЕ-L2Delta эпоха
        source: DataSource::OwnCapture,
        provenance: PROD_PROV.to_string(), // тот же provenance, что у M-18 бинаря (git-константа)
        epoch_id: EPOCH.to_string(),
        created_wall_ms: 1_752_000_000_000,
        first_seq: 0,
    };
    frame(&mut w, &postcard::to_stdvec(&header).unwrap());
    for seq in 0..n {
        let ev = Event {
            seq,
            ts_mono_ns: seq,
            ts_wall_ms: 1_752_000_000_000 + seq as i64,
            kind: EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: 6_500_000_000_000 + seq as i64,
                    size: 10_000_000,
                    side: Side::Buy,
                    ts_exch_ms: 1_752_000_000_000 + seq as i64,
                },
            ),
        };
        frame(&mut w, &postcard::to_stdvec(&ev).unwrap());
    }
    w.flush().unwrap();
    std::fs::write(dir.join("journal.meta"), n.to_le_bytes()).expect("meta");
}

fn l2delta() -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: vec![Level {
                price: 6_500_050_000_000,
                size: 30_000_000,
            }],
            asks: vec![],
            first_update_id: 101,
            final_update_id: 103,
            prev_final_update_id: None,
            ts_exch_ms: 1_752_000_000_499,
        },
    )
}

fn is_l2delta(k: &EventKind) -> bool {
    matches!(k, EventKind::Md(md) if matches!(md.payload, MdPayload::L2Delta { .. }))
}

/// Изоляция L2Delta держится ЭПОХОЙ СХЕМЫ при ИДЕНТИЧНОМ (прод-константа) provenance.
#[test]
fn l2delta_isolated_by_schema_epoch_under_constant_provenance() {
    // sanity: тест имеет смысл только если текущая эпоха ≥ 3 (L2Delta-эпоха). const-контекст —
    // избегаем clippy::assertions_on_constants (рантайм-assert на const = error под `-D warnings`).
    const {
        assert!(
            SCHEMA_VERSION >= 3,
            "L2Delta-эпоха обязана быть schema ≥ 3 (TD-031 fix)"
        )
    };

    let dir = tempfile::tempdir().expect("tempdir");
    // Фаза 1: активный сегмент ДО M-18 — schema 2, прод-провенанс, только Trade.
    write_schema2_segment(dir.path(), 5);
    assert_eq!(journal::list_segments(dir.path()).unwrap().len(), 1);

    // Фаза 2: ТЕКУЩИЙ (schema-3) бинарь с ТЕМ ЖЕ провенансом (git-константа НЕ меняется в проде!)
    // пишет L2Delta.
    let cfg = WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: PROD_PROV.to_string(), // ИДЕНТИЧЕН фазе 1
        epoch_id: EPOCH.to_string(),
    };
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        j.append(l2delta()).expect("append l2delta");
        j.flush().expect("flush");
    }

    let segs = journal::list_segments(dir.path()).unwrap();
    // ГВОЗДЬ: несмотря на ИДЕНТИЧНЫЙ provenance, L2Delta ОБЯЗАН уйти в НОВЫЙ сегмент — изоляция по
    // schema-эпохе, не по provenance. На текущем impl (reuse по provenance) тут 1 сегмент → FAIL.
    assert!(
        segs.len() >= 2,
        "L2Delta (schema {SCHEMA_VERSION}) ОБЯЗАН уйти в НОВЫЙ сегмент: активный schema=2 \
         несовместим. Provenance ИДЕНТИЧЕН (прод git-константа) — изоляция держится СХЕМОЙ, не \
         provenance (TD-031). Получено сегментов: {}",
        segs.len()
    );

    // Старый сегмент (schema 2) НЕ содержит L2Delta — он чист и quarantine-able.
    let seg0 = segs.iter().find(|s| s.index == 0).expect("segment 0");
    assert_eq!(
        seg0.header.schema_version, 2,
        "pre-M18 сегмент остался schema 2"
    );

    // Новый сегмент несёт ТЕКУЩУЮ схему (эпоха L2Delta).
    let newseg = segs
        .iter()
        .find(|s| s.header.schema_version == SCHEMA_VERSION)
        .expect("новый сегмент с текущей schema-эпохой");
    assert!(newseg.index > 0, "L2Delta-сегмент идёт после pre-M18");

    // Replay: L2Delta присутствует; seq строго возрастает (нет reuse через границу).
    let events = journal::read_all(dir.path()).unwrap();
    assert!(
        events.iter().any(|e| is_l2delta(&e.kind)),
        "L2Delta обязан быть в журнале"
    );
    let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    for w in seqs.windows(2) {
        assert!(w[1] > w[0], "seq строго возрастает (нет reuse): {seqs:?}");
    }
}
