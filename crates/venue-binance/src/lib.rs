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
use std::sync::Mutex;
use std::time::{Duration, Instant};
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

/// Аварийный backstop числа уровней на сторону (контракт v3, architect, после §8 + TD-021).
///
/// **Переспека по факту прода (2026-07-14).** Исходный «лик» TD-016 был измерен ВРАНУВШЕЙ
/// метрикой: `docker stats` считает page cache журнала (recorder пишет ~30 MB/мин) — реальный
/// рост кучи +1 MiB/час, а не +8. При 7.5 GiB RAM и 16 MB потребления **память проблемой не
/// является**, а эвикция режет уровни, попадающие в полосы OBI 6-60% — то есть портит
/// ЕДИНСТВЕННЫЙ незаменимый актив ради экономии дешёвого ресурса.
///
/// Приоритет развёрнут: **точность данных > экономия памяти**. Поэтому кап поднят до 200k/сторону
/// (≈10 MB/сторону — ничто на фоне 7.5 GiB) и остаётся ТОЛЬКО аварийным потолком от OOM.
/// Рост числа уровней 5k → 13.8k за 4 ч — вероятно СХОДИМОСТЬ (бутстрап REST-снапшотом даёт
/// top-5000, дальние уровни книга узнаёт из diff-потока), а не лик; отличить можно только по
/// асимптоте (метрика `book levels`) и по recon с биржей (P2.5). До этого — не резать.
///
/// **Не рабочий инструмент, а страховка от OOM.** Рабочая граница — ДИСТАНЦИЯ: уровни дальше
/// `MAX_REL_DIST` (±60%) от mid КНИГИ не эмитятся и ни в один расчёт не входят → эвиктятся
/// безопасно. Всё, что ВНУТРИ окна, входит в суммы полос OBI — резать его нельзя.
/// Срабатывание этого капа = ТРЕВОГА (данные подозрительны), а не штатный режим;
/// эвиктится САМОЕ ДАЛЬНЕЕ от mid, топ книги не трогается.
pub const BACKSTOP_LEVELS_PER_SIDE: usize = 200_000;

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
/// **Контракт эвикции v2 (M-08 task 9b, после C1-блокера PR-гейта):**
///
/// **A. Дифф — не источник истины о том, чего в нём НЕТ.** `@depth@100ms` содержит
///    ТОЛЬКО изменившиеся уровни. Если лучший bid не менялся в окне 100 мс (штатная
///    ситуация) — в диффе его просто нет, и `retain` по `mid` самого диффа стирает
///    ЖИВЫЕ уровни, включая лучший bid. Испорченный стакан уходит в `L2Snapshot`
///    НАВСЕГДА (журнал бессмертен), а RSS/healthcheck остаются зелёными — класс TD-011.
///    Единственное санкционированное удаление по диффу — явный `size == 0` от биржи.
///
/// **B. Граница памяти — ДИСТАНЦИЯ от mid КНИГИ** (best_bid/best_ask ПОСЛЕ применения
///    диффа). Окно — `MAX_REL_DIST` (±60%, то же, что у эмиссии в `bucket_levels`).
///    Уровни ВНЕ окна эвиктятся: они не эмитятся и ни в один расчёт не входят.
///    Уровни ВНУТРИ окна НЕ ТРОГАЮТСЯ: они входят в суммы полос OBI; резать внутри
///    = портить и сигнал, и первичные данные.
///
/// **C. `BACKSTOP_LEVELS_PER_SIDE` = 50_000 — аварийный кап от OOM** (контракт
///    эвикции v2, architect). Если после (B) уровней всё равно больше капа — эвиктится
///    САМОЕ ДАЛЬНЕЕ от mid (top книги не трогается), и логируется `tracing::warn`
///    (срабатывание = «данные подозрительны»; ожидаемый фикс — M-09, инкрементальные
///    bucket-агрегаты вместо сырой книги). Это НЕ рабочий инструмент, а страховка.
pub fn apply_diff_to_book(book: &mut OrderBook, diff: &DepthDiff) {
    // A: только size==0 удаляет уровень; всё остальное — upsert.
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

    // B: mid КНИГИ (после применения диффа).
    let mid: i64 = match (book.bids.iter().next_back(), book.asks.iter().next()) {
        (Some((bb, _)), Some((ba, _))) => (bb + ba) / 2,
        _ => 0,
    };

    if mid > 0 {
        // Окно эвикции — ±MAX_REL_DIST от mid КНИГИ. Уровни, чей |price − mid| > max_dist,
        // НЕ эмитятся и НИГДЕ не считаются → безопасно эвиктятся. Уровни внутри окна
        // входят в суммы полос OBI — НЕ ТРОГАЕМ. Используем .abs() симметрично по обеим
        // сторонам: артефакты crossed-state (bid выше mid / ask ниже mid) эвиктятся как
        // «далёкие» и не отравляют top-of-book.
        //
        // Эвикция идёт через `BTreeMap::range(..lo)` + `range((hi+1)..)` — O(log N + K),
        // где K — число подлежащих удалению. В steady-state (mid дрейфует медленно,
        // большинство уровней внутри окна) K=0 → один bound-check на сторону, ~20 нс.
        // `retain` был бы O(N) даже когда ничего не удаляется, что на 50k уровнях делает
        // бенчмарк `td016_memory_bounded_when_price_drifts_out_of_band` (200k итераций)
        // непрактично медленным. Семантика идентична.
        let max_dist = (mid as f64 * MAX_REL_DIST) as i64;
        let lo = mid.saturating_sub(max_dist);
        let hi = mid.saturating_add(max_dist);
        evict_outside_window(&mut book.bids, lo, hi);
        evict_outside_window(&mut book.asks, lo, hi);
    }

    // C: backstop-кап — аварийный от OOM, эвиктит самые дальние от mid.
    evict_backstop(&mut book.bids, mid, /*is_bids=*/ true);
    evict_backstop(&mut book.asks, mid, /*is_bids=*/ false);
}

/// Эвиктировать из `side` уровни с ценами вне окна `[lo, hi]`. Использует
/// `BTreeMap::range(..lo)` и `range((hi+1)..)` — O(log N + K) суммарно вместо O(N)
/// у `retain`. Семантика: keys < lo удаляются, keys > hi удаляются, keys ∈ [lo, hi]
/// сохраняются (включая границы).
fn evict_outside_window(side: &mut BTreeMap<i64, i64>, lo: i64, hi: i64) {
    let below: Vec<i64> = side.range(..lo).map(|(k, _)| *k).collect();
    for k in below {
        side.remove(&k);
    }
    // `hi + 1` через saturating_add: при hi == i64::MAX возвращаем i64::MAX (диапазон
    // [i64::MAX, ∞) включает только key == i64::MAX — на практике пусто).
    let hi_next = hi.saturating_add(1);
    let above: Vec<i64> = side.range(hi_next..).map(|(k, _)| *k).collect();
    for k in above {
        side.remove(&k);
    }
}

/// Backstop-кап (контракт v2, architect): если после эвикции по дистанции уровней всё
/// равно больше `BACKSTOP_LEVELS_PER_SIDE`, эвиктим САМОЕ ДАЛЬНЕЕ от mid (top книги не
/// трогается), и логируем `tracing::warn` — срабатывание = «данные подозрительны».
///
/// BTreeMap хранит ключи (цены) по возрастанию. На стороне bids ключи лежат ниже mid,
/// на стороне asks — выше mid. «Самое дальнее от mid» для bids = наименьший ключ
/// (`pop_first`), для asks = наибольший (`pop_last`). Без mid (одна сторона книги)
/// эвиктим с произвольного конца: top охраняется неявно тем, что мы эвиктим ровно
/// `len − BACKSTOP_LEVELS_PER_SIDE` уровней с одного конца, а не середину.
fn evict_backstop(side: &mut BTreeMap<i64, i64>, mid: i64, is_bids: bool) {
    if side.len() <= BACKSTOP_LEVELS_PER_SIDE {
        return;
    }
    let count = side.len();
    tracing::warn!(
        side = if is_bids { "bids" } else { "asks" },
        levels = count,
        cap = BACKSTOP_LEVELS_PER_SIDE,
        "venue-binance: backstop cap triggered — book exceeds in-band cap after distance \
         eviction; evicting furthest from mid (data suspicious; expected fix: M-09 \
         incremental bucket-aggregates instead of raw book)"
    );
    let to_drop = side.len() - BACKSTOP_LEVELS_PER_SIDE;
    if mid > 0 {
        if is_bids {
            // bids: ключи < mid, pop_first = наименьший = самый дальний от mid (снизу).
            for _ in 0..to_drop {
                if side.pop_first().is_none() {
                    break;
                }
            }
        } else {
            // asks: ключи > mid, pop_last = наибольший = самый дальний от mid (сверху).
            for _ in 0..to_drop {
                if side.pop_last().is_none() {
                    break;
                }
            }
        }
    } else {
        // Без mid (одна сторона книги / пусто) — эвиктим с наименьшего ключа. Симметрия
        // теряется, но это теоретический путь (прод-WS `@depth` всегда даёт обе стороны).
        for _ in 0..to_drop {
            if side.pop_first().is_none() {
                break;
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

/// Минимальный интервал между per-(venue,symbol) логами числа уровней (контракт
/// эвикции v2, наблюдаемость §D: "не реже 1/мин"). Глобальный статик rate-limit'а по
/// символу — защита от log-storm на каждом тике `emit_interval` (1 Гц).
const LEVELS_LOG_PERIOD: Duration = Duration::from_secs(60);

/// Last-log-timestamp per symbol. Static Mutex — не лучший паттерн в проде, но для
/// диагностической метрики (не критический путь) приемлемо; частота мутаций — 1/мин на
/// символ, contention на Mutex минимальный.
static LAST_LEVELS_LOG: std::sync::LazyLock<Mutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Наблюдаемость TD-016: периодический (≥ 1/мин) `tracing::info` с числом уровней per
/// (venue,symbol). Вызывается из `emit_book_snapshots` (раз в `EMIT_PERIOD` = 1 Гц);
/// фактически логирует ровно 1/мин на символ благодаря rate-limit'у.
fn maybe_log_book_levels(symbol: &str, bids: usize, asks: usize) {
    let mut map = LAST_LEVELS_LOG.lock().expect("levels-log mutex poisoned");
    let now = Instant::now();
    let should_log = map
        .get(symbol)
        .map(|t| now.duration_since(*t) >= LEVELS_LOG_PERIOD)
        .unwrap_or(true);
    if should_log {
        tracing::info!(symbol, bids, asks, "book levels");
        map.insert(symbol.to_string(), now);
    }
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

        // D (контракт эвикции v2): наблюдаемость per (venue,symbol) — число in-band
        // уровней логируется не реже 1/мин. §8 обязан измерять УРОВНИ, а не только RSS:
        // атрибуция лика TD-016 к книге сделана по коду и на проде НЕ доказана; без этого
        // метрика не отделить лик книги от лика HL-адаптера / tracing-буферов.
        maybe_log_book_levels(symbol, book.bids.len(), book.asks.len());

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
