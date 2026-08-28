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
//!
//! Обе формы `--flag value` И `--flag=value` принимаются наравне: compose пишет
//! `--flag=value`, cron-обёртка может писать через пробел. Соседний `journal-retention`
//! уже так работает; здесь — единый контракт.
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
    /// M-68 задача 23 (R-141 Б-1): каденция депт-серии, прочитанная из флага
    /// `--depth-cadence-ms` или env `GATEWAY_DEPTH_CADENCE_MS` (тот же источник, что у
    /// `gateway-serve::serve_config_from_env`, чтобы отпечаток селектора у писателя и
    /// читателя чекпоинта СОВПАЛ). `None` означает «не задано ни флагом, ни env» —
    /// дефолт 1000 мс подставляется ниже по тем же правилам, что и в `serve_config_from_env`.
    depth_cadence_ms: Option<i64>,
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
    // M-68 задача 23 (R-141 Б-1): CLI-флаг `--depth-cadence-ms`. Источник ЗНАЧЕНИЯ —
    // `--depth-cadence-ms` (явный приоритет оператора в cron-обёртке), при отсутствии —
    // env `GATEWAY_DEPTH_CADENCE_MS` (тот же источник, что у `gateway-serve`), при
    // отсутствии — подписанный дефолт 1000 мс. Та же логика, что в
    // `gateway-serve::serve_config_from_env` (A-015 §3 п.1: поведение прода не имеет
    // права зависеть от того, КАК забыли задать).
    let mut depth_cadence_ms: Option<i64> = None;

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
                // Раньше парсер звал `parse::<u64>()` и рейзил `invalid digit found in string`
                // (замер architect'а: `--cursor LATEST` exit=1). Также принимаем числовой
                // вариант для `--cursor <i64>` — операторские инкрементальные снапшоты.
                let s = next()?;
                cursor = Some(parse_cursor_value(s, "--cursor")?);
            }
            "--depth-cadence-ms" => {
                // M-68 задача 23 (R-141 Б-1): CLI-флаг каденции. Парсим как i64,
                // валидация (>= 1000, делит 86_400_000) — ниже, рядом с проверкой
                // `timeframe_ms` (тот же класс fail-closed, что GW-I-14). Невалидное
                // значение ⇒ exit 2 с именем флага (сообщение называет
                // GATEWAY_DEPTH_CADENCE_MS, чтобы оператор видел, ГДЕ править).
                let s = next()?;
                let parsed = s
                    .trim()
                    .parse::<i64>()
                    .map_err(|e| format!("--depth-cadence-ms: {e}"))?;
                depth_cadence_ms = Some(parsed);
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
        depth_cadence_ms,
    })
}

/// Разбор значения `--cursor`. Принимает `LATEST` (= до конца журнала) ИЛИ `u64`.
/// `LATEST` — прод-дефолт в compose; числовое значение — операторский инкрементальный
/// прогон в cron-обёртке.
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
                              [--depth-cadence-ms N]\n\
         \n\
         Обе формы `--flag value` и `--flag=value` принимаются.\n\
         Дефолты: --dir=./journal-data --ckpt-dir=./gateway-ckpt\n  \
                  --coverage-out=./gateway-ckpt/covered_through_seq\n  \
                  --venue=Binance --symbol=BTCUSDT --timeframe-ms=1000\n  \
                  --bands=0.001 --window-ms=60000 --cursor=LATEST\n  \
                  --depth-cadence-ms=GATEWAY_DEPTH_CADENCE_MS или 1000\n\
         \n\
         Exit-коды: 0=ok, 1=argv/IO, 2=validate_selector fail-closed (GW-I-10) или \
         невалидная каденция (MD-I-8 d14)."
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

    // M-68 задача 23 (R-141 Б-1): источник каденции — env GATEWAY_DEPTH_CADENCE_MS (тот же,
    // что у `gateway-serve`), если флаг `--depth-cadence-ms` НЕ задан. Политика пустого/
    // пробельного/отсутствия — ОДИН исход: подписанный дефолт 1000 мс. Основание —
    // A-015 §3 п.1, подтверждённое A-026 §1 для соседней ручки `gateway-serve`.
    //
    // Приоритет: `--depth-cadence-ms` (явная команда оператора в cron-обёртке) →
    // `GATEWAY_DEPTH_CADENCE_MS` (env, общее с `gateway-serve`) → дефолт 1000. При
    // СОВПАДЕНИИ источников отпечаток селектора у писателя и читателя чекпоинта одинаков
    // (та же логика, что и в `serve_config_from_env`), и чекпоинт читается тёплым
    // стартом, а не реплеится целиком.
    const DEFAULT_CADENCE_MS: i64 = 1_000;
    // R-147 N-1 (SYMMETRY OF BEHAVIOR): писатель чекпоинта и читатель
    // (`gateway-serve`) делят ОДНУ переменную `GATEWAY_DEPTH_CADENCE_MS`, и её
    // значение входит в `selector_fingerprint` — имя файла слепка. Оператор с
    // опечаткой в compose ПОЛУЧАЛ ранее писателя на молчаливом дефолте 1000
    // (parse через `.ok()` глотал ошибку) и читателя, отказавшегося стартовать
    // (`serve_config_from_env` отвергает мусор). Расхождение ПОВЕДЕНИЯ на одном
    // входе хуже обоих чистых исходов, и обе стороны обязаны мусор ОТВЕРГАТЬ.
    //
    // Поведение на ОТСУТСТВИИ и ПУСТОЙ строке (после `trim`) — НЕ ТРОГАЕМ: это
    // `A-015` §3 п.1 (отдельное решение founder'а), один исход — подписанный
    // дефолт 1000 мс. Меняется поведение ТОЛЬКО на невалидном ЗНАЧЕНИИ.
    //
    // Сообщение взято с соседней `serve_config_from_env` (crates/gateway-serve/
    // src/lib.rs) ДОСЛОВНО — оператор видит ТАКУЮ ЖЕ формулировку от обоих
    // процессов, и путь к фиксу не расходится по названию переменной.
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
                             не парсится как i64 ({e}) — опечатка в `.env` (мусор/\
                             суффикс/научная нотация/дробное/переполнение), а не \
                             сигнал к дефолту; оператор обязан задать валидное \
                             значение или unset/пусто/пробельное для дефолта"
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

    // M-68 задача 23 (R-141 Б-1) + задача 17 (d14): валидация каденции по тем же
    // правилам, что и в `gateway::validate_selector` (ms >= 1000, делит 86_400_000).
    // Невалидное (мусор, 0, -1, 999, ...) ⇒ exit 2 с сообщением, называющим
    // GATEWAY_DEPTH_CADENCE_MS (класс GW-I-14).
    if cadence_raw < 1000 || 86_400_000 % cadence_raw != 0 {
        eprintln!(
            "gateway-checkpoint: GATEWAY_DEPTH_CADENCE_MS={cadence_raw} невалидно: требуется \
             >= 1000 и выравнено на границу UTC-суток (86_400_000 % ms == 0); подсекундные \
             или нестандартные значения дают схлопывание ключей (тот же класс, что GW-I-14 \
             для window_ms; см. MD-I-8 d14, C-167)"
        );
        return ExitCode::from(2);
    }

    let selector = Selector {
        venue: args.venue,
        symbol: args.symbol,
        timeframe_ms: args.timeframe_ms,
        bands: args.bands,
        window_ms: args.window_ms,
        // M-68 задача 23 (R-141 Б-1): здесь Some(cadence_raw), не None — иначе
        // отпечаток селектора у писателя и читателя чекпоинта расходится, слепок не
        // находится, журнал читается целиком при каждом подключении (TD-044/R-029:
        // сотни секунд). `selector_fingerprint` (`crates/gateway/src/lib.rs:2825`)
        // включает `depth_cadence_ms`, и `Some(1000)` ≠ `None` ⇒ разные имена
        // файлов. Проводка ручки — задача 23; здесь только значение, прочитанное из
        // того же env, что у `gateway-serve`.
        depth_cadence_ms: Some(cadence_raw),
    };

    // NaN guard (M-38b): фингерпринт селектора использует `to_bits()`; NaN != NaN,
    // поэтому фингерпринт нестабилен. Здесь же валидируем, чтобы прод-cadence не
    // зацикливался на NaN-bands.
    if selector.bands.iter().any(|b| b.is_nan()) {
        eprintln!("gateway-checkpoint: bands содержат NaN — фингерпринт нестабилен (C-030 R1).");
        return ExitCode::from(2);
    }

    // B2 (M-38b rev4): `checkpoint::advance_to` теперь возвращает ДОСТИГНУТЫЙ `Cursor`.
    // Публикуем именно его как `covered_through_seq`. CLI-аргумент (особенно `--cursor LATEST`
    // → `Cursor::LATEST` → `upto_seq = None`) НЕ используется для артефакта.
    //
    // До фикса публикация была `args.cursor.upto_seq.unwrap_or(u64::MAX)`. При
    // `--cursor=LATEST` (прод-дефолт) в артефакт уходил `u64::MAX`, и гейт retention
    // `last_seq(seg) <= covered` пропускал ВСЁ. Строгая связка C-030 R1 становилась
    // no-op, причём МОЛЧА (`pruned_without_checkpoint_coverage` заполнялся только по
    // ветке override). Инверсия всех трёх условий C-031, на которых принят escape-hatch.
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
            // Курсор `Cursor::START` после advance. Журнал был пуст ИЛИ все события
            // упрунены ДО видимой части. Публикуем «0» как явное «ничего не свёрнуто»:
            // гейт retention тогда откажется прунить вообще ничего (fail-closed).
            // Раньше здесь стоял `u64::MAX` (B2), что делало строгую связку no-op.
            eprintln!(
                "gateway-checkpoint: advance_to вернул пустой курсор (журнал пуст или префикс \
                 уже спрунен); публикую `covered=0` (fail-closed): retention не прунит ничего"
            );
            0
        }
    };

    // Пишем артефакт покрытия только при успешном advance. Multi-selector deployment —
    // через деплой N инстансов на разные селекторы и ops-скрипт берёт min.
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
fn write_coverage_artifact(path: &std::path::Path, seq: u64) -> std::io::Result<()> {
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
