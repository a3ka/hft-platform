//! recorder — вход даталеера. venue-адаптеры (Binance + Hyperliquid) → mpsc-канал (EventKind)
//! → журнал (ЕДИНСТВЕННЫЙ писатель, seq тотальный порядок). docs/fa/{venues,journal}.md.
//!
//! Конфиг через env: JOURNAL_DIR, BINANCE_SYMBOLS (csv, e.g. BTCUSDT,ETHUSDT),
//! HL_COINS (csv, e.g. BTC,ETH). Reconnect — внутри venue::run; здесь supervisor + backoff.
//!
//! M-05 (engine-dev): SIGTERM/SIGINT → `shutdown` future → `run_writer` дренит mpsc +
//! flush перед exit (J1 — clean-shutdown).

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
    let binance_symbols = env_csv("BINANCE_SYMBOLS", &["BTCUSDT", "ETHUSDT"]);
    let hl_coins = env_csv("HL_COINS", &["BTC", "ETH"]);

    tracing::info!(
        journal_dir = %dir.display(), ?binance_symbols, ?hl_coins,
        schema_version = contracts::SCHEMA_VERSION, "recorder start"
    );

    let (tx, rx) = mpsc::channel::<EventKind>(50_000);

    {
        let (tx_b, syms) = (tx.clone(), binance_symbols.clone());
        tokio::spawn(async move {
            supervise("binance", Venue::Binance, tx_b, move |t| {
                venue_binance::run(t, syms.clone())
            })
            .await;
        });
    }
    {
        let (tx_h, coins) = (tx.clone(), hl_coins.clone());
        tokio::spawn(async move {
            supervise("hyperliquid", Venue::Hyperliquid, tx_h, move |t| {
                venue_hyperliquid::run(t, coins.clone())
            })
            .await;
        });
    }
    drop(tx); // writer завершится, только если все продюсеры уйдут (в норме не уходят).

    // Единственный писатель — журнал в этой задаче.
    let journal = Journal::open(&dir)?;
    let hb_path = dir.join("recorder.heartbeat");

    recorder::run_writer(rx, journal, hb_path, shutdown_signal()).await?;
    Ok(())
}
