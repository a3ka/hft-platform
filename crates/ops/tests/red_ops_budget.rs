//! RED OPS-I-9 (sacred, architect-only) — recon НЕ создаёт rate-limit-инцидентов.
//!
//! Прямой урок TD-013: hot-loop REST-ресинка = 133×418 за 25с, IP-бан Binance. Recon добавляет
//! РОВНО тот трафик. Анти-плацебо (главное): НАИВНАЯ немедленная-retry реализация
//! (`next_delay` всегда 0) обязана ВАЛИТЬ первый же тест. Против `todo!()`-скелета — все падают.
//! Чек-лист testing.md: множественность (поток ошибок), асимметрия (418 с/без Retry-After),
//! границы (cap), отсутствие (Ok сбрасывает).

use std::time::Duration;

use ops::budget::{ReconBudget, RestOutcome, RECON_BASE_DELAY, RECON_MAX_DELAY};

/// (АНТИ-HOT-LOOP, главный) После ЛЮБОЙ ошибки задержка СТРОГО > 0 и растёт (exp), cap = MAX.
/// Наивная «retry сразу» (delay=0) валит этот тест — ровно TD-013.
#[test]
fn ops_i_9_error_backoff_never_zero_and_grows() {
    let mut b = ReconBudget::new(20);
    let d1 = b.next_delay(RestOutcome::Error);
    assert!(
        d1 >= RECON_BASE_DELAY,
        "первая задержка после ошибки {d1:?} < BASE {RECON_BASE_DELAY:?} — hot-loop (TD-013)"
    );
    let d2 = b.next_delay(RestOutcome::Error);
    let d3 = b.next_delay(RestOutcome::Error);
    assert!(
        d2 > d1 && d3 > d2,
        "бэкофф не растёт ({d1:?} → {d2:?} → {d3:?}) — поток ошибок держит частоту у бана"
    );
    // Cap: много ошибок подряд не превышают MAX.
    let mut last = d3;
    for _ in 0..20 {
        last = b.next_delay(RestOutcome::Error);
    }
    assert!(
        last <= RECON_MAX_DELAY,
        "бэкофф превысил cap {RECON_MAX_DELAY:?}: {last:?}"
    );
}

/// (АСИММЕТРИЯ) 418/429 с `Retry-After` — задержка ≥ него (honor биржи, TD-013).
#[test]
fn ops_i_9_rate_limited_honors_retry_after() {
    let mut b = ReconBudget::new(20);
    let ra = Duration::from_secs(120);
    let d = b.next_delay(RestOutcome::RateLimited {
        retry_after: Some(ra),
    });
    assert!(
        d >= ra,
        "Retry-After={ra:?} не соблюдён (задержка {d:?}) — продолжение запросов во время бана \
         сбрасывает его таймер и само-поддерживает бан"
    );
}

/// (АСИММЕТРИЯ) 418/429 БЕЗ `Retry-After` — задержка всё равно > 0 (cooldown по коду).
#[test]
fn ops_i_9_rate_limited_without_retry_after_still_cools_down() {
    let mut b = ReconBudget::new(20);
    let d = b.next_delay(RestOutcome::RateLimited { retry_after: None });
    assert!(
        d >= RECON_BASE_DELAY,
        "rate-limit без Retry-After дал задержку {d:?} < BASE — нельзя долбить сразу"
    );
}

/// (ОТСУТСТВИЕ ошибки) `Ok` сбрасывает бэкофф: после серии ошибок и успеха следующая ошибка
/// стартует снова с BASE, а не с накопленного максимума.
#[test]
fn ops_i_9_ok_resets_backoff() {
    let mut b = ReconBudget::new(20);
    for _ in 0..5 {
        b.next_delay(RestOutcome::Error);
    }
    b.next_delay(RestOutcome::Ok); // reset
    let after_reset = b.next_delay(RestOutcome::Error);
    assert!(
        after_reset <= RECON_BASE_DELAY * 2,
        "после Ok бэкофф не сброшен ({after_reset:?}) — recon остаётся вялым после восстановления"
    );
}

/// (ГРАНИЦА бюджета) Не более `max_per_min` recon-запросов на venue за скользящую минуту.
#[test]
fn ops_i_9_budget_caps_requests_per_minute() {
    let mut b = ReconBudget::new(3);
    let t0 = Duration::from_secs(0);
    for i in 0..3 {
        assert!(
            b.may_request(t0),
            "запрос {i} в бюджете обязан быть разрешён"
        );
        b.on_request(t0);
    }
    assert!(
        !b.may_request(t0),
        "4-й запрос за минуту разрешён при бюджете 3 — recon превышает лимит venue (TD-013)"
    );
    // Через минуту окно сдвигается — снова можно.
    assert!(
        b.may_request(Duration::from_secs(61)),
        "после минуты бюджет обязан восстановиться"
    );
}
