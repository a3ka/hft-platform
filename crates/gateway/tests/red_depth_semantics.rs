//! RED `MD-I-8` (sacred, architect-only) — **СЕМАНТИКА, А НЕ ПРОВОДКА: вырожденный вход и
//! честность счётчика.**
//!
//! Милестоун `milestones/M-68-depth-from-book.md` rev5, задачи 13 и 14. Исполнение вердикта
//! `research/reviews/R-134-M-68-impl-depth-from-book.md`, блокеры **B-3** и **B-4**.
//!
//! Оба дефекта прошли ЗЕЛЁНЫЙ гейт (`verify_M-68.sh` exit=0) и зелёного тестера: первый —
//! потому что ни одна из девяти фикстур `d1..d8b` не подаёт одностороннюю книгу, второй —
//! потому что ни один оракул не исполняет путь `LiveReducer::pump`.
//!
//! # `d9` — GREEN-ПИН, и это объявлено, а не замаскировано
//!
//! Решение architect'а по `B-3`: при отсутствии середины точка в серию **НЕ ПИШЕТСЯ**.
//! Сегодняшняя реализация уже так и делает (`recompute_depth_from_book` уходит в
//! early-return). Значит `d9` **зелен с первого прогона** — и по нашей же норме это симптом,
//! требующий объяснения, а не повод радоваться (`testing.md`, анти-плацебо).
//!
//! Объяснение: `B-3` — не «реализация ведёт себя неверно», а «поведение НЕ СПЕЦИФИЦИРОВАНО».
//! Прежний код писал точку `0`, новый не пишет; форма выдачи изменилась молча, задачи на это
//! в §Tasks не было, и ни один оракул этого не пиннил. `d9` закрывает именно дыру
//! спецификации: он запрещает вернуться к `0`-точке.
//!
//! **Поэтому его сила предъявляется МУТАЦИЕЙ, а не прогоном.** Процедура записана в шапке
//! теста `d9` и обязана быть исполнена dev'ом в Done Block: вернуть безусловную вставку точки
//! ⇒ `d9` обязан ПОКРАСНЕТЬ. Не покраснел — оракул ничего не пиннит.
//!
//! # Почему `0` неверен по существу
//!
//! Полоса определена как «доля от середины». Нет середины — нет и границ полосы. Значение
//! `0` утверждает измерение ВНУТРИ интервала, которого не существует, и смешивает два разных
//! факта: «глубина пуста» и «мерить не от чего». Отсутствие точки не утверждает ничего —
//! и это честнее (`PL-I-7`).

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;
const NEAR_BAND: f64 = 0.001;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "MD-I-8 semantics fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![NEAR_BAND],
        window_ms: None,
    }
}

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("SETUP: tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("SETUP: open_with");
    for e in events {
        j.append(e).expect("SETUP: append");
    }
    j.flush().expect("SETUP: flush");
    dir
}

/// Снимок с ОБЕИМИ сторонами — контрольная форма.
fn two_sided(ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: vec![lvl(MID - 1.0, 5.0)],
            asks: vec![lvl(MID + 1.0, 5.0)],
            ts_exch_ms: ts,
        },
    )
}

/// Снимок ТОЛЬКО с bid-стороной: середина не определена, границы полосы не существуют.
fn one_sided(ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: vec![lvl(MID - 1.0, 5.0)],
            asks: vec![],
            ts_exch_ms: ts,
        },
    )
}

fn snap(dir: &std::path::Path) -> gateway::Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST)
        .expect("SETUP: snapshot обязан строиться")
}

// ═══════════════════════════════════════════════════════════════════════════════════════════
// d9 — вырожденный вход (задача 13, R-134 B-3)
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// **`d9` (КОНТРОЛЬ) — двусторонняя книга ДАЁТ точку.**
///
/// Без него `d9` ниже был бы зелен и против реализации, которая не пишет депт-серию НИКОГДА.
#[test]
fn md_i8_d9_c_two_sided_book_produces_a_point() {
    let dir = journal_of(vec![two_sided(T0)]);
    let s = snap(dir.path());
    let points: usize = s.series.depth_series.iter().map(|r| r.series.len()).sum();
    assert!(
        points > 0,
        "MD-I-8 d9-C SETUP: двусторонняя книга не дала НИ ОДНОЙ точки депт-серии (строк {}, \
         точек 0) — фикстура не производит предмета, и судить отсутствие точки на \
         односторонней книге не с чем",
        s.series.depth_series.len()
    );
}

/// **`d9` (задача 13) — односторонняя книга НЕ даёт точки, а не даёт точку со значением `0`.**
///
/// # Мутация, которой предъявляется сила этого оракула (Done Block dev'а обязателен)
///
/// В `recompute_depth_from_book` заменить early-return на безусловную вставку нулевой точки —
/// то есть вернуть поведение, которое было до `M-68`. `d9` обязан ПОКРАСНЕТЬ. Не покраснел ⇒
/// оракул не пиннит решение, и чинить надо оракул, а не радоваться зелёному.
#[test]
fn md_i8_d9_one_sided_book_writes_no_point_not_a_zero() {
    let dir = journal_of(vec![one_sided(T0)]);
    let s = snap(dir.path());
    let points: Vec<(i64, i64)> = s
        .series
        .depth_series
        .iter()
        .flat_map(|r| r.series.iter().copied())
        .collect();
    let zeros = points.iter().filter(|(_, v)| *v == 0).count();
    assert!(
        points.is_empty(),
        "MD-I-8 d9 (B-3): односторонняя книга дала {} точек депт-серии, из них {zeros} со \
         значением 0. Полоса определена как доля ОТ СЕРЕДИНЫ; середины нет, значит границ \
         полосы не существует, и значение 0 утверждает измерение внутри несуществующего \
         интервала. Отсутствие точки не утверждает ничего — решение спеки rev4 §0quinquies.3. \
         Точки: {points:?}",
        points.len()
    );
}
