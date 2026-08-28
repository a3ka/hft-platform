//! gateway-serve bin — WS-транспорт кокпита (M-28, D1/D6). Тонкая обёртка над `gateway_serve::server`.
//!
//! **M-37 task #7a:** main — ТОНКИЙ вызыватель `serve_config_from_env(|k| std::env::var(k).ok())`.
//! Сама логика сборки `ServeConfig` из env (`GATEWAY_ADDR` / `GATEWAY_JOURNAL_DIR` /
//! `GATEWAY_VENUE` / `GATEWAY_SYMBOL` / `GATEWAY_TIMEFRAME_MS` / `GATEWAY_BANDS` /
//! `GATEWAY_JWT_SECRET` / **`GATEWAY_WINDOW_MS`**) живёт в `gateway_serve::serve_config_from_env`
//! (тестируемая чистая функция с инжектируемым getter'ом env). Анти-TD-020: env→Selector.window_ms
//! доказуем на unit-уровне (`red_serve_window_wiring`), а не только §8 глазами на VPS.
//!
//! Read-only, stateless по юзеру: JWT-секрет берётся из env (shared с Next.js-подписателем,
//! D6), без user-БД.

use std::process::ExitCode;

use gateway_serve::server::bind;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    init_tracing();

    let cfg = match gateway_serve::serve_config_from_env(|k| std::env::var(k).ok()) {
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

#[allow(dead_code)]
fn _runtime_hook() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("gateway-serve")
        .build()
        .expect("build tokio runtime")
}
