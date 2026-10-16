//! RED задачи 13 (sacred, architect-only) — **`CT-RFC-09` §2.8: grace-окно есть КОНФИГ.**
//!
//! Предмет: `R-086` N-3, решение — `milestones/M-65-ws-session.md` §11. Суть и основания
//! только там.
//!
//! ЧТО ПИННИТ. §2.8 дословно: «сервер не отправляет ничего, пока не истечёт
//! `initial_subscribe_grace_ms` (**конфиг**, дефолт 250 ms) или не придёт первое клиентское
//! сообщение — что раньше». Реализация держит `const GRACE_MS: u64 = 250` внутри функции:
//! значение не читается ни из окружения, ни из `docker-compose.yml`. Оператор не может ни
//! увеличить окно на медленном канале, ни сжать его на стенде.
//!
//! ПОЧЕМУ ОРАКУЛ ПРОВЕРЯЕТ ОТКАЗ, А НЕ ЗНАЧЕНИЕ. Проверка «конфиг применился» потребовала бы
//! нового поля в `ServeConfig` — то есть тест не скомпилировался бы до правки прод-типа, и
//! `cargo test --all` падал бы сломанным билдом всего workspace'а. Это не RED, а поломка
//! сборки (`M-67` §10 фиксирует ровно этот класс). Поэтому пиннится наблюдаемое НА СТАРТЕ
//! свойство: раз значение стало конфигом, невалидное значение обязано отказать старту —
//! тем же fail-closed правилом, что и остальные конфиги гейта (`gates.md`: «parse-error →
//! unbounded запрещено»). Форму поля задаёт dev в задаче 13; оракул на само значение
//! добавляется после появления поля.
//!
//! RUNTIME-RED СЕЙЧАС: переменная не читается вовсе, поэтому любое значение — включая мусор —
//! принимается молча. `invalid_grace_is_rejected` обязан падать против текущего кода.
//!
//! ПАРНЫЙ VANTAGE: `valid_grace_starts` и `absent_grace_starts` не дают удовлетворить
//! требование, сломав запуск: дефолт 250 остаётся законным, отсутствие переменной — тоже
//! (в отличие от `GATEWAY_MAX_SUBSCRIPTIONS`, где §2.6 ТРЕБУЕТ отказа; §2.8 такого не
//! требует и прямо называет дефолт).

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

const BASE: &[(&str, &str)] = &[
    ("GATEWAY_JWT_SECRET", "test-secret"),
    ("GATEWAY_TIMEFRAME_MS", "1000"),
    ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
];

fn base_plus(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = BASE.to_vec();
    v.extend_from_slice(extra);
    v
}

/// ЯДРО RED: значение стало конфигом ⇒ мусор отказывает старту, а не игнорируется.
#[test]
fn invalid_grace_is_rejected() {
    for bad in ["0", "-1", "abc", " "] {
        let env = base_plus(&[("GATEWAY_INITIAL_SUBSCRIBE_GRACE_MS", bad)]);
        match serve_config_from_env(getter(&env)) {
            Err(msg) => assert!(
                msg.contains("GATEWAY_INITIAL_SUBSCRIBE_GRACE_MS"),
                "отказ обязан НАЗЫВАТЬ переменную. Значение {bad:?}, отказ: {msg}"
            ),
            Ok(_) => panic!(
                "GATEWAY_INITIAL_SUBSCRIBE_GRACE_MS={bad:?} принято молча — значит переменная \
                 не читается вовсе (CT-RFC-09 §2.8 объявляет grace-окно КОНФИГОМ, а не \
                 константой в теле функции)"
            ),
        }
    }
}

/// ПАРНЫЙ VANTAGE 1: валидное значение стартует — гвард не переширокий.
#[test]
fn valid_grace_starts() {
    for good in ["1", "250", "5000"] {
        let env = base_plus(&[("GATEWAY_INITIAL_SUBSCRIBE_GRACE_MS", good)]);
        if let Err(msg) = serve_config_from_env(getter(&env)) {
            panic!("валидное grace-окно {good:?} обязано стартовать. Отказ: {msg}");
        }
    }
}

/// ПАРНЫЙ VANTAGE 2: отсутствие переменной ЗАКОННО — §2.8 прямо называет дефолт 250 ms.
/// Отличие от `GATEWAY_MAX_SUBSCRIPTIONS` намеренное: там §2.6 требует отказа старта, здесь
/// норматив требует дефолта. Оракул обязан различать два норматива, а не применять один.
#[test]
fn absent_grace_starts() {
    if let Err(msg) = serve_config_from_env(getter(&base_plus(&[]))) {
        panic!(
            "отсутствие GATEWAY_INITIAL_SUBSCRIBE_GRACE_MS обязано быть законным — §2.8 задаёт \
             дефолт 250 ms. Отказ: {msg}"
        );
    }
}
