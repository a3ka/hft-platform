//! recorder (lib) — тестируемый seam writer-цикла. M-05 SKELETON (architect).
//!
//! Мотив (J1): сейчас единственный `journal.flush()` в `main.rs:112` срабатывает лишь
//! при «все продюсеры ушли» (в проде не бывает); SIGTERM-хендлера нет → docker stop
//! (SIGTERM→SIGKILL) убивает процесс посреди цикла → рваный фрейм + отставшая мета.
//!
//! Фикс (M-05 task 2, engine-dev): вынести select!-цикл сюда, добавить `shutdown`-ветку
//! и ДРЕЙН буфера + `flush()` перед выходом; `main` враппит SIGTERM в `shutdown`.
//! Инъектируемый `shutdown: impl Future` делает clean-shutdown ЮНИТ-тестируемым (J1)
//! без OS-сигналов.

use std::future::Future;
use std::path::PathBuf;

use contracts::EventKind;
use journal::Journal;
use tokio::sync::mpsc;

/// Writer-цикл: пишет события из `rx` в журнал; по `shutdown` ОБЯЗАН сдрейнить уже
/// буферизованные события, `flush()` (seg+meta) и выйти чисто (без рваного фрейма,
/// без потери/reuse seq). STUB — engine-dev (M-05 task 2). RED: tests/red_shutdown_j1.rs.
pub async fn run_writer(
    rx: mpsc::Receiver<EventKind>,
    journal: Journal,
    hb_path: PathBuf,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<()> {
    // STUB: игнорирует rx и shutdown, не пишет события. journal дропается → Drop::flush
    // (пустой). J1 ждёт 150 событий на диске → RED.
    let _ = (rx, hb_path, shutdown);
    drop(journal);
    Ok(())
}
