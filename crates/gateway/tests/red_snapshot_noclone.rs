//! M-56 (`TD-097`) — **`snapshot()` не смеет клонировать состояние проекции**.
//!
//! ## Замер, из-за которого milestone существует
//!
//! M-54 устранил второй проход по журналу (догон стал в 3.9 раза дешевле), но close-out на
//! проде (`research/reviews/R-029-M-54.md` §C, 18 подключений) показал обратную сторону:
//!
//! ```text
//! ДО    M-54:  250 ms + 6.67 мкс/событие
//! ПОСЛЕ M-54:  654 ms + 1.70 мкс/событие
//! ```
//!
//! Константа выросла на **+404 ms**. Точка безубыточности — backlog ≈81 300 событий, а рабочий
//! диапазон прода 0…66 600: порог не достигается никогда, то есть на реальном проде
//! подключение стало ДОРОЖЕ при любом backlog'е.
//!
//! Причина — одна строка (`crates/gateway/src/lib.rs`, `LiveReducer::snapshot`):
//! `self.full.clone().finish_with_at()`. `Reducer::finish(self)` потребляет `self`, поэтому
//! метод с `&self` вынужден сперва склонировать ВЕСЬ редьюсер — включая то, что в выходные
//! серии не попадает (книга целиком, служебные буферы). На проде состояние ключа ≈20 MiB.
//!
//! ## Почему меряются АЛЛОКАЦИИ, а не время
//!
//! Урок TD-078, подтверждённый дважды: оракул с потолком wall-clock становится измерителем
//! CI-машины и флакает (так уже случилось с `td083_tick_wallclock_does_not_grow_with_history`,
//! падал 1 раз из 5). Аллоцированные байты детерминированы и от скорости раннера не зависят.
//! Копирование мегабайтов состояния счётчик видит безошибочно.
//!
//! COMPILE-RED: `Reducer::finish_ref(&self)` ещё не существует (задача 1 milestone'а).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

// УЧЁТ ПОТОКОВЫЙ, А НЕ ПРОЦЕССНЫЙ (флак required-чека `main`, 2026-08-16).
//
// Прежняя редакция вела `CUR`/`PEAK` в `AtomicUsize` на весь процесс, а `cargo test` гоняет
// три теста этого бинаря (`o1`/`o2`/`o3`) ПАРАЛЛЕЛЬНЫМИ потоками. `PEAK.fetch_max` брал
// максимум по ВСЕМУ процессу, поэтому аллокации соседа попадали прямо в замер `o1`, и
// отношение `alloc_big/alloc_small` становилось подбрасыванием монеты. Наблюдалось на ОДНОМ
// И ТОМ ЖЕ дереве: PR-прогон зелёный, прогон на `main` — красный (`f8f6ae2`).
//
// `testing.md` («Целостность гейта», свойство 2) требует ровно этого: ресурсный оракул на
// глобальном счётчике обязан быть single-threaded-по-замеру, конфаундинг-величину держать
// КОНСТАНТНОЙ, варьировать только измеряемую. Потоковый учёт делает конфаундер (соседний
// тест) структурно недостижимым, вместо того чтобы удерживать его дисциплиной запуска.
//
// `const`-инициализатор обязателен: он исключает ленивую инициализацию TLS, а значит и
// повторный вход в аллокатор из самого аллокатора. `try_with` — на время разрушения TLS,
// когда доступ уже невозможен: такую аллокацию мы просто не считаем (она вне замера).
thread_local! {
    static T_CUR: Cell<usize> = const { Cell::new(0) };
    static T_PEAK: Cell<usize> = const { Cell::new(0) };
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let _ = T_CUR.try_with(|cur| {
                let c = cur.get().saturating_add(l.size());
                cur.set(c);
                let _ = T_PEAK.try_with(|pk| {
                    if c > pk.get() {
                        pk.set(c);
                    }
                });
            });
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        let _ = T_CUR.try_with(|cur| cur.set(cur.get().saturating_sub(l.size())));
    }
}
#[global_allocator]
static GA: Counting = Counting;

/// Пиковая аллокация (дельта над базой) во время `f`.
fn peak_delta<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = T_CUR.with(|c| c.get());
    T_PEAK.with(|p| p.set(base));
    let r = f();
    let peak = T_PEAK.with(|p| p.get());
    (r, peak.saturating_sub(base))
}

const BASE_MS: i64 = 1_784_116_800_000;
const D2_MS: i64 = BASE_MS + 86_400_000;

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m56".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// **Узкое окно + широкая книга** — ключ к оракулу.
///
/// В выходные серии попадает только то, что внутри окна (единицы килобайт). В СОСТОЯНИИ при
/// этом лежит книга целиком — мегабайты. Клонирование редьюсера видно как разрыв на порядок
/// между «сколько отдали» и «сколько аллоцировали».
fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(60_000),
        depth_cadence_ms: None,
    }
}

/// Журнал с ШИРОКОЙ книгой (много уровней) и сделками по обе стороны границы UTC-суток.
/// Чек-лист `testing.md`: асимметричный дифф, множественность, две сессии, границы.
fn journal_wide_book(levels: usize, trades: i64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");

        // Широкий снапшот книги: `levels` уровней с каждой стороны.
        let bids: Vec<Level> = (0..levels)
            .map(|i| lvl(65_000.0 - i as f64, 1.0 + (i % 7) as f64))
            .collect();
        let asks: Vec<Level> = (0..levels)
            .map(|i| lvl(65_010.0 + i as f64, 1.0 + (i % 5) as f64))
            .collect();
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids,
                asks,
                ts_exch_ms: BASE_MS,
            },
        ))
        .expect("snap");

        for i in 0..trades {
            let day2 = i >= trades / 2;
            let base = if day2 { D2_MS } else { BASE_MS };
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + (i % 17) as f64),
                    size: to_fixed(0.5),
                    side: if i % 3 == 0 { Side::Sell } else { Side::Buy },
                    ts_exch_ms: base + (i % (trades / 2).max(1)) * 500,
                },
            ))
            .expect("trade");
        }

        // Асимметричный дифф: только аски; о бидах молчит — они обязаны выжить.
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Delta {
                bids: vec![],
                asks: vec![lvl(65_010.0, 0.5)],
                first_update_id: 1,
                final_update_id: 2,
                prev_final_update_id: None,
                ts_exch_ms: D2_MS + 1_000,
            },
        ))
        .expect("delta");
        j.flush().expect("flush");
    }
    dir
}

fn live_at_tail(dir: &std::path::Path) -> gateway::LiveReducer {
    let ckpt = tempfile::tempdir().expect("ckpt");
    let (mut live, _stats) =
        gateway::LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .expect("resume");
    for _ in 0..1_000 {
        let (frames, _c, _st) = live
            .pump(dir, EpochFilter::OwnCaptureOnly, 4_096)
            .expect("pump");
        if frames.is_empty() {
            break;
        }
    }
    std::mem::forget(ckpt); // чтобы каталог жил, пока жив `live`
    live
}

/// **O-1 (главный).** Стоимость `snapshot()` не растёт вместе с размером СОСТОЯНИЯ.
///
/// ## Почему сравнение двух размеров, а не абсолютный порог
///
/// Первая редакция сравнивала аллокации с размером выхода и требовала отношение `< 20×`.
/// Она **прошла на реализации С КЛОНОМ** (замер: 440 665 байт аллокаций против 42 847 байт
/// выхода, отношение 10.3) — то есть не доказывала ничего. Абсолютный порог здесь негоден:
/// он зависит от того, насколько «широкой» получилась фикстура, и подбирать его под ответ —
/// значит подгонять оракул.
///
/// Проверяемое свойство формулируется структурно: **выход зависит только от окна, поэтому
/// при одном и том же окне удвоение СОСТОЯНИЯ не должно удваивать работу**.
///
/// Две проекции, отличающиеся ТОЛЬКО глубиной книги (×8), при одинаковом окне дают
/// практически одинаковый выход. Значит:
/// - честная реализация (построение серий из ссылок): аллокации ≈ равны, отношение ≈1;
/// - реализация с клоном: аллокации растут вместе с книгой, отношение ≈8.
///
/// Порог `2.5×` лежит посередине с запасом в обе стороны и НЕ зависит ни от машины, ни от
/// абсолютного размера фикстуры — только от наличия копирования.
#[test]
fn o1_snapshot_allocation_does_not_grow_with_state() {
    let alloc_for = |levels: usize| -> (usize, usize) {
        let dir = journal_wide_book(levels, 600);
        let live = live_at_tail(dir.path());
        // Прогрев: первый вызов тянет ленивые инициализации, не относящиеся к предмету.
        let _ = live.snapshot();
        let (snap, allocated) = peak_delta(|| live.snapshot());
        let out = serde_json::to_vec(&snap).expect("serialize").len();
        assert!(
            !snap.series.ohlcv.is_empty(),
            "O-1: снапшот пуст при {levels} уровнях — фикстура не давит"
        );
        std::mem::forget(dir); // каталог должен пережить `live`
        (allocated, out)
    };

    let (alloc_small, out_small) = alloc_for(1_000);
    let (alloc_big, out_big) = alloc_for(8_000);

    // Анти-вырождение: выход обязан быть СОПОСТАВИМ — иначе растут не аллокации, а сам ответ,
    // и сравнение теряет смысл. Окно одно и то же, поэтому в выход идёт полоса вокруг mid,
    // а не вся книга.
    let out_ratio = out_big as f64 / out_small.max(1) as f64;
    assert!(
        out_ratio < 1.5,
        "O-1: выход вырос в {out_ratio:.2} раза (с {out_small} до {out_big} байт) при росте \
         книги ×8 — фикстура построена неверно: окно пропускает наружу глубину книги, и тест \
         сравнивает несравнимое"
    );

    let ratio = alloc_big as f64 / alloc_small.max(1) as f64;
    eprintln!(
        "ЗАМЕР O-1: книга ×8 → аллокации {alloc_small} → {alloc_big} (×{ratio:.2}), \
         выход {out_small} → {out_big} (×{out_ratio:.2})"
    );
    assert!(
        ratio < 2.5,
        "O-1 (TD-097): книга выросла в 8 раз — аллокации `snapshot()` выросли в {ratio:.1} раза \
         ({alloc_small} → {alloc_big} байт), при том что ВЫХОД не изменился (×{out_ratio:.2}). \
         Значит копируется СОСТОЯНИЕ, а не строится ответ. Именно это дало +404 ms константы \
         на проде (R-029 §C): на реальном ключе состояние ≈20 MiB, и клон платится на КАЖДОМ \
         подключении, при любом backlog'е."
    );
}

/// **O-2.** `finish_ref(&self)` поэлементно равен `finish(self)` — дешевле не значит иначе.
///
/// Эталон строится НЕЗАВИСИМЫМ путём: полный реплей журнала через `gateway::snapshot`, который
/// внутри идёт потребляющим `finish`. Сверка нетавтологична: два разных пути к одному ответу.
#[test]
fn o2_snapshot_equals_independent_replay() {
    let dir = journal_wide_book(2_000, 600);
    let live = live_at_tail(dir.path());
    let from_live = live.snapshot();

    let replay = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("independent replay");

    // Анти-вырождение: под окном обе серии обязаны быть непусты, иначе сверка выродится.
    assert!(
        !replay.series.cvd_session_base.is_empty(),
        "O-2: cvd_session_base пуст — фикстура не вызывает эвикцию, сравнение выродилось"
    );
    assert!(
        !replay.series.heatmap.is_empty(),
        "O-2: heatmap пуст — широкая книга не дошла до выхода, фикстура не давит"
    );

    assert_eq!(
        from_live.cursor, replay.cursor,
        "O-2: курсор снапшота из живого состояния != независимый реплей"
    );
    assert_eq!(
        from_live.series, replay.series,
        "O-2: серии разошлись — удешевление построения ИЗМЕНИЛО данные. Это хуже, чем медленно."
    );
    assert_eq!(
        from_live.history_truncated, replay.history_truncated,
        "O-2: флаг усечения истории разошёлся"
    );
}

/// **O-3.** `snapshot()` не мутирует и не «съедает» состояние.
///
/// `finish_ref` работает по ссылке — значит повторный вызов обязан дать тот же результат, а
/// последующий `pump` обязан продолжить с того же места. Деградированный вход: снапшот берётся
/// дважды ДО и дважды ПОСЛЕ догона.
#[test]
fn o3_snapshot_is_repeatable_and_non_destructive() {
    let dir = journal_wide_book(1_000, 400);
    let mut live = live_at_tail(dir.path());

    let a1 = live.snapshot();
    let a2 = live.snapshot();
    assert_eq!(
        a1.series, a2.series,
        "O-3: два подряд snapshot() дали разное — построение мутирует состояние"
    );
    assert_eq!(a1.cursor, a2.cursor, "O-3: курсор изменился от чтения");

    // Журнал растёт, догоняем — состояние обязано быть живым, а не опустошённым.
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("reopen");
        for i in 0..5i64 {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_100.0 + i as f64),
                    size: to_fixed(1.0),
                    side: Side::Buy,
                    ts_exch_ms: D2_MS + 10_000 + i * 100,
                },
            ))
            .expect("late trade");
        }
        j.flush().expect("flush");
    }
    let (frames, _c, _st) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump после дозаписи");
    assert!(
        !frames.is_empty(),
        "O-3: после дозаписи догон не дал кадров — snapshot() опустошил состояние"
    );

    let b1 = live.snapshot();
    let b2 = live.snapshot();
    assert_eq!(
        b1.series, b2.series,
        "O-3: после pump два подряд snapshot() снова разошлись"
    );
    assert_ne!(
        a1.cursor, b1.cursor,
        "O-3: курсор не сдвинулся после догона — фикстура не давит, приращение не дошло"
    );
}
