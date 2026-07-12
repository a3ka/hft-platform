//! recorder — вход даталеера. venue-адаптеры (Binance + Hyperliquid + BinanceFutures) →
//! mpsc-канал (EventKind) → журнал (ЕДИНСТВЕННЫЙ писатель, seq тотальный порядок).
//! docs/fa/{venues,journal}.md.
//!
//! Площадки: спавн итерацией по `recorder::default_venues()` (config-driven, не хардкод).
//! Конфиг через env: JOURNAL_DIR, BINANCE_SYMBOLS / HL_COINS / BINANCE_FUTURES_SYMBOLS
//! (csv, defaults BTCUSDT,ETHUSDT / BTC,ETH / BTCUSDT,ETHUSDT). Reconnect + TD-013 backoff —
//! внутри venue::run; здесь supervisor + spawn-цикл.
//!
//! M-05 (engine-dev): SIGTERM/SIGINT → `shutdown` future → `run_writer` дренит mpsc +
//! flush перед exit (J1 — clean-shutdown).
//! M-06 #4 (reland, post-TD-013): подключён Venue::BinanceFutures — funding-breadth C5 вход.
//! MD-only → risk-critic НЕ нужен (gates.md §5 N4).

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use contracts::{EventKind, SysEvent, Venue};
use journal::Journal;
use tokio::sync::mpsc;

fn env_csv(key: &str, default: &[&str]) -> Vec<String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.split(',').map(|s| s.trim().to_string()).collect(),
        _ => default.iter().map(|s| s.to_string()).collect(),
    }
}

/// Supervisor: гоняет venue::run в цикле с exp-backoff (fail-closed к «нет данных», не паника
/// процесса). ConnDown фиксируется в журнале через канал (единый путь к писателю). ConnUp
/// эмитит сам venue::run при успешном коннекте.
async fn supervise<F, Fut>(name: &'static str, venue: Venue, tx: mpsc::Sender<EventKind>, run: F)
where
    F: Fn(mpsc::Sender<EventKind>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let mut backoff = 1u64;
    loop {
        tracing::info!(venue = name, "venue connect");
        match run(tx.clone()).await {
            Ok(()) => tracing::warn!(venue = name, "venue run exited — reconnect"),
            Err(e) => tracing::error!(venue = name, error = %e, "venue run error — reconnect"),
        }
        let _ = tx.send(EventKind::Sys(SysEvent::ConnDown(venue))).await;
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

/// Future, который резолвится по первому из SIGTERM / SIGINT (Unix) или Ctrl-C (fallback).
/// M-05 task 2 (engine-dev): даёт writer'у шанс сдрейнить буфер + flush до выхода.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "SIGTERM handler install failed — falling back to ctrl_c");
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "SIGINT handler install failed — falling back to ctrl_c");
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = sigterm.recv() => tracing::info!("SIGTERM received — initiating clean shutdown"),
            _ = sigint.recv()  => tracing::info!("SIGINT received — initiating clean shutdown"),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Ctrl-C received — initiating clean shutdown");
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let dir =
        PathBuf::from(std::env::var("JOURNAL_DIR").unwrap_or_else(|_| "./journal-data".into()));

    tracing::info!(
        journal_dir = %dir.display(),
        schema_version = contracts::SCHEMA_VERSION,
        venues = ?recorder::default_venues(),
        "recorder start"
    );

    let (tx, rx) = mpsc::channel::<EventKind>(50_000);

    // spawn one supervisor per venue from `default_venues()` — config-driven, не 3 хардкод-блока.
    // M-06 #4 (reland, post-TD-013): добавлен `Venue::BinanceFutures` (fstream @depth@100ms +
    // @forceOrder + !markPrice@arr + REST OI poll). Аргументы площадок: `BINANCE_SYMBOLS` /
    // `HL_COINS` / `BINANCE_FUTURES_SYMBOLS`.
    //
    // Type-erasure: три `::run`-функции имеют РАЗНЫЕ concrete-типы возвращаемых futures →
    // общая сигнатура `Fn(Sender) -> Fut` в `supervise()` вместить нельзя. Решение —
    // `Box<dyn Fn(Sender) -> Pin<Box<dyn Future + Send>>>` per call (один dyn-индирект на
    // создание future; внутри спарн-цикла это дешевле, чем статически дублировать N^2 supervisor'ов).
    type VenueRunFut =
        std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>;
    type VenueRunFn = Box<dyn Fn(mpsc::Sender<EventKind>) -> VenueRunFut + Send + Sync>;

    for venue in recorder::default_venues() {
        let tx_v = tx.clone();
        let (name, run_fn): (&'static str, VenueRunFn) = match venue {
            Venue::Binance => {
                let syms = env_csv("BINANCE_SYMBOLS", &["BTCUSDT", "ETHUSDT"]);
                (
                    "binance",
                    Box::new(move |t| Box::pin(venue_binance::run(t, syms.clone()))),
                )
            }
            Venue::Hyperliquid => {
                let coins = env_csv("HL_COINS", &["BTC", "ETH"]);
                (
                    "hyperliquid",
                    Box::new(move |t| Box::pin(venue_hyperliquid::run(t, coins.clone()))),
                )
            }
            Venue::BinanceFutures => {
                let syms = env_csv("BINANCE_FUTURES_SYMBOLS", &["BTCUSDT", "ETHUSDT"]);
                (
                    "binance_futures",
                    Box::new(move |t| Box::pin(venue_binance_futures::run(t, syms.clone()))),
                )
            }
        };
        tokio::spawn(async move {
            supervise(name, venue, tx_v, run_fn).await;
        });
    }
    drop(tx); // writer завершится, только если все продюсеры уйдут (в норме не уходят).

    // Единственный писатель — журнал в этой задаче.
    let journal = Journal::open(&dir)?;
    let hb_path = dir.join("recorder.heartbeat");

    recorder::run_writer(rx, journal, hb_path, shutdown_signal()).await?;
    Ok(())
}
