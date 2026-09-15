//! RED `M-84` (sacred, architect-only) — **СЕРВЕРНЫЙ ВХОД: по умолчанию семь полос;
//! `GATEWAY_BANDS` остаётся ЯВНЫМ переопределением переходного периода и ГРОМКО об этом
//! говорит.**
//!
//! Заведён решением арбитра `A-033` D-2/D-3. RUNTIME-RED: файл собирается и падает
//! ПОВЕДЕНИЕМ — сегодня дефолт `"0.001"` (`crates/gateway-serve/src/lib.rs:2010`,
//! разбор `:2082-2087`), то есть сервер сетку НЕ задаёт, а берёт её у оператора.
//!
//! ## ПОЧЕМУ ЭТОТ ФАЙЛ ВООБЩЕ ПОЯВИЛСЯ — ЦЕНА ПРЕЖНЕЙ КОНСТРУКЦИИ
//!
//! Круги 1-3 держали гвард СОСТАВА в `gateway::validate_selector`, то есть в библиотеке, по
//! которой ходят все. Арбитр прогнал корпус целиком и предъявил счёт: честный кандидат той
//! конструкции давал **209 красных из 329** в двух крейтах и отвергал СОБСТВЕННУЮ боевую
//! настройку прода (`docker-compose.yml:136` — `GATEWAY_BANDS:-0.001`, `:217` —
//! `--bands=${GATEWAY_BANDS:-0.001}`). Merge означал бы либо падение прода, либо включение
//! семи полос до закрытия предусловий `П-014`.
//!
//! Развязка `A-033` D-2: библиотека судит КОРРЕКТНОСТЬ набора, а СОСТАВ — дело продуктовых
//! входов. Этот файл — один из двух таких входов (второй, argv чекпоинтера, —
//! `crates/gateway/tests/red_fixed_bands_checkpoint_entrance.rs`).
//!
//! ## ПЕРЕХОДНЫЙ РЕЖИМ И ЕГО КОНЕЦ (`A-033` D-3, `CT-RFC-09` §2.5)
//!
//! Переменная НЕ удаляется этим милестоуном: её конец привязан к включению полос в `M-70`.
//! Но молчаливое переопределение запрещено — режим, о котором оператор не знает, неотличим
//! от дефекта. Поэтому пара утверждений ПАРНАЯ: значение применено И названо в логе.
//!
//! ## `testing.md` чек-лист
//! - п.3 **отсутствие** — переменной НЕТ: это и есть новая норма, сервер берёт свой набор;
//! - позитивный контроль в обе стороны: реализация «всегда семь, переопределение игнорирую»
//!   роняет `override_is_applied`, реализация «молча беру env» роняет `default_is_the_product_set`;
//! - анти-TD-166: в логе ищутся ТОКЕНЫ (`GATEWAY_BANDS`, `deprecated`) и ТОЧНОЕ значение, а
//!   не голая подстрока-число — цифра совпала бы с куском таймстампа.

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

/// Продуктовый набор ЛИТЕРАЛОМ (`A-033`: ссылка на ещё не существующий символ сделала бы
/// файл несобираемым и заглушила бы весь корпус крейта — ровно так и не заметили 209 красных).
const PRODUCT_BANDS: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];

/// Сегодняшняя боевая настройка прода (`docker-compose.yml:136`). После M-84 она —
/// ПЕРЕОПРЕДЕЛЕНИЕ переходного периода, а не источник истины.
const LEGACY_ENV_VALUE: &str = "0.001";

const BASE: &[(&str, &str)] = &[
    ("GATEWAY_JWT_SECRET", "test-secret"),
    ("GATEWAY_TIMEFRAME_MS", "1000"),
];

fn getter(pairs: Vec<(String, String)>) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs.into_iter().collect();
    move |k| map.get(k).cloned()
}

fn env_with(extra: &[(&str, &str)]) -> Vec<(String, String)> {
    BASE.iter()
        .chain(extra.iter())
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Captured {
    type Writer = Captured;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Прогон под ЛОКАЛЬНЫМ подписчиком: `with_default` не трогает глобальный и не зависит от
/// соседей по тестовому бинарю (прецедент — `red_max_subs_config.rs:226`).
fn bands_and_log(extra: &[(&str, &str)]) -> (Vec<f64>, String) {
    let cap = Captured::default();
    let sub = tracing_subscriber::fmt()
        .with_writer(cap.clone())
        .with_max_level(tracing::Level::WARN)
        .with_ansi(false)
        .finish();
    let env = env_with(extra);
    let mut bands = Vec::new();
    tracing::subscriber::with_default(sub, || {
        let cfg = serve_config_from_env(getter(env)).expect(
            "SETUP НЕ СОСТОЯЛСЯ: минимальное окружение обязано давать валидный конфиг — \
             иначе тест судит разбор окружения, а не набор полос",
        );
        bands = cfg.selector.bands.clone();
    });
    let buf = cap.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
    (bands, String::from_utf8_lossy(&buf).into_owned())
}

#[test]
fn default_is_the_product_set() {
    // ЯДРО. Переменной нет ⇒ сетку задаёт СЕРВЕР. Сегодня здесь `[0.001]` — дефолт живёт в
    // `serve_config_from_env`, и это ровно то, что решение П-029 отменяет.
    let (bands, _) = bands_and_log(&[]);
    assert_eq!(
        bands,
        PRODUCT_BANDS.to_vec(),
        "без GATEWAY_BANDS сервер обязан взять СВОЙ продуктовый набор из семи полос. \
         Получено {bands:?}. Пока дефолт здесь — одна полоса, сетку де-факто задаёт оператор, \
         а решение П-029 говорит, что её задаёт сервер"
    );
}

#[test]
fn default_set_is_sorted_and_within_source_coverage() {
    // Свойства набора судятся ТАМ, ГДЕ ОН НАБЛЮДАЕМ, а не сравнением литерала с собой
    // (гвард состава из библиотеки убран по `A-033` D-2).
    let (bands, _) = bands_and_log(&[]);
    assert_eq!(
        bands.len(),
        7,
        "набор обязан нести РОВНО семь полос: {bands:?}"
    );
    for w in bands.windows(2) {
        assert!(
            w[0] < w[1],
            "набор обязан строго возрастать: {} не меньше {}. Порядок входит в отпечаток и \
             определяет раскладку строк (band, side)",
            w[0],
            w[1]
        );
    }
    for b in &bands {
        assert!(
            *b > 0.0 && *b <= 0.60,
            "полоса {b} вне охвата источника (0, 0.60]: MAX_REL_DIST=0.60 режет эмиссию, и \
             такая полоса молча отдавала бы заниженное число под меткой достоверности"
        );
    }
}

#[test]
fn override_is_applied() {
    // ПАРНЫЙ vantage к следующему тесту и к переходному режиму: переопределение обязано
    // РАБОТАТЬ, иначе выкатка M-84 меняет вычисляемый набор в момент merge'а — то есть
    // включает семь полос до закрытия предусловий П-014.
    let (bands, _) = bands_and_log(&[("GATEWAY_BANDS", LEGACY_ENV_VALUE)]);
    assert_eq!(
        bands,
        vec![0.001],
        "явное переопределение оператора обязано применяться на переходный период. \
         Получено {bands:?}. Игнорировать его = сменить набор прода молча, merge'ем"
    );
}

#[test]
fn override_is_announced_as_deprecated() {
    // Молчаливый переходный режим неотличим от дефекта: оператор не узнает, что работает не
    // на продуктовом наборе. Ищем ТОКЕНЫ и ТОЧНОЕ значение — не голое число (TD-166: подстрока
    // из цифр совпадает с куском таймстампа, и красный исход становится вероятностным).
    let (_, log) = bands_and_log(&[("GATEWAY_BANDS", LEGACY_ENV_VALUE)]);
    assert!(
        log.contains("GATEWAY_BANDS"),
        "предупреждение обязано НАЗВАТЬ переменную — иначе оператор не знает, что отключать \
         при завершении переходного периода. Лог: {log:?}"
    );
    assert!(
        log.contains("deprecated"),
        "переопределение обязано быть помечено deprecated: у режима есть КОНЕЦ (задача 7 \
         M-70), и он объявляется, а не подразумевается. Лог: {log:?}"
    );
    assert!(
        log.contains(LEGACY_ENV_VALUE),
        "предупреждение обязано назвать ПРИМЕНЁННОЕ значение: «переопределение активно» без \
         значения не даёт оператору проверить, что именно работает. Лог: {log:?}"
    );
}

#[test]
fn absent_override_is_silent() {
    // Обратная сторона: предупреждать, когда переопределения НЕТ, — значит приучить
    // оператора игнорировать предупреждения. Штатный режим молчит.
    let (_, log) = bands_and_log(&[]);
    assert!(
        !log.contains("GATEWAY_BANDS"),
        "без переопределения сервер обязан молчать о нём: шум в штатном режиме обесценивает \
         предупреждение переходного. Лог: {log:?}"
    );
}
