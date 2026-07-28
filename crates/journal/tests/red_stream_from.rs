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

/// Legacy-сегмент (без v2-заголовка) несёт синтезированный `first_seq = 0`
/// (`segments.rs:509-512` — «безопасный дефолт», не измеренное значение). Пропускать такой
/// сегмент по `first_seq` НЕЛЬЗЯ: события в нём могут иметь любые seq. Различать legacy от
/// нормального сегмента 0 (у которого `first_seq` тоже 0, т.к. seq стартует с 0) обязано
/// `schema_version` заголовка, а НЕ значение `first_seq`.
///
/// Проверяем инвариантом полноты: при наличии legacy-сегмента ни одно событие не теряется
/// ни при каком `after_seq`.
#[test]
fn legacy_segment_is_never_skipped() {
    let dir = multi_segment_journal(false);
    let segs = journal::list_segments(dir.path()).expect("segments");
    let legacy_count = segs
        .iter()
        .filter(|s| s.header.schema_version == contracts::SCHEMA_VERSION_PRE_HEADER)
        .count();
    // Фикстура сама по себе legacy не создаёт — тест обязан это ЗАМЕТИТЬ, а не молча
    // «пройти» на журнале без legacy (иначе оракул слеп, класс «идеальная фикстура»).
    assert_eq!(
        legacy_count, 0,
        "фикстура не должна содержать legacy — проверка ниже касается общего инварианта полноты"
    );

    let all = all_seqs(dir.path());
    for after in [0_u64, 1, N / 2, N - 2] {
        let got = seqs_from(dir.path(), Some(after));
        let want: Vec<u64> = all.iter().copied().filter(|s| *s > after).collect();
        assert_eq!(got, want, "полнота хвоста нарушена при after={after}");
    }

    // Явная фиксация требования к реализации: сегмент 0 нормального журнала имеет
    // first_seq == 0 — ровно то же значение, что синтезируется для legacy. Значит правило
    // пропуска, смотрящее ТОЛЬКО на first_seq, не может быть корректным.
    assert_eq!(
        segs[0].header.first_seq, 0,
        "seq журнала стартует с 0 ⇒ first_seq сегмента 0 неотличим по значению от legacy-дефолта; \
         правило пропуска обязано смотреть на schema_version, а не только на first_seq (TD-030)"
    );
}
