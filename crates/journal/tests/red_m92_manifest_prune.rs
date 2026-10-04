//! RED `M-92` (sacred, architect-only) — **локальный сегмент журнала удаляется ТОЛЬКО после
//! пофайловой сверки с контрольной суммой, посчитанной НА СТОРОНЕ офсайт-копии** (строка 2ter
//! `docs/ROADMAP.md`, `TD-202`/`TD-020`, `R-180` F-1).
//!
//! # Почему прежний путь негоден — замер, а не мнение
//!
//! `verify_cold_copy(seg, cold_root)` при отсутствии файла в `cold_root` САМ копирует сегмент
//! туда и сверяет копию с источником — сверка тождественна. На проде `cold_root` — путь, которого
//! внутри контейнера уборщика нет (`R-180` F-1: «ЗАПИСЬ-НА-ЭФЕМЕРНЫЙ-СЛОЙ-УДАЛАСЬ»), а холодного
//! хранилища, примонтированного к серверу, нет вовсе: копия — Storage Box по SSH (аудит 2026-10-01).
//! Типовой барьер `ColdCopyProof` обходится конфигурацией.
//!
//! # Новая конструкция — та же, что исполнена вручную 08.09 и 01.10
//!
//! Отбор кандидатов — прежний `retention_plan` (один источник: активный, `keep_min`, возраст,
//! покрытие слепком, legacy). Доказательство — НОВОЕ: перечень `sha256sum`, посчитанный на коробке
//! (`ColdManifest`), и локальная сумма сегмента обязаны совпасть ПО ИМЕНИ. Копирования нет ни на
//! каком пути. Удаление 01.10: 298/298 совпали, 58 439 976 703 Б.
//!
//! # Форма, против которой компилируется набор (задана спекой `M-92` §4)
//!
//! ```text
//! pub struct ColdManifest;  impl ColdManifest { pub fn parse(text: &str) -> io::Result<Self>; }
//! pub fn retention_execute_with_manifest(dir, &RetentionPlan, &RetentionPolicy, &ColdManifest,
//!     RetentionMode) -> io::Result<RetentionReport>;
//! journal-retention: --plan-out <file>, --cold-manifest <file>; --mode apply без
//!     --cold-manifest или с --cold ⇒ exit ≠ 0, ничего не удалено.
//! deploy/bin/journal-retention-cron.sh: шов RETENTION_REMOTE_SHA_CMD (удалённые суммы),
//!     RETENTION_WORK_DIR (хост ↔ /work контейнера), RETENTION_AUDIT_DIR.
//! ```

use contracts::{DataSource, EventKind, MdPayload, Side, Venue};
use journal::{ColdManifest, Journal, RetentionMode, RetentionPolicy, WriterConfig};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const DAY_MS: i64 = 86_400_000;
const T0: i64 = 1_752_000_000_000;
const N: u64 = 900;
const BIN: &str = env!("CARGO_BIN_EXE_journal-retention");

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 8 * 1024,
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

fn journal() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("dir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..N {
        j.append(trade(i)).expect("append");
    }
    j.flush().expect("flush");
    let n = names(dir.path()).len();
    assert!(
        n >= 5,
        "SETUP НЕ СОСТОЯЛСЯ: нужен многосегментный журнал, есть {n}"
    );
    dir
}

/// Политика: возраст 1 сутки, `keep_min = 1`, покрытие слепком — всё, кроме активного.
/// `cold_root` указывает в КАТАЛОГ-СТОРОЖ: на манифестном пути туда не пишется ничего.
fn policy(cold_guard: &Path, covered: Option<u64>) -> RetentionPolicy {
    RetentionPolicy {
        retain_days: 1,
        keep_min_segments: 1,
        cold_root: cold_guard.to_path_buf(),
        min_free_bytes: 0,
        checkpoint_covered_through_seq: covered,
        allow_prune_without_checkpoint: false,
    }
}

fn names(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("segment-"))
        .collect()
}

fn sha_hex(p: &Path) -> String {
    let bytes = std::fs::read(p).expect("read");
    format!("{:x}", Sha256::digest(&bytes))
}

/// Форма вывода `sha256sum`: `<64 hex>  <имя>` (два пробела), имя — без каталога.
fn manifest_line(dir: &Path, name: &str) -> String {
    format!("{}  {}", sha_hex(&dir.join(name)), name)
}

fn max_covered(dir: &Path) -> u64 {
    let mut segs = journal::list_segments(dir).expect("segments");
    segs.sort_by_key(|s| s.index);
    segs.last().expect("есть сегменты").header.first_seq - 1
}

fn plan_names(plan: &journal::RetentionPlan) -> BTreeSet<String> {
    plan.offload_and_prune
        .iter()
        .map(|s| s.path.file_name().unwrap().to_string_lossy().to_string())
        .collect()
}

struct World {
    dir: tempfile::TempDir,
    guard: tempfile::TempDir,
    plan: journal::RetentionPlan,
    pol: RetentionPolicy,
}

fn world(covered: bool) -> World {
    let dir = journal();
    let guard = tempfile::tempdir().expect("guard");
    let cov = if covered {
        Some(max_covered(dir.path()))
    } else {
        None
    };
    let pol = policy(guard.path(), cov);
    let plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).expect("plan");
    if covered {
        assert!(
            plan.offload_and_prune.len() >= 3,
            "SETUP НЕ СОСТОЯЛСЯ: в плане {} кандидатов, нужно ≥ 3",
            plan.offload_and_prune.len()
        );
    }
    World {
        dir,
        guard,
        plan,
        pol,
    }
}

fn full_manifest(w: &World) -> String {
    plan_names(&w.plan)
        .iter()
        .map(|n| manifest_line(w.dir.path(), n))
        .collect::<Vec<_>>()
        .join("\n")
}

fn guard_untouched(w: &World) {
    let n = std::fs::read_dir(w.guard.path())
        .map(|r| r.count())
        .unwrap_or(0);
    assert_eq!(
        n, 0,
        "M-92: в каталог cold_root записано {n} файлов — манифестный путь НЕ копирует ничего никуда \
         (иначе возвращается «скопируй и подтверди», R-180 F-1)"
    );
}

/// Каталог — сплошной ряд индексов (`JR-I-2`): удаление обязано срезать ПРЕФИКС, а не дырявить
/// середину. Замер architect'а 2026-10-02: `journal::stream` на каталоге с дырой (удалены 2-й и 3-й
/// из шести, самый старый оставлен) молча отдал 562 события с одним разрывом нумерации, без ошибки —
/// дыра в истории выдавалась бы за непрерывную историю.
fn assert_contiguous(dir: &Path, ctx: &str) {
    // Позиции — по ИМЕНИ `segment-NNNNNNNN.jrnl[.zst]`, заголовок не читается (как в спеке §4):
    // испорченный файл занимает позицию, а `list_segments` на нём законно отказывает (`p10`).
    let mut idx: Vec<u32> = names(dir)
        .iter()
        .filter_map(|n| {
            n.strip_prefix("segment-")?
                .split('.')
                .next()?
                .parse::<u32>()
                .ok()
        })
        .collect();
    idx.sort();
    idx.dedup();
    assert!(
        idx.windows(2).all(|p| p[1] == p[0] + 1),
        "M-92 (JR-I-2): {ctx}: после удаления в каталоге дыра: {idx:?}"
    );
}

/// Манифест плана, где у ОДНОГО имени сумма испорчена (копия на коробке не совпала).
fn manifest_with_bad(dir: &Path, plan: &BTreeSet<String>, bad: &str) -> String {
    plan.iter()
        .map(|n| {
            if n == bad {
                format!("{}  {}", "0".repeat(64), n)
            } else {
                manifest_line(dir, n)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ───────────────────────── библиотека ─────────────────────────

/// **`p1` — полный совпадающий манифест: удалены РОВНО кандидаты плана, остальное цело,
/// каталог остаётся сплошным суффиксом, копий нигде нет.**
#[test]
fn p1_matching_manifest_prunes_exactly_the_plan() {
    let w = world(true);
    let before = names(w.dir.path());
    let want = plan_names(&w.plan);
    let m = ColdManifest::parse(&full_manifest(&w)).expect("parse");
    let r = journal::retention_execute_with_manifest(
        w.dir.path(),
        &w.plan,
        &w.pol,
        &m,
        RetentionMode::Apply,
    )
    .expect("execute");
    let after = names(w.dir.path());
    let gone: BTreeSet<String> = before.difference(&after).cloned().collect();
    assert_eq!(gone, want, "M-92: удалено не ровно то, что в плане");
    assert!(
        r.failed.is_empty(),
        "M-92: при полном манифесте failed = {:?}",
        r.failed
    );
    assert_eq!(r.pruned.len(), want.len());
    // R-217 Н-2: на манифестном пути ничего не выгружается — «выгружено N» было бы ложью
    assert!(
        r.offloaded.is_empty(),
        "M-92 (R-217 Н-2): offloaded = {:?} на пути без копирования",
        r.offloaded
    );
    // сплошной суффикс: оставшиеся индексы без дыр
    let mut idx: Vec<u32> = journal::list_segments(w.dir.path())
        .unwrap()
        .iter()
        .map(|s| s.index)
        .collect();
    idx.sort();
    assert!(
        idx.windows(2).all(|p| p[1] == p[0] + 1),
        "M-92: после удаления в каталоге дыра: {idx:?}"
    );
    guard_untouched(&w);
}

/// **`p2` — в манифесте НЕТ строки одного кандидата: он остаётся, назван в `failed`.**
#[test]
fn p2_missing_manifest_entry_keeps_segment() {
    let w = world(true);
    let want = plan_names(&w.plan);
    // САМЫЙ МОЛОДОЙ кандидат: удержание его не рвёт префикс старших (p8/p9 — иные позиции).
    let victim = want.iter().next_back().unwrap().clone();
    let text: String = want
        .iter()
        .filter(|n| **n != victim)
        .map(|n| manifest_line(w.dir.path(), n))
        .collect::<Vec<_>>()
        .join("\n");
    let m = ColdManifest::parse(&text).expect("parse");
    let r = journal::retention_execute_with_manifest(
        w.dir.path(),
        &w.plan,
        &w.pol,
        &m,
        RetentionMode::Apply,
    )
    .expect("execute");
    assert!(
        w.dir.path().join(&victim).exists(),
        "M-92: сегмент без строки в манифесте удалён"
    );
    assert!(
        r.failed.iter().any(|(p, _)| p.ends_with(&victim)),
        "M-92: сегмент без строки в манифесте не назван в failed: {:?}",
        r.failed
    );
    assert_eq!(
        r.pruned.len(),
        want.len() - 1,
        "M-92: прочие совпавшие обязаны удалиться"
    );
    assert_contiguous(w.dir.path(), "p2");
    guard_untouched(&w);
}

/// **`p3` — сумма в манифесте НЕ совпала (копия на коробке испорчена): сегмент остаётся.**
#[test]
fn p3_mismatching_sum_keeps_segment() {
    let w = world(true);
    let want = plan_names(&w.plan);
    let victim = want.iter().next_back().unwrap().clone();
    let text: String = want
        .iter()
        .map(|n| {
            if *n == victim {
                format!("{}  {}", "0".repeat(64), n)
            } else {
                manifest_line(w.dir.path(), n)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let m = ColdManifest::parse(&text).expect("parse");
    let r = journal::retention_execute_with_manifest(
        w.dir.path(),
        &w.plan,
        &w.pol,
        &m,
        RetentionMode::Apply,
    )
    .expect("execute");
    assert!(
        w.dir.path().join(&victim).exists(),
        "M-92: сегмент с несовпавшей суммой удалён"
    );
    assert!(
        r.failed.iter().any(|(p, _)| p.ends_with(&victim)),
        "M-92: несовпадение не названо в failed"
    );
    assert_contiguous(w.dir.path(), "p3");
}

/// **`p8` — несовпал САМЫЙ СТАРЫЙ кандидат: не удаляется НИЧЕГО** (удаление — префикс плана
/// в порядке индексов, первый неподтверждённый его обрывает). Иначе старейший остаётся, за ним
/// дыра, и чтение журнала молча сшивает историю через разрыв (`JR-I-2`). Каждый младший
/// назван в `failed` с причиной, называющей обрыв (`blocked-by`).
#[test]
fn p8_oldest_mismatch_prunes_nothing() {
    let w = world(true);
    let want = plan_names(&w.plan);
    let before = names(w.dir.path());
    let victim = want.iter().next().unwrap().clone();
    let m = ColdManifest::parse(&manifest_with_bad(w.dir.path(), &want, &victim)).expect("parse");
    let r = journal::retention_execute_with_manifest(
        w.dir.path(),
        &w.plan,
        &w.pol,
        &m,
        RetentionMode::Apply,
    )
    .expect("execute");
    assert_eq!(
        names(w.dir.path()),
        before,
        "M-92 (JR-I-2): несовпал старейший {victim}, а младшие удалены — в каталоге дыра"
    );
    assert!(r.pruned.is_empty(), "M-92: pruned = {:?}", r.pruned);
    for n in want.iter().filter(|n| **n != victim) {
        assert!(
            r.failed
                .iter()
                .any(|(p, why)| p.ends_with(n) && why.contains("blocked-by")),
            "M-92: младший {n} не назван в failed с причиной blocked-by: {:?}",
            r.failed
        );
    }
    guard_untouched(&w);
}

/// **`p9` — несовпал СРЕДНИЙ кандидат: удалён РОВНО префикс старше него**, он и младшие целы.
#[test]
fn p9_middle_mismatch_prunes_exactly_the_older_prefix() {
    let w = world(true);
    let want: Vec<String> = plan_names(&w.plan).into_iter().collect();
    let victim = want[want.len() / 2].clone();
    let set: BTreeSet<String> = want.iter().cloned().collect();
    let before = names(w.dir.path());
    let m = ColdManifest::parse(&manifest_with_bad(w.dir.path(), &set, &victim)).expect("parse");
    let r = journal::retention_execute_with_manifest(
        w.dir.path(),
        &w.plan,
        &w.pol,
        &m,
        RetentionMode::Apply,
    )
    .expect("execute");
    let gone: BTreeSet<String> = before.difference(&names(w.dir.path())).cloned().collect();
    let older: BTreeSet<String> = want.iter().filter(|n| **n < victim).cloned().collect();
    assert!(
        !older.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: у среднего кандидата нет старших"
    );
    assert_eq!(
        gone, older,
        "M-92 (JR-I-2): удалён не ровно префикс старше {victim}"
    );
    assert_eq!(r.pruned.len(), older.len());
    assert_contiguous(w.dir.path(), "p9");
    guard_untouched(&w);
}

/// **`p10` — граница удаления — КАТАЛОГ, а не план (`R-217` Б-1).** Сегмент с испорченным
/// заголовком `retention_plan` не классифицирует и в план не берёт, а его старших И младших
/// соседей — берёт. Обрыв «по плану» удалил бы младших и оставил дыру (проба `R-217`: каталог
/// после — `[2,4,5]`, `failed = 0`). Позиции каталога — индексы ИМЁН сегментов, заголовок не
/// читается: испорченный файл занимает позицию и обрывает удаление.
#[test]
fn p10_catalog_position_outside_plan_blocks_younger() {
    let w = world(true);
    let all: Vec<String> = names(w.dir.path()).into_iter().collect();
    let bad = all[2].clone();
    std::fs::write(w.dir.path().join(&bad), b"XXXX").unwrap();
    let plan = journal::retention_plan(w.dir.path(), &w.pol, T0 + 100 * DAY_MS).expect("plan");
    let want = plan_names(&plan);
    assert!(
        !want.contains(&bad)
            && want.iter().any(|n| *n < bad)
            && want.iter().any(|n| *n > bad),
        "SETUP НЕ СОСТОЯЛСЯ: испорченный {bad} обязан выпасть из плана, а в плане — соседи с обеих сторон: {want:?}"
    );
    let before = names(w.dir.path());
    let m = ColdManifest::parse(
        &want
            .iter()
            .map(|n| manifest_line(w.dir.path(), n))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .expect("parse");
    let r = journal::retention_execute_with_manifest(
        w.dir.path(),
        &plan,
        &w.pol,
        &m,
        RetentionMode::Apply,
    )
    .expect("execute");
    let gone: BTreeSet<String> = before.difference(&names(w.dir.path())).cloned().collect();
    let older: BTreeSet<String> = want.iter().filter(|n| **n < bad).cloned().collect();
    assert_eq!(
        gone, older,
        "M-92 (R-217 Б-1, JR-I-2): удалено не ровно то, что старше позиции {bad} вне плана"
    );
    assert_contiguous(w.dir.path(), "p10");
    for n in want.iter().filter(|n| **n > bad) {
        assert!(
            r.failed
                .iter()
                .any(|(p, why)| p.ends_with(n) && why.contains("blocked-by")),
            "M-92 (R-217 Б-1): младший {n} за позицией вне плана не назван в failed с blocked-by: {:?}",
            r.failed
        );
    }
    guard_untouched(&w);
}

/// **`p12` — ИЗВЕСТНАЯ плану не-кандидатная позиция обрывает удаление (`R-221` Б-2, буква `I-2ter`).**
/// Возрастной фильтр `retention_plan` не монотонен по индексу: решение берётся по биржевому
/// времени ПЕРВОГО события сегмента. Если оно у среднего сегмента «свежее» соседей, план кладёт его
/// в `skipped`, а старших И младших — в кандидаты. Пропуск такой позиции без обрыва удалил бы младших
/// и оставил дыру (проба `R-221`: каталог после — `[2,4,5]`, `failed = 0`).
#[test]
fn p12_known_non_candidate_position_blocks_younger() {
    // проход 1: те же события — узнать seq первого события сегмента 2
    let probe = journal();
    let mut segs = journal::list_segments(probe.path()).unwrap();
    segs.sort_by_key(|s| s.index);
    assert!(
        segs.len() >= 5,
        "SETUP НЕ СОСТОЯЛСЯ: сегментов {}",
        segs.len()
    );
    let fresh_at = segs[2].header.first_seq - segs[0].header.first_seq;
    // проход 2: тот же журнал, но первое событие сегмента 2 — «секунду назад» (ширина varint та же)
    let dir = tempfile::tempdir().expect("dir");
    let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
    for i in 0..N {
        let ev = if i == fresh_at {
            EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: contracts::to_fixed(65_000.0) + i as i64,
                    size: contracts::to_fixed(0.01),
                    side: Side::Buy,
                    ts_exch_ms: T0 + 100 * DAY_MS - 1_000,
                },
            )
        } else {
            trade(i)
        };
        j.append(ev).expect("append");
    }
    j.flush().expect("flush");
    drop(j);
    let all: Vec<String> = names(dir.path()).into_iter().collect();
    assert_eq!(
        all,
        names(probe.path()).into_iter().collect::<Vec<_>>(),
        "SETUP НЕ СОСТОЯЛСЯ: разбиение на сегменты изменилось"
    );
    let fresh = all[2].clone();
    let guard = tempfile::tempdir().expect("guard");
    let pol = policy(guard.path(), Some(max_covered(dir.path())));
    let plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).expect("plan");
    let want = plan_names(&plan);
    assert!(
        !want.contains(&fresh)
            && plan.skipped.iter().any(|(s, _)| s.path.ends_with(&fresh))
            && want.iter().any(|n| *n < fresh)
            && want.iter().any(|n| *n > fresh),
        "SETUP НЕ СОСТОЯЛСЯ: {fresh} обязан быть в skipped плана, а кандидаты — по обе стороны: {want:?}"
    );
    let before = names(dir.path());
    let m = ColdManifest::parse(
        &want
            .iter()
            .map(|n| manifest_line(dir.path(), n))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .expect("parse");
    let r =
        journal::retention_execute_with_manifest(dir.path(), &plan, &pol, &m, RetentionMode::Apply)
            .expect("execute");
    let gone: BTreeSet<String> = before.difference(&names(dir.path())).cloned().collect();
    let older: BTreeSet<String> = want.iter().filter(|n| **n < fresh).cloned().collect();
    assert_eq!(
        gone, older,
        "M-92 (R-221 Б-2, I-2ter, JR-I-2): удалено не ровно то, что старше известной не-кандидатной позиции {fresh}"
    );
    assert_contiguous(dir.path(), "p12");
    for n in want.iter().filter(|n| **n > fresh) {
        assert!(
            r.failed.iter().any(|(p, why)| p.ends_with(n) && why.contains("blocked-by")),
            "M-92 (R-221 Б-2): младший {n} за позицией {fresh} не назван в failed с blocked-by: {:?}",
            r.failed
        );
    }
}

/// **`p11` — `DryRun` предсказывает `Apply` (`R-217` Н-1).** Тот же манифест (копия СТАРЕЙШЕГО
/// испорчена): множество имён в `failed` у пробного прогона РАВНО множеству у настоящего, на
/// двух одинаково построенных журналах. Пробный прогон хеширует (чтение — не побочный эффект,
/// `I-4`) и не удаляет ничего. Иначе задача 4(а) — «прогон прод-пути в dry-run» — показала бы
/// «всё сходится» там, где `apply` не удалит ничего.
#[test]
fn p11_dry_run_failed_set_equals_apply() {
    let failed_names = |mode: RetentionMode| {
        let w = world(true);
        let want = plan_names(&w.plan);
        let victim = want.iter().next().unwrap().clone();
        let before = names(w.dir.path());
        let m =
            ColdManifest::parse(&manifest_with_bad(w.dir.path(), &want, &victim)).expect("parse");
        let r = journal::retention_execute_with_manifest(w.dir.path(), &w.plan, &w.pol, &m, mode)
            .expect("execute");
        if mode == RetentionMode::DryRun {
            assert_eq!(names(w.dir.path()), before, "M-92: DryRun удалил файлы");
        }
        let set: BTreeSet<String> = r
            .failed
            .iter()
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        (set, want)
    };
    let (dry, want) = failed_names(RetentionMode::DryRun);
    let (app, _) = failed_names(RetentionMode::Apply);
    assert_eq!(
        app, want,
        "SETUP НЕ СОСТОЯЛСЯ: apply при испорченном старейшем обязан удержать весь план"
    );
    assert_eq!(
        dry, app,
        "M-92 (R-217 Н-1): failed пробного прогона расходится с настоящим"
    );
}

/// **`p4` — манифест несёт ВЕРНЫЕ суммы активного и `keep_min`-сегментов: они не удаляются.**
/// Отбор — только `retention_plan`; манифест не расширяет право удалять.
#[test]
fn p4_manifest_cannot_widen_the_plan() {
    let w = world(true);
    let all = names(w.dir.path());
    let text = all
        .iter()
        .map(|n| manifest_line(w.dir.path(), n))
        .collect::<Vec<_>>()
        .join("\n");
    let m = ColdManifest::parse(&text).expect("parse");
    journal::retention_execute_with_manifest(
        w.dir.path(),
        &w.plan,
        &w.pol,
        &m,
        RetentionMode::Apply,
    )
    .expect("execute");
    let after = names(w.dir.path());
    let kept: BTreeSet<String> = all.difference(&plan_names(&w.plan)).cloned().collect();
    assert_eq!(
        after, kept,
        "M-92: манифест с суммами активного/keep_min-сегментов расширил удаление сверх плана"
    );
}

/// **`p5` — нет покрытия слепком: не удаляется НИЧЕГО даже при полном манифесте.**
#[test]
fn p5_uncovered_prunes_nothing() {
    let w = world(false);
    let before = names(w.dir.path());
    let text = before
        .iter()
        .map(|n| manifest_line(w.dir.path(), n))
        .collect::<Vec<_>>()
        .join("\n");
    let m = ColdManifest::parse(&text).expect("parse");
    let r = journal::retention_execute_with_manifest(
        w.dir.path(),
        &w.plan,
        &w.pol,
        &m,
        RetentionMode::Apply,
    )
    .expect("execute");
    assert_eq!(
        names(w.dir.path()),
        before,
        "M-92: без покрытия слепком удалены сегменты"
    );
    assert!(r.pruned.is_empty());
    guard_untouched(&w);
}

/// **`p6` — `DryRun` с полным манифестом: ноль побочных эффектов.**
#[test]
fn p6_dry_run_touches_nothing() {
    let w = world(true);
    let before = names(w.dir.path());
    let m = ColdManifest::parse(&full_manifest(&w)).expect("parse");
    let r = journal::retention_execute_with_manifest(
        w.dir.path(),
        &w.plan,
        &w.pol,
        &m,
        RetentionMode::DryRun,
    )
    .expect("execute");
    assert_eq!(names(w.dir.path()), before, "M-92: DryRun удалил сегменты");
    assert!(r.pruned.is_empty());
    guard_untouched(&w);
}

/// **`p7` — разбор манифеста fail-closed:** дубль имени с разными суммами, не-hex, не 64 знака,
/// имя с каталогом, строка не в форме `sha256sum` — `Err`. Пустой текст — пустой манифест.
#[test]
fn p7_manifest_parse_is_strict() {
    let h = "a".repeat(64);
    let g = "b".repeat(64);
    for (why, text) in [
        (
            "дубль имени с разными суммами",
            format!("{h}  segment-00000001.jrnl\n{g}  segment-00000001.jrnl"),
        ),
        (
            "не hex",
            format!("{}  segment-00000001.jrnl", "z".repeat(64)),
        ),
        ("не 64 знака", "abc  segment-00000001.jrnl".to_string()),
        (
            "имя с каталогом",
            format!("{h}  journal/segment-00000001.jrnl"),
        ),
        (
            "один пробел вместо двух",
            format!("{h} segment-00000001.jrnl"),
        ),
    ] {
        assert!(
            ColdManifest::parse(&text).is_err(),
            "M-92: разбор манифеста принял «{why}»: {text:?}"
        );
    }
    assert!(
        ColdManifest::parse("").is_ok(),
        "пустой манифест — законный (кандидатов нет)"
    );
    assert!(
        ColdManifest::parse(&format!(
            "{h}  segment-00000001.jrnl\n{h}  segment-00000001.jrnl"
        ))
        .is_ok(),
        "точный дубль строки допустим (sha256sum повторил аргумент)"
    );
}

// ───────────────────────── программа уборщика ─────────────────────────

fn run_bin(args: &[String]) -> (Option<i32>, String) {
    let o = Command::new(BIN)
        .args(args)
        .output()
        .expect("journal-retention");
    (
        o.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

fn base_args(dir: &Path, covered: u64, cov_file: &Path) -> Vec<String> {
    std::fs::write(cov_file, covered.to_string()).unwrap();
    vec![
        format!("--dir={}", dir.display()),
        "--retain-days=1".into(),
        "--keep-min=1".into(),
        "--min-free-gb=0".into(),
        format!("--now-wall-ms={}", T0 + 100 * DAY_MS),
        format!("--checkpoint-coverage={}", cov_file.display()),
    ]
}

/// **`b1` — `--mode apply` без `--cold-manifest` и с прежним `--cold`: отказ, ничего не удалено.**
#[test]
fn b1_apply_requires_manifest_and_refuses_copy_path() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let cov = work.path().join("covered");
    let before = names(dir.path());
    let mut a = base_args(dir.path(), max_covered(dir.path()), &cov);
    a.push("--mode=apply".into());
    let (code, out) = run_bin(&a);
    assert_ne!(
        code,
        Some(0),
        "M-92: apply без --cold-manifest вышел 0: {out}"
    );
    let mut b = a.clone();
    b.push(format!("--cold={}", work.path().join("cold").display()));
    let (code2, out2) = run_bin(&b);
    assert_ne!(
        code2,
        Some(0),
        "M-92: apply с прежним --cold (скопируй-и-подтверди) вышел 0: {out2}"
    );
    assert_eq!(
        names(dir.path()),
        before,
        "M-92: при отказе удалены сегменты"
    );
    assert!(
        !work.path().join("cold").exists(),
        "M-92: прежний путь создал каталог копий"
    );
}

/// **`b2` — `--plan-out` пишет имена кандидатов РОВНО по плану; `--cold-manifest` + apply удаляет
/// совпавшие; вывод/exit называют несовпавший (`exit 2`).**
#[test]
fn b2_plan_out_then_manifest_apply() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let cov = work.path().join("covered");
    let covered = max_covered(dir.path());
    let plan_file = work.path().join("plan.txt");
    let mut a = base_args(dir.path(), covered, &cov);
    a.push("--mode=dry-run".into());
    a.push(format!("--plan-out={}", plan_file.display()));
    let (code, out) = run_bin(&a);
    assert_eq!(code, Some(0), "M-92: dry-run с --plan-out: {out}");
    let planned: Vec<String> = std::fs::read_to_string(&plan_file)
        .expect("M-92: --plan-out не записал файл")
        .lines()
        .map(str::to_string)
        .collect();
    let pol = policy(work.path(), Some(covered));
    let lib_plan = journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).unwrap();
    assert_eq!(
        planned.iter().cloned().collect::<BTreeSet<_>>(),
        plan_names(&lib_plan),
        "M-92: --plan-out расходится с retention_plan"
    );
    // манифест: все кроме САМОГО МОЛОДОГО совпадают (удержание младшего не рвёт префикс)
    let victim = planned.iter().max().unwrap().clone();
    let text: String = planned
        .iter()
        .map(|n| {
            if *n == victim {
                format!("{}  {}", "0".repeat(64), n)
            } else {
                manifest_line(dir.path(), n)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let man = work.path().join("manifest.txt");
    std::fs::write(&man, text).unwrap();
    let mut b = base_args(dir.path(), covered, &cov);
    b.push("--mode=apply".into());
    b.push(format!("--cold-manifest={}", man.display()));
    let (code, out) = run_bin(&b);
    assert_eq!(
        code,
        Some(2),
        "M-92: несовпавший кандидат обязан дать exit 2: {out}"
    );
    assert!(
        out.contains(&victim),
        "M-92: вывод не называет несовпавший сегмент: {out}"
    );
    assert!(
        dir.path().join(&victim).exists(),
        "M-92: несовпавший удалён"
    );
    for n in planned.iter().filter(|n| **n != victim) {
        assert!(
            !dir.path().join(n).exists(),
            "M-92: совпавший {n} не удалён"
        );
    }
    assert_contiguous(dir.path(), "b2");
}

// ───────────────────────── скрипт расписания ─────────────────────────

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Тома сервиса `journal-retention` из НАСТОЯЩЕГО `docker-compose.yml`: `(источник, цель, только_чтение)`.
/// Разбор нарочно примитивный (блок `volumes:` сервиса — плоский список строк), как в соседних оракулах.
fn compose_volumes(service: &str) -> Vec<(String, String, bool)> {
    let text =
        std::fs::read_to_string(repo().join("docker-compose.yml")).expect("docker-compose.yml");
    let (mut in_svc, mut in_vol) = (false, false);
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with("  ") && !line.starts_with("    ") && line.trim_end().ends_with(':') {
            in_svc = line.trim().trim_end_matches(':') == service;
            in_vol = false;
            continue;
        }
        if !in_svc {
            continue;
        }
        let t = line.trim();
        if line.starts_with("    ") && !line.starts_with("      ") && t.ends_with(':') {
            in_vol = t == "volumes:";
            continue;
        }
        if in_vol {
            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim().trim_matches('"');
                // источник может нести ${VAR:-default} с двоеточием внутри — режем справа
                let (rest, ro) = match item.strip_suffix(":ro") {
                    Some(r) => (r, true),
                    None => (item.strip_suffix(":rw").unwrap_or(item), false),
                };
                let idx = rest.rfind(':').expect("том без цели");
                out.push((rest[..idx].to_string(), rest[idx + 1..].to_string(), ro));
            } else if !t.is_empty() && !t.starts_with('#') {
                in_vol = false;
            }
        }
    }
    out
}

fn compose_entrypoint(service: &str) -> String {
    let text = std::fs::read_to_string(repo().join("docker-compose.yml")).unwrap();
    let mut in_svc = false;
    for line in text.lines() {
        if line.starts_with("  ") && !line.starts_with("    ") && line.trim_end().ends_with(':') {
            in_svc = line.trim().trim_end_matches(':') == service;
            continue;
        }
        if in_svc {
            if let Some(v) = line.trim().strip_prefix("entrypoint:") {
                return v.trim().to_string();
            }
        }
    }
    String::new()
}

/// Прогнать НАСТОЯЩИЙ `deploy/bin/journal-retention-cron.sh` в окружении прод-cron'а. Окружение —
/// `cron_text` (по умолчанию НАСТОЯЩИЙ `deploy/cron.d/journal-retention`), и режим из него тест НЕ
/// перекрывает (`C-268` R2). Заменены два шва: `RETENTION_RUNNER` — заглушка `docker compose run`,
/// которая подключает каталоги ПО ТОМАМ сервиса из `docker-compose.yml` (`journal-data` → журнал
/// фикстуры, `gateway-ckpt` → слепки, источник с `RETENTION_WORK_DIR` → рабочий каталог хоста) и
/// исполняет настоящий бинарь; цель, которой нет в томах сервиса, НЕ подключается — бинарь её не
/// найдёт (`C-268` R1). `RETENTION_REMOTE_SHA_CMD` — «коробка», считающая суммы по ОТДЕЛЬНОЙ копии.
fn run_wrapper(
    dir: &Path,
    remote_copy: &Path,
    work: &Path,
    audit: &Path,
    switch: Option<&str>,
    cron_text: Option<String>,
) -> (Option<i32>, String) {
    let ckpt = work.join("ckpt");
    std::fs::create_dir_all(&ckpt).unwrap();
    std::fs::write(
        ckpt.join("covered_through_seq"),
        max_covered(dir).to_string(),
    )
    .unwrap();
    let hostwork = work.join("hostwork");
    std::fs::create_dir_all(&hostwork).unwrap();
    match switch {
        Some(m) => std::fs::write(work.join("retention.mode"), m).unwrap(),
        None => {
            let _ = std::fs::remove_file(work.join("retention.mode"));
        }
    }
    // подмены путей контейнера → фикстура, выведенные из томов сервиса
    let mut subst = String::new();
    // R-221 Б-1: ИЗОЛЯЦИЯ ТОМОВ. Внутри контейнера видны только цели томов сервиса; путь хоста
    // (`/var/lib/hft/...`), переданный в argv, в настоящем контейнере не существует. Заглушка
    // исполняет бинарь на хосте, поэтому обязана ОТКАЗАТЬ на любом абсолютном пути вне целей томов
    // — иначе она находит путь хоста там, где прод получит ENOENT (замер R-221: exit=1 на шаге 1).
    let targets: Vec<String> = compose_volumes("journal-retention")
        .into_iter()
        .map(|(_, t, _)| t)
        .collect();
    let guard = format!(
        "for a in \"$@\"; do v=\"$a\"; case \"$v\" in --*=*) v=\"${{v#*=}}\";; esac; \
         case \"$v\" in /*) ok=0; for t in {t}; do case \"$v\" in \"$t\"|\"$t\"/*) ok=1;; esac; done; \
         [ $ok = 1 ] || {{ echo \"run_wrapper: путь вне томов сервиса (в контейнере его нет): $a\" >&2; exit 97; }};; esac; done\n",
        t = targets.join(" ")
    );
    for (src, target, _ro) in compose_volumes("journal-retention") {
        let host = if src == "journal-data" {
            dir.to_path_buf()
        } else if src == "gateway-ckpt" {
            ckpt.clone()
        } else if src.contains("RETENTION_WORK_DIR") {
            hostwork.clone()
        } else {
            continue;
        };
        subst.push_str(&format!(
            "a=\"${{a//{t}/{h}}}\"; ",
            t = target.replace('/', "\\/"),
            h = host.display()
        ));
    }
    let shim_dir = work.join("shim");
    std::fs::create_dir_all(&shim_dir).unwrap();
    let runner = shim_dir.join("runner");
    std::fs::write(
        &runner,
        format!(
            "#!/bin/bash\n# заглушка docker compose run: до имени сервиса — опции compose\n\
             while [ $# -gt 0 ] && [ \"$1\" != journal-retention ]; do shift; done; shift\n\
             {guard}\
             args=(); for a in \"$@\"; do {subst}args+=(\"$a\"); done\n\
             # A-044 O-3(a): при --mode apply ДО exec снимок каталога аудита и потребляемого манифеста\n\
             apply=0; man=; prev=; for a in \"${{args[@]}}\"; do case \"$a\" in --mode=apply) apply=1;; --cold-manifest=*) man=\"${{a#--cold-manifest=}}\";; esac; [ \"$prev\" = --mode ] && [ \"$a\" = apply ] && apply=1; [ \"$prev\" = --cold-manifest ] && man=\"$a\"; prev=\"$a\"; done\n\
             if [ $apply = 1 ]; then rm -rf {snap}; mkdir -p {snap}; cp -r {audit}/. {snap}/ 2>/dev/null; [ -n \"$man\" ] && cp \"$man\" {consumed}; fi\n\
             exec {bin} \"${{args[@]}}\" --now-wall-ms={now}\n",
            bin = BIN,
            guard = guard,
            now = T0 + 100 * DAY_MS,
            snap = work.join("audit-at-apply").display(),
            consumed = work.join("manifest-consumed").display(),
            audit = audit.display()
        ),
    )
    .unwrap();
    let remote = shim_dir.join("remote-sha");
    std::fs::write(
        &remote,
        format!(
            "#!/bin/bash\n# «коробка»: аргументы вида journal/<имя>, суммы по КОПИИ — ОДНИМ вызовом, как настоящий\n\
             # `sha256sum a b c`: отсутствующий файл — строка в stderr, остальные напечатаны, выход 1 (R-221 Н-1)\n\
             cd {r} || exit 255; n=(); for a in \"$@\"; do n+=(\"${{a#journal/}}\"); done\n\
             sha256sum \"${{n[@]}}\" | sed 's#  #  journal/#'; exit ${{PIPESTATUS[0]}}\n",
            r = remote_copy.display()
        ),
    )
    .unwrap();
    for f in [&runner, &remote] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let cron = cron_text.unwrap_or_else(|| {
        std::fs::read_to_string(repo().join("deploy/cron.d/journal-retention")).unwrap()
    });
    let mut cmd = Command::new("bash");
    cmd.arg(repo().join("deploy/bin/journal-retention-cron.sh"))
        .env_clear();
    for (k, v) in cron
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
    {
        cmd.env(k, v);
    }
    cmd.env("HFT_ROOT", repo())
        .env(
            "RETENTION_RUNNER",
            format!("{} compose run --rm journal-retention", runner.display()),
        )
        .env("RETENTION_REMOTE_SHA_CMD", remote.display().to_string())
        .env("RETENTION_WORK_DIR", &hostwork)
        .env("RETENTION_AUDIT_DIR", audit)
        .env("RETENTION_MODE_FILE", work.join("retention.mode"))
        .env("RETENTION_RETAIN_DAYS", "1")
        .env("RETENTION_KEEP_MIN", "1")
        .env("RETENTION_MIN_FREE_GB", "0")
        .env("RETENTION_LOG", work.join("retention.log"))
        .env("RETENTION_ALERT_FILE", work.join("retention.alert"))
        .env("RETENTION_LAST_SUCCESS", work.join("retention.last"));
    let o = cmd.output().expect("journal-retention-cron.sh");
    let log = std::fs::read_to_string(work.join("retention.log")).unwrap_or_default();
    (
        o.status.code(),
        format!(
            "{}{}{log}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

/// Все файлы аудит-следа (рекурсивно): имя → содержимое.
fn audit_files(audit: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![audit.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((
                    p.file_name().unwrap().to_string_lossy().to_string(),
                    std::fs::read_to_string(&p).unwrap_or_default(),
                ));
            }
        }
    }
    out
}

fn remote_copy_of(dir: &Path, corrupt: Option<&str>) -> tempfile::TempDir {
    let r = tempfile::tempdir().unwrap();
    for n in names(dir) {
        std::fs::copy(dir.join(&n), r.path().join(&n)).unwrap();
    }
    if let Some(n) = corrupt {
        let p = r.path().join(n);
        let mut b = std::fs::read(&p).unwrap();
        let last = b.len() - 1;
        b[last] ^= 0xFF;
        std::fs::write(&p, b).unwrap();
        assert_ne!(
            sha_hex(&p),
            sha_hex(&dir.join(n)),
            "SETUP НЕ СОСТОЯЛСЯ: порча копии не изменила сумму"
        );
    }
    r
}

/// **`c0` — топология сервиса `journal-retention` в `docker-compose.yml` (`C-268` R1):** журнал
/// подключён С ПРАВОМ ЗАПИСИ (удаление), слепки — только на чтение, рабочий каталог хоста — `/work`,
/// холодного каталога нет; точка входа — бинарь `journal-retention`; скрипт cron'а по умолчанию зовёт
/// `docker compose run --rm journal-retention` (текстовая сверка дефолта шва — названный предел).
#[test]
fn c0_compose_topology_is_the_prune_topology() {
    let vols = compose_volumes("journal-retention");
    let find = |target: &str| vols.iter().find(|(_, t, _)| t == target).cloned();
    let j = find("/journal").expect("M-92: у journal-retention нет тома /journal");
    assert_eq!(
        j.0, "journal-data",
        "M-92: /journal — не том journal-data: {j:?}"
    );
    assert!(!j.2, "M-92: журнал подключён :ro — удаление невозможно (EROFS), apply станет тихим no-op: {vols:?}");
    let c = find("/ckpt").expect("M-92: у journal-retention нет тома /ckpt — артефакта покрытия нет, удаление fail-closed навсегда");
    assert_eq!(
        c.0, "gateway-ckpt",
        "M-92: /ckpt — не том gateway-ckpt: {c:?}"
    );
    assert!(
        c.2,
        "M-92: слепки подключены уборщику с правом записи — обязаны быть :ro: {c:?}"
    );
    let w = find("/work")
        .expect("M-92: у journal-retention нет тома /work (рабочий каталог плана и манифеста)");
    assert!(
        w.0.contains("RETENTION_WORK_DIR"),
        "M-92: источник /work не RETENTION_WORK_DIR: {w:?}"
    );
    assert!(
        find("/cold").is_none(),
        "M-92: том /cold остался — путь «скопируй и подтверди» (R-180 F-1): {vols:?}"
    );
    assert!(
        compose_entrypoint("journal-retention").contains("/usr/local/bin/journal-retention"),
        "M-92: точка входа сервиса — не бинарь journal-retention"
    );
    let script =
        std::fs::read_to_string(repo().join("deploy/bin/journal-retention-cron.sh")).unwrap();
    assert!(
        script.contains("docker compose run --rm journal-retention"),
        "M-92: дефолт RETENTION_RUNNER скрипта — не `docker compose run --rm journal-retention`"
    );
}

/// **`c1` — прод-путь cron'а с переключателем `apply`: удалены сверенные с КОПИЕЙ, испорченная
/// копия удержала свой сегмент; аудит-след — ТРИ записи, связанные с именами (`C-268` R3):** план
/// (все кандидаты), манифест коробки (строка несовпавшего — с ЕГО испорченной суммой), отчёт
/// (несовпавший назван, каждый удалённый назван).
#[test]
fn c1_cron_apply_verifies_against_remote_copy() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let audit = tempfile::tempdir().unwrap();
    let pol = policy(work.path(), Some(max_covered(dir.path())));
    let plan = plan_names(&journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).unwrap());
    let victim = plan.iter().next_back().unwrap().clone();
    let remote = remote_copy_of(dir.path(), Some(&victim));
    let (code, out) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        audit.path(),
        Some("apply"),
        None,
    );
    assert!(
        dir.path().join(&victim).exists(),
        "M-92: сегмент с испорченной КОПИЕЙ удалён — суммы не с удалённой стороны: {out}"
    );
    for n in plan.iter().filter(|n| **n != victim) {
        assert!(
            !dir.path().join(n).exists(),
            "M-92: сверенный {n} не удалён: {out}"
        );
    }
    assert_ne!(
        code,
        Some(0),
        "M-92: несовпадение копии обязано дать ненулевой выход/тревогу: {out}"
    );
    let files = audit_files(audit.path());
    let by = |suffix: &str| {
        files
            .iter()
            .find(|(n, _)| n.ends_with(suffix))
            .map(|(_, c)| c.clone())
            .unwrap_or_else(|| {
                panic!(
                    "M-92: в аудит-следе нет записи *{suffix}: {:?}",
                    files.iter().map(|(n, _)| n).collect::<Vec<_>>()
                )
            })
    };
    let (plan_txt, manifest_txt, report_txt) =
        (by("plan.txt"), by("manifest.txt"), by("report.txt"));
    for n in &plan {
        assert!(
            plan_txt.lines().any(|l| l.trim() == n),
            "M-92: запись плана не называет {n}"
        );
    }
    // C-270: КАЖДОЕ имя плана — в манифесте с суммой, посчитанной на УДАЛЁННОЙ стороне (копия
    // фикстуры): у несовпавшего — его испорченная сумма, у удалённых — сумма их копии. Удаление,
    // чья удалённая сумма не записана, восстановить по аудит-следу нельзя.
    for n in &plan {
        let remote_sum = sha_hex(&remote.path().join(n));
        assert!(
            manifest_txt
                .lines()
                .any(|l| l.starts_with(&remote_sum) && l.trim_end().ends_with(n.as_str())),
            "M-92 (C-270): манифест в аудит-следе не несёт УДАЛЁННУЮ сумму {n} — сверку удаления нельзя восстановить"
        );
    }
    // A-044 O-1: отчёт КЛАССИФИЦИРУЕТ — `pruned <имя>` / `kept <имя> <причина>` (M-92 §4 п.4)
    let rows: Vec<Vec<String>> = report_txt
        .lines()
        .map(|l| l.split_whitespace().map(str::to_string).collect())
        .collect();
    let row = |tok: &str, n: &str| {
        rows.iter()
            .find(|r| r.len() >= 2 && r[0] == tok && r[1] == n)
    };
    // C-274-1: ОДНА строка на имя плана — ни дубля статуса, ни двух противоречащих строк
    // (`pruned X` рядом с `kept X …`), ни строк сверх плана.
    let body_rows: Vec<&Vec<String>> = rows.iter().filter(|r| !r.is_empty()).collect();
    assert_eq!(
        body_rows.len(),
        plan.len(),
        "M-92 (A-044 O-1, C-274-1): строк отчёта {} при {} именах плана: {report_txt}",
        body_rows.len(),
        plan.len()
    );
    for n in &plan {
        let k = body_rows
            .iter()
            .filter(|r| r.len() >= 2 && r[1] == *n)
            .count();
        assert_eq!(
            k, 1,
            "M-92 (A-044 O-1, C-274-1): имени {n} в отчёте {k} строк, ожидается ровно одна: {report_txt}"
        );
    }
    let kept = row("kept", &victim).unwrap_or_else(|| {
        panic!("M-92 (A-044 O-1): несовпавший {victim} не на строке `kept`: {report_txt}")
    });
    assert!(
        kept.len() >= 3,
        "M-92 (A-044 O-1): у `kept {victim}` нет причины: {report_txt}"
    );
    assert!(
        row("pruned", &victim).is_none(),
        "M-92 (A-044 O-1): несовпавший {victim} на строке `pruned`"
    );
    for n in plan.iter().filter(|n| **n != victim) {
        assert!(
            row("pruned", n).is_some(),
            "M-92 (A-044 O-1): удалённый {n} не на строке `pruned`: {report_txt}"
        );
    }
    // A-044 O-3(a): к моменту `apply` план и манифест УЖЕ в аудит-следе, манифест — тот самый файл,
    // что ушёл на вход бинарю, и после прогона эти записи не изменены.
    let snap = work.path().join("audit-at-apply");
    let at_apply = audit_files(&snap);
    let snap_by = |suffix: &str| {
        at_apply
            .iter()
            .find(|(n, _)| n.ends_with(suffix))
            .cloned()
            .unwrap_or_else(|| panic!("M-92 (A-044 O-3a): к моменту apply в аудит-следе нет *{suffix} — удаление раньше записи"))
    };
    let (snap_plan_name, snap_plan) = snap_by("plan.txt");
    for n in &plan {
        assert!(
            snap_plan.lines().any(|l| l.trim() == n),
            "M-92 (A-044 O-3a): план на момент apply не называет {n}"
        );
    }
    let (snap_man_name, snap_man) = snap_by("manifest.txt");
    let consumed = std::fs::read_to_string(work.path().join("manifest-consumed"))
        .expect("M-92 (A-044 O-3a): apply позван без --cold-manifest");
    assert_eq!(
        snap_man, consumed,
        "M-92 (A-044 O-3a): манифест аудит-следа ≠ манифесту, по которому решено удаление"
    );
    let after = audit_files(audit.path());
    for (name, body) in [(snap_plan_name, snap_plan), (snap_man_name, snap_man)] {
        assert!(
            after.iter().any(|(n, b)| *n == name && *b == body),
            "M-92 (A-044 O-3a): запись {name} изменена или удалена после apply"
        );
    }
}

/// **`c2` — НАСТОЯЩИЙ файл расписания, переключателя нет: ничего не удаляется даже при полностью
/// совпадающей копии.** Режим теста не перекрывает — его даёт `deploy/cron.d/journal-retention`.
#[test]
fn c2_cron_default_is_dry_run() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let audit = tempfile::tempdir().unwrap();
    let remote = remote_copy_of(dir.path(), None);
    let before = names(dir.path());
    let (code, out) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        audit.path(),
        None,
        None,
    );
    assert_eq!(
        code,
        Some(0),
        "M-92: прод-путь без переключателя вышел ненулём: {out}"
    );
    assert_eq!(
        names(dir.path()),
        before,
        "M-92: прод-путь без переключателя удалил сегменты"
    );
}

/// **`c3` — мутант файла расписания `RETENTION_MODE=apply` (то, что запрещено границей C), без
/// переключателя: ничего не удаляется** (`C-268` R2). Режим живёт только в файле на хосте:
/// `/etc/cron.d` перезаписывается каждым кодовым деплоем (2026-09-23 стёрта строка `CHECKPOINT_BANDS`).
#[test]
fn c3_cron_file_apply_without_switch_does_not_delete() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let audit = tempfile::tempdir().unwrap();
    let remote = remote_copy_of(dir.path(), None);
    let before = names(dir.path());
    let cron = std::fs::read_to_string(repo().join("deploy/cron.d/journal-retention")).unwrap();
    assert!(
        cron.contains("RETENTION_MODE="),
        "SETUP НЕ СОСТОЯЛСЯ: в файле расписания нет RETENTION_MODE="
    );
    let mutated: String = cron
        .lines()
        .map(|l| {
            if l.starts_with("RETENTION_MODE=") {
                "RETENTION_MODE=apply".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(
        mutated, cron,
        "SETUP НЕ СОСТОЯЛСЯ: мутант файла расписания не отличается"
    );
    let (_code, out) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        audit.path(),
        None,
        Some(mutated),
    );
    assert_eq!(
        names(dir.path()),
        before,
        "M-92: RETENTION_MODE=apply в файле расписания без переключателя удалил сегменты: {out}"
    );
}

/// **`c4` — аудит-след НЕ перезаписывается следующим прогоном (`A-044` O-2).** Два прогона в одном
/// сценарии: после первого остаётся только несовпавший, второй законно снова даёт `exit ≠ 0`. После
/// второго записи первого целы байт-в-байт, записи второго лежат отдельно под ДРУГИМИ именами.
#[test]
fn c4_audit_trail_survives_next_run() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let audit = tempfile::tempdir().unwrap();
    let pol = policy(work.path(), Some(max_covered(dir.path())));
    let plan = plan_names(&journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).unwrap());
    let victim = plan.iter().next_back().unwrap().clone();
    let remote = remote_copy_of(dir.path(), Some(&victim));
    let (_c1, out1) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        audit.path(),
        Some("apply"),
        None,
    );
    let first: BTreeSet<(String, String)> = audit_files(audit.path()).into_iter().collect();
    assert!(
        !first.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: первый прогон не записал аудит-след: {out1}"
    );
    // вход первого прогона снимается: ниже `manifest-consumed` обязан быть входом ВТОРОГО
    let _ = std::fs::remove_file(work.path().join("manifest-consumed"));
    let (_c2, out2) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        audit.path(),
        Some("apply"),
        None,
    );
    let second: BTreeSet<(String, String)> = audit_files(audit.path()).into_iter().collect();
    for rec in &first {
        assert!(
            second.contains(rec),
            "M-92 (A-044 O-2): запись первого прогона {} перезаписана вторым: {out2}",
            rec.0
        );
    }
    let first_names: BTreeSet<&String> = first.iter().map(|(n, _)| n).collect();
    let new: Vec<&String> = second
        .iter()
        .map(|(n, _)| n)
        .filter(|n| !first_names.contains(n))
        .collect();
    assert!(
        new.iter().any(|n| n.ends_with("plan.txt"))
            && new.iter().any(|n| n.ends_with("report.txt")),
        "M-92 (A-044 O-2): второй прогон не записал свои план и отчёт под НОВЫМИ именами: {new:?}"
    );
    // C-274-2: второй прогон удаляет на основании СВОЕГО манифеста — он обязан лечь в след под
    // новым именем и быть ТЕМ САМЫМ файлом, что ушёл бинарю (`manifest-consumed` перезаписывается
    // заглушкой при каждом `apply`, значит здесь — вход ВТОРОГО прогона).
    let consumed2 =
        std::fs::read_to_string(work.path().join("manifest-consumed")).unwrap_or_else(|e| {
            panic!("SETUP НЕ СОСТОЯЛСЯ: второй прогон не дошёл до apply ({e}): {out2}")
        });
    let new_manifests: Vec<&(String, String)> = second
        .iter()
        .filter(|(n, _)| !first_names.contains(n) && n.ends_with("manifest.txt"))
        .collect();
    assert_eq!(
        new_manifests.len(),
        1,
        "M-92 (A-044 O-2, C-274-2): второй прогон обязан записать РОВНО один новый манифест, есть {}: {new:?}",
        new_manifests.len()
    );
    assert_eq!(
        new_manifests[0].1, consumed2,
        "M-92 (A-044 O-2/O-3a, C-274-2): манифест второго прогона в следе не равен потреблённому бинарём"
    );
}

/// **`c5` — носитель аудита недоступен ⇒ ни одного удаления, выход ненулевой, тревога записана
/// (`A-044` O-3(b)).** `RETENTION_AUDIT_DIR` указывает внутрь обычного файла — каталог не создать.
#[test]
fn c5_no_audit_no_delete() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let blocker = work.path().join("blocker");
    std::fs::write(&blocker, "не каталог").unwrap();
    let audit = blocker.join("audit");
    assert!(
        std::fs::create_dir_all(&audit).is_err(),
        "SETUP НЕ СОСТОЯЛСЯ: каталог аудита создаётся"
    );
    let remote = remote_copy_of(dir.path(), None);
    let before = names(dir.path());
    let (code, out) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        &audit,
        Some("apply"),
        None,
    );
    assert_eq!(
        names(dir.path()),
        before,
        "M-92 (A-044 O-3b): при недоступном аудите удалены сегменты: {out}"
    );
    assert_ne!(
        code,
        Some(0),
        "M-92 (A-044 O-3b): при недоступном аудите выход 0: {out}"
    );
    assert!(
        work.path().join("retention.alert").exists(),
        "M-92 (A-044 O-3b): тревога при недоступном аудите не записана: {out}"
    );
}

/// **`c6` — нет копии ОДНОГО сегмента на коробке: прогон НЕ обрывается целиком (`R-221` Н-1, `I-2`).**
/// Настоящий `sha256sum a b c` при отсутствии части файлов печатает остальные и выходит 1. Скрипт
/// обязан отличать это от отказа связи и довести дело до бинаря: старшие сверенные удаляются,
/// отсутствующий (самый молодой кандидат) остаётся с причиной в отчёте, выход ≠ 0 и тревога.
#[test]
fn c6_one_missing_remote_copy_keeps_only_it() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let audit = tempfile::tempdir().unwrap();
    let pol = policy(work.path(), Some(max_covered(dir.path())));
    let plan = plan_names(&journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).unwrap());
    let missing = plan.iter().next_back().unwrap().clone();
    let remote = remote_copy_of(dir.path(), None);
    std::fs::remove_file(remote.path().join(&missing)).unwrap();
    assert!(
        !remote.path().join(&missing).exists(),
        "SETUP НЕ СОСТОЯЛСЯ: копия {missing} не убрана"
    );
    let (code, out) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        audit.path(),
        Some("apply"),
        None,
    );
    assert!(
        dir.path().join(&missing).exists(),
        "M-92: сегмент без копии на коробке удалён: {out}"
    );
    for n in plan.iter().filter(|n| **n != missing) {
        assert!(
            !dir.path().join(n).exists(),
            "M-92 (R-221 Н-1): сверенный {n} не удалён — отсутствие ОДНОЙ копии оборвало весь прогон: {out}"
        );
    }
    assert_contiguous(dir.path(), "c6");
    assert_ne!(
        code,
        Some(0),
        "M-92: отсутствие копии обязано дать ненулевой выход: {out}"
    );
    assert!(
        work.path().join("retention.alert").exists(),
        "M-92: отсутствие копии без тревоги: {out}"
    );
    let report: String = audit_files(audit.path())
        .into_iter()
        .filter(|(n, _)| n.ends_with("report.txt"))
        .map(|(_, c)| c)
        .collect();
    assert!(
        report.lines().any(
            |l| l.starts_with(&format!("kept {missing} ")) && l.split_whitespace().count() >= 3
        ),
        "M-92 (R-221 Н-1): в отчёте нет строки «kept {missing} <причина>»: {report}"
    );
}
