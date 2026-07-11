//! M-05 RED (sacred, architect) — J1: clean-shutdown рекордера не теряет данные и не
//! оставляет рваный фрейм. Контракт: по shutdown writer ДРЕЙНит буфер + flush + выходит.
//! Падает на STUB (0 событий на диске) → engine-dev делает GREEN.

use contracts::{EventKind, SysEvent};
use journal::Journal;
use recorder::run_writer;

#[tokio::test]
async fn j1_clean_shutdown_drains_and_flushes_no_loss() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::open(dir.path()).unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel::<EventKind>(500);
    for _ in 0..150 {
        tx.send(EventKind::Sys(SysEvent::Heartbeat)).await.unwrap();
    }

    // Сигнал shutdown уже взведён ДО await — writer обязан сдрейнить 150 буферизованных.
    let (sd_tx, sd_rx) = tokio::sync::oneshot::channel::<()>();
    sd_tx.send(()).unwrap();

    run_writer(
        rx,
        journal,
        dir.path().join("recorder.heartbeat"),
        async move {
            let _ = sd_rx.await;
        },
    )
    .await
    .unwrap();

    // Журнал читается целиком (нет рваного фрейма), 150 событий, seq непрерывен 0..149.
    let evs = journal::read_all(dir.path()).unwrap();
    assert_eq!(
        evs.len(),
        150,
        "clean-shutdown обязан сдрейнить буфер и зафлашить БЕЗ потерь"
    );
    for (i, e) in evs.iter().enumerate() {
        assert_eq!(e.seq, i as u64, "seq непрерывен (нет потери/reuse)");
    }
}
