//! RED M-37 task #7 (sacred, architect-only) — GATEWAY_WINDOW_MS доходит до прод-пути gateway-serve.
//!
//! TD-039/TD-020 (reviewer-находка): reducer bounded-window (задачи 2-4) РЕАЛИЗОВАН, но НЕДОСТИЖИМ
//! из бинаря — `build_selector` не принимал окно, `main.rs` инлайнил env-чтение и НЕ читал
//! `GATEWAY_WINDOW_MS` → прод-`Selector.window_ms == None` → gateway-serve по-прежнему unbounded →
//! TD-039 OOM НЕ исправлен. Класс «механизм есть, никто не зовёт» (TD-019/TD-020).
//!
//! Анти-инерт (урок TD-020): сборка конфига ДОЛЖНА быть тестируемой чистой функцией с ИНЖЕКТИРУЕМЫМ
//! getter'ом (не std::env напрямую), а `main` — тонкий вызыватель. Тогда «env→Selector.window_ms»
//! доказуем юнит-тестом, а не только §8-глазами.
//!
//! COMPILE-RED: `serve_config_from_env` и `Selector.window_ms` ещё нет (task #7 + задача #1).
//! Анти-плацебо: инлайн-main без чтения окна / build_selector, игнорирующий арг → window_ms==None
//! → assert'ы падают.

use contracts::Venue;
use gateway_serve::{build_selector, serve_config_from_env};
use std::collections::HashMap;

/// Инжектируемый getter env: детерминированный, без глобального std::env (не флейкает).
/// Замыкание ВЛАДЕЕТ `map` (пары `&'static str`), `pairs` не заимствуется → без lifetime.
fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

#[test]
fn window_ms_env_flows_to_selector() {
    // GATEWAY_WINDOW_MS выставлен → прод-Selector обязан нести окно (иначе прод unbounded).
    let cfg = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        // `CT-RFC-09` §2.6: `max_subscriptions_per_connection` — конфиг, ОТСУТСТВИЕ
        // либо невалидное значение ⇒ отказ старта (задача 13 N-2). Прод всегда его
        // подаёт (`docker-compose.yml`, дефолт 16), поэтому фикстура БЕЗ переменной
        // никогда не была прод-формой (`testing.md` §«Форма прода снимается ЗАМЕРОМ»).
        // Ассерты ниже про лимит ничего не утверждают — добавление их не ослабляет.
        ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
        ("GATEWAY_WINDOW_MS", "60000"),
    ]))
    .expect("config собран");
    assert_eq!(
        cfg.selector.window_ms,
        Some(60_000),
        "GATEWAY_WINDOW_MS НЕ дошёл до Selector.window_ms — прод-путь gateway-serve unbounded \
         (TD-039/TD-020: env-переменная инертна)"
    );
}

#[test]
fn window_ms_absent_defaults_none() {
    // Без GATEWAY_WINDOW_MS → offline unbounded (None), прежнее поведение сохранено.
    let cfg = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        // `CT-RFC-09` §2.6: `max_subscriptions_per_connection` — конфиг, ОТСУТСТВИЕ
        // либо невалидное значение ⇒ отказ старта (задача 13 N-2). Прод всегда его
        // подаёт (`docker-compose.yml`, дефолт 16), поэтому фикстура БЕЗ переменной
        // никогда не была прод-формой (`testing.md` §«Форма прода снимается ЗАМЕРОМ»).
        // Ассерты ниже про лимит ничего не утверждают — добавление их не ослабляет.
        ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
    ]))
    .expect("config собран");
    assert_eq!(
        cfg.selector.window_ms, None,
        "без GATEWAY_WINDOW_MS Selector.window_ms обязан быть None (offline)"
    );
}

#[test]
fn build_selector_propagates_window() {
    let s = build_selector(
        Venue::Binance,
        "BTCUSDT".to_string(),
        1_000,
        vec![0.001],
        Some(60_000),
    );
    assert_eq!(
        s.window_ms,
        Some(60_000),
        "build_selector обязан пробросить window_ms в Selector"
    );
}
