//! RED OPS-I-4/7/8 (sacred, architect-only) — метрики (СЕМАНТИКА, не только имена) и тишина.
//!
//! C-009 M2: прежняя версия проверяла ТОЛЬКО присутствие имени → no-op `inc`/`set`, статический
//! вывод `<name> 0` и метрики БЕЗ labels её проходили. Оракул обязан мерить свой инвариант
//! (testing.md «Целостность гейта»): значение РЕАЛЬНО меняется, labeled-серии рендерят ключи.
//!
//! Анти-плацебо (после C-009):
//!  - no-op `inc_counter` (значение не растёт) → падает `counter_increments_change_value`;
//!  - статический `<name> 0` вывод → падает (значение != ожидаемого);
//!  - метрики без labels (`md_events_total 1` вместо `md_events_total{venue=..}`) → падает
//!    `label_bearing_metrics_render_all_keys`.
//!
//! Против `todo!()`-скелета — все падают.

use std::sync::Arc;
use std::thread;

use ops::metrics::{MetricKind, Metrics, METRICS};
use ops::silence::{is_silent, SILENCE_THRESHOLD_MS};

/// OPS-I-4: КАЖДАЯ метрика §3 (`METRICS`) присутствует в Prometheus-выводе.
#[test]
fn ops_i_4_every_metric_is_exported() {
    let m = Metrics::new();
    let text = m.prometheus_text();
    for spec in METRICS {
        assert!(
            text.contains(spec.name),
            "метрика `{}` из §3 НЕ экспортируется — подсистема, которую она измеряет, невидима \
             для мониторинга (OPS-I-4)",
            spec.name
        );
    }
    assert!(!METRICS.is_empty(), "набор метрик пуст — §3 не перенесён");
}

/// OPS-I-4/7 (СЕМАНТИКА): инкремент счётчика РЕАЛЬНО меняет экспортируемое значение.
/// No-op `inc` и статический `<name> 0` вывод обязаны ВАЛИТЬ этот тест (C-009 M2).
#[test]
fn ops_i_4_counter_increments_change_exported_value() {
    let m = Metrics::new();
    let name = METRICS
        .iter()
        .find(|s| s.kind == MetricKind::Counter && s.labels.is_empty())
        .map(|s| s.name)
        .expect("нужен хотя бы один counter без labels");

    m.inc_counter(name, &[], 5);
    assert_eq!(
        series_value(&m.prometheus_text(), name, &[]),
        Some(5),
        "после inc(+5) `{name}` != 5 (no-op inc или статический вывод) — счётчик декоративен"
    );
    m.inc_counter(name, &[], 7);
    assert_eq!(
        series_value(&m.prometheus_text(), name, &[]),
        Some(12),
        "счётчик `{name}` не накапливается (5+7 != 12) — инкремент не работает"
    );
}

/// OPS-I-4 (СЕМАНТИКА): установка gauge РЕАЛЬНО меняет экспортируемое значение.
#[test]
fn ops_i_4_gauge_set_changes_exported_value() {
    let m = Metrics::new();
    let name = METRICS
        .iter()
        .find(|s| s.kind == MetricKind::Gauge && s.labels.is_empty())
        .map(|s| s.name)
        .expect("нужен хотя бы один gauge без labels");
    m.set_gauge(name, &[], 42);
    assert_eq!(
        series_value(&m.prometheus_text(), name, &[]),
        Some(42),
        "gauge `{name}` не принял установленное значение (no-op set) — метрика декоративна"
    );
    m.set_gauge(name, &[], -7);
    assert_eq!(
        series_value(&m.prometheus_text(), name, &[]),
        Some(-7),
        "gauge `{name}` не перезаписался"
    );
}

/// OPS-I-4 (LABELS, C-009 M2): labeled-метрики рендерят ВСЕ ключи из `MetricSpec.labels`.
/// Схлопывание размерности (`md_events_total 1` вместо `md_events_total{venue=..}`) обязано ВАЛИТЬ
/// тест: без labels нельзя различить venue/symbol/kind — сигнал теряется.
#[test]
fn ops_i_4_label_bearing_metrics_render_all_keys() {
    let m = Metrics::new();
    let labeled: Vec<_> = METRICS.iter().filter(|s| !s.labels.is_empty()).collect();
    assert!(
        !labeled.is_empty(),
        "в §3 обязаны быть labeled-метрики (md_events_total и т.п.)"
    );
    for spec in labeled {
        let vals: Vec<(&str, &str)> = spec.labels.iter().map(|k| (*k, "x")).collect();
        match spec.kind {
            MetricKind::Counter => m.inc_counter(spec.name, &vals, 1),
            MetricKind::Gauge => m.set_gauge(spec.name, &vals, 1),
        }
        let text = m.prometheus_text();
        let line = text
            .lines()
            .find(|l| l.starts_with(spec.name))
            .unwrap_or_else(|| panic!("серия `{}` не выведена", spec.name));
        for key in spec.labels {
            assert!(
                line.contains(&format!("{key}=")),
                "метрика `{}` выведена без ключа label `{key}` (`{line}`) — размерность схлопнута, \
                 venue/symbol/kind не различить (C-009 M2)",
                spec.name
            );
        }
    }
}

/// OPS-I-7: инкремент через `&self` (атомик) — доказательство: `Arc<Metrics>` инкрементируется из
/// потоков без внешней синхронизации, и итог = сумме (не потеряны из-за гонки).
#[test]
fn ops_i_7_increment_is_lock_free_and_correct_under_threads() {
    let m = Arc::new(Metrics::new());
    let name = METRICS
        .iter()
        .find(|s| s.kind == MetricKind::Counter && s.labels.is_empty())
        .map(|s| s.name)
        .expect("counter без labels");
    let mut hs = Vec::new();
    for _ in 0..4 {
        let m = Arc::clone(&m);
        let name = name.to_string();
        hs.push(thread::spawn(move || {
            for _ in 0..1000 {
                m.inc_counter(&name, &[], 1);
            }
        }));
    }
    for h in hs {
        h.join().expect("поток инкремента не должен паниковать");
    }
    assert_eq!(
        series_value(&m.prometheus_text(), name, &[]),
        Some(4000),
        "4×1000 инкрементов дали != 4000 — гонка/потеря (не атомарно) или no-op"
    );
}

/// OPS-I-8: возраст последнего MD-события выше порога → тишина (алерт P1, TD-011/TD-014).
#[test]
fn ops_i_8_silence_above_threshold_alerts() {
    assert!(
        !is_silent(1000, SILENCE_THRESHOLD_MS),
        "свежий поток (1с) НЕ должен алертить"
    );
    assert!(
        is_silent(SILENCE_THRESHOLD_MS + 1, SILENCE_THRESHOLD_MS),
        "поток молчит дольше порога — обязан алертить (жив, но не работает: TD-011/TD-014)"
    );
    assert!(
        !is_silent(SILENCE_THRESHOLD_MS, SILENCE_THRESHOLD_MS),
        "ровно порог — ещё не тишина"
    );
}

/// Извлечь целое значение серии `name` с данными `labels` из Prometheus text (последнее поле
/// строки). Для `[]`-labels берёт строку `name <v>`; для labeled — строку с нужными парами.
fn series_value(text: &str, name: &str, labels: &[(&str, &str)]) -> Option<i64> {
    for line in text.lines() {
        if !line.starts_with(name) {
            continue;
        }
        if labels
            .iter()
            .all(|(k, v)| line.contains(&format!("{k}=\"{v}\"")))
        {
            return line
                .split_whitespace()
                .last()
                .and_then(|v| v.parse::<i64>().ok());
        }
    }
    None
}
