//! RED M-38b (sacred, architect-only) — **`journal::stream_from(dir, filter, after_seq)`:
//! live-seek с сегментным пропуском, БЕЗ потери событий.**
//!
//! Половина (2) M-38b: без seek'а `frames_since` досеивает состояние реплеем ВСЕГО журнала на
//! каждый live-тик (~400 с на проде) — live-push математически не сходится, и чекпоинт в
//! одиночку даёт красивый первый кадр при мёртвом live (TD-044).
//!
//! COMPILE-RED: `journal::stream_from` и счётчики `EventStream::{events_decoded, segments_opened}`
//! ещё не существуют.
//!
//! ## Порядок приоритетов оракула: сначала КОРРЕКТНОСТЬ, потом граница ресурса
//!
//! Пропуск сегментов — оптимизация; потерянное событие — порча данных. Поэтому тесты полноты
//! (`no_events_lost_*`) идут первыми и гоняются на КАЖДОМ `after_seq` подряд, а не на выбранных.
//!
//! ## `first_seq == 0` — НЕ факт, а дефолт (класс TD-030)
//!
//! `crates/journal/src/segments.rs:509-512`: у legacy-сегмента (до CT-RFC-02) `first_seq`
//! неизвестен и подставляется `0` — «безопасный дефолт», причём комментарий в коде прямо говорит,
//! что потребителю нужен явный подсчёт через stream. При этом `seq` журнала начинается с 0,
//! поэтому у НОРМАЛЬНОГО сегмента 0 значение `first_seq` тоже `0` — по ЗНАЧЕНИЮ они
//! неразличимы. Различать обязано `schema_version` заголовка: пропускать можно только сегменты
//! с настоящим v2-заголовком, legacy — никогда.

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

const N: u64 = 800;
const SEG_BYTES: u64 = 16 * 1024;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: SEG_BYTES,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(100.0 + (i % 5) as f64),
            size: to_fixed(1.0),
            side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
            ts_exch_ms: 1_752_000_000_000 + i as i64 * 100,
        },
    )
}

fn multi_segment_journal(compact: bool) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..N {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    if compact {
        journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    }
    dir
}

fn seqs_from(dir: &std::path::Path, after: Option<u64>) -> Vec<u64> {
    journal::stream_from(dir, EpochFilter::OwnCaptureOnly, after)
        .expect("stream_from")
        .map(|e| e.expect("event").seq)
        .collect()
}

fn all_seqs(dir: &std::path::Path) -> Vec<u64> {
    journal::stream(dir, EpochFilter::OwnCaptureOnly)
        .expect("stream")
        .map(|e| e.expect("event").seq)
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. КОРРЕКТНОСТЬ — ни одного потерянного и ни одного лишнего события
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn no_events_lost_for_every_after_seq() {
    for compacted in [false, true] {
        let dir = multi_segment_journal(compacted);
        let all = all_seqs(dir.path());
        assert_eq!(all.len() as u64, N, "фикстура: ожидали {N} событий");

        for after in 0..N {
            let got = seqs_from(dir.path(), Some(after));
            let want: Vec<u64> = all.iter().copied().filter(|s| *s > after).collect();
            assert_eq!(
                got,
                want,
                "stream_from(after={after}) на журнале (compacted={compacted}) вернул НЕ ровно \
                 хвост seq > {after}. Потеря/дубль события в live-seek = порча данных кокпита \
                 (VB-I-2 live==replay). Получено {} событий, ожидалось {}",
                got.len(),
                want.len()
            );
        }
    }
}

/// `None` ≡ `stream` (полный проход), и `after` за концом журнала → пусто (границы, п.4).
#[test]
fn boundaries_none_and_past_end() {
    let dir = multi_segment_journal(true);
    assert_eq!(
        seqs_from(dir.path(), None),
        all_seqs(dir.path()),
        "stream_from(None) обязан быть эквивалентен stream() — иначе два разных пути чтения"
    );
    assert!(
        seqs_from(dir.path(), Some(N + 1_000)).is_empty(),
        "after за концом журнала → пустой хвост, а не паника и не полный проход"
    );
}

/// Граница СЕГМЕНТА: `after` = последний seq сегмента и он же −1/+1. Классическая точка,
/// где пропуск «на один сегмент больше/меньше» проходит мимо середины журнала.
#[test]
fn segment_boundary_exact() {
    let dir = multi_segment_journal(true);
    let all = all_seqs(dir.path());
    let segs = journal::list_segments(dir.path()).expect("segments");
    assert!(
        segs.len() >= 4,
        "нужен многосегментный журнал, есть {}",
        segs.len()
    );

    for s in segs.iter().skip(1) {
        let first = s.header.first_seq;
        for after in [first.saturating_sub(1), first, first + 1] {
            let got = seqs_from(dir.path(), Some(after));
            let want: Vec<u64> = all.iter().copied().filter(|x| *x > after).collect();
            assert_eq!(
                got, want,
                "граница сегмента (first_seq={first}), after={after}: хвост не совпал"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. ГРАНИЦА РЕСУРСА — пропуск действительно происходит
// ─────────────────────────────────────────────────────────────────────────────

/// Парный vantage к корректности: реализация `stream_from = stream().filter(seq > after)`
/// корректна, но НЕ решает TD-044 — она читает всё. Падает здесь.
#[test]
fn tail_seek_opens_only_tail_segments() {
    let dir = multi_segment_journal(true);
    let segs = journal::list_segments(dir.path()).expect("segments").len();
    assert!(segs >= 4, "нужен многосегментный журнал, есть {segs}");

    let mut st = journal::stream_from(dir.path(), EpochFilter::OwnCaptureOnly, Some(N - 5))
        .expect("stream_from");
    let n: usize = (&mut st).count();
    assert_eq!(n, 4, "хвост после N-5 — ровно 4 события");

    assert!(
        (st.segments_opened() as usize) < segs,
        "ГРАНИЦА РЕСУРСА НЕ ДЕРЖИТ: открыто {} сегментов из {segs} при запросе последних 4 \
         событий. Это `stream().filter(...)` — корректно, но TD-044 не вылечен: на проде это \
         96 .zst, каждый из которых распаковывается на КАЖДЫЙ live-тик.",
        st.segments_opened()
    );
    assert!(
        st.events_decoded() < N,
        "декодировано {} из {N} — весь журнал",
        st.events_decoded()
    );
}

/// Счётчики обязаны быть ЧЕСТНЫМИ (парный vantage): при `None` открыт весь журнал.
/// Заглушка «счётчики всегда 0/1» валится здесь.
#[test]
fn counters_report_full_pass_honestly() {
    let dir = multi_segment_journal(true);
    let segs = journal::list_segments(dir.path()).expect("segments").len();
    let mut st =
        journal::stream_from(dir.path(), EpochFilter::OwnCaptureOnly, None).expect("stream_from");
    let n = (&mut st).count() as u64;
    assert_eq!(n, N);
    assert_eq!(
        st.events_decoded(),
        N,
        "events_decoded обязан считать реально декодированные события"
    );
    assert_eq!(
        st.segments_opened() as usize,
        segs,
        "segments_opened обязан считать реально открытые сегменты"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. LEGACY — `first_seq == 0` не является фактом (TD-030)
// ─────────────────────────────────────────────────────────────────────────────

/// **Переписано после C-030 R2 (critic REJECT): прошлая версия была ПЛАЦЕБО.** Она
/// утверждала, что защищает legacy, но legacy-сегмента не строила (`assert legacy_count == 0`)
/// и проверяла обычную полноту v2-журнала. Реализация, пропускающая сегмент по правилу
/// `header.first_seq <= after_seq` БЕЗ проверки `schema_version`, проходила её и падала бы на
/// проде. Ровно тот класс, который комментарий обещал исключить.
///
/// Теперь фикстура строит НАСТОЯЩИЙ legacy-сегмент — байт-в-байт как боевой headerless
/// `segment-00000000.jrnl` (тот же приём, что `red_segments_epochs`/`red_prod_migration`):
/// сегмент без магии + декларация в `journal.legacy.json`, поверх — v2-сегменты.
///
/// **Почему это ловит дефект.** У legacy-сегмента `first_seq` СИНТЕЗИРОВАН как `0`
/// (`segments.rs:509-512` — «безопасный дефолт», не измеренное значение), хотя реально он
/// содержит события `0..LEGACY_N`. Правило пропуска по СОБСТВЕННОМУ `first_seq` даёт
/// `0 <= after` → сегмент пропускается ВСЕГДА → при `after < LEGACY_N-1` теряются его события.
/// Корректное правило смотрит на `first_seq` СЛЕДУЮЩЕГО сегмента (сегмент можно пропустить,
/// только если все его события ≤ after) и/или на `schema_version`.
const LEGACY_N: u64 = 200;

/// Записать боевой headerless-сегмент (без магии) + `journal.meta`, затем задекларировать его.
fn legacy_plus_v2_journal() -> tempfile::TempDir {
    use std::io::Write as _;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("segment-00000000.jrnl");
    {
        let f = std::fs::File::create(&path).expect("create legacy");
        let mut w = std::io::BufWriter::new(f);
        for seq in 0..LEGACY_N {
            let ev = contracts::Event {
                seq,
                ts_mono_ns: seq,
                ts_wall_ms: 1_752_000_000_000 + seq as i64,
                kind: trade(seq),
            };
            let payload = postcard::to_stdvec(&ev).expect("ser");
            w.write_all(&(payload.len() as u32).to_le_bytes()).unwrap();
            w.write_all(&payload).unwrap();
            w.write_all(&crc32fast::hash(&payload).to_le_bytes())
                .unwrap();
        }
        w.flush().unwrap();
    }
    // `journal.meta` — следующий seq (иначе новый писатель начнёт с 0 и seq поедут).
    std::fs::write(dir.path().join("journal.meta"), LEGACY_N.to_le_bytes()).expect("meta");

    // Операторская процедура: без декларации чтение fail-closed (CT-RFC-02).
    journal::declare_legacy(
        dir.path(),
        contracts::LegacySegmentDecl {
            file_name: "segment-00000000.jrnl".to_string(),
            fingerprint_sha256: journal::fingerprint(&path).expect("fingerprint"),
            size_bytes_at_decl: std::fs::metadata(&path).expect("meta").len(),
            source: DataSource::OwnCapture,
            provenance: "M-38b fixture: headerless prod-shaped segment".to_string(),
            epoch_id: contracts::LEGACY_EPOCH_ID.to_string(),
        },
    )
    .expect("declare_legacy");

    // Поверх legacy — обычные v2-сегменты (несколько, ротация по 16 KiB).
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with поверх legacy");
        for i in LEGACY_N..LEGACY_N + 400 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

#[test]
fn legacy_segment_is_never_skipped() {
    let dir = legacy_plus_v2_journal();
    let segs = journal::list_segments(dir.path()).expect("segments");

    // АНТИ-ПЛАЦЕБО: фикстура ОБЯЗАНА содержать настоящий legacy-сегмент. Если его нет —
    // тест ничего не проверяет, и это должно быть падением, а не тихим «ok» (C-030 R2).
    let legacy: Vec<_> = segs
        .iter()
        .filter(|s| s.header.schema_version == contracts::SCHEMA_VERSION_PRE_HEADER)
        .collect();
    assert_eq!(
        legacy.len(),
        1,
        "фикстура обязана содержать РОВНО один legacy-сегмент, иначе оракул слеп; \
         заголовки: {:?}",
        segs.iter()
            .map(|s| (s.index, s.header.schema_version, s.header.first_seq))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        legacy[0].header.first_seq, 0,
        "у legacy `first_seq` СИНТЕЗИРОВАН нулём — это и есть ловушка TD-030"
    );
    assert!(
        segs.len() >= 3,
        "нужен legacy + минимум два v2-сегмента, есть {}",
        segs.len()
    );

    let all = all_seqs(dir.path());
    assert_eq!(
        all.len() as u64,
        LEGACY_N + 400,
        "фикстура: legacy + v2 события обязаны читаться все"
    );

    // Полнота на КАЖДОМ after — включая позиции ВНУТРИ legacy-диапазона, где пропуск
    // legacy-сегмента уничтожает данные.
    for after in 0..all.len() as u64 {
        let got = seqs_from(dir.path(), Some(after));
        let want: Vec<u64> = all.iter().copied().filter(|s| *s > after).collect();
        assert_eq!(
            got,
            want,
            "stream_from(after={after}) потерял события. Если пропали seq из \
             0..{LEGACY_N} — реализация пропустила LEGACY-сегмент по его синтетическому \
             first_seq=0 (TD-030). Пропускать сегмент можно, только если ВСЕ его события \
             ≤ after, т.е. по first_seq СЛЕДУЮЩЕГО сегмента и/или с проверкой schema_version. \
             Получено {} событий, ожидалось {}",
            got.len(),
            want.len()
        );
    }
}
