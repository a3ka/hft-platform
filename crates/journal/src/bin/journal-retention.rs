//! `journal-retention` — операторский путь ретеншена (M-08 task 11, TD-020).
//!
//! Отдельный бинарь (а НЕ поток внутри recorder'а) — падение уборки НЕ роняет сбор.
//! На проде запускается через cron; первый прогон — **обязательно `--mode dry-run`**
//! (per `.claude/rules/process.md` §8).
//!
//! ## Аргументы
//!
//! | Флаг | Описание | Дефолт |
//! |---|---|---|
//! | `--dir <PATH>` | каталог журнала (`segment-*.jrnl` + `journal.meta`) | `./journal-data` |
//! | `--cold <PATH>` | корень холодного хранилища (Storage Box / mount) | `./journal-cold` |
//! | `--retain-days <N>` | сегменты старше N суток — кандидаты | `14` |
//! | `--keep-min <N>` | минимум последних N сегментов остаются горячими | `4` |
//! | `--min-free-gb <N>` | ниже этого объёма (GiB) поднимается `disk_pressure` | `10` |
//! | `--now-wall-ms <i64>` | часы снаружи (детерминизм, ТОЛЬКО для тестов) | `SystemTime::now()` |
//! | `--mode <dry-run\|apply>` | режим исполнения | `dry-run` |
//!
//! ## Exit-коды
//!
//! - `0` — успех (DryRun завершился без эффектов; Apply выполнил все сверки+prune).
//! - `1` — неверные аргументы / I/O-ошибка чтения каталога.
//! - `2` — план построен, но при Apply хотя бы один сегмент дал сбой сверки (`failed` не пуст).
//! - `3` — `disk_pressure = true`: мало места, уборка не помогает. Требует внимания
//!   оператора даже если формально Apply прошёл (dry-run И apply).
//!
//! ## Безопасность
//!
//! - **Дефолт `--mode` = `dry-run`**: первая команда на проде ОБЯЗАНА быть им. Это
//!   конструктивный барьер против «случайно удалил» (урок M-07).
//! - **Битый/недоступный холодный путь → prune запрещён**, сегменты остаются
//!   горячими и попадают в `failed` (R3).
//! - **`disk_pressure` выводится всегда** (включая DryRun) — оператор видит тревогу
//!   даже при «успешном» прогоне.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use journal::{
    retention_execute, retention_plan, RetentionMode, RetentionPlan, RetentionPolicy,
    RetentionReport,
};

const DEFAULT_RETAIN_DAYS: u32 = 14;
const DEFAULT_KEEP_MIN: u32 = 4;
const DEFAULT_MIN_FREE_GB: u64 = 10;
const DEFAULT_DIR: &str = "./journal-data";
const DEFAULT_COLD: &str = "./journal-cold";

struct Args {
    dir: PathBuf,
    cold: PathBuf,
    retain_days: u32,
    keep_min: u32,
    min_free_bytes: u64,
    now_wall_ms: Option<i64>,
    mode: RetentionMode,
}

/// Парсинг argv. Возвращает Err с человеко-читаемой подсказкой на первой ошибке.
fn parse_args() -> Result<Args, String> {
    let mut dir: Option<PathBuf> = None;
    let mut cold: Option<PathBuf> = None;
    let mut retain_days: Option<u32> = None;
    let mut keep_min: Option<u32> = None;
    let mut min_free_gb: Option<u64> = None;
    let mut now_wall_ms: Option<i64> = None;
    let mut mode: Option<RetentionMode> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let next = || -> Result<&str, String> {
            args.get(i + 1)
                .map(|s| s.as_str())
                .ok_or_else(|| format!("флаг `{arg}` требует значение"))
        };
        match arg.as_str() {
            "--dir" => dir = Some(PathBuf::from(next()?)),
            "--cold" => cold = Some(PathBuf::from(next()?)),
            "--retain-days" => {
                retain_days = Some(
                    next()?
                        .parse::<u32>()
                        .map_err(|e| format!("--retain-days: {e}"))?,
                );
            }
            "--keep-min" => {
                keep_min = Some(
                    next()?
                        .parse::<u32>()
                        .map_err(|e| format!("--keep-min: {e}"))?,
                );
            }
            "--min-free-gb" => {
                min_free_gb = Some(
                    next()?
                        .parse::<u64>()
                        .map_err(|e| format!("--min-free-gb: {e}"))?,
                );
            }
            "--now-wall-ms" => {
                now_wall_ms = Some(
                    next()?
                        .parse::<i64>()
                        .map_err(|e| format!("--now-wall-ms: {e}"))?,
                );
            }
            "--mode" => {
                mode = Some(match next()? {
                    "dry-run" | "dryrun" | "dry" => RetentionMode::DryRun,
                    "apply" => RetentionMode::Apply,
                    other => {
                        return Err(format!(
                            "--mode: неизвестное значение `{other}` (ожидается dry-run|apply)"
                        ));
                    }
                });
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("неизвестный флаг `{other}` (попробуй --help)")),
        }
        i += 2;
    }

    Ok(Args {
        dir: dir.unwrap_or_else(|| PathBuf::from(DEFAULT_DIR)),
        cold: cold.unwrap_or_else(|| PathBuf::from(DEFAULT_COLD)),
        retain_days: retain_days.unwrap_or(DEFAULT_RETAIN_DAYS),
        keep_min: keep_min.unwrap_or(DEFAULT_KEEP_MIN),
        min_free_bytes: min_free_gb
            .unwrap_or(DEFAULT_MIN_FREE_GB)
            .saturating_mul(1024 * 1024 * 1024),
        now_wall_ms,
        mode: mode.unwrap_or(RetentionMode::DryRun),
    })
}

fn print_help() {
    println!(
        "journal-retention — операторский путь ретеншена (M-08 TD-020)\n\
         \n\
         Использование:\n  \
           journal-retention [--dir DIR] [--cold COLD] [--retain-days N] [--keep-min N]\n  \
                              [--min-free-gb N] [--now-wall-ms MS] [--mode dry-run|apply]\n\
         \n\
         Дефолты:\n  \
           --dir={DEFAULT_DIR}  --cold={DEFAULT_COLD}\n  \
           --retain-days={DEFAULT_RETAIN_DAYS}  --keep-min={DEFAULT_KEEP_MIN}  --min-free-gb={DEFAULT_MIN_FREE_GB}\n  \
           --mode=dry-run  (Apply — ТОЛЬКО после успешного DryRun на проде)\n\
         \n\
         Exit-коды:\n  \
           0 — успех\n  \
           1 — неверные аргументы / I/O\n  \
           2 — failed-сегменты при Apply (сверка холодной копии не прошла)\n  \
           3 — disk_pressure (мало места, уборка не помогает)\n"
    );
}

/// Дефолт для `--now-wall-ms`: системные часы. Вынесено в функцию, чтобы тест мог
/// подменить через переменную окружения без перекомпиляции.
fn default_now_wall_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("journal-retention: {e}");
            eprintln!("(см. --help)");
            return ExitCode::from(1);
        }
    };

    let now_wall_ms = args.now_wall_ms.unwrap_or_else(default_now_wall_ms);
    let policy = RetentionPolicy {
        retain_days: args.retain_days,
        keep_min_segments: args.keep_min,
        cold_root: args.cold.clone(),
        min_free_bytes: args.min_free_bytes,
    };

    let plan = match retention_plan(&args.dir, &policy, now_wall_ms) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("journal-retention: не удалось построить план: {e}");
            return ExitCode::from(1);
        }
    };

    // Печать плана (человеко-читаемая сводка) — даже если Apply будет падать, оператор
    // увидит, ЧТО планировалось сделать.
    print_plan(&args, &plan);

    let report = match retention_execute(&args.dir, &plan, &policy, args.mode) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("journal-retention: не удалось выполнить план: {e}");
            return ExitCode::from(1);
        }
    };
    print_report(&report);

    // Exit-логика: failed > disk_pressure > успех.
    // Порядок важен: при Apply с провалами сверки мы ОБЯЗАНЫ вернуть 2 ДО проверки
    // disk_pressure — иначе оператор не увидит, что данные не были выгружены.
    if !report.failed.is_empty() {
        eprintln!(
            "journal-retention: {} сегмент(ов) остались горячими из-за сбоя сверки холодной копии \
             (см. failed выше). ПОВТОРНОЕ ЗАПУСК Apply их не исправит — нужно проверить холодное хранилище.",
            report.failed.len()
        );
        return ExitCode::from(2);
    }
    if plan.disk_pressure {
        eprintln!(
            "journal-retention: ВНИМАНИЕ — свободного места меньше порога ({} GiB), \
             а выгружать было нечего (план пустой или все защищено keep_min/active). \
             Сбор данных остановится по disk-guard.",
            args.min_free_bytes / (1024 * 1024 * 1024)
        );
        return ExitCode::from(3);
    }
    ExitCode::SUCCESS
}

fn print_plan(args: &Args, plan: &RetentionPlan) {
    println!(
        "=== план ретеншена ===\n  dir={}\n  cold={}\n  retain_days={}  keep_min={}  min_free_gb={}\n  mode={:?}\n",
        args.dir.display(),
        args.cold.display(),
        args.retain_days,
        args.keep_min,
        args.min_free_bytes / (1024 * 1024 * 1024),
        args.mode,
    );
    println!(
        "  offload_and_prune: {} сегмент(ов)",
        plan.offload_and_prune.len()
    );
    for s in &plan.offload_and_prune {
        println!(
            "    - {} (index={}, size={} B, ts_exch/created={})",
            s.path.display(),
            s.index,
            s.size_bytes,
            s.header.created_wall_ms,
        );
    }
    println!("  skipped: {} сегмент(ов)", plan.skipped.len());
    for (s, reason) in &plan.skipped {
        println!("    - {} :: {}", s.path.display(), reason);
    }
    println!(
        "  disk_pressure: {}\n",
        if plan.disk_pressure { "ДА" } else { "нет" }
    );
}

fn print_report(report: &RetentionReport) {
    println!(
        "=== отчёт ===\n  mode={:?}\n  offloaded: {}  pruned: {}  failed: {}\n  freed_bytes: {}\n",
        report.mode,
        report.offloaded.len(),
        report.pruned.len(),
        report.failed.len(),
        report.freed_bytes,
    );
    for (path, reason) in &report.failed {
        println!("    FAIL {}: {}", path.display(), reason);
    }
}
