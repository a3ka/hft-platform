//! RED TD-046 (sacred, architect-only) — **GW-I-10 на СТАРТЕ прод-бинаря.**
//!
//! Парный оракул к `crates/gateway/tests/red_timeframe_session_alignment.rs`. Тот доказывает,
//! что БИБЛИОТЕКА отвергает невыравненный `timeframe_ms` (нет байпас-поверхности для любого
//! консюмера: чекпоинтер M-38b, shared-tailer M-39, research-cli). ЭТОТ доказывает, что
//! прод-бинарь падает **на входе**, а не при первом подключении клиента.
//!
//! Почему обе точки обязательны (урок TD-019/TD-020 «механизм есть, никто не зовёт», зеркально
//! `red_serve_window_wiring`): гвард только в библиотеке означает, что оператор с опечаткой в
//! `GATEWAY_TIMEFRAME_MS` поднимет ЗДОРОВЫЙ по healthcheck контейнер, который отдаёт ошибку
//! каждому клиенту — §8 eyes-on увидит «контейнер (healthy)», а кокпит будет пуст. Конфиг,
//! делающий сервис нерабочим, обязан не дать ему стартовать.
//!
//! Ожидаемая форма: `serve_config_from_env(..) -> Err(String)`, сообщение называет
//! `GATEWAY_TIMEFRAME_MS` (оператор обязан понять, ЧТО чинить, без чтения исходников).
//!
//! COMPILE/RUNTIME-RED: сейчас `serve_config_from_env` парсит `GATEWAY_TIMEFRAME_MS` только по
//! ФОРМАТУ (`crates/gateway-serve/src/lib.rs:516-519`) и принимает любое число, включая `0`
//! и `11_000`.
//!
//! testing.md п.7 ПАРНЫЙ vantage: `aligned_*_starts` доказывает, что гвард не переширокий —
//! заглушка «всегда Err» валит его. п.4 границы: `0`, невыравненный, прод-дефолт, ровно сутки.

use gateway_serve::serve_config_from_env;
use std::collections::HashMap;

fn getter(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<&'static str, &'static str> = pairs.iter().copied().collect();
    move |k| map.get(k).map(|s| s.to_string())
}

fn assert_startup_rejected(timeframe: &'static str) {
    let res = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        // `CT-RFC-09` §2.6: `max_subscriptions_per_connection` — конфиг, ОТСУТСТВИЕ
        // либо невалидное значение ⇒ отказ старта (задача 13 N-2). Прод всегда его
        // подаёт (`docker-compose.yml`, дефолт 16), поэтому фикстура БЕЗ переменной
        // никогда не была прод-формой (`testing.md` §«Форма прода снимается ЗАМЕРОМ»).
        // Ассерты ниже про лимит ничего не утверждают — добавление их не ослабляет.
        ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
        ("GATEWAY_TIMEFRAME_MS", timeframe),
    ]));
    match res {
        Err(msg) => assert!(
            msg.contains("GATEWAY_TIMEFRAME_MS"),
            "отказ обязан называть переменную GATEWAY_TIMEFRAME_MS (оператору нужно знать, что \
             чинить), получено: {msg:?}"
        ),
        Ok(cfg) => panic!(
            "GW-I-10 НАРУШЕН на СТАРТЕ: GATEWAY_TIMEFRAME_MS={timeframe} не делит 86_400_000 \
             нацело (бакет накрывает 00:00 UTC ⇒ session-anchored CVD/SVP неопределены), но \
             gateway-serve собрал конфиг и стартовал бы healthy. Selector: {:?}",
            cfg.selector
        ),
    }
}

#[test]
fn misaligned_timeframe_env_blocks_startup() {
    // Репро reviewer'а (TD-046): 11_000 мс — 86_400_000 % 11_000 = 4_000 ≠ 0.
    assert_startup_rejected("11000");
}

#[test]
fn zero_timeframe_env_blocks_startup() {
    // 0 → time-бакетные серии молча пустые, volume_profile заполнен (замер в парном оракуле).
    assert_startup_rejected("0");
}

#[test]
fn negative_timeframe_env_blocks_startup() {
    assert_startup_rejected("-1000");
}

#[test]
fn weekly_timeframe_env_blocks_startup() {
    // «Круглый», но накрывает 7 полуночей: гвард обязан проверять делимость СУТОК.
    assert_startup_rejected("604800000");
}

/// ПАРНЫЙ vantage: валидные конфиги обязаны стартовать. Валит заглушку «всегда Err».
#[test]
fn aligned_timeframes_env_starts() {
    for tf in ["1", "1000", "60000", "3600000", "86400000"] {
        let cfg = serve_config_from_env(getter(&[
            ("GATEWAY_JWT_SECRET", "test-secret"),
            // `CT-RFC-09` §2.6: `max_subscriptions_per_connection` — конфиг, ОТСУТСТВИЕ
            // либо невалидное значение ⇒ отказ старта (задача 13 N-2). Прод всегда его
            // подаёт (`docker-compose.yml`, дефолт 16), поэтому фикстура БЕЗ переменной
            // никогда не была прод-формой (`testing.md` §«Форма прода снимается ЗАМЕРОМ»).
            // Ассерты ниже про лимит ничего не утверждают — добавление их не ослабляет.
            ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
            ("GATEWAY_TIMEFRAME_MS", tf),
        ]));
        let cfg = cfg.unwrap_or_else(|e| {
            panic!(
                "GW-I-10 ПЕРЕШИРОК: GATEWAY_TIMEFRAME_MS={tf} делит 86_400_000 нацело и обязан \
                 приниматься, но старт отвергнут: {e}"
            )
        });
        assert_eq!(
            cfg.selector.timeframe_ms,
            tf.parse::<i64>().expect("тестовая константа"),
            "принятый timeframe обязан дойти до Selector без подмены"
        );
    }
}

/// Дефолт (переменная не задана) обязан остаться рабочим прод-значением 1000 —
/// гвард не должен требовать явного задания там, где раньше работал дефолт.
#[test]
fn default_timeframe_still_starts() {
    let cfg = serve_config_from_env(getter(&[
        ("GATEWAY_JWT_SECRET", "test-secret"),
        // `CT-RFC-09` §2.6: `max_subscriptions_per_connection` — конфиг, ОТСУТСТВИЕ
        // либо невалидное значение ⇒ отказ старта (задача 13 N-2). Прод всегда его
        // подаёт (`docker-compose.yml`, дефолт 16), поэтому фикстура БЕЗ переменной
        // никогда не была прод-формой (`testing.md` §«Форма прода снимается ЗАМЕРОМ»).
        // Ассерты ниже про лимит ничего не утверждают — добавление их не ослабляет.
        ("GATEWAY_MAX_SUBSCRIPTIONS", "16"),
    ]))
    .expect("дефолтный конфиг (без GATEWAY_TIMEFRAME_MS) обязан стартовать");
    assert_eq!(
        cfg.selector.timeframe_ms, 1_000,
        "прод-дефолт GATEWAY_TIMEFRAME_MS=1000 (docker-compose.yml) обязан сохраниться"
    );
}
