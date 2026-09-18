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
use sha2::{Digest, Sha256};

use crate::{RegistryStatus, Signal, SignalError, SignalId};

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
    let path = src_root.join(format!("{module}.rs"));
    let bytes = std::fs::read(&path).map_err(SignalError::Io)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Парсит статус реестра (`candidate|paper|live|retired`); неизвестное значение —
/// боевой отказ загрузки (fail-closed), не тихий default.
fn parse_status(s: &str) -> Result<RegistryStatus, SignalError> {
    match s {
        "candidate" => Ok(RegistryStatus::Candidate),
        "paper" => Ok(RegistryStatus::Paper),
        "live" => Ok(RegistryStatus::Live),
        "retired" => Ok(RegistryStatus::Retired),
        other => Err(SignalError::Parse(format!(
            "unknown RegistryStatus `{other}`"
        ))),
    }
}

/// Загрузить реестр: candidate|paper|live инстанцируются (после проверок SG-I-6/8/11),
/// retired — пропускаются (SG-I-7). Любой mismatch → Err (Reject boot), не тихий skip.
pub fn load_registry(
    registry_path: &Path,
    src_root: &Path,
) -> Result<Vec<LoadedSignal>, SignalError> {
    let bytes = std::fs::read(registry_path).map_err(SignalError::Io)?;
    let entries: Vec<RegistryEntry> =
        serde_json::from_slice(&bytes).map_err(|e| SignalError::Parse(e.to_string()))?;

    let mut loaded = Vec::with_capacity(entries.len());
    for entry in &entries {
        let status = parse_status(&entry.status)?;
        if status == RegistryStatus::Retired {
            continue; // SG-I-7: retired не инстанцируется вовсе
        }
        let computed_hash = module_code_hash(src_root, &entry.module)?;
        if computed_hash != entry.code_hash {
            return Err(SignalError::CodeHashMismatch {
                signal_id: entry.signal_id.clone(),
                expected: entry.code_hash.clone(),
                actual: computed_hash,
            });
        }
        let signal = instantiate(entry)?;
        loaded.push(LoadedSignal {
            id: entry.signal_id.clone(),
            status,
            signal,
        });
    }
    Ok(loaded)
}

/// Инстанцировать один сигнал по записи (match по module: "obi" → Obi::from_json_params).
/// Обязан вернуть сигнал, чей spec().id == entry.signal_id (SG-I-11), иначе Err.
pub fn instantiate(entry: &RegistryEntry) -> Result<Box<dyn Signal>, SignalError> {
    let status = parse_status(&entry.status)?;
    let id = SignalId::parse(&entry.signal_id).map_err(|_| {
        SignalError::InvalidParams(format!("invalid signal_id `{}`", entry.signal_id))
    })?;

    let signal: Box<dyn Signal> = match entry.module.as_str() {
        "obi" => Box::new(crate::obi::Obi::from_json_params(
            id,
            entry.version,
            status,
            &entry.params,
        )?),
        other => return Err(SignalError::UnknownModule(other.to_string())),
    };

    if signal.spec().id.as_str() != entry.signal_id {
        return Err(SignalError::IdMismatch {
            registry: entry.signal_id.clone(),
            spec: signal.spec().id.as_str().to_string(),
        });
    }

    Ok(signal)
}
