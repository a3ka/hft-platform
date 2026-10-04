//! report — детерминированная генерация ValidationReport/metrics.json (FA §7).
//! RC-I-5: те же входы → байт-идентичный metrics.json. НИКАКИХ wall-clock/HashMap-
//! итераций в сериализуемом составе. Нарратив R-NNN.md — шаблонная генерация из
//! чисел (без LLM) — интерпретация остаётся критику/человеку.
//!
//! Пре-регистрация (FA §8.1): финальная валидация ОТКАЗЫВАЕТСЯ работать без карточки
//! research/hypotheses/H-*.md с заполненным разделом «критерии фальсификации».
//!
//! Реализация — research-dev (M-04 task 4).

use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::types::{RcError, ValidationReport};

/// Имя сегмента журнала (зеркалит приватную константу `journal::SEGMENT` — тот же
/// формат имени файла; research-cli читает журнал read-only, без writer-хэндла, RC-I-7).
const JOURNAL_SEGMENT_FILE: &str = "segment-00000000.jrnl";

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// sha256 файла сегмента журнала (вход воспроизводимости отчёта).
pub fn journal_sha256(journal_dir: &Path) -> Result<String, RcError> {
    let path = journal_dir.join(JOURNAL_SEGMENT_FILE);
    let bytes = fs::read(&path).map_err(RcError::Io)?;
    Ok(sha256_hex(&bytes))
}

/// Проверить пре-регистрацию: карточка существует и содержит непустой раздел
/// критериев фальсификации (грепается заголовок «критерии фальсификации»).
pub fn require_preregistration(hypothesis_card: &Path) -> Result<(), RcError> {
    let content = fs::read_to_string(hypothesis_card).map_err(|e| {
        RcError::PreRegistrationMissing(format!(
            "{}: карточка не найдена ({e})",
            hypothesis_card.display()
        ))
    })?;
    let lower = content.to_lowercase();
    const MARKER: &str = "критерии фальсификации";
    let idx = lower.find(MARKER).ok_or_else(|| {
        RcError::PreRegistrationMissing(format!(
            "{}: раздел «критерии фальсификации» не найден",
            hypothesis_card.display()
        ))
    })?;

    // Всё после заголовка (пропускаем остаток строки заголовка), до следующего "## "
    // заголовка или конца файла — там ищем хотя бы один непустой пункт (буллет).
    let after = &content[idx + MARKER.len()..];
    let has_bullet = after
        .lines()
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with("## "))
        .any(|l| {
            let t = l.trim();
            !t.is_empty()
                && (t.starts_with('-')
                    || t.starts_with('*')
                    || t.starts_with(|c: char| c.is_ascii_digit()))
        });
    if !has_bullet {
        return Err(RcError::PreRegistrationMissing(format!(
            "{}: раздел критериев фальсификации пуст",
            hypothesis_card.display()
        )));
    }
    Ok(())
}

/// Записать metrics.json детерминированно (serde_json по фиксированному порядку
/// полей структуры; повторный вызов с тем же отчётом → байт-идентичный файл).
pub fn write_metrics_json(report: &ValidationReport, path: &Path) -> Result<(), RcError> {
    let mut json =
        serde_json::to_string_pretty(report).map_err(|e| RcError::Parse(e.to_string()))?;
    json.push('\n');
    fs::write(path, json).map_err(RcError::Io)
}

/// Шаблонный нарратив R-NNN.md из чисел отчёта (детерминированный, без LLM).
pub fn write_narrative_md(report: &ValidationReport, path: &Path) -> Result<(), RcError> {
    let mut s = String::new();
    s.push_str(&format!(
        "# {} — {}\n\n",
        report.hypothesis, report.signal_id
    ));
    s.push_str(&format!(
        "- report_schema_version: {}\n",
        report.report_schema_version
    ));
    s.push_str(&format!("- journal_sha256: {}\n", report.journal_sha256));
    s.push_str(&format!("- code_hash: {}\n", report.code_hash));
    s.push_str(&format!(
        "- ledger_n (счётчик семейства): {}\n",
        report.ledger_n
    ));
    s.push_str(&format!("- net_pnl_e8: {}\n", report.net_pnl_e8));
    s.push_str(&format!("- sharpe: {:.6}\n", report.sharpe));
    s.push_str(&format!(
        "- deflated_sharpe: {:.6}\n",
        report.deflated_sharpe
    ));
    s.push_str(&format!("- max_drawdown_e8: {}\n", report.max_drawdown_e8));
    s.push_str(&format!("- fill_rate: {:.6}\n", report.fill_rate));
    s.push_str(&format!("- turnover_e8: {}\n", report.turnover_e8));
    s.push_str(&format!(
        "- capacity_notional_e8: {} ({})\n",
        report.capacity_notional_e8, report.capacity_method
    ));

    s.push_str("\n## Decay (horizon_ms, sharpe)\n");
    for (h, sh) in &report.decay {
        s.push_str(&format!("- {h}ms: {sh:.6}\n"));
    }

    s.push_str("\n## Stress\n");
    for st in &report.stress {
        s.push_str(&format!(
            "- {:?}: sharpe={:.6} net_pnl_e8={}\n",
            st.mode, st.sharpe, st.net_pnl_e8
        ));
    }

    s.push_str("\n## Walk-forward Sharpes\n");
    for sh in &report.walkforward_sharpes {
        s.push_str(&format!("- {sh:.6}\n"));
    }

    fs::write(path, s).map_err(RcError::Io)
}
