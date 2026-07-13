//! data_quality — E8: разрывы записи как ПЕРВОКЛАССНЫЙ артефакт (M-08, carve-out A3).
//!
//! Каркас (типы + сигнатуры + `todo!()`) — architect; реализация — research-dev (task 5).
//! Заведён по находке critic C-005 M1: E8 обещался, но не имел ни одного оракула.
//!
//! Зачем: recorder перезапускался 31 раз за цикл M-05/M-06 (деплои/реверты), а WS-коннекты
//! рвутся штатно. Метрика, посчитанная по дырявым данным, врёт МОЛЧА: пропущенные минуты
//! выглядят как «рынок не двигался». Поэтому любой отчёт (`research/reports/R-NNN`) обязан
//! ссылаться на gap-артефакт своей эпохи — иначе он не воспроизводим и не проверяем.

use serde::{Deserialize, Serialize};

use crate::types::RcError;

/// Один разрыв записи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gap {
    /// Последнее событие ДО разрыва.
    pub from_seq: u64,
    pub from_wall_ms: i64,
    /// Первое событие ПОСЛЕ разрыва.
    pub to_seq: u64,
    pub to_wall_ms: i64,
    pub duration_ms: i64,
    /// Разрыв обрамлён `Sys::ConnDown`/`ConnUp` (штатный реконнект), а не «просто дыра».
    pub bounded_by_conn_events: bool,
}

/// Артефакт `research/data-quality/gaps-<epoch>.json` (детерминирован: никаких wall-clock
/// полей о моменте генерации — иначе отчёт перестаёт быть байт-воспроизводимым, RC-I-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapReport {
    pub schema_version: u32,
    /// Эпохи, по которым считалось (из `SegmentHeader` — эпоху нельзя не назвать).
    pub epoch_ids: Vec<String>,
    pub events_total: u64,
    pub first_wall_ms: i64,
    pub last_wall_ms: i64,
    /// Порог, выше которого пауза считается разрывом.
    pub gap_threshold_ms: i64,
    pub gaps: Vec<Gap>,
    pub gap_total_ms: i64,
    /// Доля времени, покрытая данными: 1 − gap_total/(last−first).
    pub coverage_e8: i64,
}

pub const GAP_REPORT_SCHEMA_VERSION: u32 = 1;
/// Дефолтный порог разрыва (мс). Мид-фреквентный поток даёт события каждые доли секунды;
/// 5 секунд тишины — уже дыра, а не рынок.
pub const DEFAULT_GAP_THRESHOLD_MS: i64 = 5_000;

/// Посчитать разрывы по журналу (bounded-memory: идёт по стриму, не по `Vec<Event>`).
pub fn gaps(
    _source: &crate::grid::JournalSource,
    _gap_threshold_ms: i64,
) -> Result<GapReport, RcError> {
    todo!("M-08 task 5 (research-dev): стрим по журналу → разрывы + coverage")
}

/// Записать артефакт `research/data-quality/gaps-<epoch>.json` (детерминированный JSON).
pub fn write_gap_artifact(_report: &GapReport, _dir: &std::path::Path) -> Result<(), RcError> {
    todo!("M-08 task 5 (research-dev): детерминированная сериализация артефакта")
}
