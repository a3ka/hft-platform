//! venue-binance — адаптер Binance. STUB (заглушка компилируется; реальный WS-клиент
//! пишется субагентом по /tmp/hft_dataplane_recon.md §A + §D).
//!
//! Контракт: `run` подключается к Binance WS, подписывается на trades + depth20@100ms по
//! символам, парсит, нормализует в `contracts::MdEvent` и шлёт в `tx`. Reconnect с backoff.
//! Emitter-not-owner (VN-I): seq не проставляет, риск/позиции не трогает.

use contracts::EventKind;
use tokio::sync::mpsc;

/// Запустить приём рыночных данных Binance. Шлёт `EventKind::Md(..)` в `tx`; ConnUp — при
/// успешном коннекте. Работает до отмены; при разрыве возвращает управление supervisor'у.
pub async fn run(_tx: mpsc::Sender<EventKind>, symbols: Vec<String>) -> anyhow::Result<()> {
    tracing::warn!(?symbols, "venue-binance STUB — WS-клиент ещё не реализован");
    Ok(())
}
