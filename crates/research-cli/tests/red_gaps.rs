//! RED M-08 (sacred, architect-only) — E8: gap-статистика (находка critic C-005 M1).
//!
//! Recorder перезапускался 31 раз за цикл M-05/M-06; WS рвутся штатно. Метрика по дырявым
//! данным врёт МОЛЧА: пропущенные минуты выглядят как «рынок стоял». Оракул требует, чтобы
//! разрывы считались точно и попадали в детерминированный артефакт, на который обязан
//! ссылаться отчёт.
//!
//! Анти-плацебо: наивная реализация («посчитать число ConnDown») падает — тест строит поток,
//! где есть (а) дыра БЕЗ Sys-событий (тихая пропажа записи — самый опасный случай) и
//! (б) дыра, обрамлённая ConnDown/ConnUp; обе обязаны быть найдены, с точными длительностями.

use contracts::{DataSource, EventKind, MdPayload, Side, SysEvent, Venue};
use journal::EpochFilter;
use research_cli::data_quality::{self, DEFAULT_GAP_THRESHOLD_MS, GAP_REPORT_SCHEMA_VERSION};
use research_cli::grid::JournalSource;

/// Журнал пишет ts_wall_ms сам (часы писателя), поэтому для детерминированной фикстуры
/// разрывов события кладём НАПРЯМУЮ, как это делает venue-путь в проде: через append,
/// но с искусственно раздвинутыми часами эмулировать нельзя — значит, фикстура строится
/// сырыми фреймами (тот же приём, что в journal-тестах legacy-сегмента).
mod fixture {
    use super::*;
    use std::io::Write;

    pub fn trade(i: u64) -> EventKind {
        EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: contracts::to_fixed(65_000.0) + i as i64,
                size: contracts::to_fixed(0.01),
                side: Side::Buy,
                ts_exch_ms: 1_752_000_000_000 + i as i64,
            },
        )
    }

    /// Сегмент нового формата (магия + заголовок) с ЗАДАННЫМИ ts_wall_ms — чтобы разрывы
    /// были точными и детерминированными.
    pub fn write_segment(dir: &std::path::Path, events: &[(u64, i64, EventKind)]) {
        let header = contracts::SegmentHeader {
            schema_version: contracts::SCHEMA_VERSION,
            source: DataSource::OwnCapture,
            provenance: "gap fixture".to_string(),
            epoch_id: "own-test".to_string(),
            created_wall_ms: events.first().map(|e| e.1).unwrap_or(0),
            first_seq: events.first().map(|e| e.0).unwrap_or(0),
        };
        let f = std::fs::File::create(dir.join("segment-00000000.jrnl")).expect("create");
        let mut w = std::io::BufWriter::new(f);
        w.write_all(&contracts::SEGMENT_MAGIC).expect("magic");
        let hp = postcard::to_stdvec(&header).expect("ser header");
        w.write_all(&(hp.len() as u32).to_le_bytes()).unwrap();
        w.write_all(&hp).unwrap();
        w.write_all(&crc32fast::hash(&hp).to_le_bytes()).unwrap();

        for (seq, wall, kind) in events {
            let ev = contracts::Event {
                seq: *seq,
                ts_mono_ns: *seq,
                ts_wall_ms: *wall,
                kind: kind.clone(),
            };
            let p = postcard::to_stdvec(&ev).expect("ser ev");
            w.write_all(&(p.len() as u32).to_le_bytes()).unwrap();
            w.write_all(&p).unwrap();
            w.write_all(&crc32fast::hash(&p).to_le_bytes()).unwrap();
        }
        w.flush().unwrap();
        std::fs::write(
            dir.join("journal.meta"),
            (events.len() as u64).to_le_bytes(),
        )
        .expect("meta");
    }
}

const T0: i64 = 1_752_000_000_000;

/// Поток: 10 событий по 1с → **тихая дыра 60с** (без Sys) → 10 событий → ConnDown → **дыра
/// 30с** → ConnUp → 10 событий.
fn build(dir: &std::path::Path) {
    let mut evs: Vec<(u64, i64, EventKind)> = Vec::new();
    let mut seq = 0u64;
    let mut t = T0;

    for _ in 0..10 {
        evs.push((seq, t, fixture::trade(seq)));
        seq += 1;
        t += 1_000;
    }
    // Тихая дыра: 60 секунд без единого события и без Sys-маркеров.
    t += 60_000;
    for _ in 0..10 {
        evs.push((seq, t, fixture::trade(seq)));
        seq += 1;
        t += 1_000;
    }
    // Штатный реконнект: ConnDown → 30с тишины → ConnUp.
    evs.push((seq, t, EventKind::Sys(SysEvent::ConnDown(Venue::Binance))));
    seq += 1;
    t += 30_000;
    evs.push((seq, t, EventKind::Sys(SysEvent::ConnUp(Venue::Binance))));
    seq += 1;
    t += 1_000;
    for _ in 0..10 {
        evs.push((seq, t, fixture::trade(seq)));
        seq += 1;
        t += 1_000;
    }
    fixture::write_segment(dir, &evs);
}

#[test]
fn e8_gap_report_finds_silent_and_conn_bounded_gaps() {
    let dir = tempfile::tempdir().expect("tempdir");
    build(dir.path());

    let source = JournalSource {
        dir: dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
    };
    let r = data_quality::gaps(&source, DEFAULT_GAP_THRESHOLD_MS).expect("gaps");

    assert_eq!(r.schema_version, GAP_REPORT_SCHEMA_VERSION);
    assert_eq!(
        r.epoch_ids,
        vec!["own-test".to_string()],
        "отчёт обязан НАЗЫВАТЬ эпоху данных (иначе он не воспроизводим)"
    );
    assert_eq!(r.events_total, 32, "10 + 10 + 2 Sys + 10");
    assert_eq!(r.gap_threshold_ms, DEFAULT_GAP_THRESHOLD_MS);

    assert_eq!(
        r.gaps.len(),
        2,
        "обязаны быть найдены ОБЕ дыры: тихая (60с) и реконнект-обрамлённая (30с); \
         найдено: {:?}",
        r.gaps
    );

    let silent = &r.gaps[0];
    // duration = ЧИСТАЯ разница wall-clock между соседними событиями (61с: последнее
    // событие батча в T0+9с, следующее — в T0+70с). Никаких вычитаний «активного периода»:
    // на проде это занижало бы реальные дыры (архитектурная правка после SVR research-dev —
    // прежний ассерт 60_000 был ОШИБКОЙ теста и вынуждал изобретать искусственную семантику).
    assert_eq!(
        silent.duration_ms, 61_000,
        "тихая дыра = to_wall_ms − from_wall_ms, без «поправок»"
    );
    assert!(
        !silent.bounded_by_conn_events,
        "дыра БЕЗ Sys-маркеров обязана быть помечена как тихая — это самый опасный случай \
         (запись просто пропала, а метрика посчитает это за «спокойный рынок»)"
    );

    let reconnect = &r.gaps[1];
    assert_eq!(
        reconnect.duration_ms, 30_000,
        "реконнект-дыра: ConnDown → 30с → ConnUp"
    );
    assert!(
        reconnect.bounded_by_conn_events,
        "дыра между ConnDown и ConnUp обязана быть распознана как штатный реконнект"
    );

    assert_eq!(r.gap_total_ms, 91_000, "61с тихой + 30с реконнект");
    // coverage = 1 − 90_000 / (last − first); фиксируем как детерминированное число.
    let span = r.last_wall_ms - r.first_wall_ms;
    let expect_cov =
        ((span - r.gap_total_ms) as i128 * contracts::PRICE_SCALE as i128 / span as i128) as i64;
    assert_eq!(
        r.coverage_e8, expect_cov,
        "coverage_e8 = (span − gaps)/span ×1e8 — доля времени, реально покрытая данными"
    );
}

/// Артефакт детерминирован: два прогона → байт-идентичный JSON (RC-I-5; никаких
/// wall-clock полей о моменте генерации).
#[test]
fn e8_gap_artifact_is_deterministic() {
    let dir = tempfile::tempdir().expect("tempdir");
    build(dir.path());
    let out = tempfile::tempdir().expect("out");

    let source = JournalSource {
        dir: dir.path().to_path_buf(),
        filter: EpochFilter::OwnCaptureOnly,
    };
    let a = data_quality::gaps(&source, DEFAULT_GAP_THRESHOLD_MS).expect("gaps");
    data_quality::write_gap_artifact(&a, out.path()).expect("write");
    let first = std::fs::read(out.path().join("gaps-own-test.json")).expect("artifact");

    let b = data_quality::gaps(&source, DEFAULT_GAP_THRESHOLD_MS).expect("gaps");
    data_quality::write_gap_artifact(&b, out.path()).expect("write");
    let second = std::fs::read(out.path().join("gaps-own-test.json")).expect("artifact");

    assert_eq!(a, b, "два прогона над одним журналом обязаны совпасть");
    assert_eq!(
        first, second,
        "артефакт обязан быть байт-идентичен (никаких timestamp'ов генерации)"
    );
}
