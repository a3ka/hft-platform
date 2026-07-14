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
use std::process::Command;
use std::time::Duration;

use contracts::{EventKind, SysEvent, Venue};
use journal::{Journal, WriterConfig};
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

    // === M-08 task 4: писать заголовок сегмента (CT-RFC-02, E2/E4) ===
    //
    // Каждый НОВЫЙ сегмент открывается заголовком SegmentHeader с provenance =
    // версия recorder'а + git sha + epoch_id. Ротация — внутри `Journal::append()`
    // (порог из `WriterConfig::max_segment_bytes`), disk-guard — там же.
    // Миграция с `Journal::open()` на `open_with()` ОДНА запись = ОДИН коммит; на этой
    // неделе прод-сегмент (8.3 GB, без магии) будет прочитан legacy-путём через
    // явную декларацию `journal.legacy.json` (CT-RFC-02 rev 2, fail-closed).
    let cfg = build_writer_config();
    tracing::info!(
        max_segment_bytes = cfg.max_segment_bytes,
        min_free_bytes = cfg.min_free_bytes,
        provenance = %cfg.provenance,
        epoch_id = %cfg.epoch_id,
        "writer config",
    );
    let journal = Journal::open_with(&dir, cfg)?;
    let hb_path = dir.join("recorder.heartbeat");

    recorder::run_writer(rx, journal, hb_path, shutdown_signal()).await?;
    Ok(())
}

/// Собрать `WriterConfig` для recorder'а: собственный захват, порог 1 GiB/сегмент,
/// 10 GiB disk-guard, provenance = версия recorder'а + короткий git SHA (если доступен).
fn build_writer_config() -> WriterConfig {
    let version = env!("CARGO_PKG_VERSION");
    let git_sha = git_short_sha().unwrap_or_else(|| "no-git-info".to_string());
    let provenance = format!("recorder v{version} (git:{git_sha})");
    // epoch_id — стабильный ключ эпохи. По умолчанию: "own-<UTC-YYYY-MM>". Оператор
    // может переопределить через env (прод-развёртывание с известным epoch_id).
    let epoch_id = std::env::var("EPOCH_ID").unwrap_or_else(|_| default_epoch_id_now());
    WriterConfig::own_capture(provenance, epoch_id)
}

/// Короткий git SHA текущего HEAD (если рабочая копия — git-репо). Иначе `None`.
/// `git rev-parse --short HEAD` вызывается синхронно на старте recorder'а (раз в процесс).
fn git_short_sha() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `own-<UTC-YYYY-MM>` из текущего времени (дефолт для дев-сборки без явного `EPOCH_ID`).
fn default_epoch_id_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs_per_day = 86_400u64;
    // Unix epoch → 1970-01-01. Грубый расчёт year-month (UTC, без leap-секунд) — достаточно
    // для группировки эпох по месяцу; точный разбор chrono — за рамками M-08.
    let days = now / secs_per_day;
    let (y, m) = days_to_ym(days);
    format!("own-{y:04}-{m:02}")
}

/// Дни → (year, month_utc) без chrono. Алгоритм: 400-летний Gregorian cycle.
fn days_to_ym(days_since_1970: u64) -> (i32, u32) {
    let mut z = days_since_1970 as i64;
    let mut year = 1970i32;
    loop {
        let leap = is_leap(year);
        let year_days = if leap { 366 } else { 365 };
        if z < year_days {
            break;
        }
        z -= year_days;
        year += 1;
    }
    let leap = is_leap(year);
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for &dm in &months {
        if z < dm {
            break;
        }
        z -= dm;
        month += 1;
    }
    (year, month)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_to_ym_epoch() {
        assert_eq!(days_to_ym(0), (1970, 1));
    }

    #[test]
    fn days_to_ym_one_year_later() {
        // 1971-01-01 = day 365
        assert_eq!(days_to_ym(365), (1971, 1));
    }

    #[test]
    fn days_to_ym_leap_year() {
        // 2000-02-29 = day 11015 (29 Feb 2000)
        // 1970-01-01 ... 2000-02-29 = 30 лет + 60 дней в феврале 2000
        // Approximate day: 30*365 + 7 (leap) = 10957 дней до 2000-01-01, + 31 (янв) + 28 (фев до 29) = 11016
        // Точное: 2000-02-29 = day 11016 в предположении что 1970-01-01 это день 0.
        let (y, m) = days_to_ym(11016);
        assert_eq!(y, 2000);
        assert_eq!(m, 2);
    }
}
