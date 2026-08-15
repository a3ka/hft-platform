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
//!
//! ## Почему счётчик ПО-ПОТОЧНЫЙ и КУМУЛЯТИВНЫЙ (`TD-098`/`TD-129`, 2026-08-15)
//!
//! Первая редакция считала ПИК ЖИВЫХ байт по ГЛОБАЛЬНЫМ статикам (`static CUR`/`static PEAK`).
//! `cargo test` гоняет тесты этого бинаря параллельными потоками ОДНОГО процесса, поэтому в
//! окно замера O-1 втекала аллокационная активность соседей — оракул мерил окружение, а не
//! свой инвариант (`testing.md`, «Целостность гейта» п.2).
//!
//! Замер на дереве `551668a` (предмет НЕ менялся ни в одном прогоне, менялся только раскид
//! потоков раннером):
//!
//! ```text
//! параллельно (12 прогонов):   alloc_small ∈ {19547 … 63170} при alloc_big = 31170 всегда
//!                              ⇒ отношение гуляло ×0.49 … ×1.59 (порог 2.5)
//! --test-threads=1 (8):        31170 → 31170 (×1.00) — 8/8 одинаково
//! фильтр `o1_` (5):            31170 → 31170 (×1.00) — 5/5 одинаково
//! ```
//!
//! То есть подвижен ЗНАМЕНАТЕЛЬ (база), и сносит его в ОБЕ стороны: сосед, освобождающий
//! память внутри окна, ЗАНИЖАЕТ базу (ложное КРАСНОЕ — так `C-089` поймал 1 падение из 10, а
//! `TD-129` — красный CI на markdown-коммите); сосед, аллоцирующий внутри окна, ЗАВЫШАЕТ базу
//! (ложное ЗЕЛЁНОЕ на коде С КЛОНОМ — направление, которое `TD-129` назвал недоказанным и
//! опасным; здесь оно закрыто сценарием, а не рассуждением).
//!
//! Развязка убирает ИСТОЧНИК, а не симптом — порог и предмет неизменны:
//!
//! 1. **Счётчик по-поточный.** Байты пишутся в `thread_local!`-ячейку аллоцирующего потока.
//!    Чужой поток структурно не может попасть в замер — это не «повезло с раскладом», а
//!    свойство конструкции. `--test-threads=1` не нужен: он лечил бы симптом и молча ломался
//!    бы при добавлении четвёртого теста.
//! 2. **Счётчик кумулятивный, а не «пик живых».** Считаются ВСЕ выданные потоку байты,
//!    освобождения не вычитаются — и это следствие пункта 1, а не отдельное усиление.
//!    «Живые байты ЭТОГО потока» — величина, которой не существует: аллокация и освобождение
//!    одного блока могут случиться на разных потоках, и по-поточная разность перестаёт быть
//!    неотрицательной. «Выданные этому потоку байты» определены точно и монотонны, поэтому
//!    замер не зависит ещё и от того, в каком порядке аллокатор чередует выдачу и возврат
//!    внутри окна. Предмет от смены величины не пострадал: против клона состояния оракул
//!    краснеет по-прежнему (мутация M2 — ×5.08 при пороге 2.5, три прогона из трёх).
//! 3. **Иммунитет предъявляется ИСПОЛНЕНИЕМ на каждом прогоне** — тест O-4: сосед делает
//!    ход (±64 MiB) ВНУТРИ окна замера по рандеву на атомиках, без sleep. Против глобального
//!    счётчика O-4 краснеет детерминированно, в обе стороны.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::SeqCst};
use std::sync::Arc;

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

thread_local! {
    /// Кумулятивно выданные байты ЭТОГО потока. `const`-инициализация + тип без `Drop` ⇒
    /// доступ к ячейке сам не аллоцирует и не рекурсирует в аллокатор.
    static TL_ALLOCATED: Cell<u64> = const { Cell::new(0) };
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            // `try_with`, а не `with`: паника внутри аллокатора недопустима. Ячейка
            // `const`-инициализирована и без `Drop`, поэтому на разрушение не регистрируется
            // и на живом потоке доступна всегда — `try_with` здесь страховка, а не штатный путь.
            // Что счётчик ДЕЙСТВИТЕЛЬНО считает, проверяется позитивным контролем в O-4.
            let _ = TL_ALLOCATED.try_with(|c| c.set(c.get().wrapping_add(l.size() as u64)));
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
    }
}
#[global_allocator]
static GA: Counting = Counting;

/// Байты, выданные ИЗМЕРЯЮЩЕМУ потоку за время `f`.
fn alloc_delta<R>(f: impl FnOnce() -> R) -> (R, u64) {
    let base = TL_ALLOCATED.with(|c| c.get());
    let r = f();
    let after = TL_ALLOCATED.with(|c| c.get());
    (r, after.wrapping_sub(base))
}

/// Объём хода соседа в тесте O-4. Порядок величины выбран так, чтобы промах был не
/// «в пределах шума», а на три десятичных порядка выше собственного замера (≈31 KB).
const NEIGHBOUR_BYTES: usize = 64 << 20;

enum NoiseMode {
    /// Сосед АЛЛОЦИРУЕТ внутри окна и держит — направление «ложное ЗЕЛЁНОЕ».
    Grow,
    /// Сосед ОСВОБОЖДАЕТ внутри окна то, что занял до окна — направление «ложное КРАСНОЕ».
    Shrink,
}

/// Сосед в ДРУГОМ потоке, чей ход гарантированно попадает ВНУТРЬ окна замера.
///
/// Рандеву на атомиках, без `sleep` и без каналов: и то и другое либо недетерминированно по
/// времени, либо аллоцирует на измеряющем потоке и портит сам замер. Поток создаётся ДО
/// открытия окна — `spawn` аллоцирует, и этой аллокации в окне быть не должно.
struct Neighbour {
    go: Arc<AtomicU8>, // 0 — ждать, 1 — ход, 2 — завершиться
    ready: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Neighbour {
    fn spawn(mode: NoiseMode, bytes: usize) -> Self {
        let go = Arc::new(AtomicU8::new(0));
        let ready = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let (g, r, d) = (go.clone(), ready.clone(), done.clone());
        let handle = std::thread::spawn(move || {
            // Для `Shrink` занять НАДО до окна: внутри окна происходит только возврат.
            let mut held: Option<Vec<u8>> = match mode {
                NoiseMode::Shrink => Some(Vec::with_capacity(bytes)),
                NoiseMode::Grow => None,
            };
            std::hint::black_box(&held);
            r.store(true, SeqCst);
            while g.load(SeqCst) == 0 {
                std::hint::spin_loop();
            }
            match mode {
                NoiseMode::Grow => held = Some(Vec::with_capacity(bytes)),
                NoiseMode::Shrink => drop(held.take()),
            }
            std::hint::black_box(&held);
            d.store(true, SeqCst);
            while g.load(SeqCst) != 2 {
                std::hint::spin_loop();
            }
            drop(held);
        });
        let n = Self {
            go,
            ready,
            done,
            handle: Some(handle),
        };
        while !n.ready.load(SeqCst) {
            std::hint::spin_loop();
        }
        n
    }

    /// Ход соседа. Зовётся ВНУТРИ измеряемого замыкания; сам ничего не аллоцирует.
    fn act(&self) {
        self.go.store(1, SeqCst);
        while !self.done.load(SeqCst) {
            std::hint::spin_loop();
        }
    }
}

impl Drop for Neighbour {
    fn drop(&mut self) {
        self.go.store(2, SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
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
///
/// Счётчик — по-поточный кумулятивный (`TD-098`/`TD-129`, см. шапку файла): соседние тесты
/// одного бинаря в замер структурно не попадают. Иммунитет предъявляется исполнением в O-4.
#[test]
fn o1_snapshot_allocation_does_not_grow_with_state() {
    let alloc_for = |levels: usize| -> (u64, usize) {
        let dir = journal_wide_book(levels, 600);
        let live = live_at_tail(dir.path());
        // Прогрев: первый вызов тянет ленивые инициализации, не относящиеся к предмету.
        let _ = live.snapshot();
        let (snap, allocated) = alloc_delta(|| live.snapshot());
        let out = serde_json::to_vec(&snap).expect("serialize").len();
        assert!(
            !snap.series.ohlcv.is_empty(),
            "O-1: снапшот пуст при {levels} уровнях — фикстура не давит"
        );
        // Setup-guard (`testing.md`, целостность гейта п.3): замер обязан ПОЙМАТЬ работу.
        // По-поточный счётчик слеп к чужим потокам — значит, если построение ответа когда-нибудь
        // уедет в другой поток, дельта схлопнется в ≈0 и оракул замолчит на любом коде. Ответ
        // весит `out` байт JSON; собрать его, не выдав на этом потоке и десятой части, нельзя.
        let floor = (out / 10) as u64;
        assert!(
            allocated >= floor,
            "O-1: замер при {levels} уровнях дал {allocated} байт при ответе {out} байт \
             (порог {floor}) — счётчик не видит работу построения. Скорее всего, `snapshot()` \
             считает не на вызывающем потоке: по-поточный счётчик такую работу не учитывает, и \
             оракул стал бы зелёным при ЛЮБОЙ реализации."
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

/// **O-4 (`TD-098`/`TD-129`).** Сам ЗАМЕР O-1 не зависит от чужих потоков.
///
/// O-1 сторожит `TD-097` (клон состояния ≈20 MiB на КАЖДОМ подключении, +404 ms константы на
/// проде — `R-029` §C). Пока счётчик был глобальным, показания O-1 двигала посторонняя
/// активность соседних тестов того же бинаря: замер 12 прогонов на неизменном предмете дал
/// базу от 19 547 до 63 170 байт при неподвижном числителе 31 170. Сторож, чьи показания
/// зависят от раскладки потоков, не сторож.
///
/// Этот тест — не «проверка на всякий случай», а ГЕЙТ на конструкцию счётчика, и он
/// детерминированный: сосед ходит внутри окна по рандеву, а не «когда-нибудь успеет».
/// Три утверждения, каждое падает против глобального счётчика:
///
/// 1. **Позитивный контроль.** Свой мегабайт учитывается. Без него оракул, ничего не
///    считающий, был бы вечно-зелёным (`harness-track.md` §3).
/// 2. **Ложное ЗЕЛЁНОЕ.** Сосед занимает +64 MiB внутри окна — глобальный счётчик приписал бы
///    их измеряемому, база O-1 раздулась бы, отношение упало бы ниже порога, и оракул промолчал
///    бы на коде С КЛОНОМ. Это ровно то направление, которое `TD-129` объявил недоказанным.
/// 3. **Ложное КРАСНОЕ.** Сосед освобождает −64 MiB внутри окна — у счётчика «пик живых байт»
///    собственный мегабайт измеряемого потока не поднялся бы над базой, замер схлопнулся бы в 0,
///    отношение улетело бы вверх. Это наблюдавшийся флак (`C-089`: 1 падение из 10; `TD-129`:
///    красный CI на markdown-коммите и зелёный перезапуск того же SHA).
#[test]
fn o4_measurement_is_immune_to_other_threads() {
    // (1) Позитивный контроль — счётчик жив и привязан к ЭТОМУ потоку.
    let ((), own) = alloc_delta(|| {
        let v = Vec::<u8>::with_capacity(1 << 20);
        std::hint::black_box(&v);
    });
    assert!(
        own >= 1 << 20,
        "O-4: собственная аллокация 1 MiB дала замер {own} байт — счётчик не считает свой поток, \
         и любой оракул на нём вечно-зелёный"
    );

    // (2) Чужой РОСТ внутри окна не виден вовсе.
    let grow = Neighbour::spawn(NoiseMode::Grow, NEIGHBOUR_BYTES);
    let ((), foreign_grow) = alloc_delta(|| grow.act());
    assert_eq!(
        foreign_grow, 0,
        "O-4: сосед занял {NEIGHBOUR_BYTES} байт в другом потоке ВНУТРИ окна, а замер показал \
         {foreign_grow} вместо 0 — счётчик общий на процесс. Направление ошибки: база O-1 \
         раздувается ⇒ отношение падает ⇒ клон состояния проезжает молча."
    );

    // (3) Чужое ОСВОБОЖДЕНИЕ внутри окна не скрадывает собственный замер.
    let shrink = Neighbour::spawn(NoiseMode::Shrink, NEIGHBOUR_BYTES);
    let ((), own_under_shrink) = alloc_delta(|| {
        shrink.act();
        let v = Vec::<u8>::with_capacity(1 << 20);
        std::hint::black_box(&v);
    });
    assert!(
        own_under_shrink >= 1 << 20,
        "O-4: сосед освободил {NEIGHBOUR_BYTES} байт в другом потоке ВНУТРИ окна, и собственный \
         мегабайт измеряемого потока просел до {own_under_shrink} — счётчик мерит процесс, а не \
         поток. Направление ошибки: база O-1 занижается ⇒ отношение растёт ⇒ ложное КРАСНОЕ на \
         здоровом коде, и роль учится перезапускать гейт вместо чтения провала."
    );
}
