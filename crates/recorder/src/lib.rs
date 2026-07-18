//! recorder (lib) — тестируемый seam writer-цикла. M-05 task 2 (engine-dev).
//!
//! Мотив (J1): раньше единственный `journal.flush()` в `main.rs:112` срабатывал лишь
//! при «все продюсеры ушли» (в проде не бывает); SIGTERM-хендлера не было → docker stop
//! (SIGTERM→SIGKILL) убивал процесс посреди цикла → рваный фрейм + отставшая мета.
//!
//! Фикс: select!-цикл с явной `shutdown`-веткой → по сигналу ДРЕЙН буфера mpsc +
//! `Journal::flush()` (seg+meta) + exit. `main` враппит SIGTERM/SIGINT в `shutdown`.
//! Инъектируемый `shutdown: impl Future` делает clean-shutdown ЮНИТ-тестируемым (J1)
//! без OS-сигналов.

use std::future::Future;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use contracts::{EventKind, SysEvent, Venue};
use journal::{Journal, StorageStatus};
use tokio::sync::mpsc;

/// M-09: recon-loop (изоляция recon-сбоя от append, JR-I-1). Скелет — impl engine-dev по RED.
pub mod recon_loop;

/// Площадки, которые рекордер супервизит по умолчанию. `main` спавнит `supervise()` по
/// ЭТОМУ списку (config-driven, не хардкод). M-06 #4 (reland, post-TD-013): BinanceFutures
/// подключён — эмиттер `venue-binance-futures::run` выдаёт depth (@depth@100ms), liquidations
/// (@forceOrder), funding (!markPrice@arr) и OI (REST poll) через одну WS-сессию +
/// honourащий TD-013 backoff (анти 418-hot-loop, см. §8 eyes-on).
/// RED-оракул: `crates/recorder/tests/red_futures_wired.rs`.
pub fn default_venues() -> Vec<Venue> {
    vec![Venue::Binance, Venue::Hyperliquid, Venue::BinanceFutures]
}

/// Writer-цикл: пишет события из `rx` в журнал. По `shutdown` ОБЯЗАН сдрейнить уже
/// буферизованные события (`try_recv` пока `Empty` или `Disconnected`), сделать
/// финальный `Journal::flush()` (seg+meta) и выйти чисто — без рваного фрейма,
/// без потери/reuse seq. `biased;` гарантирует приоритет shutdown над rx.recv().
pub async fn run_writer(
    mut rx: mpsc::Receiver<EventKind>,
    mut journal: Journal,
    hb_path: PathBuf,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<()> {
    use tokio::sync::mpsc::error::TryRecvError;

    let mut count: u64 = 0;
    let mut hb = tokio::time::interval(Duration::from_secs(10));
    tokio::pin!(shutdown);

    'outer: loop {
        tokio::select! {
            // biased: shutdown первым, чтобы стоп-сигнал не зависал за медленным rx.
            biased;
            _ = &mut shutdown => {
                tracing::info!("shutdown signalled — drain+flush");
                // Дрейн буфера канала. tx ещё может быть жив (supervisor-таски).
                loop {
                    match rx.try_recv() {
                        Ok(kind) => {
                            journal.append(kind)?;
                            count += 1;
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break,
                    }
                }
                journal.flush()?;
                tracing::info!(events = count, "shutdown clean");
                break 'outer;
            }
            maybe = rx.recv() => match maybe {
                Some(kind) => {
                    journal.append(kind)?;
                    count += 1;
                    if count.is_multiple_of(1000) {
                        journal.flush()?;
                        tracing::info!(events = count, next_seq = journal.next_seq(), "journal progress");
                    }
                }
                None => {
                    tracing::warn!("all producers gone — writer exit");
                    break 'outer;
                }
            },
            _ = hb.tick() => {
                journal.append(EventKind::Sys(SysEvent::Heartbeat))?;
                journal.flush()?;
                write_heartbeat(&hb_path, &journal, count);
                tracing::debug!(events = count, "heartbeat");
            }
        }
    }

    // Финальный heartbeat при выходе (shutdown / all-producers-gone) — гарантирует, что
    // внешний мониторинг ВСЕГДА видит последнее состояние, даже если writer не дожил
    // до очередного 10-секундного тика. Без этого red_heartbeat_status в коротких
    // прогонах не находил файл (фундаментальный класс TD-011/TD-019: «heartbeat есть»
    // отличается от «heartbeat ОТРАЖАЕТ реальность»).
    write_heartbeat(&hb_path, &journal, count);

    journal.flush()?;
    Ok(())
}

/// Записать heartbeat-файл как JSON с состоянием (TD-019, M-08 E4 наблюдаемость).
///
/// **Контракт** (RED `red_heartbeat_status.rs`): JSON-объект с полями
/// `ts_wall_ms`, `next_seq`, `segment_index`, `free_bytes`, `min_free_bytes`, `writable`.
/// Голый таймстамп (прежняя реализация) — ровно тот класс ошибки, против которого
/// написан RED: healthcheck отвечает «процесс жив», а не «процесс делает то, что должен».
///
/// **В ЖУРНАЛ НЕ ПИШЕТСЯ** (`OPS-I-6` детерминизм): heartbeat — наблюдаемость, а не
/// данные; повторение heartbeat в журнале сломало бы DET-I-1 (replay ×3 бит-идентичен).
/// Ошибки записи логируются и ГЛОТАЮТСЯ — сбой heartbeat-файла НЕ роняет recorder
/// (recorder пишет данные, а не мониторинг).
fn write_heartbeat(hb_path: &std::path::Path, journal: &Journal, events: u64) {
    let ts_wall_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let next_seq = journal.next_seq();
    let segment_index = journal.active_segment_index();
    let storage: Option<StorageStatus> = journal.storage_status().ok();
    let payload = serde_json::json!({
        "ts_wall_ms": ts_wall_ms,
        "next_seq": next_seq,
        "segment_index": segment_index,
        "events": events,
        "free_bytes": storage.as_ref().map(|s| s.free_bytes),
        "min_free_bytes": storage.as_ref().map(|s| s.min_free_bytes),
        "writable": storage.as_ref().map(|s| s.writable),
    });
    // Ошибки heartbeat ГЛОТАЮТСЯ: recorder пишет данные, а не мониторинг.
    // Аналогия: VM-probe; если пульс не дошёл — повторная попытка на следующем тике.
    if let Ok(body) = serde_json::to_string(&payload) {
        if let Err(e) = std::fs::write(hb_path, body) {
            tracing::warn!(error = %e, "heartbeat write failed (non-fatal)");
        }
    } else {
        tracing::warn!("heartbeat JSON serialize failed (non-fatal)");
    }
}
