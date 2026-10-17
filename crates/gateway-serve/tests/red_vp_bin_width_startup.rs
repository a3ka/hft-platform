//! RED `M-86` `V5` (sacred, architect-only) — **НЕВАЛИДНАЯ ШИРИНА КОРЗИНЫ ПРОФИЛЯ НЕ ДАЁТ
//! ПРОД-БИНАРЮ СТАРТОВАТЬ; ОТСУТСТВИЕ И ПУСТОЕ — ОДНО НЕПОЛНОЕ СОСТОЯНИЕ, И ОНО ДАЁТ
//! ПОДПИСАННУЮ НОРМУ.**
//!
//! Милестоун `milestones/M-86-vp-bin-width.md`, задача 4. Парный библиотечный оракул (ручка
//! доходит до РАЗБИЕНИЯ, а не только до глобала) — `crates/gateway/tests/red_vp_bin_width_governed.rs`.
//!
//! ## Почему отказ обязан быть на СТАРТЕ, а не на первом запросе
//!
//! Дословный прецедент — `GW-I-14` (`M-69`, `GATEWAY_WINDOW_MS`) и `PL-I-5` (`M-71`,
//! `GATEWAY_MAX_RESPONSE_BYTES`). Healthcheck прода — TCP-проба порта: контейнер с
//! испорченной конфигурацией стартует, отвечает на порт и рапортует `(healthy)`, деплой-гейт
//! §8 видит зелёное. Конфиг, делающий выдачу неподъёмной, обязан не дать сервису стартовать.
//!
//! ## ИНВЕРСИЯ НАМЕРЕНИЯ — центральный вход
//!
//! `"0"` читается оператором как «без огрубления, отдавай точные цены». Исполненный
//! буквально, он даёт деление на ноль либо сетку шириной в тик — то есть ровно ту аварию,
//! против которой заведён милестоун. Намерение «точнее» исполняется как «кокпит не работает».
//!
//! ## ПОЛИТИКА — ТА ЖЕ, ЧТО У ТРЁХ СОСЕДЕЙ, и это исполнение `A-015` §3 п.1
//!
//! «Поведение прода не имеет права зависеть от того, КАК переменную забыли задать».
//! Соседи: `red_max_subs_config.rs` (`empty_var_is_same_as_absent`),
//! `red_window_guard_startup.rs` (`offline_forms_still_start`), `red_egress_cap_startup.rs`
//! (`empty_and_blank_are_same_as_absent`). Четвёртая политика для четвёртого лимита ОДНОГО
//! сервиса запрещена: оператор не обязан учить четыре разных поведения.
//!
//! ## Парный vantage (`testing.md`)
//!
//! `valid_widths_start` и `absent_width_starts_with_default` валят переширокую заглушку
//! «всегда `Err`». Без них набор был бы зелен против реализации, не дающей стартовать никогда.
//!
//! RUNTIME-RED: против сегодняшнего кода падают все `*_blocks_startup` — переменная не
//! читается вовсе, конфиг собирается, старт разрешён.

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;

/// Имя ручки зафиксировано спекой (`M-86` §2.3): тот же префикс и та же форма, что у
/// `GATEWAY_MAX_RESPONSE_BYTES` и `GATEWAY_WINDOW_MS`.
const VAR: &str = "GATEWAY_VP_BIN_WIDTH_E8";

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

fn cfg_with(width: Option<&'static str>) -> Result<gateway_serve::server::ServeConfig, String> {
    let mut pairs: Vec<(&'static str, &'static str)> = vec![("GATEWAY_JWT_SECRET", "test-secret")];
    if let Some(v) = width {
        pairs.push((VAR, v));
    }
    serve_config_from_env(getter(&pairs))
}

fn assert_startup_rejected(value: &'static str, why: &str) {
    match cfg_with(Some(value)) {
        Err(msg) => assert!(
            msg.contains(VAR),
            "отказ обязан НАЗЫВАТЬ переменную {VAR} — оператор должен понять, что чинить, \
             без чтения исходников (прецедент GW-I-14). Получено: {msg:?}"
        ),
        Ok(_) => panic!(
            "M-86 V5 НАРУШЕН на СТАРТЕ: {VAR}={value:?} — {why}, но gateway-serve собрал \
             конфиг и стартовал бы healthy. Замер 2026-09-18: без осмысленной сетки кадр \
             прода весит 5 533 287 Б против предела 2 000 000 Б и не доезжает до клиента \
             ВООБЩЕ."
        ),
    }
}

// ── Отказ ────────────────────────────────────────────────────────────────────────────────

#[test]
fn garbage_width_blocks_startup() {
    assert_startup_rejected("abc", "не число вовсе");
}

#[test]
fn zero_width_blocks_startup() {
    // ИНВЕРСИЯ НАМЕРЕНИЯ: «0» читается как «не огрублять», исполняется как деление на ноль
    // либо сетка в тик — то есть как сегодняшняя авария.
    assert_startup_rejected("0", "нулевая ширина корзины не определена");
}

#[test]
fn negative_width_blocks_startup() {
    assert_startup_rejected("-25000000", "отрицательная ширина бессмысленна");
}

#[test]
fn overflowing_width_blocks_startup() {
    assert_startup_rejected("999999999999999999999", "переполнение при разборе");
}

#[test]
fn float_width_blocks_startup() {
    assert_startup_rejected("25000000.0", "дробное значение не парсится");
}

#[test]
fn width_with_rust_separator_blocks_startup() {
    // Человек читает как валидное; `from_str` не принимает — ловушка, уже пойманная GW-I-14.
    assert_startup_rejected("25_000_000", "Rust-разделитель разрядов не парсится");
}

#[test]
fn width_with_unit_suffix_blocks_startup() {
    assert_startup_rejected("0.25usd", "число с суффиксом единицы");
}

// ── Норма (парный vantage — без него набор зелен против «всегда Err») ────────────────────

#[test]
fn absent_width_starts_with_default() {
    cfg_with(None).unwrap_or_else(|e| {
        panic!(
            "ОТСУТСТВИЕ {VAR} обязано давать ПОДПИСАННУЮ НОРМУ, а не отказ старта \
             (`A-015` §3 п.1; так живут три соседних лимита этого же сервиса). Получено: {e}"
        )
    });
}

#[test]
fn empty_and_blank_are_same_as_absent() {
    // Пиннится РАВЕНСТВО ИСХОДА трёх неполных состояний, а не «все три Ok».
    let absent = cfg_with(None).is_ok();
    let empty = cfg_with(Some("")).is_ok();
    let blank = cfg_with(Some("   ")).is_ok();
    assert!(
        absent && empty && blank,
        "НАРУШЕНО: отсутствие/пустое/пробельное обязаны давать ОДИН исход (подписанная \
         норма). Получено: absent={absent} empty={empty} blank={blank}. Разные исходы = \
         четвёртая политика для четвёртого лимита одного сервиса, запрещено `A-015` §3 п.1"
    );
}

#[test]
fn valid_widths_start() {
    for v in ["1000000", "25000000", "100000000"] {
        let leaked: &'static str = Box::leak(v.to_string().into_boxed_str());
        cfg_with(Some(leaked)).unwrap_or_else(|e| {
            panic!(
                "ЛОЖНОЕ КРАСНОЕ: честная ширина {v} отвергнута на старте ({e}). \
                 Переширокая заглушка «всегда Err» опаснее пропуска — она гасит сервис на \
                 валидной конфигурации."
            )
        });
    }
}
