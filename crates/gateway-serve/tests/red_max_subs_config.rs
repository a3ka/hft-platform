//! RED задачи 13 (sacred, architect-only) — **`CT-RFC-09` §2.6 на СТАРТЕ прод-бинаря.**
//!
//! Предмет: `R-086` N-2, решение — `milestones/M-65-ws-session.md` §11 (contract-RFC НЕ
//! требуется; правится реализация). Суть решения и отвергнутые развязки — только там.
//!
//! ЧТО ПИННИТ. `CT-RFC-09` §2.6 дословно: «`max_subscriptions_per_connection` — конфиг,
//! отсутствие/невалидное значение ⇒ **отказ старта**». Сейчас `serve_config_from_env`
//! отвергает пустую строку, `0`, отрицательное и нечисловое — но при ОТСУТСТВИИ переменной
//! молча берёт `16`. Два неполных состояния конфигурации ведут себя по-разному, и поведение
//! прода зависит от того, КАК именно переменную забыли задать.
//!
//! ПОЧЕМУ ЭТО НЕ ПРИДИРКА К ДЕФОЛТУ. Подпись founder'а 11.08 покрывает ЗНАЧЕНИЕ лимита
//! (`CT-RFC-09` §6 п.2 дословно: «Значение … — продуктовое решение»), а не право стартовать
//! без конфига. Дефолт остаётся — в `docker-compose.yml:145`, где его видит тот, кто
//! разворачивает; спрятанный в коде не видит никто. Ветка `None` живёт там, где опасна:
//! прямой запуск бинаря, локальный прогон, чужой compose — тихо выдаёт 16, когда конфиг не
//! доехал, и дефект проявляется не как поломка, а как «странно, почему потолок 16».
//!
//! RUNTIME-RED СЕЙЧАС: `crates/gateway-serve/src/lib.rs` — `None => 16_usize`.
//! `absent_is_rejected` обязан падать против текущего кода; если он зелёный с первого
//! прогона — фикстура не давит на инвариант (`testing.md` §«Анти-плацебо»).
//!
//! ПАРНЫЙ VANTAGE (`testing.md` п.7): `valid_value_starts` доказывает, что гвард не
//! переширокий — заглушка «всегда `Err`» валит его. Без этой пары «отказ старта» можно
//! удовлетворить, сломав запуск вообще.

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;

/// Окружение инъецируется замыканием: тест не трогает process env и потому не зависит от
/// соседей по бинарю (`testing.md` §«Целостность гейта», свойство 2 — мерить свой инвариант,
/// а не окружение).
fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

/// Минимально валидное окружение БЕЗ лимита — база для сценария «переменная отсутствует».
const BASE: &[(&str, &str)] = &[
    ("GATEWAY_JWT_SECRET", "test-secret"),
    ("GATEWAY_TIMEFRAME_MS", "1000"),
];

fn base_plus(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = BASE.to_vec();
    v.extend_from_slice(extra);
    v
}

fn assert_rejected(env: &[(&'static str, &'static str)], case: &str) {
    match serve_config_from_env(getter(env)) {
        Err(msg) => assert!(
            msg.contains("GATEWAY_MAX_SUBSCRIPTIONS"),
            "{case}: отказ обязан НАЗЫВАТЬ переменную — оператор чинит конфиг, не читая \
             исходники. Получено: {msg}"
        ),
        Ok(_) => panic!(
            "{case}: старт РАЗРЕШЁН при неполном конфиге лимита. CT-RFC-09 §2.6 требует отказа \
             старта; урок R7 — «parse-error → unbounded запрещено»"
        ),
    }
}

/// ЯДРО RED: отсутствие переменной = отказ старта.
#[test]
fn absent_is_rejected() {
    assert_rejected(&base_plus(&[]), "GATEWAY_MAX_SUBSCRIPTIONS отсутствует");
}

/// СИММЕТРИЯ (§11): два неполных состояния конфигурации ведут себя ОДИНАКОВО. Без этого
/// теста «пусто ⇒ ошибка, отсутствует ⇒ дефолт» остаётся законной асимметрией.
#[test]
fn empty_and_absent_behave_identically() {
    let absent = serve_config_from_env(getter(&base_plus(&[])));
    let empty = serve_config_from_env(getter(&base_plus(&[("GATEWAY_MAX_SUBSCRIPTIONS", "")])));
    assert_eq!(
        absent.is_err(),
        empty.is_err(),
        "«переменная не задана» и «задана пустой» — оба неполные состояния конфигурации; \
         поведение прода не имеет права зависеть от того, КАК её забыли задать"
    );
}

/// ГРАНИЦЫ (`testing.md` п.4 «Дегенерированный вход»). `1` — валидный минимум по §2.6
/// («целое >= 1»), `0` невалиден и отказывает наравне с мусором.
#[test]
fn invalid_values_are_rejected() {
    for bad in ["0", "-1", "abc", " ", "1.5"] {
        assert_rejected(
            &base_plus(&[("GATEWAY_MAX_SUBSCRIPTIONS", bad)]),
            &format!("GATEWAY_MAX_SUBSCRIPTIONS={bad:?}"),
        );
    }
}

/// ПАРНЫЙ VANTAGE: гвард не переширокий. Заглушка «всегда Err» валит именно этот тест.
#[test]
fn valid_value_starts() {
    for good in ["1", "16", "64"] {
        // `ServeConfig` не реализует `Debug`, и добавлять его — правка прод-типа вне зоны
        // architect'а (`scope-guard.md`). Ошибку печатаем из ветки `Err`, конфиг не форматируем.
        match serve_config_from_env(getter(&base_plus(&[("GATEWAY_MAX_SUBSCRIPTIONS", good)]))) {
            Ok(_) => {}
            Err(msg) => panic!(
                "валидный лимит {good:?} обязан стартовать — иначе «отказ старта» \
                 удовлетворяется поломкой запуска вообще. Отказ: {msg}"
            ),
        }
    }
}
