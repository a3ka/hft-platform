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
use std::time::Duration;
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

/// Кап числа уровней на одну сторону (`bids`/`asks`) в локальной full-book книге.
/// Равен глубине REST-снапшота (`limit=5000`, `REST_DEPTH_LIMIT`), из которого книга
/// бутстрапится. При превышении капа самые дальние от mid уровни ЭВИКТЯТСЯ при каждом
/// `apply_diff_to_book`. Корневой фикс TD-016: без эвикции книга копит «мёртвые»
/// уровни, из которых цена давно ушла (апдейтов биржа больше не шлёт, `size==0` не
/// приходит) — измерено: ~+6.5 MiB/час на проде. Хранить дальше `MAX_BOOK_LEVELS_PER_SIDE`
/// от mid бессмысленно: (а) за пределами капа в эмиссию (`±60%`, bucketed) они не
/// влияют; (б) восстановить их из REST-снапшота всё равно нельзя — снапшот сам
/// ограничен той же глубиной 5000.
const MAX_BOOK_LEVELS_PER_SIDE: usize = 5000;

/// Локальная копия полного стакана одного символа. price/size — fixed-point ×1e8
/// (per `contracts::PRICE_SCALE`), ключ `BTreeMap` — цена, что даёт бесплатную
/// сортировку + O(log n) upsert/remove на diff-апдейте.
pub struct OrderBook {
    pub bids: BTreeMap<i64, i64>,
    pub asks: BTreeMap<i64, i64>,
    pub last_update_id: u64,
    /// Биржевое время (`E`) последнего применённого WS diff-апдейта, мс since epoch.
    /// `0` означает "ещё ни один diff не применялся" (только REST-бутстрап) — книга
    /// в этом состоянии не несёт биржевого времени и НЕ эмитится в `emit_book_snapshots`.
    pub last_event_time_ms: i64,
}

/// Один WS `@depth@100ms` diff-апдейт после парсинга. `u_first`/`u_final` — `U`/`u` из
/// payload Binance. size==0 в уровне означает "удалить уровень" (per Binance diff-sync
/// docs), это разворачивается в `apply_diff_to_book`. `event_time_ms` — биржевое время
/// `E` из payload (0, если отсутствует — не паникуем, не подставляем now()).
pub struct DepthDiff {
    pub event_time_ms: i64,
    pub u_first: u64,
    pub u_final: u64,
    pub bids: Vec<(i64, i64)>,
    pub asks: Vec<(i64, i64)>,
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
    // "E" отсутствует → 0 (не ошибка парсинга; not-yet-observed биржевое время).
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
///
/// **Кап `MAX_BOOK_LEVELS_PER_SIDE` (TD-016, M-08 task 9):** после upsert/remove книга
/// ОБЯЗАНА быть ограничена в размере — иначе происходит лик (+6.5 MiB/час на проде по
/// замерам reviewer), потому что биржа никогда не шлёт size=0 для уровней, из которых
/// цена давно ушла (их просто не существует в её представлении). Стратегия эвикции:
///
/// 1. **Activity reference = `diff_mid`** = (max bid in this diff + min ask in this diff) / 2
///    — точка, вокруг которой лежит новый снимок книги. Использовать (best_bid +
///    best_ask) / 2 из самой книги НЕЛЬЗЯ: при дрейфе цены вверх (как у
///    `crates/venues/tests/red_book_bounded.rs`) книга становится crossed (старые asks
///    ниже новых bids), и computed-mid становится бессмысленным.
/// 2. **Strict-side filter:** bids ≥ diff_mid и asks ≤ diff_mid — артефакты crossed/drift
///    состояния, удаляются. В нормальной (не-crossed) книге фильтр не трогает ничего
///    (bids всегда < mid, asks всегда > mid), но при дрейфе именно он отделяет
///    «актуальные» уровни рядом с текущим mid от накопленного исторического хвоста.
/// 3. **Cap:** `MAX_BOOK_LEVELS_PER_SIDE` = 5000 (= глубина REST-снапшота, из которого
///    книга бутстрапится; хранить дальше 5000 от mid — бессмысленно: (а) в эмиссию ±60%
///    bucketed не влияет, (б) восстановить из REST нельзя). Эвикция идёт ХВОСТОМ:
///    `pop_first` (min bid = самая дальняя ниже mid) и `pop_last` (max ask = самая
///    дальняя выше mid). Топ книги (лучший bid/ask) и производные (OBI-полосы) не
///    деградируют.
///
/// Fallback: односторонний/пустой diff → activity reference недоступен → только пп. 2-3
/// стандартного cap (по min/max BTreeMap без фильтра по стороне). Это не идеал, но
/// безопасный деградированный путь до прихода двустороннего диффа.
pub fn apply_diff_to_book(book: &mut OrderBook, diff: &DepthDiff) {
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

    // ── TD-016: ограничение книги ────────────────────────────────────────
    // 1) activity reference из самого диффа (он имеет обе стороны в нормальном
    //    `@depth` стриме; для пустых/односторонних — fallback ниже на cap-только).
    let diff_mid: Option<i64> = match (
        diff.bids.iter().map(|(p, _)| *p).max(),
        diff.asks.iter().map(|(p, _)| *p).min(),
    ) {
        (Some(bb), Some(ba)) => {
            let m = (bb + ba) / 2;
            if m <= 0 {
                None
            } else {
                Some(m)
            }
        }
        _ => None,
    };

    if let Some(diff_mid) = diff_mid {
        // 2) Strict-side filter: bids ≥ diff_mid и asks ≤ diff_mid — нарушение нормальной
        //    топологии книги (bids должны быть ниже mid, asks — выше), удаляются. Это
        //    очищает остатки crossed-состояния при дрейфе.
        book.bids.retain(|p, _| *p < diff_mid);
        book.asks.retain(|p, _| *p > diff_mid);

        // 3) Cap до MAX_BOOK_LEVELS_PER_SIDE per side, эвикция ХВОСТА от mid.
        if book.bids.len() > MAX_BOOK_LEVELS_PER_SIDE {
            let to_drop = book.bids.len() - MAX_BOOK_LEVELS_PER_SIDE;
            for _ in 0..to_drop {
                if book.bids.pop_first().is_none() {
                    break;
                }
            }
        }
        if book.asks.len() > MAX_BOOK_LEVELS_PER_SIDE {
            let to_drop = book.asks.len() - MAX_BOOK_LEVELS_PER_SIDE;
            for _ in 0..to_drop {
                if book.asks.pop_last().is_none() {
                    break;
                }
            }
        }
    } else {
        // Fallback для одностороннего/пустого диффа: cap-только без side-filter'а
        // (нельзя вычислить diff_mid → нельзя фильтровать «неправильную сторону»).
        // В прод-WS-стриме `@depth` всегда даёт обе стороны в одном сообщении, так что
        // этот путь — теоретический, но без него возможна регрессия при маломальски
        // неполном payload'е (ретраи, частичные апдейты по WS).
        if book.bids.len() > MAX_BOOK_LEVELS_PER_SIDE {
            let to_drop = book.bids.len() - MAX_BOOK_LEVELS_PER_SIDE;
            for _ in 0..to_drop {
                if book.bids.pop_first().is_none() {
                    break;
                }
            }
        }
        if book.asks.len() > MAX_BOOK_LEVELS_PER_SIDE {
            let to_drop = book.asks.len() - MAX_BOOK_LEVELS_PER_SIDE;
            for _ in 0..to_drop {
                if book.asks.pop_last().is_none() {
                    break;
                }
            }
        }
    }
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
        // REST snapshot несёт lastUpdateId, но НЕ биржевое время — оно появляется только
        // с первым применённым WS diff'ом (per apply_diff_to_book).
        last_event_time_ms: 0,
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
    for (symbol, state) in states.iter() {
        let Some(book) = &state.book else {
            continue;
        };
        // Биржевое время последнего применённого diff'а. Символ, синхронизированный
        // ТОЛЬКО через REST-бутстрап (ни одного WS diff ещё не применялось), биржевого
        // времени не несёт — не выдумываем его через now_ms(), просто не эмитим символ.
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

// ─────────────────────────────────────────────────────────────────────────────
// SACRED (architect-owned; venue-dev не меняет): RED-тесты фикса ts_exch_ms=0
// у L2Snapshot (SESSION-HANDOFF §5 п.4). Спецификация: снапшот несёт БИРЖЕВОЕ
// время «E» последнего применённого diff'а, per-symbol; без применённых diff'ов
// (только REST-бутстрап — биржевого времени нет) символ НЕ эмитится (время не
// выдумываем). Inline-модуль: тестируемые функции приватны (конвенция крейта).
// Тесты компилируются после добавления полей event_time_ms/last_event_time_ms
// (compile-RED на текущем коде — named в коммите фикс-цикла).
#[cfg(test)]
mod ts_exch_tests {
    use super::*;

    fn diff_json(e: i64, u_first: u64, u_final: u64) -> serde_json::Value {
        serde_json::json!({
            "e": "depthUpdate", "E": e, "s": "BTCUSDT",
            "U": u_first, "u": u_final,
            "b": [["100.0", "2.0"]],
            "a": [["101.0", "3.0"]]
        })
    }

    #[test]
    fn parse_depth_diff_extracts_event_time() {
        let (sym, diff) =
            parse_depth_diff("btcusdt@depth@100ms", &diff_json(1_752_000_000_123, 5, 7))
                .expect("валидный diff парсится");
        assert_eq!(sym, "BTCUSDT");
        assert_eq!(
            diff.event_time_ms, 1_752_000_000_123,
            "E обязан извлекаться"
        );
        // отсутствие E → 0 (не паника, не now)
        let mut v = diff_json(0, 8, 9);
        v.as_object_mut().unwrap().remove("E");
        let (_, d2) = parse_depth_diff("btcusdt@depth@100ms", &v).expect("diff без E валиден");
        assert_eq!(d2.event_time_ms, 0);
    }

    #[test]
    fn apply_diff_propagates_event_time_to_book() {
        let mut book = OrderBook {
            bids: std::collections::BTreeMap::new(),
            asks: std::collections::BTreeMap::new(),
            last_update_id: 4,
            last_event_time_ms: 0,
        };
        let (_, diff) = parse_depth_diff("btcusdt@depth@100ms", &diff_json(777_000, 5, 6)).unwrap();
        apply_diff_to_book(&mut book, &diff);
        assert_eq!(book.last_update_id, 6);
        assert_eq!(
            book.last_event_time_ms, 777_000,
            "время последнего diff'а — в книге"
        );
    }

    #[tokio::test]
    async fn emit_uses_last_diff_event_time_not_wallclock() {
        let mut synced = SymbolState::new();
        let mut book = OrderBook {
            bids: std::collections::BTreeMap::new(),
            asks: std::collections::BTreeMap::new(),
            last_update_id: 10,
            last_event_time_ms: 0,
        };
        let (_, diff) =
            parse_depth_diff("btcusdt@depth@100ms", &diff_json(1_600_000_000_000, 11, 12)).unwrap();
        apply_diff_to_book(&mut book, &diff);
        synced.book = Some(book);

        // символ ТОЛЬКО после REST-бутстрапа: diff'ы не применялись → биржевого времени нет
        let mut bootstrap_only = SymbolState::new();
        bootstrap_only.book = Some(OrderBook {
            bids: [(to_fixed(50.0), to_fixed(1.0))].into_iter().collect(),
            asks: [(to_fixed(51.0), to_fixed(1.0))].into_iter().collect(),
            last_update_id: 1,
            last_event_time_ms: 0,
        });

        let mut states = HashMap::new();
        states.insert("BTCUSDT".to_string(), synced);
        states.insert("NODIFF".to_string(), bootstrap_only);

        let (tx, mut rx) = mpsc::channel(16);
        assert!(emit_book_snapshots(&states, &tx).await);
        drop(tx);

        let mut got = Vec::new();
        while let Some(ev) = rx.recv().await {
            got.push(ev);
        }
        assert_eq!(
            got.len(),
            1,
            "символ без применённых diff'ов НЕ эмитится (время не выдумываем)"
        );
        let EventKind::Md(md) = &got[0] else {
            panic!("ожидали Md")
        };
        assert_eq!(md.symbol, "BTCUSDT");
        let MdPayload::L2Snapshot { ts_exch_ms, .. } = &md.payload else {
            panic!("ожидали L2Snapshot")
        };
        assert_eq!(
            *ts_exch_ms, 1_600_000_000_000,
            "ts_exch_ms = E последнего diff'а, НЕ now_ms()"
        );
    }
}
