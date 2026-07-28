//! RED TD-046 (sacred, architect-only) — **GW-I-10: `timeframe_ms` обязан быть выравнен на
//! границу UTC-суток; иначе селектор отвергается fail-closed.**
//!
//! ## Дефект (найден reviewer'ом на PR-гейте M-38a, 2026-07-27)
//!
//! Session-anchored серии (CVD — M-38a/TD-043, SVP — M-24) якорятся на 00:00 UTC. Модель
//! сессии постулирует `session_of(time_s) = time_s.div_euclid(86_400)` ≡ `utc_session_id(ts_ms)`
//! (`milestones/M-38a-cvd-session-ledger.md:60-62`). **Это верно ТОЛЬКО если бакет не пересекает
//! 00:00 UTC.** `GATEWAY_TIMEFRAME_MS` — свободный env (`crates/gateway-serve/src/lib.rs:516`),
//! парсится fail-closed по ФОРМАТУ, но НЕ проверяется на делимость суток.
//!
//! При `timeframe_ms`, не делящем `86_400_000` нацело, бакет накрывает полночь и сделки ДВУХ
//! сессий попадают в ОДИН `bucket_time_s`. Замер reviewer'а (`GATEWAY_TIMEFRAME_MS=11_000`,
//! две сделки: `d2−2s` BUY 5.0 и `d2+2s` SELL 3.0):
//!
//! ```text
//! cumulative_delta = [(1752105597, 500000000), (1752105597, -300000000)]
//! cvd_session_base = []
//! ```
//!
//! Две строки с ОДИНАКОВЫМ `time_s` (running не монотонен — `Reducer::finish` эмитит по строке
//! на сессию), а merge-путь (`session_of(time_s)`) сваливает обе в ОДНУ сессию. Для бакета,
//! накрывающего полночь, КОРРЕКТНОГО `session_id` не существует в принципе — это не «неудобный
//! конфиг», а неопределённая семантика.
//!
//! ## Дизайн фикса (architect, gates.md §4 — reviewer описывает дефект, architect проектирует)
//!
//! **Выбран fail-closed гвард, а НЕ вывод сессии бакета из `ts_exch_ms`.** Обоснование:
//! вывод сессии из `ts_exch_ms` требует, чтобы КЛЮЧ бакета на проводе нёс сессию
//! (`cumulative_delta: Vec<(time_s, value)>` физически не различает две сессии с равным
//! `time_s`) ⇒ non-additive смена формы v7→v8 + переработка merge-пути — прямо в момент, когда
//! M-38b замораживает форму состояния в чекпоинте. Ценой этого покупается поддержка конфигов,
//! для которых session-anchored серия всё равно семантически неопределена. Fail-closed —
//! честный ответ: неизвестный/неподдерживаемый вход → отказ, не «правдоподобное» значение
//! (`CLAUDE.md` fail-closed; docs/08 R7 называет тихий fallback единственным отступлением).
//!
//! **Гвард живёт в `crates/gateway` (модель владеет своим предусловием), а НЕ только в
//! `serve_config_from_env`.** Проверка ТОЛЬКО в конфиге транспорта оставила бы байпас-поверхность:
//! `Selector` конструируется напрямую любым консюмером библиотеки (чекпоинтер M-38b, shared-tailer
//! M-39, research-cli). Отвергать обязаны ВСЕ публичные входы: `snapshot` / `frames_since` /
//! `replay` (их тип уже `io::Result` — смена сигнатуры не нужна). `gateway-serve` дополнительно
//! падает на СТАРТЕ (отдельный оракул `red_timeframe_guard_startup.rs`), чтобы дефект не ждал
//! первого подключения.
//!
//! Требуемая форма отказа: `io::ErrorKind::InvalidInput`, сообщение содержит `timeframe_ms`.
//!
//! ## Почему это RED сейчас (замерено прогоном, не выведено рассуждением)
//!
//! Гварда нет: `snapshot` с `timeframe_ms = 11_000` возвращает `Ok(..)` с испорченной серией —
//! прогон воспроизвёл замер reviewer'а буквально:
//! `cumulative_delta: [(1752105597, 500000000), (1752105597, -300000000)]`.
//!
//! **Прогон опроверг ожидание про `timeframe_ms <= 0`.** Паники НЕТ: `Reducer::bucket_time_s`
//! (`crates/gateway/src/lib.rs:671-677`) возвращает `None` при `timeframe_ms <= 0`, поэтому
//! `ohlcv` / `cumulative_delta` / `vwap` / `heatmap` / `bubbles` выходят **ПУСТЫМИ**, а
//! `volume_profile` — **ЗАПОЛНЕННЫМ** (VP якорится напрямую от `utc_session_id(ts_ms)`, мимо
//! бакета) с обеими сессиями `[20278, 20279]`. То есть кокпит получил бы `Ok` со свечами,
//! которых нет, и профилем объёма, который есть — **тихая полу-правда без единой ошибки**.
//! Это ХУЖЕ паники (паника хотя бы видна) и относится к тому же классу «код на main ≠ функция
//! в проде». Поэтому оракулы `zero_/negative_` требуют именно `Err(InvalidInput)`, а не
//! «отсутствия паники».
//!
//! ## testing.md чек-лист
//! - п.1 **асимметрия** — `misaligned_*`: сделки по одну сторону полуночи ≠ по другую (BUY 5.0 до /
//!   SELL 3.0 после, разные размеры, разные стороны).
//! - п.2 **множественность** — `aligned_timeframe_keeps_sessions_separate`: 2+ сделки в ОДНОМ
//!   бакете и 2+ бакета в каждой сессии.
//! - п.3 **отсутствие** — S2 не наследует running S1 (`base` не «додумывается»).
//! - п.4 **границы** — `0` / отрицательный / ровно `86_400_000` / больше суток (недельный) /
//!   `1` (минимальный делитель).
//! - п.5 прод-масштаб — N/A: гвард — чистая проверка конфига без I/O-границы ресурса.
//! - п.6 **композиция стадий** — гвард обязан держать на ВСЕХ трёх входах (`snapshot`,
//!   `frames_since`, `replay`), иначе байпас через merge-путь.
//! - п.7 **ПАРНЫЙ vantage** — `aligned_timeframes_accepted` доказывает, что гвард не
//!   переширокий: заглушка «всегда Err» валит этот тест (анти-плацебо к анти-плацебо).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const DAY_S: i64 = 86_400;
/// Граница UTC-суток, использованная в репро reviewer'а: `20_279 * 86_400_000` = 1752105600000 мс.
const D2_MS: i64 = 20_279 * DAY_MS;

fn session_of(time_s: i64) -> i64 {
    time_s.div_euclid(DAY_S)
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn trade(price: f64, size: f64, side: Side, ts: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms: ts,
        },
    )
}

fn journal_of(events: Vec<EventKind>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for e in events {
            j.append(e).expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

fn sel(timeframe_ms: i64) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms,
        bands: vec![0.001],
        window_ms: None,
    }
}

/// Репро reviewer'а: две сделки по РАЗНЫЕ стороны 00:00 UTC (асимметрия — разные стороны,
/// разные размеры). При `timeframe_ms=11_000` обе попадают в бакет `1752105597`.
fn boundary_journal() -> tempfile::TempDir {
    journal_of(vec![
        trade(100.0, 5.0, Side::Buy, D2_MS - 2_000),
        trade(100.0, 3.0, Side::Sell, D2_MS + 2_000),
    ])
}

/// Единственная точка правды о требуемой форме отказа — engine-dev реализует ровно это.
fn assert_rejected(what: &str, res: std::io::Result<impl std::fmt::Debug>, timeframe_ms: i64) {
    match res {
        Err(e) => {
            assert_eq!(
                e.kind(),
                std::io::ErrorKind::InvalidInput,
                "{what}: timeframe_ms={timeframe_ms} отвергнут, но НЕ как InvalidInput: {e:?}"
            );
            let msg = e.to_string();
            assert!(
                msg.contains("timeframe_ms"),
                "{what}: сообщение об отказе обязано называть поле `timeframe_ms` \
                 (оператор должен понять, ЧТО чинить), получено: {msg:?}"
            );
        }
        Ok(v) => panic!(
            "GW-I-10 НАРУШЕН — {what}: timeframe_ms={timeframe_ms} НЕ делит {DAY_MS} нацело \
             (бакет накрывает 00:00 UTC ⇒ session_id бакета не определён), но вход принят. \
             Выход: {v:?}"
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RED 1 — невыравненный timeframe отвергается на ВСЕХ публичных входах (п.6 композиция)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn misaligned_timeframe_rejected_by_snapshot() {
    let dir = boundary_journal();
    let s = sel(11_000);
    assert_rejected(
        "gateway::snapshot",
        gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST),
        11_000,
    );
}

#[test]
fn misaligned_timeframe_rejected_by_frames_since() {
    let dir = boundary_journal();
    let s = sel(11_000);
    assert_rejected(
        "gateway::frames_since",
        gateway::frames_since(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &s,
            Cursor::START,
            usize::MAX,
        ),
        11_000,
    );
}

#[test]
fn misaligned_timeframe_rejected_by_replay() {
    let dir = boundary_journal();
    let s = sel(11_000);
    assert_rejected(
        "gateway::replay",
        gateway::replay(
            dir.path(),
            EpochFilter::OwnCaptureOnly,
            &s,
            Cursor::START,
            Cursor::LATEST,
        ),
        11_000,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// RED 2 — границы (п.4): 0 / отрицательный / больше суток недельный
// ─────────────────────────────────────────────────────────────────────────────

/// `timeframe_ms = 0` СЕЙЧАС отдаёт `Ok` с ПУСТЫМИ time-бакетными сериями и ЗАПОЛНЕННЫМ
/// `volume_profile` (`bucket_time_s` → `None`, VP считается мимо бакета) — тихая полу-правда.
/// `catch_unwind` оставлен как страховка: если engine-dev реализует гвард через `assert!`/
/// `unwrap`, тест обязан отличить панику от честного `Err`, а не «случайно позеленеть».
#[test]
fn zero_timeframe_rejected_not_panic() {
    let dir = boundary_journal();
    let s = sel(0);
    let res = std::panic::catch_unwind(|| {
        gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
    });
    match res {
        Ok(r) => assert_rejected("gateway::snapshot(tf=0)", r, 0),
        Err(_) => panic!(
            "GW-I-10 НАРУШЕН: timeframe_ms=0 ПАНИКУЕТ (деление на ноль в бакетировании) \
             вместо fail-closed Err(InvalidInput). Паника — не отказ: она валит соединение \
             в рантайме вместо явной ошибки конфигурации на входе."
        ),
    }
}

#[test]
fn negative_timeframe_rejected() {
    let dir = boundary_journal();
    let s = sel(-1_000);
    let res = std::panic::catch_unwind(|| {
        gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
    });
    match res {
        Ok(r) => assert_rejected("gateway::snapshot(tf<0)", r, -1_000),
        Err(_) => panic!("GW-I-10 НАРУШЕН: timeframe_ms=-1000 паникует вместо Err(InvalidInput)"),
    }
}

/// Недельный бакет «круглый», но НЕ делит сутки (`86_400_000 % 604_800_000 = 86_400_000`):
/// он накрывает 7 полуночей. Гвард обязан проверять ДЕЛИМОСТЬ СУТОК, а не «круглость».
/// Заглушка вида `timeframe_ms % 1000 == 0` валится ровно здесь.
#[test]
fn weekly_timeframe_longer_than_day_rejected() {
    let dir = boundary_journal();
    let weekly = 7 * DAY_MS;
    let s = sel(weekly);
    assert_rejected(
        "gateway::snapshot(недельный бакет)",
        gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST),
        weekly,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// RED 3 — ПАРНЫЙ vantage (п.7): гвард не переширокий. Валит заглушку «всегда Err».
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn aligned_timeframes_accepted() {
    let dir = boundary_journal();
    // Все делят 86_400_000 нацело: минимальный, прод-дефолт, минутный, часовой, ровно сутки.
    for tf in [1_i64, 1_000, 60_000, 3_600_000, DAY_MS] {
        let s = sel(tf);
        let got = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST);
        assert!(
            got.is_ok(),
            "GW-I-10 ПЕРЕШИРОК: timeframe_ms={tf} делит {DAY_MS} нацело (бакет не пересекает \
             00:00 UTC) и обязан приниматься, но отвергнут: {:?}",
            got.err()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Структурный инвариант на ПРИНЯТОМ пути (регресс-гвард, GREEN сегодня — не RED).
// Фиксирует, ЧТО именно защищает гвард: на любом принятом timeframe бакет принадлежит
// ровно одной сессии ⇒ `time_s` строго возрастает и сессии не смешиваются.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn aligned_timeframe_keeps_sessions_separate() {
    // п.2 множественность: 2 сделки в одном бакете и 2+ бакета в каждой сессии;
    // п.1 асимметрия: в S1 преобладает BUY, в S2 — SELL, размеры разные.
    let dir = journal_of(vec![
        trade(100.0, 5.0, Side::Buy, D2_MS - 3_000),
        trade(100.0, 2.0, Side::Buy, D2_MS - 3_000 + 100), // тот же бакет (tf=1000)
        trade(100.0, 1.0, Side::Sell, D2_MS - 2_000),
        trade(100.0, 3.0, Side::Sell, D2_MS + 2_000),
        trade(100.0, 4.0, Side::Sell, D2_MS + 2_000 + 100), // тот же бакет
        trade(100.0, 1.0, Side::Buy, D2_MS + 5_000),
    ]);
    let s = sel(1_000);
    let snap = gateway::snapshot(dir.path(), EpochFilter::OwnCaptureOnly, &s, Cursor::LATEST)
        .expect("snapshot на выравненном timeframe обязан строиться");
    let cd = &snap.series.cumulative_delta;

    // Строго возрастающий time_s: дубль ключа = бакет описан двумя строками (симптом TD-046).
    for w in cd.windows(2) {
        assert!(
            w[0].0 < w[1].0,
            "time_s обязан строго возрастать (дубль = бакет накрыл две сессии): {cd:?}"
        );
    }

    // Обе сессии присутствуют и различимы.
    let s1 = session_of((D2_MS - 1) / 1_000);
    let s2 = session_of(D2_MS / 1_000);
    assert_ne!(s1, s2, "фикстура обязана пересекать 00:00 UTC");
    for sid in [s1, s2] {
        assert!(
            cd.iter().any(|(t, _)| session_of(*t) == sid),
            "сессия {sid} обязана быть представлена в cumulative_delta: {cd:?}"
        );
    }

    // п.3 отсутствие: running S2 стартует с нуля — НЕ наследует итог S1 (M-38a/TD-043).
    // S1: +5 +2 −1 = +6.0; S2: −3 −4 +1 = −6.0. Наследование дало бы 0.0 в конце S2.
    let last_s1 = cd
        .iter()
        .filter(|(t, _)| session_of(*t) == s1)
        .next_back()
        .expect("S1 в серии")
        .1;
    let last_s2 = cd
        .iter()
        .filter(|(t, _)| session_of(*t) == s2)
        .next_back()
        .expect("S2 в серии")
        .1;
    assert_eq!(last_s1, to_fixed(6.0), "итог S1: +5 +2 −1 = +6.0");
    assert_eq!(
        last_s2,
        to_fixed(-6.0),
        "итог S2 обязан считаться С НУЛЯ (−3 −4 +1 = −6.0), а не наследовать running S1 \
         (наследование дало бы 0.0): {cd:?}"
    );
}
