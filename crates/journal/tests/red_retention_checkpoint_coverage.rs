//! RED M-38b rev2 (sacred, architect-only) — **C-030 R1: локальный prune требует ДОКАЗАННОГО
//! покрытия чекпоинтом; иначе сегмент остаётся горячим и попадает в skip-репорт.**
//!
//! ## Почему связка обязана быть строгой (решение critic C-030, принято)
//!
//! `docs/06` определяет ретеншен как УПРАВЛЯЕМОЕ перемещение, а не тихое удаление. После M-38b
//! у read-пути появляется состояние, свёрнутое ДО определённого `seq`. Если retention удалит
//! локальный префикс, который чекпоинт ещё не свернул, то:
//! - `snapshot_from_checkpoint` уйдёт в rebuild (или пересчёт) по остаткам и молча вернёт
//!   УСЕЧЁННУЮ историю — all-time VWAP (VB-I-6) поедет без единой ошибки;
//! - восстановить нечем: локальные данные удалены, а холодная копия read-путём не читается.
//!
//! Это тихая ложь в данных кокпита, поэтому дефолт — fail-closed.
//!
//! ## Критерий покрытия (селектор-агностичный — journal НЕ знает про селекторы)
//!
//! `gateway-checkpoint` публикует ОДНО число: `covered_through_seq` (минимум по всем
//! сконфигурированным селекторам). Journal потребляет только его:
//!
//! ```text
//! сегмент prunable ⟺ last_seq(сегмент) <= covered_through_seq
//!                  ⟺ first_seq(следующий сегмент) <= covered_through_seq + 1
//! ```
//!
//! `last_seq` считается по заголовку СЛЕДУЮЩЕГО сегмента — читать сам сегмент не нужно.
//!
//! ## Offload НЕ гейтится — гейтится только PRUNE
//!
//! Иначе строгость заблокировала бы R1 (offsite-бэкап, экзистенциальный риск docs/08). Поэтому
//! план разделяется: `offload_and_prune` (покрыт → копия в cold + удаление локальной) и
//! `offload_only` (устарел, но НЕ покрыт → копия в cold, локальная ОСТАЁТСЯ, skip-репорт).
//! Бэкап продолжает работать всегда; ждёт только освобождение места.
//!
//! ## Явный операторский override (ОТКЛОНЕНИЕ от буквы C-030 — architect, для повторного критика)
//!
//! Критик потребовал «иначе не prune». Буквальная строгость означает: если чекпоинтер сломан
//! или остановлен, место не освобождается НИКОГДА → disk-guard остановит ЗАПИСЬ (потеря НОВЫХ
//! данных ради старых). Поэтому добавлен `allow_prune_without_checkpoint` — **не дефолт**,
//! задаётся явным флагом оператора и ОБЯЗАН быть назван в отчёте (аудит-трейл). Дефолт
//! остаётся fail-closed. Если критик считает escape-hatch недопустимым — удаляется вместе с
//! `override_prunes_but_is_named_in_report`.
//!
//! COMPILE-RED: `RetentionPolicy.checkpoint_covered_through_seq`,
//! `RetentionPolicy.allow_prune_without_checkpoint`, `RetentionPlan.offload_only`,
//! `RetentionReport.pruned_without_checkpoint_coverage` ещё не существуют.
//!
//! testing.md: п.4 границы (покрытие ровно на `last_seq` и на `last_seq − 1`), п.7 парный
//! vantage (покрытый сегмент ОБЯЗАН пруниться — иначе гвард выродится в «не пруним никогда»),
//! п.3 отсутствие (нет артефакта покрытия → не «считать покрытым»).

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{Journal, RetentionMode, RetentionPolicy, WriterConfig};

const DAY_MS: i64 = 86_400_000;
const T0: i64 = 1_752_000_000_000;
const N: u64 = 900;

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 16 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

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

fn journal_with_segments(dir: &std::path::Path) {
    let mut j = Journal::open_with(dir, cfg()).expect("open_with");
    for i in 0..N {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
}

/// Политика с явным покрытием чекпоинта. `covered = None` — артефакта покрытия нет.
fn policy(cold: &std::path::Path, covered: Option<u64>, allow_override: bool) -> RetentionPolicy {
    RetentionPolicy {
        retain_days: 1,
        keep_min_segments: 1,
        cold_root: cold.to_path_buf(),
        min_free_bytes: 0,
        checkpoint_covered_through_seq: covered,
        allow_prune_without_checkpoint: allow_override,
    }
}

/// `last_seq` сегмента `idx` = `first_seq` следующего − 1. Для последнего — None (активный).
fn last_seq_of(dir: &std::path::Path, idx: u32) -> Option<u64> {
    let mut segs = journal::list_segments(dir).expect("segments");
    segs.sort_by_key(|s| s.index);
    let pos = segs.iter().position(|s| s.index == idx)?;
    segs.get(pos + 1).map(|next| next.header.first_seq - 1)
}

fn seg_count(dir: &std::path::Path) -> usize {
    journal::list_segments(dir).expect("segments").len()
}

fn setup() -> (tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("dir");
    let cold = tempfile::tempdir().expect("cold");
    journal_with_segments(dir.path());
    assert!(
        seg_count(dir.path()) >= 4,
        "нужен многосегментный журнал, есть {}",
        seg_count(dir.path())
    );
    (dir, cold)
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Непокрытые сегменты НЕ прунятся и названы в отчёте
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn uncovered_segments_are_not_pruned_and_are_reported() {
    let (dir, cold) = setup();
    let before = seg_count(dir.path());

    // Чекпоинт покрывает только САМОЕ начало — устаревшие сегменты не покрыты.
    let pol = policy(cold.path(), Some(0), false);
    let plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).expect("plan");

    assert!(
        plan.offload_and_prune.is_empty(),
        "C-030 R1 НАРУШЕН: непокрытые чекпоинтом сегменты попали в offload_and_prune — \
         retention удалил бы локальные данные, которых read-путь ещё не свернул. Кокпит молча \
         получил бы усечённую историю (all-time VWAP), восстановить нечем. План: {:?}",
        plan.offload_and_prune
            .iter()
            .map(|s| s.index)
            .collect::<Vec<_>>()
    );
    assert!(
        !plan.offload_only.is_empty(),
        "устаревшие непокрытые сегменты обязаны попасть в offload_only — бэкап (R1) НЕ \
         блокируется строгостью prune"
    );
    assert!(
        plan.skipped
            .iter()
            .any(|(_, reason)| reason.contains("checkpoint")),
        "skip-репорт обязан НАЗЫВАТЬ причину «нет покрытия чекпоинтом» (оператор должен \
         понимать, почему место не освобождается). Причины: {:?}",
        plan.skipped.iter().map(|(_, r)| r).collect::<Vec<_>>()
    );

    let report =
        journal::retention_execute(dir.path(), &plan, &pol, RetentionMode::Apply).expect("execute");
    assert!(
        report.pruned.is_empty(),
        "Apply удалил непокрытые сегменты: {:?}",
        report.pruned
    );
    assert_eq!(
        seg_count(dir.path()),
        before,
        "ни один сегмент не должен исчезнуть с диска"
    );
    assert!(
        !report.offloaded.is_empty(),
        "холодная копия обязана быть сделана даже без покрытия (R1 не блокируется)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. ПАРНЫЙ vantage: покрытые сегменты ОБЯЗАНЫ пруниться (гвард не переширок)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn covered_segments_are_pruned() {
    let (dir, cold) = setup();
    let before = seg_count(dir.path());

    // Покрываем ВСЁ, что вообще может быть спрунено.
    let pol = policy(cold.path(), Some(N), false);
    let plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).expect("plan");
    assert!(
        !plan.offload_and_prune.is_empty(),
        "при полном покрытии чекпоинтом устаревшие сегменты обязаны пруниться — иначе гвард \
         выродился в «не пруним никогда» и диск не освобождается (заглушка «всегда skip»)"
    );

    let report =
        journal::retention_execute(dir.path(), &plan, &pol, RetentionMode::Apply).expect("execute");
    assert!(
        !report.pruned.is_empty(),
        "Apply обязан удалить покрытые сегменты"
    );
    assert!(
        seg_count(dir.path()) < before,
        "покрытые сегменты обязаны физически исчезнуть: было {before}, стало {}",
        seg_count(dir.path())
    );
    assert!(
        report.pruned_without_checkpoint_coverage.is_empty(),
        "покрытие было — отчёт не должен помечать prune как беспокрытийный"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. ГРАНИЦА (п.4): покрытие ровно на last_seq против last_seq − 1
// ─────────────────────────────────────────────────────────────────────────────

/// Классический off-by-one: `covered == last_seq` — сегмент свёрнут ЦЕЛИКОМ (prunable);
/// `covered == last_seq − 1` — последнее событие ещё не свёрнуто (НЕ prunable).
/// Реализация со строгим/нестрогим неравенством наперекос падает ровно здесь.
#[test]
fn coverage_boundary_is_exact() {
    let (dir, cold) = setup();
    let mut segs = journal::list_segments(dir.path()).expect("segments");
    segs.sort_by_key(|s| s.index);
    let first_idx = segs[0].index;
    let last_seq = last_seq_of(dir.path(), first_idx).expect("не активный сегмент");

    let plan_exact = journal::retention_plan(
        dir.path(),
        &policy(cold.path(), Some(last_seq), false),
        T0 + 100 * DAY_MS,
    )
    .expect("plan exact");
    assert!(
        plan_exact
            .offload_and_prune
            .iter()
            .any(|s| s.index == first_idx),
        "covered == last_seq({last_seq}) ⇒ сегмент {first_idx} свёрнут ЦЕЛИКОМ и обязан быть \
         prunable (граница включительная)"
    );

    let plan_short = journal::retention_plan(
        dir.path(),
        &policy(cold.path(), Some(last_seq - 1), false),
        T0 + 100 * DAY_MS,
    )
    .expect("plan short");
    assert!(
        !plan_short
            .offload_and_prune
            .iter()
            .any(|s| s.index == first_idx),
        "covered == last_seq−1 ⇒ последнее событие сегмента {first_idx} ещё НЕ свёрнуто, \
         удалять его нельзя (off-by-one съедает ровно одно событие — молча)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. ОТСУТСТВИЕ (п.3): нет артефакта покрытия ≠ «покрыто»
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn absent_coverage_blocks_prune_but_still_offloads() {
    let (dir, cold) = setup();
    let before = seg_count(dir.path());

    let pol = policy(cold.path(), None, false);
    let plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).expect("plan");
    assert!(
        plan.offload_and_prune.is_empty(),
        "отсутствие артефакта покрытия обязано трактоваться как «НЕ покрыто» (fail-closed), \
         а не как «покрыто всё»"
    );
    assert!(
        !plan.offload_only.is_empty(),
        "бэкап обязан идти и без чекпоинтера — иначе строгость prune блокирует R1 \
         (экзистенциальный риск: журнал в единственной копии)"
    );

    let report =
        journal::retention_execute(dir.path(), &plan, &pol, RetentionMode::Apply).expect("execute");
    assert!(
        report.pruned.is_empty(),
        "prune без покрытия: {:?}",
        report.pruned
    );
    assert_eq!(seg_count(dir.path()), before);
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Явный операторский override — разрешён, но ОБЯЗАН быть назван в отчёте
// ─────────────────────────────────────────────────────────────────────────────

/// Отклонение от буквы C-030 (см. шапку). Смысл: fail-closed по умолчанию, но у оператора
/// есть аудируемый выход, когда выбор стоит между «не освобождать место» и «остановить запись
/// новых данных disk-guard'ом». Молчаливого выхода нет: prune без покрытия ОБЯЗАН быть
/// поимённо перечислен в отчёте.
#[test]
fn override_prunes_but_is_named_in_report() {
    let (dir, cold) = setup();
    let pol = policy(cold.path(), None, true);
    let plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).expect("plan");
    assert!(
        !plan.offload_and_prune.is_empty(),
        "с явным allow_prune_without_checkpoint prune обязан планироваться"
    );

    let report =
        journal::retention_execute(dir.path(), &plan, &pol, RetentionMode::Apply).expect("execute");
    assert!(!report.pruned.is_empty(), "override обязан реально удалить");
    assert_eq!(
        report.pruned_without_checkpoint_coverage.len(),
        report.pruned.len(),
        "КАЖДЫЙ prune без покрытия обязан быть назван в отчёте — иначе операторский override \
         становится тихим: в логе «freed N bytes», а то, что read-путь потерял историю, \
         не видно нигде. pruned={:?} named={:?}",
        report.pruned,
        report.pruned_without_checkpoint_coverage
    );
}

/// DryRun остаётся безопасным при любых настройках покрытия (регресс-гвард к существующему
/// контракту `journal-retention --mode dry-run`, дефолту прод-cron).
#[test]
fn dry_run_never_deletes_regardless_of_coverage() {
    let (dir, cold) = setup();
    let before = seg_count(dir.path());
    for (covered, over) in [(None, true), (Some(N), false)] {
        let pol = policy(cold.path(), covered, over);
        let plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).expect("plan");
        let report = journal::retention_execute(dir.path(), &plan, &pol, RetentionMode::DryRun)
            .expect("execute dry-run");
        assert!(report.pruned.is_empty(), "DryRun удалил сегменты");
        assert_eq!(seg_count(dir.path()), before, "DryRun изменил диск");
    }
}
