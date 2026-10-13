//! `M-85` задача 3 — СТОРОЖ ЦЕНЫ ЗАТРАВКИ КНИГИ (sacred, architect-only).
//!
//! Милестоун `milestones/M-85-frames-book-continuity.md` §2bis.1. Инвариант — `VB-I-10`
//! (bounded-window, `docs/fa/viz-backend.md:207`): состояние ограничено ОКНОМ и КНИГОЙ, а не
//! длиной истории.
//!
//! # Почему ось — ДЛИНА ЖУРНАЛА, и почему прежняя ось была ошибкой
//!
//! rev1 спеки строила выбор на оси «полный скан против подъёма из чекпоинта» (К1/К2). Ось была
//! ЛОЖНОЙ, и `C-223` B-2 это предъявил: `gateway::frames_since` УЖЕ открывает полный
//! `journal::stream` (`crates/gateway/src/lib.rs:2966-2975`), то есть уже является К1, и делает
//! это НАМЕРЕННО — соседний комментарий (`:2956-2965`) запрещает делегировать его seek-варианту,
//! потому что seek структурно ломает since-genesis семантику `seed_vwap`.
//!
//! Следствие, меняющее предмет задачи 3: события ДО `after` УЖЕ читаются и УЖЕ передаются
//! редьюсеру (`reduce_event_stream` зовёт на них `seed_vwap`). Значит затравка книги **не
//! добавляет ни одного чтения**. Выбирать между кандидатами нечего — и мерить надо не «сколько
//! читаем», а **не начало ли состояние расти с длиной истории**.
//!
//! # Что именно опасно — назван КОНКРЕТНЫЙ неверный способ, а не «неэффективность вообще»
//!
//! Затравку книги можно реализовать двумя способами, и они неразличимы по результату:
//!
//! · ВЕРНЫЙ — применять `L2Snapshot`/`L2Delta` к КНИГЕ редьюсера. Книга ограничена
//!   инструментом (`MAX_REL_DIST`), поэтому стоимость от длины журнала НЕ зависит;
//! · НЕВЕРНЫЙ — копить per-event наблюдения в буфер (как `capture_book_observations` для
//!   `pump`) и разбирать их в конце. Буфер — `O(журнал)`, и на длинной истории это
//!   host-OOM того же класса, что `VB-I-10` ловил на `snapshot` (прод: RSS 7.3 GB).
//!
//! Спека запрещает второй способ прозой (§2.1). **Проза без сторожа — не запрет**, и этот файл
//! есть сторож.
//!
//! # СТАТУС ОРАКУЛА: ЗЕЛЁН СЕГОДНЯ, И ЭТО ОБЪЯВЛЕНО, А НЕ СКРЫТО
//!
//! Сегодня затравки нет вовсе ⇒ аллокации от длины журнала не зависят ⇒ оракул зелен. Он НЕ
//! «шаг, позеленевший раньше своей задачи»: его предмет — не прогресс, а ГРАНИЦА, которую
//! развязка не смеет перейти. Он обязан остаться зелёным после верного фикса и покраснеть
//! против неверного.
//!
//! **Различающая сила предъявлена мутацией** (Done Block architect'а, `M-85` §4): в отдельном
//! дереве `reduce_event_stream` копит затравочные события в `Vec` вместо применения к книге —
//! отношение уходит за потолок, оракул краснеет.
//!
//! # Методика — те же три правила, что у `red_m77_pump_cost.rs`
//!
//! 1. мера — СУММА аллоцированных байт, а не время: время меряет CI-машину (урок `TD-078`);
//! 2. варьируется РОВНО ОДНА величина — число событий ДО `after`. Книга, окно, число
//!    сегментов и число событий ПОСЛЕ `after` держатся КОНСТАНТНЫМИ
//!    (`testing.md` «Целостность гейта», свойство 2);
//! 3. замер single-threaded по построению: счётчик потоко-локален, тест не ветвится.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

thread_local! {
    static T_TOTAL: Cell<usize> = const { Cell::new(0) };
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let _ = T_TOTAL.try_with(|t| t.set(t.get().saturating_add(l.size())));
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
    }
}
#[global_allocator]
static GA: Counting = Counting;

/// СУММА аллоцированных байт за время `f`.
fn total_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = T_TOTAL.with(|c| c.get());
    let r = f();
    (r, T_TOTAL.with(|c| c.get()).saturating_sub(base))
}

const T0: i64 = 1_752_000_010_000;
const MID: f64 = 65_000.0;

/// Короткая история и длинная — ось измерения. Множитель 4, как у `M-77`: достаточно, чтобы
/// линейный рост вышел за любой разумный потолок, и мало, чтобы тест оставался быстрым.
const SEED_SHORT: i64 = 200;
const SEED_LONG: i64 = 800;

/// Событий ПОСЛЕ курсора — КОНСТАНТА в обоих замерах: это и есть конфаундер, который обязан
/// держаться неподвижным, иначе сравнивались бы два разных объёма работы.
const TAIL_EVENTS: i64 = 8;

/// Потолок отношения. Верный фикс даёт ≈1 (работа пропорциональна КНИГЕ, книга не растёт);
/// буферизующий даёт ≈4 при множителе истории 4. Порог лежит посередине и НЕ подобран под
/// ответ: он отделяет «состояние ограничено книгой» от «состояние растёт с историей».
const RATIO_CEILING: f64 = 2.0;

fn setup_failed(what: &str) -> ! {
    panic!("SETUP НЕ СОСТОЯЛСЯ: {what} — тест НЕ судил предмет, зелёное было бы вакуумом");
}

/// `max_segment_bytes` заведомо больше фикстуры: число СЕГМЕНТОВ обязано совпадать в обоих
/// замерах, иначе оно само стало бы варьируемой величиной.
fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 256 << 20,
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

/// Якорный снимок ПОСТОЯННОГО размера — книга не варьируется.
fn book_at(ts: i64) -> EventKind {
    let mut bids = vec![(MID - 1.0, 5.0)];
    let mut asks = vec![(MID + 1.0, 5.0)];
    for k in 1..=8 {
        let f = 0.05 * (k as f64) / 8.0;
        bids.push((MID * (1.0 - f), 1.0));
        asks.push((MID * (1.0 + f), 1.0));
    }
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: lvls(&bids),
            asks: lvls(&asks),
            ts_exch_ms: ts,
        },
    )
}

/// Дельта ПОСТОЯННОГО размера: одна и та же пара уровней «дышит».
fn delta_at(ts: i64, uid: u64, size: f64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: lvls(&[(MID * 0.96, size)]),
            asks: lvls(&[(MID * 1.04, size)]),
            first_update_id: uid,
            final_update_id: uid,
            prev_final_update_id: None,
            ts_exch_ms: ts,
        },
    )
}

fn sel() -> Selector {
    gateway::set_effective_heatmap_window_frac(0.10);
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001, 0.02],
        window_ms: None,
        depth_cadence_ms: None,
    }
}

/// Журнал: `seed_events` событий ДО курсора + ровно `TAIL_EVENTS` после.
/// Возвращает каталог и курсор, отделяющий затравку от хвоста.
fn journal_with_history(seed_events: i64) -> (tempfile::TempDir, Cursor) {
    let dir = tempfile::tempdir().expect("tempdir");
    let last_seed_seq;
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        // Якорь — ОДИН, в самом начале: дальше только дельты, чтобы хвост был delta-only и
        // предмет (книга нужна затравке) существовал.
        let mut seq = j.append(book_at(T0)).expect("anchor").seq;
        for i in 1..=seed_events {
            let ts = T0 + i;
            let size = 1.0 + ((i % 3) as f64);
            seq = j.append(delta_at(ts, i as u64, size)).expect("seed").seq;
        }
        last_seed_seq = seq;
        for i in 1..=TAIL_EVENTS {
            let ts = T0 + seed_events + i * 1_000;
            j.append(delta_at(ts, (seed_events + i) as u64, 7.0))
                .expect("tail");
        }
        j.flush().expect("flush");
    }
    (dir, Cursor::at(last_seed_seq))
}

/// Замер: аллокации ОДНОГО вызова `frames_since` при заданной длине затравки.
fn measure(seed_events: i64) -> usize {
    let (dir, after) = journal_with_history(seed_events);
    let s = sel();
    // Прогрев ВНЕ замера: первый вызов тянет ленивую инициализацию (кеши, форматтеры),
    // и без него короткий замер нёс бы её целиком, а длинный — нет.
    let _ = gateway::frames_since(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        after,
        usize::MAX,
    )
    .expect("warmup");
    let (frames, bytes) = total_delta(|| {
        gateway::frames_since(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &s,
            after,
            usize::MAX,
        )
        .expect("frames_since")
    });
    if frames.0.is_empty() {
        setup_failed("кадров НОЛЬ — хвоста нет, замерялась пустая работа");
    }
    bytes
}

/// СТОРОЖ ГРАНИЦЫ. Работа `frames_since` не смеет расти пропорционально ДЛИНЕ ИСТОРИИ.
///
/// Зелен сегодня (затравки нет). Обязан остаться зелёным после ВЕРНОГО фикса (книга
/// ограничена инструментом) и покраснеть против буферизующего (`O(журнал)`), который §2.1
/// милестоуна запрещает прозой.
#[test]
fn m85_4_bootstrap_cost_does_not_grow_with_history_length() {
    let short = measure(SEED_SHORT);
    let long = measure(SEED_LONG);

    if short == 0 || long == 0 {
        setup_failed("нулевые аллокации — счётчик не сработал, сравнение бессмысленно");
    }

    let ratio = long as f64 / short as f64;
    assert!(
        ratio <= RATIO_CEILING,
        "VB-I-10 / M-85 §2.1 НАРУШЕН: работа `frames_since` растёт с ДЛИНОЙ ИСТОРИИ.\n  \
         затравка {SEED_SHORT} событий → {short} Б\n  \
         затравка {SEED_LONG} событий → {long} Б\n  \
         отношение {ratio:.3} при потолке {RATIO_CEILING}\n\
         Книга ограничена инструментом (`MAX_REL_DIST`), поэтому ВЕРНАЯ затравка даёт ≈1. \
         Рост означает, что затравка КОПИТ per-event наблюдения вместо применения к книге — \
         буфер `O(журнал)`, тот же класс, что host-OOM на `snapshot` (`VB-I-10`)."
    );
}

/// СТРАЖ ОСИ. Предъявляет, что варьируется РОВНО ОДНА величина: хвост (работа ПОСЛЕ курсора)
/// в обоих замерах одинаков.
///
/// Без него `m85_4` мог бы сравнивать два разных объёма работы и молчать о настоящем предмете —
/// конфаундер обязан держаться константным, а не подразумеваться (`testing.md`, свойство 2).
#[test]
fn m85_5_axis_guard_tail_work_is_constant_across_both_measurements() {
    let s = sel();
    let mut counts = Vec::new();
    for seed in [SEED_SHORT, SEED_LONG] {
        let (dir, after) = journal_with_history(seed);
        let (frames, _cur) = gateway::frames_since(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &s,
            after,
            usize::MAX,
        )
        .expect("frames_since");
        if frames.is_empty() {
            setup_failed("кадров НОЛЬ — ось не построена");
        }
        let pts: usize = frames
            .iter()
            .map(|f| {
                f.delta
                    .depth_series
                    .iter()
                    .map(|r| r.series.len())
                    .sum::<usize>()
            })
            .sum();
        counts.push(pts);
    }
    assert_eq!(
        counts[0], counts[1],
        "ОСЬ НЕ ЧИСТА: работа ПОСЛЕ курсора различается между замерами ({} против {}), значит \
         `m85_4` сравнивал бы не длину истории, а два разных хвоста. Конфаундер обязан быть \
         КОНСТАНТНЫМ.",
        counts[0], counts[1]
    );
}
