//! Watchdog — live cron-мост «heartbeat/cron-маркеры/docker-состояние → человекочитаемый
//! алерт» (founder-задача 2026-07-31, мост к M-43 task#3 "минимальный cron-watchdog→Telegram").
//!
//! **Отдельный namespace от `alerts::ALERT_RULES`.** Тот каталог — канон БУДУЩИХ
//! Prometheus/Alertmanager правил, parity-locked к `docs/fa/ops.md §7.1` и
//! `scripts/verify_M-09.sh` (architect-only, я их не трогаю). Этот модуль — работающий
//! СЕЙЧАС мост без Prometheus: коды инцидентов начинаются с `WD-` (watchdog), чтобы не
//! создавать иллюзию, что они входят в тот парный канон.
//!
//! Всё здесь — ЧИСТЫЕ функции: часы (`now_ms`) и все данные — параметры, никакого
//! `SystemTime::now()`/`docker`-вызовов/файлового I/O внутри модуля (I/O — в
//! `src/bin/ops-watchdog.rs`). Это то же разделение "чистая логика / грязный wiring",
//! что у `server.rs` (`http_response` чистый, socket-accept — в `recorder`).
//!
//! Контракт «один тест на условие» + анти-плацебо парность (здоровый вход → `None`,
//! больной вход → `Some(Alert)`) — обе стороны покрыты `crates/ops/tests/red_ops_watchdog.rs`.

use serde::{Deserialize, Serialize};

/// Код инцидента watchdog'а (стабильный, используется как ключ дедупликации в
/// `state::WatchdogState` и как anchor рантайм-сообщения, см. `format::format_alert`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Incident {
    HeartbeatMissing,
    HeartbeatStale,
    NotWritable,
    SeqStalled,
    /// `next_seq` УМЕНЬШИЛСЯ относительно якоря прогресса (R-008 F-8) — том журнала
    /// пересоздан/восстановлен из бэкапа/сегмент откатили. Отдельно от `SeqStalled`: без
    /// собственного кода регрессия НИКОГДА не даёт якорю переехать (условие роста не
    /// выполняется, пока сбор не догонит старое значение) — застой светился бы CRITICAL
    /// неделями при живом, реально растущем сборе. Само по себе тоже тревожный признак —
    /// та же категория риска (seq-reuse), что закрывали M-49/M-50.
    SeqRegressed,
    DiskLow,
    ContainerMissing,
    ContainerUnhealthy,
    ContainerRestarted,
    CronMarkerMissing,
    CronMarkerStale,
    /// `<job>.alert` маркер присутствует — последний прогон cron-задачи упал (R-005 F-6).
    /// Отдельно от `CronMarkerStale`/`CronMarkerMissing` (те читают ТОЛЬКО позитивный
    /// `.last-success` и молчат до 26ч) — этот код виден НЕМЕДЛЕННО, на первом же такте
    /// после сбоя. Детектируется в слое склейки (`watchdog_cycle::run_cycle`), не здесь —
    /// вход (`CronJobObservation`/`CronFailureMarker`) специфичен для этого слоя.
    CronFailed,
}

impl Incident {
    pub fn code(self) -> &'static str {
        match self {
            Incident::HeartbeatMissing => "WD-HB-MISSING",
            Incident::HeartbeatStale => "WD-HB-STALE",
            Incident::NotWritable => "WD-NOT-WRITABLE",
            Incident::SeqStalled => "WD-SEQ-STALLED",
            Incident::SeqRegressed => "WD-SEQ-REGRESSED",
            Incident::DiskLow => "WD-DISK-LOW",
            Incident::ContainerMissing => "WD-CONTAINER-MISSING",
            Incident::ContainerUnhealthy => "WD-CONTAINER-UNHEALTHY",
            Incident::ContainerRestarted => "WD-CONTAINER-RESTARTED",
            Incident::CronMarkerMissing => "WD-CRON-MISSING",
            Incident::CronMarkerStale => "WD-CRON-STALE",
            Incident::CronFailed => "WD-CRON-FAILED",
        }
    }
}

/// CRITICAL — сбор данных стоит / под немедленной угрозой. WARNING — деградация или
/// приближение к порогу, человек должен посмотреть, но не прямо сейчас ночью.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Critical,
    Warning,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Critical => "CRITICAL",
            Level::Warning => "WARNING",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub incident: Incident,
    pub level: Level,
    /// Человекочитаемая суть с конкретными цифрами (без заголовка/хоста/времени —
    /// это добавляет `format::format_alert`).
    pub message: String,
    /// Цель условия, когда их МНОГО (имя контейнера, имя cron-маркера) — `None` для
    /// singleton-условий (heartbeat/writable/seq/disk, на систему один журнал). Нужен для
    /// дедупликации: `WD-CONTAINER-UNHEALTHY` на `hft-recorder` и на `hft-gateway-serve` —
    /// РАЗНЫЕ инциденты, не должны подавлять друг друга (см. `state::WatchdogState`,
    /// ключ дедупа в `src/bin/ops-watchdog.rs::dedup_key`).
    pub target: Option<String>,
}

impl Alert {
    fn new(incident: Incident, level: Level, message: String) -> Self {
        Self {
            incident,
            level,
            message,
            target: None,
        }
    }

    fn with_target(incident: Incident, level: Level, message: String, target: &str) -> Self {
        Self {
            incident,
            level,
            message,
            target: Some(target.to_string()),
        }
    }
}

/// Снэпшот `recorder.heartbeat` (TD-019 JSON, `crates/recorder/src/lib.rs::write_heartbeat`).
/// Опциональные поля — потому что сам recorder кладёт `None`, если `storage_status()` упал
/// (диск недоступен для statvfs) — watchdog обязан не паниковать на этом, а просто не
/// проверять то, о чём recorder сам не знает (не додумывать за источник, testing.md п.3).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct HeartbeatSample {
    pub ts_wall_ms: i64,
    pub next_seq: u64,
    pub segment_index: u64,
    pub events: u64,
    pub free_bytes: Option<i64>,
    pub min_free_bytes: Option<i64>,
    pub writable: Option<bool>,
}

/// Состояние контейнера, как его видит `docker ps` (плюс `docker inspect
/// --format {{.RestartCount}}`). `healthy: None` = контейнер НЕ найден в выводе `docker ps`
/// вовсе (не запущен/упал) — отдельный (более тяжёлый) случай от "запущен, но unhealthy".
#[derive(Debug, Clone)]
pub struct ContainerStatus {
    pub name: String,
    pub healthy: Option<bool>,
    pub restart_count: Option<u64>,
}

/// Позитивный маркер успешного прогона cron-задачи (компакция/чекпоинт/ретеншен), формат —
/// как у `deploy/bin/journal-retention-cron.sh` (`RETENTION_LAST_SUCCESS`): содержимое файла —
/// UTC ISO-8601 момента последнего успеха, распарсенное вызывающим в epoch ms.
#[derive(Debug, Clone, Copy)]
pub struct CronMarker<'a> {
    pub name: &'a str,
    pub last_success_ms: Option<i64>,
}

/// Пороги. Обоснование каждого — в комментарии `Default` (замеры на проде 2026-07-31,
/// см. отчёт агента в handoff): heartbeat-тик recorder'а измерен в коде (10с), темп
/// убыли диска замерен двумя сэмплами `recorder.heartbeat` с разницей 30с на VPS.
#[derive(Debug, Clone, Copy)]
pub struct Thresholds {
    pub heartbeat_warn_ms: i64,
    pub heartbeat_crit_ms: i64,
    /// Не судить "next_seq не растёт" по сэмплам ближе друг к другу, чем это (шум/повторный
    /// ручной запуск watchdog'а не должен читаться как "recorder встал").
    pub seq_stall_min_gap_ms: i64,
    pub disk_warn_hours: f64,
    pub disk_crit_hours: f64,
    pub cron_warn_age_ms: i64,
    pub cron_crit_age_ms: i64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            // recorder тикает heartbeat КАЖДЫЕ 10с (`crates/recorder/src/lib.rs`:
            // `tokio::time::interval(Duration::from_secs(10))`, замерено чтением кода
            // 2026-07-31). Docker healthcheck в `docker-compose.yml` уже считает heartbeat
            // протухшим на 60с ("recorder жив + том пишется") — WARN синхронизирован с
            // этим порогом: watchdog сигналит РОВНО тогда, когда контейнер вот-вот начнёт
            // проваливать свой собственный healthcheck. CRIT — втрое дальше (18 пропущенных
            // тиков = 3 минуты): контейнер почти наверняка уже unhealthy, риск потери данных
            // реален.
            heartbeat_warn_ms: 60_000,
            heartbeat_crit_ms: 180_000,
            // Watchdog предполагается в cron раз в 5 минут (см. README/раннер) — 60с гарантирует,
            // что сравниваются сэмплы РАЗНЫХ прогонов, а не два вызова в одной пачке.
            seq_stall_min_gap_ms: 60_000,
            // Замер на проде 2026-07-31 23:11 UTC: два сэмпла `recorder.heartbeat` с разницей
            // 30с показали убыль free_bytes на 3_514_368 байт → ~117 КБ/с при живом рынке без
            // ретеншена. При free_bytes≈77.6 GiB и min_free_bytes=10 GiB это ~172ч (~7 суток)
            // до порога — большой запас, НО ретеншен/компакция это единственное, что мешает
            // тренду; если они молчат (см. cron-маркеры), тренд реален. 72ч WARNING — минимум
            // сутки на реакцию поверх суточного цикла ретеншена (04:07 UTC); 24ч CRITICAL —
            // меньше одного цикла ретеншена, значит цикл уже пропущен и данные под угрозой.
            disk_warn_hours: 72.0,
            disk_crit_hours: 24.0,
            // Cron-задачи (compaction 03:50 / checkpoint 04:00 / retention 04:07) — ежесуточные.
            // `deploy/bin/journal-retention-cron.sh` уже документирует именно эту конвенцию:
            // "старше ~26ч = cron не отработал" (комментарий у `RETENTION_LAST_SUCCESS`), даю
            // запас сверх суток на дрейф расписания. CRIT — двое суток подряд пропущено.
            cron_warn_age_ms: 26 * 3_600_000,
            cron_crit_age_ms: 48 * 3_600_000,
        }
    }
}

/// heartbeat-файл отсутствует/нечитаем/не парсится как JSON — то есть вызывающий вообще
/// не смог получить `HeartbeatSample`. Здоровая пара: `Some(&hb)` → `None`.
pub fn check_heartbeat_missing(hb: Option<&HeartbeatSample>) -> Option<Alert> {
    if hb.is_some() {
        return None;
    }
    Some(Alert::new(
        Incident::HeartbeatMissing,
        Level::Critical,
        "recorder.heartbeat отсутствует, нечитаем или не парсится как JSON — либо \
         recorder никогда не писал по этому пути, либо файл/том пропал"
            .to_string(),
    ))
}

/// `now_ms − hb.ts_wall_ms` относительно порогов. Здоровая пара: возраст ≤ `heartbeat_warn_ms`
/// → `None`.
pub fn check_heartbeat_stale(now_ms: i64, hb: &HeartbeatSample, thr: &Thresholds) -> Option<Alert> {
    let age_ms = now_ms - hb.ts_wall_ms;
    if age_ms > thr.heartbeat_crit_ms {
        Some(Alert::new(
            Incident::HeartbeatStale,
            Level::Critical,
            format!(
                "recorder heartbeat не обновлялся {age_ms} мс (порог CRITICAL {} мс; тик \
                 recorder'а — 10с, т.е. пропущено ~{} тиков подряд) — сбор данных, вероятно, встал",
                thr.heartbeat_crit_ms,
                age_ms / 10_000
            ),
        ))
    } else if age_ms > thr.heartbeat_warn_ms {
        Some(Alert::new(
            Incident::HeartbeatStale,
            Level::Warning,
            format!(
                "recorder heartbeat не обновлялся {age_ms} мс (порог WARNING {} мс)",
                thr.heartbeat_warn_ms
            ),
        ))
    } else {
        None
    }
}

/// `writable == Some(false)` — журнал перестал принимать записи. `writable == None`
/// (recorder сам не знает) НЕ алертит — не додумываем за источник. Здоровая пара:
/// `Some(true)`/`None` → `None`.
pub fn check_writable(hb: &HeartbeatSample) -> Option<Alert> {
    if hb.writable == Some(false) {
        Some(Alert::new(
            Incident::NotWritable,
            Level::Critical,
            "recorder.heartbeat сообщает writable=false — журнал перестал принимать записи \
             (disk-guard/fs ошибка)"
                .to_string(),
        ))
    } else {
        None
    }
}

/// R-008 F-8: `next_seq` УМЕНЬШИЛСЯ относительно якоря прогресса (`watchdog_cycle`) —
/// признак seq-reuse (пересозданный/восстановленный том журнала, откат сегмента). Отдельный
/// инцидент от `check_seq_stalled`: регрессия и застой — разные факты о мире, требуют разной
/// реакции (застой ждёт, регрессия — тревога прямо сейчас, независимо от того, сколько времени
/// прошло с прошлой проверки).
pub fn check_seq_regressed(prev: &HeartbeatSample, cur: &HeartbeatSample) -> Option<Alert> {
    if cur.next_seq < prev.next_seq {
        Some(Alert::new(
            Incident::SeqRegressed,
            Level::Critical,
            format!(
                "next_seq УМЕНЬШИЛСЯ ({} → {}) — признак seq-reuse (пересозданный/\
                 восстановленный том журнала, откат сегмента); якорь застоя сброшен на \
                 текущее значение, дальнейший застой будет отсчитан заново от него",
                prev.next_seq, cur.next_seq
            ),
        ))
    } else {
        None
    }
}

/// Самый опасный класс: процесс жив (heartbeat свежий), а `next_seq` НЕ вырос между двумя
/// проверками, разнесёнными хотя бы на `seq_stall_min_gap_ms`. Здоровая пара:
/// `cur.next_seq > prev.next_seq` → `None`.
pub fn check_seq_stalled(
    prev: &HeartbeatSample,
    prev_check_ms: i64,
    cur: &HeartbeatSample,
    cur_check_ms: i64,
    thr: &Thresholds,
) -> Option<Alert> {
    let gap_ms = cur_check_ms - prev_check_ms;
    if gap_ms < thr.seq_stall_min_gap_ms {
        return None;
    }
    if cur.next_seq <= prev.next_seq {
        Some(Alert::new(
            Incident::SeqStalled,
            Level::Critical,
            format!(
                "next_seq не вырос за {gap_ms} мс ({} → {}) — recorder(контейнер)/heartbeat \
                 живы, но данные не идут (тихая деградация, healthcheck её не ловит)",
                prev.next_seq, cur.next_seq
            ),
        ))
    } else {
        None
    }
}

/// Диск: абсолютный backstop (`free_bytes ≤ min_free_bytes`) + прогноз по тренду убыли между
/// текущим и предыдущим сэмплом (если оба несут диск-поля) + абсолютный WARNING-backstop
/// (`free_bytes < 3×min_free_bytes`) на случай, когда тренда ещё нет (первый прогон).
/// Здоровая пара: большой запас и убывание медленнее порогов (или рост free_bytes) → `None`.
pub fn check_disk(
    cur: &HeartbeatSample,
    prev: Option<(&HeartbeatSample, i64)>,
    cur_check_ms: i64,
    thr: &Thresholds,
) -> Option<Alert> {
    let (free, min_free) = match (cur.free_bytes, cur.min_free_bytes) {
        (Some(f), Some(m)) => (f, m),
        _ => return None, // recorder сам не знает — не додумываем.
    };

    if free <= min_free {
        return Some(Alert::new(
            Incident::DiskLow,
            Level::Critical,
            format!(
                "free_bytes={free} ≤ min_free_bytes={min_free} — диск на полу disk-guard \
                 порога, запись под угрозой немедленно"
            ),
        ));
    }

    if let Some((prev_hb, prev_ms)) = prev {
        if let Some(prev_free) = prev_hb.free_bytes {
            let dt_s = (cur_check_ms - prev_ms) as f64 / 1000.0;
            if dt_s > 0.0 {
                let decline_bps = (prev_free - free) as f64 / dt_s; // > 0 = убывает
                if decline_bps > 0.0 {
                    let hours_left = ((free - min_free) as f64 / decline_bps) / 3600.0;
                    if hours_left < thr.disk_crit_hours {
                        return Some(Alert::new(
                            Incident::DiskLow,
                            Level::Critical,
                            format!(
                                "диск кончится через ~{hours_left:.1}ч при текущем темпе убыли \
                                 (~{:.0} КБ/с), free_bytes={free}, min_free_bytes={min_free}",
                                decline_bps / 1024.0
                            ),
                        ));
                    } else if hours_left < thr.disk_warn_hours {
                        return Some(Alert::new(
                            Incident::DiskLow,
                            Level::Warning,
                            format!(
                                "диск кончится через ~{hours_left:.1}ч при текущем темпе убыли \
                                 (~{:.0} КБ/с), free_bytes={free}, min_free_bytes={min_free}",
                                decline_bps / 1024.0
                            ),
                        ));
                    }
                }
            }
        }
    }

    if free < min_free.saturating_mul(3) {
        return Some(Alert::new(
            Incident::DiskLow,
            Level::Warning,
            format!(
                "free_bytes={free} < 3×min_free_bytes({}) — запас сокращается, тренда ещё нет \
                 (первый прогон watchdog'а или недостаточно сэмплов)",
                min_free.saturating_mul(3)
            ),
        ));
    }

    None
}

/// Контейнер вообще не найден в `docker ps`. Здоровая пара: `healthy.is_some()` → `None`.
pub fn check_container_missing(status: &ContainerStatus) -> Option<Alert> {
    if status.healthy.is_none() {
        Some(Alert::with_target(
            Incident::ContainerMissing,
            Level::Critical,
            format!(
                "контейнер {} не найден в выводе `docker ps` — не запущен или упал",
                status.name
            ),
            &status.name,
        ))
    } else {
        None
    }
}

/// Контейнер найден, но не healthy (unhealthy/restarting/exited). Здоровая пара:
/// `healthy == Some(true)` (или `None` — тогда это `check_container_missing`) → `None`.
pub fn check_container_unhealthy(status: &ContainerStatus) -> Option<Alert> {
    if status.healthy == Some(false) {
        Some(Alert::with_target(
            Incident::ContainerUnhealthy,
            Level::Critical,
            format!("контейнер {} запущен, но НЕ healthy", status.name),
            &status.name,
        ))
    } else {
        None
    }
}

/// `RestartCount` вырос с прошлой проверки — контейнер падал и сам поднялся; healthcheck
/// может уже быть зелёным, но человек должен знать про сам факт крэша. Здоровая пара:
/// `prev == cur` (или нет baseline, `prev_restart_count == None`) → `None`.
pub fn check_container_restarted(
    status: &ContainerStatus,
    prev_restart_count: Option<u64>,
) -> Option<Alert> {
    match (prev_restart_count, status.restart_count) {
        (Some(prev), Some(cur)) if cur > prev => Some(Alert::with_target(
            Incident::ContainerRestarted,
            Level::Warning,
            format!(
                "контейнер {} перезапустился: RestartCount {prev} → {cur}",
                status.name
            ),
            &status.name,
        )),
        _ => None,
    }
}

/// Маркер cron-задачи не найден/пуст/не распарсен во время. Здоровая пара:
/// `last_success_ms.is_some()` → `None`.
pub fn check_cron_marker_missing(marker: &CronMarker) -> Option<Alert> {
    if marker.last_success_ms.is_none() {
        Some(Alert::with_target(
            Incident::CronMarkerMissing,
            Level::Warning,
            format!(
                "маркер cron-задачи '{}' отсутствует/нечитаем — ни одного успешного прогона \
                 не зафиксировано",
                marker.name
            ),
            marker.name,
        ))
    } else {
        None
    }
}

/// Маркер есть, но старше порога. Здоровая пара: возраст ≤ `cron_warn_age_ms` → `None`.
pub fn check_cron_marker_stale(
    marker: &CronMarker,
    now_ms: i64,
    thr: &Thresholds,
) -> Option<Alert> {
    let ts = marker.last_success_ms?;
    let age_ms = now_ms - ts;
    if age_ms > thr.cron_crit_age_ms {
        Some(Alert::with_target(
            Incident::CronMarkerStale,
            Level::Critical,
            format!(
                "cron-задача '{}' не отчиталась об успехе {age_ms} мс (порог CRITICAL {} мс \
                 — ~двое суток пропущено подряд)",
                marker.name, thr.cron_crit_age_ms
            ),
            marker.name,
        ))
    } else if age_ms > thr.cron_warn_age_ms {
        Some(Alert::with_target(
            Incident::CronMarkerStale,
            Level::Warning,
            format!(
                "cron-задача '{}' не отчиталась об успехе {age_ms} мс (порог WARNING {} мс \
                 — ~26ч)",
                marker.name, thr.cron_warn_age_ms
            ),
            marker.name,
        ))
    } else {
        None
    }
}

/// Разобрать вторую+ колонку `docker ps --format '{{.Status}}'` в грубое "жив и не бедствует".
/// Чистая строковая функция (никакого `docker`-вызова здесь). Неизвестный формат — по
/// умолчанию `true` (не додумываем беду там, где её текст явно не называет); явные плохие
/// статусы (`unhealthy`/`Restarting`/`Exited`) — `false`.
pub fn parse_docker_status_healthy(status: &str) -> bool {
    if status.contains("(unhealthy)") {
        return false;
    }
    if status.starts_with("Restarting") {
        return false;
    }
    if status.starts_with("Exited") {
        return false;
    }
    true
}
