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

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

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

/// Ключ серии: имя + сортированный по ключу вектор `(key, value)`-пар.
/// Сортировка детерминирует канонический порядок меток в выводе (одинаковые серии рендерятся
/// одинаково вне зависимости от порядка передачи).
type SeriesKey = (String, Vec<(String, String)>);

/// Реестр метрик. Инкремент — lock-free (`&self`, атомики, OPS-I-7); экспорт — по запросу.
///
/// Внутренняя структура:
///  - `Mutex<HashMap>` защищает только создание/поиск серии (холодный путь: первое касание);
///  - `Arc<AtomicI64>` — инкремент/запись БЕЗ удержания мьютекса (горячий путь, OPS-I-7);
///  - на чтение (`prometheus_text`) мьютекс удерживается — scrape-вызов НЕ горячий.
///
/// Counter и Gauge делят `AtomicI64`: counter растёт монотонно (`fetch_add`), gauge перезаписывается
/// (`store`). Один HashMap дешевле, чем два; тип метрики уже различает семантику.
pub struct Metrics {
    series: Mutex<HashMap<SeriesKey, Arc<AtomicI64>>>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            series: Mutex::new(HashMap::new()),
        }
    }

    /// Найти или создать серию (короткое удержание мьютекса). Возвращает `Arc<AtomicI64>`,
    /// с которым операции можно делать БЕЗ мьютекса.
    fn get_or_create(&self, name: &str, labels: &[(&str, &str)]) -> Arc<AtomicI64> {
        let key = build_key(name, labels);
        let mut map = self.series.lock().expect("metrics mutex poisoned");
        map.entry(key)
            .or_insert_with(|| Arc::new(AtomicI64::new(0)))
            .clone()
    }

    /// Инкремент СЧЁТЧИКА `name` c данными `labels` на `by`. `&self` (атомик, OPS-I-7).
    /// `labels` — пары (ключ, значение) для размерностей из `MetricSpec.labels`.
    pub fn inc_counter(&self, name: &str, labels: &[(&str, &str)], by: u64) {
        let cell = self.get_or_create(name, labels);
        cell.fetch_add(by as i64, Ordering::Relaxed);
    }

    /// Установить GAUGE `name` c `labels` в `value`. `&self` (атомик).
    pub fn set_gauge(&self, name: &str, labels: &[(&str, &str)], value: i64) {
        let cell = self.get_or_create(name, labels);
        cell.store(value, Ordering::Relaxed);
    }

    /// Prometheus text. OPS-I-4: КАЖДАЯ метрика `METRICS` присутствует (через `# HELP`/`# TYPE`
    /// строки — иначе неизмеренная метрика исчезает, и grep-канарейка валится); labeled-серии
    /// рендерятся как `name{k1="v1",k2="v2"} value` с ВСЕМИ ключами из `MetricSpec.labels`
    /// (C-009 M2 — размерность не схлопывается).
    pub fn prometheus_text(&self) -> String {
        let mut out = String::with_capacity(2048);

        // (1) Каждая МЕТРИКА из канона → `# HELP` и `# TYPE` строки. Это гарантирует, что
        // `text.contains(spec.name)` проходит ДО того, как у метрики появилась хотя бы одна серия
        // (OPS-I-4: «отсутствие метрики = отсутствие подсистемы»).
        for spec in METRICS {
            out.push_str(&format!("# HELP {} {}\n", spec.name, spec.name));
            out.push_str(&format!(
                "# TYPE {} {}\n",
                spec.name,
                match spec.kind {
                    MetricKind::Counter => "counter",
                    MetricKind::Gauge => "gauge",
                }
            ));
        }

        // (2) Все известные серии. Короткое копирование ключей+значений под мьютексом, дальше
        // рендер без блокировки.
        let snapshot: Vec<(SeriesKey, i64)> = {
            let map = self.series.lock().expect("metrics mutex poisoned");
            map.iter()
                .map(|(k, cell)| (k.clone(), cell.load(Ordering::Relaxed)))
                .collect()
        };

        for ((name, labels), value) in snapshot {
            out.push_str(&render_series(&name, &labels, value));
        }

        out
    }
}

/// Канонический ключ серии: имя + лекс-сортированные `(key, value)`-пары.
fn build_key(name: &str, labels: &[(&str, &str)]) -> SeriesKey {
    let mut sorted: Vec<(String, String)> = labels
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    (name.to_string(), sorted)
}

/// Рендер одной серии: `name{k="v",...} value` (или `name value` для безлейбловых).
fn render_series(name: &str, labels: &[(String, String)], value: i64) -> String {
    if labels.is_empty() {
        format!("{name} {value}\n")
    } else {
        let mut s = String::with_capacity(64);
        s.push_str(name);
        s.push('{');
        for (i, (k, v)) in labels.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("{k}=\"{v}\""));
        }
        s.push_str(&format!("}} {value}\n"));
        s
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
