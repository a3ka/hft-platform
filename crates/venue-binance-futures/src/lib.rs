//! venue-binance-futures — Binance USDT-M перп (fstream). M-06 (architect skeleton + venue-dev impl).
//!
//! Emitter-not-owner (docs/fa/venues.md): WS/REST -> parse -> normalize -> MdEvent
//! (`Venue::BinanceFutures`). seq/ts_wall/ts_mono НЕ проставляет — это журнал (JR-I-1),
//! поэтому парс-функции возвращают `MdEvent`, не `Event`.
//!
//! Парс-функции (`parse_force_order` / `parse_depth_snapshot` / `parse_open_interest`) —
//! чистые детерминированные функции границы нормализации, покрытые RED-оракулами
//! `tests/red_parse.rs`. Fail-closed: битая/неожиданная форма → `None` (не паника, не
//! фабрикация правдоподобного значения, VN-I-7).
//!
//! `run` — async-сессия: WS fstream (`<sym>@depth@100ms` + `<sym>@forceOrder`) + REST
//! snapshot-sync (`/fapi/v1/depth`) + REST OI-poll (`/fapi/v1/openInterest`). Одна
//! WS-сессия; reconnect/backoff — забота вызывающего supervisor (как в `venue-binance`).

use contracts::{to_fixed, EventKind, Level, MdEvent, MdPayload, Side, SysEvent, Venue};
use futures_util::stream::FuturesUnordered;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const WS_BASE: &str = "wss://fstream.binance.com/stream?streams=";
const REST_DEPTH_BASE: &str = "https://fapi.binance.com/fapi/v1/depth?symbol=";
const REST_DEPTH_LIMIT: &str = "1000";
const REST_OI_BASE: &str = "https://fapi.binance.com/fapi/v1/openInterest?symbol=";

/// Ширина relative-distance бакета при сжатии полного фьючерс-стакана в L2Snapshot
/// (0.02% от mid — единообразно с `venue-binance`, BUCKET_WIDTH).
const BUCKET_WIDTH: f64 = 0.0002;
/// Максимальная relative-distance от mid, включаемая в L2Snapshot (±60%).
const MAX_REL_DIST: f64 = 0.60;
/// Период эмиссии bounded L2Snapshot per symbol (1с, единообразно с `venue-binance`).
const EMIT_PERIOD: Duration = Duration::from_secs(1);
/// Период опроса REST `/fapi/v1/openInterest` (10с). Binance Futures REST не публикует
/// push-стрим OI — это KISS-выбор; конкретный cadence уточняется recorder'ом (M-06 task 4).
const OI_POLL_PERIOD: Duration = Duration::from_secs(10);

// ─────────────────────────────────────────────────────────────────────────────
// Чистые парс-функции (RED-boundary)
// ─────────────────────────────────────────────────────────────────────────────

/// `forceOrder` (ликвидация) fstream → `MdEvent{BinanceFutures, Liquidation}`.
/// `side` = сторона ФОРС-ордера `o.S`: `SELL` ⟺ ликвидируется LONG, `BUY` ⟺ ликвидируется
/// SHORT (C-003 note: ЛИКВИДИРУЕМАЯ сторона, НЕ агрессор — иначе CVD/liq-flow инвертирует знак).
/// Битая/не-Binance-форма → `None` (VN-I-7).
pub fn parse_force_order(json: &str) -> Option<MdEvent> {
    let v: Value = serde_json::from_str(json).ok()?;
    let o = v.get("o")?;
    let symbol = o.get("s")?.as_str()?.to_string();
    let side = match o.get("S")?.as_str()? {
        "BUY" => Side::Buy,
        "SELL" => Side::Sell,
        // Force order не может иметь иной стороны; неизвестно → fail-closed.
        _ => return None,
    };
    let price: f64 = o.get("p")?.as_str()?.parse().ok()?;
    let size: f64 = o.get("q")?.as_str()?.parse().ok()?;
    let ts_exch_ms = o.get("T")?.as_i64()?;

    Some(MdEvent {
        venue: Venue::BinanceFutures,
        symbol,
        payload: MdPayload::Liquidation {
            price: to_fixed(price),
            size: to_fixed(size),
            side,
            ts_exch_ms,
        },
    })
}

/// `/fapi/v1/depth` снапшот → `MdEvent{BinanceFutures, L2Snapshot}`. `ts_exch_ms` = поле `T`
/// (transact-time снапшота; `E` — event-time диспатча). Битая форма → `None`.
pub fn parse_depth_snapshot(symbol: &str, json: &str) -> Option<MdEvent> {
    let v: Value = serde_json::from_str(json).ok()?;
    let bids = parse_l2_levels(v.get("bids")?)?;
    let asks = parse_l2_levels(v.get("asks")?)?;
    let ts_exch_ms = v.get("T")?.as_i64()?;

    Some(MdEvent {
        venue: Venue::BinanceFutures,
        symbol: symbol.to_string(),
        payload: MdPayload::L2Snapshot {
            bids,
            asks,
            ts_exch_ms,
        },
    })
}

/// `/fapi/v1/openInterest` → `MdEvent{BinanceFutures, OpenInterest}`. `oi_e8` = БАЗОВЫЙ
/// актив ×1e8. Битая форма → `None`.
pub fn parse_open_interest(symbol: &str, json: &str) -> Option<MdEvent> {
    let v: Value = serde_json::from_str(json).ok()?;
    let oi: f64 = v.get("openInterest")?.as_str()?.parse().ok()?;
    let ts_exch_ms = v.get("time")?.as_i64()?;

    Some(MdEvent {
        venue: Venue::BinanceFutures,
        symbol: symbol.to_string(),
        payload: MdPayload::OpenInterest {
            oi_e8: to_fixed(oi),
            ts_exch_ms,
        },
    })
}

/// `[["price","qty"], ...]` → `Vec<Level>` (fixed-point ×1e8). Невалидный уровень — весь
/// снапшот `None` (транзакционно: битый стакан полезнее дропнуть, чем синтезировать).
fn parse_l2_levels(levels: &Value) -> Option<Vec<Level>> {
    let arr = levels.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for level in arr {
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

// ─────────────────────────────────────────────────────────────────────────────
// Async runner (WS depth@100ms + WS forceOrder + REST snapshot/OI poll)
// Зеркалит `venue-binance::run` — одна сессия, supervisor-pattern снаружи.
// ─────────────────────────────────────────────────────────────────────────────

/// Локальная копия полного фьючерс-стакана одного символа. price/size — fixed-point ×1e8.
struct OrderBook {
    bids: BTreeMap<i64, i64>,
    asks: BTreeMap<i64, i64>,
    last_update_id: u64,
    /// `E` последнего применённого WS diff'а, мс. `0` — только REST-бутстрап без diff'ов
    /// (нет биржевого времени → символ НЕ эмитится в `emit_book_snapshots`).
    last_event_time_ms: i64,
}

/// Один WS `@depth@100ms` diff (fstream формат идентичен spot: `U`/`u` update IDs,
/// `b`/`a` — массивы `[price, qty]`-пар). `size==0` — удалить уровень.
struct DepthDiff {
    event_time_ms: i64,
    u_first: u64,
    u_final: u64,
    bids: Vec<(i64, i64)>,
    asks: Vec<(i64, i64)>,
}

/// Состояние sync-конечного-автомата одного символа (см. `venue-binance`).
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

enum DiffAction {
    Skip,
    Buffer,
    Gap,
    Apply,
}

type SnapshotFuture = Pin<Box<dyn Future<Output = (String, anyhow::Result<OrderBook>)> + Send>>;

/// Запустить сессию Binance USDT-M futures WS+REST. Шлёт `EventKind::Md(..)` в `tx`;
/// `Sys(ConnUp(BinanceFutures))` — сразу после успешного WS-коннекта. Возвращает `Ok(())`
/// при штатном закрытии/дисконнекте/уходе получателя; `Err(_)` — при ошибке коннекта.
/// Reconnect/backoff — забота supervisor'а снаружи (emitter-not-owner, как в `venue-binance`).
pub async fn run(tx: mpsc::Sender<EventKind>, symbols: Vec<String>) -> anyhow::Result<()> {
    let mut streams = Vec::with_capacity(symbols.len() * 2);
    for s in &symbols {
        let lower = s.to_lowercase();
        streams.push(format!("{lower}@depth@100ms"));
        streams.push(format!("{lower}@forceOrder"));
    }
    let url = format!("{WS_BASE}{}", streams.join("/"));

    let (ws_stream, _response) = tokio_tungstenite::connect_async(&url).await?;
    let (mut write, mut read) = ws_stream.split();

    if tx
        .send(EventKind::Sys(SysEvent::ConnUp(Venue::BinanceFutures)))
        .await
        .is_err()
    {
        return Ok(());
    }

    let client = reqwest::Client::new();
    let mut states: HashMap<String, SymbolState> = HashMap::new();
    let mut pending_snapshots: FuturesUnordered<SnapshotFuture> = FuturesUnordered::new();

    // Стартовая синхронизация depth по каждому символу (REST snapshot + буфер diff'ов).
    for s in &symbols {
        let symbol = s.to_uppercase();
        let mut state = SymbolState::new();
        state.resyncing = true;
        pending_snapshots.push(make_snapshot_future(client.clone(), symbol.clone()));
        states.insert(symbol, state);
    }

    let mut emit_interval = tokio::time::interval(EMIT_PERIOD);
    emit_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // OI poll: первый tick `interval` срабатывает мгновенно — это намеренно (наблюдение
    // OI сразу после синхронизации WS, не через OI_POLL_PERIOD); дальнейшие тики — каждые
    // OI_POLL_PERIOD. `Burst` — дефолт, подходит.
    let mut oi_interval = tokio::time::interval(OI_POLL_PERIOD);

    loop {
        tokio::select! {
            msg = read.next() => {
                let Some(msg) = msg else { return Ok(()) };
                let msg = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::debug!(error = %e, "venue-binance-futures: WS read error, ending session");
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
            _ = oi_interval.tick() => {
                if !poll_open_interest(&client, &symbols, &tx).await {
                    return Ok(());
                }
            }
        }
    }
}

/// Разобрать одно combined-stream текстовое сообщение и применить эффект:
/// `forceOrder` → немедленная эмиссия `Liquidation`; `depth` diff → прогон через sync-автомат.
/// Возвращает `false`, если `tx` закрыт (получатель ушёл).
async fn handle_text_message(
    text: &str,
    tx: &mpsc::Sender<EventKind>,
    states: &mut HashMap<String, SymbolState>,
    client: &reqwest::Client,
    pending_snapshots: &mut FuturesUnordered<SnapshotFuture>,
) -> bool {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(error = %e, raw = %text, "venue-binance-futures: malformed JSON, skipping");
            return true;
        }
    };

    let stream = match value.get("stream").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            tracing::debug!(raw = %text, "venue-binance-futures: message without 'stream', skipping");
            return true;
        }
    };
    let data = match value.get("data") {
        Some(d) => d,
        None => {
            tracing::debug!(raw = %text, "venue-binance-futures: message without 'data', skipping");
            return true;
        }
    };

    if stream.ends_with("@forceOrder") {
        // Combined-stream fstream оборачивает событие в `{"stream":"...", "data":{...}}`,
        // где `data` уже форма fstream — его прямо скармливаем `parse_force_order`.
        let raw = match serde_json::to_string(data) {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(error = %e, "venue-binance-futures: failed to re-serialize forceOrder data");
                return true;
            }
        };
        if let Some(event) = parse_force_order(&raw) {
            if tx.send(EventKind::Md(event)).await.is_err() {
                return false;
            }
        } else {
            tracing::debug!(raw = %raw, "venue-binance-futures: malformed forceOrder, skipping");
        }
    } else if stream.contains("@depth") {
        if let Some((symbol, diff)) = parse_depth_diff(stream, data) {
            let state = states
                .entry(symbol.clone())
                .or_insert_with(SymbolState::new);
            handle_diff(state, &symbol, diff, client, pending_snapshots);
        } else {
            tracing::debug!(stream = %stream, "venue-binance-futures: unparseable depth frame");
        }
    } else {
        tracing::debug!(stream = %stream, "venue-binance-futures: unrecognized stream, skipping");
    }

    true
}

/// fstream `@depth@100ms` diff payload (формат идентичен spot): `U`/`u` + `b`/`a`.
fn parse_depth_diff(stream: &str, data: &Value) -> Option<(String, DepthDiff)> {
    let symbol = match data.get("s").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => stream.split('@').next()?.to_uppercase(),
    };
    let u_first = data.get("U")?.as_u64()?;
    let u_final = data.get("u")?.as_u64()?;
    let bids = parse_diff_levels(data.get("b")?)?;
    let asks = parse_diff_levels(data.get("a")?)?;
    let event_time_ms = data.get("E").and_then(|v| v.as_i64()).unwrap_or(0);
    Some((
        symbol,
        DepthDiff {
            event_time_ms,
            u_first,
            u_final,
            bids,
            asks,
        },
    ))
}

/// `[["price","qty"], ...]` → `Vec<(price_fixed, qty_fixed)>`. size==0 — удалить.
fn parse_diff_levels(levels: &Value) -> Option<Vec<(i64, i64)>> {
    let arr = levels.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for level in arr {
        let pair = level.as_array()?;
        let price: f64 = pair.first()?.as_str()?.parse().ok()?;
        let size: f64 = pair.get(1)?.as_str()?.parse().ok()?;
        out.push((to_fixed(price), to_fixed(size)));
    }
    Some(out)
}

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
    book.last_event_time_ms = diff.event_time_ms;
}

/// Прогнать diff через sync-автомат (см. Binance snapshot+diff-sync docs).
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
                "venue-binance-futures: depth continuity gap detected, resyncing book"
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

/// REST snapshot завершился — реконcилировать с буфером diff'ов (Binance algorithm).
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
            tracing::warn!(symbol = %symbol, error = %e, "venue-binance-futures: snapshot fetch failed, retrying");
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
            state.pending.pop_front();
            continue;
        }
        if u_first > book.last_update_id + 1 {
            tracing::warn!(
                symbol = %symbol,
                "venue-binance-futures: snapshot stale vs buffered diffs, refetching"
            );
            state.resyncing = true;
            pending_snapshots.push(make_snapshot_future(client.clone(), symbol.clone()));
            return;
        }
        let diff = state
            .pending
            .pop_front()
            .expect("front() just returned Some");
        apply_diff_to_book(&mut book, &diff);
    }
}

/// `GET /fapi/v1/depth` → `OrderBook`. `T` снапшота НЕ несёт биржевого времени —
/// оно появляется с первым применённым WS diff'ом (Binance docs).
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
        last_event_time_ms: 0,
    })
}

#[derive(Deserialize)]
struct DepthSnapshotResponse {
    #[serde(rename = "lastUpdateId")]
    last_update_id: u64,
    bids: Vec<(String, String)>,
    asks: Vec<(String, String)>,
}

fn make_snapshot_future(client: reqwest::Client, symbol: String) -> SnapshotFuture {
    Box::pin(async move {
        let result = fetch_snapshot(&client, &symbol).await;
        (symbol, result)
    })
}

/// Периодический REST-опрос `/fapi/v1/openInterest` per symbol → `MdEvent::OpenInterest`.
/// Fail-closed: HTTP/parse failure → логируем + skip конкретный символ (не паникуем,
/// polling продолжится со следующего тика — VN-I-7).
async fn poll_open_interest(
    client: &reqwest::Client,
    symbols: &[String],
    tx: &mpsc::Sender<EventKind>,
) -> bool {
    for symbol in symbols {
        let sym = symbol.to_uppercase();
        let url = format!("{REST_OI_BASE}{sym}");
        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(symbol = %sym, error = %e, "venue-binance-futures: OI poll HTTP error, skipping");
                continue;
            }
        };
        let body = match resp.error_for_status() {
            Ok(r) => match r.text().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::debug!(symbol = %sym, error = %e, "venue-binance-futures: OI poll body read, skipping");
                    continue;
                }
            },
            Err(e) => {
                tracing::debug!(symbol = %sym, error = %e, "venue-binance-futures: OI poll HTTP status, skipping");
                continue;
            }
        };
        match parse_open_interest(&sym, &body) {
            Some(event) => {
                if tx.send(EventKind::Md(event)).await.is_err() {
                    return false;
                }
            }
            None => {
                tracing::debug!(symbol = %sym, body = %body, "venue-binance-futures: OI poll malformed, skipping");
            }
        }
    }
    true
}

/// Эмитировать по одному `L2Snapshot` на синхронизированный символ. Символ только после
/// REST-бутстрапа (без применённых WS diff'ов) не имеет биржевого времени — не выдумываем.
async fn emit_book_snapshots(
    states: &HashMap<String, SymbolState>,
    tx: &mpsc::Sender<EventKind>,
) -> bool {
    for (symbol, state) in states.iter() {
        let Some(book) = &state.book else {
            continue;
        };
        let ts_exch_ms = book.last_event_time_ms;
        if ts_exch_ms == 0 {
            continue;
        }
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
            Venue::BinanceFutures,
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

/// Сжать одну сторону книги в бакеты по relative-distance от `mid` (как `venue-binance`).
fn bucket_levels<'a, I>(iter: I, mid: i64) -> Vec<Level>
where
    I: Iterator<Item = (&'a i64, &'a i64)>,
{
    let mut bucket_order: Vec<i64> = Vec::new();
    let mut buckets: HashMap<i64, (i64, i64)> = HashMap::new();
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

/// `!markPrice@arr` / `<symbol>@markPrice` (markPriceUpdate) → `MdEvent{BinanceFutures, Funding}`.
/// `rate_e8` = поле `r` (funding rate) ×1e8; `ts_exch_ms` = `E` (event time); symbol = `s`.
/// Нужен как РЕАЛЬНЫЙ вход для derive funding-breadth (M-06 #5) — иначе breadth без данных.
pub fn parse_mark_price(_json: &str) -> Option<MdEvent> {
    None // STUB — venue-dev (M-06 funding-parser task, N3)
}
