//! OPS-I-9 — rate-budget/backoff для recon REST. Recon НЕ создаёт rate-limit-инцидентов.
//!
//! Прямой урок TD-013: futures-адаптер попал в hot-loop REST-ресинка → 133×418 за 25с (IP-бан
//! Binance, коллатеральный риск для спот-сбора). Recon добавляет РОВНО тот REST-трафик, что нас
//! банил, поэтому rate-budget здесь — не «здравый смысл», а проверяемый инвариант.
//!
//! Политика чистая и детерминированная (часы — снаружи, как в `Backoff` M-06): honor
//! 418/429/`Retry-After`, exp backoff с cap, ОБЩИЙ бюджет запросов на venue, запрет ресинк-штормов.

use std::time::Duration;

/// Базовая задержка ретрая (первый бэкофф после ошибки). НИКОГДА не 0 (анти-hot-loop).
pub const RECON_BASE_DELAY: Duration = Duration::from_millis(100);
/// Потолок бэкоффа.
pub const RECON_MAX_DELAY: Duration = Duration::from_secs(300);

/// Ответ REST recon-запроса, влияющий на бюджет/бэкофф.
#[derive(Debug, Clone, Copy)]
pub enum RestOutcome {
    /// Успех — `reset()` бэкоффа.
    Ok,
    /// 418/429 — rate-limit; honor `retry_after` (если биржа прислала), иначе cooldown по коду.
    RateLimited { retry_after: Option<Duration> },
    /// Прочая ошибка (сеть/таймаут) — exp backoff.
    Error,
}

/// Бюджет recon-запросов на venue + бэкофф. Единственный владелец частоты recon REST на площадку.
pub struct ReconBudget {
    _priv: (),
}

impl ReconBudget {
    /// `max_per_min` — жёсткий потолок recon REST-запросов на venue за скользящую минуту.
    pub fn new(_max_per_min: u32) -> Self {
        todo!("OPS-I-9: инициализировать бюджет + бэкофф (BASE, cap MAX)")
    }

    /// Следующая задержка после ответа. Контракт (анти-hot-loop, TD-013):
    ///
    /// - `Ok` → `reset()`, задержка = обычный интервал опроса;
    /// - `RateLimited{retry_after}` → `≥ max(cooldown_по_коду, retry_after)`, НИКОГДА не 0;
    /// - `Error` → exp backoff (×2), `≥ RECON_BASE_DELAY`, cap `RECON_MAX_DELAY`.
    ///
    /// Возврат ПОСЛЕ ошибки/rate-limit СТРОГО > 0 — иначе hot-loop (наивная немедленная-retry
    /// реализация обязана ВАЛИТЬ оракул OPS-I-9).
    pub fn next_delay(&mut self, _outcome: RestOutcome) -> Duration {
        todo!("OPS-I-9: honor Retry-After/cooldown; exp backoff; НИКОГДА 0 после ошибки")
    }

    /// Можно ли сделать recon-запрос в момент `now` без превышения `max_per_min`.
    pub fn may_request(&self, _now: Duration) -> bool {
        todo!("OPS-I-9: скользящее окно 60с < max_per_min")
    }

    /// Зафиксировать выполненный запрос в момент `now` (для учёта бюджета).
    pub fn on_request(&mut self, _now: Duration) {
        todo!("OPS-I-9: записать timestamp запроса")
    }
}
