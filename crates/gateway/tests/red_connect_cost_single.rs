//! M-54 (`TD-093`) — **подключение обязано читать хвост журнала ОДИН раз, а не дважды**.
//!
//! ## Замер, из-за которого milestone существует
//!
//! Подключение к `gateway-serve` на проде стоило **28.5 / 142.5 / 66.3 s**
//! (`research/reviews/R-026-M-53.md` §7). Слагаемых два:
//! backlog от суточного чекпоинта (закрыто — чекпоинт теперь каждые 15 минут) и **двойной
//! расчёт состояния**, предмет этого файла.
//!
//! `run_authorized_session` сегодня: `LiveReducer::resume()` поднимает состояние и догоняет
//! хвост, затем `snapshot_from_checkpoint()` делает **ровно то же самое второй раз**, чтобы
//! получить `Snapshot` для клиента. Причина не в небрежности: у `LiveReducer` нет способа
//! отдать своё состояние наружу — публичный API это `resume`/`pump`/`cursor`.
//!
//! ## Что меряется
//!
//! **РАБОТА (`ReadStats.events_decoded`), а не время** — сознательно, урок TD-078: оракул с
//! потолком wall-clock превращается в измеритель CI-машины. Двойной проход виден как
//! удвоенное число декодированных событий и от скорости раннера не зависит.
//!
//! COMPILE-RED: `LiveReducer::snapshot()` ещё не существует (задача 1 milestone'а).

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, ReadStats, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const BASE_MS: i64 = 1_784_116_800_000;
/// Вторые сутки — граница UTC-сессии внутри фикстуры (CVD ресетится, VWAP нет).
const D2_MS: i64 = BASE_MS + 86_400_000;

fn writer_cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 64 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "m54".to_string(),
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
        bands: vec![0.001],
        // Режим ПРОДА. Без окна `cvd_session_base` пуст, и сверка в O-2 выродилась бы
        // в сравнение пустых векторов — слепая зона, найденная критиком в `C-055` §2.
        window_ms: Some(60_000),
    }
}

/// Смешанная фикстура: книга + сделки по обе стороны границы UTC-суток.
/// Чек-лист `testing.md`: асимметричный дифф, мульти-филл в одном такте, две сессии.
fn journal_mixed(trades: i64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), writer_cfg()).expect("open_with");
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![lvl(65_000.0, 2.0), lvl(64_990.0, 3.0)],
                asks: vec![lvl(65_010.0, 1.5), lvl(65_020.0, 4.0)],
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

/// **O-1 (главный).** Снапшот для клиента берётся из УЖЕ ПОСТРОЕННОГО состояния —
/// физически без повторного чтения журнала.
///
/// **Свойство доказывает СИГНАТУРА, а не замер счётчика.** `LiveReducer::snapshot(&self)
/// -> Snapshot` не принимает ни `dir`, ни `filter` — у него просто НЕТ доступа к журналу,
/// поэтому второй проход невозможен по построению. Компилятор здесь часть теста (тот же
/// приём, что `RK-I-1`: «venue-адаптер принимает ТОЛЬКО `RiskApproved<Order>`»).
///
/// Почему не счётчиком: первая редакция этого оракула сравнивала `resume_stats.events_decoded`
/// саму с собой — тавтология ровно того класса, который M-53 и разбирал (`pump` возвращал
/// результат `frames_since`, и byte-identity сравнивала функцию с собой). Замерить полную
/// стоимость подключения на уровне `gateway` нельзя: второй проход живёт в `gateway-serve`.
/// Поэтому здесь фиксируется НЕВОЗМОЖНОСТЬ второго прохода, а фактическая экономия
/// проверяется прогоном против прода (§6 milestone'а: было 28-142 s).
///
/// Тест падает компиляцией, пока метода нет; и упадёт снова, если кто-то добавит в него
/// параметр пути — то есть вернёт возможность читать журнал.
#[test]
fn o1_snapshot_comes_from_state_not_from_journal() {
    let dir = journal_mixed(600);
    let ckpt = tempfile::tempdir().expect("ckpt");

    let (live, resume_stats): (_, ReadStats) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .expect("resume");

    assert!(
        resume_stats.events_decoded > 0,
        "O-1: резюме не прочитало НИ ОДНОГО события — фикстура не давит, тест бесполезен"
    );

    // Ключ: вызов БЕЗ пути к журналу. Если сигнатура потребует `dir`, тест не скомпилируется.
    let snapshot = live.snapshot();

    assert!(
        !snapshot.series.ohlcv.is_empty(),
        "O-1: снапшот из живого состояния пуст — «не читать журнал» удовлетворено тривиально,          отдачей пустоты. Содержательность проверяется в O-2, но пустой снапшот отсекаем здесь"
    );
    assert_eq!(
        snapshot.cursor,
        live.cursor(),
        "O-1: снапшот свёрнут не до того курсора, на котором стоит живое состояние"
    );
}

/// **O-2.** Снапшот из живого состояния поэлементно равен независимому полному реплею.
///
/// Без этого O-1 удовлетворялся бы тривиально: «не читать журнал» легко, если отдавать
/// пустой снапшот. Здесь проверяется, что дешевле — не значит неправильнее.
#[test]
fn o2_livereducer_snapshot_equals_independent_replay() {
    let dir = journal_mixed(600);
    let ckpt = tempfile::tempdir().expect("ckpt");

    let (live, _stats) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .expect("resume");
    let from_live = live.snapshot();

    let replay = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("independent replay");

    // Анти-вырождение: под окном `cvd_session_base` обязан быть НЕпуст, иначе сравнение
    // этого поля превратится в `[] == []` (слепая зона C-055 §2).
    assert!(
        !replay.series.cvd_session_base.is_empty(),
        "O-2: cvd_session_base пуст даже под окном — фикстура не вызывает эвикцию, \
         сравнение выродилось"
    );

    assert_eq!(
        from_live.cursor, replay.cursor,
        "O-2: курсор снапшота из живого состояния != независимый реплей"
    );
    assert_eq!(
        from_live.series, replay.series,
        "O-2: серии снапшота из живого состояния != независимый реплей — дешевле оказалось \
         НЕПРАВИЛЬНЕЕ, а это хуже, чем медленно"
    );
    assert_eq!(
        from_live.history_truncated, replay.history_truncated,
        "O-2: флаг усечения истории разошёлся"
    );
}

/// **O-3.** Между снапшотом и началом push нет пропущенных событий (`TD-093(а)`).
///
/// Сегодня свойство держится «по построению» (оба берут курсор из одного `live.cursor()`),
/// но не проверено ничем. Конструкция может быть переписана — тест переживёт.
/// Деградированный вход: журнал РАСТЁТ между резюме и отправкой снапшота.
#[test]
fn o3_no_gap_between_snapshot_and_push() {
    let dir = journal_mixed(400);
    let ckpt = tempfile::tempdir().expect("ckpt");

    let (mut live, _stats) =
        gateway::LiveReducer::resume(dir.path(), EpochFilter::OwnCaptureOnly, &sel(), ckpt.path())
            .expect("resume");

    // Журнал растёт ПОСЛЕ резюме — ровно то, что делает recorder в проде.
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

    let snapshot = live.snapshot();
    let push_from = live.cursor();

    assert_eq!(
        snapshot.cursor, push_from,
        "O-3: снапшот свёрнут до {:?}, а push начнётся с {:?} — события между ними не попадут \
         ни в снапшот, ни в кадры. Клиент получит дыру, которую не сможет обнаружить.",
        snapshot.cursor, push_from
    );

    // И догон обязан подобрать дозаписанное, а не потерять его.
    let (frames, _c, _st) = live
        .pump(dir.path(), EpochFilter::OwnCaptureOnly, 256)
        .expect("pump после дозаписи");
    assert!(
        !frames.is_empty(),
        "O-3: после дозаписи 5 событий push-цикл не дал ни одного кадра — приращение потеряно"
    );
}
