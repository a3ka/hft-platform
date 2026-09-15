//! Слой склейки cron-цикла watchdog'а (R-005 `research/reviews/R-005-alerting.md`, находки
//! F-1/F-3/F-5/F-6/F-7). Переехал сюда из `src/bin/ops-watchdog.rs` (F-10: слой склейки был
//! нетестируем из `tests/`, потому что жил в бинаре). Бинарь после этой правки делает ТОЛЬКО
//! I/O (файлы/`docker`/HTTP) и зовёт [`run_cycle`] — сам цикл ЧИСТЫЙ (часы и данные —
//! параметры, как и `watchdog.rs`).
//!
//! # Три находки, зашитые в дизайн этого модуля
//!
//! **F-1 (детектор застоя `next_seq` выключается интервалом cron'а).** Причина бага —
//! `prev_heartbeat`/`prev_check_ms` двигались КАЖДЫЙ цикл, даже когда вердикт был "слишком
//! рано судить"; расстояние между сравниваемыми сэмплами навсегда равнялось интервалу
//! cron'а. Фикс: отдельный якорь `seq_progress_heartbeat`/`seq_progress_check_ms`
//! (`state::WatchdogState`), который двигается ТОЛЬКО когда `next_seq` реально вырос.
//! Возраст застоя = `now − момент_последнего_прогресса`, растёт с реальным временем
//! независимо от того, как часто cron проверяет.
//!
//! **F-3 (прогноз диска ложно тревожит на всплеске компакции/ретеншена).** Причина —
//! тренд считался по соседней паре сэмплов (5 минут), всплеск масштабировался на сутки.
//! Фикс: `state::WatchdogState::disk_history` — окно, покрывающее окно обслуживания целиком
//! ([`DISK_TREND_HORIZON_MS`]), с минимальным накопленным спаном перед тем, как доверять
//! прогнозу ([`DISK_TREND_MIN_SPAN_MS`]) — иначе холодный старт на двух сэмплах читался бы
//! как достоверный тренд. Абсолютные backstop'ы (`free ≤ min_free`, `< 3×min_free` без
//! истории) не тронуты — они уже были устойчивы к этому классу бага.
//!
//! **F-5 («не смог оценить» стирал дедуп-память).** `None` от детектора означает ДВЕ разные
//! вещи: "условие здорово" и "недостаточно данных, чтобы судить". Раньше оба схлопывались в
//! один `push_or_clear`, который снимал подавление в обоих случаях. Здесь — трёхвариантный
//! [`Verdict`]: `Healthy` (снять подавление), `Alert` (сработало), `Unknown` (НЕ трогать
//! состояние дедупа вообще).
//!
//! **F-6 (`<job>.alert` не читался).** Новый вход [`CronJobObservation::failure`] +
//! `Incident::CronFailed` — проверяется НЕЗАВИСИМО от `CronMarkerStale` (которая всё ещё
//! молчит 26ч, ориентируясь только на позитивный `.last-success`).
//!
//! **F-7 (рестарт-петля замолкала после первого сообщения).** `ContainerRestarted`
//! ЦЕЛИКОМ обходит дедуп-окно (`Cycle::record_always_delivered`) — по построению
//! `check_container_restarted` срабатывает ТОЛЬКО когда `RestartCount` вырос с прошлого
//! цикла, то есть каждое срабатывание УЖЕ является новым фактом; подавлять по времени
//! нечего, а раньше подавлялось (закрывало реальные новые рестарты остаточным окном).

use crate::state::{DiskSample, WatchdogState};
use crate::watchdog::{
    check_container_missing, check_container_restarted, check_container_unhealthy,
    check_cron_marker_missing, check_cron_marker_stale, check_disk, check_heartbeat_missing,
    check_heartbeat_stale, check_seq_regressed, check_seq_stalled, check_writable, Alert,
    ContainerStatus, CronMarker, HeartbeatSample, Incident, Level, Thresholds,
};

/// Горизонт истории `free_bytes`, используемой для тренда (R-005 F-3): обязан покрывать окно
/// обслуживания целиком (компакция 03:50 UTC → ретеншен 04:07 UTC — 2 часа с запасом).
pub const DISK_TREND_HORIZON_MS: i64 = 2 * 3_600_000;

/// Минимальный накопленный спан истории, прежде чем прогнозу по тренду можно доверять.
/// Меньше — и холодный старт (два сэмпла в 5 минут) читался бы как достоверный тренд
/// (см. F-3 vantage `f3_cold_start_spike_without_history_does_not_project_exhaustion`).
pub const DISK_TREND_MIN_SPAN_MS: i64 = 30 * 60_000;

/// Один вход cron-цикла: heartbeat (`None` — файл не прочитан/не распарсен), состояние
/// контейнеров (`docker ps`/`docker inspect`), наблюдения cron-задач (позитивный маркер +
/// маркер сбоя, F-6).
#[derive(Debug, Clone)]
pub struct CycleInputs {
    pub heartbeat: Option<HeartbeatSample>,
    pub containers: Vec<ContainerStatus>,
    pub cron_jobs: Vec<CronJobObservation>,
}

/// `<job>.alert` — «последний прогон УПАЛ» (`deploy/README.md` §5(A)). `None` — маркера нет
/// (успешный прогон гасит его сам, см. `deploy/bin/journal-retention-cron.sh`).
#[derive(Debug, Clone)]
pub struct CronFailureMarker {
    /// Первая строка файла — UTC ISO-8601 момента сбоя; `None`, если строка не распарсилась
    /// (обрезанная запись/кривой `date`) — само присутствие файла ВСЁ РАВНО алертит,
    /// fail-closed (R-005 F-6, `f6_failure_marker_with_unparseable_timestamp_still_alerts`).
    pub reported_at_ms: Option<i64>,
    /// Вторая+ строка(и) — текст сбоя (например `"dry-run exit=2 (2=failed_cold_verify — ..."`).
    pub detail: String,
}

/// Одна cron-задача обслуживания журнала (`compaction`/`gateway-checkpoint`/`retention`).
/// Имя — БЕЗ суффикса `.last-success`/`.alert`: у задачи ДВА независимых маркера, сущность
/// одна — задача.
#[derive(Debug, Clone)]
pub struct CronJobObservation {
    pub name: String,
    /// `<name>.last-success`, распарсенный в epoch ms.
    pub last_success_ms: Option<i64>,
    pub failure: Option<CronFailureMarker>,
}

/// Итог одного такта cron'а.
#[derive(Debug, Clone, Default)]
pub struct CycleOutcome {
    /// ВСЕ условия, сработавшие на этом такте, ДО дедупликации.
    pub fired: Vec<Alert>,
    /// Прошедшие дедуп — ровно то, что уходит в транспорт.
    pub delivered: Vec<Alert>,
    /// Сколько сработавших условий подавлено дедуп-окном (аудит "почему не пришло").
    pub suppressed: usize,
}

/// Вердикт одной проверки за такт. Различает "здорово" (можно снять подавление) от "не
/// смог оценить" (дедуп-память трогать нельзя, R-005 F-5) — то, что старая склейка не умела.
enum Verdict {
    Healthy,
    Alert(Alert),
    Unknown,
}

fn state_key(incident: Incident, target: Option<&str>) -> String {
    match target {
        Some(t) => format!("{}:{}", incident.code(), t),
        None => incident.code().to_string(),
    }
}

/// Аккумулятор одного такта: держит `&mut WatchdogState` и собирает `CycleOutcome`, применяя
/// единую дисциплину дедупликации (F-5) ко всем проверкам, КРОМЕ `ContainerRestarted` (F-7).
struct Cycle<'a> {
    state: &'a mut WatchdogState,
    now_ms: i64,
    dedup_window_ms: i64,
    outcome: CycleOutcome,
}

impl<'a> Cycle<'a> {
    fn record(&mut self, incident: Incident, target: Option<&str>, verdict: Verdict) {
        let key = state_key(incident, target);
        match verdict {
            Verdict::Alert(alert) => {
                self.outcome.fired.push(alert.clone());
                if self
                    .state
                    .should_fire(&key, self.now_ms, self.dedup_window_ms)
                {
                    self.outcome.delivered.push(alert);
                } else {
                    self.outcome.suppressed += 1;
                }
            }
            Verdict::Healthy => self.state.clear(&key),
            // F-5: "не смог оценить" ≠ "всё хорошо" — дедуп-память НЕ трогаем ни в какую
            // сторону (ни ставим, ни снимаем).
            Verdict::Unknown => {}
        }
    }

    /// R-005 F-7: `ContainerRestarted` обходит дедуп-окно целиком. `check_container_restarted`
    /// по построению срабатывает ТОЛЬКО когда `RestartCount` вырос с прошлого такта — то есть
    /// каждое срабатывание УЖЕ является новым, ранее не виденным фактом; временное окно здесь
    /// не защищает ни от чего, а глушит ПОСЛЕДУЮЩИЕ (текущие!) рестарты внутри своего окна.
    fn record_always_delivered(&mut self, alert: Alert) {
        self.outcome.fired.push(alert.clone());
        self.outcome.delivered.push(alert);
    }
}

/// Один такт cron'а: чистая функция над входом/часами/порогами/состоянием. Бинарь обязан
/// звать ТОЛЬКО это (плюс I/O для сборки `CycleInputs` и отправки `CycleOutcome::delivered`
/// в транспорт) — вся диагностическая логика здесь, юнит-тестируема без реальной машины.
pub fn run_cycle(
    inputs: &CycleInputs,
    now_ms: i64,
    thr: &Thresholds,
    dedup_window_ms: i64,
    state: &mut WatchdogState,
) -> CycleOutcome {
    let mut cycle = Cycle {
        state,
        now_ms,
        dedup_window_ms,
        outcome: CycleOutcome::default(),
    };

    run_heartbeat_checks(&mut cycle, inputs.heartbeat.as_ref(), thr);
    run_container_checks(&mut cycle, &inputs.containers);
    run_cron_checks(&mut cycle, &inputs.cron_jobs, thr);

    cycle.outcome
}

fn run_heartbeat_checks(cycle: &mut Cycle, hb: Option<&HeartbeatSample>, thr: &Thresholds) {
    let missing_verdict = match check_heartbeat_missing(hb) {
        Some(a) => Verdict::Alert(a),
        None => Verdict::Healthy,
    };
    cycle.record(Incident::HeartbeatMissing, None, missing_verdict);

    let Some(hb) = hb else {
        // Без сэмпла остальные heartbeat-производные проверки бессмысленны в этом такте.
        // Якоря (`seq_progress_*`, `prev_heartbeat`) НЕ трогаем — нечитаемый такт не должен
        // сбрасывать историю (R-005 F-1, `f1_unreadable_heartbeat_tick_does_not_reset_the_stall_anchor`).
        return;
    };
    let hb = *hb;

    let stale_verdict = match check_heartbeat_stale(cycle.now_ms, &hb, thr) {
        Some(a) => Verdict::Alert(a),
        None => Verdict::Healthy,
    };
    cycle.record(Incident::HeartbeatStale, None, stale_verdict);

    let writable_verdict = match check_writable(&hb) {
        Some(a) => Verdict::Alert(a),
        None => Verdict::Healthy,
    };
    cycle.record(Incident::NotWritable, None, writable_verdict);

    run_seq_stalled_check(cycle, &hb, thr);
    run_disk_check(cycle, &hb, thr);

    // Оставлено для обратной совместимости состояния (`red_ops_state.rs`
    // `roundtrip_preserves_dedup_map_and_prev_samples` читает/пишет ИМЕННО эти поля) — сама
    // диагностика их больше не использует (см. `seq_progress_*`/`disk_history` выше).
    cycle.state.prev_heartbeat = Some(hb);
    cycle.state.prev_check_ms = Some(cycle.now_ms);
}

/// R-005 F-1: якорь — момент ПОСЛЕДНЕГО НАБЛЮДАВШЕГОСЯ ПРОГРЕССА `next_seq`, а не момент
/// прошлой проверки. Двигается ТОЛЬКО когда `next_seq` реально вырос.
///
/// R-008 F-8: регрессия (`next_seq` УМЕНЬШИЛСЯ относительно якоря) обрабатывается ДО обычной
/// проверки застоя, отдельной веткой — см. [`Incident::SeqRegressed`] выше по модулю
/// `watchdog.rs`. Без неё якорь роста никогда не переезжает (условие `next_seq >
/// anchor.next_seq` не выполняется, пока сбор не догонит прежнее значение) — при проде
/// `next_seq ≈ 1.4e8` и темпе ~96 ev/s это недели непрерывного ложного `WD-SEQ-STALLED`
/// поверх реально растущего сбора.
fn run_seq_stalled_check(cycle: &mut Cycle, hb: &HeartbeatSample, thr: &Thresholds) {
    let now_ms = cycle.now_ms;

    if let Some(anchor_hb) = cycle.state.seq_progress_heartbeat {
        if let Some(alert) = check_seq_regressed(&anchor_hb, hb) {
            // Регрессия — ОТДЕЛЬНЫЙ инцидент, не "продолжение застоя". Якорь сбрасывается на
            // текущее значение НЕМЕДЛЕННО (та же дисциплина, что и для реального прогресса
            // выше), чтобы обычный детектор застоя начинал отсчёт заново от нового состояния
            // мира, а не гнался за значением, которое сбор физически не может снова достичь
            // (новый том/сегмент стартует с меньшего `next_seq`).
            cycle.state.seq_progress_heartbeat = Some(*hb);
            cycle.state.seq_progress_check_ms = Some(now_ms);
            // Обходит дедуп-окно целиком — тот же аргумент, что у F-7
            // (`Cycle::record_always_delivered`): по построению эта ветка срабатывает ТОЛЬКО
            // когда `next_seq` реально уменьшился относительно ТЕКУЩЕГО якоря, а якорь после
            // срабатывания сразу переезжает на новое значение — то есть повторное срабатывание
            // возможно ТОЛЬКО на действительно НОВОЙ регрессии (ещё одно уменьшение относительно
            // уже сброшенного якоря), а не на повторе того же факта. Дедуп по времени здесь
            // подавлял бы независимые события, как подавлял независимые рестарты в F-7.
            cycle.record_always_delivered(alert);
            // На такте регрессии о "застое" ещё нечего сказать — якорь только что сброшен,
            // возраст с него равен нулю. Не "здорово" (дедуп-память не снимаем на случай, если
            // реальный застой сохранится и после нормализации якоря) и не "тревога" — та же
            // семантика, что у первого-когда-либо сэмпла ниже.
            cycle.record(Incident::SeqStalled, None, Verdict::Unknown);
            return;
        }
    }

    let anchor = cycle.state.seq_progress_heartbeat;
    let anchor_ms = cycle.state.seq_progress_check_ms;
    let verdict = match (anchor, anchor_ms) {
        (Some(anchor_hb), Some(_)) if hb.next_seq > anchor_hb.next_seq => {
            // Прогресс — якорь переезжает на этот такт, условие определённо здорово.
            cycle.state.seq_progress_heartbeat = Some(*hb);
            cycle.state.seq_progress_check_ms = Some(now_ms);
            Verdict::Healthy
        }
        (Some(anchor_hb), Some(anchor_ms)) => {
            // Прогресса нет — якорь НЕ двигаем; возраст застоя = now − момент якоря, растёт с
            // реальным временем независимо от интервала cron'а.
            match check_seq_stalled(&anchor_hb, anchor_ms, hb, now_ms, thr) {
                Some(a) => Verdict::Alert(a),
                // Анти-флап: гэп с якоря ещё мал — "слишком рано судить", не "здорово".
                None => Verdict::Unknown,
            }
        }
        _ => {
            // Первый когда-либо сэмпл — истории для сравнения ещё нет.
            cycle.state.seq_progress_heartbeat = Some(*hb);
            cycle.state.seq_progress_check_ms = Some(now_ms);
            Verdict::Unknown
        }
    };
    cycle.record(Incident::SeqStalled, None, verdict);
}

/// R-005 F-3: тренд считается по ИСТОРИИ, покрывающей окно обслуживания целиком, а не по
/// соседней паре сэмплов. Абсолютные backstop'ы (`check_disk`'s `free ≤ min_free` и
/// `< 3×min_free` без истории) не тронуты — они уже устойчивы к этому классу бага.
fn run_disk_check(cycle: &mut Cycle, hb: &HeartbeatSample, thr: &Thresholds) {
    let (Some(free), Some(_min_free)) = (hb.free_bytes, hb.min_free_bytes) else {
        // recorder сам не знает про диск (statvfs упал) — не додумываем за источник, и не
        // портим историю фиктивным сэмплом.
        cycle.record(Incident::DiskLow, None, Verdict::Unknown);
        return;
    };
    let now_ms = cycle.now_ms;

    cycle.state.disk_history.push(DiskSample {
        check_ms: now_ms,
        free_bytes: free,
    });
    cycle
        .state
        .disk_history
        .retain(|s| now_ms - s.check_ms <= DISK_TREND_HORIZON_MS);

    let trend_prev = cycle.state.disk_history.first().and_then(|oldest| {
        if now_ms - oldest.check_ms >= DISK_TREND_MIN_SPAN_MS {
            Some((oldest.check_ms, oldest.free_bytes))
        } else {
            None
        }
    });

    let verdict = match trend_prev {
        Some((prev_ms, prev_free)) => {
            // `check_disk` читает из `prev_hb` только `.free_bytes` — синтетический сэмпл
            // несёт ИМЕННО ту точку истории, остальные поля не участвуют в расчёте.
            let synthetic_prev = HeartbeatSample {
                free_bytes: Some(prev_free),
                ..*hb
            };
            check_disk(hb, Some((&synthetic_prev, prev_ms)), now_ms, thr)
        }
        None => check_disk(hb, None, now_ms, thr),
    };
    let verdict = match verdict {
        Some(a) => Verdict::Alert(a),
        None => Verdict::Healthy,
    };
    cycle.record(Incident::DiskLow, None, verdict);
}

fn run_container_checks(cycle: &mut Cycle, containers: &[ContainerStatus]) {
    for status in containers {
        let missing_verdict = match check_container_missing(status) {
            Some(a) => Verdict::Alert(a),
            None => Verdict::Healthy,
        };
        cycle.record(
            Incident::ContainerMissing,
            Some(&status.name),
            missing_verdict,
        );

        let unhealthy_verdict = match check_container_unhealthy(status) {
            Some(a) => Verdict::Alert(a),
            None => Verdict::Healthy,
        };
        cycle.record(
            Incident::ContainerUnhealthy,
            Some(&status.name),
            unhealthy_verdict,
        );

        let prev_restart = cycle.state.prev_restart_counts.get(&status.name).copied();
        if let Some(alert) = check_container_restarted(status, prev_restart) {
            // F-7: НЕ через `record` — обходит дедуп-окно целиком, см. doc-comment метода.
            cycle.record_always_delivered(alert);
        }
        if let Some(cur) = status.restart_count {
            cycle
                .state
                .prev_restart_counts
                .insert(status.name.clone(), cur);
        }
    }
}

fn run_cron_checks(cycle: &mut Cycle, jobs: &[CronJobObservation], thr: &Thresholds) {
    for job in jobs {
        let marker = CronMarker {
            name: &job.name,
            last_success_ms: job.last_success_ms,
        };

        let missing_verdict = match check_cron_marker_missing(&marker) {
            Some(a) => Verdict::Alert(a),
            None => Verdict::Healthy,
        };
        cycle.record(
            Incident::CronMarkerMissing,
            Some(&job.name),
            missing_verdict,
        );

        let stale_verdict = match check_cron_marker_stale(&marker, cycle.now_ms, thr) {
            Some(a) => Verdict::Alert(a),
            None => Verdict::Healthy,
        };
        cycle.record(Incident::CronMarkerStale, Some(&job.name), stale_verdict);

        // R-005 F-6: `<job>.alert` — сигнал "прогон УПАЛ", проверяется НЕЗАВИСИМО от
        // свежести позитивного `.last-success` (та проверка выше молчит ещё 26ч).
        let failed_verdict = match &job.failure {
            Some(failure) => Verdict::Alert(build_cron_failed_alert(&job.name, failure)),
            None => Verdict::Healthy,
        };
        cycle.record(Incident::CronFailed, Some(&job.name), failed_verdict);
    }
}

fn build_cron_failed_alert(name: &str, failure: &CronFailureMarker) -> Alert {
    let ts_note = match failure.reported_at_ms {
        Some(ts) => format!("зафиксирован в {ts} мс (epoch)"),
        // Fail-closed (R-005 F-6, `f6_failure_marker_with_unparseable_timestamp_still_alerts`):
        // нечитаемый таймстамп — не разрешение молчать, сам факт файла = факт сбоя.
        None => "таймстамп в маркере не распознан — считаем сбоем, fail-closed".to_string(),
    };
    Alert {
        incident: Incident::CronFailed,
        level: Level::Critical,
        message: format!(
            "cron-задача '{name}' сообщила о провале последнего прогона ({ts_note}): {}",
            failure.detail
        ),
        target: Some(name.to_string()),
    }
}
