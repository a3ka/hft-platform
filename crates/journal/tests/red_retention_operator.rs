//! SACRED (architect-only) — TD-020: ОПЕРАТОРСКИЙ путь ретеншена.
//!
//! **Провал M-08, найденный reviewer'ом на §8:** `verify_cold_copy` / `prune_segment` /
//! `ColdCopyProof` были написаны как БИБЛИОТЕКА — и их не вызывает НИКТО (ни recorder, ни CLI,
//! ни cron). Главная цель milestone'а («сбор данных не остановится НИКОГДА») поэтому НЕ
//! достигнута: диск растёт те же ~2.8 GB/сут, просто кусками по 1 GiB; ~40 дней до disk-guard.
//! Я спроектировал типовой барьер и забыл спроектировать ОПЕРАТОРА.
//!
//! Контракт (architect):
//!  - отдельный бинарь `journal-retention` + cron; **не поток внутри recorder'а** — падение
//!    уборки не имеет права ронять СБОР (сбор дороже уборки);
//!  - `retention_plan(dir, policy, now_wall_ms)` — детерминирован (часы снаружи);
//!  - **`DryRun` — дефолт**: ноль побочных эффектов (первый прогон на проде — обязательно он);
//!  - `Apply`: сверка холодной копии → `ColdCopyProof` → и только потом `prune`.
//!
//! Деградированные входы (правило `.claude/rules/testing.md`): активный сегмент, legacy без
//! декларации, битая холодная копия, нехватка места при пустом плане.

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{Journal, RetentionMode, RetentionPolicy, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const T0: i64 = 1_752_000_000_000;

fn trade(i: u64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: contracts::to_fixed(65_000.0) + i as i64,
            size: contracts::to_fixed(0.01),
            side: Side::Buy,
            ts_exch_ms: T0 + i as i64,
        },
    )
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 16 * 1024, // много сегментов
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "retention fixture".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn policy(cold: &std::path::Path, retain_days: u32, keep_min: u32) -> RetentionPolicy {
    RetentionPolicy {
        retain_days,
        keep_min_segments: keep_min,
        cold_root: cold.to_path_buf(),
        min_free_bytes: 0,
    }
}

/// Журнал с несколькими сегментами.
fn journal_with_segments(dir: &std::path::Path, n: u64) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in 0..n {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
}

/// R1 (ГЛАВНОЕ): АКТИВНЫЙ сегмент никогда не попадает в план — в него пишут ПРЯМО СЕЙЧАС.
/// Удалить его = потерять хвост боевых данных и оборвать запись.
#[test]
fn r1_active_segment_is_never_planned_for_prune() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    journal_with_segments(dir.path(), 3_000);

    let segs = journal::list_segments(dir.path()).expect("segments");
    assert!(segs.len() > 2, "предусловие: несколько сегментов");
    let active = segs.last().expect("active").path.clone();

    // Все сегменты «старые» (now = T0 + 100 дней), keep_min = 0 → максимально агрессивный план.
    let plan = journal::retention_plan(dir.path(), &policy(cold.path(), 1, 0), T0 + 100 * DAY_MS)
        .expect("plan");

    assert!(
        plan.offload_and_prune.iter().all(|s| s.path != active),
        "АКТИВНЫЙ сегмент попал в план удаления — уборка оборвёт запись и съест свежие данные"
    );
    assert!(
        plan.skipped.iter().any(|(s, _)| s.path == active),
        "активный сегмент обязан быть в skipped С ПРИЧИНОЙ (молчаливый пропуск = непрозрачно)"
    );
}

/// R2: `DryRun` — НОЛЬ побочных эффектов. Первый прогон на проде идёт только так;
/// реализация, которая «на всякий случай уже скопировала», нарушает контракт.
#[test]
fn r2_dry_run_touches_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    journal_with_segments(dir.path(), 3_000);

    let before: Vec<(std::path::PathBuf, u64)> = journal::list_segments(dir.path())
        .expect("segs")
        .iter()
        .map(|s| (s.path.clone(), s.size_bytes))
        .collect();

    let plan = journal::retention_plan(dir.path(), &policy(cold.path(), 1, 0), T0 + 100 * DAY_MS)
        .expect("plan");
    let report = journal::retention_execute(
        dir.path(),
        &plan,
        &policy(cold.path(), 1, 0),
        RetentionMode::DryRun,
    )
    .expect("dry-run");

    assert_eq!(report.mode, RetentionMode::DryRun);
    assert!(report.pruned.is_empty(), "DryRun УДАЛИЛ сегменты");
    assert!(report.offloaded.is_empty(), "DryRun СКОПИРОВАЛ сегменты");
    assert_eq!(report.freed_bytes, 0);

    let after: Vec<(std::path::PathBuf, u64)> = journal::list_segments(dir.path())
        .expect("segs")
        .iter()
        .map(|s| (s.path.clone(), s.size_bytes))
        .collect();
    assert_eq!(before, after, "DryRun изменил горячие сегменты");
    assert_eq!(
        std::fs::read_dir(cold.path()).expect("cold").count(),
        0,
        "DryRun записал что-то в холодное хранилище"
    );
}

/// R3: `Apply` — только через сверку. Порченая холодная копия → сегмент ОСТАЁТСЯ горячим
/// и попадает в `failed` (оператор обязан узнать), а не «удалён, потому что скопировали».
#[test]
fn r3_apply_prunes_only_after_verified_cold_copy() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    journal_with_segments(dir.path(), 3_000);

    let pol = policy(cold.path(), 1, 0);
    let plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).expect("plan");
    assert!(
        !plan.offload_and_prune.is_empty(),
        "план обязан быть непустым"
    );
    let victim = plan.offload_and_prune[0].clone();

    let report =
        journal::retention_execute(dir.path(), &plan, &pol, RetentionMode::Apply).expect("apply");

    assert!(
        report.pruned.contains(&victim.path),
        "честно выгруженный сегмент обязан быть удалён из горячей копии"
    );
    assert!(!victim.path.exists(), "горячая копия осталась");
    let cold_copy = cold.path().join(victim.path.file_name().expect("name"));
    assert!(cold_copy.exists(), "холодной копии нет — данные ПОТЕРЯНЫ");
    assert!(report.freed_bytes > 0);

    // Деградированный вход: холодное хранилище недоступно на запись → prune ЗАПРЕЩЁН.
    let dir2 = tempfile::tempdir().expect("dir2");
    journal_with_segments(dir2.path(), 3_000);
    let bad_cold = std::path::PathBuf::from("/proc/hft-nonexistent-cold-root");
    let pol2 = RetentionPolicy {
        cold_root: bad_cold,
        ..policy(cold.path(), 1, 0)
    };
    let plan2 = journal::retention_plan(dir2.path(), &pol2, T0 + 100 * DAY_MS).expect("plan2");
    let segs_before = journal::list_segments(dir2.path()).expect("segs").len();
    let r2 = journal::retention_execute(dir2.path(), &plan2, &pol2, RetentionMode::Apply);
    match r2 {
        Err(_) => {}
        Ok(rep) => assert!(
            rep.pruned.is_empty() && !rep.failed.is_empty(),
            "выгрузка провалилась, а сегменты удалены — это потеря единственной копии данных"
        ),
    }
    assert_eq!(
        journal::list_segments(dir2.path()).expect("segs").len(),
        segs_before,
        "при недоступном холодном хранилище горячие сегменты обязаны остаться на месте"
    );
}

/// R4: `keep_min_segments` последних остаются горячими независимо от возраста —
/// недавний реплей/диагностика не должны ходить в холодное хранилище.
#[test]
fn r4_keep_min_segments_are_never_pruned() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    journal_with_segments(dir.path(), 4_000);

    let segs = journal::list_segments(dir.path()).expect("segs");
    let keep = 3usize;
    let protected: Vec<_> = segs
        .iter()
        .rev()
        .take(keep)
        .map(|s| s.path.clone())
        .collect();

    let plan = journal::retention_plan(
        dir.path(),
        &policy(cold.path(), 1, keep as u32),
        T0 + 100 * DAY_MS,
    )
    .expect("plan");

    for p in &protected {
        assert!(
            plan.offload_and_prune.iter().all(|s| &s.path != p),
            "сегмент из keep_min_segments попал в план удаления: {p:?}"
        );
    }
}

/// R5: legacy-сегмент БЕЗ декларации не удаляется. Нет эпохи → нет права его трогать:
/// это может быть чужой/неизвестный файл, а данные — единственная копия.
#[test]
fn r5_undeclared_legacy_segment_is_never_pruned() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    // Сегмент старого формата (без магии), манифеста нет.
    {
        let mut j = Journal::open(dir.path()).expect("legacy open");
        for i in 0..500 {
            j.append(trade(i)).expect("append");
        }
        j.flush().expect("flush");
    }
    journal_with_segments(dir.path(), 3_000); // + новые сегменты v2

    let pol = policy(cold.path(), 1, 0);
    // Либо план строится и НЕ включает незадекларированный legacy, либо план вообще
    // отказывается строиться (тоже fail-closed) — но молча удалить его нельзя.
    if let Ok(plan) = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS) {
        let legacy = dir.path().join("segment-00000000.jrnl");
        assert!(
            plan.offload_and_prune.iter().all(|s| s.path != legacy),
            "НЕЗАДЕКЛАРИРОВАННЫЙ legacy-сегмент попал в план удаления — у него нет эпохи, \
             значит нет и права его удалять (это может быть чужой файл, а копия одна)"
        );
    }
}

/// R6: план ДЕТЕРМИНИРОВАН — часы приходят снаружи; два вызова с одним `now` идентичны.
#[test]
fn r6_plan_is_deterministic() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    journal_with_segments(dir.path(), 3_000);
    let pol = policy(cold.path(), 1, 1);

    let a = journal::retention_plan(dir.path(), &pol, T0 + 50 * DAY_MS).expect("a");
    let b = journal::retention_plan(dir.path(), &pol, T0 + 50 * DAY_MS).expect("b");
    assert_eq!(
        a, b,
        "план обязан быть воспроизводим (никакого wall-clock внутри)"
    );
}

/// R7: диск кончается, а выгружать нечего (всё в keep_min / активное) → это НЕ «ок»,
/// это ТРЕВОГА: сбор данных остановится, и оператор обязан узнать заранее.
#[test]
fn r7_disk_pressure_with_empty_plan_is_flagged() {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    journal_with_segments(dir.path(), 1_000);

    let pol = RetentionPolicy {
        retain_days: 3650,        // ничего не устарело
        keep_min_segments: 1_000, // всё защищено
        cold_root: cold.path().to_path_buf(),
        min_free_bytes: u64::MAX, // места «не хватает» заведомо
    };
    let plan = journal::retention_plan(dir.path(), &pol, T0 + DAY_MS).expect("plan");

    assert!(
        plan.offload_and_prune.is_empty(),
        "предусловие: удалять нечего"
    );
    assert!(
        plan.disk_pressure,
        "места не хватает, а выгружать нечего — план обязан ПОДНЯТЬ ФЛАГ (иначе сбор данных \
         тихо остановится по disk-guard, и мы узнаем об этом постфактум)"
    );
}
