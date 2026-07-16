//! Слой 7 — наблюдаемость + сверка с биржей (`docs/fa/ops.md`). M-09.
//!
//! СКЕЛЕТ (architect: типы + сигнатуры + `todo!()`). Impl — engine-dev по RED-оракулам
//! `crates/ops/tests/red_ops_*.rs` (sacred). MD-only: НЕ трогает risk/killswitch/oms и не
//! эмитит ордера. Всё здесь — детерминированные чистые функции (recon-компаратор, rate-budget,
//! реестр метрик); REST-fetch снапшота — в `venue-*` (venue-dev), scrape/бэкап — `deploy/`.

pub mod budget;
pub mod metrics;
pub mod recon;
pub mod silence;
