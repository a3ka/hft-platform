//! RED OPS-I-5 ПРАВИЛА АЛЕРТОВ + ДВУСТОРОННИЙ ПАРИТЕТ (sacred, architect-only). `ops.md` §7/§7.1.
//!
//! «Метрика без алерта бесполезна, алерт без метрики невозможен, класс инцидента без правила — дыра»
//! (OPS-I-5). Реестр метрик и паритет ИМЁН уже есть (`red_ops_metrics`, `verify_M-09.sh`). Task 4B
//! добавляет КАТАЛОГ ПРАВИЛ как машиночитаемый артефакт — ОДИН канон `ops::alerts::ALERT_RULES`,
//! из которого рендерятся Prometheus-правила (`deploy/alerts/`, engine-dev). Живой Alertmanager не
//! провижен (§O) — правила авторируются + паритет-проверяются сейчас; live-alerting включит founder ★.
//!
//! Контракт `ops::alerts`:
//!  - `Severity { P0, P1, P2 }`;
//!  - `AlertRule { incident: &'static str, severity: Severity, metric: &'static str, summary: &'static str }`;
//!  - `ALERT_RULES: &[AlertRule]`;
//!  - `to_prometheus_rules() -> String` (рендер каталога в Prometheus-правила).
//!
//! Анти-плацебо В ОБЕ СТОРОНЫ: правило→несуществующая метрика валит (1); удаление правила
//! обязательного класса валит (2); пустой/безсеверитийный рендер валит (3). Против `todo!()` — все падают.

use ops::alerts::{Severity, ALERT_RULES};
use ops::metrics::metric_names;

/// Канон обязательных P0/P1-классов инцидентов §7.1 (тот же список, что `verify_M-09.sh`
/// REQUIRED_INCIDENTS). Класс без правила = дыра OPS-I-5 (ровно C-007 C1: целый класс порчи вне
/// алертов). P2 (book_levels рост, cadence) — наблюдательный слой, вне строгого паритета.
const REQUIRED_INCIDENTS: &[&str] = &[
    "TD-011",      // запись остановилась (P0)
    "TD-013",      // 418/429 rate-limit (P1)
    "TD-014",      // класс событий пропал (P1)
    "TD-016",      // RssAnon-тренд (P1)
    "C1-M08",      // порча данных / recon-ресинк (P0)
    "TD-006",      // диск кончается (P0)
    "OPS-BKP",     // restore-drill провален (P0)
    "OPS-SILENCE", // тишина потока (P1)
    "OPS-RESYNC",  // частые ресинки (P1)
    "OPS-GAP",     // gap-доля за сутки (P1)
];

/// (1) КАЖДОЕ правило ссылается на СУЩЕСТВУЮЩУЮ метрику (`METRICS`). Правило-без-метрики
/// невозможно (OPS-I-5). Анти-плацебо: правило с `metric` вне `metric_names()` валит.
#[test]
fn every_rule_references_existing_metric() {
    let names = metric_names();
    assert!(
        !ALERT_RULES.is_empty(),
        "каталог правил пуст — §7 не перенесён"
    );
    for rule in ALERT_RULES {
        assert!(
            names.contains(&rule.metric),
            "правило `{}` (severity {:?}) ссылается на метрику `{}`, которой НЕТ в METRICS §3 — \
             алерт без метрики невозможен (OPS-I-5)",
            rule.incident,
            rule.severity,
            rule.metric
        );
    }
}

/// (2) КАЖДЫЙ обязательный класс §7.1 имеет ≥1 правило. Класс-без-правила = дыра (целый класс
/// инцидента вне алертов). Анти-плацебо: удаление правила любого класса из каталога валит тест.
#[test]
fn every_required_incident_class_has_a_rule() {
    for &inc in REQUIRED_INCIDENTS {
        let covered = ALERT_RULES.iter().any(|r| r.incident == inc);
        assert!(
            covered,
            "класс инцидента `{inc}` (§7.1) НЕ имеет ни одного правила в ALERT_RULES — целый класс \
             порчи/деградации вне алертов (OPS-I-5 rule-side, регрессия C-007 C1)"
        );
    }
}

/// (2-обратно) КАЖДОЕ правило относится к известному классу §7.1 (нет «правила-сироты» на несуществующий
/// класс — иначе паритет односторонний). Канон REQUIRED_INCIDENTS + P2-наблюдательные допускаются, но
/// incident обязан быть непустым и осмысленным ID.
#[test]
fn every_rule_has_nonempty_incident_id() {
    for rule in ALERT_RULES {
        assert!(
            !rule.incident.trim().is_empty(),
            "правило на метрику `{}` без incident-ID — не привязано к классу §7.1 (паритет односторонний)",
            rule.metric
        );
        assert!(
            !rule.summary.trim().is_empty(),
            "правило `{}` без summary — оператор не поймёт, что сработало",
            rule.incident
        );
    }
}

/// (3) Рендер `to_prometheus_rules()` несёт для КАЖДОГО правила его метрику И severity (семантика, не
/// пустой рендер). Анти-плацебо: `String::new()`/статическая заглушка валит.
#[test]
fn rendered_rules_carry_metric_and_severity() {
    let rendered = ops::alerts::to_prometheus_rules();
    assert!(
        !rendered.trim().is_empty(),
        "to_prometheus_rules() пуст — deploy/alerts/ получит пустой артефакт (алертов нет)"
    );
    for rule in ALERT_RULES {
        assert!(
            rendered.contains(rule.metric),
            "рендер правил не содержит метрику `{}` правила `{}` — Prometheus-правило без выражения по \
             метрике не сработает",
            rule.metric,
            rule.incident
        );
        let sev = format!("{:?}", rule.severity); // P0 / P1 / P2
        assert!(
            rendered.contains(&sev),
            "рендер правила `{}` не несёт severity `{sev}` — Alertmanager не разведёт P0/P1/P2 маршрут",
            rule.incident
        );
    }
}

/// Severity — ровно три уровня; каждое правило несёт валидный (тип гарантирует, но фиксируем
/// присутствие всех трёх в системе — P0/P1 обязательны, P2 наблюдательный).
#[test]
fn severities_cover_p0_and_p1() {
    let has = |s: Severity| ALERT_RULES.iter().any(|r| r.severity == s);
    assert!(has(Severity::P0), "нет ни одного P0-правила — критические инциденты (запись стоит, диск, порча) не будят человека");
    assert!(
        has(Severity::P1),
        "нет ни одного P1-правила — деградации (тишина, rate-limit, gap) не эскалируются"
    );
}
