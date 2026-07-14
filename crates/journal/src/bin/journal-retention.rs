//! `journal-retention` — операторский путь ретеншена И компакции (M-08 task 11+16, TD-020+TD-022).
//!
//! ОДИН бинарь для двух операций: ретеншен (выгрузка+prune) и компакция закрытых
//! сегментов (zstd). Логика — это две разные операции, но контракт argv и шов
//! алерта/логирования ОБЩИЕ — один `-cron.sh`, один гейт cron'а, один Dockerfile.
//! Парадигма: «CLI ДОЛЖЕН уметь всё, что оператор зовёт из cron'а», а не плодить
//! `--bin journal-compactor` рядом (rev 9 блокер reviewer'а: «функция без оператора»).
//!
//! Отдельный бинарь (а НЕ поток внутри recorder'а) — падение уборки НЕ роняет сбор.
//! На проде запускается через cron; первый прогон ретеншена — **обязательно `--mode dry-run`**
//! (per `.claude/rules/process.md` §8). Компакция — перманентно безопасна: пишет во
//! временный файл, сверяет sha256, и только потом удаляет оригинал (D-COMP-2).
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
//! | `--keep-raw <N>` | `--mode compact`: последние N закрытых сегментов остаются сырыми | `2` |
//! | `--now-wall-ms <i64>` | часы снаружи (детерминизм, ТОЛЬКО для тестов) | `SystemTime::now()` |
//! | `--mode <dry-run\|apply\|compact>` | режим исполнения | `dry-run` |
//!
//! ## Exit-коды
//!
//! - `0` — успех (DryRun завершился без эффектов; Apply выполнил все сверки+prune;
//!   Compact завершился без провалов верификации).
//! - `1` — неверные аргументы / I/O-ошибка чтения каталога.
//! - `2` — план построен, но при Apply хотя бы один сегмент дал сбой сверки (`failed` не пуст);
//!   в `--mode compact` — `compact_closed_segments` вернул `Err` (хотя бы один сегмент
//!   не доделал операцию: крах-окно с битым `.zst` → оригинал остался ГОРЯЧИМ, сегмент
//!   в `failed`. Никаких данных не потеряно, но оператор ОБЯЗАН увидеть алерт).
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
//!
//! ## Компакция (D-COMP-3)
//!
//! `--mode compact` — отдельный режим:
//! - не задействует cold storage и disk-guard (`compact_closed_segments` сам
//!   обрабатывает крах-окно: D-COMP-1/2);
//! - `--keep-raw N` оставляет последние N закрытых сегментов сырыми (свежее
//!   читается чаще, несжатый доступ дешевле);
//! - уровень zstd фиксированный `DEFAULT_COMPACT_LEVEL=3` (9.1× на боевых данных);
//! - exit 0 — все сегменты успешно сжаты или самоизлечены; exit 2 — была ошибка
//!   верификации (битый .zst, несверяемая копия → оригинал оставлен ГОРЯЧИМ).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use journal::{
    compact_closed_segments, retention_execute, retention_plan, CompactionReport, RetentionMode,
    RetentionPlan, RetentionPolicy, RetentionReport, DEFAULT_COMPACT_LEVEL,
};

const DEFAULT_RETAIN_DAYS: u32 = 14;
const DEFAULT_KEEP_MIN: u32 = 4;
const DEFAULT_MIN_FREE_GB: u64 = 10;
const DEFAULT_KEEP_RAW: u32 = 2;
const DEFAULT_DIR: &str = "./journal-data";
const DEFAULT_COLD: &str = "./journal-cold";

struct Args {
    dir: PathBuf,
    cold: PathBuf,
    retain_days: u32,
    keep_min: u32,
    min_free_bytes: u64,
    /// `--mode compact` — последние N закрытых сегментов остаются сырыми (D-COMP-3).
    keep_raw: u32,
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
    let mut keep_raw: Option<u32> = None;
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
            "--keep-raw" => {
                keep_raw = Some(
                    next()?
                        .parse::<u32>()
                        .map_err(|e| format!("--keep-raw: {e}"))?,
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
                    // D-COMP-3: третий режим — компакция закрытых сегментов (zstd).
                    "compact" | "compaction" => RetentionMode::Compact,
                    other => {
                        return Err(format!(
                            "--mode: неизвестное значение `{other}` \
                             (ожидается dry-run|apply|compact)"
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
        keep_raw: keep_raw.unwrap_or(DEFAULT_KEEP_RAW),
        now_wall_ms,
        mode: mode.unwrap_or(RetentionMode::DryRun),
    })
}

fn print_help() {
    println!(
        "journal-retention — операторский путь ретеншена И компакции (M-08 TD-020+TD-022)\n\
         \n\
         Использование:\n  \
           journal-retention [--dir DIR] [--cold COLD] [--retain-days N] [--keep-min N]\n  \
                              [--min-free-gb N] [--keep-raw N] [--now-wall-ms MS]\n  \
                              [--mode dry-run|apply|compact]\n\
         \n\
         Дефолты:\n  \
           --dir={DEFAULT_DIR}  --cold={DEFAULT_COLD}\n  \
           --retain-days={DEFAULT_RETAIN_DAYS}  --keep-min={DEFAULT_KEEP_MIN}\n  \
           --min-free-gb={DEFAULT_MIN_FREE_GB}  --keep-raw={DEFAULT_KEEP_RAW}\n  \
           --mode=dry-run  (Apply — ТОЛЬКО после успешного DryRun на проде; Compact безопасен по дизайну)\n\
         \n\
         Exit-коды:\n  \
           0 — успех\n  \
           1 — неверные аргументы / I/O\n  \
           2 — failed-сегменты при Apply/Compact (сверка холодной копии / sha256 .zst не прошла)\n  \
           3 — disk_pressure (мало места, уборка не помогает — только retention)\n"
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

    // D-COMP-3: компакция — ОТДЕЛЬНЫЙ режим ТОГО ЖЕ бинаря, со своим argv-минимумом
    // (--dir, --keep-raw, --mode compact). Никакого retention_plan/cold — компакция
    // безопасна по дизайну (D-COMP-2 самоизлечение), и шов с cold-storage тут не нужен.
    // Сужаем поведение до понятного cron'у подмножества.
    if args.mode == RetentionMode::Compact {
        return run_compact(&args);
    }

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

/// Запустить компакцию закрытых сегментов (D-COMP-3).
///
/// Отдельная функция, потому что путь короткий (нет cold, нет retention_plan/execute,
/// нет disk-guard), и смешивать его с ретеншеном в одном теле — значит заводить
/// развесистый `if/else` по двум разным алгоритмам. Здесь — всё одной сводкой:
/// сжать → отчитаться → exit 0/2 (провал sha256 = exit 2, без потери данных).
fn run_compact(args: &Args) -> ExitCode {
    println!(
        "=== компакция закрытых сегментов (D-COMP-3) ===\n  \
         dir={}\n  keep_raw={}  compact_level={}\n",
        args.dir.display(),
        args.keep_raw,
        DEFAULT_COMPACT_LEVEL,
    );

    // На пустом каталоге / на свежем deploy'е `compact_closed_segments` обязан вернуть
    // пустой Vec без ошибок: первая строка cron-задания (`$RETENTION_MODE=dry-run`-ом
    // всё разворачивается, вызывается на VPS ещё до появления сегментов) не должна
    // ронять алерт.
    match compact_closed_segments(&args.dir, args.keep_raw, DEFAULT_COMPACT_LEVEL) {
        Ok(reports) => {
            print_compact_reports(&reports);
            if reports.is_empty() {
                println!("  (нет закрытых сегментов — нечего сжимать)");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            // D-COMP-2: Err из compact_closed_segments — это сбой sha256-сверки
            // существующего .zst (D-COMP-2). Данные НЕ ПОТЕРЯНЫ (оригинал остался
            // ГОРЯЧИМ), но оператор обязан увидеть тревогу: следующий прогон
            // перепишет .zst; если ошибка не уходит — что-то глубже сломано.
            eprintln!(
                "journal-retention[compact]: {} сегмент(ов) остались в конфликте raw+.zst \
                 после самоизлечения. Следующий прогон перепишет .zst с нуля. Оригиналы НЕ \
                 удалены. Подробности: {e}",
                1
            );
            ExitCode::from(2)
        }
    }
}

/// Печать итогов компакции (отдельная, чтобы не перегружать print_plan/print_report).
fn print_compact_reports(reports: &[CompactionReport]) {
    if reports.is_empty() {
        return;
    }
    println!("  compacted: {} сегмент(ов)", reports.len());
    let mut total_before: u64 = 0;
    let mut total_after: u64 = 0;
    for r in reports {
        let ratio = if r.bytes_before > 0 {
            1.0 - (r.bytes_after as f64 / r.bytes_before as f64)
        } else {
            0.0
        };
        println!(
            "    - {} → {} ({} → {} B, −{:.1}%)",
            r.source.display(),
            r.compacted.display(),
            r.bytes_before,
            r.bytes_after,
            ratio * 100.0,
        );
        total_before += r.bytes_before;
        total_after += r.bytes_after;
    }
    if total_before > 0 {
        println!(
            "  итого: {} → {} B (коэффициент {:.2}×)",
            total_before,
            total_after,
            total_before as f64 / total_after.max(1) as f64,
        );
    }
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
