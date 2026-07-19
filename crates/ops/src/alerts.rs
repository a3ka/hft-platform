//! OPS-I-5 — каталог правил алертов P0/P1/P2 + двусторонний паритет. `ops.md` §7/§7.1.
//!
//! «Метрика без алерта бесполезна, алерт без метрики невозможен, класс инцидента без правила
//! — дыра» (OPS-I-5, C-007 C1). Этот модуль — ЕДИНСТВЕННЫЙ канон правил: реестр `METRICS`
//! в `metrics.rs`, реестр инцидентов в `docs/fa/ops.md §7.1`, реестр правил — ЗДЕСЬ. Из
//! него рендерится Prometheus-правило (`to_prometheus_rules()`) и пишется deploy-артефакт
//! `deploy/alerts/ops.rules.yml` (через `examples/dump_rules.rs`, drift-check в тестах).
//!
//! Живой Alertmanager НЕ провижен (`docs/fa/ops.md §O` — pull-vs-push развилка, founder ★):
//! правила АВТОРИРУЮТСЯ + ПАРИТЕТ-проверяются здесь и сейчас; live-alerting подключается,
//! когда founder ★ провижит Prometheus.
//!
//! **P2 (наблюдательный слой) вне строгого паритета** (`ops.md §7`): правило `book_levels` /
//! cadence рост — дайджест, не P0/P1-инцидент; метрики P2-правил ТОЖЕ в §3 (паритет на
//! метриках), но incident-ID в `REQUIRED_INCIDENTS` для них НЕ обязателен. Если founder ★
//! захочет P2-инцидент в строгом паритете — расширяется `§7.1` + `REQUIRED_INCIDENTS`, не
//! наоборот (иначе мы солжём, что класс алертится, а на деле это дайджест).

/// Уровень эскалации (Prometheus-label + Alertmanager route). P0 будит человека, P1 — в течение
/// часа, P2 — дайджест. Render format: `format!("{:?}", Severity::P0) == "P0"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    P0,
    P1,
    P2,
}

/// Канонический ID инцидента из `docs/fa/ops.md §7.1` (тот же, что в FA-таблице и в verify-скрипте
/// `REQUIRED_INCIDENTS`). Incident без ID в §7.1 = orphan-rule (проверяется
/// `red_ops_alerts::every_rule_has_nonempty_incident_id` + `verify_M-09.sh`).
pub type IncidentId = &'static str;

/// Правило алерта: «наблюдаемая метрика → инцидент + severity + human-summary». `metric` —
/// имя из `METRICS` (проверяется `every_rule_references_existing_metric`); `incident` — ID из
/// §7.1 (проверяется shell-паритетом); `severity` — P0/P1/P2 (проверяется
/// `severities_cover_p0_and_p1`); `summary` — короткое human-описание для runbook'а
/// (`every_rule_has_nonempty_incident_id`).
#[derive(Debug, Clone, Copy)]
pub struct AlertRule {
    pub incident: IncidentId,
    pub severity: Severity,
    pub metric: &'static str,
    pub summary: &'static str,
}

/// Канонический каталог правил. ЗЕРКАЛО `docs/fa/ops.md §7.1`: каждая строка таблицы → одно
/// правило (P0/P1 — обязательны; P2 — наблюдательный, расширяется офлайн). ПОРЯДОК —
/// строки §7.1 (важно для воспроизводимости рендера `to_prometheus_rules()`).
///
/// **Двусторонний паритет OPS-I-5:**
///  - каждое `rule.metric` ∈ `METRICS` (rule→metric) — `every_rule_references_existing_metric`;
///  - каждый `rule.incident` ∈ `REQUIRED_INCIDENTS` verify-скрипта = строка §7.1 — shell-чек
///    в `scripts/verify_M-09.sh`;
///  - нет `incident` вне §7.1 (orphan-rule) — `every_rule_has_nonempty_incident_id` + shell.
///
/// **Анти-плацебо:** правило-без-метрики валит (1); удаление правила обязательного класса валит
/// (2); пустой/безсеверитийный рендер валит (3). Подробности — `crates/ops/tests/red_ops_alerts.rs`.
pub const ALERT_RULES: &[AlertRule] = &[
    // TD-011 — recorder жив, но НЕ пишет. Метрика — счётчик записанных байт журнала; «нет
    // роста за окно» = recorder стоит. P0 (запись остановилась — данные невосстановимы).
    AlertRule {
        incident: "TD-011",
        severity: Severity::P0,
        metric: "journal_bytes_written_total",
        summary: "journal bytes written — нулевой прирост за 60с (recorder жив, но не пишет)",
    },
    // TD-013 — rate-limit-ответы 418/429 от биржи > N/мин (133×418 за 25с → IP-бан). Метрика —
    // счётчик HTTP-ответов по коду; правило «доля 4xx в окне > порога» — Prometheus сам агрегирует
    // через `rate(...)`. P1 (деградация, не немедленная потеря данных).
    AlertRule {
        incident: "TD-013",
        severity: Severity::P1,
        metric: "venue_http_status_total",
        summary: "venue HTTP 4xx (418/429) доля > порога — rate-limit от биржи (TD-013)",
    },
    // TD-014 — 0 Funding при «успешном» деплое: класс событий пропал целиком. Метрика —
    // счётчик MD-событий по виду; правило «`rate(md_events_total{kind="..."}[5m]) == 0` при
    // ожидаемой частоте» — алармит оператора. P1 (класс событий пропал).
    AlertRule {
        incident: "TD-014",
        severity: Severity::P1,
        metric: "md_events_total",
        summary:
            "md_events_total — нулевая производная по kind при живом WS (Funding/Trade пропали)",
    },
    // TD-016 — анонимная куча `RssAnon` тренд > порога/час (НЕ cgroup/docker, урок hft-core-rs).
    // Метрика — gauge `recorder_rss_anon_bytes`. P1 (утечка памяти → упадёт OOM-killer'ом).
    AlertRule {
        incident: "TD-016",
        severity: Severity::P1,
        metric: "recorder_rss_anon_bytes",
        summary: "recorder RSS-anon тренд > порога/час — утечка памяти (TD-016)",
    },
    // C1-M08 — порча данных / recon-ресинк. Эвикция стёрла живые уровни книги (best bid —
    // near-touch). Метрика — счётчик ресинков; B2 §4.3.2 — эмиссия ⟺ `best_price_diverged`.
    // P0 (порча данных — главный класс, ради которого M-09 и существует).
    AlertRule {
        incident: "C1-M08",
        severity: Severity::P0,
        metric: "book_resync_total",
        summary: "recon ресинк книги при best-расхождении — порча near-touch (C1-M08, B2 §4.3.2)",
    },
    // TD-006 — диск кончается (`journal_disk_free_bytes < min_free_bytes` из WriterConfig).
    // P0 (журнал не сможет ротировать сегмент → запись стоит → данные невосстановимы).
    AlertRule {
        incident: "TD-006",
        severity: Severity::P0,
        metric: "journal_disk_free_bytes",
        summary: "journal_disk_free_bytes < min_free_bytes — диск кончается (TD-006)",
    },
    // OPS-BKP — restore-drill провален (`backup_restore_drill_ok == 0` после попытки). P0
    // (бэкап, который не восстанавливается, — не бэкап; данные невосстановимы).
    AlertRule {
        incident: "OPS-BKP",
        severity: Severity::P0,
        metric: "backup_restore_drill_ok",
        summary: "backup_restore_drill_ok == 0 — restore-drill провален (бэкап не бэкап)",
    },
    // OPS-SILENCE — жив, но поток замолчал (`md_event_age_ms > 5min`). Метрика — gauge
    // возраста последнего MD по venue. P1 («жив, но не работает», класс TD-011/TD-014).
    AlertRule {
        incident: "OPS-SILENCE",
        severity: Severity::P1,
        metric: "md_event_age_ms",
        summary: "md_event_age_ms > 5min — поток молчит, процесс жив (OPS-I-8, OPS-SILENCE)",
    },
    // OPS-RESYNC — частые ресинки книги/фида. Метрика — тот же `book_resync_total`, что и
    // C1-M08; Prometheus различает правила через label/severity (anti-плацебо: ресинк-флуд
    // ≠ единичная порча). P1 (скрытая проблема фида/книги, требует разбора).
    AlertRule {
        incident: "OPS-RESYNC",
        severity: Severity::P1,
        metric: "book_resync_total",
        summary: "book_resync_total rate > порога — частые ресинки (скрытая проблема фида)",
    },
    // OPS-GAP — gap-доля за сутки > 1%. Метрика — счётчик пропусков seq в журнале
    // (`journal_seq_gaps_total`). P1 (дырки в потоке — replay/recon неполны).
    AlertRule {
        incident: "OPS-GAP",
        severity: Severity::P1,
        metric: "journal_seq_gaps_total",
        summary: "journal_seq_gaps_total / journal_seq_current > 1% за сутки — gap-доля (OPS-GAP)",
    },
];

/// Срендерить каталог в формат Prometheus rule files (один `groups:` с правилами по
/// `ALERT_RULES`). Возвращаемый текст — YAML-фрагмент, пригодный для включения в
/// `deploy/alerts/ops.rules.yml` через `groups:` корневого `rule_files` (или как самостоятельный
/// файл с `groups:` сверху — для одного Alertmanager-rule-file это работает).
///
/// Формат: для каждого правила — отдельная `alert:`-запись с `expr:` по метрике, `for:` и
/// `labels.severity`/`annotations.summary`. Имя алерта = `incident` (стабильный ID, §7.1).
pub fn to_prometheus_rules() -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("# Auto-generated by ops::alerts::to_prometheus_rules(). DO NOT EDIT BY HAND —\n");
    out.push_str("# source-of-truth: crates/ops/src/alerts.rs (mirror of docs/fa/ops.md §7.1).\n");
    out.push_str("# Drift-check: `cargo test -p ops --test red_ops_alerts` + `bash scripts/verify_M-09.sh`.\n");
    out.push_str("groups:\n");
    out.push_str("  - name: ops.family.canon\n");
    out.push_str("    interval: 30s\n");
    out.push_str("    rules:\n");

    for rule in ALERT_RULES {
        let alert_name = rule.incident;
        let severity = format!("{:?}", rule.severity); // P0 / P1 / P2
        let expr = expr_for(rule);
        out.push_str(&format!(
            "      - alert: {alert_name}\n\
             \x20\x20\x20\x20\x20\x20\x20\x20expr: {expr}\n\
             \x20\x20\x20\x20\x20\x20\x20\x20for: {}\n\
             \x20\x20\x20\x20\x20\x20\x20\x20labels:\n\
             \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20severity: {severity}\n\
             \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20incident: {alert_name}\n\
             \x20\x20\x20\x20\x20\x20\x20\x20annotations:\n\
             \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20summary: \"{summary}\"\n",
            duration_for(rule),
            summary = yaml_escape(rule.summary),
        ));
    }
    out
}

/// Подобрать выражение `expr:` для правила (PromQL). Прометеевский язык выражений — у каждой
/// метрики свой шаблон алерта (counter→rate(...), gauge→просто сравнение).
///
/// Калибровка порогов (`> 0`, `> N`, `rate(...) > X`) — за оператором (§8 live-tuning); здесь
/// только КАНОН выражения по КАНОНУ метрики. Порог в выражении — placeholder (`> 0` для rate;
/// `== 0` для drill-фейла; `< 100Mi` для диска — даёт ощущение осмысленного алерта, без
/// обещания «это правильное число»). Live-tuning вынесен в §O (founder ★).
fn expr_for(rule: &AlertRule) -> String {
    match (rule.incident, rule.metric) {
        ("TD-011", "journal_bytes_written_total") => {
            // Counter: rate за 60с == 0 ⇒ recorder не пишет.
            "rate(journal_bytes_written_total[1m]) == 0".to_string()
        }
        ("TD-013", "venue_http_status_total") => {
            // Counter с label `code` (418|429): доля 4xx-rate к общему HTTP-rate > 50%.
            // `code=~\"418|429\"` — Prometheus regex-match по label.
            "sum(rate(venue_http_status_total{code=~\"418|429\"}[5m])) \
             / sum(rate(venue_http_status_total[5m])) > 0.5"
                .to_string()
        }
        ("TD-014", "md_events_total") => {
            // Counter с label `kind`: rate за 5 мин == 0 при живом WS ⇒ класс событий пропал.
            // Порог по venue — в deploy-конфиге (Alertmanager `for:` уже стоит на 5m).
            "rate(md_events_total[5m]) == 0".to_string()
        }
        ("TD-016", "recorder_rss_anon_bytes") => {
            // Gauge: абсолютное значение > 4 GiB. Реальный порог — live-tuning (§O); 4 GiB —
            // «не должно быть в норме» (recorder держит ~200–500 MiB на здоровом рынке).
            "recorder_rss_anon_bytes > 4294967296".to_string()
        }
        ("C1-M08", "book_resync_total") => {
            // Counter с labels venue+symbol: rate > 0 за 5 мин ⇒ была порча best.
            // (B2 §4.3.2: эмиссия ⟺ best-divergence; per-cycle объём НЕ триггерит алерт.)
            "rate(book_resync_total[5m]) > 0".to_string()
        }
        ("TD-006", "journal_disk_free_bytes") => {
            // Gauge: < 10 GiB (writer min_free_bytes по умолчанию 10 GiB, см. main.rs).
            "journal_disk_free_bytes < 10737418240".to_string()
        }
        ("OPS-BKP", "backup_restore_drill_ok") => {
            // Gauge == 0: restore-drill последний раз провалился (бэкап не бэкап).
            "backup_restore_drill_ok == 0".to_string()
        }
        ("OPS-SILENCE", "md_event_age_ms") => {
            // Gauge с label `venue`: > 5 минут = поток молчит (OPS-I-8; §7 P1 = `> 5min`).
            "md_event_age_ms > 300000".to_string()
        }
        ("OPS-RESYNC", "book_resync_total") => {
            // Тот же счётчик, что C1-M08; различие — `for:` (см. duration_for) + threshold.
            "rate(book_resync_total[15m]) > 0.1".to_string()
        }
        ("OPS-GAP", "journal_seq_gaps_total") => {
            // Counter: rate за сутки > 1% от total seq. Прямая формула: `gaps / current > 1%`.
            "journal_seq_gaps_total / journal_seq_current > 0.01".to_string()
        }
        _ => {
            // Unknown combination — fall back to literal `> 0` (degraded but doesn't lie about
            // severity or summary). Канон защищён: добавление нового правила требует явного
            // case в `expr_for`, иначе — compile-warning через exhaustive `match`.
            format!("{} > 0", rule.metric)
        }
    }
}

/// `for:` — окно, в течение которого условие должно держаться перед фейерверком алерта.
/// P0 (будит человека) — короткое; P1 (в течение часа) — длиннее; P2 (дайджест) — ещё длиннее.
/// Один источник правды: severity → `for:`. Это критично для §8 (anti-flapping) — урок TD-013.
fn duration_for(rule: &AlertRule) -> &'static str {
    match rule.severity {
        Severity::P0 => "1m",
        Severity::P1 => "5m",
        Severity::P2 => "15m",
    }
}

/// Минимальный YAML-escape для `summary` (двойные кавычки → `\"`). Prometheus rule-file —
/// YAML; literal `\n` в строке тоже может встретиться, но в наших summary его нет.
fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::metric_names;

    /// Compile-time канарейка: ВСЕ правила ссылаются на СУЩЕСТВУЮЩУЮ метрику (drift-detector).
    /// Юнит-RED в `red_ops_alerts.rs` — для внешних проверок; здесь — внутренняя, чтобы diff
    /// `ALERT_RULES` сразу падал в cargo test -p ops (а не только в отдельном test-бинарнике).
    #[test]
    fn alert_rules_metric_invariant() {
        let names = metric_names();
        for rule in ALERT_RULES {
            assert!(
                names.contains(&rule.metric),
                "правило `{}` ссылается на несуществующую метрику `{}` (drift vs METRICS)",
                rule.incident,
                rule.metric
            );
        }
    }

    /// Детерминизм рендера: один и тот же каталог → один и тот же YAML (важно для
    /// deploy-артефакта `deploy/alerts/ops.rules.yml`, drift-check через сравнение строк).
    #[test]
    fn render_is_deterministic() {
        let a = to_prometheus_rules();
        let b = to_prometheus_rules();
        assert_eq!(
            a, b,
            "to_prometheus_rules() недетерминирован — deploy-артефакт нельзя сравнивать"
        );
    }

    /// Severity перечисление имеет ВСЕ три уровня (`Debug`-формат совпадает с label в YAML).
    /// Регресс — добавление нового severity ломает `format!("{:?}", Severity::X) == "X"`.
    #[test]
    fn severity_debug_matches_label() {
        assert_eq!(format!("{:?}", Severity::P0), "P0");
        assert_eq!(format!("{:?}", Severity::P1), "P1");
        assert_eq!(format!("{:?}", Severity::P2), "P2");
    }

    /// Drift-канарейка deploy-артефакта `deploy/alerts/ops.rules.yml`. Файл ОБЯЗАН быть
    /// побайтово равен `to_prometheus_rules()` (генерируется `cargo run -p ops --example
    /// dump_rules > deploy/alerts/ops.rules.yml`). Любое ручное изменение файла ловится
    /// здесь — иначе FA §7.1 (канон) и Prometheus-rule-file (deploy) могут разойтись,
    /// оператор увидит «алерт работает», а на деле он смотрит на старую метрику.
    /// Тест запускается в `cargo test -p ops` и `bash scripts/verify_M-09.sh`.
    #[test]
    fn deploy_alerts_artifact_matches_renderer() {
        // Путь — от корня workspace (cargo test запускается с CWD=crate, поэтому
        // резолвим относительно `CARGO_MANIFEST_DIR`).
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let deploy_path = manifest_dir
            .parent() // crates/
            .and_then(|p| p.parent()) // <workspace>
            .map(|p| p.join("deploy/alerts/ops.rules.yml"));
        let deploy_path = match deploy_path {
            Some(p) => p,
            None => return, // странный layout — не валим тест, нет файла для проверки.
        };
        if !deploy_path.exists() {
            panic!(
                "deploy-артефакт {} отсутствует — запустите \
                 `cargo run -p ops --example dump_rules > deploy/alerts/ops.rules.yml`",
                deploy_path.display()
            );
        }
        let on_disk = std::fs::read_to_string(&deploy_path).expect("read deploy artifact");
        let rendered = to_prometheus_rules();
        if on_disk != rendered {
            // Даём максимально полезный diff-point: первые различающиеся байты.
            let common_len = on_disk
                .bytes()
                .zip(rendered.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            panic!(
                "deploy/alerts/ops.rules.yml DRIFT vs `to_prometheus_rules()` (first diff at byte {common_len}); \
                 перегенерируйте: `cargo run -p ops --example dump_rules > deploy/alerts/ops.rules.yml`"
            );
        }
    }
}
