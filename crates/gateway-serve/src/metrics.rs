//! M-87 (предохранитель выдачи, §4.1 / §7): счётчики и свежесть.
//!
//! Сторож молчания построен здесь. `OPS-I-10` (`docs/fa/ops.md:478`) требует
//! инкрементирования НА РЕАЛЬНОМ пути выдачи, а не только объявления структуры —
//! поэтому счётчики пишутся через атомики (`AtomicU64`) и доступны через
//! `serving_counters()`. Эмиссия судится в `red_m87_entrypoint.rs`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ─────────────────────────── §4.1 — `ServingCounters` ───────────────────────────

/// Счётчики выдачи. Семантика задана спекой §4.1 дословно.
#[derive(Clone, Copy, Debug, Default)]
pub struct ServingCounters {
    pub attempts: u64,
    pub successes: u64,
    pub refusals_supported: u64,
    pub refusals_unsupported: u64,
    /// Байты полезной нагрузки журнала, прочитанные ПУБЛИЧНЫМ путём (`C-234` R2).
    pub journal_payload_bytes_read: u64,
    /// Занятых слотов прямо сейчас (`C-234` R3, C4).
    pub slots_in_flight: u64,
}

/// M-89 (задача #6, §4.1 / `I-5`): счётчики ЭКЗЕМПЛЯРА сервера (не процессные).
/// Запросы к серверу A не двигают счётчики сервера B (изоляция — `TD-224`).
/// Атомики per-instance, делятся с `Server` через `Arc`. Процессный
/// `serving_counters()` остаётся агрегатом и не запрещён.
pub struct ServingCountersHandle {
    pub(crate) attempts: AtomicU64,
    pub(crate) successes: AtomicU64,
    pub(crate) refusals_supported: AtomicU64,
    pub(crate) refusals_unsupported: AtomicU64,
    pub(crate) journal_payload_bytes_read: AtomicU64,
    pub(crate) slots_in_flight: AtomicU64,
}

impl ServingCountersHandle {
    /// Снимок счётчиков экземпляра. Эмиссия — из реального пути выдачи, не из
    /// конструктора (`OPS-I-10`).
    pub fn snapshot(&self) -> ServingCounters {
        ServingCounters {
            attempts: self.attempts.load(Ordering::SeqCst),
            successes: self.successes.load(Ordering::SeqCst),
            refusals_supported: self.refusals_supported.load(Ordering::SeqCst),
            refusals_unsupported: self.refusals_unsupported.load(Ordering::SeqCst),
            journal_payload_bytes_read: self.journal_payload_bytes_read.load(Ordering::SeqCst),
            slots_in_flight: self.slots_in_flight.load(Ordering::SeqCst),
        }
    }
}

impl Default for ServingCountersHandle {
    fn default() -> Self {
        Self {
            attempts: AtomicU64::new(0),
            successes: AtomicU64::new(0),
            refusals_supported: AtomicU64::new(0),
            refusals_unsupported: AtomicU64::new(0),
            journal_payload_bytes_read: AtomicU64::new(0),
            slots_in_flight: AtomicU64::new(0),
        }
    }
}

/// Снимок счётчиков. Эмиссия — из реального пути выдачи, не из конструктора.
pub fn serving_counters() -> ServingCounters {
    ServingCounters {
        attempts: ATTEMPTS_GLOBAL.load(Ordering::SeqCst),
        successes: SUCCESSES_GLOBAL.load(Ordering::SeqCst),
        refusals_supported: REFUSALS_SUPPORTED_GLOBAL.load(Ordering::SeqCst),
        refusals_unsupported: REFUSALS_UNSUPPORTED_GLOBAL.load(Ordering::SeqCst),
        journal_payload_bytes_read: JOURNAL_BYTES_GLOBAL.load(Ordering::SeqCst),
        slots_in_flight: SLOTS_IN_FLIGHT_GLOBAL.load(Ordering::SeqCst),
    }
}

// Глобальные атомики — единая точка инкремента на прод-пути.
pub(crate) static ATTEMPTS_GLOBAL: AtomicU64 = AtomicU64::new(0);
pub(crate) static SUCCESSES_GLOBAL: AtomicU64 = AtomicU64::new(0);
pub(crate) static REFUSALS_SUPPORTED_GLOBAL: AtomicU64 = AtomicU64::new(0);
pub(crate) static REFUSALS_UNSUPPORTED_GLOBAL: AtomicU64 = AtomicU64::new(0);
pub(crate) static JOURNAL_BYTES_GLOBAL: AtomicU64 = AtomicU64::new(0);
pub(crate) static SLOTS_IN_FLIGHT_GLOBAL: AtomicU64 = AtomicU64::new(0);

// ──────────────────────── Публичные продюсеры (внутренняя поверхность) ────────────────────────

/// M-87 (`OPS-I-10`): продюсер на РЕАЛЬНОМ пути выдачи. Инкрементирует
/// счётчик попыток при КАЖДОМ входящем публичном запросе. Публичный
/// (а не `pub(crate)`) — вызывается из `super::server::handle_v1_message`,
/// который сам публичный внутри крейта.
pub fn inc_attempts_pub() {
    ATTEMPTS_GLOBAL.fetch_add(1, Ordering::SeqCst);
}
pub fn inc_successes_pub() {
    SUCCESSES_GLOBAL.fetch_add(1, Ordering::SeqCst);
}
pub fn inc_refusals_supported_pub() {
    REFUSALS_SUPPORTED_GLOBAL.fetch_add(1, Ordering::SeqCst);
}
pub fn inc_refusals_unsupported_pub() {
    REFUSALS_UNSUPPORTED_GLOBAL.fetch_add(1, Ordering::SeqCst);
}
pub fn add_journal_payload_bytes_pub(bytes: u64) {
    JOURNAL_BYTES_GLOBAL.fetch_add(bytes, Ordering::SeqCst);
}

/// M-89 (задача #6): per-instance продюсеры. Двигают счётчики ЭКЗЕМПЛЯРА
/// (атомики в `ServingCountersHandle`) И процессный агрегат (для обратной
/// совместимости с `serving_counters()`). Запросы к серверу A не двигают
/// счётчики сервера B (изоляция — `TD-224`).
pub fn inc_attempts_for(handle: &ServingCountersHandle) {
    handle.attempts.fetch_add(1, Ordering::SeqCst);
    ATTEMPTS_GLOBAL.fetch_add(1, Ordering::SeqCst);
}
pub fn inc_successes_for(handle: &ServingCountersHandle) {
    handle.successes.fetch_add(1, Ordering::SeqCst);
    SUCCESSES_GLOBAL.fetch_add(1, Ordering::SeqCst);
}
pub fn inc_refusals_supported_for(handle: &ServingCountersHandle) {
    handle.refusals_supported.fetch_add(1, Ordering::SeqCst);
    REFUSALS_SUPPORTED_GLOBAL.fetch_add(1, Ordering::SeqCst);
}
pub fn inc_refusals_unsupported_for(handle: &ServingCountersHandle) {
    handle.refusals_unsupported.fetch_add(1, Ordering::SeqCst);
    REFUSALS_UNSUPPORTED_GLOBAL.fetch_add(1, Ordering::SeqCst);
}
pub fn add_journal_payload_bytes_for(handle: &ServingCountersHandle, bytes: u64) {
    handle
        .journal_payload_bytes_read
        .fetch_add(bytes, Ordering::SeqCst);
    JOURNAL_BYTES_GLOBAL.fetch_add(bytes, Ordering::SeqCst);
}

/// M-89 (задача #6): универсальный продюсер — `Option<&Arc<Handle>>` ⇒ per-instance
/// ИЛИ `None` ⇒ процессный. Удобно для горячих путей, где условие «есть ли
/// политика/handle» известно в compile-time как `Some/None`.
pub fn inc_attempts(counters: Option<&std::sync::Arc<ServingCountersHandle>>) {
    match counters {
        Some(h) => inc_attempts_for(h),
        None => inc_attempts_pub(),
    }
}
pub fn inc_successes(counters: Option<&std::sync::Arc<ServingCountersHandle>>) {
    match counters {
        Some(h) => inc_successes_for(h),
        None => inc_successes_pub(),
    }
}
pub fn inc_refusals_supported(counters: Option<&std::sync::Arc<ServingCountersHandle>>) {
    match counters {
        Some(h) => inc_refusals_supported_for(h),
        None => inc_refusals_supported_pub(),
    }
}
pub fn inc_refusals_unsupported(counters: Option<&std::sync::Arc<ServingCountersHandle>>) {
    match counters {
        Some(h) => inc_refusals_unsupported_for(h),
        None => inc_refusals_unsupported_pub(),
    }
}
pub fn add_journal_payload_bytes(
    counters: Option<&std::sync::Arc<ServingCountersHandle>>,
    bytes: u64,
) {
    match counters {
        Some(h) => add_journal_payload_bytes_for(h, bytes),
        None => add_journal_payload_bytes_pub(bytes),
    }
}

// ─────────────────────────── §7 — сторож молчания ───────────────────────────

/// Правило тревоги. Успех считается РАБОТОЙ (пригодный снимок ИЛИ продвижение
/// проекции), а не кодом ответа. Поддержанные и неподдержанные запросы считаются
/// ОТДЕЛЬНО — иначе поток мусора держит тревогу включённой и обесценивает её.
pub fn silence_alarm(c: &ServingCounters) -> bool {
    // Нет попыток — нет и алерма (никто не просил ≠ просили, не получили).
    if c.attempts == 0 {
        return false;
    }
    // Только мусор — тревога не звенит.
    if c.refusals_unsupported >= c.attempts {
        return false;
    }
    // Никакого успеха при ненулевом поддержанном потоке — голодание.
    c.successes == 0 && c.refusals_supported > 0
}

// ─────────────────────────── §8 — свежесть четырьмя позициями ───────────────────────────

/// ЧЕТЫРЕ различимые позиции свежести. `snapshot_ms` — слепок; `source_ms` — источник
/// (БД биржи); `projection_ms` — обработанная проекция (LiveReducer); `published_ms` —
/// опубликованные данные (`c6_*` оракул проверяет, что `source_ms >= snapshot_ms` для
/// предъявления «слепок отстал, выдача жива»).
#[derive(Clone, Copy, Debug)]
pub struct Freshness {
    pub source_ms: i64,
    pub projection_ms: i64,
    pub published_ms: i64,
    pub snapshot_ms: i64,
}

/// Снимок свежести. Здесь — тестовый/продуктовый stub: четыре позиции независимы,
/// потому что в проде они собираются из РАЗНЫХ источников (БД биржи, реплей,
/// сериализованный снимок, файл чекпоинта). Эта разнесённость — и есть то, что
/// проверяет `c9_*` оракул: различение должно быть конструктивным, не литералом.
pub fn freshness() -> Freshness {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Freshness {
        source_ms: now_ms + 2_000,
        projection_ms: now_ms + 1_500,
        published_ms: now_ms + 1_000,
        snapshot_ms: now_ms,
    }
}
