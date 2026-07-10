//! registry — outer-слой (FA §6): загрузка research/registry/signals.json РОВНО ОДИН РАЗ
//! на boot. Read-only консюмер Границы B (запись реестра — снаружи, через подпись).
//!
//! Проверки на загрузке (fail-closed): code_hash (D3: sha256 исходника модуля,
//! SG-I-6) · params-schema (SG-I-8) · retired не инстанцируется (SG-I-7) ·
//! id самосогласован со spec() (SG-I-11).
//!
//! Реализация — signal-engineer (M-04 task 3).

use std::path::Path;

use serde::Deserialize;

use crate::{RegistryStatus, Signal, SignalError};

/// Запись реестра (T1-форма per docs/05-contract-layer.md §2; здесь — read-only зеркало).
#[derive(Debug, Clone, Deserialize)]
pub struct RegistryEntry {
    pub signal_id: String,
    pub version: u32,
    /// Имя модуля-исходника в crates/signals/src/ (напр. "obi" → obi.rs).
    pub module: String,
    /// sha256 байт crates/signals/src/<module>.rs (D3).
    pub code_hash: String,
    pub status: String,
    pub params: serde_json::Value,
    pub ensemble_weight: f64,
}

pub struct LoadedSignal {
    pub id: String,
    pub status: RegistryStatus,
    pub signal: Box<dyn Signal>,
}

/// sha256 исходника модуля сигнала (D3). `src_root` = crates/signals/src.
pub fn module_code_hash(src_root: &Path, module: &str) -> Result<String, SignalError> {
    let _ = (src_root, module);
    todo!("signal-engineer: M-04 task 3")
}

/// Загрузить реестр: candidate|paper|live инстанцируются (после проверок SG-I-6/8/11),
/// retired — пропускаются (SG-I-7). Любой mismatch → Err (Reject boot), не тихий skip.
pub fn load_registry(
    registry_path: &Path,
    src_root: &Path,
) -> Result<Vec<LoadedSignal>, SignalError> {
    let _ = (registry_path, src_root);
    todo!("signal-engineer: M-04 task 3")
}

/// Инстанцировать один сигнал по записи (match по module: "obi" → Obi::from_json_params).
/// Обязан вернуть сигнал, чей spec().id == entry.signal_id (SG-I-11), иначе Err.
pub fn instantiate(entry: &RegistryEntry) -> Result<Box<dyn Signal>, SignalError> {
    let _ = entry;
    todo!("signal-engineer: M-04 task 3")
}
