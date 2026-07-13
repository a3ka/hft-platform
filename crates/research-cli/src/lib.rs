//! research-cli — мост движок↔деск (docs/fa/research-cli.md). Гриды/walk-forward/
//! trials-ledger/метрики/детерминированные отчёты. Анти-оверфит МЕХАНИЗИРОВАН
//! (FA §8): пре-регистрация обязательна, test-сегмент за val-гейтом, каждая ячейка —
//! запись в ledger.
//!
//! Каркас — architect (M-04 task 1); реализация — research-dev (task 4).
//! Инварианты RC-I-1..11 — RED-оракулы в `tests/` (sacred).

pub mod grid;
pub mod ledger;
pub mod metrics;
pub mod report;
pub mod split;
pub mod strategy_cell;
pub mod types;
pub mod walkforward;

pub use ledger::{Ledger, LedgerTrialCount};
pub use split::{SplitState, ValGateToken};
pub use types::*;
