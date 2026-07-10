//! venue-binance — адаптер Binance (docs/fa/venues.md; recon /tmp/hft_dataplane_recon.md §A+§D).
//!
//! Контракт: `run` подключается к Binance WS (combined-stream), подписывается на
//! `{symbol}@trade` + `{symbol}@depth20@100ms` по символам, парсит, нормализует в
//! `contracts::MdEvent` и шлёт в `tx`. ОДНА сессия соединения — reconnect/backoff делает
//! вызывающий supervisor, а не этот модуль. Emitter-not-owner (VN-I): seq не проставляет,
//! риск/позиции не трогает.

use contracts::{to_fixed, EventKind, Level, MdPayload, Side, SysEvent, Venue};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const WS_BASE: &str = "wss://stream.binance.com:9443/stream?streams=";

/// Запустить приём рыночных данных Binance для одной WS-сессии. Шлёт `EventKind::Md(..)`
/// в `tx`; `ConnUp` — сразу после успешного коннекта. Возвращает `Ok(())` при штатном
/// закрытии/дисконнекте/остановке получателя; `Err` — при ошибке коннекта. Reconnect —
/// забота вызывающего.
pub async fn run(tx: mpsc::Sender<EventKind>, symbols: Vec<String>) -> anyhow::Result<()> {
    let mut streams = Vec::with_capacity(symbols.len() * 2);
    for s in &symbols {
        let lower = s.to_lowercase();
        streams.push(format!("{lower}@trade"));
        streams.push(format!("{lower}@depth20@100ms"));
    }
    let url = format!("{WS_BASE}{}", streams.join("/"));

    let (ws_stream, _response) = tokio_tungstenite::connect_async(&url).await?;
    let (mut write, mut read) = ws_stream.split();

    if tx
        .send(EventKind::Sys(SysEvent::ConnUp(Venue::Binance)))
        .await
        .is_err()
    {
        return Ok(());
    }

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                tracing::debug!(error = %e, "venue-binance: WS read error, ending session");
                return Ok(());
            }
        };

        match msg {
            Message::Text(text) => {
                for event in parse_message(&text) {
                    if tx.send(event).await.is_err() {
                        return Ok(());
                    }
                }
            }
            Message::Ping(payload) => {
                if write.send(Message::Pong(payload)).await.is_err() {
                    return Ok(());
                }
            }
            Message::Close(_) => return Ok(()),
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }

    Ok(())
}

/// Разобрать одно combined-stream сообщение `{"stream":"<name>","data":{...}}` в 0 или 1
/// нормализованное событие. Некорректный/неожиданный формат — лог на debug, пустой Vec
/// (без паники).
fn parse_message(text: &str) -> Vec<EventKind> {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(error = %e, raw = %text, "venue-binance: malformed JSON, skipping");
            return Vec::new();
        }
    };

    let stream = match value.get("stream").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            tracing::debug!(raw = %text, "venue-binance: message without 'stream', skipping");
            return Vec::new();
        }
    };
    let data = match value.get("data") {
        Some(d) => d,
        None => {
            tracing::debug!(raw = %text, "venue-binance: message without 'data', skipping");
            return Vec::new();
        }
    };

    let event = if stream.ends_with("@trade") {
        parse_trade(data)
    } else if stream.contains("@depth") {
        parse_depth(stream, data)
    } else {
        tracing::debug!(stream = %stream, "venue-binance: unrecognized stream, skipping");
        None
    };

    match event {
        Some(e) => vec![e],
        None => Vec::new(),
    }
}

/// `data`: `{"s":"BTCUSDT","p":"<price>","q":"<qty>","m":<bool is_buyer_maker>,"T":<ms>}`.
fn parse_trade(data: &serde_json::Value) -> Option<EventKind> {
    let symbol = data.get("s")?.as_str()?.to_string();
    let price: f64 = data.get("p")?.as_str()?.parse().ok()?;
    let qty: f64 = data.get("q")?.as_str()?.parse().ok()?;
    let is_buyer_maker = data.get("m")?.as_bool()?;
    let ts_exch_ms = data.get("T")?.as_i64()?;

    // Binance `m` = "is this trade the buyer's maker order?" — taker side is the inverse.
    let side = if is_buyer_maker {
        Side::Sell
    } else {
        Side::Buy
    };

    Some(EventKind::md(
        Venue::Binance,
        symbol,
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(qty),
            side,
            ts_exch_ms,
        },
    ))
}

/// `data`: `{"bids":[["p","q"],...],"asks":[[...]],"E":<ms, optional>}`. `data` has no `s`
/// field for depth pushes — symbol is derived from the stream name
/// (`"btcusdt@depth20@100ms"` -> `"BTCUSDT"`).
fn parse_depth(stream: &str, data: &serde_json::Value) -> Option<EventKind> {
    let symbol = match data.get("s").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => stream.split('@').next()?.to_uppercase(),
    };

    let bids = parse_levels(data.get("bids")?)?;
    let asks = parse_levels(data.get("asks")?)?;
    let ts_exch_ms = data.get("E").and_then(|v| v.as_i64()).unwrap_or(0);

    Some(EventKind::md(
        Venue::Binance,
        symbol,
        MdPayload::L2Snapshot {
            bids,
            asks,
            ts_exch_ms,
        },
    ))
}

/// `[["price","qty"], ...]` -> `Vec<Level>` (fixed-point ×1e8).
fn parse_levels(levels: &serde_json::Value) -> Option<Vec<Level>> {
    let levels = levels.as_array()?;
    let mut out = Vec::with_capacity(levels.len());
    for level in levels {
        let pair = level.as_array()?;
        let price: f64 = pair.first()?.as_str()?.parse().ok()?;
        let size: f64 = pair.get(1)?.as_str()?.parse().ok()?;
        out.push(Level {
            price: to_fixed(price),
            size: to_fixed(size),
        });
    }
    Some(out)
}
