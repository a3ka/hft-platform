//! recorder — вход даталеера. venue-адаптеры (Binance + Hyperliquid) → mpsc-канал (EventKind)
//! → журнал (ЕДИНСТВЕННЫЙ писатель, seq тотальный порядок). docs/fa/{venues,journal}.md.
//!
//! Конфиг через env: JOURNAL_DIR, BINANCE_SYMBOLS (csv, e.g. BTCUSDT,ETHUSDT),
//! HL_COINS (csv, e.g. BTC,ETH). Reconnect — внутри venue::run; здесь supervisor + backoff.

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

    let (tx, mut rx) = mpsc::channel::<EventKind>(50_000);

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
    let mut journal = Journal::open(&dir)?;
    let hb_path = dir.join("recorder.heartbeat");
    let mut count: u64 = 0;
    let mut hb = tokio::time::interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            maybe = rx.recv() => match maybe {
                Some(kind) => {
                    journal.append(kind)?;
                    count += 1;
                    if count.is_multiple_of(1000) {
                        journal.flush()?;
                        tracing::info!(events = count, next_seq = journal.next_seq(), "journal progress");
                    }
                }
                None => { tracing::warn!("all producers gone — writer exit"); break; }
            },
            _ = hb.tick() => {
                journal.append(EventKind::Sys(SysEvent::Heartbeat))?;
                journal.flush()?;
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
                let _ = std::fs::write(&hb_path, now_ms.to_string());
                tracing::debug!(events = count, "heartbeat");
            }
        }
    }
    journal.flush()?;
    Ok(())
}
