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
    compact_closed_segments, retention_execute, retention_plan, CompactionReport, EpochFilter,
    RetentionMode, RetentionPlan, RetentionPolicy, RetentionReport, DEFAULT_COMPACT_LEVEL,
};

/// Имя машинной записи дайджеста (JR-I-12, M-52/TD-067) — КОНТРАКТНОЕ: канарейка
/// `verify_M-52.sh` стоит на этой строке.
const REPLAY_DIGEST_RECORD: &str = "journal.replay-digest.json";

/// Exit-код расхождения дайджеста с `--expect` (1/2/3 уже заняты аргументами/сверкой
/// холодной копии/disk_pressure — см. док-шапку файла).
const EXIT_DIGEST_MISMATCH: u8 = 4;

/// Режим исполнения CLI. Отдельный от `journal::RetentionMode`: `ReplayDigest` — не операция
/// ретеншена, а самостоятельный операторский прогон дайджеста (JR-I-12); заводить под него
/// вариант в БИБЛИОТЕЧНОМ `RetentionMode` означало бы новую публичную поверхность крейта
/// сверх режима CLI (запрещено milestone M-52) и ломало бы исчерпывающие матчи
/// `retention_plan`/`retention_execute`, которые про дайджест ничего не знают.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    DryRun,
    Apply,
    Compact,
    ReplayDigest,
}

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
    mode: Mode,
    /// `--mode replay-digest`: нижняя граница окна (включительно), JR-I-12.
    from_seq: Option<u64>,
    /// `--mode replay-digest`: верхняя граница окна (включительно), JR-I-12.
    to_seq: Option<u64>,
    /// `--mode replay-digest`: ожидаемый `state_hash` (hex) — расхождение даёт
    /// `EXIT_DIGEST_MISMATCH` (JR-I-12).
    expect: Option<String>,
    /// M-38b (C-030 R1): путь к артефакту покрытия `covered_through_seq`, публикуемому
    /// `gateway-checkpoint` (минимум по всем сконфигурированным селекторам).
    /// None = «нет артефакта» = fail-closed (никаких prune, даже с override).
    checkpoint_coverage: Option<PathBuf>,
    /// M-38b: операторский escape-hatch — разрешает prune БЕЗ покрытия чекпоинтом.
    /// Дефолт `false` (fail-closed); verify-канарейка verify_M-38b.sh блокирует
    /// передачу этого флага в проде (поведенческий fail-closed).
    allow_prune_without_checkpoint: bool,
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
    let mut mode: Option<Mode> = None;
    let mut from_seq: Option<u64> = None;
    let mut to_seq: Option<u64> = None;
    let mut expect: Option<String> = None;
    let mut checkpoint_coverage: Option<PathBuf> = None;
    let mut allow_prune_without_checkpoint: bool = false;

    // TD-024: нормализовать argv ПЕРЕД циклом разбора. `--flag=value` (equals-форма — ровно
    // то, что лежит в `docker-compose.yml command:` и печатает `--help`) раскладываем в два
    // отдельных элемента `--flag` + `value`, чтобы ручной `match arg { "--flag" => next() }`
    // ниже мог работать единой формой для обоих вариантов. Раздельная форма (как в cron-скрипте)
    // проходит через `vec![a]` без изменений — регрессии нет.
    let args: Vec<String> = std::env::args()
        .skip(1)
        .flat_map(|a| {
            if a.starts_with("--") {
                if let Some((k, v)) = a.split_once('=') {
                    return vec![k.to_string(), v.to_string()];
                }
            }
            vec![a]
        })
        .collect();
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
                    "dry-run" | "dryrun" | "dry" => Mode::DryRun,
                    "apply" => Mode::Apply,
                    // D-COMP-3: третий режим — компакция закрытых сегментов (zstd).
                    "compact" | "compaction" => Mode::Compact,
                    // JR-I-12 (M-52/TD-067): четвёртый режим — дайджест реплея боевого
                    // журнала уже доставленным бинарём (на VPS нет Rust toolchain).
                    "replay-digest" | "replay_digest" => Mode::ReplayDigest,
                    other => {
                        return Err(format!(
                            "--mode: неизвестное значение `{other}` \
                             (ожидается dry-run|apply|compact|replay-digest)"
                        ));
                    }
                });
            }
            "--from" => {
                from_seq = Some(next()?.parse::<u64>().map_err(|e| format!("--from: {e}"))?);
            }
            "--to" => {
                to_seq = Some(next()?.parse::<u64>().map_err(|e| format!("--to: {e}"))?);
            }
            "--expect" => {
                expect = Some(next()?.to_string());
            }
            // M-38b (C-030 R1): путь к артефакту покрытия `covered_through_seq`. Если
            // файла нет или он не парсится в u64 — `covered = None` (fail-closed).
            "--checkpoint-coverage" => {
                checkpoint_coverage = Some(PathBuf::from(next()?));
            }
            // M-38b: операторский escape-hatch. Дефолт `false` (fail-closed);
            // verify-канарейка verify_M-38b.sh ЗАПРЕЩАЕТ передачу этого флага в проде.
            "--allow-prune-without-checkpoint" => {
                allow_prune_without_checkpoint = true;
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
        mode: mode.unwrap_or(Mode::DryRun),
        from_seq,
        to_seq,
        expect,
        checkpoint_coverage,
        allow_prune_without_checkpoint,
    })
}

fn print_help() {
    println!(
        "journal-retention — операторский путь ретеншена, компакции И дайджеста реплея \
         (M-08 TD-020+TD-022, M-52 TD-067)\n\
         \n\
         Использование:\n  \
           journal-retention [--dir DIR] [--cold COLD] [--retain-days N] [--keep-min N]\n  \
                              [--min-free-gb N] [--keep-raw N] [--now-wall-ms MS]\n  \
                              [--mode dry-run|apply|compact|replay-digest]\n  \
                              [--from SEQ] [--to SEQ] [--expect HEX]\n\
         \n\
         Дефолты:\n  \
           --dir={DEFAULT_DIR}  --cold={DEFAULT_COLD}\n  \
           --retain-days={DEFAULT_RETAIN_DAYS}  --keep-min={DEFAULT_KEEP_MIN}\n  \
           --min-free-gb={DEFAULT_MIN_FREE_GB}  --keep-raw={DEFAULT_KEEP_RAW}\n  \
           --mode=dry-run  (Apply — ТОЛЬКО после успешного DryRun на проде; Compact безопасен по дизайну)\n\
         \n\
         --mode replay-digest (JR-I-12, TD-067): считает `journal::replay_digest` ПОТОКОВО\n  \
           (не read_all/recover — 26 GB/148M событий в RAM недопустимо), печатает\n  \
           events/first_seq/last_seq/state_hash, пишет {REPLAY_DIGEST_RECORD} рядом с\n  \
           журналом (атомарно). Read-only по данным журнала — recorder дайджест не считает\n  \
           никогда. `--from`/`--to` — ЗАКРЫТОЕ окно (включительно) — единственная форма,\n  \
           воспроизводимая на живом журнале; без них окно ОТКРЫТОЕ и растёт под сканом.\n  \
           `--expect HEX` — сверка со state_hash: расхождение даёт exit {EXIT_DIGEST_MISMATCH}\n  \
           и печатает ОБЕ величины.\n\
         \n\
         Exit-коды:\n  \
           0 — успех\n  \
           1 — неверные аргументы / I/O\n  \
           2 — failed-сегменты при Apply/Compact (сверка холодной копии / sha256 .zst не прошла)\n  \
           3 — disk_pressure (мало места, уборка не помогает — только retention)\n  \
           {EXIT_DIGEST_MISMATCH} — replay-digest: state_hash разошёлся с --expect\n"
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
    if args.mode == Mode::Compact {
        return run_compact(&args);
    }

    // JR-I-12 (M-52/TD-067): дайджест реплея — ОТДЕЛЬНЫЙ режим, не операция ретеншена.
    // Read-only по данным журнала, не проходит через retention_plan/execute/cold.
    if args.mode == Mode::ReplayDigest {
        return run_replay_digest(&args);
    }

    let rmode = match args.mode {
        Mode::DryRun => RetentionMode::DryRun,
        Mode::Apply => RetentionMode::Apply,
        Mode::Compact | Mode::ReplayDigest => unreachable!("обработаны выше"),
    };

    let now_wall_ms = args.now_wall_ms.unwrap_or_else(default_now_wall_ms);
    // M-38b (C-030 R1): артефакт покрытия чекпоинта `--checkpoint-coverage <path>` —
    // читаем число `covered_through_seq` (минимум по сконфигурированным селекторам,
    // публикуемый `gateway-checkpoint`). Если файла нет — `covered = None` (fail-closed).
    // M-38b (C-031 NOTE): escape-hatch `--allow-prune-without-checkpoint` ЗАПРЕЩЁН на проде
    // (канарейка verify_M-38b.sh блокирует дефолт), но CLI его объявляет для явных override.
    let covered_through_seq = if let Some(ref path) = args.checkpoint_coverage {
        match std::fs::read_to_string(path) {
            Ok(s) => s.trim().parse::<u64>().ok(),
            Err(_) => None, // missing file → fail-closed
        }
    } else {
        None
    };
    let policy = RetentionPolicy {
        retain_days: args.retain_days,
        keep_min_segments: args.keep_min,
        cold_root: args.cold.clone(),
        min_free_bytes: args.min_free_bytes,
        checkpoint_covered_through_seq: covered_through_seq,
        allow_prune_without_checkpoint: args.allow_prune_without_checkpoint,
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

    let report = match retention_execute(&args.dir, &plan, &policy, rmode) {
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

/// Hex-кодировка `state_hash` (32 байта → 64 hex-символа, lowercase).
fn hex32(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// `--mode replay-digest` (JR-I-12, M-52/TD-067): `DET-I-1` обязан быть наблюдаем на боевом
/// журнале средствами, УЖЕ доставленными в прод (на VPS нет Rust toolchain — до сих пор
/// единственной проверкой был sha256 ФАЙЛА, что доказывает неизменность байт, а НЕ
/// воспроизводимость реплея).
///
/// Считает `journal::replay_digest` ПОТОКОВО (не `read_all`/`recover` — прод 26 GB/148M
/// событий в RAM недопустимо, класс TD-011), печатает окно+хэш, пишет машинную запись
/// атомарно рядом с журналом, при `--expect` сверяет и возвращает `EXIT_DIGEST_MISMATCH` на
/// расхождении. Read-only по данным журнала — recorder дайджест не считает НИКОГДА (отдельный
/// операторский прогон, не горячий путь сбора).
fn run_replay_digest(args: &Args) -> ExitCode {
    let digest =
        match journal::replay_digest(&args.dir, EpochFilter::All, args.from_seq, args.to_seq) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("journal-retention[replay-digest]: не удалось посчитать дайджест: {e}");
                return ExitCode::from(1);
            }
        };

    let hash = hex32(&digest.state_hash);
    // Честность окна (JR-I-12): дайджест ОТКРЫТОГО окна на живом журнале невоспроизводим по
    // построению (recorder дописывает события, пока идёт скан) — запись обязана называть
    // РЕАЛЬНО покрытое окно, а не молчать `none` как «ноль», когда событий не было вовсе.
    let first = digest
        .first_seq
        .map(|s| s.to_string())
        .unwrap_or_else(|| "none".to_string());
    let last = digest
        .last_seq
        .map(|s| s.to_string())
        .unwrap_or_else(|| "none".to_string());
    println!(
        "events={} first_seq={} last_seq={} state_hash={}",
        digest.events, first, last, hash
    );

    let record = serde_json::json!({
        "events": digest.events,
        "first_seq": digest.first_seq,
        "last_seq": digest.last_seq,
        "state_hash": hash,
    });
    if let Err(e) = write_replay_digest_record(&args.dir, &record) {
        eprintln!(
            "journal-retention[replay-digest]: не удалось записать {REPLAY_DIGEST_RECORD}: {e}"
        );
        return ExitCode::from(1);
    }

    if let Some(expect) = &args.expect {
        let expect_norm = expect.to_lowercase();
        if expect_norm != hash {
            eprintln!(
                "journal-retention[replay-digest]: JR-I-12 РАСХОЖДЕНИЕ state_hash — DET-I-1 \
                 НАРУШЕН на боевом журнале: expect={expect_norm} actual={hash}"
            );
            println!("expect={expect_norm} actual={hash}");
            return ExitCode::from(EXIT_DIGEST_MISMATCH);
        }
    }
    ExitCode::SUCCESS
}

/// Атомарная запись машинной записи дайджеста (tmp + rename в том же каталоге — не
/// оставляет `.tmp`-хвостов ни при первом, ни при повторном прогоне).
fn write_replay_digest_record(
    dir: &std::path::Path,
    record: &serde_json::Value,
) -> std::io::Result<()> {
    let path = dir.join(REPLAY_DIGEST_RECORD);
    let tmp = dir.join(format!("{REPLAY_DIGEST_RECORD}.tmp"));
    let bytes = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
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
        // M-49 (TD-051): `seg_ts` — ФАКТИЧЕСКОЕ значение, по которому retention_plan принял
        // решение о возрасте (ts_exch_ms первого события, fallback на header.created_wall_ms
        // — journal::segment_decision_ts — ЕДИНЫЙ источник истины с retention_plan). Печатаем
        // ОБА поля раздельно и честно подписанными: раньше здесь стоял ТОЛЬКО
        // header.created_wall_ms под подписью «ts_exch/created=», выдавая его за основу
        // решения, хотя решение принималось по данным события.
        println!(
            "    - {} (index={}, size={} B, seg_ts={}, header.created_wall_ms={})",
            s.path.display(),
            s.index,
            s.size_bytes,
            journal::segment_decision_ts(s),
            s.header.created_wall_ms,
        );
    }
    println!("  offload_only: {} сегмент(ов)", plan.offload_only.len());
    for s in &plan.offload_only {
        println!(
            "    - {} (index={}, size={} B, seg_ts={}, header.created_wall_ms={})",
            s.path.display(),
            s.index,
            s.size_bytes,
            journal::segment_decision_ts(s),
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
