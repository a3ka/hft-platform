//! `gateway-checkpoint` — операторский путь снятия чекпоинта (M-38b, TD-044, GW-I-9).
//!
//! Снимает чекпоинт `Reducer` (полное состояние + lineage) для каждого сконфигурированного
//! селектора, ПУБЛИКУЕТ минимум `covered_through_seq` в артефакт (для ops-сервиса
//! `journal-retention` — C-030 R1), и опционально пишет «состояние на сейчас»
//! (snapshots — отдельный путь, не блокирует checkpoint-cadence).
//!
//! **OPS-модель:** отдельный сервис под `profiles: ["ops"]`, journal-том смонтирован
//! `:ro` (JR-I-1 — единственный писатель журнала это recorder). Cadence — cron
//! (`cron-скрипт` чекпоинтер → retention, см. deploy/README.md).
//!
//! ## Аргументы
//!
//! | Флаг | Описание | Дефолт |
//! |---|---|---|
//! | `--dir <PATH>` | каталог журнала (`segment-*.jrnl` + `journal.meta`) | `./journal-data` |
//! | `--ckpt-dir <PATH>` | каталог чекпоинта (создаётся) | `./gateway-ckpt` |
//! | `--coverage-out <PATH>` | куда писать артефакт `covered_through_seq` (min по селекторам) | `./gateway-ckpt/covered_through_seq` |
//! | `--venue <Binance\|BinanceFutures\|Hyperliquid>` | площадка | `Binance` |
//! | `--symbol <STR>` | канонический тикер | `BTCUSDT` |
//! | `--timeframe-ms <i64>` | таймфрейм, должен делить 86_400_000 (GW-I-10) | `1000` |
//! | `--bands <f64,f64,...>` | depth-полосы (×1, напр. `0.001,0.005`) | `0.001` |
//! | `--window-ms <i64>` | bounded-window (M-37); `0` = offline unbounded | `60000` |
//! | `--cursor <LATEST\|i64>` | курсор для advance_to. `LATEST` = до конца журнала | `LATEST` |
//! | `--depth-cadence-ms <i64>` | каденция депт-серии (M-68); дефолт = `GATEWAY_DEPTH_CADENCE_MS` из env, иначе 1000 | env `GATEWAY_DEPTH_CADENCE_MS` или `1000` |
//! | `--print-ckpt-name` | (M-94) напечатать имя слепка профиля и выйти 0 БЕЗ открытия журнала | (нет) |
//!
//! **M-90 (TD-227) — единый источник селектора с сервером выдачи.** Все шесть осей
//! (`--venue` / `--symbol` / `--timeframe-ms` / `--bands` / `--window-ms` /
//! `--depth-cadence-ms`) при ОТСУТСТВИИ флага читаются из `GATEWAY_*` ТЕМ ЖЕ путём,
//! каким уже читается `--depth-cadence-ms` → `GATEWAY_DEPTH_CADENCE_MS`. Флаг имеет
//! приоритет (явная команда оператора); env — следующий; подписанный дефолт в коде —
//! последний. Невалидное значение env ⇒ exit 2 с сообщением, называющим ПЕРЕМЕННУЮ
//! (оператор видит, ГДЕ править) — тот же класс `A-015` §3 п.1, что у
//! `GATEWAY_DEPTH_CADENCE_MS` (fail-closed).
//!
//! Обе формы `--flag value` И `--flag=value` принимаются наравне: compose пишет
//! `--flag=value`, cron-обёртка может писать через пробел. Соседний `journal-retention`
//! уже так работает; здесь — единый контракт.
//!
//! ## Режим профиля (M-94, П-032)
//!
//! `GATEWAY_CALC_PROFILE=<path>` в env ⇒ бинарь в **режиме профиля**: все восемь величин
//! (`CALC_PROFILE_VERSION` + 7 `GATEWAY_*`) берутся из файла; наличие любого из них
//! в env (`П-032` п.1 — даже значение, РАВНОЕ профилю) ИЛИ флага оси (`--bands`,
//! `--timeframe-ms`, `--window-ms`, `--depth-cadence-ms`) ⇒ отказ СТАРТА с сообщением,
//! называющим ключ/флаг И слово `GATEWAY_CALC_PROFILE` (оператор видит, ГДЕ править;
//! оракулы `p3`/`p4`/`s2`). `--print-ckpt-name` в этом режиме печатает имя слепка
//! профиля (`ckpt-<fp16>.bin`) и выходит 0, НЕ открывая журнал — это опора гейта
//! деплоя (спека §3.7, оракул `p7`).
//!
//! ## Exit-коды
//!
//! - `0` — успех (чекпоинт снят + артефакт покрытия записан + `.profile` записан).
//! - `1` — неверные аргументы / I/O.
//! - `2` — `validate_selector` отверг селектор (GW-I-10 fail-closed) или
//!   невалидная каденция (MD-I-8 d14), или отказ режима профиля.
//!
//! ## Безопасность
//!
//! - **Journal-том только-чтение** на уровне compose (`docker-compose.yml` mount `:ro`) —
//!   JR-I-1 гарантирован.
//! - **Дефолт `--window-ms=60000`** (M-37 анти-TD-020): без активного окна прод-снапшот
//!   ООМ-ит (TD-039 воспроизводится через §8 E2E).
//! - **`covered_through_seq`** публикуется ТОЛЬКО если чекпоинт успешно записан. Если
//!   `advance_to` падает — артефакт остаётся СТАРЫМ (write через tmp+rename), и
//!   retention-сервис в следующем цикле использует устаревший артефакт (= fail-closed).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gateway::calc_profile::{
    self, AppliedProfile, RawLiveProfile, PROFILE_AXIS_FLAGS,
};
use gateway::checkpoint;
use gateway::{Cursor, Selector};
use contracts::Venue;
use journal::EpochFilter;

#[derive(Debug)]
struct Args {
    dir: PathBuf,
    ckpt_dir: PathBuf,
    coverage_out: PathBuf,
    venue: Venue,
    symbol: String,
    timeframe_ms: i64,
    bands: Vec<f64>,
    window_ms: Option<i64>,
    cursor: Cursor,
    /// M-68 задача 23 (R-141 Б-1): каденция депт-серии, прочитанная из флага
    /// `--depth-cadence-ms` или env `GATEWAY_DEPTH_CADENCE_MS` (тот же источник, что у
    /// `gateway-serve::serve_config_from_env`, чтобы отпечаток селектора у писателя и
    /// читателя чекпоинта СОВПАЛ). `None` означает «не задано ни флагом, ни env» —
    /// дефолт 1000 мс подставляется ниже по тем же правилам, что и в `serve_config_from_env`.
    depth_cadence_ms: Option<i64>,
    /// M-94 (задача 2): `--print-ckpt-name` — напечатать имя слепка (для гейта деплоя)
    /// и выйти 0. В режиме профиля: имя из профиля, журнал НЕ открывается.
    print_ckpt_name: bool,
}

fn parse_venue(s: &str) -> Result<Venue, String> {
    match s {
        "Binance" => Ok(Venue::Binance),
        "BinanceFutures" => Ok(Venue::BinanceFutures),
        "Hyperliquid" => Ok(Venue::Hyperliquid),
        other => Err(format!(
            "unsupported venue `{other}` (Binance|BinanceFutures|Hyperliquid)"
        )),
    }
}

fn parse_args() -> Result<Args, String> {
    let mut dir: Option<PathBuf> = None;
    let mut ckpt_dir: Option<PathBuf> = None;
    let mut coverage_out: Option<PathBuf> = None;
    let mut venue_str: Option<String> = None;
    let mut symbol: Option<String> = None;
    let mut timeframe_ms: Option<i64> = None;
    let mut bands_str: Option<String> = None;
    let mut window_ms: Option<i64> = None;
    let mut cursor: Option<Cursor> = None;
    let mut depth_cadence_ms: Option<i64> = None;
    let mut print_ckpt_name = false;

    // B1 (M-38b rev4): нормализовать argv ПЕРЕД разбором. `--flag=value` (equals-форма —
    // ровно то, что лежит в `docker-compose.yml command:`) раскладываем в два отдельных
    // элемента `--flag` + `value`. Раздельная форма (как в cron-скрипте) проходит через
    // `vec![a]` без изменений. Тот же подход, что у `journal-retention`.
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
            "--ckpt-dir" => ckpt_dir = Some(PathBuf::from(next()?)),
            "--coverage-out" => coverage_out = Some(PathBuf::from(next()?)),
            "--venue" => venue_str = Some(next()?.to_string()),
            "--symbol" => symbol = Some(next()?.to_string()),
            "--timeframe-ms" => {
                timeframe_ms = Some(
                    next()?
                        .parse::<i64>()
                        .map_err(|e| format!("--timeframe-ms: {e}"))?,
                );
            }
            "--bands" => bands_str = Some(next()?.to_string()),
            "--window-ms" => {
                window_ms = Some(
                    next()?
                        .parse::<i64>()
                        .map_err(|e| format!("--window-ms: {e}"))?,
                );
            }
            "--cursor" => {
                // B1 (rev4): `--cursor LATEST` — прод-дефолт в `docker-compose.yml`.
                let s = next()?;
                cursor = Some(parse_cursor_value(s, "--cursor")?);
            }
            "--depth-cadence-ms" => {
                let s = next()?;
                let parsed = s
                    .trim()
                    .parse::<i64>()
                    .map_err(|e| format!("--depth-cadence-ms: {e}"))?;
                depth_cadence_ms = Some(parsed);
            }
            "--print-ckpt-name" => {
                // M-94: печать имени без открытия журнала (опора гейта деплоя).
                print_ckpt_name = true;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("неизвестный флаг `{other}` (попробуй --help)")),
        }
        i += 2;
    }

    // Дефолты для прод-cadence.
    //
    // M-90 (TD-227): приоритет источника селектора — `--flag` (явная команда) →
    // `GATEWAY_*` env (ТОТ ЖЕ источник, что у `gateway-serve`) → подписанный дефолт.
    let venue = match venue_str.as_deref() {
        Some(s) => parse_venue(s)?,
        None => match env_string("GATEWAY_VENUE")? {
            Some(v) => parse_venue(&v)?,
            None => Venue::Binance,
        },
    };
    let symbol = match symbol {
        Some(s) => s,
        None => match env_string("GATEWAY_SYMBOL")? {
            Some(s) => s,
            None => "BTCUSDT".to_string(),
        },
    };
    let timeframe_ms = match timeframe_ms {
        Some(t) => t,
        None => match env_string("GATEWAY_TIMEFRAME_MS")? {
            Some(s) => s
                .trim()
                .parse::<i64>()
                .map_err(|e| format!("GATEWAY_TIMEFRAME_MS={s:?} не парсится как i64 ({e})"))?,
            None => 1_000,
        },
    };
    let bands: Vec<f64> = match bands_str.as_deref() {
        Some(s) => s
            .split(',')
            .map(|p| p.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("--bands parse: {e}"))?,
        None => match env_string("GATEWAY_BANDS")? {
            Some(s) => s
                .split(',')
                .map(|p| p.trim().parse::<f64>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("GATEWAY_BANDS={s:?} не парсится как f64-список ({e})"))?,
            None => vec![0.001],
        },
    };
    let window_ms = match window_ms {
        Some(0) => None,
        Some(w) => Some(w),
        None => match env_string("GATEWAY_WINDOW_MS")? {
            Some(s) => {
                let v = s
                    .trim()
                    .parse::<i64>()
                    .map_err(|e| format!("GATEWAY_WINDOW_MS={s:?} не парсится как i64 ({e})"))?;
                if v == 0 {
                    None
                } else {
                    Some(v)
                }
            }
            None => Some(60_000_i64),
        },
    };
    Ok(Args {
        dir: dir.unwrap_or_else(|| PathBuf::from("./journal-data")),
        ckpt_dir: ckpt_dir.unwrap_or_else(|| PathBuf::from("./gateway-ckpt")),
        coverage_out: coverage_out
            .unwrap_or_else(|| PathBuf::from("./gateway-ckpt/covered_through_seq")),
        venue,
        symbol,
        timeframe_ms,
        bands,
        window_ms,
        cursor: cursor.unwrap_or(Cursor::LATEST),
        depth_cadence_ms,
        print_ckpt_name,
    })
}

/// M-90 (TD-227): прочитать `GATEWAY_*` из env. Пустая строка в env считается
/// ОТСУТСТВИЕМ и уступает дефолту.
fn env_string(var: &str) -> Result<Option<String>, String> {
    match std::env::var(var) {
        Err(_) => Ok(None),
        Ok(s) if s.is_empty() => Ok(None),
        Ok(s) => Ok(Some(s)),
    }
}

fn parse_cursor_value(s: &str, flag: &str) -> Result<Cursor, String> {
    if s.eq_ignore_ascii_case("LATEST") {
        Ok(Cursor::LATEST)
    } else {
        let seq = s
            .parse::<u64>()
            .map_err(|e| format!("{flag}: {e} (ожидается `LATEST` или u64)"))?;
        Ok(Cursor {
            upto_seq: Some(seq),
        })
    }
}

fn print_help() {
    println!(
        "gateway-checkpoint — операторский снимок чекпоинта редьюсера (M-38b)\n\
         \n\
         Использование:\n  \
           gateway-checkpoint [--dir DIR] [--ckpt-dir DIR] [--coverage-out PATH]\n  \
                              [--venue VENUE] [--symbol STR] [--timeframe-ms N]\n  \
                              [--bands f,f,...] [--window-ms N] [--cursor LATEST|N]\n  \
                              [--depth-cadence-ms N] [--print-ckpt-name]\n\
         \n\
         Обе формы `--flag value` и `--flag=value` принимаются.\n\
         M-90 (TD-227): при ОТСУТСТВИИ флага селектора значение читается из\n  \
         переменной окружения GATEWAY_VENUE / _SYMBOL / _TIMEFRAME_MS / _BANDS /\n  \
         _WINDOW_MS / _DEPTH_CADENCE_MS — единый источник с сервером выдачи.\n  \
         Флаг имеет приоритет; env — следующий; подписанный дефолт — последний.\n\
         \n\
         M-94 (П-032): GATEWAY_CALC_PROFILE=<path> в env переводит бинарь в режим\n  \
         профиля (закрытый набор 8 ключей берётся из файла; наличие ключа\n  \
         профиля или флага оси в окружении ⇒ отказ старта). --print-ckpt-name в\n  \
         режиме профиля печатает имя слепка и выходит 0 без открытия журнала.\n\
         \n\
         Дефолты: --dir=./journal-data --ckpt-dir=./gateway-ckpt\n  \
                  --coverage-out=./gateway-ckpt/covered_through_seq\n  \
                  --venue=Binance --symbol=BTCUSDT --timeframe-ms=1000\n  \
                  --bands=0.001 --window-ms=60000 --cursor=LATEST\n  \
                  --depth-cadence-ms=GATEWAY_DEPTH_CADENCE_MS или 1000\n\
         \n\
         Exit-коды: 0=ok, 1=argv/IO, 2=validate_selector fail-closed (GW-I-10) или \
         невалидная каденция (MD-I-8 d14) или отказ режима профиля."
    );
}

fn main() -> ExitCode {
    init_tracing();

    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("gateway-checkpoint: {e}");
            return ExitCode::from(1);
        }
    };

    // M-94 (задача 2): определить режим профиля по `GATEWAY_CALC_PROFILE`.
    let calc_profile_path = env_string("GATEWAY_CALC_PROFILE")
        .ok()
        .flatten()
        .map(|s| PathBuf::from(s.trim()));
    let profile_mode = calc_profile_path.is_some();

    if profile_mode {
        return run_profile_mode(args, calc_profile_path.as_deref().unwrap());
    }

    run_legacy_mode(args)
}

// ══════════════════════════════════════════════════════════════════════════════
// РЕЖИМ ПРОФИЛЯ (M-94, П-032)
// ══════════════════════════════════════════════════════════════════════════════

/// Режим профиля: `GATEWAY_CALC_PROFILE` задан. Все 8 величин берутся из файла;
/// наличие любого ключа профиля или `GATEWAY_CANONICAL_BANDS` в env ИЛИ любого
/// флага оси (`--bands`/`--timeframe-ms`/`--window-ms`/`--depth-cadence-ms`) в
/// argv ⇒ отказ старта с сообщением, называющим ключ/флаг И слово
/// `GATEWAY_CALC_PROFILE` (милестоун §3.3, оракулы `p3`/`p4`).
fn run_profile_mode(args: Args, profile_path: &Path) -> ExitCode {
    // 1. Проверка env на «наличие — отказ» (`П-032` п.1). ВКЛЮЧАЕТ
    //    `GATEWAY_CANONICAL_BANDS` (третий носитель полос выводится из оборота;
    //    в режиме профиля канонический набор = полосы профиля).
    let env_keys_to_check: &[&str] = &[
        "GATEWAY_BANDS",
        "GATEWAY_CANONICAL_BANDS",
        "GATEWAY_DEPTH_CADENCE_MS",
        "GATEWAY_TIMEFRAME_MS",
        "GATEWAY_WINDOW_MS",
        "GATEWAY_ALLOWED_PROFILES",
        "GATEWAY_VP_BIN_WIDTH_E8",
        "GATEWAY_HEATMAP_WINDOW",
        "CALC_PROFILE_VERSION",
    ];
    for k in env_keys_to_check {
        if std::env::var(k).is_ok() {
            // `.is_ok()` ловит и пустую строку (что эквивалентно отсутствию, см. `env_string`,
            // но в режиме профиля `GATEWAY_CALC_PROFILE` уже задан, и любая попытка
            // ПЕРЕОПРЕДЕЛИТЬ ось через env — отказ, даже пустой строкой: операторский
            // дефолт `${VAR:-…}` в compose всё равно подставит значение, и тогда
            // `.is_some()` сработает).
            eprintln!(
                "gateway-checkpoint: {k} must not be set in env when \
                 GATEWAY_CALC_PROFILE={} is set (П-032 п.1: «наличие — отказ»); \
                 значение берётся ИЗ профиля {}",
                profile_path.display(),
                profile_path.display()
            );
            return ExitCode::from(2);
        }
    }

    // 2. Проверка argv на флаги оси (`p4`). Поддерживаем ОБЕ формы.
    let argv_strings: Vec<String> = std::env::args().skip(1).collect();
    for a in &argv_strings {
        for f in PROFILE_AXIS_FLAGS {
            if a == f || a.starts_with(&format!("{f}=")) {
                eprintln!(
                    "gateway-checkpoint: {f} must not be set on argv when \
                     GATEWAY_CALC_PROFILE={} is set (П-032 п.1: «наличие — отказ», \
                     определение оси — в профиле)",
                    profile_path.display()
                );
                return ExitCode::from(2);
            }
        }
    }

    // 3. Загрузить профиль (закрытый набор 8 ключей, sha256 от байтов, проверка тройки).
    let profile = match calc_profile::load_profile(profile_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "gateway-checkpoint: GATEWAY_CALC_PROFILE={} invalid: {e}",
                profile_path.display()
            );
            return ExitCode::from(2);
        }
    };

    // 4. `--print-ckpt-name` — печать имени БЕЗ открытия журнала (опора гейта деплоя,
    //    спека §3.7, оракул `p7`). На несуществующем `--dir`/`--ckpt-dir` ДОЛЖЕН
    //    работать (замоканные пути) — `ckpt_path_for` берёт имя от отпечатка, не
    //    от файла.
    if args.print_ckpt_name {
        let sel = selector_from_profile(&profile);
        let path = checkpoint::ckpt_path_for_pub(&args.ckpt_dir, &sel);
        match path.file_name() {
            Some(name) => {
                println!("{}", name.to_string_lossy());
                return ExitCode::SUCCESS;
            }
            None => {
                eprintln!(
                    "gateway-checkpoint: --print-ckpt-name: cannot derive filename from {}",
                    path.display()
                );
                return ExitCode::from(1);
            }
        }
    }

    // 5. Собрать `Selector` из профиля + дефолты `venue`/`symbol` (не в профиле —
    //    `П-032` §3.1, ровно 8 ключей).
    let selector = selector_from_profile(&profile);

    // 6. M-47 (GW-I-10, TD-046): fail-closed гвард. Дублирует `validate_selector`
    //    в библиотеке — здесь он СРАЗУ ЖЕ после сборки, до `advance_to`, чтобы
    //    `ckpt_path_for` дал корректное имя.
    if selector.timeframe_ms <= 0 || 86_400_000 % selector.timeframe_ms != 0 {
        eprintln!(
            "gateway-checkpoint: GATEWAY_TIMEFRAME_MS={} (из профиля) не выравнен на границу \
             UTC-суток (требуется > 0 и 86_400_000 % timeframe_ms == 0)",
            selector.timeframe_ms
        );
        return ExitCode::from(2);
    }
    if selector.depth_cadence_ms.unwrap_or(0) < 1000
        || 86_400_000 % selector.depth_cadence_ms.unwrap_or(0) != 0
    {
        eprintln!(
            "gateway-checkpoint: GATEWAY_DEPTH_CADENCE_MS={} (из профиля) невалидно: \
             требуется >= 1000 и выравнено на границу UTC-суток",
            selector.depth_cadence_ms.unwrap_or(0)
        );
        return ExitCode::from(2);
    }

    // 7. NaN guard (M-38b): фингерпринт селектора использует `to_bits()`; NaN != NaN.
    if selector.bands.iter().any(|b| b.is_nan()) {
        eprintln!("gateway-checkpoint: bands из профиля содержат NaN — фингерпринт нестабилен.");
        return ExitCode::from(2);
    }

    // 8. M-94 §3.6 (б) / `p6` / `P-032` п.4: записать `<слепок>.profile` с
    //    `version` и `sha256` БАЙТОВ файла, АТОМАРНО (`.tmp` → `rename`), ПОСЛЕ
    //    успешной записи слепка. Проверка `p6`: на неуспешном прогоне
    //    `.profile` НЕ создаётся.
    let achieved_cursor = match checkpoint::advance_to(
        &args.dir,
        &args.ckpt_dir,
        &selector,
        EpochFilter::OwnCaptureOnly,
        args.cursor,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "gateway-checkpoint: advance_to failed dir={} ckpt={} err={e}",
                args.dir.display(),
                args.ckpt_dir.display()
            );
            return ExitCode::from(1);
        }
    };
    let covered_through_seq = match achieved_cursor.upto_seq {
        Some(s) => s,
        None => {
            eprintln!(
                "gateway-checkpoint: advance_to вернул пустой курсор (журнал пуст или префикс \
                 уже спрунен); публикую `covered=0` (fail-closed)"
            );
            0
        }
    };

    if let Err(e) = write_coverage_artifact(&args.coverage_out, covered_through_seq) {
        eprintln!(
            "gateway-checkpoint: не удалось записать артефакт покрытия {}: {e}",
            args.coverage_out.display()
        );
        return ExitCode::from(1);
    }

    // `.profile` — ПОСЛЕ `advance_to` И покрытия, иначе нарушается «по какому
    // определению посчитан ЭТОТ слепок» (спека §3.6): без `.profile` репродьюсер
    // не знает, каким файлом профиля руководствоваться.
    if let Err(e) = write_profile_sidecar(&args.ckpt_dir, &selector, &profile) {
        eprintln!(
            "gateway-checkpoint: не удалось записать .profile рядом со слепком (ckpt_dir={}): {e}",
            args.ckpt_dir.display()
        );
        return ExitCode::from(1);
    }

    eprintln!(
        "gateway-checkpoint: ok dir={} ckpt={} profile={} version={} sha256={} \
         covered={} out={}",
        args.dir.display(),
        args.ckpt_dir.display(),
        profile_path.display(),
        profile.version,
        profile.sha256,
        covered_through_seq,
        args.coverage_out.display()
    );
    ExitCode::SUCCESS
}

/// Собрать `Selector` из `AppliedProfile`. `venue`/`symbol` — НЕ в профиле
/// (`П-032` §3.1, ровно 8 ключей), поэтому подставляются дефолты — `Binance` /
/// `BTCUSDT`. Тот же селектор, что собирает тест (`selector_from_profile` в
/// `red_m94_calc_profile_warmer.rs`), чтобы `selector_fingerprint` у прод-пути
/// и у оракула совпали.
fn selector_from_profile(p: &AppliedProfile) -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: p.timeframe_ms,
        bands: p.bands.clone(),
        window_ms: Some(p.window_ms),
        depth_cadence_ms: Some(p.depth_cadence_ms),
    }
}

/// Atomic write `ckpt-<fp>.bin.profile` (tmp + rename, уникальное имя tmp) —
/// отчёт о ПРОЧИТАННОМ файле профиля. Ошибка записи — НЕ критична для самого
/// слепка (он уже на диске), но ДОЛЖНА прерывать деплой: без `.profile` нет
/// способа восстановить «по какому определению» (милестоун §3.6).
fn write_profile_sidecar(
    ckpt_dir: &Path,
    sel: &Selector,
    profile: &AppliedProfile,
) -> std::io::Result<()> {
    use std::time::{SystemTime, UNIX_EPOCH};
    std::fs::create_dir_all(ckpt_dir)?;
    let ckpt_path = checkpoint::ckpt_path_for_pub(ckpt_dir, sel);
    let sidecar = match ckpt_path.file_name() {
        Some(name) => ckpt_dir.join(format!("{}.profile", name.to_string_lossy())),
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "ckpt_path has no filename",
            ));
        }
    };
    let body = format!(
        "{{\"version\":{},\"sha256\":\"{}\"}}",
        profile.version, profile.sha256
    );
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut tmp = sidecar.clone();
    let new_ext = format!("tmp.{pid}.{nanos}");
    tmp.set_extension(&new_ext);
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &sidecar)?;
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// LEGACY-РЕЖИМ (M-90 и старше; GATEWAY_CALC_PROFILE НЕ задан)
// ══════════════════════════════════════════════════════════════════════════════

fn run_legacy_mode(args: Args) -> ExitCode {
    // GW-I-10 (M-47, TD-046): fail-closed гвард на СТАРТЕ прод-бинаря.
    let timeframe_ms = args.timeframe_ms;
    if timeframe_ms <= 0 || 86_400_000 % timeframe_ms != 0 {
        eprintln!(
            "gateway-checkpoint: GATEWAY_TIMEFRAME_MS={timeframe_ms} не выравнен на границу \
             UTC-суток (требуется > 0 и 86_400_000 % timeframe_ms == 0)"
        );
        return ExitCode::from(2);
    }

    // M-68 задача 23 (R-141 Б-1): источник каденции — env GATEWAY_DEPTH_CADENCE_MS
    // (тот же, что у `gateway-serve`).
    const DEFAULT_CADENCE_MS: i64 = 1_000;
    let cadence_from_env: Option<i64> = match std::env::var("GATEWAY_DEPTH_CADENCE_MS") {
        Err(_) => None,
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Some(DEFAULT_CADENCE_MS)
            } else {
                match trimmed.parse::<i64>() {
                    Ok(ms) => Some(ms),
                    Err(e) => {
                        eprintln!(
                            "gateway-checkpoint: GATEWAY_DEPTH_CADENCE_MS={trimmed:?} \
                             не парсится как i64 ({e})"
                        );
                        return ExitCode::from(2);
                    }
                }
            }
        }
    };
    let cadence_raw: i64 = match args.depth_cadence_ms.or(cadence_from_env) {
        Some(ms) => ms,
        None => DEFAULT_CADENCE_MS,
    };
    if cadence_raw < 1000 || 86_400_000 % cadence_raw != 0 {
        eprintln!(
            "gateway-checkpoint: GATEWAY_DEPTH_CADENCE_MS={cadence_raw} невалидно: требуется \
             >= 1000 и выравнено на границу UTC-суток"
        );
        return ExitCode::from(2);
    }

    let selector = Selector {
        venue: args.venue,
        symbol: args.symbol,
        timeframe_ms: args.timeframe_ms,
        bands: args.bands,
        window_ms: args.window_ms,
        depth_cadence_ms: Some(cadence_raw),
    };

    if selector.bands.iter().any(|b| b.is_nan()) {
        eprintln!("gateway-checkpoint: bands содержат NaN — фингерпринт нестабилен.");
        return ExitCode::from(2);
    }

    let achieved_cursor = match checkpoint::advance_to(
        &args.dir,
        &args.ckpt_dir,
        &selector,
        EpochFilter::OwnCaptureOnly,
        args.cursor,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "gateway-checkpoint: advance_to failed dir={} ckpt={} err={e}",
                args.dir.display(),
                args.ckpt_dir.display()
            );
            return ExitCode::from(1);
        }
    };
    let covered_through_seq = match achieved_cursor.upto_seq {
        Some(s) => s,
        None => {
            eprintln!(
                "gateway-checkpoint: advance_to вернул пустой курсор (журнал пуст или префикс \
                 уже спрунен); публикую `covered=0` (fail-closed)"
            );
            0
        }
    };

    if let Err(e) = write_coverage_artifact(&args.coverage_out, covered_through_seq) {
        eprintln!(
            "gateway-checkpoint: не удалось записать артефакт покрытия {}: {e}",
            args.coverage_out.display()
        );
        return ExitCode::from(1);
    }

    eprintln!(
        "gateway-checkpoint: ok dir={} ckpt={} requested_cursor={:?} achieved_cursor={:?} \
         covered={} out={}",
        args.dir.display(),
        args.ckpt_dir.display(),
        args.cursor,
        achieved_cursor,
        covered_through_seq,
        args.coverage_out.display()
    );
    ExitCode::SUCCESS
}

/// Atomic write (tmp + rename) с УНИКАЛЬНЫМ именем tmp: защищает от полу-записанного
/// артефакта покрытия и от гонки двух writers на одном `tmp` (RN-22 in depth).
fn write_coverage_artifact(path: &Path, seq: u64) -> std::io::Result<()> {
    use std::time::{SystemTime, UNIX_EPOCH};
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut tmp = path.to_path_buf();
    let new_ext = format!("tmp.{pid}.{nanos}");
    tmp.set_extension(&new_ext);
    std::fs::write(&tmp, seq.to_string())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}

// Compile-time check: тип импортируется, чтобы компилятор не выкинул его за неиспользованием
// (пригодится в следующих задачах M-94, когда `gateway-serve` начнёт маппить
// `RawLiveProfile` в свой `LiveProfile`).
#[allow(dead_code)]
fn _ensure_rawliveprofile_path(_: RawLiveProfile) {}
