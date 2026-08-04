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

// ═══════════════════ TD-083 rev2: замена ТАВТОЛОГИЧНЫХ проверок ═══════════════════
//
// Найдено architect'ом 2026-08-03 при разборе TD-083 (P0, прод-read-path был мёртв).
// Оба теста выше ЗЕЛЁНЫЕ и при этом не давят ни на что:
//
// 1. `pumped_frames_identical_to_frames_since` сравнивает кадры `pump` с `frames_since`.
//    Но `pump` (`gateway/src/lib.rs:2941`) БУКВАЛЬНО ВОЗВРАЩАЕТ результат `frames_since` —
//    комментарий в коде признаётся: «Используем frames_since для byte-identity с эталоном…
//    Это компромисс». ⇒ функция сравнивается сама с собой, зелено всегда.
// 2. `pump_at_tail_is_bounded` проверяет `events_decoded <= 64`, но `stats` берутся из
//    `read_stats_from_stream(&stream)` — потока `stream_from`, то есть ТОЛЬКО второй фазы.
//    Работа, потраченная внутри `frames_since` (чтение журнала с ГОЛОВЫ), в `stats` не
//    попадает. ⇒ измеряется половина, подтверждается ограниченность, которой нет.
//
// Это ровно класс `C-055` §2: «сравнение технически присутствует, но математически
// сравнивает пустое с пустым». Ниже — проверки, которые ПАДАЮТ против такой конструкции.

/// **TD-083 O-A.** Byte-identity против НЕЗАВИСИМОГО эталона.
///
/// Эталон — не `frames_since` (тогда сравнение тавтологично), а **полный реплей**:
/// `snapshot(START) + все кадры pump ≡ snapshot(LATEST)`. Это свойство нельзя удовлетворить,
/// вернув чужой результат: оно связывает кадры с независимо посчитанным состоянием журнала.
#[test]
fn td083_pumped_frames_fold_into_full_replay_snapshot() {
    let dir = journal_upto(N);
    let s = sel();

    let ckpt = tempfile::tempdir().expect("ckpt");
    let (mut live, _resume_stats) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .expect("resume");

    // Стартовое состояние — независимый реплей до курсора, с которого начал LiveReducer.
    let mut folded = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        gateway::Cursor::START,
    )
    .expect("snapshot(START)");

    // Цепочка тиков — ровно то, что крутится в соединении.
    for _ in 0..8 {
        let (frames, _cursor, _stats) = live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 100)
            .expect("pump");
        if frames.is_empty() {
            break;
        }
        for f in &frames {
            folded.apply(f);
        }
    }

    let full = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &s,
        gateway::Cursor::LATEST,
    )
    .expect("snapshot(LATEST)");

    assert_eq!(
        folded.cursor, full.cursor,
        "TD-083 O-A: свёртка кадров pump не дошла до конца журнала"
    );
    assert_eq!(
        folded.series, full.series,
        "TD-083 O-A: snapshot(START) + кадры pump ≠ snapshot(LATEST) — live-путь расходится \
         с независимым полным реплеем. Сравнение с `frames_since` этого НЕ поймало бы: pump \
         возвращает её же результат"
    );
}

/// **TD-083 O-B.** Стоимость тика не растёт с историей.
///
/// ## Почему мера двойная (переписано 2026-08-03, TD-094 — тест флакал)
///
/// Первая редакция меряла ТОЛЬКО время и падала примерно раз в пять прогонов на загруженной
/// машине: тик по приращению в 3 события занимает доли миллисекунды, и шум планировщика в
/// `big` пробивал порог ×4 при полностью здоровой реализации. Флакающий sacred-оракул делает
/// `main` случайно красным и блокирует деплой — цена ложной тревоги здесь выше, чем кажется.
/// Это ровно урок TD-078, который эта же docstring и цитировала, — нарушенный её автором.
///
/// Теперь свойство проверяется двумя независимыми мерами:
///
/// 1. **Работа (основная, детерминированная).** `ReadStats.events_decoded` измеряемого тика
///    обязан быть порядка ПРИРАЩЕНИЯ (3 события), а не накопленной истории (8 000). Эта
///    проверка не зависит от скорости машины вообще. Возражение первой редакции — «stats
///    самого `pump` доверять нельзя, они и оказались половинными» — снято: честность `stats`
///    теперь охраняет `pumped_frames_identical_to_frames_since` (байт-идентичность кадров
///    live-пути и `frames_since`), а `red_push_seek_bounded.rs` независимо меряет
///    `segments_opened`. Врать в одиночку `events_decoded` больше не может.
/// 2. **Время (подтверждающая, устойчивая).** Берётся МИНИМУМ из `REPEATS` прогонов:
///    планировщик способен только замедлить, поэтому минимум — нижняя оценка честной
///    стоимости, а шум в неё не попадает. Порог ×4 против структурного эффекта ×8.
#[test]
fn td083_tick_wallclock_does_not_grow_with_history() {
    /// Сколько раз повторить замер и взять минимум (устойчивость к шуму планировщика).
    const REPEATS: usize = 5;
    /// Приращение между тиками — столько recorder успевает записать за период push.
    const INCREMENT: u64 = 3;

    // ── мера 1: РАБОТА (детерминированная) ──
    //
    // ПЕРЕПИСАНО в M-57 (`C-059` §2.2). Прежняя редакция ассертила `st.events_decoded` — тот
    // самый счётчик, слепоту которого M-57 и доказывает: он считает ВЫДАННЫЕ события, а не
    // прочитанные, и потому равен приращению даже когда сегмент пересканирован целиком.
    // То есть «детерминированная» мера была ВАКУУМНОЙ, и единственными зубами оракула
    // оставалась мера 2 (wall-clock) — а она меряет планировщик, а не инвариант, и флакала
    // (engine-dev: падение 1 раз из 3 прогонов подряд). Оракул либо проходил впустую, либо
    // краснел от нагрузки соседних тестов.
    let tick_work = |events: u64| -> u64 {
        let dir = journal_upto(events);
        let s = sel();
        let ckpt = tempfile::tempdir().expect("ckpt");
        let (mut live, _resume_stats) =
            gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
                .expect("resume");
        while let Ok((frames, _c, _st)) = live.pump(dir.path(), EpochFilter::OwnCaptureOnly, 10_000)
        {
            if frames.is_empty() {
                break;
            }
        }
        {
            let mut j = Journal::open_with(dir.path(), cfg()).expect("reopen");
            for i in events..events + INCREMENT {
                j.append(trade(i)).expect("append");
            }
            j.flush().expect("flush");
        }
        let (_f, _c, st) = live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
            .expect("pump измеряемый");
        // `events_scanned` — честный счётчик M-57 (задача 1): сколько событий РЕАЛЬНО
        // прочитано и декодировано, включая отброшенные фильтром `after_seq`.
        st.events_scanned
    };

    let work_big = tick_work(8_000);
    assert!(
        work_big <= INCREMENT * 4,
        "TD-083 O-B (работа): тик на журнале в 8 000 событий ПРОЧИТАЛ {work_big} событий при \
         приращении всего в {INCREMENT}. Значит журнал читается С ГОЛОВЫ, а не с курсора. На \
         проде (139M событий до курсора) это ≈12 минут на тик при периоде 250 ms: live-push \
         молчит, accept-loop мёртв. Мера детерминированная: `events_scanned` не зависит ни от \
         планировщика, ни от нагрузки соседних тестов."
    );

    // ── мера 2: ВРЕМЯ (подтверждающая, минимум из REPEATS) ──
    let tick_cost = |events: u64| -> std::time::Duration {
        let dir = journal_upto(events);
        let s = sel();
        let ckpt = tempfile::tempdir().expect("ckpt");
        let (mut live, _resume_stats) =
            gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &s, ckpt.path())
                .expect("resume");
        // Догнать хвост, чтобы следующий тик был «живым» — по приращению.
        while let Ok((frames, _c, _st)) = live.pump(dir.path(), EpochFilter::OwnCaptureOnly, 10_000)
        {
            if frames.is_empty() {
                break;
            }
        }
        // Дописать РОВНО 3 события — столько recorder успевает между тиками.
        {
            let mut j = Journal::open_with(dir.path(), cfg()).expect("reopen");
            for i in events..events + INCREMENT {
                j.append(trade(i)).expect("append");
            }
            j.flush().expect("flush");
        }
        let t0 = std::time::Instant::now();
        let _ = live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
            .expect("pump измеряемый");
        t0.elapsed()
    };
    let best_of = |events: u64| -> std::time::Duration {
        (0..REPEATS)
            .map(|_| tick_cost(events))
            .min()
            .expect("REPEATS > 0")
    };

    let small = best_of(1_000);
    let big = best_of(8_000);

    let ratio = big.as_secs_f64() / small.as_secs_f64().max(1e-9);
    eprintln!(
        "TD-083 O-B (время, диагностика): журнал ×8 → тик ×{ratio:.1} ({small:?} → {big:?}), \
         минимум из {REPEATS} прогонов, приращение {INCREMENT}"
    );
    // АССЕРТ ВОССТАНОВЛЕН (2026-08-04). Первая редакция этой правки заменила его одной
    // печатью — то есть СНЯЛА покрытие с sacred-оракула ЧУЖОГО milestone'а (M-53) внутри
    // цепочки, где ослабление гейта выгодно автору правки: мой milestone от этого зеленел.
    // Наблюдение «wall-clock меряет окружение» остаётся верным, но снятие ассерта — решение
    // для независимого круга, а не для того, кому оно удобно. Мера 1 (`events_scanned`)
    // ДОБАВЛЕНА, а не заменяет эту; наблюдавшийся флак (1 падение из 3 прогонов) фиксируется
    // долгом, а не удалением проверки.
    assert!(
        ratio < 4.0,
        "TD-083 O-B (время): тик на журнале ×8 длиннее занял в {ratio:.1} раза больше ({small:?} \
         → {big:?}) на ОДИНАКОВОМ приращении в {INCREMENT} события, по МИНИМУМУ из {REPEATS} \
         прогонов — то есть это не шум планировщика. Цена тика зависит от НАКОПЛЕННОЙ ИСТОРИИ, \
         а не от приращения: журнал читается с головы."
    );
}
