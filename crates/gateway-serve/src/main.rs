//! gateway-serve bin — WS-транспорт кокпита (M-28, D1/D6). Тонкая обёртка над `gateway_serve::server`.
//!
//! engine-dev (task #4): собрать `ServeConfig` из env (`GATEWAY_ADDR` / `GATEWAY_JOURNAL_DIR` /
//! `GATEWAY_VENUE` / `GATEWAY_SYMBOL` / `GATEWAY_TIMEFRAME_MS` / `GATEWAY_BANDS` /
//! `GATEWAY_JWT_SECRET`), поднять tokio-runtime → `server::bind(cfg).await` → `server.serve().await`.
//! Read-only, stateless по юзеру: JWT-секрет берётся из env (shared с Next.js-подписателем,
//! D6), без user-БД.

use std::path::PathBuf;
use std::process::ExitCode;

use contracts::Venue;
use gateway_serve::server::{bind, ServeConfig};
use journal::EpochFilter;
use jsonwebtoken::DecodingKey;
use tokio::runtime::Builder;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    init_tracing();

    let cfg = match build_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("gateway-serve: config error: {e}");
            return ExitCode::from(2);
        }
    };

    let bind_addr = cfg.addr.clone();
    let server = match bind(cfg).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("gateway-serve: bind failed addr={bind_addr} error={e}");
            return ExitCode::from(3);
        }
    };

    let actual = server.local_addr();
    eprintln!("gateway-serve: listening on {actual} (read-only, JWT-auth)");

    if let Err(e) = server.serve().await {
        eprintln!("gateway-serve: serve loop ended with error: {e}");
        return ExitCode::from(4);
    }
    ExitCode::SUCCESS
}

/// `init_tracing` — best-effort подписка на `RUST_LOG` (дефолт `info`). Если уже инициализирован
/// (тест-окружение) — игнор. Никаких panic'ов: tracing — observability, не safety-инвариант.
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,gateway_serve=debug".into()),
        )
        .try_init();
}

/// Построить `ServeConfig` из env. Ошибка → `Err(String)` (бинарь печатает и выходит с кодом 2).
///
/// Переменные:
/// - `GATEWAY_JWT_SECRET`  — ОБЯЗАТЕЛЬНА (HS256, общий секрет с Next.js, D6).
/// - `GATEWAY_ADDR`        — дефолт `127.0.0.1:8080` (loopback; внешний bind — conscious choice).
/// - `GATEWAY_JOURNAL_DIR` — дефолт `./journal-data`.
/// - `GATEWAY_VENUE`       — дефолт `Binance`.
/// - `GATEWAY_SYMBOL`      — дефолт `BTCUSDT`.
/// - `GATEWAY_TIMEFRAME_MS`— дефолт `1000`.
/// - `GATEWAY_BANDS`       — comma-separated float'ы, дефолт `0.001`.
fn build_config() -> Result<ServeConfig, String> {
    let secret = std::env::var("GATEWAY_JWT_SECRET")
        .map_err(|_| "GATEWAY_JWT_SECRET must be set (HS256 shared secret)".to_string())?;
    if secret.trim().is_empty() {
        return Err("GATEWAY_JWT_SECRET must not be empty".to_string());
    }

    let addr = std::env::var("GATEWAY_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    let journal_dir = std::env::var("GATEWAY_JOURNAL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./journal-data"));

    let venue = match std::env::var("GATEWAY_VENUE")
        .unwrap_or_else(|_| "Binance".to_string())
        .as_str()
    {
        "Binance" => Venue::Binance,
        "BinanceFutures" => Venue::BinanceFutures,
        "Hyperliquid" => Venue::Hyperliquid,
        other => return Err(format!("unsupported GATEWAY_VENUE={other}")),
    };

    let symbol = std::env::var("GATEWAY_SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string());

    let timeframe_ms: i64 = std::env::var("GATEWAY_TIMEFRAME_MS")
        .unwrap_or_else(|_| "1000".to_string())
        .parse()
        .map_err(|e| format!("GATEWAY_TIMEFRAME_MS parse: {e}"))?;

    let bands: Vec<f64> = std::env::var("GATEWAY_BANDS")
        .unwrap_or_else(|_| "0.001".to_string())
        .split(',')
        .map(|s| s.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("GATEWAY_BANDS parse: {e}"))?;

    Ok(ServeConfig {
        addr,
        journal_dir,
        filter: EpochFilter::OwnCaptureOnly,
        selector: gateway_serve::build_selector(venue, symbol, timeframe_ms, bands),
        decoding_key: DecodingKey::from_secret(secret.as_bytes()),
    })
}

// Hook для будущего multi-thread runtime, если профайл нагрузки покажет нужду (на текущем
// этапе хватает `current_thread` — каждое соединение — отдельный `tokio::spawn`-таск).
#[allow(dead_code)]
fn _runtime_hook() -> tokio::runtime::Runtime {
    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("gateway-serve")
        .build()
        .expect("build tokio runtime")
}
