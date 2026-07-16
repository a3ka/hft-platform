//! OPS-I-4/6/7 — реестр метрик (`/metrics`, Prometheus text). `ops.md` §3.
//!
//! - OPS-I-4: каждая метрика §3 экспортируется; отсутствие метрики = отсутствие подсистемы.
//! - OPS-I-6: метрики в журнал НЕ пишутся (журнал детерминирован; RSS/wall-clock — не события
//!   домена). Реестр НЕ зависит от `journal` и не производит `EventKind` (проверяется структурно).
//! - OPS-I-7: экспорт не в горячем пути — инкремент через атомики (`&self`, без блокировок).

use std::sync::atomic::AtomicU64;

/// Канонический набор метрик §3 (`ops.md`). ЕДИНСТВЕННЫЙ источник имён — на него ссылается
/// паритет-проверка OPS-I-5 (`scripts/verify_M-09.sh`) и grep-канарейка OPS-I-4. Порядок не важен.
pub const METRIC_NAMES: &[&str] = &[
    "recorder_rss_anon_bytes",
    "journal_bytes_written_total",
    "journal_seq_current",
    "journal_seq_gaps_total",
    "journal_segment_index",
    "journal_disk_free_bytes",
    "journal_write_errors_total",
    "md_events_total",
    "md_event_age_ms",
    "venue_ws_reconnects_total",
    "venue_http_status_total",
    "book_levels",
    "book_divergence_bps",
    "book_resync_total",
    "backup_restore_drill_ok",
];

/// Реестр метрик. Инкремент — lock-free (`&self`, атомики, OPS-I-7); экспорт — по запросу.
/// Поля приватны; форма (labels) — забота impl. Здесь фиксируется КОНТРАКТ: имена + экспорт.
pub struct Metrics {
    // Пример: реальные счётчики — в impl. Один атомик, чтобы тип был непустым и `&self`-инкремент
    // был выражаем в сигнатуре (OPS-I-7: без &mut, без Mutex в горячем пути).
    _writes: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        todo!("OPS-I-4: инициализировать все метрики METRIC_NAMES нулями")
    }

    /// Инкремент/установка — `&self` (атомик), НЕ `&mut` и НЕ под Mutex (OPS-I-7: горячий путь
    /// recorder/venue не блокируется экспортом).
    pub fn inc(&self, _name: &str, _by: u64) {
        todo!("OPS-I-7: атомарный fetch_add по имени метрики")
    }
    pub fn set(&self, _name: &str, _value: i64) {
        todo!("OPS-I-7: атомарная запись gauge по имени")
    }

    /// Prometheus text. OPS-I-4: КАЖДАЯ метрика `METRIC_NAMES` присутствует в выводе (иначе
    /// подсистема, которую она измеряет, «не существует» для мониторинга).
    pub fn prometheus_text(&self) -> String {
        todo!("OPS-I-4: вывести все METRIC_NAMES в Prometheus text-формате")
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
