//! M-06 RED (sacred, architect) — TD-013: resync-путь snapshot'а обязан BACKOFF'ить при
//! повторных ошибках/rate-limit (418/429), а НЕ немедленно ре-пушить `make_snapshot_future`
//! (hot-loop → hammering Binance → IP-ban; §8 eyes-on поймал это в проде, #4 откачен).
//!
//! Прод-масштаб дисциплина (.claude/rules/testing.md): оракул моделирует RATE-LIMIT-ответ
//! (Retry-After/cooldown), не happy-path парсинг. Падает на текущем immediate-retry:
//! чистой политики `Backoff` НЕТ (compile-RED) → venue-dev реализует + wire'ит в resync
//! (lib.rs snapshot-fail/stale ветки), honor'я задержку перед re-push.
//!
//! Джиттер применяет async-вызывающий (I/O-boundary); ЭТА политика детерминирована (тестируема).

use std::time::Duration;

use venue_binance_futures::Backoff;

#[test]
fn td013_snapshot_retry_backs_off_not_hotloop() {
    let mut b = Backoff::new();

    // (1) Первая неудача обязана ЖДАТЬ (не hot-loop). Текущий resync re-push'ит немедленно (0) → RED.
    let d1 = b.next_delay(None);
    assert!(
        d1 >= Duration::from_millis(100),
        "первый ретрай обязан ждать (не hot-loop), got {d1:?}"
    );

    // (2) Экспоненциальный рост при повторных неудачах.
    let d2 = b.next_delay(None);
    let d3 = b.next_delay(None);
    assert!(
        d2 > d1 && d3 > d2,
        "backoff обязан расти (exp): {d1:?} < {d2:?} < {d3:?}"
    );

    // (3) Ограничен сверху (cap) — не растёт в бесконечность.
    for _ in 0..40 {
        b.next_delay(None);
    }
    let capped = b.next_delay(None);
    assert!(
        capped <= Duration::from_secs(300),
        "backoff обязан иметь cap, got {capped:?}"
    );

    // (4) Honor Retry-After / cooldown из 418/429 (в т.ч. на INITIAL-connect после IP-ban).
    let mut b2 = Backoff::new();
    let d = b2.next_delay(Some(Duration::from_secs(30)));
    assert!(
        d >= Duration::from_secs(30),
        "418/429 Retry-After обязан honored'иться: delay >= cooldown, got {d:?}"
    );

    // (5) Успешный снапшот сбрасывает backoff к базовому.
    b.reset();
    let after_reset = b.next_delay(None);
    assert_eq!(
        after_reset, d1,
        "reset() после success → задержка обратно к базовой ({d1:?}), got {after_reset:?}"
    );
}
