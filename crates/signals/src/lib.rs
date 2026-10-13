//! signals — Граница A: единственная зона записи квант-агентов (docs/fa/signals.md).
//!
//! Архитектурный каркас (типы+трейт) — architect (M-04 task 1, sacred-контракт).
//! Реализации (`bank`, `registry`, `obi`) — signal-engineer (M-04 task 3).
//!
//! Инварианты SG-I-1..11 — RED-оракулы в `tests/` (sacred, architect-only).
//! Решения M-04 (milestones/M-04-research-core.md): D1 (value = направленный score
//! ×1e8 ∈ [-1e8,+1e8]), D2 (horizon_ms — метаданные для downstream), D3 (code_hash =
//! sha256 исходника модуля), D9 (OBI держит book::OrderBook, примитивы полос — book).

pub mod bank;
pub mod obi;
pub mod registry;

use contracts::Event;

/// Масштаб value (D1): ±1e8 = ±1.0 направленного score.
pub const SIGNAL_VALUE_SCALE: i64 = contracts::PRICE_SCALE;

/// T2: единственная форма, которую `signals` отдаёт наружу (FA §3).
#[derive(Debug, Clone, PartialEq)]
pub struct SignalOut {
    pub signal_id: SignalId,
    pub ts_event_mono_ns: u64,
    /// Направленный score ×1e8 ∈ [-SIGNAL_VALUE_SCALE, +SIGNAL_VALUE_SCALE] (D1).
    pub value: i64,
    pub status: RegistryStatus,
    pub meta: SignalMeta,
}

/// Метаданные сигнала для downstream-консюмеров (D2: horizon — не ответственность signals).
#[derive(Debug, Clone, PartialEq)]
pub struct SignalMeta {
    pub horizon_ms: i64,
}

/// Newtype над String, формат `S-NNN-<slug>` (FA §3). Голая строка запрещена —
/// опечатка в id не должна тихо создать «новый» сигнал.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignalId(String);

impl SignalId {
    /// Валидация формата `S-NNN-<slug>` (NNN — 3 цифры; slug — сегменты `[a-z0-9]+`,
    /// разделённые одиночным `-`, без ведущего/хвостового/двойного дефиса). Ручная
    /// побайтовая проверка (без regex-крейта — меньше зависимостей).
    pub fn parse(s: &str) -> Result<Self, SignalError> {
        if !s.is_ascii() {
            return Err(SignalError::InvalidId(s.to_string()));
        }
        let bytes = s.as_bytes();
        // Минимум: "S-000-a" = 7 байт.
        if bytes.len() < 7 {
            return Err(SignalError::InvalidId(s.to_string()));
        }
        if bytes[0] != b'S' || bytes[1] != b'-' {
            return Err(SignalError::InvalidId(s.to_string()));
        }
        if !bytes[2].is_ascii_digit() || !bytes[3].is_ascii_digit() || !bytes[4].is_ascii_digit() {
            return Err(SignalError::InvalidId(s.to_string()));
        }
        if bytes[5] != b'-' {
            return Err(SignalError::InvalidId(s.to_string()));
        }
        // Safe: s.is_ascii() verified above, so byte index 6 is a char boundary.
        let slug = &s[6..];
        if slug.is_empty() {
            return Err(SignalError::InvalidId(s.to_string()));
        }
        let mut prev_hyphen = true; // запрещает ведущий дефис в slug
        for b in slug.bytes() {
            match b {
                b'a'..=b'z' | b'0'..=b'9' => prev_hyphen = false,
                b'-' => {
                    if prev_hyphen {
                        // двойной или ведущий дефис
                        return Err(SignalError::InvalidId(s.to_string()));
                    }
                    prev_hyphen = true;
                }
                _ => return Err(SignalError::InvalidId(s.to_string())),
            }
        }
        if prev_hyphen {
            // хвостовой дефис
            return Err(SignalError::InvalidId(s.to_string()));
        }
        Ok(SignalId(s.to_string()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Зеркало поля `status` из signals.json (FA §3). `signals` читает и пробрасывает,
/// но не присваивает.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryStatus {
    Candidate,
    Paper,
    Live,
    Retired,
}

/// Ссылка-хэндл на карточку `research/specs/S-NNN-<name>.md` (FA §3).
#[derive(Debug, Clone, PartialEq)]
pub struct SignalSpecRef {
    pub id: SignalId,
    pub version: u32,
}

/// Граница A (FA §5). Единственный вход — `&Event`; никаких часов/сети/файлов/будущего.
pub trait Signal {
    fn on_event(&mut self, ev: &Event) -> Option<SignalOut>;
    fn spec(&self) -> SignalSpecRef;
}

#[derive(Debug)]
pub enum SignalError {
    InvalidId(String),
    CodeHashMismatch {
        signal_id: String,
        expected: String,
        actual: String,
    },
    InvalidParams(String),
    UnknownModule(String),
    IdMismatch {
        registry: String,
        spec: String,
    },
    Io(std::io::Error),
    Parse(String),
}
