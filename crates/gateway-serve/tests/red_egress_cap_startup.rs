//! RED `PL-I-5` D (sacred, architect-only) — **НЕВАЛИДНЫЙ ПРЕДЕЛ ОБЪЁМА ОТВЕТА НЕ ДАЁТ
//! ПРОД-БИНАРЮ СТАРТОВАТЬ.**
//!
//! Милестоун `milestones/M-71-egress-cap.md`, задача 4. Парный библиотечный оракул (предел
//! действует и мимо транспорта) — `crates/gateway/tests/red_egress_cap.rs`.
//!
//! ## Почему отказ обязан быть на СТАРТЕ
//!
//! Дословный прецедент — `M-69`/`GW-I-14` для `GATEWAY_WINDOW_MS`
//! (`crates/gateway-serve/tests/red_window_guard_startup.rs`): healthcheck прода — TCP-проба
//! порта (`docker-compose.yml:160`). Контейнер с испорченным пределом стартует, отвечает на
//! порт и рапортует `(healthy)`; деплой-гейт §8 видит зелёное, а сервер обслуживает запросы
//! без всякого предела. Конфиг, делающий сервис небезопасным, обязан не дать ему стартовать.
//!
//! `PL-I-5` (`docs/DESIGN.md` §22) дословно: «отсутствие/**невалидность** лимита = отказ, не
//! unbounded (урок R7)». Это ВТОРОЙ экземпляр того же класса после `GW-I-14`; первый чинили
//! милестоуном, и форма отказа здесь воспроизводится один в один — оператор не должен учить
//! два разных поведения для двух лимитов одного сервиса.
//!
//! ## Худший вход — ИНВЕРСИЯ НАМЕРЕНИЯ
//!
//! `"999999999999999999999"` — оператор хочет предел ПОБОЛЬШЕ. Парс переполняется, `.ok()`
//! глотает ошибку, предела не остаётся ВООБЩЕ. Намерение «больше» исполняется как «без
//! границ». Этот кейс центральный, как и в `GW-I-14`.
//!
//! ## Парный vantage (`testing.md` п.7)
//!
//! `valid_limits_start` и `absent_limit_starts_with_default` валят переширокую заглушку
//! «всегда `Err`». Без них набор был бы зелен против реализации, не дающей стартовать никогда.
//!
//! RUNTIME-RED: против сегодняшнего кода падают все `*_blocks_startup` — переменная не
//! читается вовсе, конфиг собирается, старт разрешён.

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;

/// Имя ручки зафиксировано спекой (`M-71` §5): единообразно с `GATEWAY_WINDOW_MS` и
/// `GATEWAY_MAX_SUBSCRIPTIONS` — тот же префикс, та же форма отказа.
const VAR: &str = "GATEWAY_MAX_RESPONSE_BYTES";

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

fn cfg_with(limit: Option<&'static str>) -> Result<gateway_serve::server::ServeConfig, String> {
    let mut pairs: Vec<(&'static str, &'static str)> = vec![("GATEWAY_JWT_SECRET", "test-secret")];
    if let Some(v) = limit {
        pairs.push((VAR, v));
    }
    serve_config_from_env(getter(&pairs))
}

/// Единственная точка правды о требуемой форме отказа — engine-dev реализует ровно это.
fn assert_startup_rejected(value: &'static str, why: &str) {
    match cfg_with(Some(value)) {
        Err(msg) => assert!(
            msg.contains(VAR),
            "отказ обязан НАЗЫВАТЬ переменную {VAR} — оператор должен понять, что чинить, без \
             чтения исходников (прецедент GW-I-14). Получено: {msg:?}"
        ),
        Ok(_) => panic!(
            "PL-I-5 НАРУШЕН на СТАРТЕ: {VAR}={value:?} — {why}, но gateway-serve собрал конфиг \
             и стартовал бы healthy БЕЗ предела объёма ответа. DESIGN §22: «невалидность \
             лимита = отказ, не unbounded». Замер: один запрос bands=[0.99] даёт ×600 к \
             прод-дефолту, а подписок на соединение разрешено 16."
        ),
    }
}

// ── Отказ ────────────────────────────────────────────────────────────────────────────────

#[test]
fn garbage_limit_blocks_startup() {
    assert_startup_rejected("abc", "не число вовсе");
}

#[test]
fn overflowing_limit_blocks_startup() {
    // ИНВЕРСИЯ НАМЕРЕНИЯ: оператор просит предел побольше, получает его отсутствие.
    assert_startup_rejected("999999999999999999999", "переполнение при разборе");
}

#[test]
fn zero_limit_blocks_startup() {
    // Ноль — не «без предела» и не «запретить всё»: это конфиг, при котором сервис не может
    // отдать ни одного ответа. Такой конфиг обязан быть отвергнут, а не исполнен буквально.
    assert_startup_rejected("0", "предел 0 делает сервис неработоспособным");
}

#[test]
fn negative_limit_blocks_startup() {
    assert_startup_rejected("-1", "отрицательный предел бессмыслен");
}

#[test]
fn limit_with_unit_suffix_blocks_startup() {
    assert_startup_rejected("2000000bytes", "число с суффиксом");
}

#[test]
fn limit_with_rust_separator_blocks_startup() {
    // Читается человеком как валидное, но `from_str` его не принимает — та же ловушка, что
    // поймал GW-I-14.
    assert_startup_rejected("2_000_000", "Rust-разделитель разрядов не парсится");
}

#[test]
fn float_limit_blocks_startup() {
    assert_startup_rejected("2000000.0", "дробное значение не парсится");
}

#[test]
fn empty_limit_blocks_startup() {
    // Отличается от ОТСУТСТВИЯ переменной (см. позитивный контроль ниже): пустое значение —
    // это заданная переменная с невалидным содержимым, `PL-I-5` про неё говорит прямо.
    assert_startup_rejected("", "переменная задана пустой");
}

// ── Парный vantage: честная работа не ломается ───────────────────────────────────────────

#[test]
fn valid_limits_start() {
    for v in ["1", "2000000", "100000000"] {
        let pairs: Vec<(&'static str, &'static str)> = vec![
            ("GATEWAY_JWT_SECRET", "test-secret"),
            (VAR, Box::leak(v.to_string().into_boxed_str())),
        ];
        assert!(
            serve_config_from_env(getter(&pairs)).is_ok(),
            "PL-I-5: валидный предел {v:?} обязан давать старт. Заглушка «всегда Err» ловится \
             ровно здесь — без этого оракула набор был бы зелен против сервиса, который не \
             стартует никогда."
        );
    }
}

#[test]
fn absent_limit_starts_with_default() {
    // ОТСУТСТВИЕ переменной — легитимно: предел берётся из подписанного дефолта, как у
    // `GATEWAY_MAX_SUBSCRIPTIONS` (`crates/gateway-serve/src/lib.rs:191`). Реализация не
    // додумывает за оператора и не превращает «не задано» в ошибку — но и не превращает его
    // в «без предела» (это проверяет библиотечный оракул A в crates/gateway).
    assert!(
        cfg_with(None).is_ok(),
        "PL-I-5: отсутствие {VAR} обязано давать старт с ДЕФОЛТНЫМ пределом, а не отказ. \
         Требовать переменную обязательной значило бы ломать каждый существующий деплой."
    );
}
