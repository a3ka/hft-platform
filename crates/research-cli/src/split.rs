//! split — time-split дисциплина (FA §3/§8): test-сегмент СТРУКТУРНО недостижим до
//! прохождения val-гейта (RC-I-8 — токен без публичного конструктора), трогается не
//! более одного раза без явного override+обоснования (RC-I-4).
//!
//! Реализация — research-dev (M-04 task 4).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::types::{RcError, TimeSplit};

/// Доказательство прохождения val-гейта. Поле приватно, публичного конструктора НЕТ:
/// код, читающий test-диапазон, требует &ValGateToken — получить его можно только из
/// SplitState::pass_val_gate (RC-I-8, компиляционная граница).
pub struct ValGateToken {
    _priv: (),
}

/// Персистентное состояние прогона гипотезы (research/state/<hypothesis>.split.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitState {
    pub hypothesis: String,
    pub split: TimeSplit,
    pub val_gate_passed: bool,
    pub test_touched: bool,
    /// Аудит-лог касаний test (включая override-обоснования).
    pub touch_log: Vec<String>,
}

impl SplitState {
    pub fn new(hypothesis: &str, split: TimeSplit) -> Self {
        Self {
            hypothesis: hypothesis.to_string(),
            split,
            val_gate_passed: false,
            test_touched: false,
            touch_log: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, RcError> {
        let _ = path;
        todo!("research-dev: M-04 task 4")
    }

    pub fn save(&self, path: &Path) -> Result<(), RcError> {
        let _ = path;
        todo!("research-dev: M-04 task 4")
    }

    /// Val-гейт: критерии пре-регистрации к val-результатам. Провал → Err (токена нет).
    pub fn pass_val_gate(
        &mut self,
        val_sharpe: f64,
        min_val_sharpe: f64,
    ) -> Result<ValGateToken, RcError> {
        let _ = (val_sharpe, min_val_sharpe);
        todo!("research-dev: M-04 task 4")
    }

    /// Выдать test-диапазон. Второе касание БЕЗ override_reason → Err::GateDenied;
    /// с override — Ok + причина в touch_log (аудит, FA §3 таблица).
    pub fn touch_test(
        &mut self,
        _proof: &ValGateToken,
        override_reason: Option<&str>,
    ) -> Result<(i64, i64), RcError> {
        let _ = override_reason;
        todo!("research-dev: M-04 task 4")
    }
}
