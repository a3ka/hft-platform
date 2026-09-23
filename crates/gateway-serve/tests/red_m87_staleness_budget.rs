//! RED M-87 круг `R-196` (sacred, architect-only) — **порог свежести и каденция прогрева
//! суть ОДНА пара параметров.**
//!
//! Находка `R-196` B2, предъявленная ЗАМЕРОМ НА ПРОДЕ, а не рассуждением:
//! ```text
//! next_seq_t0 = 670349265 ; next_seq_t30 = 670353524   → 4 259 событий за 30 с ≈ 142/с
//! прогреватель:  */15 * * * *  (deploy/cron.d/journal-retention:79)
//! между двумя прогревами:  142 × 900 ≈ 127 770 событий
//! порог в коде:            const TAIL_SEQ_BUDGET: u64 = 250
//! ```
//! Порог перекрывается через ≈1.8 секунды после прогрева, и дальше ПЯТНАДЦАТЬ МИНУТ подряд
//! `readiness` отвечает `NotReady` на каждый запрос. Кокпит не получает кадров вообще — при
//! живом контейнере и свежем heartbeat. Три liveness-проверки деплой-гейта такого не видят
//! (`gates.md` §8 предупреждает ровно про этот класс).
//!
//! **Чья это ошибка.** Моя: спека §4.1 требовала «порог — конфиг политики, не константа», но
//! НЕ НАЗВАЛА, от чего он считается. Требование без опорной величины исполняется числом с
//! потолка — что и произошло. Порог и каденция прогрева не независимы: между двумя
//! прогревами слепок ОБЯЗАН отстать ровно на объём этого интервала, и порог, меньший этого
//! объёма, делает исправную выдачу невозможной ПО ПОСТРОЕНИЮ, а не при неудачном стечении.
//!
//! **Что здесь пинуется — три разных утверждения, и путать их нельзя:**
//!  · `t1`/`t2` — ОТНОШЕНИЕ: порог ниже объёма одного прогрева ⇒ отказ СТАРТА с названием
//!    обоих чисел. Недонастроенное развёртывание обязано падать громко при запуске, а не
//!    тихо отказывать пятнадцать минут из пятнадцати;
//!  · `t3` — ЗНАЧЕНИЕ ПО УМОЛЧАНИЮ: дефолт обязан переживать ИЗМЕРЕННУЮ прод-каденцию;
//!  · `t4`/`t5` — ПОВЕДЕНИЕ: в пределах порога слепок годен, сверх порога — названный отказ.
//!    Без `t4` реализация «отказывать всегда» прошла бы `t5`; без `t5` — «принимать всегда».
//!
//! Фикстуры `t4`/`t5` работают на МАЛЫХ числах намеренно: они судят ОТНОШЕНИЕ лага к порогу,
//! а не прод-объём. Воспроизводить 128 000 событий ради линейной величины — значит платить
//! минутами прогона за то, что уже доказано арифметикой в `t3`.
//!
//! COMPILE-RED по построению: у `AdmissionPolicy` ещё нет полей `max_tail_events` /
//! `expected_warmup_events`, нет `validate()`, а `readiness` не принимает политику.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Venue};
use gateway::Selector;
use gateway_serve::admission::{readiness, AdmissionPolicy, LiveProfile, ServingOutcome};
use journal::{EpochFilter, Journal, WriterConfig};

const T0: i64 = 1_784_116_800_000;

/// ИЗМЕРЕННАЯ прод-каденция (`R-196` B2, замер 2026-09-23). Числа названы здесь, а не
/// спрятаны в коде: величина, на которой стоит инвариант, сама подлежит предъявлению
/// (`testing.md` §«Оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ» п.1).
const PROD_EVENTS_PER_SEC: u64 = 142;
const WARMUP_INTERVAL_SEC: u64 = 900; // cron */15
const PROD_WARMUP_VOLUME: u64 = PROD_EVENTS_PER_SEC * WARMUP_INTERVAL_SEC; // ≈127 800

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

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

fn policy_with(max_tail_events: u64, expected_warmup_events: u64) -> AdmissionPolicy {
    AdmissionPolicy {
        allowed_symbols: vec!["BTCUSDT".to_string()],
        canonical_bands: vec![0.001],
        allowed_profiles: vec![LiveProfile {
            timeframe_ms: 1_000,
            window_ms: 60_000,
            depth_cadence_ms: None,
        }],
        max_concurrent_serves: 1,
        max_tail_events,
        expected_warmup_events,
    }
}

fn append_events(dir: &std::path::Path, n: usize, base_ms: i64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in 0..n {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::L2Snapshot {
                bids: vec![Level {
                    price: to_fixed(65_000.0 + i as f64),
                    size: to_fixed(1.0),
                }],
                asks: vec![Level {
                    price: to_fixed(65_100.0 + i as f64),
                    size: to_fixed(1.0),
                }],
                ts_exch_ms: base_ms + (i as i64) * 100,
            },
        ))
        .expect("append");
    }
    j.flush().expect("flush");
}

/// Журнал, слепок которого отстал РОВНО на `lag` событий: пишем `head`, снимаем слепок,
/// дописываем `lag`. Отставание задаётся конструкцией, а не ожиданием.
fn journal_with_lag(head: usize, lag: usize) -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ckpt = tempfile::tempdir().expect("ckpt");
    append_events(dir.path(), head, T0);
    gateway::checkpoint::advance(dir.path(), ckpt.path(), &sel(), EpochFilter::OwnCaptureOnly)
        .expect("advance");
    if lag > 0 {
        append_events(dir.path(), lag, T0 + 1_000_000);
    }
    (dir, ckpt)
}

// ═══════════ t1/t2 — ОТНОШЕНИЕ: недонастроенный порог валит СТАРТ, а не выдачу ═══════════

#[test]
fn t1_policy_with_threshold_below_one_warmup_cycle_is_refused_at_startup() {
    // Ровно сегодняшняя прод-ситуация в миниатюре: порог 250 при объёме цикла ≈127 800.
    let bad = policy_with(250, PROD_WARMUP_VOLUME);
    let got = bad.validate();
    assert!(
        got.is_err(),
        "R-196 B2: политика с порогом свежести 250 при объёме одного прогрева \
         {PROD_WARMUP_VOLUME} принята как ГОДНАЯ. Это конфигурация, при которой выдача \
         отказывает 100 % времени между прогревами: слепок перекрывает порог за ≈1.8 с и \
         остаётся за ним до следующего цикла. Недонастроенное развёртывание обязано падать \
         ПРИ СТАРТЕ и называть оба числа, а не молчать пятнадцать минут из пятнадцати"
    );
    let msg = got.unwrap_err();
    assert!(
        msg.contains("250") && msg.contains(&PROD_WARMUP_VOLUME.to_string()),
        "отказ старта обязан НАЗВАТЬ оба числа — порог и объём цикла, — иначе инженер \
         ищет причину в коде, а она в конфигурации. Сообщение: {msg}"
    );
}

#[test]
fn t2_policy_with_threshold_above_one_warmup_cycle_is_accepted() {
    // АНТИ-ПЛАЦЕБО к t1: «отказывать всегда» решением не является.
    let good = policy_with(PROD_WARMUP_VOLUME * 2, PROD_WARMUP_VOLUME);
    assert!(
        good.validate().is_ok(),
        "политика с запасом вдвое над объёмом цикла отвергнута — проверка старта \
         превратилась в безусловный отказ"
    );
}

// ═══════════════ t3 — ЗНАЧЕНИЕ ПО УМОЛЧАНИЮ переживает измеренную каденцию ═══════════════

#[test]
fn t3_default_threshold_survives_measured_prod_cadence() {
    let d = AdmissionPolicy::default();
    assert!(
        d.max_tail_events >= PROD_WARMUP_VOLUME,
        "дефолтный порог {} НЕ переживает измеренную прод-каденцию ({PROD_EVENTS_PER_SEC} \
         событий/с × {WARMUP_INTERVAL_SEC} с = {PROD_WARMUP_VOLUME}). Развёртывание, не \
         переопределившее порог явно, встанет — а именно так выглядит развёртывание по \
         умолчанию",
        d.max_tail_events
    );
    assert!(
        d.validate().is_ok(),
        "дефолтная политика не проходит собственную проверку старта"
    );
}

// ═══════════════ t4/t5 — ПОВЕДЕНИЕ: в пределах порога годен, сверх — назван отказ ═══════════════

#[test]
fn t4_checkpoint_lagging_within_threshold_is_ready() {
    let pol = policy_with(600, 300);
    let (dir, ckpt) = journal_with_lag(40, 300); // лаг равен объёму «цикла», порог вдвое выше
    assert_eq!(
        readiness(ckpt.path(), dir.path(), &sel(), &pol),
        ServingOutcome::Ready,
        "слепок, отставший на ОБЫЧНЫЙ объём одного цикла прогрева, признан негодным. Это \
         ровно дефект B2: отставание на величину цикла — НОРМА, а не авария, и выдача \
         обязана его переживать"
    );
}

#[test]
fn t5_checkpoint_lagging_beyond_threshold_is_not_ready() {
    let pol = policy_with(100, 50);
    let (dir, ckpt) = journal_with_lag(40, 400); // лаг вчетверо выше порога
    assert_eq!(
        readiness(ckpt.path(), dir.path(), &sel(), &pol),
        ServingOutcome::NotReady,
        "слепок, отставший СВЕРХ порога, признан годным — 'валидный слепок' становится \
         обходным путём к неограниченной докормке хвоста на живом пути"
    );
}
