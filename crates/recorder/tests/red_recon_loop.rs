//! RED M-09 recon-loop ИЗОЛЯЦИЯ (sacred, architect-only) — recon-сбой НЕ останавливает append.
//!
//! `JR-I-1` + 24/7: рекордер — единственный писатель журнала; recon (REST/сверка) может паниковать
//! (parse, бан, книга), но append священен. Recon обязан исполняться ИЗОЛИРОВАННЫМ таском.
//!
//! Анти-плацебо: если recon НЕ изолирован (исполняется инлайн в writer-стеке / `spawn_recon_isolated`
//! не спавнит) — паника разворачивает стек вызывающего и append-канал не доживает → тест падает.
//! Против `todo!()` — падает. Против корректного `tokio::spawn` — проходит.

use contracts::{EventKind, SysEvent};
use recorder::recon_loop::spawn_recon_isolated;
use tokio::sync::mpsc;

/// Паника внутри recon-таска изолирована: append-канал продолжает доставлять события ПОСЛЕ неё.
#[tokio::test]
async fn recon_panic_does_not_stop_append() {
    let (append_tx, mut append_rx) = mpsc::channel::<EventKind>(8);

    // recon-таск паникует (эмулируем сбой fetch/reconcile/бан).
    let handle = spawn_recon_isolated(|| async {
        panic!("recon boom: parse/бан/книга — НЕ должно ронять append");
    });

    // Append-путь рекордера продолжает работать, НЕ затронут recon-паникой (JR-I-1, 24/7).
    append_tx
        .send(EventKind::Sys(SysEvent::Heartbeat))
        .await
        .expect("append-канал затронут recon-паникой — 24/7 сбор нарушен (JR-I-1)");

    // Изолированная паника отдаётся как JoinError, а НЕ разворачивает наш стек.
    let joined = handle.await;
    assert!(
        joined.is_err(),
        "recon-таск паниковал, но JoinHandle вернул Ok — паника не там, где ожидалась (проверь \
         изоляцию)"
    );

    assert_eq!(
        append_rx.recv().await,
        Some(EventKind::Sys(SysEvent::Heartbeat)),
        "append-канал не доставил событие после recon-паники — recon уронил писателя (нельзя!)"
    );
}
