//! report — детерминированная генерация ValidationReport/metrics.json (FA §7).
//! RC-I-5: те же входы → байт-идентичный metrics.json. НИКАКИХ wall-clock/HashMap-
//! итераций в сериализуемом составе. Нарратив R-NNN.md — шаблонная генерация из
//! чисел (без LLM) — интерпретация остаётся критику/человеку.
//!
//! Пре-регистрация (FA §8.1): финальная валидация ОТКАЗЫВАЕТСЯ работать без карточки
//! research/hypotheses/H-*.md с заполненным разделом «критерии фальсификации».
//!
//! Реализация — research-dev (M-04 task 4).

use std::path::Path;

use crate::types::{RcError, ValidationReport};

/// sha256 файла сегмента журнала (вход воспроизводимости отчёта).
pub fn journal_sha256(journal_dir: &Path) -> Result<String, RcError> {
    let _ = journal_dir;
    todo!("research-dev: M-04 task 4")
}

/// Проверить пре-регистрацию: карточка существует и содержит непустой раздел
/// критериев фальсификации (грепается заголовок «критерии фальсификации»).
pub fn require_preregistration(hypothesis_card: &Path) -> Result<(), RcError> {
    let _ = hypothesis_card;
    todo!("research-dev: M-04 task 4")
}

/// Записать metrics.json детерминированно (serde_json по фиксированному порядку
/// полей структуры; повторный вызов с тем же отчётом → байт-идентичный файл).
pub fn write_metrics_json(report: &ValidationReport, path: &Path) -> Result<(), RcError> {
    let _ = (report, path);
    todo!("research-dev: M-04 task 4")
}

/// Шаблонный нарратив R-NNN.md из чисел отчёта (детерминированный, без LLM).
pub fn write_narrative_md(report: &ValidationReport, path: &Path) -> Result<(), RcError> {
    let _ = (report, path);
    todo!("research-dev: M-04 task 4")
}
