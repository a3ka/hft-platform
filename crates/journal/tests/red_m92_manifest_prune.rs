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
    let victim = want.iter().next().unwrap().clone();
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
    guard_untouched(&w);
}

/// **`p3` — сумма в манифесте НЕ совпала (копия на коробке испорчена): сегмент остаётся.**
#[test]
fn p3_mismatching_sum_keeps_segment() {
    let w = world(true);
    let want = plan_names(&w.plan);
    let victim = want.iter().next().unwrap().clone();
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
    // манифест: все кроме одного совпадают
    let victim = planned[0].clone();
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
}

// ───────────────────────── скрипт расписания ─────────────────────────

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Прогнать НАСТОЯЩИЙ `deploy/bin/journal-retention-cron.sh` в окружении прод-cron'а
/// (`deploy/cron.d/journal-retention`), заменив ТОЛЬКО два шва: `RETENTION_RUNNER` — заглушкой,
/// исполняющей настоящий бинарь с путями контейнера `/journal`, `/ckpt`, `/work`, перенацеленными на
/// фикстуру; `RETENTION_REMOTE_SHA_CMD` — «коробкой», считающей суммы по ОТДЕЛЬНОМУ каталогу копии.
fn run_wrapper(
    dir: &Path,
    remote_copy: &Path,
    work: &Path,
    audit: &Path,
    mode: Option<&str>,
    env_mode: &str,
) -> (Option<i32>, String) {
    let ckpt = work.join("ckpt");
    std::fs::create_dir_all(&ckpt).unwrap();
    std::fs::write(
        ckpt.join("covered_through_seq"),
        max_covered(dir).to_string(),
    )
    .unwrap();
    // Режим — ТОЛЬКО файл-переключатель на хосте (деплой его не перезаписывает, в отличие от
    // /etc/cron.d); None — файла нет ⇒ dry-run.
    match mode {
        Some(m) => std::fs::write(work.join("retention.mode"), m).unwrap(),
        None => {
            let _ = std::fs::remove_file(work.join("retention.mode"));
        }
    }
    let shim_dir = work.join("shim");
    std::fs::create_dir_all(&shim_dir).unwrap();
    let runner = shim_dir.join("runner");
    std::fs::write(
        &runner,
        format!(
            "#!/bin/bash\n# заглушка docker compose run: всё до имени сервиса отбрасывается\nwhile [ $# -gt 0 ] && [ \"$1\" != journal-retention ]; do shift; done; shift\n\
             args=(); for a in \"$@\"; do a=\"${{a//\\/journal/{d}}}\"; a=\"${{a//\\/ckpt/{c}}}\"; a=\"${{a//\\/work/{w}}}\"; args+=(\"$a\"); done\n\
             exec {bin} \"${{args[@]}}\" --now-wall-ms={now}\n",
            d = dir.display(),
            c = ckpt.display(),
            w = work.join("hostwork").display(),
            bin = BIN,
            now = T0 + 100 * DAY_MS
        ),
    )
    .unwrap();
    let remote = shim_dir.join("remote-sha");
    std::fs::write(
        &remote,
        format!(
            "#!/bin/bash\n# «коробка»: аргументы вида journal/<имя>, суммы по КОПИИ, вывод как у sha256sum\n\
             cd {r} && for a in \"$@\"; do sha256sum \"${{a#journal/}}\" | sed 's#  #  journal/#'; done\n",
            r = remote_copy.display()
        ),
    )
    .unwrap();
    for f in [&runner, &remote] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let cron = std::fs::read_to_string(repo().join("deploy/cron.d/journal-retention")).unwrap();
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
        .env("RETENTION_WORK_DIR", work.join("hostwork"))
        .env("RETENTION_AUDIT_DIR", audit)
        .env("RETENTION_MODE_FILE", work.join("retention.mode"))
        .env("RETENTION_MODE", env_mode)
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

/// **`c1` — прод-путь cron'а в режиме `apply`: удалены сверенные с КОПИЕЙ, испорченная копия
/// удержала свой сегмент; аудит-след записан.** Свидетель «суммы взяты с удалённой стороны»: копия
/// одного сегмента испорчена — сумма, посчитанная локально, этого не увидела бы.
#[test]
fn c1_cron_apply_verifies_against_remote_copy() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let audit = tempfile::tempdir().unwrap();
    let pol = policy(work.path(), Some(max_covered(dir.path())));
    let plan = plan_names(&journal::retention_plan(dir.path(), &pol, T0 + 100 * DAY_MS).unwrap());
    let victim = plan.iter().next().unwrap().clone();
    let remote = remote_copy_of(dir.path(), Some(&victim));
    let (code, out) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        audit.path(),
        Some("apply"),
        "dry-run",
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
    let trail: Vec<String> = std::fs::read_dir(audit.path())
        .map(|r| {
            r.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !trail.is_empty(),
        "M-92: аудит-след в RETENTION_AUDIT_DIR не записан"
    );
}

/// **`c2` — файла-переключателя нет (по умолчанию): ничего не удаляется даже при полностью
/// совпадающей копии.** Включение — запись `apply` в файл-переключатель (`M-92` §4, `П-031`).
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
        "dry-run",
    );
    assert_eq!(
        code,
        Some(0),
        "M-92: dry-run прод-пути вышел ненулём: {out}"
    );
    assert_eq!(
        names(dir.path()),
        before,
        "M-92: dry-run прод-пути удалил сегменты"
    );
}

/// **`c3` — `RETENTION_MODE=apply` в окружении cron'а БЕЗ файла-переключателя: ничего не удаляется.**
/// Режим живёт только в файле на хосте: `/etc/cron.d` перезаписывается каждым кодовым деплоем
/// (так 2026-09-23 была стёрта строка `CHECKPOINT_BANDS`), и режим удаления не смеет зависеть от него.
#[test]
fn c3_env_mode_without_switch_file_does_not_delete() {
    let dir = journal();
    let work = tempfile::tempdir().unwrap();
    let audit = tempfile::tempdir().unwrap();
    let remote = remote_copy_of(dir.path(), None);
    let before = names(dir.path());
    let (_code, out) = run_wrapper(
        dir.path(),
        remote.path(),
        work.path(),
        audit.path(),
        None,
        "apply",
    );
    assert_eq!(
        names(dir.path()),
        before,
        "M-92: RETENTION_MODE=apply из окружения без файла-переключателя удалил сегменты: {out}"
    );
}
