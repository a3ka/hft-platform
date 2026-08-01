//! RED-оракулы на находки PR-гейта **R-005** (`research/reviews/R-005-alerting.md`):
//! F-1 (BLOCKER), F-3 (MAJOR), F-5, F-6, F-7. Автор — architect (RED-first, `.claude/rules/testing.md`);
//! реализации на момент коммита этого файла НЕ существует — файл обязан быть КРАСНЫМ.
//!
//! # Почему эти оракулы СЦЕНАРНЫЕ, а не «вызов детектора с подставленными аргументами»
//!
//! F-4 из R-005: тесты прошлого круга написал автор реализации, и они доказывали, что код
//! делает то, что делает. Оба блокера живут НЕ в поштучных детекторах (`ops::watchdog::check_*`
//! — они корректны и покрыты), а в **слое склейки**: в том, как результаты детекторов
//! соединяются с состоянием между запусками cron'а. Тест вида
//! `assert!(check_seq_stalled(&prev, 0, &cur, 300_000, &thr).is_some())` зелёный — и при этом
//! на проде при cron `*/30s` детектор не срабатывает НИКОГДА (R-005 §F-1, десять «норма»
//! подряд на полностью вставшем сборе).
//!
//! Поэтому здесь моделируется **последовательность запусков cron'а**: каждый «тик» —
//! ОТДЕЛЬНЫЙ процесс, который загружает состояние с диска, отрабатывает и сохраняет его
//! обратно (`CronSim::tick` — ровно путь `main()`: `load_or_default` → работа → `save`).
//! Состояние проходит через JSON на каждом такте — оракул падает и на «история живёт только
//! в памяти процесса».
//!
//! # Контракт, который эти оракулы ЗАДАЮТ (спецификация для engine-dev)
//!
//! Слой склейки переезжает из `src/bin/ops-watchdog.rs` (нетестируемо из `tests/` — R-005 §F-10)
//! в библиотеку: новый модуль `ops::watchdog_cycle`. Бинарь после этого обязан делать ТОЛЬКО
//! I/O (файлы, `docker`, HTTP) и звать `run_cycle` — это проверяет `scripts/verify_alerting.sh`
//! grep-канарейкой (иначе получим «библиотека зелёная, а на прод уехала старая склейка»).
//!
//! ```ignore
//! // crates/ops/src/watchdog_cycle.rs
//! pub struct CycleInputs {
//!     pub heartbeat: Option<HeartbeatSample>,   // None = файл не прочитан/не распарсен
//!     pub containers: Vec<ContainerStatus>,
//!     pub cron_jobs: Vec<CronJobObservation>,
//! }
//!
//! #[derive(Debug, Clone)]
//! pub struct CronJobObservation {
//!     /// Имя ЗАДАЧИ без суффикса: "retention" / "compaction" / "gateway-checkpoint".
//!     /// У задачи ДВА маркера (`<name>.last-success` и `<name>.alert`), сущность — задача.
//!     pub name: String,
//!     /// `<name>.last-success`, распарсенный в epoch ms (позитивный heartbeat).
//!     pub last_success_ms: Option<i64>,
//!     /// `<name>.alert` — «последний прогон УПАЛ» (`deploy/README.md` §5(A)). None = маркера
//!     /// нет (успешный прогон его гасит).
//!     pub failure: Option<CronFailureMarker>,
//! }
//!
//! #[derive(Debug, Clone)]
//! pub struct CronFailureMarker {
//!     /// Первая строка файла — UTC ISO-8601; None, если строка не распарсилась.
//!     pub reported_at_ms: Option<i64>,
//!     /// Вторая строка — текст сбоя (`"dry-run exit=2 (2=failed_cold_verify ...)"`).
//!     pub detail: String,
//! }
//!
//! #[derive(Debug, Clone)]
//! pub struct CycleOutcome {
//!     /// Все условия, сработавшие на этом такте (ДО дедупликации).
//!     pub fired: Vec<Alert>,
//!     /// Прошедшие дедуп — ровно то, что уходит в транспорт.
//!     pub delivered: Vec<Alert>,
//!     pub suppressed: usize,
//! }
//!
//! pub fn run_cycle(
//!     inputs: &CycleInputs,
//!     now_ms: i64,
//!     thr: &Thresholds,
//!     dedup_window_ms: i64,
//!     state: &mut WatchdogState,
//! ) -> CycleOutcome;
//! ```
//!
//! Плюс новый вариант `ops::watchdog::Incident::CronFailed` с кодом `WD-CRON-FAILED` (F-6).
//!
//! **Чистые детекторы `ops::watchdog::check_*` менять НЕ требуется** — их сигнатуры и
//! существующие тесты (`red_ops_watchdog.rs`) остаются валидными. Все находки чинятся в
//! слое склейки: какой сэмпл берётся за базу сравнения, что происходит с дедуп-памятью, и
//! читается ли `.alert`-маркер вообще.
//!
//! # Чек-лист деградированного входа (`.claude/rules/testing.md`)
//!
//! - **Асимметрия** — застой ПОСЛЕ нормального роста (не с первого такта); ночной всплеск
//!   расхода диска против ровного тренда; один контейнер перезапускается, второй здоров.
//! - **Множественность** — две cron-задачи упали одновременно; `RestartCount` прыгает на 3
//!   за один такт; 72 такта в одном сценарии.
//! - **Отсутствие** — пропущенные запуски cron'а (crond умер на 10 минут); heartbeat
//!   нечитаем на такте; `free_bytes: None` (recorder не смог `statvfs`); `.alert`-маркер с
//!   нераспарсенным таймстампом. «Не смог оценить» ≠ «всё хорошо» — центральный инвариант.
//! - **Границы** — анти-флап (два прогона в 5 с не должны судить о застое); ровно на пороге
//!   `free_bytes == min_free_bytes`; холодный старт без истории.
//! - **Прод-масштаб** — реальные интервалы cron (30 с … 5 мин, `docs/runbooks/alerting.md`),
//!   реальные числа прод-heartbeat'а (`free=83.1 ГБ`, `min_free=10.7 ГБ`, `next_seq≈1.4e8`),
//!   реальное окно обслуживания (компакция 03:50 / чекпоинт 04:00 / ретеншен 04:07 UTC),
//!   сценарии длиной 6 часов и 7 суток, состояние через JSON на каждом такте.

use std::collections::HashSet;
use std::path::PathBuf;

use ops::state::{WatchdogState, DEFAULT_DEDUP_WINDOW_MS};
use ops::watchdog::{Alert, ContainerStatus, HeartbeatSample, Incident, Level, Thresholds};
use ops::watchdog_cycle::{
    run_cycle, CronFailureMarker, CronJobObservation, CycleInputs, CycleOutcome,
};

// ─────────────────────────── прод-числа (замер VPS 2026-07-31, R-005 Done Block) ───────────────
//
// {"events":3456495,"free_bytes":83116052480,"min_free_bytes":10737418240,
//  "next_seq":140762639,"segment_index":145,"ts_wall_ms":1785541305840,"writable":true}

/// 2026-07-31T02:00:00Z — старт «ночных» сценариев (окно обслуживания 03:50–04:10 UTC внутри).
const START_02_UTC: i64 = 1_785_463_200_000;
const TICK_5MIN: i64 = 300_000;
const PROD_FREE: i64 = 83_116_052_480;
const PROD_MIN_FREE: i64 = 10_737_418_240;
const PROD_SEQ: u64 = 140_762_639;
/// Замер на проде: 2881 события за 30 с ≈ 96/с.
const EVENTS_PER_SEC: u64 = 96;
/// Замер на проде: ~117 КБ/с убыли свободного места в установившемся режиме → 35.1 МБ за 5 мин.
const BASELINE_DECLINE_PER_5MIN: i64 = 35_100_000;

// ─────────────────────────── харнесс: последовательность запусков cron'а ───────────────────────

/// Симулятор cron'а. КАЖДЫЙ тик — отдельный процесс: состояние читается с диска и пишется
/// обратно, как в `main()`. Это часть контракта: любое поле состояния, нужное детектору,
/// обязано переживать перезапуск процесса (иначе на проде watchdog «забывает» всё каждые
/// 5 минут — ровно тот класс, что F-1).
struct CronSim {
    dir: tempfile::TempDir,
}

impl CronSim {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn state_path(&self) -> PathBuf {
        self.dir.path().join("watchdog.state.json")
    }

    fn tick(&self, inputs: &CycleInputs, now_ms: i64) -> CycleOutcome {
        let thr = Thresholds::default();
        let path = self.state_path();
        let mut state = WatchdogState::load_or_default(&path);
        let outcome = run_cycle(inputs, now_ms, &thr, DEFAULT_DEDUP_WINDOW_MS, &mut state);
        state
            .save(&path)
            .expect("состояние watchdog'а обязано сохраняться между запусками cron'а");
        outcome
    }

    fn state_file_len(&self) -> u64 {
        std::fs::metadata(self.state_path())
            .expect("файл состояния должен существовать после первого тика")
            .len()
    }
}

// ─────────────────────────── фикстуры ───────────────────────────

fn hb(ts_wall_ms: i64, next_seq: u64, free_bytes: i64) -> HeartbeatSample {
    HeartbeatSample {
        ts_wall_ms,
        next_seq,
        segment_index: 145,
        events: 3_456_495,
        free_bytes: Some(free_bytes),
        min_free_bytes: Some(PROD_MIN_FREE),
        writable: Some(true),
    }
}

/// recorder не смог получить `storage_status()` — диск-поля отсутствуют ЦЕЛИКОМ (так пишет
/// сам recorder, TD-019). «Не знаю» — не «всё хорошо».
fn hb_without_disk_fields(ts_wall_ms: i64, next_seq: u64) -> HeartbeatSample {
    HeartbeatSample {
        ts_wall_ms,
        next_seq,
        segment_index: 145,
        events: 3_456_495,
        free_bytes: None,
        min_free_bytes: None,
        writable: Some(true),
    }
}

fn healthy_containers() -> Vec<ContainerStatus> {
    vec![
        ContainerStatus {
            name: "hft-recorder".to_string(),
            healthy: Some(true),
            restart_count: Some(0),
        },
        ContainerStatus {
            name: "hft-gateway-serve".to_string(),
            healthy: Some(true),
            restart_count: Some(0),
        },
    ]
}

fn healthy_cron_jobs(now_ms: i64) -> Vec<CronJobObservation> {
    ["compaction", "gateway-checkpoint", "retention"]
        .into_iter()
        .map(|name| CronJobObservation {
            name: name.to_string(),
            last_success_ms: Some(now_ms - 3 * 3_600_000), // ночной прогон, 3ч назад — свежо
            failure: None,
        })
        .collect()
}

/// Здоровое окружение с подменяемым heartbeat'ом.
fn inputs_with_hb(heartbeat: Option<HeartbeatSample>, now_ms: i64) -> CycleInputs {
    CycleInputs {
        heartbeat,
        containers: healthy_containers(),
        cron_jobs: healthy_cron_jobs(now_ms),
    }
}

/// Свежий heartbeat: recorder тикает каждые 10 с, значит на момент проверки ему 3 с —
/// контейнер healthy, docker-healthcheck зелёный. Ровно тот фон, на котором тихая
/// деградация невидима.
fn fresh_hb(now_ms: i64, next_seq: u64, free_bytes: i64) -> Option<HeartbeatSample> {
    Some(hb(now_ms - 3_000, next_seq, free_bytes))
}

fn fired(out: &CycleOutcome, incident: Incident) -> Vec<Alert> {
    out.fired
        .iter()
        .filter(|a| a.incident == incident)
        .cloned()
        .collect()
}

fn delivered(out: &CycleOutcome, incident: Incident) -> Vec<Alert> {
    out.delivered
        .iter()
        .filter(|a| a.incident == incident)
        .cloned()
        .collect()
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// F-1 (BLOCKER) — детектор застоя обязан работать при ЛЮБОМ интервале cron'а
// ══════════════════════════════════════════════════════════════════════════════════════════

/// ГЛАВНЫЙ оракул находки F-1.
///
/// Воспроизведение из R-005: recorder встал (`next_seq` заморожен), heartbeat СВЕЖИЙ,
/// контейнер healthy — healthcheck зелёный, в логе десять раз «норма». Причина: база
/// сравнения двигалась даже тогда, когда вердикт был «слишком рано судить», поэтому
/// расстояние между сравниваемыми сэмплами навсегда равнялось интервалу cron'а.
///
/// Контракт: база сравнения — момент ПОСЛЕДНЕГО НАБЛЮДАВШЕГОСЯ ПРОГРЕССА `next_seq`, а не
/// момент прошлой проверки. Тогда возраст застоя растёт с реальным временем и порог
/// `seq_stall_min_gap_ms` рано или поздно преодолевается при ЛЮБОМ интервале.
///
/// Границы, зашитые в assert'ы:
/// - НЕ раньше `seq_stall_min_gap_ms` (анти-флап сохранён — иначе «фикс» = алертить всегда);
/// - НЕ позже `seq_stall_min_gap_ms + interval` (не позже первого такта, на котором возраст
///   застоя перевалил порог).
#[test]
fn f1_seq_stall_is_detected_at_every_realistic_cron_interval() {
    let thr = Thresholds::default();
    // Реальные интервалы: 30 с и 1 мин — «оператор поставил чаще, чтобы узнавать быстрее»
    // (именно этот случай убил детектор); 2 мин и 5 мин — рекомендация runbook'а.
    for interval_ms in [30_000_i64, 60_000, 120_000, 300_000] {
        let sim = CronSim::new();
        let mut first_fire_at: Option<i64> = None;
        let mut first_delivery_at: Option<i64> = None;

        // 30 минут полностью вставшего сбора.
        let ticks = (30 * 60_000) / interval_ms;
        for i in 0..=ticks {
            let elapsed = i * interval_ms;
            let now = START_02_UTC + elapsed;
            // next_seq ЗАМОРОЖЕН, heartbeat свежий, диск не двигается.
            let inputs = inputs_with_hb(fresh_hb(now, PROD_SEQ, PROD_FREE), now);
            let out = sim.tick(&inputs, now);

            if first_fire_at.is_none() && !fired(&out, Incident::SeqStalled).is_empty() {
                first_fire_at = Some(elapsed);
            }
            if first_delivery_at.is_none() && !delivered(&out, Incident::SeqStalled).is_empty() {
                first_delivery_at = Some(elapsed);
            }
        }

        let fire_at = first_fire_at.unwrap_or_else(|| {
            panic!(
                "WD-SEQ-STALLED не сработал НИ РАЗУ за 30 минут полностью вставшего сбора при \
                 интервале cron'а {interval_ms} мс — детектор выключен интервалом (R-005 F-1)"
            )
        });
        assert!(
            fire_at >= thr.seq_stall_min_gap_ms,
            "интервал {interval_ms} мс: сработал через {fire_at} мс — РАНЬШЕ анти-флап-порога \
             {} мс; «фикс», который алертит всегда, не принимается",
            thr.seq_stall_min_gap_ms
        );
        assert!(
            fire_at <= thr.seq_stall_min_gap_ms + interval_ms,
            "интервал {interval_ms} мс: сработал через {fire_at} мс — позже первого такта, на \
             котором возраст застоя перевалил порог {} мс (ожидание ≤ {} мс)",
            thr.seq_stall_min_gap_ms,
            thr.seq_stall_min_gap_ms + interval_ms
        );
        assert_eq!(
            first_delivery_at,
            Some(fire_at),
            "интервал {interval_ms} мс: первое срабатывание обязано быть ДОСТАВЛЕНО (дедуп-память \
             пуста, подавлять нечего)"
        );
    }
}

/// Асимметрия (`testing.md` п.1): застой начинается НЕ с первого такта. Сбор шёл нормально
/// 5 минут, потом встал. Отсчёт возраста застоя обязан идти от последнего прогресса, а не от
/// старта процесса и не от прошлой проверки.
#[test]
fn f1_stall_after_normal_progress_is_measured_from_last_progress() {
    let thr = Thresholds::default();
    let interval = 30_000_i64;
    let sim = CronSim::new();
    let stall_starts_at = 5 * 60_000_i64; // 5 минут нормальной работы
    let mut first_fire_at: Option<i64> = None;

    for i in 0..=((20 * 60_000) / interval) {
        let elapsed = i * interval;
        let now = START_02_UTC + elapsed;
        // До момента застоя seq растёт с прод-темпом, после — заморожен.
        let grown_ms = elapsed.min(stall_starts_at);
        let seq = PROD_SEQ + (grown_ms as u64 / 1000) * EVENTS_PER_SEC;
        let inputs = inputs_with_hb(fresh_hb(now, seq, PROD_FREE), now);
        let out = sim.tick(&inputs, now);
        if first_fire_at.is_none() && !fired(&out, Incident::SeqStalled).is_empty() {
            first_fire_at = Some(elapsed);
        }
    }

    let fire_at = first_fire_at.expect(
        "WD-SEQ-STALLED не сработал за 15 минут застоя, начавшегося после нормальной работы \
         (интервал cron'а 30 с) — R-005 F-1",
    );
    assert!(
        fire_at >= stall_starts_at + thr.seq_stall_min_gap_ms,
        "сработал через {fire_at} мс — раньше, чем застой прожил {} мс (ложная тревога на \
         нормальной работе)",
        thr.seq_stall_min_gap_ms
    );
    assert!(
        fire_at <= stall_starts_at + thr.seq_stall_min_gap_ms + interval,
        "сработал через {fire_at} мс — позже ожидаемого ≤ {} мс",
        stall_starts_at + thr.seq_stall_min_gap_ms + interval
    );
}

/// Отсутствие (`testing.md` п.3): cron не запускался 10 минут (crond умер / ребут), потом
/// вернулся. Пропущенные запуски не должны ни терять факт застоя, ни давать ложную тревогу
/// на здоровом сборе.
#[test]
fn f1_missed_cron_runs_do_not_lose_the_stall() {
    let sim = CronSim::new();
    let t0 = START_02_UTC;
    // Тик 1 — база; сбор уже стоит.
    let out0 = sim.tick(&inputs_with_hb(fresh_hb(t0, PROD_SEQ, PROD_FREE), t0), t0);
    assert!(
        fired(&out0, Incident::SeqStalled).is_empty(),
        "на первом такте истории ещё нет — судить о застое нельзя"
    );

    // crond молчал 10 минут, затем один запуск.
    let t1 = t0 + 10 * 60_000;
    let out1 = sim.tick(&inputs_with_hb(fresh_hb(t1, PROD_SEQ, PROD_FREE), t1), t1);
    assert!(
        !fired(&out1, Incident::SeqStalled).is_empty(),
        "после 10 минут простоя (пусть и с пропущенными запусками cron'а) застой обязан быть виден"
    );
}

/// Отсутствие #2: на одном такте heartbeat нечитаем (файл в момент записи / том отвалился).
/// Это «не смог прочитать», а не «сбор восстановился» — якорь застоя не сбрасывается.
#[test]
fn f1_unreadable_heartbeat_tick_does_not_reset_the_stall_anchor() {
    let sim = CronSim::new();
    let interval = 30_000_i64;
    let t0 = START_02_UTC;

    sim.tick(&inputs_with_hb(fresh_hb(t0, PROD_SEQ, PROD_FREE), t0), t0);

    // Такт 2: heartbeat не прочитан.
    let t1 = t0 + interval;
    let out1 = sim.tick(&inputs_with_hb(None, t1), t1);
    assert!(
        !fired(&out1, Incident::HeartbeatMissing).is_empty(),
        "нечитаемый heartbeat обязан алертить сам по себе"
    );

    // Такт 3 и далее: heartbeat снова читается, seq всё ещё заморожен.
    let mut fire_at: Option<i64> = None;
    for i in 2..=10 {
        let now = t0 + i * interval;
        let out = sim.tick(
            &inputs_with_hb(fresh_hb(now, PROD_SEQ, PROD_FREE), now),
            now,
        );
        if fire_at.is_none() && !fired(&out, Incident::SeqStalled).is_empty() {
            fire_at = Some(now - t0);
        }
    }
    let fire_at = fire_at.expect(
        "пропуск heartbeat'а на одном такте не должен сбрасывать якорь застоя — WD-SEQ-STALLED \
         обязан сработать",
    );
    assert!(
        fire_at <= Thresholds::default().seq_stall_min_gap_ms + 2 * interval,
        "застой обнаружен через {fire_at} мс — якорь, похоже, сброшен нечитаемым тактом"
    );
}

/// ПАРНЫЙ VANTAGE: здоровый сбор не даёт ложной тревоги ни на одном интервале.
/// Ловит «фикс», сводящийся к «алертить всегда».
#[test]
fn f1_healthy_growth_never_fires_seq_stalled_at_any_interval() {
    for interval_ms in [30_000_i64, 60_000, 120_000, 300_000] {
        let sim = CronSim::new();
        for i in 0..=((30 * 60_000) / interval_ms) {
            let elapsed = i * interval_ms;
            let now = START_02_UTC + elapsed;
            let seq = PROD_SEQ + (elapsed as u64 / 1000) * EVENTS_PER_SEC;
            let out = sim.tick(&inputs_with_hb(fresh_hb(now, seq, PROD_FREE), now), now);
            assert!(
                fired(&out, Incident::SeqStalled).is_empty(),
                "ложная тревога WD-SEQ-STALLED на здоровом растущем сборе (интервал \
                 {interval_ms} мс, такт {i})"
            );
        }
    }
}

/// Граница (`testing.md` п.4): анти-флап. Два запуска в 5 секунд (ручной повтор оператора)
/// не дают вердикта о застое — next_seq физически не успел бы вырасти заметно.
#[test]
fn f1_double_run_within_anti_flap_window_does_not_fire() {
    let sim = CronSim::new();
    let t0 = START_02_UTC;
    for offset in [0_i64, 5_000, 10_000] {
        let now = t0 + offset;
        let out = sim.tick(
            &inputs_with_hb(fresh_hb(now, PROD_SEQ, PROD_FREE), now),
            now,
        );
        assert!(
            fired(&out, Incident::SeqStalled).is_empty(),
            "анти-флап нарушен: вердикт о застое вынесен через {offset} мс после первой проверки"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// F-5 — «не смог оценить» НЕ стирает дедуп-память
// ══════════════════════════════════════════════════════════════════════════════════════════

/// R-005 F-5: `push_or_clear` трактует `None` детектора как «условие здорово» и снимает
/// подавление. Но `None` означает ДВА разных вещи: «здорово» и «недостаточно данных, чтобы
/// судить». Второе не должно сбрасывать окно подавления активного инцидента — иначе ручной
/// (или просто более частый) прогон watchdog'а превращает один инцидент в поток сообщений,
/// а поток сообщений перестают читать. Тот же класс, что `Unknown` против `NoSegments` в M-49.
///
/// Сценарий: застой обнаружен и доставлен; затем оператор запускает watchdog вручную через
/// 5 секунд («не смог оценить»); затем очередной штатный тик. Внутри 30-минутного окна
/// подавления доставка обязана быть РОВНО ОДНА, при том что инцидент продолжает
/// ОБНАРУЖИВАТЬСЯ (замолчать — не решение).
#[test]
fn f5_unknown_verdict_does_not_reset_dedup_of_seq_stalled() {
    let sim = CronSim::new();
    let t0 = START_02_UTC;
    let mut deliveries = 0usize;

    // База.
    sim.tick(&inputs_with_hb(fresh_hb(t0, PROD_SEQ, PROD_FREE), t0), t0);

    // Штатный тик через 5 минут — застой обнаружен и доставлен.
    let t1 = t0 + 300_000;
    let out1 = sim.tick(&inputs_with_hb(fresh_hb(t1, PROD_SEQ, PROD_FREE), t1), t1);
    deliveries += delivered(&out1, Incident::SeqStalled).len();
    assert_eq!(
        deliveries, 1,
        "первое обнаружение застоя обязано быть доставлено"
    );

    // Ручной повторный прогон через 5 секунд — «слишком рано судить».
    let t2 = t1 + 5_000;
    let out2 = sim.tick(&inputs_with_hb(fresh_hb(t2, PROD_SEQ, PROD_FREE), t2), t2);
    deliveries += delivered(&out2, Incident::SeqStalled).len();

    // Следующий штатный тик — инцидент тот же, окно подавления то же.
    let t3 = t2 + 300_000;
    let out3 = sim.tick(&inputs_with_hb(fresh_hb(t3, PROD_SEQ, PROD_FREE), t3), t3);
    deliveries += delivered(&out3, Incident::SeqStalled).len();

    assert_eq!(
        deliveries, 1,
        "тот же непрерывный застой доставлен {deliveries} раз(а) внутри 30-минутного окна \
         подавления — «не смог оценить» стёрло дедуп-память (R-005 F-5)"
    );
    assert!(
        !fired(&out3, Incident::SeqStalled).is_empty(),
        "инцидент обязан продолжать ОБНАРУЖИВАТЬСЯ (подавлена доставка, а не детекция) — иначе \
         watchdog просто замолчал"
    );
    assert!(
        out3.suppressed > 0,
        "подавление обязано быть видно в CycleOutcome::suppressed (аудит: «почему не пришло»)"
    );
}

/// Тот же инвариант на диске: recorder не смог `statvfs` (`free_bytes: None`) — это «не знаю»,
/// а не «место появилось».
#[test]
fn f5_unknown_disk_reading_does_not_reset_dedup_of_disk_low() {
    let sim = CronSim::new();
    let t0 = START_02_UTC;
    let mut deliveries = 0usize;

    // Диск ровно на полу disk-guard порога — CRITICAL без всякой истории.
    let out0 = sim.tick(
        &inputs_with_hb(fresh_hb(t0, PROD_SEQ, PROD_MIN_FREE), t0),
        t0,
    );
    deliveries += delivered(&out0, Incident::DiskLow).len();
    assert_eq!(
        deliveries, 1,
        "free_bytes ≤ min_free_bytes обязано алертить сразу"
    );

    // Такт, на котором recorder не знает про диск.
    let t1 = t0 + 300_000;
    let out1 = sim.tick(
        &inputs_with_hb(
            Some(hb_without_disk_fields(t1 - 3_000, PROD_SEQ + 28_800)),
            t1,
        ),
        t1,
    );
    deliveries += delivered(&out1, Incident::DiskLow).len();
    assert!(
        fired(&out1, Incident::DiskLow).is_empty(),
        "неизвестные диск-поля не должны ПОРОЖДАТЬ алерт — не додумываем за источник"
    );

    // Диск снова виден и всё так же на полу.
    let t2 = t1 + 300_000;
    let out2 = sim.tick(
        &inputs_with_hb(fresh_hb(t2, PROD_SEQ + 57_600, PROD_MIN_FREE), t2),
        t2,
    );
    deliveries += delivered(&out2, Incident::DiskLow).len();

    assert_eq!(
        deliveries, 1,
        "тот же непрерывный WD-DISK-LOW доставлен {deliveries} раз(а) внутри окна подавления — \
         «recorder не знает» стёрло дедуп-память (R-005 F-5)"
    );
}

/// ПАРНЫЙ VANTAGE к F-5: НАСТОЯЩЕЕ восстановление обязано снимать подавление, иначе новый
/// срыв внутри остаточного окна окажется проглочен. Ловит «фикс» вида «никогда не чистим».
#[test]
fn f5_genuine_recovery_does_reset_dedup_so_the_next_incident_is_delivered() {
    let sim = CronSim::new();
    let t0 = START_02_UTC;
    let mut deliveries = 0usize;

    let out0 = sim.tick(
        &inputs_with_hb(fresh_hb(t0, PROD_SEQ, PROD_MIN_FREE), t0),
        t0,
    );
    deliveries += delivered(&out0, Incident::DiskLow).len();

    // Ретеншен отработал — место вернулось, условие ЗДОРОВО (значение известно).
    let t1 = t0 + 300_000;
    let out1 = sim.tick(
        &inputs_with_hb(fresh_hb(t1, PROD_SEQ + 28_800, PROD_FREE), t1),
        t1,
    );
    assert!(
        fired(&out1, Incident::DiskLow).is_empty(),
        "здоровый диск не должен алертить"
    );

    // Новый срыв внутри того же 30-минутного окна — это НОВЫЙ инцидент, он обязан прийти.
    let t2 = t1 + 300_000;
    let out2 = sim.tick(
        &inputs_with_hb(fresh_hb(t2, PROD_SEQ + 57_600, PROD_MIN_FREE), t2),
        t2,
    );
    deliveries += delivered(&out2, Incident::DiskLow).len();

    assert_eq!(
        deliveries, 2,
        "после настоящего восстановления новый срыв обязан быть доставлен (доставок: \
         {deliveries}) — иначе подавление проглатывает реальные инциденты"
    );
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// F-6 — маркер `<job>.alert` («прогон УПАЛ») обязан читаться
// ══════════════════════════════════════════════════════════════════════════════════════════

fn failed_job(name: &str, reported_at_ms: Option<i64>, detail: &str) -> CronJobObservation {
    CronJobObservation {
        name: name.to_string(),
        // Позитивный маркер СВЕЖИЙ (последний УСПЕХ был ночью) — именно поэтому WD-CRON-STALE
        // промолчит ещё 26 часов, и упавший прогон виден только через `.alert`.
        last_success_ms: Some(START_02_UTC - 3 * 3_600_000),
        failure: Some(CronFailureMarker {
            reported_at_ms,
            detail: detail.to_string(),
        }),
    }
}

/// R-005 F-6: `deploy/README.md` §5 описывает ДВА сигнала — `*.alert` («прогон упал»,
/// пишется немедленно) и `*.last-success` («прогон случился»). Watchdog читал только второй,
/// поэтому упавший ретеншен оставался невидим 26 часов. Файл уже пишется на проде
/// (`deploy/bin/journal-retention-cron.sh:61`), читать его стоит столько же, сколько соседний.
#[test]
fn f6_failed_cron_run_is_detected_immediately_from_alert_marker() {
    // Код инцидента — якорь runbook'а и ключ дедупликации; фиксируем его здесь.
    assert_eq!(Incident::CronFailed.code(), "WD-CRON-FAILED");

    let sim = CronSim::new();
    let now = START_02_UTC;
    let inputs = CycleInputs {
        heartbeat: fresh_hb(now, PROD_SEQ, PROD_FREE),
        containers: healthy_containers(),
        cron_jobs: vec![
            failed_job(
                "retention",
                Some(now - 600_000),
                "dry-run exit=2 (2=failed_cold_verify — сегмент остался ГОРЯЧИМ)",
            ),
            CronJobObservation {
                name: "compaction".to_string(),
                last_success_ms: Some(now - 3 * 3_600_000),
                failure: None,
            },
        ],
    };

    let out = sim.tick(&inputs, now);
    let alerts = fired(&out, Incident::CronFailed);
    assert_eq!(
        alerts.len(),
        1,
        "упавший прогон ретеншена не обнаружен: маркер retention.alert не читается (R-005 F-6)"
    );
    assert_eq!(alerts[0].target.as_deref(), Some("retention"));
    assert_eq!(
        alerts[0].level,
        Level::Critical,
        "упавшая задача обслуживания журнала — CRITICAL: альтернатива — молчать 26 часов до \
         WD-CRON-STALE"
    );
    assert!(
        alerts[0].message.contains("exit=2"),
        "сообщение обязано нести содержимое маркера (спросонья нужна причина, а не «что-то \
         упало»): {}",
        alerts[0].message
    );
    assert_eq!(
        delivered(&out, Incident::CronFailed).len(),
        1,
        "первое обнаружение обязано быть доставлено"
    );
}

/// Отсутствие (`testing.md` п.3), fail-closed: маркер есть, но первая строка не распарсилась
/// (обрезанная запись, кривой `date`). Нечитаемый таймстамп — не разрешение молчать: сам факт
/// существования файла означает «последний прогон упал».
#[test]
fn f6_failure_marker_with_unparseable_timestamp_still_alerts() {
    let sim = CronSim::new();
    let now = START_02_UTC;
    let inputs = CycleInputs {
        heartbeat: fresh_hb(now, PROD_SEQ, PROD_FREE),
        containers: healthy_containers(),
        cron_jobs: vec![failed_job("compaction", None, "compact exit=1 (arg/io)")],
    };

    let out = sim.tick(&inputs, now);
    assert_eq!(
        fired(&out, Incident::CronFailed).len(),
        1,
        "маркер сбоя с нечитаемым таймстампом обязан алертить (fail-closed): факт файла = факт сбоя"
    );
}

/// Множественность (`testing.md` п.2): упали ДВЕ задачи разом — это два разных инцидента,
/// они не должны подавлять друг друга (ключ дедупа несёт цель).
#[test]
fn f6_two_failed_jobs_produce_two_distinct_delivered_alerts() {
    let sim = CronSim::new();
    let now = START_02_UTC;
    let inputs = CycleInputs {
        heartbeat: fresh_hb(now, PROD_SEQ, PROD_FREE),
        containers: healthy_containers(),
        cron_jobs: vec![
            failed_job(
                "retention",
                Some(now - 600_000),
                "dry-run exit=3 (disk_pressure)",
            ),
            failed_job(
                "compaction",
                Some(now - 900_000),
                "compact exit=2 (sha256 mismatch)",
            ),
        ],
    };

    let out = sim.tick(&inputs, now);
    let delivered_alerts = delivered(&out, Incident::CronFailed);
    assert_eq!(
        delivered_alerts.len(),
        2,
        "две упавшие задачи — два инцидента; доставлено: {}",
        delivered_alerts.len()
    );
    let targets: HashSet<String> = delivered_alerts
        .iter()
        .filter_map(|a| a.target.clone())
        .collect();
    assert_eq!(
        targets,
        HashSet::from(["retention".to_string(), "compaction".to_string()]),
        "цели инцидентов обязаны различаться (иначе дедуп схлопнет их в один)"
    );
}

/// ПАРНЫЙ VANTAGE: маркера сбоя нет, позитивный маркер свежий — тишина.
#[test]
fn f6_healthy_cron_jobs_produce_no_failure_alert() {
    let sim = CronSim::new();
    let now = START_02_UTC;
    let out = sim.tick(
        &inputs_with_hb(fresh_hb(now, PROD_SEQ, PROD_FREE), now),
        now,
    );
    assert!(
        fired(&out, Incident::CronFailed).is_empty(),
        "ложная тревога WD-CRON-FAILED на здоровых cron-задачах"
    );
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// F-7 — рестарт-петля обязана продолжать о себе сообщать
// ══════════════════════════════════════════════════════════════════════════════════════════

fn containers_with_restart_count(recorder_restarts: u64) -> Vec<ContainerStatus> {
    vec![
        ContainerStatus {
            name: "hft-recorder".to_string(),
            healthy: Some(true), // успел подняться к моменту проверки — healthcheck зелёный
            restart_count: Some(recorder_restarts),
        },
        ContainerStatus {
            name: "hft-gateway-serve".to_string(),
            healthy: Some(true),
            restart_count: Some(0),
        },
    ]
}

/// R-005 F-7: запись дедупа `WD-CONTAINER-RESTARTED:<name>` живёт все 30 минут окна, а
/// `check_container_restarted` идёт мимо ветки «условие здорово». Контейнер, падающий по
/// кругу, даёт ОДНО сообщение и дальше молчит — при том что каждый новый рестарт это НОВЫЙ
/// факт, а не повтор старого.
///
/// Контракт: за 2 часа рестарт-петли (5-минутный cron, рестарт на каждом такте) человек
/// получает не одно сообщение, сообщения РАЗНЫЕ (несут актуальный счётчик), и — ключевое —
/// САМЫЙ СВЕЖИЙ рестарт доставлен, а не проглочен остаточным окном подавления. Иначе
/// «периодическое напоминание» вырождается в отчёт о позапрошлом падении, а последние
/// (текущие!) рестарты человек не видит вовсе.
#[test]
fn f7_restart_loop_keeps_reporting_across_dedup_windows() {
    let sim = CronSim::new();
    let ticks = 24_i64;
    let mut delivered_messages: Vec<String> = Vec::new();
    let mut delivered_at_last_tick: Vec<String> = Vec::new();

    for i in 0..=ticks {
        // 2 часа по 5 минут
        let now = START_02_UTC + i * TICK_5MIN;
        let inputs = CycleInputs {
            heartbeat: fresh_hb(
                now,
                PROD_SEQ + (i as u64 * 300) * EVENTS_PER_SEC,
                PROD_FREE - i * BASELINE_DECLINE_PER_5MIN,
            ),
            containers: containers_with_restart_count(i as u64),
            cron_jobs: healthy_cron_jobs(now),
        };
        let out = sim.tick(&inputs, now);
        let msgs: Vec<String> = delivered(&out, Incident::ContainerRestarted)
            .into_iter()
            .map(|a| a.message)
            .collect();
        if i == ticks {
            delivered_at_last_tick = msgs.clone();
        }
        delivered_messages.extend(msgs);
    }

    // 2 часа / окно подавления 30 минут = 4 окна.
    assert!(
        delivered_messages.len() >= 4,
        "контейнер перезапускался {ticks} раз за 2 часа, доставлено сообщений: {} — рестарт-петля \
         замолчала после первого (R-005 F-7)",
        delivered_messages.len()
    );
    let distinct: HashSet<&String> = delivered_messages.iter().collect();
    assert!(
        distinct.len() >= 2,
        "все доставленные сообщения одинаковы — это повтор старого факта, а не новые рестарты"
    );
    assert!(
        delivered_at_last_tick
            .iter()
            .any(|m| m.contains(&ticks.to_string())),
        "самый свежий рестарт (RestartCount={ticks}) не доставлен — новые падения проглочены \
         окном подавления, человек читает отчёт о позапрошлом (R-005 F-7). Доставлено на \
         последнем такте: {delivered_at_last_tick:?}"
    );
}

/// Множественность (`testing.md` п.2): между двумя проверками контейнер упал ТРИ раза
/// (`RestartCount` 1 → 4). Это новый факт внутри окна подавления, он обязан прийти и обязан
/// нести актуальный счётчик.
#[test]
fn f7_multi_restart_jump_within_dedup_window_is_delivered_with_actual_counts() {
    let sim = CronSim::new();
    let t0 = START_02_UTC;

    // База.
    sim.tick(
        &CycleInputs {
            heartbeat: fresh_hb(t0, PROD_SEQ, PROD_FREE),
            containers: containers_with_restart_count(0),
            cron_jobs: healthy_cron_jobs(t0),
        },
        t0,
    );

    // Первый рестарт — доставлен.
    let t1 = t0 + TICK_5MIN;
    let out1 = sim.tick(
        &CycleInputs {
            heartbeat: fresh_hb(t1, PROD_SEQ + 28_800, PROD_FREE),
            containers: containers_with_restart_count(1),
            cron_jobs: healthy_cron_jobs(t1),
        },
        t1,
    );
    assert_eq!(
        delivered(&out1, Incident::ContainerRestarted).len(),
        1,
        "первый рестарт обязан быть доставлен"
    );

    // Ещё три рестарта внутри того же окна подавления.
    let t2 = t1 + TICK_5MIN;
    let out2 = sim.tick(
        &CycleInputs {
            heartbeat: fresh_hb(t2, PROD_SEQ + 57_600, PROD_FREE),
            containers: containers_with_restart_count(4),
            cron_jobs: healthy_cron_jobs(t2),
        },
        t2,
    );
    let delivered_alerts = delivered(&out2, Incident::ContainerRestarted);
    assert_eq!(
        delivered_alerts.len(),
        1,
        "рестарты 1→4 внутри окна подавления не доставлены — новый факт проглочен дедупом \
         (R-005 F-7)"
    );
    assert!(
        delivered_alerts[0].message.contains('4'),
        "сообщение обязано нести актуальный RestartCount: {}",
        delivered_alerts[0].message
    );
}

/// ПАРНЫЙ VANTAGE: рестарты прекратились — watchdog обязан замолчать. Ловит «фикс» вида
/// «слать WD-CONTAINER-RESTARTED каждый такт».
#[test]
fn f7_stable_container_after_restart_burst_stops_reporting() {
    let sim = CronSim::new();

    // 3 такта с рестартами.
    for i in 0..3_i64 {
        let now = START_02_UTC + i * TICK_5MIN;
        sim.tick(
            &CycleInputs {
                heartbeat: fresh_hb(now, PROD_SEQ + (i as u64 * 300) * EVENTS_PER_SEC, PROD_FREE),
                containers: containers_with_restart_count(i as u64),
                cron_jobs: healthy_cron_jobs(now),
            },
            now,
        );
    }

    // Час стабильной работы: счётчик не меняется.
    for i in 3..15_i64 {
        let now = START_02_UTC + i * TICK_5MIN;
        let out = sim.tick(
            &CycleInputs {
                heartbeat: fresh_hb(now, PROD_SEQ + (i as u64 * 300) * EVENTS_PER_SEC, PROD_FREE),
                containers: containers_with_restart_count(2),
                cron_jobs: healthy_cron_jobs(now),
            },
            now,
        );
        assert!(
            fired(&out, Incident::ContainerRestarted).is_empty(),
            "ложная тревога WD-CONTAINER-RESTARTED на стабильном контейнере (такт {i})"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// F-3 — прогноз диска обязан переживать ночное окно обслуживания
// ══════════════════════════════════════════════════════════════════════════════════════════

/// R-005 F-3: прогноз строится по производной между ДВУМЯ СОСЕДНИМИ сэмплами (5 минут).
/// Компакция (03:50 UTC) и ретеншен (04:07 UTC) пишут `.zst`-копии сегментов по 1 ГиБ —
/// 300 МБ за пять минут для них рядовая величина. Замер reviewer'а на прод-числах:
/// убыль 300 МБ за 5 мин → «[CRITICAL] диск кончится через ~19.1ч» при 82.8 ГБ свободных.
/// Пара таких ночей — и сообщения перестают читать.
///
/// Контракт: тренд считается по ИСТОРИИ, покрывающей окно обслуживания целиком (компакция
/// 03:50 → ретеншен 04:07, т.е. горизонт ≥ 2 ч), а не по соседней паре. Разовый всплеск в
/// одно окно не масштабируется на сутки. Абсолютный backstop (`free ≤ min_free`) не трогается —
/// см. парный vantage ниже.
///
/// Сценарий: 6 часов (02:00–08:00 UTC), тик каждые 5 минут, прод-числа. Ровный фон 117 КБ/с,
/// компакция даёт +300 МБ расхода на двух тактах, ретеншен возвращает 2 ГБ.
#[test]
fn f3_nightly_maintenance_spike_does_not_raise_disk_alert() {
    let sim = CronSim::new();
    let mut free = PROD_FREE;

    for i in 0..=72_i64 {
        let now = START_02_UTC + i * TICK_5MIN;
        free -= BASELINE_DECLINE_PER_5MIN;
        match i {
            // 03:50 и 03:55 UTC — компакция пишет .zst рядом с оригиналом.
            22 | 23 => free -= 300_000_000,
            // 04:10 UTC — ретеншен выгрузил сегменты в холодное хранилище и удалил горячие.
            26 => free += 2_000_000_000,
            _ => {}
        }
        let inputs = inputs_with_hb(
            fresh_hb(now, PROD_SEQ + (i as u64 * 300) * EVENTS_PER_SEC, free),
            now,
        );
        let out = sim.tick(&inputs, now);
        let alerts = fired(&out, Incident::DiskLow);
        assert!(
            alerts.is_empty(),
            "ложная тревога WD-DISK-LOW на такте {i} (UTC-смещение {} мин от 02:00, \
             free={free}, min_free={PROD_MIN_FREE} — запаса 72 ГБ): {:?} (R-005 F-3)",
            i * 5,
            alerts.iter().map(|a| &a.message).collect::<Vec<_>>()
        );
    }
}

/// ПАРНЫЙ VANTAGE: настоящее исчерпание обязано быть замечено. Устойчивая убыль ~1 МБ/с
/// съедает 72 ГБ запаса за ~20 часов — это CRITICAL, и он обязан прийти задолго до конца.
/// Ловит «фикс» вида «выключить прогноз».
#[test]
fn f3_sustained_real_decline_still_raises_critical() {
    let sim = CronSim::new();
    let per_tick = 300_000_000_i64; // ~1 МБ/с
    let mut first_critical_at: Option<i64> = None;

    for i in 0..=72_i64 {
        let now = START_02_UTC + i * TICK_5MIN;
        let free = PROD_FREE - i * per_tick;
        let out = sim.tick(
            &inputs_with_hb(
                fresh_hb(now, PROD_SEQ + (i as u64 * 300) * EVENTS_PER_SEC, free),
                now,
            ),
            now,
        );
        if first_critical_at.is_none()
            && fired(&out, Incident::DiskLow)
                .iter()
                .any(|a| a.level == Level::Critical)
        {
            first_critical_at = Some(i * TICK_5MIN);
        }
    }

    let at = first_critical_at
        .expect("устойчивая убыль ~1 МБ/с (72 ГБ запаса → ~20 часов) обязана дать CRITICAL");
    assert!(
        at <= 3 * 3_600_000,
        "CRITICAL пришёл через {at} мс — слишком поздно: сглаживание не должно превращаться в \
         слепоту (ожидание ≤ 3 часов)"
    );
}

/// ПАРНЫЙ VANTAGE #2: абсолютный backstop не зависит ни от какой истории — на полу
/// disk-guard порога алерт немедленный, на первом же такте.
#[test]
fn f3_absolute_floor_alerts_immediately_without_history() {
    let sim = CronSim::new();
    let now = START_02_UTC;
    let out = sim.tick(
        &inputs_with_hb(fresh_hb(now, PROD_SEQ, PROD_MIN_FREE), now),
        now,
    );
    let alerts = fired(&out, Incident::DiskLow);
    assert_eq!(
        alerts.len(),
        1,
        "free_bytes == min_free_bytes (ровно на пороге) обязано алертить на первом же такте"
    );
    assert_eq!(alerts[0].level, Level::Critical);
}

/// Границы + холодный старт: watchdog запущен впервые (истории нет) и сразу попал на всплеск
/// компакции. Двух точек недостаточно, чтобы утверждать «диск кончится через 19 часов» —
/// «мало данных» не равно «беда». Абсолютный backstop при этом продолжает работать (см. выше).
#[test]
fn f3_cold_start_spike_without_history_does_not_project_exhaustion() {
    let sim = CronSim::new();
    let t0 = START_02_UTC;
    let out0 = sim.tick(&inputs_with_hb(fresh_hb(t0, PROD_SEQ, PROD_FREE), t0), t0);
    assert!(fired(&out0, Incident::DiskLow).is_empty());

    let t1 = t0 + TICK_5MIN;
    let out1 = sim.tick(
        &inputs_with_hb(
            fresh_hb(
                t1,
                PROD_SEQ + 28_800,
                PROD_FREE - BASELINE_DECLINE_PER_5MIN - 300_000_000,
            ),
            t1,
        ),
        t1,
    );
    let alerts = fired(&out1, Incident::DiskLow);
    assert!(
        alerts.is_empty(),
        "прогноз по ДВУМ точкам на холодном старте (запас 72 ГБ) — ложная тревога: {:?}",
        alerts.iter().map(|a| &a.message).collect::<Vec<_>>()
    );
}

/// Отсутствие: часть тактов без диск-полей (recorder не смог `statvfs`). Пропуски не должны
/// ни алертить сами по себе, ни портить тренд.
#[test]
fn f3_unknown_disk_ticks_neither_alert_nor_corrupt_the_trend() {
    let sim = CronSim::new();
    let mut free = PROD_FREE;

    for i in 0..=72_i64 {
        let now = START_02_UTC + i * TICK_5MIN;
        free -= BASELINE_DECLINE_PER_5MIN;
        let seq = PROD_SEQ + (i as u64 * 300) * EVENTS_PER_SEC;
        let heartbeat = if (20..=24).contains(&i) {
            Some(hb_without_disk_fields(now - 3_000, seq))
        } else {
            fresh_hb(now, seq, free)
        };
        let out = sim.tick(&inputs_with_hb(heartbeat, now), now);
        assert!(
            fired(&out, Incident::DiskLow).is_empty(),
            "ложная тревога WD-DISK-LOW на такте {i} при пропусках диск-полей"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// Сквозные vantage-оракулы (прод-масштаб)
// ══════════════════════════════════════════════════════════════════════════════════════════

/// Главный анти-шумовой vantage: 2 часа полностью здорового прода (24 запуска cron'а) — НОЛЬ
/// алертов и ноль доставок. Сторож, который шумит на здоровой системе, будет выключен
/// человеком, и тогда мы вернёмся ровно к тому, ради чего всё это пишется.
#[test]
fn fully_healthy_two_hour_run_delivers_zero_alerts() {
    let sim = CronSim::new();
    for i in 0..=24_i64 {
        let now = START_02_UTC + i * TICK_5MIN;
        let out = sim.tick(
            &inputs_with_hb(
                fresh_hb(
                    now,
                    PROD_SEQ + (i as u64 * 300) * EVENTS_PER_SEC,
                    PROD_FREE - i * BASELINE_DECLINE_PER_5MIN,
                ),
                now,
            ),
            now,
        );
        assert!(
            out.fired.is_empty(),
            "ложная тревога на здоровом проде, такт {i}: {:?}",
            out.fired
                .iter()
                .map(|a| (a.incident.code(), &a.message))
                .collect::<Vec<_>>()
        );
        assert!(out.delivered.is_empty());
    }
}

/// Прод-масштаб (`testing.md`, «граница ресурса, не только корректность»): неделя работы —
/// 2016 запусков cron'а по 5 минут. Файл состояния лежит в `/var/lib/hft` рядом с журналом и
/// читается/пишется КАЖДЫЙ запуск: история для тренда обязана быть ограниченной. Оракул
/// падает на реализации, которая накапливает сэмплы без границы.
#[test]
fn state_file_stays_bounded_over_a_week_of_cron_runs() {
    let sim = CronSim::new();
    let mut free = PROD_FREE;
    for i in 0..2016_i64 {
        let now = START_02_UTC + i * TICK_5MIN;
        // Ретеншен раз в сутки возвращает место — иначе за неделю уйдём ниже min_free.
        free -= BASELINE_DECLINE_PER_5MIN;
        if i % 288 == 0 {
            free = PROD_FREE;
        }
        sim.tick(
            &inputs_with_hb(
                fresh_hb(now, PROD_SEQ + (i as u64 * 300) * EVENTS_PER_SEC, free),
                now,
            ),
            now,
        );
    }
    let len = sim.state_file_len();
    assert!(
        len < 64 * 1024,
        "файл состояния вырос до {len} байт за неделю запусков — история не ограничена"
    );
}

// ══════════════════════════════════════════════════════════════════════════════════════════
// F-10 (BLOCKER, R-009) — регрессия `next_seq` как ОТДЕЛЬНОЕ событие + сброс якоря
// ══════════════════════════════════════════════════════════════════════════════════════════
//
// Находка R-008 F-8 («уменьшение `next_seq` стопорит `WD-SEQ-STALLED` навечно») починена
// коммитом `62f56cd` — но БЕЗ оракула. Reviewer в R-009 это показал двумя мутациями:
//   * **B** — ветка регрессии удалена целиком: `passed=146 failed=0`, `verify` упал ТОЛЬКО на
//     clippy (неиспользованный импорт). Линтер — не поведенческий гейт.
//   * **B2** — ветка на месте, убраны ДВЕ строки сброса якоря (`seq_progress_heartbeat`/
//     `seq_progress_check_ms` на текущее значение), то есть ровно ядро фикса: линтер доволен,
//     `passed=146 failed=0`, `verify → VERDICT: PASS`.
//
// Оракулы ниже обязаны падать на ОБЕИХ мутациях, поэтому смотрят на ДВА разных наблюдаемых
// следствия:
//   1. на такте регрессии обязан прийти ИМЕННО `WD-SEQ-REGRESSED` (мутация B убирает его);
//   2. регрессия — СОБЫТИЕ, а не состояние: на последующих тактах нормального роста от нового
//      (низкого) значения не должно быть НИ повторных `WD-SEQ-REGRESSED`, НИ `WD-SEQ-STALLED`.
//      Мутация B2 даёт бесконечный поток `WD-SEQ-REGRESSED` (якорь остался на старом высоком
//      значении, каждый следующий сэмпл «меньше якоря»); удаление ветки даёт бесконечный
//      `WD-SEQ-STALLED`. На проде `next_seq ≈ 1.4e8` при 96 ev/s — догонять старое значение
//      ~17 суток, то есть ~17 суток непрерывного ложного CRITICAL. Сторож, который так себя
//      ведёт, выключают — а выключенный сторож равен отсутствующему.
//
// Почему это тревога, а не тихая нормализация: уменьшение `next_seq` — признак seq-reuse
// (пересозданный/восстановленный из бэкапа том, откат сегмента), та же категория риска, что
// закрывали M-49/M-50.
//
// # Чек-лист деградированного входа (`.claude/rules/testing.md`)
// - **Асимметрия**: регрессия наступает ПОСЛЕ нормального роста, а не с первого такта;
//   меняется только `next_seq` (heartbeat свежий, диск ровный, контейнеры healthy).
// - **Множественность**: ДВЕ независимые регрессии внутри одного 30-минутного дедуп-окна —
//   обе обязаны быть доставлены (это разные факты, а не повтор одного).
// - **Отсутствие**: нечитаемый heartbeat между тактами не создаёт ложной регрессии и не
//   стирает якорь — регрессия за ним всё равно детектируется.
// - **Границы**: `next_seq` РАВЕН якорю — это застой (`WD-SEQ-STALLED`), а не регрессия;
//   реализация через `<=` вместо `<` заглушила бы детектор застоя целиком.
// - **Прод-масштаб**: 24 часа (288 запусков cron'а) после регрессии на прод-числах и
//   прод-темпе; состояние проходит через JSON на каждом такте (`CronSim::tick` = путь `main()`).

/// Значение `next_seq` сразу после пересоздания тома журнала: сбор стартует почти с нуля,
/// а старый якорь стоит на прод-масштабе (1.4e8).
const RECREATED_VOLUME_SEQ: u64 = 1_024;

/// Прогресс `next_seq` за один 5-минутный такт при прод-темпе 96 ev/s.
const SEQ_PER_TICK: u64 = EVENTS_PER_SEC * (TICK_5MIN as u64) / 1000;

/// Прогнать `ticks` тактов нормального роста от `start_seq`, вернуть (время следующего такта,
/// достигнутый `next_seq`, суммарные счётчики сработавших seq-инцидентов).
fn run_growth(sim: &CronSim, mut now: i64, mut seq: u64, ticks: usize) -> (i64, u64, usize, usize) {
    let (mut regressed, mut stalled) = (0usize, 0usize);
    for _ in 0..ticks {
        let out = sim.tick(&inputs_with_hb(fresh_hb(now, seq, PROD_FREE), now), now);
        regressed += fired(&out, Incident::SeqRegressed).len();
        stalled += fired(&out, Incident::SeqStalled).len();
        now += TICK_5MIN;
        seq += SEQ_PER_TICK;
    }
    (now, seq, regressed, stalled)
}

/// ГЛАВНЫЙ оракул F-10 (часть 1): уменьшение `next_seq` распознаётся как СОБСТВЕННЫЙ
/// инцидент `WD-SEQ-REGRESSED` уровня CRITICAL — не как застой и не как «всё хорошо».
/// Мутация B (удаление ветки) валит этот тест.
#[test]
fn f10_next_seq_regression_fires_its_own_critical_incident_not_a_stall() {
    let sim = CronSim::new();
    let (now, seq_before, _, _) = run_growth(&sim, START_02_UTC, PROD_SEQ, 3);

    // Том журнала пересоздан/восстановлен из бэкапа — `next_seq` стартовал заново.
    let out = sim.tick(
        &inputs_with_hb(fresh_hb(now, RECREATED_VOLUME_SEQ, PROD_FREE), now),
        now,
    );

    let regressed = delivered(&out, Incident::SeqRegressed);
    assert_eq!(
        regressed.len(),
        1,
        "уменьшение next_seq ({} → {RECREATED_VOLUME_SEQ}) не дало WD-SEQ-REGRESSED — признак \
         seq-reuse проглочен (R-008 F-8 / R-009 F-10)",
        seq_before - SEQ_PER_TICK
    );
    assert_eq!(
        regressed[0].level,
        Level::Critical,
        "регрессия next_seq — это CRITICAL (та же категория риска, что M-49/M-50 seq-reuse)"
    );
    assert!(
        fired(&out, Incident::SeqStalled).is_empty(),
        "такт регрессии переинтерпретирован как застой — это разные факты о мире: {:?}",
        fired(&out, Incident::SeqStalled)
    );
    let msg = &regressed[0].message;
    assert!(
        msg.contains(&(seq_before - SEQ_PER_TICK).to_string())
            && msg.contains(&RECREATED_VOLUME_SEQ.to_string()),
        "сообщение не называет ОБА значения (было → стало), разбирать инцидент нечем: {msg}"
    );
}

/// ГЛАВНЫЙ оракул F-10 (часть 2) — ЯДРО фикса, ровно мутация B2 reviewer'а.
///
/// После регрессии якорь прогресса обязан немедленно переехать на новое (меньшее) значение.
/// Тогда нормальный рост от нового старта — это ТИШИНА. Без сброса якоря каждый следующий
/// сэмпл «меньше якоря» → бесконечный поток CRITICAL (мутация B2); при удалённой ветке →
/// бесконечный ложный `WD-SEQ-STALLED` (мутация B). Прод-масштаб: 24 часа, 288 запусков cron'а.
#[test]
fn f10_after_regression_normal_growth_from_the_new_baseline_stays_silent_for_a_day() {
    let sim = CronSim::new();
    let (now, _, _, _) = run_growth(&sim, START_02_UTC, PROD_SEQ, 3);

    // Такт регрессии — здесь тревога законна и проверена тестом выше.
    let out = sim.tick(
        &inputs_with_hb(fresh_hb(now, RECREATED_VOLUME_SEQ, PROD_FREE), now),
        now,
    );
    assert_eq!(
        delivered(&out, Incident::SeqRegressed).len(),
        1,
        "предусловие оракула не выполнено: регрессия не сработала — см. \
         f10_next_seq_regression_fires_its_own_critical_incident_not_a_stall"
    );

    // 24 часа нормального роста от НОВОГО (низкого) значения — 288 запусков cron'а.
    let ticks_per_day = (24 * 3_600_000) / TICK_5MIN as usize;
    let (_, seq_end, regressed_after, stalled_after) = run_growth(
        &sim,
        now + TICK_5MIN,
        RECREATED_VOLUME_SEQ + SEQ_PER_TICK,
        ticks_per_day,
    );

    assert_eq!(
        regressed_after, 0,
        "WD-SEQ-REGRESSED повторился {regressed_after} раз за сутки нормального роста — якорь не \
         сброшен на текущее значение (R-009 F-10, мутация B2): регрессия превращена из СОБЫТИЯ в \
         вечное состояние"
    );
    assert_eq!(
        stalled_after, 0,
        "WD-SEQ-STALLED сработал {stalled_after} раз за сутки РАСТУЩЕГО сбора — детектор гонится \
         за недостижимым старым значением (на проде это ~17 суток ложного CRITICAL)"
    );
    assert!(
        seq_end < PROD_SEQ,
        "сценарий выродился: сбор догнал старое значение ({seq_end} ≥ {PROD_SEQ}) — оракул \
         перестал давить на инвариант, увеличьте разрыв"
    );
}

/// Границы (`testing.md` п.4): `next_seq` РАВЕН якорю — это застой, а НЕ регрессия. Ловит
/// «фикс» через `<=` вместо `<`: он бы объявлял регрессией любой застой и тем самым глушил
/// `WD-SEQ-STALLED` — то есть чинил бы F-8 ценой возврата F-1.
#[test]
fn f10_flat_next_seq_is_a_stall_not_a_regression() {
    let sim = CronSim::new();
    let (mut now, seq, _, _) = run_growth(&sim, START_02_UTC, PROD_SEQ, 3);
    let frozen = seq - SEQ_PER_TICK; // сбор встал ровно на достигнутом значении

    let (mut stalled, mut regressed) = (0usize, 0usize);
    for _ in 0..6 {
        // 30 минут заморозки
        let out = sim.tick(&inputs_with_hb(fresh_hb(now, frozen, PROD_FREE), now), now);
        stalled += fired(&out, Incident::SeqStalled).len();
        regressed += fired(&out, Incident::SeqRegressed).len();
        now += TICK_5MIN;
    }

    assert!(
        stalled > 0,
        "замороженный next_seq не дал WD-SEQ-STALLED — детектор застоя выключен (регресс к F-1)"
    );
    assert_eq!(
        regressed, 0,
        "застой (next_seq РАВЕН якорю) объявлен регрессией {regressed} раз — реализация \
         использует `<=` вместо `<`"
    );
}

/// Множественность (`testing.md` п.2): ДВЕ независимые регрессии внутри ОДНОГО 30-минутного
/// дедуп-окна. Это два разных факта (второе уменьшение — уже относительно сброшенного якоря),
/// обе обязаны быть ДОСТАВЛЕНЫ. Ловит «фикс», который сбрасывает якорь, но глушит второе
/// событие остаточным окном подавления (та же ловушка, что F-7 с рестартами).
#[test]
fn f10_two_independent_regressions_within_one_dedup_window_are_both_delivered() {
    let sim = CronSim::new();
    let (mut now, _, _, _) = run_growth(&sim, START_02_UTC, PROD_SEQ, 3);
    let mut delivered_total = 0usize;

    // Регрессия №1.
    let out = sim.tick(
        &inputs_with_hb(fresh_hb(now, RECREATED_VOLUME_SEQ, PROD_FREE), now),
        now,
    );
    delivered_total += delivered(&out, Incident::SeqRegressed).len();
    now += TICK_5MIN;

    // 15 минут нормального роста — тишина (внутри дедуп-окна 30 мин).
    let (next_now, seq, regressed_between, _) =
        run_growth(&sim, now, RECREATED_VOLUME_SEQ + SEQ_PER_TICK, 3);
    assert_eq!(
        regressed_between, 0,
        "между регрессиями сторож шумел — якорь не сброшен (мутация B2)"
    );

    // Регрессия №2 — том пересоздан ВТОРОЙ раз, всё ещё внутри 30-минутного окна.
    let out = sim.tick(
        &inputs_with_hb(fresh_hb(next_now, 512, PROD_FREE), next_now),
        next_now,
    );
    delivered_total += delivered(&out, Incident::SeqRegressed).len();
    assert!(
        seq > 512,
        "сценарий выродился: второе значение не меньше достигнутого ({seq})"
    );

    assert_eq!(
        delivered_total, 2,
        "две независимые регрессии дали {delivered_total} доставленных алертов — второе \
         (актуальное!) событие проглочено дедуп-окном первого"
    );
}

/// Отсутствие (`testing.md` п.3): на одном такте heartbeat нечитаем (файл читается в момент
/// записи / том отвалился). Это «не смог прочитать», а не «мир изменился»: якорь обязан
/// пережить такой такт, и регрессия ЗА НИМ обязана быть замечена.
#[test]
fn f10_unreadable_heartbeat_tick_neither_fakes_nor_hides_a_regression() {
    let sim = CronSim::new();
    let (mut now, seq_before, _, _) = run_growth(&sim, START_02_UTC, PROD_SEQ, 3);

    // Такт без heartbeat'а: сам по себе не регрессия.
    let out = sim.tick(&inputs_with_hb(None, now), now);
    assert!(
        fired(&out, Incident::SeqRegressed).is_empty(),
        "нечитаемый heartbeat выдан за регрессию next_seq — додумывание за источник"
    );
    now += TICK_5MIN;

    // Регрессия сразу после пропуска — якорь обязан быть цел.
    let out = sim.tick(
        &inputs_with_hb(fresh_hb(now, RECREATED_VOLUME_SEQ, PROD_FREE), now),
        now,
    );
    assert_eq!(
        delivered(&out, Incident::SeqRegressed).len(),
        1,
        "регрессия после нечитаемого такта не замечена — якорь потерян на пропуске \
         (было {} → стало {RECREATED_VOLUME_SEQ})",
        seq_before - SEQ_PER_TICK
    );
}

/// ПАРНЫЙ VANTAGE: здоровый растущий сбор НИКОГДА не даёт `WD-SEQ-REGRESSED`. Ловит «фикс»
/// вида «слать регрессию всегда» и любую путаницу знака сравнения.
#[test]
fn f10_healthy_growth_never_fires_seq_regressed() {
    let sim = CronSim::new();
    let (_, _, regressed, stalled) = run_growth(&sim, START_02_UTC, PROD_SEQ, 24); // 2 часа
    assert_eq!(
        regressed, 0,
        "здоровый растущий сбор дал {regressed} ложных WD-SEQ-REGRESSED"
    );
    assert_eq!(
        stalled, 0,
        "здоровый растущий сбор дал {stalled} ложных WD-SEQ-STALLED"
    );
}
