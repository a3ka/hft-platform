//! venue-hyperliquid — адаптер Hyperliquid. STUB (компилируется; реальный WS-клиент пишется
//! субагентом по /tmp/hft_dataplane_recon.md §B + §D).
//!
//! Контракт: `run` подключается к `wss://api.hyperliquid.xyz/ws`, подписывается на trades +
//! l2Book по коинам (нативные тикеры: "BTC", не "BTCUSDT"), парсит (l2Book levels = объекты
//! {px,sz,n}!), нормализует в `contracts::MdEvent`, шлёт в `tx`. Reconnect с backoff.

use contracts::EventKind;
use tokio::sync::mpsc;

/// Запустить приём рыночных данных Hyperliquid. `coins` — нативные тикеры ("BTC","ETH").
/// Шлёт `EventKind::Md(..)` в `tx`; ConnUp — при успешном коннекте.
pub async fn run(_tx: mpsc::Sender<EventKind>, coins: Vec<String>) -> anyhow::Result<()> {
    tracing::warn!(
        ?coins,
        "venue-hyperliquid STUB — WS-клиент ещё не реализован"
    );
    Ok(())
}
