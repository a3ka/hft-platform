//! trials-ledger — глобальный append-only счётчик попыток (FA §6; INTG-I-6/RC-I-2).
//!
//! ЕДИНСТВЕННЫЙ писатель — этот модуль; файл открывается ТОЛЬКО O_APPEND; API
//! удаления/перезаписи записи НЕ СУЩЕСТВУЕТ (структурно). Формат: JSON Lines,
//! каждая строка — TrialRecord; hash-chain (D8): prev_sha256 = sha256 предыдущей
//! строки-байт; первая запись — "genesis".
//!
//! Реализация — research-dev (M-04 task 4).

use std::io;
use std::path::{Path, PathBuf};

use crate::types::TrialRecord;

/// Счётчик попыток семейства. Поля ПРИВАТНЫ: единственный конструктор —
/// Ledger::trial_count (RC-I-3: deflated-Sharpe не может получить N из литерала).
#[derive(Debug, Clone)]
pub struct LedgerTrialCount {
    n: u64,
    family: String,
}

impl LedgerTrialCount {
    pub fn n(&self) -> u64 {
        self.n
    }
    pub fn family(&self) -> &str {
        &self.family
    }
    // dead_code до реализации trial_count (research-dev task 4 использует и снимает allow)
    #[allow(dead_code)]
    pub(crate) fn new_from_ledger(n: u64, family: String) -> Self {
        Self { n, family }
    }
}

pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    /// Открыть/создать ledger. Все записи идут через append (O_APPEND на уровне
    /// файловой операции).
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Дописать запись (проставляет prev_sha256 сам по последней строке файла).
    /// Отказ записи → Err: вызывающий грид ОБЯЗАН abort-нуть весь прогон (FA §3).
    pub fn append(&mut self, rec: TrialRecord) -> io::Result<()> {
        let _ = rec;
        todo!("research-dev: M-04 task 4")
    }

    pub fn read_all(&self) -> io::Result<Vec<TrialRecord>> {
        todo!("research-dev: M-04 task 4")
    }

    /// Проверка hash-chain целостности (D8): каждая запись ссылается на sha256
    /// предыдущей строки. Ручное редактирование файла в обход инструмента → false.
    pub fn verify_chain(&self) -> io::Result<bool> {
        todo!("research-dev: M-04 task 4")
    }

    /// ЕДИНСТВЕННЫЙ источник N для deflated-Sharpe (RC-I-3).
    pub fn trial_count(&self, family: &str) -> io::Result<LedgerTrialCount> {
        let _ = family;
        todo!("research-dev: M-04 task 4")
    }

    /// Sharpe-ряд семейства (для V[SR] в формуле D4).
    pub fn family_sharpes(&self, family: &str) -> io::Result<Vec<f64>> {
        let _ = family;
        todo!("research-dev: M-04 task 4")
    }
}
