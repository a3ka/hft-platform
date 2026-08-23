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
///
/// Состояние:
///  - `max_per_min`: жёсткий потолок запросов за скользящую минуту;
///  - `timestamps`: кольцевой буфер последних N моментов запросов (N = max_per_min);
///  - `backoff_attempt`: счётчик неудач подряд (exp backoff); сбрасывается на `Ok`;
///  - `cooldown_until`: если был rate-limit/error — следующий запрос не раньше этого момента.
pub struct ReconBudget {
    max_per_min: u32,
    timestamps: Vec<Duration>,
    backoff_attempt: u32,
    cooldown_until: Duration,
}

impl ReconBudget {
    /// `max_per_min` — жёсткий потолок recon REST-запросов на venue за скользящую минуту.
    pub fn new(max_per_min: u32) -> Self {
        let cap = max_per_min.max(1) as usize;
        Self {
            max_per_min,
            timestamps: Vec::with_capacity(cap),
            backoff_attempt: 0,
            cooldown_until: Duration::ZERO,
        }
    }

    /// Следующая задержка после ответа. Контракт (анти-hot-loop, TD-013):
    ///
    /// - `Ok` → reset бэкоффа, задержка = `RECON_BASE_DELAY` (нормальный интервал опроса);
    /// - `RateLimited{retry_after}` → `≥ max(code_cooldown, retry_after)`, НИКОГДА не 0;
    /// - `Error` → exp backoff (×2), `≥ RECON_BASE_DELAY`, cap `RECON_MAX_DELAY`.
    ///
    /// Возврат ПОСЛЕ ошибки/rate-limit СТРОГО > 0 — иначе hot-loop (наивная немедленная-retry
    /// реализация обязана ВАЛИТЬ оракул OPS-I-9).
    pub fn next_delay(&mut self, outcome: RestOutcome) -> Duration {
        match outcome {
            RestOutcome::Ok => {
                self.backoff_attempt = 0;
                self.cooldown_until = Duration::ZERO;
                RECON_BASE_DELAY
            }
            RestOutcome::RateLimited { retry_after } => {
                // Honor биржи: минимум = max(code_cooldown, retry_after).
                let code_cooldown = exp_backoff(self.backoff_attempt);
                let next = match retry_after {
                    Some(ra) if ra > code_cooldown => ra,
                    _ => code_cooldown,
                };
                self.backoff_attempt = self.backoff_attempt.saturating_add(1);
                let clamped = next.min(RECON_MAX_DELAY).max(RECON_BASE_DELAY);
                self.cooldown_until = clamped;
                clamped
            }
            RestOutcome::Error => {
                let next = exp_backoff(self.backoff_attempt);
                self.backoff_attempt = self.backoff_attempt.saturating_add(1);
                let clamped = next.min(RECON_MAX_DELAY).max(RECON_BASE_DELAY);
                self.cooldown_until = clamped;
                clamped
            }
        }
    }

    /// Можно ли сделать recon-запрос в момент `now` без превышения `max_per_min`.
    ///
    /// Скользящее окно 60с: запрос в момент `t` учитывается для любого `now` при `now − t ≤ 60с`
    /// (включительно по границе). Также уважает `cooldown_until` от rate-limit/error.
    pub fn may_request(&self, now: Duration) -> bool {
        if now < self.cooldown_until {
            return false;
        }
        let cutoff = now.saturating_sub(Duration::from_secs(60));
        let in_window = self.timestamps.iter().filter(|&&t| t >= cutoff).count();
        (in_window as u32) < self.max_per_min
    }

    /// Зафиксировать выполненный запрос в момент `now` (для учёта бюджета).
    ///
    /// FIFO-буфер: при переполнении отбрасываем самый старый timestamp. Это сохраняет инвариант
    /// «кол-во timestamps ∈ окне == фактическое число запросов за минуту».
    pub fn on_request(&mut self, now: Duration) {
        let cap = self.max_per_min.max(1) as usize;
        if self.timestamps.len() >= cap {
            self.timestamps.remove(0);
        }
        self.timestamps.push(now);
    }
}

/// Exp backoff: `BASE × 2^attempt`, с защитой от переполнения shift (saturating).
fn exp_backoff(attempt: u32) -> Duration {
    let shift = attempt.min(31);
    let factor = 1u64 << shift;
    let nanos = (RECON_BASE_DELAY.as_nanos() as u64).saturating_mul(factor);
    Duration::from_nanos(nanos)
}
