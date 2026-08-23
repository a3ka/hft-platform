//! M-09 task 4B — dump `ops::alerts::ALERT_RULES` как Prometheus rule file на stdout.
//!
//! Используется деплоем для материализации `deploy/alerts/ops.rules.yml` из ЕДИНСТВЕННОГО
//! канона (`crates/ops/src/alerts.rs`). Запуск: `cargo run -p ops --example dump_rules >
//! deploy/alerts/ops.rules.yml`. CI-канарейка (drift-detector) живёт в
//! `crates/ops/tests/red_ops_alerts.rs::rendered_rules_carry_metric_and_severity` +
//! shell-паритет в `scripts/verify_M-09.sh` — если рендер и FA §7.1 расходятся, гейт падает.
//!
//! Почему не binary в `crates/ops/src/bin/` — example проще в запуске (`cargo run --example`)
//! и не требует отдельного `[[bin]]`-объявления. `examples/` — канон cargo-крейта.
//!
//! Замечание: пример не зависит от tokio/net/IO; читает только `ALERT_RULES` и пишет в stdout.

fn main() {
    let rules = ops::alerts::to_prometheus_rules();
    // stdout — pipe-friendly; deploy-скрипт перенаправляет в файл.
    print!("{rules}");
}
