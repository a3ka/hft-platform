//! data_quality — E8: разрывы записи как ПЕРВОКЛАССНЫЙ артефакт (M-08, carve-out A3).
//!
//! Каркас (типы + сигнатуры + `todo!()`) — architect; реализация — research-dev (task 5).
//! Заведён по находке critic C-005 M1: E8 обещался, но не имел ни одного оракула.
//!
//! Зачем: recorder перезапускался 31 раз за цикл M-05/M-06 (деплои/реверты), а WS-коннекты
//! рвутся штатно. Метрика, посчитанная по дырявым данным, врёт МОЛЧА: пропущенные минуты
//! выглядят как «рынок не двигался». Поэтому любой отчёт (`research/reports/R-NNN`) обязан
//! ссылаться на gap-артефакт своей эпохи — иначе он не воспроизводим и не проверяем.
//!
//! Реализация — BUILT на `journal::stream` (E5/E6), без материализации в `Vec<Event>`.
//! На боевом журнале 8.3 GB последняя бы OOM-нула машину (класс TD-011). Стрим даёт
//! O(1) памяти по размеру журнала.

use std::fs;

use contracts::{Event, EventKind, SysEvent};

use crate::grid::JournalSource;
use crate::types::RcError;

/// Один разрыв записи.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Доля времени, покрытая данными: 1 − gap_total/(last−first), в fixed-point ×1e8.
    pub coverage_e8: i64,
}

pub const GAP_REPORT_SCHEMA_VERSION: u32 = 1;
/// Дефолтный порог разрыва (мс). Мид-фреквентный поток даёт события каждые доли секунды;
/// 5 секунд тишины — уже дыра, а не рынок.
pub const DEFAULT_GAP_THRESHOLD_MS: i64 = 5_000;

/// Посчитать разрывы по журналу через `journal::stream` (E5: bounded-memory).
///
/// Эпохи берутся из `SegmentHeader.headers()` сегментов, прошедших `EpochFilter`.
/// Тихая дыра (между двумя не-Sys событиями, разрыв > порога) и реконнект-обрамлённая
/// (между `Sys::ConnDown` и `Sys::ConnUp`) находятся ОБЕ; вторая помечается
/// `bounded_by_conn_events = true` — критично, потому что отчёт по ней можно строить
/// (знаем причину), а по тихой — нет (причина утеряна).
///
/// Контракт `duration_ms`: **честная разница wall-clock** между соседними событиями
/// (`to_wall_ms − from_wall_ms`). Без вычетов «активного периода» или иных поправок:
/// они занижали бы реальные дыры на проде (SVR research-dev в red_gaps — прежний
/// «semantic hack» ради сломанного оракула в архитектурском тесте отвергнут).
pub fn gaps(source: &JournalSource, gap_threshold_ms: i64) -> Result<GapReport, RcError> {
    // EpochFilter обязан быть НАЗВАН (CT-RFC02-2): вендор/синтетика не подмешиваются молча.
    let stream = journal::stream(&source.dir, source.filter.clone()).map_err(RcError::Io)?;

    // Эпохи читаем из заголовков стрима (CT-RFC02-2: provenance читаемо доносится до отчёта).
    // Дедуп + сортировка → детерминированный JSON.
    let mut epoch_ids: Vec<String> = stream
        .headers()
        .iter()
        .map(|header| header.epoch_id.clone())
        .collect();
    epoch_ids.sort();
    epoch_ids.dedup();

    let mut events_total: u64 = 0;
    let mut first_wall_ms: i64 = 0;
    let mut last_wall_ms: i64 = 0;
    let mut prev: Option<Event> = None;
    let mut gaps: Vec<Gap> = Vec::new();

    for result in stream {
        let event = result.map_err(RcError::Io)?;
        events_total += 1;
        if events_total == 1 {
            first_wall_ms = event.ts_wall_ms;
        }
        last_wall_ms = event.ts_wall_ms;

        if let Some(previous) = prev.take() {
            // Контракт: duration_ms = ЧИСТАЯ разница wall-clock между соседними событиями.
            // Никаких вычетов «активного периода» / поправок на интер-event spacing —
            // они ЗАНИЖАЮТ реальные дыры на проде (например, реальный 30-секундный gap
            // между батчами трейдов по 1с превратился бы в 29с — отчёт врёт).
            let duration_ms = event.ts_wall_ms - previous.ts_wall_ms;
            if duration_ms >= gap_threshold_ms {
                // Дыра «обрамлена Conn-событиями», если с одной стороны ConnDown,
                // с другой — ConnUp. Тихая дыра — обе границы не-Sys → bounded=false.
                let prev_is_conn_down =
                    matches!(&previous.kind, EventKind::Sys(SysEvent::ConnDown(_)));
                let next_is_conn_up = matches!(&event.kind, EventKind::Sys(SysEvent::ConnUp(_)));
                gaps.push(Gap {
                    from_seq: previous.seq,
                    from_wall_ms: previous.ts_wall_ms,
                    to_seq: event.seq,
                    to_wall_ms: event.ts_wall_ms,
                    duration_ms,
                    bounded_by_conn_events: prev_is_conn_down || next_is_conn_up,
                });
            }
        }
        prev = Some(event);
    }

    let gap_total_ms: i64 = gaps.iter().map(|gap| gap.duration_ms).sum();
    let span_ms = last_wall_ms - first_wall_ms;
    let coverage_e8 = if span_ms > 0 {
        // (1 − gap_total/span) × 1e8 — fixed-point; отрицательного результата быть не
        // может: сумма gap'ов не превышает span (каждая дыра вложена в span), но клемпим
        // на всякий случай (если первое и последнее события внезапно совпадут по wall_ms).
        let raw = (span_ms - gap_total_ms) as i128;
        let coverage = raw
            .saturating_mul(contracts::PRICE_SCALE as i128)
            .checked_div(span_ms as i128)
            .unwrap_or(contracts::PRICE_SCALE as i128);
        coverage.clamp(0, contracts::PRICE_SCALE as i128) as i64
    } else {
        // span == 0 (ровно одно событие или пусто): coverage = 1 — «окно определено».
        contracts::PRICE_SCALE
    };

    Ok(GapReport {
        schema_version: GAP_REPORT_SCHEMA_VERSION,
        epoch_ids,
        events_total,
        first_wall_ms,
        last_wall_ms,
        gap_threshold_ms,
        gaps,
        gap_total_ms,
        coverage_e8,
    })
}

/// Записать артефакт `research/data-quality/gaps-<epoch>.json` (детерминированный JSON:
/// serde_json печатает поля struct в порядке объявления, `Vec` — в порядке элементов;
/// никаких wall-clock полей о моменте генерации → байт-идентично при тех же входах, RC-I-5).
///
/// Один файл — одна эпоха: иначе отчёт неоднозначен (какой эпохе принадлежит).
pub fn write_gap_artifact(report: &GapReport, dir: &std::path::Path) -> Result<(), RcError> {
    if report.epoch_ids.len() != 1 {
        return Err(RcError::Parse(format!(
            "write_gap_artifact: ожидается ровно одна эпоха, получено {} \
             (смешение эпох в одном файле делает отчёт неоднозначным — \
              сначала split по EpochFilter::Explicit(epoch_ids))",
            report.epoch_ids.len()
        )));
    }
    fs::create_dir_all(dir).map_err(RcError::Io)?;
    let path = dir.join(format!("gaps-{}.json", report.epoch_ids[0]));
    let mut body =
        serde_json::to_string_pretty(report).map_err(|error| RcError::Parse(error.to_string()))?;
    body.push('\n');
    fs::write(&path, body).map_err(RcError::Io)?;
    Ok(())
}
