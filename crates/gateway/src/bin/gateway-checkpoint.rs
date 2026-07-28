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
//! | `--cursor <i64>` | курсор для advance_to (None = LATEST) | `LATEST` |
//!
//! ## Exit-коды
//!
//! - `0` — успех (чекпоинт снят + артефакт покрытия записан).
//! - `1` — неверные аргументы / I/O.
//! - `2` — `validate_selector` отверг селектор (GW-I-10 fail-closed).
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

use std::path::PathBuf;
use std::process::ExitCode;

use gateway::checkpoint;
use gateway::{Cursor, Selector};
use journal::EpochFilter;

#[derive(Debug)]
struct Args {
    dir: PathBuf,
    ckpt_dir: PathBuf,
    coverage_out: PathBuf,
    venue: contracts::Venue,
    symbol: String,
    timeframe_ms: i64,
    bands: Vec<f64>,
    window_ms: Option<i64>,
    cursor: Cursor,
}

fn parse_venue(s: &str) -> Result<contracts::Venue, String> {
    match s {
        "Binance" => Ok(contracts::Venue::Binance),
        "BinanceFutures" => Ok(contracts::Venue::BinanceFutures),
        "Hyperliquid" => Ok(contracts::Venue::Hyperliquid),
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
                let s = next()?;
                let seq = s.parse::<u64>().map_err(|e| format!("--cursor: {e}"))?;
                cursor = Some(Cursor {
                    upto_seq: Some(seq),
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

    // Дефолты для прод-cadence.
    let venue = match venue_str.as_deref() {
        Some(s) => parse_venue(s)?,
        None => contracts::Venue::Binance,
    };
    let bands: Vec<f64> = match bands_str.as_deref() {
        Some(s) => s
            .split(',')
            .map(|p| p.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("--bands parse: {e}"))?,
        None => vec![0.001],
    };
    // M-37: bounded-window по умолчанию = Some(60_000). 0 ⇒ None (offline unbounded).
    let window_ms = match window_ms {
        None => Some(60_000_i64),
        Some(0) => None,
        Some(w) => Some(w),
    };
    Ok(Args {
        dir: dir.unwrap_or_else(|| PathBuf::from("./journal-data")),
        ckpt_dir: ckpt_dir.unwrap_or_else(|| PathBuf::from("./gateway-ckpt")),
        coverage_out: coverage_out
            .unwrap_or_else(|| PathBuf::from("./gateway-ckpt/covered_through_seq")),
        venue,
        symbol: symbol.unwrap_or_else(|| "BTCUSDT".to_string()),
        timeframe_ms: timeframe_ms.unwrap_or(1_000),
        bands,
        window_ms,
        cursor: cursor.unwrap_or(Cursor::LATEST),
    })
}

fn print_help() {
    println!(
        "gateway-checkpoint — операторский снимок чекпоинта редьюсера (M-38b)\n\
         \n\
         Использование:\n  \
           gateway-checkpoint [--dir DIR] [--ckpt-dir DIR] [--coverage-out PATH]\n  \
                              [--venue VENUE] [--symbol STR] [--timeframe-ms N]\n  \
                              [--bands f,f,...] [--window-ms N] [--cursor N]\n\
         \n\
         Дефолты: --dir=./journal-data --ckpt-dir=./gateway-ckpt\n  \
                  --coverage-out=./gateway-ckpt/covered_through_seq\n  \
                  --venue=Binance --symbol=BTCUSDT --timeframe-ms=1000\n  \
                  --bands=0.001 --window-ms=60000 --cursor=LATEST\n\
         \n\
         Exit-коды: 0=ok, 1=argv/IO, 2=validate_selector fail-closed (GW-I-10)."
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

    // GW-I-10 (M-47, TD-046): fail-closed гвард на СТАРТЕ прод-бинаря. Без него оператор с
    // опечаткой поднимет ЗДОРОВЫЙ по healthcheck контейнер, отдающий ошибку каждому
    // клиенту (§8 eyes-on увидит (healthy), а кокпит будет пуст).
    let timeframe_ms = args.timeframe_ms;
    if timeframe_ms <= 0 || 86_400_000 % timeframe_ms != 0 {
        eprintln!(
            "gateway-checkpoint: GATEWAY_TIMEFRAME_MS={timeframe_ms} не выравнен на границу \
             UTC-суток (требуется > 0 и 86_400_000 % timeframe_ms == 0; иначе бакет пересекает \
             00:00 UTC ⇒ session_id бакета не определён)"
        );
        return ExitCode::from(2);
    }

    let selector = Selector {
        venue: args.venue,
        symbol: args.symbol,
        timeframe_ms: args.timeframe_ms,
        bands: args.bands,
        window_ms: args.window_ms,
    };

    // NaN guard (M-38b): фингерпринт селектора использует `to_bits()`; NaN != NaN,
    // поэтому фингерпринт нестабилен. Здесь же валидируем, чтобы прод-cadence не
    // зацикливался на NaN-bands.
    if selector.bands.iter().any(|b| b.is_nan()) {
        eprintln!("gateway-checkpoint: bands содержат NaN — фингерпринт нестабилен (C-030 R1).");
        return ExitCode::from(2);
    }

    // Снять чекпоинт через `checkpoint::advance_to`. Любой I/O / GW-I-9 сбой →
    // ExitCode 1 (оператор узнает). Артефакт покрытия пишем ТОЛЬКО при успехе
    // (см. safety note в шапке файла).
    if let Err(e) = checkpoint::advance_to(
        &args.dir,
        &args.ckpt_dir,
        &selector,
        EpochFilter::OwnCaptureOnly,
        args.cursor,
    ) {
        eprintln!(
            "gateway-checkpoint: advance_to failed dir={} ckpt={} err={e}",
            args.dir.display(),
            args.ckpt_dir.display()
        );
        return ExitCode::from(1);
    }

    // M-38b (C-030 R1, rev2 NOTE): артефакт покрытия `covered_through_seq`.
    // Значение = `cursor.upto_seq` (то, что свёрнуто). Multi-selector deployment
    // пишет минимум по всем селекторам в один файл (формат — число в ASCII).
    // MVP пишет для ОДНОГО селектора; multi-selector — TODO для прод-расширения
    // (через деплой с N инстансами gateway-checkpoint на разные селекторы и
    // ops-скрипт берёт min по всем файлам).
    let covered_through_seq = match args.cursor.upto_seq {
        Some(seq) => seq,
        None => u64::MAX, // LATEST = всё свёрнуто. Прод-конвенция: MAX означает «до конца».
    };
    if let Err(e) = write_coverage_artifact(&args.coverage_out, covered_through_seq) {
        eprintln!(
            "gateway-checkpoint: не удалось записать артефакт покрытия {}: {e}",
            args.coverage_out.display()
        );
        return ExitCode::from(1);
    }

    eprintln!(
        "gateway-checkpoint: ok dir={} ckpt={} cursor={:?} covered={} out={}",
        args.dir.display(),
        args.ckpt_dir.display(),
        args.cursor,
        covered_through_seq,
        args.coverage_out.display()
    );
    ExitCode::SUCCESS
}

/// Atomic write (tmp + rename): защищает от полу-записанного артефакта покрытия.
fn write_coverage_artifact(path: &std::path::Path, seq: u64) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, seq.to_string())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Best-effort tracing init: `RUST_LOG=info` покажет дополнительный контекст, но
/// без env — тихо. Не критичный путь (observability), не паникуем.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}
