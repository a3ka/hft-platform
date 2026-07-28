//! RED M-38b (sacred, architect-only) — **GW-I-11 + GW-I-8: живой путь докармливается ХВОСТОМ,
//! а не пересчитывается от START на каждый тик.**
//!
//! Вторая половина TD-044, без которой первая бесполезна: `frames_since` сейчас на КАЖДОМ
//! live-тике (250 мс) досеивает состояние реплеем всего журнала (~400 с на проде), после чего
//! сворачивает хвост ≤256 событий. За один такой «тик» recorder успевает записать несопоставимо
//! больше — **live-push математически не сходится**. Чекпоинт в одиночку дал бы быстрый первый
//! кадр и мёртвый live.
//!
//! Решение: резюмируемый `LiveReducer` — состояние живёт МЕЖДУ тиками и докармливается только
//! новыми событиями через `journal::stream_from(cursor)`.
//!
//! COMPILE-RED: `gateway::LiveReducer` и `gateway::ReadStats` ещё не существуют.
//!
//! testing.md: п.6 **композиция стадий** — проверяется не одиночный `pump`, а ЦЕПОЧКА
//! «resume → pump → pump → …», т.е. ровно то, что крутится в соединении; п.7 **парный vantage** —
//! кадры обязаны быть байт-идентичны текущему `frames_since` (не «быстро, но иначе») И
//! ресурс обязан быть ограничен (не «идентично, но всё так же медленно»).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const D2_MS: i64 = 20_279 * DAY_MS;
const N: u64 = 1_200;
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
            size: to_fixed(1.0 + (i % 3) as f64),
            side: if i.is_multiple_of(2) {
                Side::Buy
            } else {
                Side::Sell
            },
            ts_exch_ms: D2_MS - (N as i64 * 100) + (i as i64 * 100),
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
    }
}

fn journal_upto(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for i in 0..n {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal::compact_closed_segments(dir.path(), 2, 3).expect("compact");
    dir
}

fn canon<T: serde::Serialize>(v: &T) -> Vec<u8> {
    serde_json::to_vec(v).expect("сериализация")
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Кадры резюм-пути БАЙТ-ИДЕНТИЧНЫ текущему frames_since (GW-I-8 контигуальность)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn pumped_frames_identical_to_frames_since() {
    let dir = journal_upto(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let s = sel();

    // Эталон: как сегодня — повторные frames_since с капом.
    let mut want: Vec<gateway::Frame> = Vec::new();
    let mut cur = Cursor::START;
    loop {
        let (frames, next) =
            gateway::frames_since(dir.path(), EpochFilter::OwnCaptureOnly, &s, cur, 100)
                .expect("frames_since");
        if frames.is_empty() {
            break;
        }
        want.extend(frames);
        assert_ne!(next, cur, "курсор обязан двигаться");
        cur = next;
    }

    // Резюм-путь: одно состояние, докорм хвостом.
    let (mut live, _st) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .expect("resume");
    let mut got: Vec<gateway::Frame> = Vec::new();
    loop {
        let (frames, _cursor, _stats) = live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 100)
            .expect("pump");
        if frames.is_empty() {
            break;
        }
        got.extend(frames);
    }

    assert_eq!(
        canon(&got),
        canon(&want),
        "GW-I-8/VB-I-2 НАРУШЕН: кадры резюмируемого пути НЕ байт-идентичны кадрам frames_since. \
         Ускорение, меняющее данные, — это не ускорение, а расхождение live vs replay."
    );
    assert_eq!(
        live.cursor(),
        cur,
        "финальный курсор резюм-пути обязан совпасть с курсором эталонного пути"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Композиция стадий (п.6): чекпоинт + докорм ≡ полный снапшот
// ─────────────────────────────────────────────────────────────────────────────

/// Сквозной инвариант соединения: `snapshot_from_checkpoint(C)` + свёртка кадров `pump` ≡
/// `snapshot(START, LATEST)`. Именно эта КОМПОЗИЦИЯ работает в проде; обе стадии по
/// отдельности могут быть зелёными, а их склейка — нет (docs/08 §Системные паттерны, п.1).
#[test]
fn checkpoint_plus_pumped_frames_equals_full_snapshot() {
    let dir = journal_upto(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let s = sel();

    // Чекпоинт в середине истории.
    let k = Cursor {
        upto_seq: Some(N / 2),
    };
    gateway::checkpoint::advance_to(dir.path(), ckpt.path(), &s, EpochFilter::OwnCaptureOnly, k)
        .expect("advance_to");

    let (mut snap, _stats) = gateway::snapshot_from_checkpoint(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        ckpt.path(),
        k,
    )
    .expect("snapshot_from_checkpoint");

    let (mut live, _st) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .expect("resume");
    loop {
        let (frames, _cursor, _stats) = live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 64)
            .expect("pump");
        if frames.is_empty() {
            break;
        }
        for f in &frames {
            snap.apply(f);
        }
    }

    let want = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot(START, LATEST)");
    assert_eq!(
        canon(&snap),
        canon(&want),
        "КОМПОЗИЦИЯ НАРУШЕНА: snapshot(C) + свёрнутые кадры ≢ snapshot(LATEST). Это тот самый \
         шов, на котором ловились TD-042 и TD-045 — под окном merge-путь обязан воспроизводить \
         критерии редьюсера буква в букву (VB-I-2/VB-I-10)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Граница ресурса: докорм НЕ перечитывает историю
// ─────────────────────────────────────────────────────────────────────────────

/// Парный vantage к п.1: `LiveReducer`, который на каждый `pump` заново реплеит журнал,
/// даёт байт-идентичные кадры и проходит первый тест. Падает здесь.
#[test]
fn pump_at_tail_is_bounded() {
    let dir = journal_upto(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let s = sel();
    let segs = journal::list_segments(dir.path()).expect("segments").len();

    // Догоняем до конца.
    let (mut live, _st) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .expect("resume");
    loop {
        let (frames, _c, _st) = live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, usize::MAX)
            .expect("pump");
        if frames.is_empty() {
            break;
        }
    }

    // Дописываем 3 новых события — ровно то, что делает recorder между тиками.
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("reopen");
        for i in N..N + 3 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }

    let (frames, _cursor, stats) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, usize::MAX)
        .expect("pump после дозаписи");
    assert!(!frames.is_empty(), "3 новых события обязаны дать кадр");

    assert!(
        stats.events_decoded <= 64,
        "GW-I-11 НАРУШЕН: на 3 новых события декодировано {} (в журнале {N}+3). Живой тик \
         перечитывает историю — TD-044 в части live НЕ вылечен, и live-push не сойдётся: за \
         время одного тика recorder пишет больше, чем тик успевает обработать.",
        stats.events_decoded
    );
    assert!(
        (stats.segments_opened as usize) <= 2,
        "GW-I-11: на 3 новых события открыто {} сегментов из {segs} — сегментный пропуск не \
         работает (на проде это 96 .zst на КАЖДЫЙ тик)",
        stats.segments_opened
    );
}

/// Парный vantage к бюджету: `resume` БЕЗ чекпоинта обязан честно отработать полный реплей
/// (и сообщить это в `ReadStats`), а не притвориться, что состояние уже готово.
#[test]
fn resume_without_checkpoint_reports_full_replay() {
    let dir = journal_upto(N);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let (_live, stats) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .expect("resume без чекпоинта");
    assert_eq!(
        stats.events_decoded, N,
        "без чекпоинта resume обязан прочитать весь журнал ({N}) и СКАЗАТЬ об этом; \
         получено {}. Счётчик, который всегда мал, обесценивает оракулы бюджета.",
        stats.events_decoded
    );
}
