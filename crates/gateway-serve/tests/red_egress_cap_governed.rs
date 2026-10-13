//! RED `PL-I-5` (sacred, architect-only) — **РАЗОБРАННЫЙ ПРЕДЕЛ ДОХОДИТ ДО ТОЧЕК ПРИМЕНЕНИЯ.**
//!
//! Милестоун `milestones/M-71-egress-cap.md`. Исполнение вердиктов
//! `research/reviews/R-133-M-71-egress-cap-impl.md` (блокер **B-1**) и
//! `research/critiques/C-166-M-71-egress-cap-rev6.md` (обе находки приняты целиком).
//!
//! # Первая редакция этого файла БЫЛА НЕВЕРНА — и вот чем, дословно
//!
//! Она требовала перенести предел в поле `ServeConfig::max_response_bytes`. `C-166` показал
//! два дефекта, и оба справедливы:
//!
//! 1. **Задача была невыполнима легально.** `ServeConfig` конструируется литералом в
//!    ДЕВЯТИ файлах `crates/gateway-serve/tests/**` — sacred-зоне architect'а. Новое поле
//!    ломает все девять; у dev'а не остаётся ни одного разрешённого хода.
//! 2. **Оракул обходил `serve_config_from_env`** и потому не мог поймать ровно тот дефект,
//!    ради которого написан: env-значение отбрасывается ИМЕННО там.
//!
//! **А главное я нашёл сам, когда наконец применил правило предшественника** (`reading-map`
//! §2 — «искать РЕШЁННОЕ прежде, чем проектировать»): в этом крейте УЖЕ ЕСТЬ домашний
//! образец, прошедший гейты ДВАЖДЫ:
//!
//! | механизм | подпись founder'а | читается |
//! |---|---|---|
//! | `EFFECTIVE_MAX_SUBS` (M-65) | 16 | `lib.rs:821` |
//! | `EFFECTIVE_GRACE_MS` (M-65 задача 13 N-3) | 250 мс | `lib.rs:603` |
//! | `EFFECTIVE_MAX_RESPONSE_BYTES` (M-71) | `П-020`, 2 000 000 | **НИКЕМ** |
//!
//! Комментарий у первого дословно называет причину выбора atomic'а вместо поля: «добавление
//! поля сломало бы тесты с фиксированной формой литерала `ServeConfig { .. }`». То есть dev
//! не изобрёл плохой образец — он повторил домашний, а я предписал ломать его, не поискав.
//!
//! # Значит дефект НЕ в глобале, а в том, что его никто не читает
//!
//! Точки применения предела живут в ДРУГОМ крейте — `crates/gateway`, — и он не может звать
//! `gateway-serve` (цикл зависимостей). Поэтому dev и передал константу: иного способа не
//! было. Развязка того же образца, но в том крейте, где предел ПРИМЕНЯЕТСЯ:
//! `gateway` заводит свой эффективный предел с сеттером, `serve_config_from_env` вызывает
//! сеттер один раз при старте, точки применения читают его вместо константы.
//!
//! Ни одно поле не добавляется, ни один sacred-литерал не ломается, dev остаётся в своей зоне.
//!
//! # Оракул судит ЦЕПЬ ЦЕЛИКОМ: `env → serve_config_from_env → эффективное значение → отказ`
//!
//! Форма взята у домашнего образца `red_max_subs_config.rs:95-98` (`effective_after`), чтобы
//! путь был тот же, каким его дёргает прод, а не сконструированный тестом (`testing.md`,
//! «Целостность гейта» свойство 1).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use gateway_serve::serve_config_from_env;
use journal::{EpochFilter, Journal, WriterConfig};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

const MID: f64 = 65_000.0;
const T0: i64 = 1_752_000_000_000;

/// Крошечный предел: заведомо меньше честного ответа фикстуры.
const TINY_CAP: usize = 1_000;
/// Сколько сделок кладём: ответ заведомо больше `TINY_CAP` и заведомо меньше прод-дефолта.
const HONEST_TRADES: usize = 200;

/// Эффективное значение — процессное. Тесты, трогающие его, обязаны идти ПОСЛЕДОВАТЕЛЬНО,
/// иначе они меряют друг друга. Тот же приём и та же причина, что в
/// `red_max_subs_config.rs:70`.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

const BASE: &[(&str, &str)] = &[
    ("GATEWAY_JWT_SECRET", "test-secret"),
    ("GATEWAY_TIMEFRAME_MS", "1000"),
];

fn base_plus(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = BASE.to_vec();
    v.extend_from_slice(extra);
    v
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 1 << 26,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "PL-I-5 governed fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: None,
        // M-68 задача 22: поле добавлено в Selector; `None` = пер-событийно,
        // то есть НЕЙТРАЛ — прежняя семантика этого теста сохранена бит-в-бит.
        depth_cadence_ms: None,
    }
}

fn journal_of_trades(n: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("SETUP: tempdir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("SETUP: open_with");
    for i in 0..n as i64 {
        j.append(EventKind::md(
            Venue::Binance,
            "BTCUSDT",
            MdPayload::Trade {
                price: to_fixed(MID + i as f64 * 0.01),
                size: to_fixed(1.0),
                side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                ts_exch_ms: T0 + i,
            },
        ))
        .expect("SETUP: append");
    }
    j.flush().expect("SETUP: flush");
    dir
}

/// Прогон конфигурации из env + снятие ЭФФЕКТИВНОГО предела ТАМ, ГДЕ ОН ПРИМЕНЯЕТСЯ.
///
/// **COMPILE-RED:** `gateway::effective_max_response_bytes()` ещё не существует — предел
/// сегодня живёт эффективным значением в `gateway-serve`, а применяется в `gateway`, и
/// мост между ними отсутствует. В этом и предмет.
fn effective_after(env: &[(&'static str, &'static str)]) -> Result<usize, String> {
    serve_config_from_env(getter(env))?;
    Ok(gateway::effective_max_response_bytes())
}

/// **N1-C (КОНТРОЛЬ) — отсутствие переменной даёт ПОДПИСАННУЮ норму, а не отказ.**
///
/// Без него `N1` был бы зелен и против реализации, которая отвергает старт при любом
/// значении. Образец — `red_max_subs_config.rs:106`.
#[test]
fn pl_i_5_n1_c_absent_var_yields_signed_default() {
    let _g = serial();
    match effective_after(BASE) {
        Ok(n) => assert_eq!(
            n,
            gateway::DEFAULT_MAX_RESPONSE_BYTES,
            "PL-I-5 N1-C: без переменной эффективный предел обязан равняться подписанной \
             норме П-020 ({}), получено {n}",
            gateway::DEFAULT_MAX_RESPONSE_BYTES
        ),
        Err(e) => panic!(
            "PL-I-5 N1-C: отсутствие GATEWAY_MAX_RESPONSE_BYTES отвергло старт ({e}). \
             Подписанная норма существует именно затем, чтобы отсутствие переменной было \
             нормой, а не отказом"
        ),
    }
}

/// **N1a (`R-133` B-1, первая половина) — значение из env ДОХОДИТ до точки применения.**
///
/// Цепь судится целиком и той же формой, какой её дёргает прод:
/// `env → serve_config_from_env → эффективное значение в крейте, который предел ПРИМЕНЯЕТ`.
/// Первая редакция оракула обходила `serve_config_from_env` и потому не могла поймать
/// отбрасывание значения (`C-166`, находка 2).
#[test]
fn pl_i_5_n1a_env_value_reaches_the_enforcement_crate() {
    let _g = serial();
    let got = effective_after(&base_plus(&[("GATEWAY_MAX_RESPONSE_BYTES", "1000")]))
        .unwrap_or_else(|e| panic!("PL-I-5 N1a SETUP: валидное значение отвергло старт: {e}"));
    assert_eq!(
        got, TINY_CAP,
        "PL-I-5 B-1: оператор задал предел {TINY_CAP} Б, а до крейта, который предел \
         ПРИМЕНЯЕТ, дошло {got}. Значение разобрано и потеряно по дороге — ручка без \
         механизма (built-not-wired, gates.md §4 DoD)"
    );
}

/// **N1b (`R-133` B-1, вторая половина) — дошедшее значение ДЕЙСТВУЕТ.**
///
/// «Значение дошло» и «значение управляет» — разные утверждения, и второе не следует из
/// первого: сегодня эффективное значение существует в `gateway-serve` и НЕ ЧИТАЕТСЯ точками
/// применения, которые передают константу. Замер `R-133`: `effective = 1000` ⇒ отдано
/// 224 854 Б.
#[test]
fn pl_i_5_n1b_reached_value_actually_governs() {
    let _g = serial();
    let dir = journal_of_trades(HONEST_TRADES);

    // Контроль внутри оракула: при подписанной норме тот же запрос обслуживается.
    effective_after(BASE).expect("SETUP: базовое окружение обязано конфигурироваться");
    let ok = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("PL-I-5 N1b SETUP: честная нагрузка отвергнута при подписанной норме");
    let honest = serde_json::to_vec(&ok.series)
        .expect("SETUP: сериализация")
        .len();
    assert!(
        honest > TINY_CAP,
        "PL-I-5 N1b SETUP: честный ответ весит {honest} Б — не больше крошечного предела \
         {TINY_CAP} Б, и различить «предел действует» от «ответ и так мал» нельзя"
    );

    // Предмет: тот же запрос при КРОШЕЧНОМ пределе обязан быть отвергнут.
    effective_after(&base_plus(&[("GATEWAY_MAX_RESPONSE_BYTES", "1000")]))
        .expect("SETUP: конфигурация с крошечным пределом");
    let got = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    );
    assert!(
        got.is_err(),
        "PL-I-5 B-1: при эффективном пределе {TINY_CAP} Б ответ на {honest} Б был ОБСЛУЖЕН. \
         Точки применения читают зашитую константу, а не эффективное значение — оператор, \
         задавший предел, получает поведение, о котором не просил"
    );
}

/// **N1-D (`A-026` O-2, вторая половина R1) — пустое значение даёт ТОТ ЖЕ ЭФФЕКТИВНЫЙ
/// предел, что и отсутствие переменной.**
///
/// Парный к `empty_and_blank_are_same_as_absent` в `red_egress_cap_startup.rs`, и парность
/// не декоративна: тот файл наблюдает только `Result`, поэтому реализация «пусто ⇒ `Ok`, но
/// эффективный предел 16» проходила бы его, оставаясь асимметричной ровно в том, ради чего
/// оракул написан. Дословный урок домашнего образца — `red_max_subs_config.rs:127-131`:
/// «Прежняя редакция сравнивала `is_err()` с `is_err()`, и это слабее».
///
/// Основание тождества — `A-015` §3 п.1; перевёрнуто в наборе решением `A-026` §1.
#[test]
fn pl_i_5_n1_d_empty_var_yields_same_effective_as_absent() {
    let _g = serial();
    let absent = effective_after(BASE);
    for v in ["", " "] {
        let empty = effective_after(&base_plus(&[("GATEWAY_MAX_RESPONSE_BYTES", v)]));
        assert_eq!(
            empty, absent,
            "PL-I-5 N1-D: GATEWAY_MAX_RESPONSE_BYTES={v:?} и ОТСУТСТВИЕ переменной обязаны \
             давать ОДИН эффективный предел (A-015 §3 п.1). Получено {empty:?} против {absent:?}"
        );
    }
    assert_eq!(
        absent,
        Ok(gateway::DEFAULT_MAX_RESPONSE_BYTES),
        "PL-I-5 N1-D: и общий исход — именно подписанная норма П-020 ({}), а не любой \
         совпавший: равенство двух ОТКАЗОВ тоже «одинаково», но решению A-015 противоречит",
        gateway::DEFAULT_MAX_RESPONSE_BYTES
    );
}

/// **N1-E (`A-026` O-6, часть (а) требования моста) — при `Err` разбора эффективное значение
/// НЕ УСТАНАВЛИВАЕТСЯ.**
///
/// Единственная safety-несущая половина «один старт, без рантайм-перезаписи»: класс
/// «parse-error → чужое/сброшенное эффективное значение», тот же, что `GW-I-14` чинил для
/// окна. Две другие части требования оракулом не пиннятся и названы в спеке §4bis.2 явно:
/// (б) — механический инвентарь вызывателей `scripts/tests/red_egress_doors.sh`; (в) —
/// внутрипроцессный запрет рантайм-переустановки — НЕПОКРЫТ, вынесен долгом (`A-026` §3bis).
///
/// **Анти-плацебо.** Падает против реализации store-before-validate: она поставит мусорное
/// или дефолтное значение до того, как вернёт `Err`.
///
/// **Дегенерированный порядок ОБЯЗАТЕЛЕН** (`testing.md` §«Дегенерированный вход»): сперва
/// валидное `V`, только потом невалидное. Без первого шага исход «эффективное значение не
/// изменилось» неотличим от «его и не было» — оракул был бы зелен против реализации, которая
/// вообще ничего не ставит, то есть против того самого `built-not-wired`, что чинит `N1a`.
#[test]
fn pl_i_5_n1_e_parse_error_does_not_install_a_value() {
    let _g = serial();

    // Шаг 1 — установить ЗАВЕДОМО НЕ-ДЕФОЛТНОЕ значение по прод-цепи.
    const V: usize = 5_000;
    let installed = effective_after(&base_plus(&[("GATEWAY_MAX_RESPONSE_BYTES", "5000")]))
        .unwrap_or_else(|e| panic!("N1-E SETUP: валидное значение отвергло старт: {e}"));
    assert_eq!(
        installed, V,
        "N1-E SETUP: валидное {V} не доехало до точки применения (получено {installed}) — \
         предмет оракула не установлен, судить нечего"
    );
    assert_ne!(
        V,
        gateway::DEFAULT_MAX_RESPONSE_BYTES,
        "N1-E SETUP: контрольное значение обязано ОТЛИЧАТЬСЯ от подписанной нормы, иначе \
         «сброс в дефолт» неотличим от «значение сохранилось»"
    );

    // Шаг 2 — скормить невалидное. Разбор обязан отказать...
    let err = effective_after(&base_plus(&[("GATEWAY_MAX_RESPONSE_BYTES", "abc")]));
    assert!(
        err.is_err(),
        "N1-E: невалидное значение обязано валить разбор (это уже пиннит оракул D; здесь — \
         предусловие). Получено {err:?}"
    );

    // ...и НЕ тронуть эффективное значение.
    assert_eq!(
        gateway::effective_max_response_bytes(),
        V,
        "N1-E (мост, часть «а»): после `Err` разбора эффективный предел стал {}, а обязан \
         остаться {V}. Сеттер вызван ДО валидации — ровно класс GW-I-14/R7: конфигурация, \
         которую отвергли, всё равно управляет сервисом",
        gateway::effective_max_response_bytes()
    );
}
