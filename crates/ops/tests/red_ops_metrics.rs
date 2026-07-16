//! RED OPS-I-4/7/8 (sacred, architect-only) — метрики и тишина потока.
//!
//! OPS-I-4: каждая метрика §3 экспортируется (иначе подсистема «не существует» для мониторинга).
//! OPS-I-7: инкремент lock-free (`&self`, атомик) — горячий путь recorder/venue не блокируется
//!          экспортом (компилируемость `Arc<Metrics>` + инкремент из потоков это доказывает).
//! OPS-I-8: тишина в потоке = алерт (жив, но не работает — TD-011/TD-014).
//! Против `todo!()`-скелета все падают.

use std::sync::Arc;
use std::thread;

use ops::metrics::{Metrics, METRIC_NAMES};
use ops::silence::{is_silent, SILENCE_THRESHOLD_MS};

/// OPS-I-4: КАЖДОЕ имя из `METRIC_NAMES` присутствует в Prometheus-выводе.
#[test]
fn ops_i_4_every_metric_is_exported() {
    let m = Metrics::new();
    let text = m.prometheus_text();
    for name in METRIC_NAMES {
        assert!(
            text.contains(name),
            "метрика `{name}` из §3 НЕ экспортируется — подсистема, которую она измеряет, невидима \
             для мониторинга (OPS-I-4)"
        );
    }
    assert!(
        !METRIC_NAMES.is_empty(),
        "набор метрик пуст — §3 не перенесён"
    );
}

/// OPS-I-7: инкремент через `&self` (атомик), НЕ `&mut` и НЕ под Mutex — иначе экспорт/скрейп
/// блокировал бы горячий путь. Доказательство: `Arc<Metrics>` инкрементируется из нескольких
/// потоков без внешней синхронизации (сигнатура `inc(&self, ...)` это разрешает).
#[test]
fn ops_i_7_increment_is_lock_free_shared() {
    let m = Arc::new(Metrics::new());
    let mut hs = Vec::new();
    for _ in 0..4 {
        let m = Arc::clone(&m);
        hs.push(thread::spawn(move || {
            for _ in 0..1000 {
                m.inc("journal_bytes_written_total", 1);
            }
        }));
    }
    for h in hs {
        h.join()
            .expect("поток инкремента метрики не должен паниковать");
    }
    // Экспорт читает те же атомики без блокировки писателей.
    let text = m.prometheus_text();
    assert!(text.contains("journal_bytes_written_total"));
}

/// OPS-I-8: возраст последнего MD-события выше порога → тишина (алерт P1). Отсутствие события —
/// сам сигнал, а не «подождём ещё».
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
    // Граница: ровно порог — ещё не тишина (строгое превышение).
    assert!(!is_silent(SILENCE_THRESHOLD_MS, SILENCE_THRESHOLD_MS));
}
