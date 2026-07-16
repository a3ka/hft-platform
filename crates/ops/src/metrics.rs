//! OPS-I-4/6/7 — реестр метрик (`/metrics`, Prometheus text). `ops.md` §3.
//!
//! - OPS-I-4: каждая метрика §3 экспортируется; отсутствие метрики = отсутствие подсистемы.
//! - OPS-I-6: метрики в журнал НЕ пишутся (журнал детерминирован; RSS/wall-clock — не события
//!   домена). Реестр НЕ зависит от `journal` в рантайме и не производит `EventKind`.
//! - OPS-I-7: экспорт не в горячем пути — инкремент через атомики (`&self`, без блокировок).
//!
//! **Labels в КОНТРАКТЕ (C-009 M2).** Метрики §3 несут размерности (`md_events_total{venue,
//! symbol,kind}` и т.п.). Если labels не в СИГНАТУРЕ, impl молча схлопнет размерности и оракул
//! этого не увидит. Поэтому: `MetricSpec` фиксирует имя+тип+ключи labels; API принимает labels
//! явно; RED-оракул проверяет, что labeled-серии рендерятся с нужными ключами и что значение
//! РЕАЛЬНО меняется (no-op/статический вывод обязаны падать).

use std::sync::atomic::AtomicU64;

/// Тип метрики: счётчик (монотонно растёт) или gauge (устанавливается).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    Counter,
    Gauge,
}

/// Спецификация метрики §3: имя + тип + ключи labels. ЕДИНСТВЕННЫЙ источник (канон) — на него
/// ссылаются паритет OPS-I-5 (`verify_M-09.sh`), grep-канарейка OPS-I-4 и label-контракт RED.
#[derive(Debug, Clone, Copy)]
pub struct MetricSpec {
    pub name: &'static str,
    pub kind: MetricKind,
    pub labels: &'static [&'static str],
}

use MetricKind::{Counter, Gauge};

/// Метрики §3 `ops.md` с ТОЧНЫМИ размерностями. Labels — часть контракта (C-009 M2): impl не
/// смеет схлопнуть размерность, RED проверяет рендер ключей.
pub const METRICS: &[MetricSpec] = &[
    MetricSpec {
        name: "recorder_rss_anon_bytes",
        kind: Gauge,
        labels: &[],
    },
    MetricSpec {
        name: "journal_bytes_written_total",
        kind: Counter,
        labels: &[],
    },
    MetricSpec {
        name: "journal_seq_current",
        kind: Gauge,
        labels: &[],
    },
    MetricSpec {
        name: "journal_seq_gaps_total",
        kind: Counter,
        labels: &[],
    },
    MetricSpec {
        name: "journal_segment_index",
        kind: Gauge,
        labels: &[],
    },
    MetricSpec {
        name: "journal_disk_free_bytes",
        kind: Gauge,
        labels: &[],
    },
    MetricSpec {
        name: "journal_write_errors_total",
        kind: Counter,
        labels: &[],
    },
    MetricSpec {
        name: "md_events_total",
        kind: Counter,
        labels: &["venue", "symbol", "kind"],
    },
    MetricSpec {
        name: "md_event_age_ms",
        kind: Gauge,
        labels: &["venue"],
    },
    MetricSpec {
        name: "venue_ws_reconnects_total",
        kind: Counter,
        labels: &["venue"],
    },
    MetricSpec {
        name: "venue_http_status_total",
        kind: Counter,
        labels: &["venue", "code"],
    },
    MetricSpec {
        name: "book_levels",
        kind: Gauge,
        labels: &["venue", "symbol", "side"],
    },
    MetricSpec {
        name: "book_divergence_bps",
        kind: Gauge,
        labels: &["venue", "symbol"],
    },
    MetricSpec {
        name: "book_resync_total",
        kind: Counter,
        labels: &["venue", "symbol"],
    },
    MetricSpec {
        name: "backup_restore_drill_ok",
        kind: Gauge,
        labels: &[],
    },
];

/// Имена метрик (для grep-канарейки OPS-I-4 и совместимости). Производно от `METRICS`.
pub fn metric_names() -> Vec<&'static str> {
    METRICS.iter().map(|m| m.name).collect()
}

/// Реестр метрик. Инкремент — lock-free (`&self`, атомики, OPS-I-7); экспорт — по запросу.
pub struct Metrics {
    _writes: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        todo!("OPS-I-4: инициализировать все METRICS (с нулями), учесть размерности labels")
    }

    /// Инкремент СЧЁТЧИКА `name` c данными `labels` на `by`. `&self` (атомик, OPS-I-7).
    /// `labels` — пары (ключ, значение) для размерностей из `MetricSpec.labels`.
    pub fn inc_counter(&self, _name: &str, _labels: &[(&str, &str)], _by: u64) {
        todo!("OPS-I-7: атомарный fetch_add по (name, labels); no-op запрещён (RED проверит рост)")
    }

    /// Установить GAUGE `name` c `labels` в `value`. `&self` (атомик).
    pub fn set_gauge(&self, _name: &str, _labels: &[(&str, &str)], _value: i64) {
        todo!("OPS-I-7: атомарная запись по (name, labels); no-op запрещён")
    }

    /// Prometheus text. OPS-I-4: КАЖДАЯ метрика `METRICS` присутствует; labeled-серии рендерятся
    /// как `name{k1="v1",k2="v2"} value` с ВСЕМИ ключами из `MetricSpec.labels` (C-009 M2).
    pub fn prometheus_text(&self) -> String {
        todo!("OPS-I-4: рендер всех METRICS; labeled — с ключами labels; значения из атомиков")
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
