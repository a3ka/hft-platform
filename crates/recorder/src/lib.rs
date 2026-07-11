//! recorder (lib) — тестируемый seam writer-цикла. M-05 (engine-dev).
//!
//! Мотив (J1): раньше единственный `journal.flush()` срабатывал лишь при «все продюсеры
//! ушли» (в проде не бывает); SIGTERM-хендлера не было → docker stop (SIGTERM→SIGKILL)
//! убивал процесс посреди цикла → рваный фрейм + отставшая мета.
//!
//! Фикс (M-05 task 2): select!-цикл вынесен сюда; ветка `shutdown` ДРЕЙНит буфер +
//! `flush()` (seg+meta) перед выходом; `main` враппит SIGTERM/SIGINT в `shutdown`.
//! Инъектируемый `shutdown: impl Future` делает clean-shutdown ЮНИТ-тестируемым (J1)
//! без OS-сигналов.

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use contracts::{EventKind, SysEvent};
use journal::Journal;
use tokio::sync::mpsc;

/// Writer-цикл: пишет события из `rx` в журнал; по `shutdown` ДРЕЙНит уже буферизованные
/// события, `flush()` (seg+meta) и выходит чисто (без рваного фрейма, без потери/reuse seq).
pub async fn run_writer(
    mut rx: mpsc::Receiver<EventKind>,
    mut journal: Journal,
    hb_path: PathBuf,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<()> {
    let mut count: u64 = 0;
    // Первый tick смещён на период вперёд (иначе `interval` срабатывает мгновенно
    // на первой итерации — лишний heartbeat).
    let period = Duration::from_secs(10);
    let mut hb = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    tokio::pin!(shutdown);
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
                write_heartbeat(&hb_path);
                tracing::debug!(events = count, "heartbeat");
            }
            _ = &mut shutdown => {
                tracing::info!(events = count, "shutdown signal — дрейн буфера + flush");
                // Сдрейнить УЖЕ буферизованные события без ожидания новых (без потерь).
                while let Ok(kind) = rx.try_recv() {
                    journal.append(kind)?;
                    count += 1;
                }
                break;
            }
        }
    }
    journal.flush()?; // seg+meta согласованы, next_seq переживёт рестарт
    tracing::info!(
        events = count,
        next_seq = journal.next_seq(),
        "writer stopped — flushed"
    );
    Ok(())
}

fn write_heartbeat(hb_path: &std::path::Path) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let _ = std::fs::write(hb_path, now_ms.to_string());
}
