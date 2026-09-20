//! RED M-69 (sacred, architect-only) — **GW-I-14: невалидный `GATEWAY_WINDOW_MS` не даёт
//! прод-бинарю стартовать.**
//!
//! ## Дефект
//!
//! `GATEWAY_WINDOW_MS` — единственная ручка, ограничивающая память свёртки live-кокпита.
//! Сегодня её невалидное значение молча даёт БЕЗГРАНИЧНОЕ окно
//! (`crates/gateway-serve/src/lib.rs:740-744`):
//!
//! ```ignore
//! Some(s) => s.trim().parse::<i64>().ok(),   // parse-error → None → unbounded
//! ```
//!
//! Это прямо противоречит `PL-I-5` (`docs/DESIGN.md:940`): «отсутствие/невалидность лимита =
//! отказ, не unbounded (урок R7)» — и названо в `docs/08-arch-improvement-roadmap.md:35`
//! (риск R7, CRIT) как «единственное отступление от fail-closed», с уже назначенным фиксом
//! «(а) невалидный env → `Err` при старте». Режим unbounded — тот самый, что разваливал прод
//! (TD-020 / TD-039).
//!
//! ## Почему отказ обязан быть на СТАРТЕ (урок M-47 / TD-019 / TD-020)
//!
//! Healthcheck прода — TCP-проба порта (`docker-compose.yml:160`). Контейнер с испорченным
//! окном стартует, отвечает на порт и рапортует `(healthy)`; §8 eyes-on видит зелёное, а
//! свёртка растёт неограниченно. Конфиг, делающий сервис нерабочим, обязан не дать ему
//! стартовать — ровно та формулировка, которой M-47 обосновал гвард `GW-I-10` в этом же файле.
//!
//! ## Худший вход — ИНВЕРСИЯ НАМЕРЕНИЯ, а не просто опечатка
//!
//! `"99999999999999999999"` — оператор хочет окно ПОБОЛЬШЕ. `parse::<i64>` переполняется,
//! `.ok()` глотает ошибку, и окна не остаётся ВООБЩЕ. Намерение «больше» исполняется как
//! «без границ». Этот кейс — центральный в наборе.
//!
//! ## Что здесь НЕ проверяется (граница предмета)
//!
//! Оконная арифметика (`Selector::window_lo_time_s`) — предмет M-37/`VB-I-10`, здесь чинится
//! ВХОД, а не поведение окна. Парный библиотечный оракул (анти-байпас для чекпоинтера M-38b /
//! shared-tailer M-39 / research-cli) — `crates/gateway/tests/red_window_selector_guard.rs`.
//!
//! ## testing.md чек-лист
//! - п.3 **отсутствие** — `unset` / `""` / пробелы остаются легитимным offline; реализация не
//!   додумывает за оператора и не превращает «не задано» в ошибку.
//! - п.4 **границы** — `0` (offline, паритет с argv), `1` (минимальное окно), `-1`, `-60000`,
//!   `i64::MAX`, переполнение `i64` на разряд.
//! - п.7 **ПАРНЫЙ vantage** — `offline_forms_still_start` + `valid_windows_start` валят
//!   переширокую заглушку «всегда `Err`».
//!
//! RUNTIME-RED: против сегодняшнего кода падают все `*_blocks_startup` — `.ok()` возвращает
//! `None`, конфиг собирается, старт разрешён.

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

/// Единственная точка правды о требуемой форме отказа — engine-dev реализует ровно это.
fn assert_startup_rejected(window: &'static str, why: &str) {
    let res = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        ("GATEWAY_WINDOW_MS", window),
    ]));
    match res {
        Err(msg) => assert!(
            msg.contains("GATEWAY_WINDOW_MS"),
            "отказ обязан НАЗЫВАТЬ переменную GATEWAY_WINDOW_MS (оператор должен понять, что \
             чинить, без чтения исходников), получено: {msg:?}"
        ),
        Ok(cfg) => panic!(
            "GW-I-14 НАРУШЕН на СТАРТЕ: GATEWAY_WINDOW_MS={window:?} — {why}, но gateway-serve \
             собрал конфиг и стартовал бы healthy с window_ms={:?}. PL-I-5 (DESIGN.md:940): \
             невалидность лимита = отказ, НЕ unbounded. Режим unbounded разваливал прод \
             (TD-020/TD-039).",
            cfg.selector.window_ms
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RED 1 — нечисловой мусор не даёт стартовать
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn garbage_window_blocks_startup() {
    assert_startup_rejected("abc", "не число вовсе");
}

#[test]
fn window_with_unit_suffix_blocks_startup() {
    // Правдоподобнейшая опечатка оператора: единица измерения в значении.
    assert_startup_rejected("60000ms", "число с суффиксом единицы измерения");
}

#[test]
fn window_with_rust_separator_blocks_startup() {
    // `60_000` читается человеком как валидное, но `i64::from_str` его не принимает.
    assert_startup_rejected("60_000", "Rust-разделитель разрядов не парсится i64");
}

#[test]
fn scientific_notation_window_blocks_startup() {
    assert_startup_rejected("6e4", "научная нотация не парсится i64");
}

#[test]
fn float_window_blocks_startup() {
    assert_startup_rejected("60000.0", "дробное значение не парсится i64");
}

// ─────────────────────────────────────────────────────────────────────────────
// RED 2 — ИНВЕРСИЯ НАМЕРЕНИЯ: переполнение просят «больше», получают «без границ»
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn overflowing_window_blocks_startup() {
    assert_startup_rejected(
        "99999999999999999999",
        "переполнение i64: оператор просил ОЧЕНЬ БОЛЬШОЕ окно, тихий fallback даёт ОТСУТСТВИЕ \
         окна — прямая инверсия намерения в OOM-режим",
    );
}

#[test]
fn i64_max_plus_one_blocks_startup() {
    // Ровно на разряд за границу i64 — переполнение обязано быть отказом, а не unbounded.
    assert_startup_rejected("9223372036854775808", "i64::MAX + 1, переполнение");
}

// ─────────────────────────────────────────────────────────────────────────────
// RED 3 — отрицательное окно: парсится успешно, доходит до Selector, ведёт себя как unbounded
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn negative_window_blocks_startup() {
    // Хуже мусора по диагностике: `Some(-60000)` доходит до Selector и попадает в
    // selector_fingerprint (M-38b) — чекпоинт снимается под режимом, которого не заказывали,
    // и остаётся валидным по CRC.
    assert_startup_rejected(
        "-60000",
        "отрицательное окно: window_lo_time_s возвращает None (gateway/src/lib.rs:130-133) ⇒ \
         поведение unbounded при непустом Selector.window_ms",
    );
}

#[test]
fn minus_one_window_blocks_startup() {
    assert_startup_rejected("-1", "минимальное отрицательное — граница знака");
}

// ─────────────────────────────────────────────────────────────────────────────
// ПАРНЫЙ vantage (testing.md п.7) — валит переширокую заглушку «всегда Err»
// ─────────────────────────────────────────────────────────────────────────────

/// Три эквивалентные формы «offline» обязаны остаться рабочими. `0` — не опечатка, а принятый
/// в этом коде способ сказать «offline»: `gateway-checkpoint.rs:162-163` («`0` ⇒ None offline»).
/// Отвергать их — сломать research-cli / replay-tutor / чекпоинтер, у которых окна нет по
/// построению.
#[test]
fn offline_forms_still_start() {
    for (name, pairs) in [
        (
            "переменная не задана",
            vec![("GATEWAY_JWT_SECRET", "test-secret")],
        ),
        (
            "пустая строка",
            vec![
                ("GATEWAY_JWT_SECRET", "test-secret"),
                ("GATEWAY_WINDOW_MS", ""),
            ],
        ),
        (
            "пробелы",
            vec![
                ("GATEWAY_JWT_SECRET", "test-secret"),
                ("GATEWAY_WINDOW_MS", "   "),
            ],
        ),
        (
            "явный ноль = offline",
            vec![
                ("GATEWAY_JWT_SECRET", "test-secret"),
                ("GATEWAY_WINDOW_MS", "0"),
            ],
        ),
    ] {
        let cfg = serve_config_from_env(getter(&pairs)).unwrap_or_else(|e| {
            panic!(
                "GW-I-14 ПЕРЕШИРОК: «{name}» — легитимная форма offline-режима, старт обязан \
                 состояться, но отвергнут: {e}"
            )
        });
        // КАНОНИЗАЦИЯ (C-099 B-2): недостаточно совпадения наблюдаемого поведения — три формы
        // offline обязаны давать ОДНО внутреннее представление. `Some(0)` даёт тот же
        // `window_lo_time_s == None`, но ДРУГОЙ `selector_fingerprint`
        // (crates/gateway/src/lib.rs:2268-2280) ⇒ два ключа чекпоинта для одного режима.
        // Тот же аргумент, которым отвергается отрицательное значение.
        assert_eq!(
            cfg.selector.window_ms, None,
            "«{name}» обязано канонизироваться в window_ms == None, а не в иное представление \
             того же поведения: иначе selector_fingerprint расщепляет offline-режим на два \
             ключа чекпоинта (M-38b)"
        );
        assert_eq!(
            cfg.selector.window_lo_time_s(1_000_000),
            None,
            "«{name}» обязано означать offline (unbounded): window_lo_time_s → None"
        );
    }
}

/// Валидные положительные окна обязаны стартовать и доходить до Selector без подмены.
#[test]
fn valid_windows_start() {
    for w in ["1", "60000", "3600000", "9223372036854775807"] {
        let cfg = serve_config_from_env(getter(&[
            ("GATEWAY_JWT_SECRET", "test-secret"),
            ("GATEWAY_WINDOW_MS", w),
        ]))
        .unwrap_or_else(|e| {
            panic!("GW-I-14 ПЕРЕШИРОК: GATEWAY_WINDOW_MS={w} валидно и обязано приниматься: {e}")
        });
        assert_eq!(
            cfg.selector.window_ms,
            Some(w.parse::<i64>().expect("тестовая константа")),
            "принятое окно обязано дойти до Selector без подмены"
        );
    }
}

/// Прод-дефолт (`docker-compose.yml:139`, замер на VPS: `GATEWAY_WINDOW_MS=60000`) обязан
/// остаться рабочим — гвард не имеет права уронить работающий прод.
#[test]
fn prod_window_value_still_starts() {
    let cfg = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        ("GATEWAY_WINDOW_MS", "60000"),
    ]))
    .expect("прод-значение 60000 обязано стартовать");
    assert_eq!(cfg.selector.window_ms, Some(60_000));
}

/// Окружающие пробелы уже поддержаны (`s.trim()`); гвард не должен это отменять.
#[test]
fn padded_valid_window_still_starts() {
    let cfg = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        ("GATEWAY_WINDOW_MS", "  60000  "),
    ]))
    .expect("значение с окружающими пробелами обязано приниматься (trim уже есть)");
    assert_eq!(cfg.selector.window_ms, Some(60_000));
}

/// Соседний инвариант GW-I-10 (M-47) обязан остаться нетронутым: гвард окна не имеет права
/// ни ослабить, ни подменить гвард таймфрейма.
#[test]
fn timeframe_guard_untouched_by_window_guard() {
    let res = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        ("GATEWAY_TIMEFRAME_MS", "11000"),
        ("GATEWAY_WINDOW_MS", "60000"),
    ]));
    match res {
        Err(msg) => assert!(
            msg.contains("GATEWAY_TIMEFRAME_MS"),
            "при валидном окне и невыравненном таймфрейме отказ обязан называть \
             GATEWAY_TIMEFRAME_MS (GW-I-10, M-47), получено: {msg:?}"
        ),
        Ok(_) => panic!("GW-I-10 (M-47) регрессировал: timeframe_ms=11000 принят"),
    }
}
