//! venue-hyperliquid — адаптер Hyperliquid (docs/fa/venues.md; recon
//! /tmp/hft_dataplane_recon.md §B+§D).
//!
//! Контракт: `run` подключается к `wss://api.hyperliquid.xyz/ws`, подписывается на trades +
//! l2Book по коинам (нативные тикеры: "BTC", не "BTCUSDT"), парсит (l2Book levels = ОБЪЕКТЫ
//! `{px,sz,n}`, не массивы!), нормализует в `contracts::MdEvent`, шлёт в `tx`. ОДНА сессия
//! соединения — reconnect/backoff делает вызывающий supervisor, а не этот модуль.
//! Приложенческий keepalive `{"method":"ping"}` каждые ~30с.

use contracts::{to_fixed, EventKind, Level, MdPayload, Side, SysEvent, Venue};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tokio_tungstenite::tungstenite::Message;

const WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// Запустить приём рыночных данных Hyperliquid для одной WS-сессии. `coins` — нативные
/// тикеры ("BTC","ETH"). Шлёт `EventKind::Md(..)` в `tx`; `ConnUp` — сразу после успешного
/// коннекта. Возвращает `Ok(())` при штатном закрытии/дисконнекте/остановке получателя;
/// `Err` — при ошибке коннекта. Reconnect — забота вызывающего.
pub async fn run(tx: mpsc::Sender<EventKind>, coins: Vec<String>) -> anyhow::Result<()> {
    let (ws_stream, _response) = tokio_tungstenite::connect_async(WS_URL).await?;
    let (mut write, mut read) = ws_stream.split();

    for coin in &coins {
        let trades_sub = serde_json::json!({
            "method": "subscribe",
            "subscription": { "type": "trades", "coin": coin },
        });
        let l2book_sub = serde_json::json!({
            "method": "subscribe",
            "subscription": { "type": "l2Book", "coin": coin },
        });
        if write
            .send(Message::Text(trades_sub.to_string()))
            .await
            .is_err()
        {
            return Ok(());
        }
        if write
            .send(Message::Text(l2book_sub.to_string()))
            .await
            .is_err()
        {
            return Ok(());
        }
    }

    if tx
        .send(EventKind::Sys(SysEvent::ConnUp(Venue::Hyperliquid)))
        .await
        .is_err()
    {
        return Ok(());
    }

    let mut ping_timer = interval(PING_INTERVAL);
    ping_timer.tick().await; // first tick fires immediately; consume it, connect already implies liveness

    loop {
        tokio::select! {
            msg = read.next() => {
                let Some(msg) = msg else {
                    return Ok(());
                };
                let msg = match msg {
                    Ok(msg) => msg,
                    Err(e) => {
                        tracing::debug!(error = %e, "venue-hyperliquid: WS read error, ending session");
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
            _ = ping_timer.tick() => {
                let ping = serde_json::json!({ "method": "ping" });
                if write.send(Message::Text(ping.to_string())).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

/// Разобрать одно `{"channel":"<type>","data":{...}}` сообщение в 0..N нормализованных
/// событий (trades — массив, может дать несколько событий за раз). Некорректный/неожиданный
/// формат — лог на debug, пустой Vec (без паники).
pub fn parse_message(text: &str) -> Vec<EventKind> {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(error = %e, raw = %text, "venue-hyperliquid: malformed JSON, skipping");
            return Vec::new();
        }
    };

    let channel = match value.get("channel").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            tracing::debug!(raw = %text, "venue-hyperliquid: message without 'channel', skipping");
            return Vec::new();
        }
    };

    match channel {
        "trades" => {
            let Some(data) = value.get("data").and_then(|v| v.as_array()) else {
                tracing::debug!(raw = %text, "venue-hyperliquid: trades without array 'data', skipping");
                return Vec::new();
            };
            data.iter().filter_map(parse_trade).collect()
        }
        "l2Book" => {
            let Some(data) = value.get("data") else {
                tracing::debug!(raw = %text, "venue-hyperliquid: l2Book without 'data', skipping");
                return Vec::new();
            };
            parse_l2book(data).into_iter().collect()
        }
        "pong" | "subscriptionResponse" => Vec::new(),
        other => {
            tracing::debug!(channel = %other, "venue-hyperliquid: unhandled channel, skipping");
            Vec::new()
        }
    }
}

/// One element of the `trades` `data` array:
/// `{"coin":"BTC","px":"<str>","sz":"<str>","time":<ms>,"side":"A"|"B"}`.
/// "A" = aggressive buy (taker buy) -> Side::Buy; "B" -> Side::Sell.
fn parse_trade(item: &serde_json::Value) -> Option<EventKind> {
    let coin = item.get("coin")?.as_str()?.to_string();
    if coin.contains("MID") {
        return None;
    }
    let price: f64 = item.get("px")?.as_str()?.parse().ok()?;
    let size: f64 = item.get("sz")?.as_str()?.parse().ok()?;
    let ts_exch_ms = item.get("time")?.as_i64()?;
    let side_raw = item.get("side")?.as_str()?;
    let side = match side_raw {
        "A" => Side::Buy,
        "B" => Side::Sell,
        _ => {
            tracing::debug!(side = %side_raw, "venue-hyperliquid: unknown trade side, skipping");
            return None;
        }
    };

    Some(EventKind::md(
        Venue::Hyperliquid,
        coin,
        MdPayload::Trade {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms,
        },
    ))
}

/// `data`: `{"coin":"BTC","time":<ms>,"levels":[[{"px","sz","n"},...],[{...},...]]}`.
/// `levels[0]` = bids, `levels[1]` = asks. CRITICAL: each level is an OBJECT `{px,sz,n}`,
/// NOT an array.
fn parse_l2book(data: &serde_json::Value) -> Option<EventKind> {
    let coin = data.get("coin")?.as_str()?.to_string();
    if coin.contains("MID") {
        return None;
    }
    let levels = data.get("levels")?.as_array()?;
    let bids = parse_level_objects(levels.first()?)?;
    let asks = parse_level_objects(levels.get(1)?)?;
    let ts_exch_ms = data.get("time").and_then(|v| v.as_i64()).unwrap_or(0);

    Some(EventKind::md(
        Venue::Hyperliquid,
        coin,
        MdPayload::L2Snapshot {
            bids,
            asks,
            ts_exch_ms,
        },
    ))
}

/// `[{"px":"<str>","sz":"<str>","n":<int>}, ...]` -> `Vec<Level>` (fixed-point ×1e8).
fn parse_level_objects(levels: &serde_json::Value) -> Option<Vec<Level>> {
    let levels = levels.as_array()?;
    let mut out = Vec::with_capacity(levels.len());
    for level in levels {
        let price: f64 = level.get("px")?.as_str()?.parse().ok()?;
        let size: f64 = level.get("sz")?.as_str()?.parse().ok()?;
        out.push(Level {
            price: to_fixed(price),
            size: to_fixed(size),
        });
    }
    Some(out)
}
