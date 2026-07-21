//! trials-ledger — глобальный append-only счётчик попыток (FA §6; INTG-I-6/RC-I-2).
//!
//! ЕДИНСТВЕННЫЙ писатель — этот модуль; файл открывается ТОЛЬКО O_APPEND; API
//! удаления/перезаписи записи НЕ СУЩЕСТВУЕТ (структурно). Формат: JSON Lines,
//! каждая строка — TrialRecord; hash-chain (D8): prev_sha256 = sha256 предыдущей
//! строки-байт; первая запись — "genesis".
//!
//! Реализация — research-dev (M-04 task 4).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

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
    pub(crate) fn new_from_ledger(n: u64, family: String) -> Self {
        Self { n, family }
    }
}

pub struct Ledger {
    path: PathBuf,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Сырые непустые строки файла (байты, без завершающего \n), в порядке появления.
/// Используется как для дописывания (последняя строка → prev_sha256), так и для
/// сверки hash-chain (D8).
fn raw_lines(content: &[u8]) -> Vec<&[u8]> {
    content
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect()
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
    ///
    /// МЕХАНИЗМ (FA §6, RC-I-2): файл открывается ТОЛЬКО через
    /// `OpenOptions::append(true).create(true)` — ни один путь этого модуля не
    /// открывает файл на запись/усечение иначе; API удаления/перезаписи
    /// существующей записи структурно отсутствует.
    pub fn append(&mut self, mut rec: TrialRecord) -> io::Result<()> {
        let existing = if self.path.exists() {
            fs::read(&self.path)?
        } else {
            Vec::new()
        };
        let lines = raw_lines(&existing);
        rec.prev_sha256 = match lines.last() {
            Some(l) => sha256_hex(l),
            None => "genesis".to_string(),
        };
        let line = serde_json::to_string(&rec)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let mut f = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)?;
        writeln!(f, "{line}")?;
        Ok(())
    }

    pub fn read_all(&self) -> io::Result<Vec<TrialRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.path)?;
        let mut out = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let rec: TrialRecord = serde_json::from_str(line)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            out.push(rec);
        }
        Ok(out)
    }

    /// Проверка hash-chain целостности (D8): каждая запись ссылается на sha256
    /// предыдущей строки. Ручное редактирование файла в обход инструмента → false.
    pub fn verify_chain(&self) -> io::Result<bool> {
        if !self.path.exists() {
            return Ok(true);
        }
        let content = fs::read(&self.path)?;
        let lines = raw_lines(&content);
        let mut expected_prev = "genesis".to_string();
        for line in &lines {
            let rec: TrialRecord = match serde_json::from_slice(line) {
                Ok(r) => r,
                Err(_) => return Ok(false),
            };
            if rec.prev_sha256 != expected_prev {
                return Ok(false);
            }
            expected_prev = sha256_hex(line);
        }
        Ok(true)
    }

    /// ЕДИНСТВЕННЫЙ источник N для deflated-Sharpe (RC-I-3).
    pub fn trial_count(&self, family: &str) -> io::Result<LedgerTrialCount> {
        let all = self.read_all()?;
        let n = all.iter().filter(|r| r.signal_family == family).count() as u64;
        Ok(LedgerTrialCount::new_from_ledger(n, family.to_string()))
    }

    /// Счётчик post-M-07 эпохи для семейства. Фильтр по code_hash обязателен:
    /// старые записи append-only ledger не переписываются и не могут участвовать
    /// в deflated-Sharpe новой семантики (TD-015).
    pub fn trial_count_for_code_hash(
        &self,
        family: &str,
        code_hash: &str,
    ) -> io::Result<LedgerTrialCount> {
        let all = self.read_all()?;
        let n = all
            .iter()
            .filter(|record| record.signal_family == family && record.code_hash == code_hash)
            .count() as u64;
        Ok(LedgerTrialCount::new_from_ledger(n, family.to_string()))
    }

    /// Sharpe-ряд только указанной code-хэш эпохи (источник V[SR] для DSR).
    pub fn family_sharpes_for_code_hash(
        &self,
        family: &str,
        code_hash: &str,
    ) -> io::Result<Vec<f64>> {
        let all = self.read_all()?;
        Ok(all
            .iter()
            .filter(|record| record.signal_family == family && record.code_hash == code_hash)
            .filter_map(|record| record.sharpe)
            .collect())
    }

    /// Sharpe-ряд семейства (для V[SR] в формуле D4).
    pub fn family_sharpes(&self, family: &str) -> io::Result<Vec<f64>> {
        let all = self.read_all()?;
        Ok(all
            .iter()
            .filter(|r| r.signal_family == family)
            .filter_map(|r| r.sharpe)
            .collect())
    }
}
