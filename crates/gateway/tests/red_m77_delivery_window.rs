//! RED `VB-I-2` (sacred, architect-only) — **`M-77` задача 2bis: ОКНО «СОСТОЯНИЕ ВПЕРЕДИ
//! КУРСОРА ДОСТАВКИ».**
//!
//! Милестоун `milestones/M-77-frame-book-continuity.md` §6bis (контракт развязки Б).
//! Инвариант — `VB-I-2` («live == replay», `docs/fa/viz-backend.md:199`).
//!
//! # Что здесь судится и почему это ОТДЕЛЬНЫЙ файл
//!
//! `red_m77_frame_book_continuity.rs` судит установившийся режим, где две позиции
//! `LiveReducer` совпадают. Развязка Б (выбрана `C-211`) берёт книгу-зависимые серии из
//! ЖИВОГО редьюсера `self.full`, а он по построению может УЙТИ ВПЕРЁД закладки доставки:
//!
//! > `cursor` — до какого seq кадр УЖЕ ОТДАН потребителю; `full_applied_seq` — до какого seq
//! > СОСТОЯНИЕ уже свёрнуто. Инвариант: `full_applied_seq >= cursor.upto_seq`; расхождение —
//! > окно «батч свёрнут, но отвергнут пределом и потому не отдан».
//! > (`crates/gateway/src/lib.rs:3659-3664`, доки полей — цитата, не пересказ.)
//!
//! `C-211` назвал это окно ЕДИНСТВЕННОЙ реальной ценой развязки Б и потребовал RED на него
//! как условие её выбора. Опасность конкретна и предъявима: реализация, которая берёт
//! книго-зависимые серии как ПОБОЧНЫЙ ЭФФЕКТ `full.apply(event)`, на повторной попытке не
//! получит НИЧЕГО — повторный проход применяет к `self.full` только события
//! `seq > full_applied_seq` (`:4020-4026`), то есть для уже свёрнутого батча не зовёт
//! `apply` вовсе. Кадр повторной попытки уедет БЕЗ глубины, и потеря будет молчаливой:
//! ровно класс `R-140`, поднятый на уровень выше.
//!
//! Отсюда требование контракта, которое эти тесты пиннят: **книго-зависимая серия кадра
//! есть функция ДИАПАЗОНА кадра, а не текущей позиции состояния.**
//!
//! # Файл отдельный, потому что предел ПРОЦЕССНО-ГЛОБАЛЕН
//!
//! `gateway::set_effective_max_response_bytes` пишет в статик процесса
//! (`crates/gateway/src/lib.rs:138`, `:152`). Тесты, его двигающие, обязаны идти под общим
//! мьютексом, иначе соседний тест того же бинаря получит чужой предел. Держать их в файле
//! `red_m77_frame_book_continuity.rs` значило бы навязать `serial()` и его тестам —
//! прецедент и причина те же, что у `red_egress_cap_paths.rs:130` и `red_heatmap.rs:85`.
//!
//! # Мера — на границе ПОТРЕБИТЕЛЯ (`Р-1`)
//!
//! Судится состояние, собранное КЛИЕНТОМ (`snapshot(C) + Σ delivered frames`), против
//! НЕЗАВИСИМОГО эталона — полного реплея. Ни один сервер-внутренний счётчик мерой не служит.
//!
//! # Сравнивается ВЕСЬ `SeriesBundle`, а не депт-серия (`Р-3`)
//!
//! Книго-зависимых серий в кадре НЕ ОДНА: `depth_series`, `heatmap` и `cob` строятся из
//! `self.book` (`:1150`, `:1156`, `:1177`, `:1183`). Оракул на выписанный перечень пропустил
//! бы серию, добавленную позже, — «опасна ровно та группа, которая НЕ ВЫПИСАНА»
//! (`oracle-blindness-class-2026-08-28.md` §5, `Р-3`). Сравнение целого `SeriesBundle`
//! накрывает группу ПО КОНСТРУКЦИИ и не требует поддержки перечня.

use std::sync::{Mutex, MutexGuard, OnceLock};

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector, Snapshot};
use journal::{EpochFilter, Journal, WriterConfig};

const MID: f64 = 65_000.0;
const T: i64 = 1_752_000_000_000;
const PROD_BAND: f64 = 0.001;
const DEEP_BAND: f64 = 0.02;
/// Прод-настройка push-цикла (`crates/gateway-serve/src/lib.rs:1119-1120`).
const PUSH_MAX_EVENTS: usize = 256;
/// Предел, заведомо меньший кадра этой фикстуры. Значение — то же, что у соседних
/// cap-оракулов (`red_egress_cap_paths.rs`), чтобы пороги предмета не расходились.
const TINY: usize = 20_000;
/// Отказов подряд под стоящим пределом. Три — минимум, отличающий «медленно» от «никогда».
const REFUSALS: usize = 3;
/// `W3`: размер батча и предел подобраны так, чтобы отказ настигал ДРОБЛЁНЫЙ тик. Предел
/// здесь СВОЙ и меньше `TINY` — иначе мелкий батч в него укладывается, отказа нет, и
/// сценарий ролловера недостижим (setup-guard `W3` это и предъявил на первой редакции).
const ROLLOVER_BATCH: usize = 32;
const ROLLOVER_CAP: usize = 3_000;

/// Предел объёма — ПРОЦЕССНОЕ значение; тесты этого файла идут строго по одному.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn setup_failed(what: &str) -> ! {
    panic!("SETUP НЕ СОСТОЯЛСЯ: {what} — тест НЕ судил предмет, зелёное было бы вакуумом");
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 64 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "M-77 delivery window".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn lvl(price: f64, size: f64) -> Level {
    Level {
        price: to_fixed(price),
        size: to_fixed(size),
    }
}

/// Селектор ПРОД-ФОРМЫ (`Р-2`): замер `docker-compose.yml:135,136,142,154` на `origin/main`.
fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![PROD_BAND, DEEP_BAND],
        window_ms: Some(60_000),
        depth_cadence_ms: Some(1_000),
    }
}

/// Якорь: ближние уровни на 0.05 % — ВНУТРИ прод-полосы, но не на её границе
/// (`testing.md` §«Дегенерированный вход» п.4).
fn anchor(ts: i64, reach: f64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: vec![lvl(MID * (1.0 - reach), 3.0), lvl(MID * 0.9995, 4.0)],
            asks: vec![lvl(MID * (1.0 + reach), 3.0), lvl(MID * 1.0005, 4.0)],
            ts_exch_ms: ts,
        },
    )
}

fn delta(ts: i64, seq: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids: vec![lvl(MID * 0.995, 7.0)],
            asks: vec![lvl(MID * 1.005, 7.0)],
            ts_exch_ms: ts,
            first_update_id: seq,
            final_update_id: seq,
            prev_final_update_id: Some(seq.saturating_sub(1)),
        },
    )
}

/// Дельта, населяющая ОКНО heatmap (`±0.1 %` от mid, прод-дефолт `docker-compose.yml:137`)
/// множеством уровней. Нужна, чтобы кадр вырос до размера, не влезающего в `TINY`: без
/// неё эталон весит 16 КБ при пределе 20 КБ, и сценарий «предел стоит» недостижим
/// (первая редакция этого файла на том и остановилась — setup-guard сработал, а не
/// пропустил вакуумный зелёный).
fn wide_delta(ts: i64, seq: u64, phase: u64) -> EventKind {
    let step = 0.000_02;
    let bids: Vec<Level> = (1..=30)
        .map(|j| {
            lvl(
                MID * (1.0 - j as f64 * step),
                1.0 + ((j as u64 + phase) % 9) as f64,
            )
        })
        .collect();
    let asks: Vec<Level> = (1..=30)
        .map(|j| {
            lvl(
                MID * (1.0 + j as f64 * step),
                1.0 + ((j as u64 + phase) % 7) as f64,
            )
        })
        .collect();
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Delta {
            bids,
            asks,
            ts_exch_ms: ts,
            first_update_id: seq,
            final_update_id: seq,
            prev_final_update_id: Some(seq.saturating_sub(1)),
        },
    )
}

/// Сделка — «наполнитель», которым хвост доводится до размера, не влезающего в `TINY`.
/// Цены разбросаны по бакетам: иначе OHLCV схлопнулся бы в одну строку и предел не сработал.
fn trade(ts: i64, i: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(MID + (i % 23) as f64),
            size: to_fixed(0.25),
            side: if i % 3 == 0 { Side::Sell } else { Side::Buy },
            ts_exch_ms: ts,
        },
    )
}

fn append(dir: &std::path::Path, evs: Vec<EventKind>) {
    let mut j =
        Journal::open_with(dir, cfg()).unwrap_or_else(|e| setup_failed(&format!("open_with: {e}")));
    for e in evs {
        j.append(e)
            .unwrap_or_else(|e| setup_failed(&format!("append: {e}")));
    }
    j.flush()
        .unwrap_or_else(|e| setup_failed(&format!("flush: {e}")));
}

/// Бэклог до подключения клиента: якорь + дельты на 2.2 с ⇒ снимок несёт точки глубины.
fn backlog() -> Vec<EventKind> {
    let mut evs = vec![anchor(T, 0.05)];
    let mut ts = T + 100;
    for seq in 2..=22_u64 {
        evs.push(delta(ts, seq));
        ts += 100;
    }
    evs
}

const SEQ_AFTER_BACKLOG: u64 = 23;
const TS_AFTER_BACKLOG: i64 = T + 2_200;

/// Хвост ПОСЛЕ подключения: дельты (носители глубины) вперемешку со сделками (объём,
/// которым кадр перерастает `TINY`). Пять каденс-интервалов ⇒ точки глубины в хвосте есть.
fn fat_tail(n_intervals: i64) -> Vec<EventKind> {
    let mut evs = Vec::new();
    let mut seq = SEQ_AFTER_BACKLOG;
    let mut ts = TS_AFTER_BACKLOG;
    for _ in 0..n_intervals {
        for k in 0..10_u64 {
            // Узкая дельта — носитель предмета (уровни ВНЕ прод-полосы, как в §2 спеки);
            // широкая — наполнитель окна heatmap, которым кадр перерастает `TINY`.
            evs.push(delta(ts, seq));
            seq += 1;
            evs.push(wide_delta(ts + 10, seq, k));
            seq += 1;
            for i in 0..6 {
                evs.push(trade(ts, (seq as i64) + i + k as i64));
            }
            ts += 100;
        }
    }
    evs
}

fn replay(dir: &std::path::Path) -> Snapshot {
    gateway::snapshot(dir, EpochFilter::OwnCaptureOnly, &sel(), Cursor::LATEST)
        .unwrap_or_else(|e| setup_failed(&format!("полный реплей: {e}")))
}

/// Состояние подписки, доведённое до хвоста бэклога, и снимок-при-подключении клиента.
/// Возвращает `(live, client)`; каталог чекпоинта намеренно переживает вызов.
fn connect(dir: &std::path::Path) -> (gateway::LiveReducer, Snapshot) {
    let ckpt = tempfile::tempdir().unwrap_or_else(|e| setup_failed(&format!("ckpt: {e}")));
    let s = sel();
    let (mut live, _) =
        gateway::LiveReducer::resume(dir, EpochFilter::OwnCaptureOnly, &s, ckpt.path())
            .unwrap_or_else(|e| setup_failed(&format!("resume: {e}")));
    loop {
        match live.pump(dir, EpochFilter::OwnCaptureOnly, PUSH_MAX_EVENTS) {
            Ok((f, _, _)) if f.is_empty() => break,
            Ok(_) => continue,
            Err(e) => setup_failed(&format!("pump бэклога: {e}")),
        }
    }
    let client = live
        .snapshot_checked()
        .unwrap_or_else(|e| setup_failed(&format!("снимок-при-подключении отвергнут: {e}")));
    std::mem::forget(ckpt);
    (live, client)
}

/// Число точек глубины во всех строках снимка — мера «есть ли вообще что терять».
fn depth_points(s: &Snapshot) -> usize {
    s.series.depth_series.iter().map(|r| r.series.len()).sum()
}

/// Диагностика: какие поля бандла разошлись. Печатается в сообщении ассерта, чтобы
/// следующий круг не выяснял это заново.
fn diverged(client: &Snapshot, full: &Snapshot) -> String {
    let c = &client.series;
    let f = &full.series;
    let mut out: Vec<String> = Vec::new();
    macro_rules! chk {
        ($field:ident) => {
            if c.$field != f.$field {
                out.push(format!(
                    "{} (клиент {}, реплей {})",
                    stringify!($field),
                    c.$field.len(),
                    f.$field.len()
                ));
            }
        };
    }
    chk!(ohlcv);
    chk!(cumulative_delta);
    chk!(cvd_session_base);
    chk!(depth_series);
    chk!(vwap);
    chk!(volume_profile);
    chk!(vp_session_max_time_s);
    chk!(heatmap);
    chk!(cob);
    chk!(volume_bubbles);
    if c.cadence_ms != f.cadence_ms {
        out.push("cadence_ms".to_string());
    }
    // `depth_series` сравнивается ещё и поточечно: длины строк совпадают почти всегда,
    // расходятся ЗНАЧЕНИЯ (замер `M-77` §2).
    if c.depth_series != f.depth_series {
        out.push(format!(
            "depth_series точек: клиент {}, реплей {}",
            depth_points(client),
            depth_points(full)
        ));
    }
    if out.is_empty() {
        "расхождений по полям нет".to_string()
    } else {
        out.join("; ")
    }
}

/// Прогон: подключение → отказы под стоящим пределом → снятие предела → догон.
/// Возвращает `(клиент, число отказов, терминальность после отказов, кадров доставлено)`.
fn drive_with_refusals(
    dir: &std::path::Path,
    tail: Vec<EventKind>,
    max_events: usize,
    cap: usize,
) -> (Snapshot, usize, bool, usize) {
    gateway::set_effective_max_response_bytes(usize::MAX);
    append(dir, backlog());
    let (mut live, mut client) = connect(dir);
    append(dir, tail);

    // ── Фаза отказов: предел СТОИТ, клиенту не достаётся ничего ─────────────────────
    gateway::set_effective_max_response_bytes(cap);
    let mut refusals = 0usize;
    for _ in 0..REFUSALS {
        match live.pump(dir, EpochFilter::OwnCaptureOnly, max_events) {
            Ok((frames, _, _)) => {
                for f in &frames {
                    client.apply(f);
                }
            }
            Err(_) => refusals += 1,
        }
    }
    let terminal = live.is_cap_terminal();

    // ── Фаза догона: предел снят, всё, что сервер отдаст, клиент применяет ──────────
    gateway::set_effective_max_response_bytes(usize::MAX);
    let mut delivered = 0usize;
    loop {
        match live.pump(dir, EpochFilter::OwnCaptureOnly, max_events) {
            Ok((frames, _, _)) if frames.is_empty() => break,
            Ok((frames, _, _)) => {
                for f in &frames {
                    delivered += 1;
                    client.apply(f);
                }
            }
            Err(e) => setup_failed(&format!("pump догона отказал при снятом пределе: {e}")),
        }
    }
    gateway::set_effective_max_response_bytes(gateway::DEFAULT_MAX_RESPONSE_BYTES);
    (client, refusals, terminal, delivered)
}

// ═══════════════════════════════════════════════════════════════════════════════════════
// W1 — ДИСКРИМИНАТОР: опасное окно ДОСТИЖИМО. Зелен и до, и после развязки
// ═══════════════════════════════════════════════════════════════════════════════════════

/// **W1 (дискриминатор, `Р-4`).** Отказ по объёму РЕАЛЬНО оставляет состояние впереди
/// закладки доставки, и подписка объявляет себя терминальной.
///
/// Без него `W2`/`W3` зелены вакуумно: если предел на этой фикстуре не срабатывает, они
/// судят установившийся режим, который уже покрыт соседним файлом. Guard стоит на
/// ДОСТИЖИМОСТИ СЦЕНАРИЯ, а не на исходе реализации — урок `P6`
/// (`red_egress_cap_paths.rs:645-651`): guard на исход краснел против ПРАВИЛЬНОГО фикса.
#[test]
fn vb_i_2_w1_refusal_by_cap_is_reachable_and_signals_terminality() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("tempdir");

    gateway::set_effective_max_response_bytes(usize::MAX);
    append(dir.path(), backlog());
    let (mut live, _client) = connect(dir.path());
    let cursor_before = live.cursor();
    append(dir.path(), fat_tail(5));

    // Сценарий существует, если ДАННЫЕ хвоста не влезают в предел одним ответом. Это
    // свойство фикстуры и предела, независимое от поведения реализации.
    let reference = replay(dir.path());
    let full_bytes = serde_json::to_vec(&reference.series)
        .unwrap_or_else(|e| setup_failed(&format!("эталон не сериализуется: {e}")))
        .len();
    if full_bytes <= TINY {
        setup_failed(&format!(
            "эталонная серия — {full_bytes} Б при пределе {TINY} Б: данные укладываются, \
             сценарий «предел стоит» недостижим в принципе"
        ));
    }

    gateway::set_effective_max_response_bytes(TINY);
    let mut refusals = 0usize;
    for _ in 0..REFUSALS {
        if live
            .pump(dir.path(), EpochFilter::OwnCaptureOnly, PUSH_MAX_EVENTS)
            .is_err()
        {
            refusals += 1;
        }
    }
    let terminal = live.is_cap_terminal();
    let counted = live.cap_refusals();
    let cursor_after = live.cursor();
    gateway::set_effective_max_response_bytes(gateway::DEFAULT_MAX_RESPONSE_BYTES);

    assert_eq!(
        refusals, REFUSALS,
        "W1: из {REFUSALS} попыток под пределом {TINY} Б отказов {refusals}. Опасное окно \
         M-77 (состояние впереди закладки доставки) не воспроизведено — значит W2/W3 \
         судили бы установившийся режим, уже покрытый red_m77_frame_book_continuity.rs. \
         Эталон весит {full_bytes} Б."
    );
    assert!(
        terminal && counted == REFUSALS,
        "W1: после {REFUSALS} отказов подряд `is_cap_terminal()` = {terminal}, \
         `cap_refusals()` = {counted}. Вызыватель обязан ОТЛИЧАТЬ первый терминальный отказ \
         от повторного (`crates/gateway/src/lib.rs:4111-4120`), иначе подписка молча \
         зависает."
    );
    assert_eq!(
        cursor_before, cursor_after,
        "W1: закладка ДОСТАВКИ сдвинулась при отказах ({cursor_before:?} → {cursor_after:?}). \
         Отказ обязан оставлять её на месте (`R-140`): иначе кадр потерян, а клиент считает \
         себя полным."
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════
// ПРЕДМЕТ
// ═══════════════════════════════════════════════════════════════════════════════════════

/// **W2 — ЯДРО ОКНА.** После отказов по объёму и последующего догона состояние КЛИЕНТА
/// бит-идентично полному реплею.
///
/// Здесь `self.full` заведомо ушёл вперёд закладки: батч свёрнут, но отвергнут. Кадр
/// повторной попытки строится заново от того же `batch_from`, и его книго-зависимые серии
/// обязаны описывать ЕГО диапазон — не то, где сейчас стоит состояние, и не пустоту,
/// оставшуюся от того, что `full.apply` на повторном проходе не зовётся (`:4020-4026`).
#[test]
fn vb_i_2_w2_client_equals_replay_after_refusals_are_retried() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("tempdir");
    let (client, refusals, terminal, delivered) =
        drive_with_refusals(dir.path(), fat_tail(5), PUSH_MAX_EVENTS, TINY);
    let full = replay(dir.path());

    if refusals != REFUSALS {
        setup_failed(&format!(
            "отказов {refusals} из {REFUSALS} — окно не воспроизведено (см. W1)"
        ));
    }
    if !terminal {
        setup_failed("после отказов подписка не объявлена терминальной — см. W1");
    }
    if delivered == 0 {
        setup_failed("догон не доставил НИ ОДНОГО кадра — сравнивать нечего");
    }
    // Р-4(а): признак различает миры, только если в хвосте ЕСТЬ что терять.
    if depth_points(&full) == 0 {
        setup_failed("реплей не дал ни одной точки глубины — фикстура не различает миры");
    }

    // `assert!`, а не `assert_eq!`: последний дампит ОБА `SeriesBundle` целиком (замер:
    // 127 КБ на прогон), что упирается в лимит ответа агента и топит лог CI. Расхождение
    // называет `diverged()` — адресно и в одну строку (`commit-discipline.md`: «зелёное
    // агрегируется, красное печатается целиком» — целиком печатается ПРИЧИНА, не дамп).
    assert!(
        client.series == full.series,
        "VB-I-2 НАРУШЕН В ОКНЕ ОТКАЗА (M-77 W2): после {refusals} отказов по объёму и \
         догона {delivered} кадрами состояние клиента не равно полному реплею. \
         Разошлось: {}. \
         Контракт развязки Б (`M-77` §6bis): книго-зависимая серия кадра есть функция его \
         ДИАПАЗОНА, а не текущей позиции `self.full`. Наиболее вероятная причина красноты \
         после внесения Б — серия берётся как ПОБОЧНЫЙ ЭФФЕКТ `full.apply(event)`, который \
         на повторном проходе не зовётся для уже свёрнутых seq \
         (`crates/gateway/src/lib.rs:4020-4026`), и кадр повторной попытки уходит пустым.",
        diverged(&client, &full)
    );
}

/// **W3 — ОКНО ПОВЕРХ РОЛЛОВЕРА БАТЧА.** Отказ настигает тик, который дробится на
/// несколько батчей: `self.full` уходит вперёд на ЦЕЛЫЙ батч, и повторная попытка обязана
/// пересобрать ВСЕ невыданные батчи, каждый со своим диапазоном.
///
/// Отдельно от `W2`, потому что бьёт по ВТОРОМУ месту создания батча
/// (`crates/gateway/src/lib.rs:4011`), которое кандидат-развязка А, замеренная в `M-77` §6,
/// не трогала вовсе — и четыре теста соседнего файла остались против неё ЗЕЛЁНЫМИ
/// (замер `M-77` §9bis). Ролловер — не теоретический случай: на проде `PUSH_MAX_EVENTS`
/// равен 256, и всякий backlog длиннее одного батча идёт именно через него.
#[test]
fn vb_i_2_w3_client_equals_replay_when_refusal_hits_a_batch_rollover() {
    let _g = serial();
    let dir = tempfile::tempdir().expect("tempdir");
    // `max_events = ROLLOVER_BATCH` при хвосте в сотни событий ⇒ много батчей в одном `pump`.
    let (client, refusals, _terminal, delivered) =
        drive_with_refusals(dir.path(), fat_tail(5), ROLLOVER_BATCH, ROLLOVER_CAP);
    let full = replay(dir.path());

    if refusals == 0 {
        setup_failed(&format!(
            "ни одного отказа при max_events={ROLLOVER_BATCH} и пределе {ROLLOVER_CAP} Б — \
             окно не воспроизведено"
        ));
    }
    if delivered < 2 {
        setup_failed(&format!(
            "догон доставил {delivered} кадров при max_events={ROLLOVER_BATCH} — дробление \
             не состоялось, ролловер батча не задет"
        ));
    }
    if depth_points(&full) == 0 {
        setup_failed("реплей не дал ни одной точки глубины — фикстура не различает миры");
    }

    assert!(
        client.series == full.series,
        "VB-I-2 НАРУШЕН НА РОЛЛОВЕРЕ БАТЧА В ОКНЕ ОТКАЗА (M-77 W3): {refusals} отказов, \
         догон {delivered} кадрами, состояние клиента не равно реплею. Разошлось: {}. \
         Второе место создания батча — `crates/gateway/src/lib.rs:4011` — обязано \
         подчиняться тому же правилу источника, что и первое (`:3900`). Правка, внесённая \
         только в одно из двух, оставляет дефект живым на всяком тике длиннее одного батча.",
        diverged(&client, &full)
    );
}
