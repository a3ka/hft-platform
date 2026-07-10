//! venue-binance — адаптер Binance (docs/fa/venues.md; recon /tmp/hft_dataplane_recon.md §A+§D).
//!
//! Контракт: `run` подключается к Binance WS (combined-stream), подписывается на
//! `{symbol}@trade` + `{symbol}@depth@100ms` (RAW DIFF, не depth20) по символам, парсит,
//! ведёт ПОЛНЫЙ стакан на клиенте через стандартный snapshot+diff-sync алгоритм Binance
//! (REST `/api/v3/depth` snapshot + непрерывная последовательность WS diff-апдейтов по
//! `U`/`u`), нормализует в периодический `contracts::MdEvent::L2Snapshot` и шлёт в `tx`.
//! ОДНА сессия соединения — reconnect/backoff делает вызывающий supervisor, а не этот
//! модуль. Emitter-not-owner (VN-I): seq не проставляет, риск/позиции не трогает.

use contracts::{to_fixed, EventKind, Level, MdPayload, Side, SysEvent, Venue};
use futures_util::stream::FuturesUnordered;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const WS_BASE: &str = "wss://stream.binance.com:9443/stream?streams=";
const REST_DEPTH_BASE: &str = "https://api.binance.com/api/v3/depth?symbol=";
const REST_DEPTH_LIMIT: &str = "5000";

/// Ширина relative-distance бакета при сжатии полного стакана в периодический снапшот
/// (0.02% от mid).
const BUCKET_WIDTH: f64 = 0.0002;
/// Максимальная relative-distance от mid, включаемая в периодический снапшот (±60%).
const MAX_REL_DIST: f64 = 0.60;
/// Период эмиссии bounded L2Snapshot per symbol.
const EMIT_PERIOD: Duration = Duration::from_secs(1);

/// Локальная копия полного стакана одного символа. price/size — fixed-point ×1e8
/// (per `contracts::PRICE_SCALE`), ключ `BTreeMap` — цена, что даёт бесплатную
/// сортировку + O(log n) upsert/remove на diff-апдейте.
struct OrderBook {
    bids: BTreeMap<i64, i64>,
    asks: BTreeMap<i64, i64>,
    last_update_id: u64,
}

/// Один WS `@depth@100ms` diff-апдейт после парсинга. `u_first`/`u_final` — `U`/`u` из
/// payload Binance. size==0 в уровне означает "удалить уровень" (per Binance diff-sync
/// docs), это разворачивается в `apply_diff_to_book`.
struct DepthDiff {
    u_first: u64,
    u_final: u64,
    bids: Vec<(i64, i64)>,
    asks: Vec<(i64, i64)>,
}

/// Состояние sync-конечного-автомата одного символа.
///
/// - `book == None` => ещё не синхронизирован (стартап ИЛИ обнаружен gap) — входящие
///   diff-апдейты буферизуются в `pending`, снапшот запрашивается через REST.
/// - `book == Some(_)` => синхронизирован — diff-апдейты применяются напрямую с
///   проверкой непрерывности `U == last_update_id + 1`.
struct SymbolState {
    book: Option<OrderBook>,
    pending: VecDeque<DepthDiff>,
    resyncing: bool,
}

impl SymbolState {
    fn new() -> Self {
        SymbolState {
            book: None,
            pending: VecDeque::new(),
            resyncing: false,
        }
    }
}

/// Что делать с входящим diff-апдейтом относительно текущего состояния символа.
enum DiffAction {
    /// Апдейт старше текущего состояния книги — отбросить.
    Skip,
    /// Книга ещё не синхронизирована — буферизовать, запросить снапшот если ещё не в
    /// процессе.
    Buffer,
    /// Разрыв непрерывности (`U != last_update_id + 1`) — книга инвалидируется,
    /// пере-синхронизация с нуля.
    Gap,
    /// Апдейт непрерывен — применить к книге.
    Apply,
}

type SnapshotFuture = Pin<Box<dyn Future<Output = (String, anyhow::Result<OrderBook>)> + Send>>;

/// Запустить приём рыночных данных Binance для одной WS-сессии. Шлёт `EventKind::Md(..)`
/// в `tx`; `ConnUp` — сразу после успешного коннекта. Возвращает `Ok(())` при штатном
/// закрытии/дисконнекте/остановке получателя; `Err` — при ошибке коннекта. Reconnect —
/// забота вызывающего.
pub async fn run(tx: mpsc::Sender<EventKind>, symbols: Vec<String>) -> anyhow::Result<()> {
    let mut streams = Vec::with_capacity(symbols.len() * 2);
    for s in &symbols {
        let lower = s.to_lowercase();
        streams.push(format!("{lower}@trade"));
        streams.push(format!("{lower}@depth@100ms"));
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

    let client = reqwest::Client::new();
    let mut states: HashMap<String, SymbolState> = HashMap::new();
    let mut pending_snapshots: FuturesUnordered<SnapshotFuture> = FuturesUnordered::new();

    // Стартовая синхронизация: для каждого символа сразу заводим REST snapshot-фетч и
    // помечаем состояние как "resyncing" — любые diff-апдейты, пришедшие по WS ДО того,
    // как фетч завершится, буферизуются (per `handle_diff` DiffAction::Buffer), а не
    // теряются.
    for s in &symbols {
        let symbol = s.to_uppercase();
        let mut state = SymbolState::new();
        state.resyncing = true;
        pending_snapshots.push(make_snapshot_future(client.clone(), symbol.clone()));
        states.insert(symbol, state);
    }

    let mut emit_interval = tokio::time::interval(EMIT_PERIOD);
    emit_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            msg = read.next() => {
                let Some(msg) = msg else {
                    return Ok(());
                };
                let msg = match msg {
                    Ok(msg) => msg,
                    Err(e) => {
                        tracing::debug!(error = %e, "venue-binance: WS read error, ending session");
                        return Ok(());
                    }
                };

                match msg {
                    Message::Text(text) => {
                        if !handle_text_message(&text, &tx, &mut states, &client, &mut pending_snapshots).await {
                            return Ok(());
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
            Some((symbol, result)) = pending_snapshots.next(), if !pending_snapshots.is_empty() => {
                handle_snapshot(&mut states, symbol, result, &client, &mut pending_snapshots);
            }
            _ = emit_interval.tick() => {
                if !emit_book_snapshots(&states, &tx).await {
                    return Ok(());
                }
            }
        }
    }
}

/// Разобрать одно combined-stream текстовое сообщение и применить эффект: trade —
/// немедленная эмиссия в `tx`; depth diff — прогон через sync-автомат символа. Возвращает
/// `false`, если `tx` закрыт (получатель ушёл) — вызывающий должен завершить сессию.
async fn handle_text_message(
    text: &str,
    tx: &mpsc::Sender<EventKind>,
    states: &mut HashMap<String, SymbolState>,
    client: &reqwest::Client,
    pending_snapshots: &mut FuturesUnordered<SnapshotFuture>,
) -> bool {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(error = %e, raw = %text, "venue-binance: malformed JSON, skipping");
            return true;
        }
    };

    let stream = match value.get("stream").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            tracing::debug!(raw = %text, "venue-binance: message without 'stream', skipping");
            return true;
        }
    };
    let data = match value.get("data") {
        Some(d) => d,
        None => {
            tracing::debug!(raw = %text, "venue-binance: message without 'data', skipping");
            return true;
        }
    };

    if stream.ends_with("@trade") {
        if let Some(event) = parse_trade(data) {
            if tx.send(event).await.is_err() {
                return false;
            }
        }
    } else if stream.contains("@depth") {
        if let Some((symbol, diff)) = parse_depth_diff(stream, data) {
            let state = states
                .entry(symbol.clone())
                .or_insert_with(SymbolState::new);
            handle_diff(state, &symbol, diff, client, pending_snapshots);
        }
    } else {
        tracing::debug!(stream = %stream, "venue-binance: unrecognized stream, skipping");
    }

    true
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

/// `data`: `{"s":"BTCUSDT","U":<first update id>,"u":<final update id>,"b":[["p","q"],...],"a":[[...]]}`
/// (raw `@depth@100ms` diff — NOT the `@depth20` snapshot stream). `s` normally present on
/// diff payloads; fallback derives symbol from the stream name (`"btcusdt@depth@100ms"` ->
/// `"BTCUSDT"`) for defensiveness.
fn parse_depth_diff(stream: &str, data: &serde_json::Value) -> Option<(String, DepthDiff)> {
    let symbol = match data.get("s").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => stream.split('@').next()?.to_uppercase(),
    };

    let u_first = data.get("U")?.as_u64()?;
    let u_final = data.get("u")?.as_u64()?;
    let bids = parse_diff_levels(data.get("b")?)?;
    let asks = parse_diff_levels(data.get("a")?)?;

    Some((
        symbol,
        DepthDiff {
            u_first,
            u_final,
            bids,
            asks,
        },
    ))
}

/// `[["price","qty"], ...]` -> `Vec<(price, qty)>` (fixed-point ×1e8). Zero-qty entries are
/// passed through as-is (0) — `apply_diff_to_book` interprets size==0 as "remove level".
fn parse_diff_levels(levels: &serde_json::Value) -> Option<Vec<(i64, i64)>> {
    let levels = levels.as_array()?;
    let mut out = Vec::with_capacity(levels.len());
    for level in levels {
        let pair = level.as_array()?;
        let price: f64 = pair.first()?.as_str()?.parse().ok()?;
        let size: f64 = pair.get(1)?.as_str()?.parse().ok()?;
        out.push((to_fixed(price), to_fixed(size)));
    }
    Some(out)
}

/// Применить один diff (уже проверенный на непрерывность вызывающим) к книге:
/// upsert/remove по уровням, `last_update_id = diff.u_final`.
fn apply_diff_to_book(book: &mut OrderBook, diff: &DepthDiff) {
    for (price, size) in &diff.bids {
        if *size == 0 {
            book.bids.remove(price);
        } else {
            book.bids.insert(*price, *size);
        }
    }
    for (price, size) in &diff.asks {
        if *size == 0 {
            book.asks.remove(price);
        } else {
            book.asks.insert(*price, *size);
        }
    }
    book.last_update_id = diff.u_final;
}

/// Прогнать входящий diff через sync-автомат символа per Binance snapshot+diff-sync
/// algorithm: пока книга не синхронизирована — буферизовать; при разрыве непрерывности —
/// инвалидировать книгу и пере-синхронизироваться.
fn handle_diff(
    state: &mut SymbolState,
    symbol: &str,
    diff: DepthDiff,
    client: &reqwest::Client,
    pending_snapshots: &mut FuturesUnordered<SnapshotFuture>,
) {
    let action = match &state.book {
        None => DiffAction::Buffer,
        Some(book) => {
            if diff.u_final <= book.last_update_id {
                DiffAction::Skip
            } else if diff.u_first != book.last_update_id + 1 {
                DiffAction::Gap
            } else {
                DiffAction::Apply
            }
        }
    };

    match action {
        DiffAction::Skip => {}
        DiffAction::Buffer => {
            state.pending.push_back(diff);
            if !state.resyncing {
                state.resyncing = true;
                pending_snapshots.push(make_snapshot_future(client.clone(), symbol.to_string()));
            }
        }
        DiffAction::Gap => {
            tracing::warn!(
                symbol = %symbol,
                "venue-binance: depth continuity gap detected, resyncing book"
            );
            state.book = None;
            state.pending.clear();
            state.pending.push_back(diff);
            state.resyncing = true;
            pending_snapshots.push(make_snapshot_future(client.clone(), symbol.to_string()));
        }
        DiffAction::Apply => {
            if let Some(book) = state.book.as_mut() {
                apply_diff_to_book(book, &diff);
            }
        }
    }
}

/// REST snapshot-фетч завершился (успешно или нет) — реконcилировать с буфером
/// `pending` diff-апдейтов, накопленным за время фетча, per Binance algorithm:
/// отбросить устаревшие (`u_final <= lastUpdateId`), проверить, что первый применимый
/// diff покрывает `lastUpdateId+1` (`U <= lastUpdateId+1 <= u`) — иначе снапшот устарел,
/// refetch; иначе применить буфер последовательно и пометить символ синхронизированным.
fn handle_snapshot(
    states: &mut HashMap<String, SymbolState>,
    symbol: String,
    result: anyhow::Result<OrderBook>,
    client: &reqwest::Client,
    pending_snapshots: &mut FuturesUnordered<SnapshotFuture>,
) {
    let Some(state) = states.get_mut(&symbol) else {
        return;
    };

    let mut book = match result {
        Ok(book) => book,
        Err(e) => {
            tracing::warn!(symbol = %symbol, error = %e, "venue-binance: snapshot fetch failed, retrying");
            pending_snapshots.push(make_snapshot_future(client.clone(), symbol.clone()));
            return;
        }
    };

    loop {
        let front = state.pending.front().map(|d| (d.u_final, d.u_first));
        let Some((u_final, u_first)) = front else {
            state.book = Some(book);
            state.resyncing = false;
            return;
        };

        if u_final <= book.last_update_id {
            // Stale relative to the snapshot — discard and keep reconciling.
            state.pending.pop_front();
            continue;
        }

        if u_first > book.last_update_id + 1 {
            // Snapshot (or last-applied diff) is stale relative to the buffer — the gap
            // means we cannot bridge lastUpdateId -> this diff; refetch. Keep buffered
            // diffs — a fresher snapshot may cover them.
            tracing::warn!(
                symbol = %symbol,
                "venue-binance: snapshot stale vs buffered diffs, refetching"
            );
            state.resyncing = true;
            pending_snapshots.push(make_snapshot_future(client.clone(), symbol.clone()));
            return;
        }

        // u_first <= last_update_id+1 <= u_final: applicable (first application) or
        // exactly continuous (subsequent applications from the buffer).
        let diff = state
            .pending
            .pop_front()
            .expect("front() just returned Some");
        apply_diff_to_book(&mut book, &diff);
    }
}

/// Запросить REST snapshot полного стакана для символа (`limit=5000`).
async fn fetch_snapshot(client: &reqwest::Client, symbol: &str) -> anyhow::Result<OrderBook> {
    let url = format!("{REST_DEPTH_BASE}{symbol}&limit={REST_DEPTH_LIMIT}");
    let response = client.get(&url).send().await?.error_for_status()?;
    let snapshot: DepthSnapshotResponse = response.json().await?;

    let mut bids = BTreeMap::new();
    for (price, qty) in &snapshot.bids {
        let price: f64 = price.parse()?;
        let qty: f64 = qty.parse()?;
        let size = to_fixed(qty);
        if size != 0 {
            bids.insert(to_fixed(price), size);
        }
    }

    let mut asks = BTreeMap::new();
    for (price, qty) in &snapshot.asks {
        let price: f64 = price.parse()?;
        let qty: f64 = qty.parse()?;
        let size = to_fixed(qty);
        if size != 0 {
            asks.insert(to_fixed(price), size);
        }
    }

    Ok(OrderBook {
        bids,
        asks,
        last_update_id: snapshot.last_update_id,
    })
}

/// `GET /api/v3/depth` response shape: `{"lastUpdateId":u64,"bids":[["p","q"],...],"asks":[[...]]}`.
#[derive(Deserialize)]
struct DepthSnapshotResponse {
    #[serde(rename = "lastUpdateId")]
    last_update_id: u64,
    bids: Vec<(String, String)>,
    asks: Vec<(String, String)>,
}

/// Обернуть REST snapshot-фетч в boxed future, помеченный символом, для вставки в
/// `FuturesUnordered` рядом с WS read loop.
fn make_snapshot_future(client: reqwest::Client, symbol: String) -> SnapshotFuture {
    Box::pin(async move {
        let result = fetch_snapshot(&client, &symbol).await;
        (symbol, result)
    })
}

/// Эмитировать по одному `L2Snapshot` на синхронизированный символ: полный стакан сжат в
/// 0.02%-от-mid бакеты, обрезан на ±60% от mid. Возвращает `false`, если `tx` закрыт.
async fn emit_book_snapshots(
    states: &HashMap<String, SymbolState>,
    tx: &mpsc::Sender<EventKind>,
) -> bool {
    let ts_exch_ms = now_ms();

    for (symbol, state) in states.iter() {
        let Some(book) = &state.book else {
            continue;
        };
        let Some((&best_bid, _)) = book.bids.iter().next_back() else {
            continue;
        };
        let Some((&best_ask, _)) = book.asks.iter().next() else {
            continue;
        };

        let mid = (best_bid + best_ask) / 2;
        if mid <= 0 {
            continue;
        }

        let bids = bucket_levels(book.bids.iter().rev(), mid);
        let asks = bucket_levels(book.asks.iter(), mid);

        let event = EventKind::md(
            Venue::Binance,
            symbol.clone(),
            MdPayload::L2Snapshot {
                bids,
                asks,
                ts_exch_ms,
            },
        );

        if tx.send(event).await.is_err() {
            return false;
        }
    }

    true
}

/// Сжать одну сторону книги в бакеты по relative-distance от `mid` (ширина
/// `BUCKET_WIDTH`), отбрасывая всё за пределами `MAX_REL_DIST`. `iter` ДОЛЖЕН отдавать
/// уровни в порядке "ближе к mid -> дальше от mid" (bids: `.iter().rev()`; asks:
/// `.iter()`) — представительная цена бакета берётся с первого встреченного (ближайшего)
/// уровня, размер — сумма всех уровней, попавших в бакет.
fn bucket_levels<'a, I>(iter: I, mid: i64) -> Vec<Level>
where
    I: Iterator<Item = (&'a i64, &'a i64)>,
{
    let mut bucket_order: Vec<i64> = Vec::new();
    let mut buckets: HashMap<i64, (i64, i64)> = HashMap::new(); // bucket_idx -> (repr_price, summed_size)

    for (price, size) in iter {
        let dist = (*price - mid).abs() as f64;
        let rel = dist / mid as f64;
        if rel > MAX_REL_DIST {
            continue;
        }

        let bucket_idx = (rel / BUCKET_WIDTH).floor() as i64;
        match buckets.get_mut(&bucket_idx) {
            Some(entry) => entry.1 += *size,
            None => {
                buckets.insert(bucket_idx, (*price, *size));
                bucket_order.push(bucket_idx);
            }
        }
    }

    bucket_order
        .into_iter()
        .map(|idx| {
            let (price, size) = buckets[&idx];
            Level { price, size }
        })
        .collect()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
