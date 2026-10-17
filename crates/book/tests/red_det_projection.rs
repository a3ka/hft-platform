//! RED M-51 — **DET-I-2** (sacred, architect-only): проекция, посчитанная ИНКРЕМЕНТАЛЬНО в
//! рантайме, обязана совпасть с проекцией, ПЕРЕСОБРАННОЙ реплеем с нуля — бит-в-бит.
//!
//! ## Почему это главный оракул продукта, а не «ещё один тест книги»
//!
//! `docs/DESIGN.md` §0: «продаётся не сигнал, а **доказуемость**: каждая цифра на экране
//! пользователя выводится реплеем из журнала». §14 объявляет проекции ПРОИЗВОДНЫМИ:
//! «Пересборка = replay от чекпоинта (или с нуля) тем же детерминированным кодом. Проекция не
//! бэкапится как истина». `PL-I-1` (§22) формулирует то же как инвариант — и стоит там со
//! статусом **PENDING, без единого оракула**.
//!
//! Практическое следствие расхождения: HOT-проекция живёт в RAM и считается инкрементально по
//! мере прихода событий; после рестарта/пересборки она собирается реплеем. Если два пути
//! расходятся, то цифра, показанная пользователю до рестарта, **не воспроизводится** после —
//! то есть ровно то, что продукт продаёт, не выполняется. Аудит
//! (`research/measurements/td-007-determinism-coverage.md` §5) фиксирует: этот параллелизм
//! реально проверен ТОЛЬКО для gateway-агрегатов (`red_gateway_live_eq_replay.rs`), а для
//! `crates/book` — реконструированной книги, на которой стоят все остальные проекции, —
//! **не проверен вовсе**.
//!
//! ## Контракт (две части)
//!
//! **DET-I-2.** Для любого окна журнала: состояние проекции, полученное применением событий по
//! мере их поступления (live), == состояние, полученное применением тех же событий, прочитанных
//! из журнала (replay), == состояние, полученное как «префикс + догон» (checkpoint + tail).
//! Равенство — по КАНОНИЧЕСКОМУ представлению состояния, а не «на глаз по best bid».
//!
//! Это дословно `JR-I-4` из `docs/fa/journal.md:114` («`snapshot + tail == full replay`»),
//! объявленный вместе с именем теста `test_snapshot_equals_full_replay`, которого в крейте
//! никогда не существовало.
//!
//! **Недостающий примитив (контрактная часть, реализует dev).** Сегодня канонического
//! представления НЕ СУЩЕСТВУЕТ: `Books` держит `map: HashMap<(Venue, String), OrderBook>`
//! (`crates/book/src/lib.rs:303`) и отдаёт наружу ТОЛЬКО `get(venue, symbol)` — то есть
//! перечислить проекцию можно лишь обходом хэш-карты, а он запрещён `CLAUDE.md` и
//! недетерминирован. Требуется:
//!
//! ```text
//! impl Books {
//!     /// Детерминированный обход проекции: инструменты в возрастающем порядке (venue, symbol).
//!     pub fn iter_sorted(&self) -> Vec<((Venue, &str), &OrderBook)>;
//! }
//! ```
//! Без него «проекция воспроизводима» — непроверяемое утверждение по построению: нет способа
//! снять состояние целиком, не положившись на порядок `HashMap`.
//!
//! ## Анти-плацебо
//!
//! `det_21` требует, чтобы канон РАЗЛИЧАЛ состояния: проекция, собранная без одного события,
//! обязана отличаться. Реализация `iter_sorted`, возвращающая пустой вектор (или канон,
//! теряющий поля), провалит его, хотя все равенства выше прошла бы тривиально.

use book::Books;
use contracts::{to_fixed, EventKind, Level, MdPayload, Side, Venue};
use journal::{EpochFilter, Journal, WriterConfig};

/// Минимальный самоуборочный временный каталог. `tempfile` не значится в
/// `[dev-dependencies]` крейта `book`, а правка `Cargo.toml` — не зона architect'а
/// (`.claude/rules/scope-guard.md`). Пятнадцать строк дешевле, чем блокировать RED-набор на
/// чужой правке билд-конфига.
struct TmpDir(std::path::PathBuf);

impl TmpDir {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let p = std::env::temp_dir().join(format!(
            "hft-det-proj-{}-{}-{}",
            std::process::id(),
            nanos,
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&p).expect("create temp dir");
        TmpDir(p)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cfg(max_segment_bytes: u64) -> WriterConfig {
    WriterConfig {
        max_segment_bytes,
        min_free_bytes: 0,
        source: contracts::DataSource::OwnCapture,
        provenance: "det-projection".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// КАНОНИЧЕСКОЕ представление проекции: детерминированный обход + полное состояние каждой
/// книги (все поля `OrderBook` сериализуемы с M-38b — чекпоинт-редьюсер на это уже опирается).
/// Сравнение строк — это и есть «бит-в-бит» для проекции.
fn canon(books: &Books) -> String {
    let parts: Vec<serde_json::Value> = books
        .iter_sorted()
        .into_iter()
        .map(|((venue, symbol), b)| {
            serde_json::json!({
                "venue": format!("{venue:?}"),
                "symbol": symbol,
                "book": serde_json::to_value(b).expect("OrderBook сериализуем (M-38b)"),
            })
        })
        .collect();
    serde_json::to_string(&parts).expect("canon")
}

fn snapshot(venue: Venue, symbol: &str, bids: &[(f64, f64)], asks: &[(f64, f64)]) -> EventKind {
    EventKind::md(
        venue,
        symbol,
        MdPayload::L2Snapshot {
            bids: bids.iter().map(|&(p, s)| lvl(p, s)).collect(),
            asks: asks.iter().map(|&(p, s)| lvl(p, s)).collect(),
            ts_exch_ms: 1_752_000_000_000,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn delta(
    venue: Venue,
    symbol: &str,
    bids: &[(f64, f64)],
    asks: &[(f64, f64)],
    first_update_id: u64,
    final_update_id: u64,
    prev_final_update_id: Option<u64>,
) -> EventKind {
    EventKind::md(
        venue,
        symbol,
        MdPayload::L2Delta {
            bids: bids.iter().map(|&(p, s)| lvl(p, s)).collect(),
            asks: asks.iter().map(|&(p, s)| lvl(p, s)).collect(),
            first_update_id,
            final_update_id,
            prev_final_update_id,
            ts_exch_ms: 1_752_000_000_000,
        },
    )
}

fn trade(venue: Venue, symbol: &str, price: f64) -> EventKind {
    EventKind::md(
        venue,
        symbol,
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(0.5),
            side: Side::Buy,
            ts_exch_ms: 1_752_000_000_000,
        },
    )
}

/// Поток событий с ВСТРОЕННЫМИ деградациями (чек-лист `.claude/rules/testing.md`).
///
/// - **АСИММЕТРИЯ:** дельты, трогающие ТОЛЬКО bid-сторону (`asks: &[]`) — штатная ситуация,
///   на которой уже ломался M-08/TD-016 (симметричная фикстура скрыла дефект).
/// - **ОТСУТСТВИЕ:** то, чего в дельте НЕТ, не имеет права быть стёртым; ноль в дельте —
///   наоборот, ЕСТЬ указание удалить уровень.
/// - **МНОЖЕСТВЕННОСТЬ:** три инструмента на двух площадках; две дельты подряд без снапшота.
/// - **ГРАНИЦЫ:** разрыв чейна (`Gap` → книга `stale`) и последующий ресинк снапшотом —
///   деградированное состояние обязано реплеиться ТАК ЖЕ, а не «починиться» при пересборке.
fn stream_of_events() -> Vec<EventKind> {
    let b = Venue::Binance;
    let h = Venue::Hyperliquid;
    vec![
        // ── базовые снапшоты трёх инструментов ─────────────────────────────────────────
        snapshot(
            b,
            "BTCUSDT",
            &[(100.0, 2.0), (99.0, 5.0)],
            &[(101.0, 3.0), (102.0, 1.0)],
        ),
        snapshot(b, "ETHUSDT", &[(50.0, 8.0), (49.5, 2.0)], &[(50.5, 4.0)]),
        snapshot(h, "BTC", &[(100.5, 1.0)], &[(101.5, 1.0)]),
        // Trade — книгой игнорируется; обязан игнорироваться ОДИНАКОВО в обоих путях.
        trade(b, "BTCUSDT", 100.5),
        // ── АСИММЕТРИЯ: дельта только по bid-стороне; про ask не сказано НИЧЕГО ────────
        delta(b, "BTCUSDT", &[(100.0, 7.0)], &[], 1, 1, None),
        // ── ОТСУТСТВИЕ vs НОЛЬ: ноль удаляет уровень, умолчание — не трогает ───────────
        delta(b, "BTCUSDT", &[(99.0, 0.0)], &[], 2, 2, None),
        // ── МНОЖЕСТВЕННОСТЬ: две дельты подряд, второй инструмент ─────────────────────
        delta(b, "ETHUSDT", &[(50.0, 9.0)], &[], 1, 1, None),
        delta(b, "ETHUSDT", &[], &[(50.5, 6.0)], 2, 2, None),
        // ── ГРАНИЦА: разрыв чейна → Gap → книга stale (деградированное состояние) ──────
        delta(b, "ETHUSDT", &[(49.5, 1.0)], &[], 99, 99, None),
        // stale-книга обязана ОТВЕРГАТЬ последующие дельты одинаково в обоих путях
        delta(b, "ETHUSDT", &[(49.0, 3.0)], &[], 100, 100, None),
        // ── ресинк снапшотом ──────────────────────────────────────────────────────────
        snapshot(b, "ETHUSDT", &[(50.2, 3.0)], &[(50.7, 2.0)]),
        // хвост: ещё асимметрия по третьему инструменту
        delta(h, "BTC", &[], &[(101.5, 4.0)], 1, 1, None),
        trade(h, "BTC", 101.0),
    ]
    .into_iter()
    // ── НАПОЛНЕНИЕ: достаточно событий, чтобы журнал реально разошёлся на много сегментов
    //    (иначе не проверяются ни сшивка, ни компакция: закрытых сегментов не хватает).
    //    Заодно МНОЖЕСТВЕННОСТЬ — каждый инструмент обновляется многократно.
    .chain((0..48u64).map(move |i| {
        let p = 100.0 + (i % 17) as f64 * 0.5;
        match i % 3 {
            0 => snapshot(b, "BTCUSDT", &[(p, 1.0 + i as f64)], &[(p + 1.0, 2.0)]),
            1 => snapshot(
                b,
                "ETHUSDT",
                &[(p / 2.0, 3.0)],
                &[(p / 2.0 + 0.5, 1.0 + i as f64)],
            ),
            _ => snapshot(h, "BTC", &[(p, 2.0)], &[(p + 1.0, 1.0 + i as f64)]),
        }
    }))
    .chain(std::iter::once(trade(b, "BTCUSDT", 100.25)))
    // ── ПОСЛЕДНЕЕ событие ОБЯЗАНО двигать книгу ──────────────────────────────────────
    //    Первая редакция фикстуры заканчивалась `trade`, который книга игнорирует, — и
    //    анти-плацебо `det_21` («без последнего события канон обязан отличаться») был
    //    ЗЕЛЁН ПО НЕВЕРНОЙ ПРИЧИНЕ бы, окажись он менее строгим. Здесь последним идёт
    //    дельта, рвущая чейн: она переводит книгу в `stale` — деградированное состояние,
    //    которое обязано пережить реплей, а не «почититься» при пересборке.
    .chain(std::iter::once(delta(
        h,
        "BTC",
        &[(99.0, 1.0)],
        &[],
        777,
        777,
        None,
    )))
    .collect()
}

/// Записать поток в журнал; вернуть (каталог, seq'ы, live-проекция, посчитанная ПО ХОДУ).
///
/// Live-путь принципиально не касается диска: события применяются в проекцию в тот же момент,
/// когда отдаются журналу — как это делает рантайм.
fn build(max_segment_bytes: u64) -> (TmpDir, Vec<u64>, Books) {
    let dir = TmpDir::new();
    let mut seqs = Vec::new();
    let mut live = Books::new();
    {
        let mut j = Journal::open_with(dir.path(), cfg(max_segment_bytes)).expect("open");
        for kind in stream_of_events() {
            let ev = j.append(kind).expect("append");
            if let EventKind::Md(md) = &ev.kind {
                live.apply(md);
            }
            seqs.push(ev.seq);
        }
        j.flush().expect("flush");
    }
    (dir, seqs, live)
}

/// Пересобрать проекцию РЕПЛЕЕМ (опционально — только хвост после `after_seq`, поверх `base`).
fn replay_into(dir: &std::path::Path, base: Books, after_seq: Option<u64>) -> Books {
    let mut books = base;
    for e in journal::stream_from(dir, EpochFilter::OwnCaptureOnly, after_seq).expect("stream") {
        let ev = e.expect("event");
        if let EventKind::Md(md) = &ev.kind {
            books.apply(md);
        }
    }
    books
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_18 — ЯДРО: инкрементальный рантайм == пересборка реплеем с нуля.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_18_live_projection_equals_full_replay() {
    // Мелкий сегмент → окно РЕАЛЬНО пересекает границы сегментов (иначе «сшивка» не проверена).
    let (dir, _seqs, live) = build(512);
    let n_seg = journal::list_segments(dir.path()).expect("segments").len();
    assert!(
        n_seg >= 2,
        "фикстура обязана дать >=2 сегмента, а дала {n_seg}"
    );

    let rebuilt = replay_into(dir.path(), Books::new(), None);

    assert_eq!(
        canon(&rebuilt),
        canon(&live),
        "DET-I-2/PL-I-1: проекция, пересобранная реплеем, НЕ совпала с посчитанной \
         инкрементально. Продуктовое следствие: цифра на экране не воспроизводится реплеем — \
         то, что продукт продаёт (DESIGN §0), не выполняется"
    );

    // Проекция обязана быть непустой и содержать ВСЕ три инструмента — иначе равенство выше
    // тривиально (две пустые проекции всегда равны).
    let n = live.iter_sorted().len();
    assert_eq!(
        n, 3,
        "фикстура/примитив: проекция обязана содержать 3 инструмента, а содержит {n} — \
         равенство канонов ничего не доказывает на пустой проекции"
    );
    // И деградированное состояние обязано ДОЙТИ до проекции, а не «починиться» по дороге:
    // ETHUSDT прошёл через Gap и ресинк снапшотом.
    assert!(
        canon(&live).contains("ETHUSDT"),
        "фикстура: инструмент, переживший Gap и ресинк, обязан присутствовать в каноне"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_19 — checkpoint-класс: «префикс + догон» == полный реплей, ЧЕРЕЗ границу компакции.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_19_prefix_plus_tail_equals_full_replay_across_compaction() {
    // Это дословно JR-I-4 (`docs/fa/journal.md:114`, тест `test_snapshot_equals_full_replay`,
    // которого не существует) и механика §14 «пересборка от последнего чекпоинта».
    //
    // Догон идёт через `stream_from(after_seq)` — путь с СЕГМЕНТНЫМ ПРОПУСКОМ (M-38b/GW-I-11).
    // Между префиксом и догоном мы СЖИМАЕМ закрытые сегменты: реальный прод именно так и
    // устроен (компакция догоняет хвост, `.zst` и сырые сосуществуют — замер 144 + 10).
    // Off-by-one на границе пропуска или потеря события на границе формата проявятся здесь как
    // расхождение проекции, а не как «странный лог».
    let (dir, seqs, live) = build(512);
    let full = replay_into(dir.path(), Books::new(), None);

    let k = seqs.len() / 2;
    let cut = seqs[k];

    // Фаза 1: префикс до `cut` включительно (журнал ещё сырой).
    let mut prefix = Books::new();
    for e in journal::stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream") {
        let ev = e.expect("event");
        if ev.seq > cut {
            break;
        }
        if let EventKind::Md(md) = &ev.kind {
            prefix.apply(md);
        }
    }
    assert_ne!(
        canon(&prefix),
        canon(&full),
        "фикстура: префикс обязан ОТЛИЧАТЬСЯ от полного состояния (иначе догон нечего проверять)"
    );

    // Фаза 2: компакция — форма журнала меняется под ногами догоняющего.
    let reports = journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact");
    assert!(
        !reports.is_empty(),
        "фикстура: компакция обязана реально сжать хотя бы один сегмент"
    );
    let files = {
        let mut v: Vec<String> = std::fs::read_dir(dir.path())
            .expect("read_dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        v.sort();
        v
    };
    assert!(
        files.iter().any(|f| f.ends_with(".jrnl.zst"))
            && files.iter().any(|f| f.ends_with(".jrnl")),
        "фикстура: обязана получиться смешанная форма raw + .zst (форма прода): {files:?}"
    );

    // Фаза 3: догон поверх префикса.
    let caught_up = replay_into(dir.path(), prefix, Some(cut));

    assert_eq!(
        canon(&caught_up),
        canon(&full),
        "DET-I-2/JR-I-4: «префикс + догон» разошёлся с полным реплеем через границу компакции. \
         Быстрый старт проекции от чекпоинта (DESIGN §14, M-48) даёт НЕ ТО состояние, что \
         честная пересборка — расхождение уедет пользователю молча"
    );
    assert_eq!(
        canon(&caught_up),
        canon(&live),
        "DET-I-2: «префикс + догон» разошёлся с инкрементальным рантаймом"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_20 — реплей проекции повторяем; сжатие сегмента её не меняет.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_20_projection_replay_is_repeatable_and_format_independent() {
    let (dir, _seqs, _live) = build(512);

    let a = canon(&replay_into(dir.path(), Books::new(), None));
    let b = canon(&replay_into(dir.path(), Books::new(), None));
    assert_eq!(
        a, b,
        "DET-I-2: два реплея проекции подряд дали разное состояние"
    );

    journal::compact_closed_segments(dir.path(), 1, journal::DEFAULT_COMPACT_LEVEL)
        .expect("compact");
    let c = canon(&replay_into(dir.path(), Books::new(), None));
    assert_eq!(
        a, c,
        "DET-I-2: проекция, пересобранная из СЖАТЫХ сегментов, отличается от собранной из \
         сырых — COLD-архив (DESIGN §14) перестал быть эквивалентен горячим данным"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// det_21 — АНТИ-ПЛАЦЕБО: канон обязан РАЗЛИЧАТЬ состояния.
// ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn det_21_canonical_state_discriminates() {
    let (dir, seqs, _live) = build(512);
    let full = canon(&replay_into(dir.path(), Books::new(), None));

    // (а) Проекция без ПОСЛЕДНЕГО события обязана отличаться.
    let mut minus_last = Books::new();
    let last = *seqs.last().expect("seqs непуст");
    for e in journal::stream(dir.path(), EpochFilter::OwnCaptureOnly).expect("stream") {
        let ev = e.expect("event");
        if ev.seq == last {
            continue;
        }
        if let EventKind::Md(md) = &ev.kind {
            minus_last.apply(md);
        }
    }
    assert_ne!(
        canon(&minus_last),
        full,
        "АНТИ-ПЛАЦЕБО: проекция без последнего события дала ТОТ ЖЕ канон — канон слеп \
         (`iter_sorted` пуст / состояние книги теряется при сериализации), и все равенства \
         выше ничего не доказывают"
    );

    // (б) Пустая проекция обязана отличаться от заполненной.
    assert_ne!(
        canon(&Books::new()),
        full,
        "АНТИ-ПЛАЦЕБО: пустая проекция дала тот же канон, что заполненная"
    );

    // (в) Обход обязан быть ОТСОРТИРОВАННЫМ, а не «каким-то стабильным»: порядок — свойство
    //     данных. Иначе два процесса (разный хэш-сид) дадут разные каноны на равном состоянии.
    let books = replay_into(dir.path(), Books::new(), None);
    let keys: Vec<(String, String)> = books
        .iter_sorted()
        .into_iter()
        .map(|((v, s), _)| (format!("{v:?}"), s.to_string()))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(
        keys, sorted,
        "DET-I-2: `Books::iter_sorted` вернул инструменты НЕ в возрастающем порядке — обход \
         проекции определяется хэш-картой, а не данными"
    );
}
