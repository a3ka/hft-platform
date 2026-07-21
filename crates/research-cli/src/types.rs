//! T2-словарь research-cli (FA §3) + T1-формы отчёта/ledger-записи.
//!
//! TrialRecord/ValidationReport — T1 per docs/05-contract-layer.md §2; их Rust-типы
//! временно живут здесь (единственный продюсер/консюмер), JSON несёт
//! report_schema_version. Промоушен в crates/contracts — отдельный contract-RFC
//! (M-04 «Contract impact»; TECH-DEBT при merge).

use serde::{Deserialize, Serialize};

pub const REPORT_SCHEMA_VERSION: u32 = 2;
pub const TRIALS_LEDGER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitKind {
    Train,
    Val,
    Test,
}

/// Режим издержек прогона (FA §5): стресс — ОТДЕЛЬНЫЙ прогон через sim, не пост-обработка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostsMode {
    Baseline,
    /// Издержки ×1.5.
    CostX15,
    /// Латентность ×2.
    LatencyX2,
}

/// Единица trials-ledger (T1, append-only; D8: hash-chain).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrialRecord {
    pub schema_version: u32,
    /// Семейство сигналов (для deflated-Sharpe счётчика), напр. "obi".
    pub signal_family: String,
    pub signal_id: String,
    /// sha256(канонический JSON параметров ячейки + costs_mode) — стресс-прогон
    /// обязан дать ОТЛИЧНЫЙ хэш (RC-I-10).
    pub params_hash: String,
    pub split: SplitKind,
    pub costs_mode: CostsMode,
    pub ts_wall_ms: i64,
    pub code_hash: String,
    /// Ссылка на результат (файл/идентификатор) ИЛИ "KILL" — отрицательные
    /// результаты не удаляются (RC-I-9).
    pub result_ref: String,
    /// Sharpe ячейки (для V[SR] семейства в deflated-формуле D4); None если прогон упал.
    pub sharpe: Option<f64>,
    /// sha256 предыдущей записи ledger'а (D8); у первой — "genesis".
    pub prev_sha256: String,
}

/// Явное состояние time-split (FA §3): границы + «test уже тронут?» как данные.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeSplit {
    pub train_ms: (i64, i64),
    pub val_ms: (i64, i64),
    pub test_ms: (i64, i64),
}

/// Скользящее окно walk-forward (FA §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalkForwardWindow {
    pub train_window_ms: i64,
    pub test_window_ms: i64,
    pub step_ms: i64,
}

/// Спецификация грида (FA §3): ячейки параметров сигнала + режим издержек.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridSpec {
    pub signal_family: String,
    pub signal_id_prefix: String,
    /// Параметры ячеек — JSON, десериализуемые сигналом (obi::ObiParams).
    pub cells: Vec<serde_json::Value>,
    pub costs_mode: CostsMode,
    pub seed: u64,
}

/// Результат одной ячейки грида.
#[derive(Debug, Clone, PartialEq)]
pub struct CellResult {
    pub params: serde_json::Value,
    pub params_hash: String,
    pub net_pnl_e8: i64,
    pub sharpe: f64,
    pub max_drawdown_e8: i64,
    pub intents: usize,
    pub fills: usize,
    pub turnover_e8: i64,
    /// Пошаговые доходности позиции (для Sharpe/DSR).
    pub returns: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StressResult {
    pub mode: CostsMode,
    pub sharpe: f64,
    pub net_pnl_e8: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Kill(String),
    Inconclusive(String),
    Pass,
}

impl Serialize for Verdict {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Kill(reason) => serializer.serialize_str(&format!("Kill: {reason}")),
            Self::Inconclusive(reason) => {
                serializer.serialize_str(&format!("Inconclusive: {reason}"))
            }
            Self::Pass => serializer.serialize_str("Pass"),
        }
    }
}

impl<'de> Deserialize<'de> for Verdict {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "Pass" {
            return Ok(Self::Pass);
        }
        if let Some(reason) = value.strip_prefix("Kill: ") {
            return Ok(Self::Kill(reason.to_string()));
        }
        if let Some(reason) = value.strip_prefix("Inconclusive: ") {
            return Ok(Self::Inconclusive(reason.to_string()));
        }
        Err(serde::de::Error::custom(
            "verdict должен быть Pass, Kill: <reason> или Inconclusive: <reason>",
        ))
    }
}

/// Финальный детерминированный отчёт (T1; RC-I-5: байт-идентичен при тех же входах —
/// НИКАКИХ wall-clock полей внутри).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub report_schema_version: u32,
    pub hypothesis: String,
    pub signal_id: String,
    pub params: serde_json::Value,
    /// Входы, делающие отчёт воспроизводимым доказательством (FA §7).
    pub journal_sha256: String,
    pub code_hash: String,
    /// Счётчик семейства из ГЛОБАЛЬНОГО ledger'а на момент запуска (RC-I-3).
    pub ledger_n: u64,
    pub net_pnl_e8: i64,
    pub sharpe: f64,
    pub deflated_sharpe: f64,
    pub max_drawdown_e8: i64,
    pub fill_rate: f64,
    pub turnover_e8: i64,
    pub capacity_notional_e8: i64,
    /// Методика capacity (D5): "v1-participation".
    pub capacity_method: String,
    /// (horizon_ms, sharpe) — decay-кривая по горизонтам.
    pub decay: Vec<(i64, f64)>,
    pub stress: Vec<StressResult>,
    pub walkforward_sharpes: Vec<f64>,
    /// Календарная длина окна, на котором считался отчёт (KS-I-1/5).
    pub data_span_days: f64,
    /// Стандартная ошибка годового Sharpe (KS-I-1).
    pub se_sharpe: f64,
    /// Машинный kill-screen вердикт (KS-I-4).
    pub verdict: Verdict,
    /// Ссылка на E8 gap-артефакт именно этого окна (KS-I-5).
    pub gap_ref: String,
    /// Граница эпохи trials-ledger (TD-015), например `5141fd9`.
    pub ledger_cutoff: String,
}

#[derive(Debug)]
pub enum RcError {
    Io(std::io::Error),
    Parse(String),
    /// Повреждённый сегмент/цепочка — abort, никаких частичных результатов (FA §3).
    CorruptInput(String),
    /// Отказ ledger-записи → abort ВСЕГО прогона (FA §3).
    LedgerWrite(String),
    Sim(sim::SimError),
    Signal(String),
    /// Пре-регистрация не найдена/не заполнена (FA §8.1).
    PreRegistrationMissing(String),
    GateDenied(String),
}
