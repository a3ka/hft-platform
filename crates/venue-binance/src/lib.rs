//! venue-binance — адаптер Binance (docs/fa/venues.md; recon /tmp/hft_dataplane_recon.md §A+§D).
//!
//! Контракт: `run` подключается к Binance WS (combined-stream), подписывается на
//! `{symbol}@trade` + `{symbol}@depth@100ms` (RAW DIFF, не depth20) по символам, парсит,
//! ведёт ПОЛНЫЙ стакан на клиенте через стандартный snapshot+diff-sync алгоритм Binance
//! (REST `/api/v3/depth` snapshot + непрерывная последовательность WS diff-апдейтов по
//! `U`/`u`), нормализует в периодический `contracts::MdEvent::L2Snapshot` и шлёт в `tx`.
//! ОДНА сессия соединения — reconnect/backoff делает вызывающий supervisor, а не этот
//! модуль. Emitter-not-owner (VN-I): seq не проставляет, риск/позиции не трогает.

use contracts::{to_fixed, EventKind, Level, MdEvent, MdPayload, Side, SysEvent, Venue};
use futures_util::stream::FuturesUnordered;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, VecDeque};

pub mod recon;
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

/// Эффект sync-state-машины `SpotSession` (M-45, паттерн TD-014 из
/// `venue-binance-futures::FuturesSession`): то, что она «хочет» эмитить во внешний мир
/// или запросить у I/O-слоя. `run()` ОБЯЗАН делегировать в `SpotSession` — иначе
/// allow-list не гарантирован на живом пути (D-1, C-049).
///
/// * `Emit(MdEvent)` — нормализованное событие для журнала (Trade/L2Delta/L2Snapshot).
/// * `FetchSnapshot { symbol, after }` — запросить REST-снапшот. Спот НЕ имеет backoff
///   (TD-013 — futures-специфика, вне объёма M-45): `after` всегда `Duration::ZERO`,
///   байт-в-байт сегодняшнее поведение (fire immediately).
#[derive(Debug, Clone)]
pub enum SessionEffect {
    Emit(MdEvent),
    FetchSnapshot { symbol: String, after: Duration },
}

/// Тестируемая sync-state-машина `venue-binance` (СПОТ) БЕЗ сети/каналов (M-45 task 3c,
/// паттерн TD-014 из `venue-binance-futures::FuturesSession`). Инкапсулирует per-symbol
/// `SymbolState` (буфер diff'ов + book) и allow-list эмиссии `L2Delta`. Никогда не ходит
/// в сеть и не шлёт в mpsc — только накапливает состояние и возвращает
/// `Vec<SessionEffect>`; `run()` — тонкая I/O-обёртка, исполняющая эффекты.
pub struct SpotSession {
    states: HashMap<String, SymbolState>,
    /// M-45: allow-list эмиссии `L2Delta` (явный параметр, независимый от книг символов
    /// — символ может вестись в книге и НЕ капчиться в журнал сырых дельт).
    l2delta_allow: Vec<String>,
}

impl SpotSession {
    /// Новая сессия для подписки `subs`. Per-symbol состояние заводится СРАЗУ с
    /// `resyncing = true` (байт-в-байт прежний bootstrap `run()`'а) — `bootstrap()`
    /// возвращает соответствующие `FetchSnapshot`-эффекты. `l2delta_allow` — allow-list
    /// эмиссии `L2Delta`, задаётся ЯВНЫМ параметром (M-45 API-контракт).
    pub fn new_with_l2delta(subs: &[String], l2delta_allow: &[String]) -> Self {
        let mut states = HashMap::new();
        for s in subs {
            let symbol = s.to_uppercase();
            let mut state = SymbolState::new();
            state.resyncing = true;
            states.insert(symbol, state);
        }
        Self {
            states,
            l2delta_allow: l2delta_allow.to_vec(),
        }
    }

    /// Эффекты стартовой синхронизации: REST snapshot-фетч для каждого символа из
    /// `subs` (состояние уже помечено `resyncing` конструктором — здесь только запрос).
    pub fn bootstrap(&self) -> Vec<SessionEffect> {
        let mut symbols: Vec<&String> = self.states.keys().collect();
        symbols.sort(); // JR-I/детерминизм: без сортировки порядок эффектов зависел бы
                         // от итерации HashMap (недетерминизм, CLAUDE.md).
        symbols
            .into_iter()
            .map(|symbol| SessionEffect::FetchSnapshot {
                symbol: symbol.clone(),
                after: Duration::ZERO,
            })
            .collect()
    }

    /// Обработать одно combined-stream текстовое сообщение спота. Возвращает эффекты —
    /// I/O-обёртка (`run`) их применяет: эмитит в `tx` / запрашивает REST-снапшот.
    /// Sync, БЕЗ сети/каналов (M-45 D-1, C-049): решающий оракул O-8 дёргает эту функцию
    /// сырым wire-текстом напрямую, проверяя РЕАЛЬНОЕ поведение, а не структуру кода.
    pub fn on_ws_text(&mut self, text: &str) -> Vec<SessionEffect> {
        let mut effects = Vec::new();
        let value: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, raw = %text, "venue-binance: malformed JSON, skipping");
                return effects;
            }
        };

        let stream = match value.get("stream").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                tracing::debug!(raw = %text, "venue-binance: message without 'stream', skipping");
                return effects;
            }
        };
        let data = match value.get("data") {
            Some(d) => d,
            None => {
                tracing::debug!(raw = %text, "venue-binance: message without 'data', skipping");
                return effects;
            }
        };

        if stream.ends_with("@trade") {
            if let Some(EventKind::Md(md)) = parse_trade(data) {
                effects.push(SessionEffect::Emit(md));
            }
        } else if stream.contains("@depth") {
            // M-45: решение об эмиссии L2Delta делегировано ЕДИНСТВЕННОЙ точке
            // `l2delta_emission_for` (allow-list из конфига, не хардкод) — капчим
            // КАЖДЫЙ распарсенный diff как сырое L2Delta-событие независимо от
            // book-sync FSM (ground-truth рыночное событие), но только если символ
            // разрешён `l2delta_allow`.
            if let Some(EventKind::Md(md)) = l2delta_emission_for(stream, data, &self.l2delta_allow) {
                effects.push(SessionEffect::Emit(md));
            }
            if let Some((symbol, diff)) = parse_depth_diff(stream, data) {
                let state = self
                    .states
                    .entry(symbol.clone())
                    .or_insert_with(SymbolState::new);
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
                            effects.push(SessionEffect::FetchSnapshot {
                                symbol: symbol.clone(),
                                after: Duration::ZERO,
                            });
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
                        effects.push(SessionEffect::FetchSnapshot {
                            symbol: symbol.clone(),
                            after: Duration::ZERO,
                        });
                    }
                    DiffAction::Apply => {
                        if let Some(book) = state.book.as_mut() {
                            apply_diff_to_book(book, &diff);
                        }
                    }
                }
            } else {
                tracing::debug!(stream = %stream, "venue-binance: unparseable depth frame");
            }
        } else {
            tracing::debug!(stream = %stream, "venue-binance: unrecognized stream, skipping");
        }

        effects
    }

    /// REST snapshot завершился (успешно или нет) — реконcилировать с буфером `pending`
    /// (эквивалент прежнего свободного `handle_snapshot`, теперь метод сессии). Парсинг
    /// HTTP-ответа остаётся в I/O-обёртке (`fetch_snapshot`) — сессия работает с уже
    /// распарсенным `OrderBook`, никакого HTTP/JSON внутри seam'а.
    pub fn on_snapshot_result(
        &mut self,
        symbol: &str,
        result: Result<OrderBook, ()>,
    ) -> Vec<SessionEffect> {
        let mut effects = Vec::new();
        let Some(state) = self.states.get_mut(symbol) else {
            return effects;
        };

        let mut book = match result {
            Ok(book) => book,
            Err(()) => {
                tracing::warn!(symbol = %symbol, "venue-binance: snapshot fetch failed, retrying");
                effects.push(SessionEffect::FetchSnapshot {
                    symbol: symbol.to_string(),
                    after: Duration::ZERO,
                });
                return effects;
            }
        };

        loop {
            let front = state.pending.front().map(|d| (d.u_final, d.u_first));
            let Some((u_final, u_first)) = front else {
                state.book = Some(book);
                state.resyncing = false;
                return effects;
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
                effects.push(SessionEffect::FetchSnapshot {
                    symbol: symbol.to_string(),
                    after: Duration::ZERO,
                });
                return effects;
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

    /// Периодический тик: эмитит по одному `L2Snapshot` на синхронизированный символ
    /// (эквивалент прежней свободной `emit_book_snapshots`, теперь метод сессии —
    /// возвращает эффекты вместо прямого `tx.send`). Делегирует в
    /// `compute_book_snapshot_effects` — тот же хелпер использует legacy-обёртка
    /// `emit_book_snapshots` (сохранена для SACRED-теста `ts_exch_tests` ниже).
    pub fn tick(&self) -> Vec<SessionEffect> {
        compute_book_snapshot_effects(&self.states)
    }
}

/// Чистое вычисление `L2Snapshot`-эффектов из states (без I/O): полный стакан сжат в
/// 0.02%-от-mid бакеты, обрезан на ±60% от mid; символ без применённого WS diff'а
/// (только REST-бутстрап, биржевого времени нет) не эмитится. Общий хелпер для
/// `SpotSession::tick` и legacy free-fn `emit_book_snapshots` (SACRED-тест
/// `ts_exch_tests` дёргает последнюю напрямую сигнатурой `states: &HashMap<...>` —
/// не трогать тест, поэтому обёртка сохранена byte-for-byte).
fn compute_book_snapshot_effects(states: &HashMap<String, SymbolState>) -> Vec<SessionEffect> {
    let mut effects = Vec::new();
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

        effects.push(SessionEffect::Emit(MdEvent {
            venue: Venue::Binance,
            symbol: symbol.clone(),
            payload: MdPayload::L2Snapshot {
                bids,
                asks,
                ts_exch_ms,
            },
        }));
    }
    effects
}

/// Legacy free-fn обёртка вокруг `compute_book_snapshot_effects` (SACRED-тест
/// `ts_exch_tests` внизу файла дёргает ЭТУ сигнатуру напрямую с
/// `states: HashMap<String, SymbolState>` — не трогать тест, поэтому сигнатура
/// сохранена byte-for-byte; `run()`/`SpotSession` этой функцией больше не пользуются).
/// `#[cfg(test)]` — вне тестовой сборки эта обёртка dead-code (прод-путь идёт через
/// `SpotSession::tick`), компилируется только вместе с sacred-тестом, который её зовёт.
#[cfg(test)]
async fn emit_book_snapshots(
    states: &HashMap<String, SymbolState>,
    tx: &mpsc::Sender<EventKind>,
) -> bool {
    for eff in compute_book_snapshot_effects(states) {
        if let SessionEffect::Emit(md) = eff {
            if tx.send(EventKind::Md(md)).await.is_err() {
                return false;
            }
        }
    }
    true
}

/// Запустить приём рыночных данных Binance для одной WS-сессии. Шлёт `EventKind::Md(..)`
/// в `tx`; `ConnUp` — сразу после успешного коннекта. Возвращает `Ok(())` при штатном
/// закрытии/дисконнекте/остановке получателя; `Err` — при ошибке коннекта. Reconnect —
/// забота вызывающего.
///
/// M-45: `run()` — ТОНКАЯ I/O-обёртка (async, ws/REST/mpsc), делегирующая в
/// `SpotSession` (sync-state-машина БЕЗ сети/каналов, паттерн TD-014). Вся логика
/// allow-list/book-sync живёт в seam'е и покрыта RED-оракулом `tests/red_l2delta_allowlist.rs`
/// (O-8: реальная точка входа, не структура кода).
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
    // M-45 task 6: env читается ОДНОЙ строкой на самом верху точки входа; дальше вниз
    // передаётся уже разобранным параметром (env НЕ читается внутри разбора/обработки).
    let l2delta_allow =
        parse_capture_symbols(std::env::var("L2DELTA_CAPTURE_SYMBOLS").ok().as_deref());
    let mut session = SpotSession::new_with_l2delta(&symbols, &l2delta_allow);
    let mut pending_snapshots: FuturesUnordered<SnapshotFuture> = FuturesUnordered::new();

    // Стартовая синхронизация: REST snapshot-фетч для каждого символа (состояние уже
    // помечено `resyncing` конструктором `SpotSession` — любые diff-апдейты, пришедшие
    // по WS ДО того, как фетч завершится, буферизуются, а не теряются).
    for eff in session.bootstrap() {
        apply_session_effect(eff, &tx, &client, &mut pending_snapshots).await;
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
                        for eff in session.on_ws_text(&text) {
                            if !apply_session_effect(eff, &tx, &client, &mut pending_snapshots).await {
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
            Some((symbol, result)) = pending_snapshots.next(), if !pending_snapshots.is_empty() => {
                let session_result: Result<OrderBook, ()> = match result {
                    Ok(book) => Ok(book),
                    Err(e) => {
                        tracing::warn!(symbol = %symbol, error = %e, "venue-binance: snapshot fetch failed");
                        Err(())
                    }
                };
                for eff in session.on_snapshot_result(&symbol, session_result) {
                    if !apply_session_effect(eff, &tx, &client, &mut pending_snapshots).await {
                        return Ok(());
                    }
                }
            }
            _ = emit_interval.tick() => {
                for eff in session.tick() {
                    if let SessionEffect::Emit(md) = eff {
                        if tx.send(EventKind::Md(md)).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

/// Применить `SessionEffect` к I/O: `Emit` → `tx.send`; `FetchSnapshot` → пушнуть future
/// с пред-calculated задержкой (спот: всегда `Duration::ZERO`). `false`, если `tx`
/// закрыт (получатель ушёл) — `run` обязан вернуть `Ok(())`.
async fn apply_session_effect(
    eff: SessionEffect,
    tx: &mpsc::Sender<EventKind>,
    client: &reqwest::Client,
    pending_snapshots: &mut FuturesUnordered<SnapshotFuture>,
) -> bool {
    match eff {
        SessionEffect::Emit(md) => tx.send(EventKind::Md(md)).await.is_ok(),
        SessionEffect::FetchSnapshot { symbol, after } => {
            pending_snapshots.push(make_snapshot_future(client.clone(), symbol, after));
            true
        }
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

/// M-45 (CT-RFC-06 §1): разбор allow-list эмиссии `L2Delta` из СЫРОЙ строки конфигурации
/// (env `L2DELTA_CAPTURE_SYMBOLS`, через запятую). ЧИСТАЯ функция — env читается ОДНОЙ
/// строкой на верху `run` и передаётся сюда параметром (`env` — глобальное состояние
/// процесса, `cargo test` гоняет тесты параллельно; парсер обязан быть детерминирован).
///
/// Дефолт при `None`/пустой/вырожденной строке (`""`, `"   "`, `","`, `" , , "`) —
/// `["BTCUSDT"]`, БАЙТ-В-БАЙТ сегодняшнее прод-поведение: расширение состава — решение
/// founder'а (Граница C), не инженерное. Пустые элементы отбрасываются, пробелы
/// обрезаются, порядок сохраняется, регистр нормализуется в ВЕРХНИЙ.
pub fn parse_capture_symbols(raw: Option<&str>) -> Vec<String> {
    const PROD_DEFAULT: &str = "BTCUSDT";
    let parsed: Vec<String> = raw
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();
    if parsed.is_empty() {
        vec![PROD_DEFAULT.to_string()]
    } else {
        parsed
    }
}

/// Решение об эмиссии `L2Delta`: wire-символ против разобранного allow-list'а.
/// Регистронезависимо (обе стороны нормализуются в верхний регистр), но НЕ по
/// подстроке — `BTC`/`BTCUSDT_PERP` не совпадают с `BTCUSDT`.
pub fn should_capture_l2delta(symbols: &[String], symbol: &str) -> bool {
    let up = symbol.to_uppercase();
    symbols.contains(&up)
}

/// ЕДИНСТВЕННАЯ точка решения об эмиссии `L2Delta` на РЕАЛЬНОМ пути (M-45, вердикт
/// критика `C-048`): разбор `stream`/`data` → символ → allow-list → событие. Чистая:
/// без I/O, без env, без async. `Some(event)` ⇔ сообщение — валидный depth-diff И
/// символ разрешён `symbols`. `SpotSession::on_ws_text` ОБЯЗАН делегировать сюда, а не
/// дублировать сравнение символов у себя — T5/T5b-канарейки `verify_M-45.sh` проверяют
/// именно это (единственный call site `l2delta_event(` — внутри этой функции).
pub fn l2delta_emission_for(
    stream: &str,
    data: &serde_json::Value,
    symbols: &[String],
) -> Option<EventKind> {
    if !stream.contains("@depth") {
        return None;
    }
    let (symbol, diff) = parse_depth_diff(stream, data)?;
    if !should_capture_l2delta(symbols, &symbol) {
        return None;
    }
    Some(l2delta_event(&symbol, &diff))
}

/// Чистый транслятор СЫРОГО `@depth` diff в канонический `EventKind::Md(L2Delta)`
/// (CT-RFC-04, L2D-I-2/3). Персистит diff БЕЗ ПОТЕРЬ, независимо от book-sync FSM —
/// сырой diff это ground-truth рыночное событие; наш sync-автомат (REST-бутстрап,
/// gap-resync) не является свойством данных. СПОТ: `prev_final_update_id = None`
/// (непрерывность спот-потока — `U == prev.u + 1`, чейн по `pu` не несёт смысла).
pub fn l2delta_event(symbol: &str, diff: &DepthDiff) -> EventKind {
    let bids = diff
        .bids
        .iter()
        .map(|&(price, size)| Level { price, size })
        .collect();
    let asks = diff
        .asks
        .iter()
        .map(|&(price, size)| Level { price, size })
        .collect();
    EventKind::md(
        Venue::Binance,
        symbol,
        MdPayload::L2Delta {
            bids,
            asks,
            first_update_id: diff.u_first,
            final_update_id: diff.u_final,
            prev_final_update_id: None,
            ts_exch_ms: diff.event_time_ms,
        },
    )
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
/// `FuturesUnordered` рядом с WS read loop. `after` — задержка перед фетчем (спот не
/// имеет backoff-политики — `SpotSession` всегда просит `Duration::ZERO`, но поле
/// части `SessionEffect::FetchSnapshot` по контракту M-45, тот же паттерн, что у
/// `venue-binance-futures`).
fn make_snapshot_future(client: reqwest::Client, symbol: String, after: Duration) -> SnapshotFuture {
    Box::pin(async move {
        if !after.is_zero() {
            tokio::time::sleep(after).await;
        }
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
// M-35: margin-inventory (CT-RFC-05, MarginInventory дискриминант 7).
//
// **Назначение.** Сбор СЫРОГО supply-пула `/sapi/v1/margin/available-inventory`
// (Binance, read-only, signed) для активов USDT/USDC. Это proxy-индикатор
// ёмкости margin (см. milestone §9 — «утилизация/флоу = Δ available downstream»).
//
// **Read-only by design (MI-I-3):** никаких submit/cancel/торговых-подписей. Ключ
// берётся исключительно из env (`BINANCE_API_KEY`/`BINANCE_API_SECRET`), никогда
// не логируется, не коммитится, не пишется в журнал. canary в verify_M-35.sh.
//
// **Auth-форма (Binance, общая):** HMAC-SHA256 от query-string (type + timestamp)
// → hex-строка → `&signature=<hex>` + header `X-MBX-APIKEY: <key>`.

/// Spot-домен для `/sapi/v1/margin/available-inventory`. Endpoint НЕ относится к
/// futures-USDT-M (использующему `fapi.binance.com`) — это cross-margin / margin
/// (spot-домен, `api.binance.com`). Сверять с фактическим HTTP-ответом (§9
/// milestone) — наш контракт фиксирует только spot.
const MARGIN_INVENTORY_URL: &str = "https://api.binance.com/sapi/v1/margin/available-inventory";
/// Cadence опроса (founder-tunable, ~2 мин). Binance-лимит 1200 req/мин для
/// `/sapi/v1/margin/*` — наш трафик ≪ этого, но 2 мин выбран по принципу
/// «достаточно для downstream-Δ, не чаще».
const MARGIN_INVENTORY_POLL_PERIOD: Duration = Duration::from_secs(120);
/// Активы, для которых ведётся сбор. Фиксировано под M-35 (founder scope).
/// Добавление новых — отдельный milestone (cross-margin borrow rules разные).
const MARGIN_INVENTORY_ASSETS: &[&str] = &["USDT", "USDC"];
/// HTTP-таймаут одного опроса (auth-REST имеет свой recvWindow=5s по умолчанию,
/// 10с сверху — запас на TLS+подпись).
const MARGIN_INVENTORY_TIMEOUT: Duration = Duration::from_secs(10);

/// Разобрать ответ `/sapi/v1/margin/available-inventory` (read-only) в
/// `Vec<MdEvent>`. Каждый asset из `assets`, присутствующий в JSON-ответе,
/// порождает ОДНО `MdEvent` с `MarginInventory{available_e8, ts_exch_ms}`.
///
/// **Fail-closed (per `red_margin_inventory::mi_i_2_absent_and_malformed_are_empty`):**
/// - битый JSON → `Vec::new()` (НЕ паника, НЕ выдуманное значение);
/// - нет ключа `assets` или `assets` — не объект → `Vec::new()`;
/// - нет `updateTime` или `updateTime` — не число → `Vec::new()`;
/// - asset из `assets` отсутствует в ответе / его значение не парсится в f64 →
///   пропуск (не ошибка, остальные активы продолжают эмититься).
///
/// **scale:** `available_e8 = to_fixed(value)` — `value` × 1e8, округление half-even.
/// **ts_exch_ms:** `updateTime` (сек) × 1000 — без этого событие помечалось бы
///   «1970-01-01» (анти-плацебо теста `mi_i_2_parses_filtered_assets`).
///
/// **Pure:** никакой сети, env, IO — тестируется фикстурами в `red_margin_inventory`.
pub fn parse_available_inventory(json: &str, assets: &[&str]) -> Vec<MdEvent> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let assets_obj = match v.get("assets").and_then(|a| a.as_object()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let update_time = match v.get("updateTime").and_then(|u| u.as_i64()) {
        Some(t) => t,
        None => return Vec::new(),
    };
    // saturating_mul: при updateTime == i64::MAX (заведомо невозможно с биржи) избегаем
    // overflow; здесь ровно 1e3, но стиль — единый с другими ts_exch_ms-конверсиями.
    let ts_exch_ms = update_time.saturating_mul(1000);

    let mut out = Vec::with_capacity(assets.len());
    for asset in assets {
        let value_str = match assets_obj.get(*asset).and_then(|x| x.as_str()) {
            Some(s) => s,
            None => continue, // asset вне ответа — fail-closed, остальные активы живут
        };
        let value: f64 = match value_str.parse() {
            Ok(v) => v,
            Err(_) => continue, // malformed number — fail-closed (VN-I-7 анти-фабрикация)
        };
        out.push(MdEvent {
            venue: Venue::Binance,
            symbol: (*asset).to_string(),
            payload: MdPayload::MarginInventory {
                available_e8: to_fixed(value),
                ts_exch_ms,
            },
        });
    }
    out
}

/// Прочитать `(BINANCE_API_KEY, BINANCE_API_SECRET)` из env. Возвращает `None`,
/// если хотя бы одной переменной нет / пустая. **НИКОГДА не логирует значения** —
/// только факт «present / missing» (caller решает, как это отражать).
pub fn read_margin_credentials() -> Option<(String, String)> {
    let key = std::env::var("BINANCE_API_KEY").ok()?;
    let secret = std::env::var("BINANCE_API_SECRET").ok()?;
    if key.is_empty() || secret.is_empty() {
        return None;
    }
    Some((key, secret))
}

/// HMAC-SHA256(query, secret) → hex-строка (lower-case). Стандарт Binance, кривая
/// `hex` для совместимости с их reference-impl; апострофы не экранируются (hex
/// алфавит — `[0-9a-f]`, URL-safe).
fn sign_query(query: &str, secret: &str) -> String {
    hex::encode(hmac_sha256::HMAC::mac(query.as_bytes(), secret.as_bytes()))
}

/// Один тик опроса: signed GET → `Vec<MdEvent>`. Без таймеров, без retry —
/// `run_margin_inventory` оркеструет cadence + backoff. На network/HTTP/parse
/// сбое возвращает `Vec::new()` (fail-closed) — caller отразит в метриках/логе.
async fn poll_available_inventory(
    client: &reqwest::Client,
    api_key: &str,
    api_secret: &str,
) -> Vec<MdEvent> {
    // timestamp — wall-clock ms (Binance требует ≤ recvWindow=5000ms от серверного
    // времени; вне теста — это синхронизируется NTP, рассинхронизация >5с означает
    // уже -1021 "Timestamp outside recvWindow" — caller логирует и пропускает).
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let query = format!("type=MARGIN&timestamp={timestamp}");
    let signature = sign_query(&query, api_secret);
    let url = format!("{MARGIN_INVENTORY_URL}?{query}&signature={signature}");

    let resp = match client
        .get(&url)
        .header("X-MBX-APIKEY", api_key)
        .timeout(MARGIN_INVENTORY_TIMEOUT)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %e, "venue-binance: margin-inventory poll HTTP error");
            return Vec::new();
        }
    };

    if !resp.status().is_success() {
        let code = resp.status();
        // Не логируем body: Binance может вернуть наш же query-string с подписью.
        let _ = resp.text().await;
        tracing::debug!(status = %code, "venue-binance: margin-inventory poll non-2xx");
        return Vec::new();
    }

    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            tracing::debug!(error = %e, "venue-binance: margin-inventory poll body read");
            return Vec::new();
        }
    };

    parse_available_inventory(&body, MARGIN_INVENTORY_ASSETS)
}

/// Периодический signed read-only poll margin-inventory. Эмитит `MarginInventory`
/// события в `tx` для активов из `MARGIN_INVENTORY_ASSETS` (USDT/USDC).
///
/// **Wiring (recorder-side, M-35 task 2):** спавнить как отдельную задачу рядом с
/// `run` (WS depth/trade); общий `tx` — `EventKind::md`-канал recorder'а. На
/// graceful-shutdown (закрытие `tx`) — корректный выход.
///
/// **Auth:** env (`BINANCE_API_KEY`/`SECRET`). При их отсутствии — сэмплинг раз в
/// `MARGIN_INVENTORY_POLL_PERIOD` пропускается (логируем warn ОДИН раз); никаких
/// логов с key/secret, никаких записей в журнал.
///
/// **Rate-limit защита:** `OPS-I-9`-семантика — на 418/429 — exp backoff (не
/// hot-loop); на прочих ошибках — также exp backoff; на 2xx — reset. Здесь
/// используем упрощённый вариант через `ops::budget::ReconBudget` (он уже
/// RED-тестирован, переиспользуем проверенный rate-limit модуль).
///
/// **НЕТ order-egress (MI-I-3 canary):** функция делает ТОЛЬКО read-only GET
/// `/sapi/v1/margin/available-inventory`. Никаких submit/cancel/торговых-подписей.
pub async fn run_margin_inventory(tx: mpsc::Sender<EventKind>) {
    use ops::budget::{ReconBudget, RestOutcome};

    let (api_key, api_secret) = match read_margin_credentials() {
        Some(c) => c,
        None => {
            tracing::warn!(
                "venue-binance: margin-inventory poll requires BINANCE_API_KEY / \
                 BINANCE_API_SECRET in env; skipping (no key — read-only, no panic)"
            );
            // Никакого busy-loop: пусть supervisor решает, нужен ли вообще этот поллер.
            // Возврат Ok — нормальный graceful exit (supervisor может перезапустить
            // после применения env); с `tx` ничего не сделано.
            return;
        }
    };

    let client = reqwest::Client::builder()
        .timeout(MARGIN_INVENTORY_TIMEOUT)
        .build()
        .expect("reqwest builder с фиксированными опциями не падает");
    // max_per_min=1 жёстко: 1 запрос / 60с = spacing ≥ 60с скользящего окна; cadence
    // 120с ⇒ в окне 0–1 запросов, но backoff от ошибок учитывается.
    let mut budget = ReconBudget::new(1);
    let start = std::time::Instant::now();
    let mut interval = tokio::time::interval(MARGIN_INVENTORY_POLL_PERIOD);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        interval.tick().await;

        let now = start.elapsed();
        if !budget.may_request(now) {
            continue;
        }
        budget.on_request(now);

        let events = poll_available_inventory(&client, &api_key, &api_secret).await;
        let outcome = if events.is_empty() {
            // Не различаем «пустой ответ» vs «ошибка» — оба ведут к backoff: empty
            // могут означать -1021 (timestamp drift) или -2015 (invalid API key); оба
            // требуют НЕ продолжать на той же cadence. Если Binance вернул 200 с
            // пустым assets — это legit edge, но backoff 1 раз не вредит.
            RestOutcome::Error
        } else {
            RestOutcome::Ok
        };

        for ev in events {
            if tx.send(EventKind::Md(ev)).await.is_err() {
                return;
            }
        }

        budget.next_delay(outcome);
    }
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
