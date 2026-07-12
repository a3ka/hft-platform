//! venue-binance-futures — Binance USDT-M перп (fstream). M-06 (architect skeleton + venue-dev impl).
//!
//! Emitter-not-owner (docs/fa/venues.md): WS/REST -> parse -> normalize -> MdEvent
//! (`Venue::BinanceFutures`). seq/ts_wall/ts_mono НЕ проставляет — это журнал (JR-I-1),
//! поэтому парс-функции возвращают `MdEvent`, не `Event`.
//!
//! Парс-функции (`parse_force_order` / `parse_depth_snapshot` / `parse_open_interest` /
//! `parse_mark_price`) — чистые детерминированные функции границы нормализации, покрытые
//! RED-оракулами `tests/red_parse.rs` и `tests/red_funding.rs`. Fail-closed: битая/неожиданная
//! форма → `None` (не паника, не фабрикация правдоподобного значения, VN-I-7).
//!
//! TD-014: sync-state-машина `FuturesSession` (БЕЗ сети/каналов) — тестируемый seam
//! для liveness-проблем (multi-diff stale, Funding-starve). `run()` — тонкая I/O-обёртка,
//! ДЕЛЕГИРУЮЩАЯ в `FuturesSession` (live == tested; иначе дефект снова невидим, §8 REJECT
//! #4 reland). `SessionEffect` — что сессия «хочет» эмитить/запросить: `Emit(MdEvent)`
//! или `FetchSnapshot { symbol, after }` (after — задержка перед REST-фетчем; TD-013:
//! `Duration::ZERO` для bootstrap/gap → fire immediately, `>= Backoff::BASE` для retry →
//! не hot-loop; для Err(418/429) — honor `default_rate_limit_cooldown(status)`).

use contracts::{
    from_fixed, to_fixed, EventKind, Level, MdEvent, MdPayload, Side, SysEvent, Venue,
};
use futures_util::stream::FuturesUnordered;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
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
// Book maintainer (seam, N2/INV-N2)
// ─────────────────────────────────────────────────────────────────────────────

/// Maintainer фьючерс-стакана per symbol. Seam тестируемого book-maintainer'а
/// (N2/INV-N2): при `apply_snapshot` — ПОЛНАЯ пересборка (REPLACE-семантика,
/// без переноса stale уровней через gap-ресинк); `apply_diff` — upsert/remove по уровням
/// (`size==0` → удалить); `notional_within` — аналитика глубины внутри полосы от mid.
/// Используется в async runner'е (sync-автомат per symbol хранит одну такую книгу).
/// price/size — fixed-point ×1e8 (per `contracts::PRICE_SCALE`).
pub struct FuturesDepthBook {
    bids: BTreeMap<i64, i64>,
    asks: BTreeMap<i64, i64>,
}

impl FuturesDepthBook {
    /// Пустой стакан.
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// REPLACE-снапшот: полная замена обеих сторон (clear + insert). Уровни `size<=0`
    /// (битые) дропаются на insert-стадии — в стакане таких быть не должно.
    /// Корректность INV-N2 (no phantom-liq после gap) обеспечивается именно REPLACE:
    /// stale дальние уровни из старого state'а НЕ переносятся, если их нет в `bids`/`asks`.
    pub fn apply_snapshot(&mut self, bids: &[Level], asks: &[Level]) {
        let mut new_bids = BTreeMap::new();
        for lvl in bids {
            if lvl.size > 0 {
                new_bids.insert(lvl.price, lvl.size);
            }
        }
        let mut new_asks = BTreeMap::new();
        for lvl in asks {
            if lvl.size > 0 {
                new_asks.insert(lvl.price, lvl.size);
            }
        }
        self.bids = new_bids;
        self.asks = new_asks;
    }

    /// Diff: `size==0` → удалить уровень (per Binance diff-spec); `size>0` → upsert;
    /// `size<0` (защитно) → игнорировать. Невалидный уровень не паникует.
    pub fn apply_diff(&mut self, bids: &[Level], asks: &[Level]) {
        for lvl in bids {
            if lvl.size == 0 {
                self.bids.remove(&lvl.price);
            } else if lvl.size > 0 {
                self.bids.insert(lvl.price, lvl.size);
            }
        }
        for lvl in asks {
            if lvl.size == 0 {
                self.asks.remove(&lvl.price);
            } else if lvl.size > 0 {
                self.asks.insert(lvl.price, lvl.size);
            }
        }
    }

    /// Σ(price × size) по уровням стороны `side`, чья relative-distance от mid ≤ `band`.
    /// `mid = (best_bid + best_ask) / 2`. Конвенция стороны: `Side::Buy` → bids,
    /// `Side::Sell` → asks (bids несут BUY-интерес, asks — SELL-интерес). Пустая книга /
    /// невалидный mid → 0.0. NOTIONAL возвращается в «долларах» (price×size как real
    /// float, делённый на `PRICE_SCALE²` неявно через `from_fixed`).
    pub fn notional_within(&self, side: Side, band: f64) -> f64 {
        let (best_bid, best_ask) = match (self.bids.iter().next_back(), self.asks.iter().next()) {
            (Some((&b, _)), Some((&a, _))) => (b, a),
            _ => return 0.0,
        };
        let mid_f = (best_bid as f64 + best_ask as f64) / 2.0;
        if mid_f <= 0.0 {
            return 0.0;
        }
        let book = match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        };
        let mut total = 0.0f64;
        for (&price, &size) in book.iter() {
            let rel = ((price as f64) - mid_f).abs() / mid_f;
            if rel <= band {
                total += from_fixed(price) * from_fixed(size);
            }
        }
        total
    }

    /// Crate-private доступ к bids/asks для runner'а (сжатие в L2Snapshot-бакеты, —
    /// book-maintainer не владеет output-форматом; emit-логика в runner'е). НЕ часть
    /// публичного seam — только `new`/`apply_snapshot`/`apply_diff`/`notional_within`.
    pub(crate) fn bids_map(&self) -> &BTreeMap<i64, i64> {
        &self.bids
    }
    pub(crate) fn asks_map(&self) -> &BTreeMap<i64, i64> {
        &self.asks
    }
}

impl Default for FuturesDepthBook {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Backoff (TD-013): чистая политика задержки retry snapshot-fetch'а
// ─────────────────────────────────────────────────────────────────────────────

/// Чистая детерминированная политика задержки retry snapshot-fetch'а (TD-013).
/// §8 eyes-on поймал прод-регрессию: snapshot-fail/stale ветки немедленно
/// `pending_snapshots.push(make_snapshot_future(...))` → hot-loop → 418-ban от Binance.
/// Эта политика + её wiring в `handle_snapshot` фиксят это.
///
/// Контракт (`tests/red_backoff.rs`):
///  • `next_delay(None)` первый раз возвращает ≥ `BASE` (не hot-loop);
///  • exp рост `BASE × MULTIPLIER^attempt` за каждую неудачу;
///  • ограничено сверху `CAP` (5 мин) — не уходит в бесконечность;
///  • `next_delay(Some(ra))` обязан honor'ить `Retry-After` из 418/429 (≥ `ra`);
///  • `reset()` (на успешном снапшоте) возвращает к базовой задержке.
///
/// Джиттер НЕ в политике (детерминизм тестов) — применяет I/O-boundary caller
/// на async-уровне (`handle_snapshot` — там, где конструируется future).
pub struct Backoff {
    attempt: u32,
}

impl Backoff {
    /// Базовая задержка первого retry (100мс — тест-минимум + практичный не-hot-loop).
    pub const BASE: Duration = Duration::from_millis(100);
    /// Верхняя граница (5 мин — после cap'а retry продолжается с этим интервалом).
    pub const CAP: Duration = Duration::from_secs(300);
    /// Множитель экспоненты (×2 за неудачу).
    pub const MULTIPLIER: u32 = 2;

    /// Новая политика (первый retry даёт `BASE`, далее exp).
    pub fn new() -> Self {
        Self { attempt: 0 }
    }

    /// Следующая задержка. `retry_after` из 418/429 `Retry-After` header (если есть)
    /// обязан быть honor'нут: возвращаемый `delay ≥ max(exp_computed, retry_after)`.
    /// После вызова `attempt` инкрементируется (следующая неудача — exp растёт).
    pub fn next_delay(&mut self, retry_after: Option<Duration>) -> Duration {
        let factor = Self::MULTIPLIER.saturating_pow(self.attempt);
        let exp = Self::BASE.saturating_mul(factor).min(Self::CAP);
        let chosen = match retry_after {
            Some(ra) if ra > exp => ra,
            _ => exp,
        };
        self.attempt = self.attempt.saturating_add(1);
        chosen
    }

    /// Успешный snapshot — сброс attempt к 0 (следующая неудача — снова базовая).
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sync-state-машина FuturesSession (TD-014) — тестируемый seam
// ─────────────────────────────────────────────────────────────────────────────

/// Локальная копия полного фьючерс-стакана одного символа. Делегирует поддержание
/// bids/asks maintainer'у `FuturesDepthBook` (seam N2/INV-N2), здесь держит ТОЛЬКО
/// метаданные для sync-автомата (last_update_id для непрерывности, event_time для
/// честного биржевого времени в `L2Snapshot`).
struct OrderBook {
    book: FuturesDepthBook,
    last_update_id: u64,
    /// `E` последнего применённого WS diff'а, мс. `0` — только REST-бутстрап без diff'ов
    /// (нет биржевого времени → символ НЕ эмитится в `tick`).
    last_event_time_ms: i64,
}

/// Один WS `@depth@100ms` diff (fstream формат для USDT-M FUTURES). Поля:
/// `pu` (previous final update id) — ЧЕЙНИТСЯ на book.last_update_id (==) и определяет
/// continuity (per Binance USD-M docs); `U`/`u` — update-id'ы ВНУТРИ diff'а, МОГУТ
/// ПРЫГАТЬ (не +1) — потому СПОТ-правило `u_first == last+1` НЕПРИМЕНИМО.
/// `b`/`a` — массивы `[price, qty]`-пар. `size==0` — удалить уровень.
/// `pu` обязателен в fstream-payload (отсутствие = malformed → Skip).
struct DepthDiff {
    event_time_ms: i64,
    pu: u64,
    u_first: u64,
    u_final: u64,
    bids: Vec<(i64, i64)>,
    asks: Vec<(i64, i64)>,
}

/// Состояние sync-конечного-автомата одного символа (см. `venue-binance`).
/// `backoff` (TD-013) — per-symbol политика retry для snapshot-fetch'а: на fail/stale
/// вычисляет задержку (exp + honor `Retry-After`), на success — `reset()`.
struct SymbolState {
    book: Option<OrderBook>,
    pending: VecDeque<DepthDiff>,
    resyncing: bool,
    backoff: Backoff,
}

impl SymbolState {
    fn new() -> Self {
        SymbolState {
            book: None,
            pending: VecDeque::new(),
            resyncing: false,
            backoff: Backoff::new(),
        }
    }
}

/// Что делать с входящим diff-апдейтом относительно текущего состояния символа
/// (sync-конечный-автомат). Внутри `FuturesSession::on_ws_text`.
enum DiffAction {
    /// Апдейт старше текущего состояния книги — отбросить.
    Skip,
    /// Книга ещё не синхронизирована — буферизовать, запросить снапшот если ещё не в
    /// процессе.
    Buffer,
    /// Разрыв непрерывности (`pu != last_update_id` для FUTURES) — книга инвалидируется,
    /// пере-синхронизация с нуля. (Исторически было спот-правило `U != last+1`; см. TD-014 T2.)
    Gap,
    /// Апдейт непрерывен — применить к книге.
    Apply,
}

/// Эффект sync-state-машины `FuturesSession`: то, что она «хочет» эмитить во внешний мир
/// или запросить у I/O-слоя. `run()` ОБЯЗАН делегировать в `FuturesSession` — иначе
/// дефекты (TD-014) снова невидимы юнит-тестам (live != tested → регресс повторяется).
///
/// * `Emit(MdEvent)` — нормализованное событие для журнала (Liquidation/Funding/L2Snapshot).
/// * `FetchSnapshot { symbol, after }` — запросить REST-снапшот; `after` — задержка
///   перед запросом: `Duration::ZERO` для bootstrap/gap (fire immediately), `>= Backoff::BASE`
///   для retry (TD-013: не hot-loop); для Err(418/429) session уже заложил
///   `default_rate_limit_cooldown(status)` внутрь `after`.
#[derive(Debug, Clone)]
pub enum SessionEffect {
    Emit(MdEvent),
    FetchSnapshot { symbol: String, after: Duration },
}

/// Тестируемая sync-state-машина `venue-binance-futures` БЕЗ сети/каналов (TD-014).
/// Инкапсулирует per-symbol `SymbolState` (буфер diff'ов + book + `Backoff`) и
/// symbol-set для фильтрации `!markPrice@arr`. Никогда не ходит в сеть и не шлёт в mpsc —
/// только накапливает состояние и возвращает `Vec<SessionEffect>` для I/O-обёртки.
///
/// Контракт (`tests/red_live_emit.rs`):
///  • `on_ws_text(&str)` обрабатывает depth-diff / forceOrder / `!markPrice@arr`;
///    depth-diff → sync-автомат (Buffer/Gap/Apply/Skip); forceOrder/markPrice → `Emit`.
///  • `on_snapshot_result(&str, Result<String, u16>)` — Ok(json) реконсилит с буфером
///    (TD-014 FIX: применяет НЕСКОЛЬКО contiguous diff'ов подряд, двигая
///    `last_update_id = diff.u_final` при каждом apply), Err(418/429) → `Backoff`-delay,
///    Err(other) → exp `Backoff`-delay (не hot-loop).
///  • `tick()` эмитит bounded L2Snapshot per синкнутый символ (нужен биржевой ts).
///
/// `run()` — тонкая I/O-обёртка (async, ws/REST/mpsc), которая ДЕЛЕГИРУЕТ в этот seam.
/// «Верный рефактор текущей логики» оставил multi-diff-stale → оракул RED, форсит фикс
/// (TD-014 FIX (a): `apply_diff_to_book` ДВИГАЕТ `last_update_id`; (b): Funding из
/// `!markPrice@arr` эмитится НЕЗАВИСИМО от состояния книги, не starve).
pub struct FuturesSession {
    symbol_set: HashSet<String>,
    states: HashMap<String, SymbolState>,
}

impl FuturesSession {
    /// Новая сессия для выборки `symbols`. symbol-set (upcased) — для O(1) фильтрации
    /// `!markPrice@arr` (Binance агрегирует mark-prices по всем перпам; нас интересуют
    /// только символы нашей выборки). States — пустой; per-symbol `SymbolState` создаётся
    /// lazily при первом depth-diff'е (через `entry().or_insert_with`).
    pub fn new(symbols: &[String]) -> Self {
        let symbol_set: HashSet<String> = symbols.iter().map(|s| s.to_uppercase()).collect();
        Self {
            symbol_set,
            states: HashMap::new(),
        }
    }

    /// Обработать одно combined-stream текстовое сообщение fstream. Возвращает эффекты,
    /// которые I/O-обёртка (`run`) должна применить: эмитить в `tx` / поставить snapshot
    /// future с пред-calculated задержкой.
    ///
    /// Funding из `!markPrice@arr` эмитится НЕЗАВИСИМО от состояния книги (TD-014 FIX (b):
    /// иначе во время длительного depth-resync funding starves → 0 Funding в журнале).
    pub fn on_ws_text(&mut self, text: &str) -> Vec<SessionEffect> {
        let mut effects = Vec::new();
        let value: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, raw = %text, "venue-binance-futures: malformed JSON, skipping");
                return effects;
            }
        };

        let stream = match value.get("stream").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                tracing::debug!(raw = %text, "venue-binance-futures: message without 'stream', skipping");
                return effects;
            }
        };
        let data = match value.get("data") {
            Some(d) => d,
            None => {
                tracing::debug!(raw = %text, "venue-binance-futures: message without 'data', skipping");
                return effects;
            }
        };

        if stream.ends_with("@forceOrder") {
            // Combined-stream fstream оборачивает событие в `{"stream":"...", "data":{...}}`,
            // где `data` уже форма fstream — его прямо скармливаем `parse_force_order`.
            let raw = match serde_json::to_string(data) {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!(error = %e, "venue-binance-futures: failed to re-serialize forceOrder data");
                    return effects;
                }
            };
            if let Some(event) = parse_force_order(&raw) {
                effects.push(SessionEffect::Emit(event));
            } else {
                tracing::debug!(raw = %raw, "venue-binance-futures: malformed forceOrder, skipping");
            }
        } else if stream.ends_with("@markPrice") || stream.ends_with("@markPrice@1s") {
            // TD-014 T3 FIX: per-symbol `<sym>@markPrice[/@1s]` (combined-stream fstream).
            // `data` = ОДИНОЧНЫЙ markPriceUpdate объект (не array). Агрегированный
            // `!markPrice@arr` на combined endpoint НЕ доставляется Binance'ом (live-capture:
            // markPrice=0 при depth=139) → Funding=0 в журнале. Per-symbol форма надёжна.
            // `parse_mark_price` уже понимает одиночный объект (поля `s`/`r`/`E`).
            let raw = match serde_json::to_string(data) {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!(error = %e, "venue-binance-futures: failed to re-serialize per-symbol markPrice data");
                    return effects;
                }
            };
            if let Some(event) = parse_mark_price(&raw) {
                if !self.symbol_set.contains(&event.symbol) {
                    tracing::debug!(stream = %stream, sym = %event.symbol, "venue-binance-futures: per-symbol markPrice не в нашей выборке");
                    return effects;
                }
                effects.push(SessionEffect::Emit(event));
            } else {
                tracing::debug!(raw = %raw, "venue-binance-futures: malformed per-symbol markPrice, skipping");
            }
        } else if stream == "!markPrice@arr" {
            // TD-014 FIX (b): агрегированная array-форма (legacy / не-combined endpoint).
            // На combined НЕ доставляется (см. T3), но оставлена для регрессии (оракул
            // `red_live_funding.rs` assert'ит обе формы) — и для отдельных WS-сессий
            // без depth (где `!markPrice@arr` работает). Парсим каждый item, фильтруем
            // по нашей выборке.
            let Some(arr) = data.as_array() else {
                tracing::debug!(stream = %stream, "venue-binance-futures: !markPrice@arr without array, skipping");
                return effects;
            };
            for item in arr {
                let raw = match serde_json::to_string(item) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let Some(event) = parse_mark_price(&raw) else {
                    // Парс-фейл по конкретному item — skip, остальные item'ы пробуем.
                    continue;
                };
                if !self.symbol_set.contains(&event.symbol) {
                    continue;
                }
                effects.push(SessionEffect::Emit(event));
            }
        } else if stream.contains("@depth") {
            if let Some((symbol, diff)) = parse_depth_diff(stream, data) {
                // Сначала вычислить action (immutable borrow), потом мутировать.
                let state = self
                    .states
                    .entry(symbol.clone())
                    .or_insert_with(SymbolState::new);
                let action = match &state.book {
                    None => DiffAction::Buffer,
                    Some(book) => {
                        // TD-014 T2 FIX (continuity = pu, не спот u_first+1): FUTURES-правило
                        // `pu == book.last_update_id` → Apply; иначе → Gap. СПОТ-правило
                        // `u_first == last+1` ложно флагает валидные futures-jump'ы как gap
                        // (U/u ПРЫГАЮТ у perp, чейн через `pu`) → вечный resync churn → sparse
                        // L2 + 429-ban + 0 Funding downstream.
                        if diff.u_final <= book.last_update_id {
                            DiffAction::Skip
                        } else if diff.pu != book.last_update_id {
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
                            "venue-binance-futures: depth continuity gap detected, resyncing book"
                        );
                        state.book = None;
                        state.pending.clear();
                        state.pending.push_back(diff);
                        state.resyncing = true;
                        // Gap: предыдущий book инвалидирован; backoff сброшен предыдущим
                        // success → attempt=0 → BASE=100ms. None (новая попытка, не retry).
                        effects.push(SessionEffect::FetchSnapshot {
                            symbol: symbol.clone(),
                            after: Duration::ZERO,
                        });
                    }
                    DiffAction::Apply => {
                        if let Some(book) = state.book.as_mut() {
                            // TD-014 FIX (a): `apply_diff_to_book` ДВИГАЕТ
                            // `last_update_id = diff.u_final` — без этого 2-й contiguous
                            // diff вечно "stale" → книга не синкается → 0 L2.
                            apply_diff_to_book(book, &diff);
                        }
                    }
                }
            } else {
                tracing::debug!(stream = %stream, "venue-binance-futures: unparseable depth frame");
            }
        } else {
            tracing::debug!(stream = %stream, "venue-binance-futures: unrecognized stream, skipping");
        }

        effects
    }

    /// REST snapshot завершился — реконcилировать с буфером diff'ов (Binance algorithm).
    /// `Ok(json)` — распарсить, применить contiguous buffered diff'ы, пометить символ
    /// синхронизированным, сбросить backoff. `Err(status)` — увеличить backoff и вернуть
    /// `FetchSnapshot` с задержкой (TD-013: не hot-loop; 418/429 → honor
    /// `default_rate_limit_cooldown`).
    pub fn on_snapshot_result(
        &mut self,
        symbol: &str,
        result: Result<String, u16>,
    ) -> Vec<SessionEffect> {
        let mut effects = Vec::new();
        let state = match self.states.get_mut(symbol) {
            Some(s) => s,
            None => return effects,
        };

        let mut book = match result {
            Ok(json) => match parse_snapshot_for_book(&json) {
                Some((last_update_id, ts_exch_ms, bids, asks)) => {
                    let mut fb = FuturesDepthBook::new();
                    fb.apply_snapshot(&bids, &asks);
                    OrderBook {
                        book: fb,
                        last_update_id,
                        // TD-014 v2 FIX (recovery-sync): pre-populate `last_event_time_ms`
                        // из `T` снапшота (fallback `E`, иначе 0). Если буфер-diff'ы
                        // applied в reconcile — `apply_diff_to_book` перезапишет их
                        // более свежим `diff.event_time_ms`. Если буфер пуст/DROP'нут
                        // (recovery-снапшот впереди буфера) — `ts_exch_ms` снапшота
                        // остаётся, и `tick()` эмитит L2 (а не пропускает по gate'у
                        // «нет биржевого времени»). Live: после gap+stale+recovery книга
                        // остаётся с `last_event_time_ms=0` → вечный 0 L2 в журнале.
                        last_event_time_ms: ts_exch_ms,
                    }
                }
                None => {
                    tracing::warn!(
                        symbol = %symbol,
                        "venue-binance-futures: snapshot malformed, retrying with backoff"
                    );
                    let delay = state.backoff.next_delay(None);
                    effects.push(SessionEffect::FetchSnapshot {
                        symbol: symbol.to_string(),
                        after: delay,
                    });
                    return effects;
                }
            },
            Err(status) => {
                // 418 (IP-ban) / 429 (rate-limit) → honor `default_rate_limit_cooldown`;
                // прочие → exp база без cooldown.
                let retry_after = if status == 418 || status == 429 {
                    Some(default_rate_limit_cooldown(status))
                } else {
                    None
                };
                tracing::warn!(
                    symbol = %symbol,
                    status,
                    "venue-binance-futures: snapshot fetch failed, retrying with backoff"
                );
                let delay = state.backoff.next_delay(retry_after);
                effects.push(SessionEffect::FetchSnapshot {
                    symbol: symbol.to_string(),
                    after: delay,
                });
                return effects;
            }
        };

        // Reconcile-loop: применять buffered diff'ы, пока они contiguous с `last_update_id`
        // снапшота. TD-014 T2 FIX: для STEADY-STATE (on_ws_text) continuity = pu (== last),
        // но для RECONCILE-LOOP правило ЛЕНЬЕЕЕ — Binance-стиль `U <= lastUpdateId+1 AND
        // u >= lastUpdateId+1` (мы имеем snapshot как fallback, можем быть снисходительнее):
        //  • DROP: `u_final <= L` (diff полностью покрыт снапшотом);
        //  • STALE: `u_first > L+1` (diff начинается ПОСЛЕ gap'а от снапшота — не bridge'нуть,
        //    сервер/VENUE race; refetch с backoff);
        //  • APPLY: иначе (`u_first <= L+1`, diff contiguous или перекрывает snapshot).
        // После первого apply `last_update_id` двигается на `u_final`, и последующие diff'ы
        // проверяются по той же схеме (что для валидных futures diff'ов с `U == pu+1`
        // вырождается в `pu == previous u` — FUTURES-continuity).
        loop {
            let front = state.pending.front().map(|d| (d.u_final, d.u_first));
            let Some((u_final, u_first)) = front else {
                state.book = Some(book);
                state.resyncing = false;
                // SUCCESS: снапшот согласован с буфером (или буфер пуст) → сброс backoff.
                state.backoff.reset();
                return effects;
            };
            if u_final <= book.last_update_id {
                // DROP: diff полностью покрыт снапшотом (мы уже знаем эти updates).
                state.pending.pop_front();
                continue;
            }
            if u_first > book.last_update_id + 1 {
                // STALE: diff стартует ПОСЛЕ gap'а от снапшота (`U > L+1` в Binance-терминах)
                // — не bridge'нуть: между `L+1` и `u_first-1` неизвестные updates.
                // Сетевой/венюный race, НЕ server rate-limit (Retry-After нерелевантен),
                // backoff всё равно применяем (анти-hot-loop: не лупить REST до полного resync).
                tracing::warn!(
                    symbol = %symbol,
                    "venue-binance-futures: snapshot stale vs buffered diffs, refetching with backoff"
                );
                state.resyncing = true;
                let delay = state.backoff.next_delay(None);
                effects.push(SessionEffect::FetchSnapshot {
                    symbol: symbol.to_string(),
                    after: delay,
                });
                return effects;
            }
            // APPLY: diff contiguous или перекрывает snapshot (`u_first <= L+1`).
            // TD-014 FIX (a): `apply_diff_to_book` двигает `last_update_id`, поэтому
            // НЕСКОЛЬКО contiguous diff'ов применяются подряд.
            let diff = state
                .pending
                .pop_front()
                .expect("front() just returned Some");
            apply_diff_to_book(&mut book, &diff);
        }
    }

    /// Периодический тик (вызывается раз в `EMIT_PERIOD` из `run`): эмитит один
    /// `L2Snapshot` на синхронизированный символ (есть book + биржевое время).
    /// Символ только после REST-бутстрапа (без применённых diff'ов) не имеет биржевого
    /// времени — НЕ выдумываем, пропускаем.
    pub fn tick(&self) -> Vec<SessionEffect> {
        let mut effects = Vec::new();
        for (symbol, state) in self.states.iter() {
            let Some(book_state) = &state.book else {
                continue;
            };
            let ts_exch_ms = book_state.last_event_time_ms;
            if ts_exch_ms == 0 {
                continue;
            }
            let Some((&best_bid, _)) = book_state.book.bids_map().iter().next_back() else {
                continue;
            };
            let Some((&best_ask, _)) = book_state.book.asks_map().iter().next() else {
                continue;
            };
            let mid = (best_bid + best_ask) / 2;
            if mid <= 0 {
                continue;
            }
            let bids = bucket_levels(book_state.book.bids_map().iter().rev(), mid);
            let asks = bucket_levels(book_state.book.asks_map().iter(), mid);
            effects.push(SessionEffect::Emit(MdEvent {
                venue: Venue::BinanceFutures,
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
}

/// fstream `@depth@100ms` diff payload (FUTURES формат): `pu` (previous final update id,
/// обязателен) + `U`/`u` (update-id'ы внутри diff'а, МОГУТ ПРЫГАТЬ) + `b`/`a`.
/// `pu` — ЯКОРЬ continuity для FUTURES (чейн на book.last_update_id), см. TD-014 T2.
fn parse_depth_diff(stream: &str, data: &Value) -> Option<(String, DepthDiff)> {
    let symbol = match data.get("s").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => stream.split('@').next()?.to_uppercase(),
    };
    // `pu` обязателен для USDT-M futures (отсутствие = malformed → None, fail-closed).
    let pu = data.get("pu")?.as_u64()?;
    let u_first = data.get("U")?.as_u64()?;
    let u_final = data.get("u")?.as_u64()?;
    let bids = parse_diff_levels(data.get("b")?)?;
    let asks = parse_diff_levels(data.get("a")?)?;
    let event_time_ms = data.get("E").and_then(|v| v.as_i64()).unwrap_or(0);
    Some((
        symbol,
        DepthDiff {
            event_time_ms,
            pu,
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

/// Распарсить snapshot JSON → `(lastUpdateId, ts_exch_ms, bids, asks)` для `OrderBook`.
/// `ts_exch_ms` — поле `T` (transact-time снапшота; primary, как в `parse_depth_snapshot`),
/// fallback `E` (event-time диспатча), иначе `0`. Битая/неполная форма → `None`. `bids`/`asks`
/// проходят через `apply_snapshot`, который фильтрует `size<=0` (битые уровни) на insert-стадии.
fn parse_snapshot_for_book(json: &str) -> Option<(u64, i64, Vec<Level>, Vec<Level>)> {
    let v: Value = serde_json::from_str(json).ok()?;
    let last_update_id = v.get("lastUpdateId")?.as_u64()?;
    let ts_exch_ms = v
        .get("T")
        .and_then(|t| t.as_i64())
        .or_else(|| v.get("E").and_then(|t| t.as_i64()))
        .unwrap_or(0);
    let bids = parse_l2_levels(v.get("bids")?)?;
    let asks = parse_l2_levels(v.get("asks")?)?;
    Some((last_update_id, ts_exch_ms, bids, asks))
}

fn apply_diff_to_book(book: &mut OrderBook, diff: &DepthDiff) {
    let bids: Vec<Level> = diff
        .bids
        .iter()
        .map(|&(price, size)| Level { price, size })
        .collect();
    let asks: Vec<Level> = diff
        .asks
        .iter()
        .map(|&(price, size)| Level { price, size })
        .collect();
    book.book.apply_diff(&bids, &asks);
    // TD-014 FIX (a): ДВИГАТЬ `last_update_id` при apply, иначе 2-й contiguous diff
    // вечно "stale" относительно неподвижного last_update_id → вечный resync → 0 L2.
    book.last_update_id = diff.u_final;
    book.last_event_time_ms = diff.event_time_ms;
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

// ─────────────────────────────────────────────────────────────────────────────
// I/O-обёртка (TD-014 FIX): run() ДЕЛЕГИРУЕТ в FuturesSession (live == tested)
// ─────────────────────────────────────────────────────────────────────────────

/// Ошибка snapshot-fetch'а с разделением rate-limit (418/429 + `Retry-After`) и прочих.
/// `status` пробросится через `run` в `FuturesSession::on_snapshot_result` как `Err(status)`,
/// чтобы сессия могла применить `default_rate_limit_cooldown` для rate-limit'нутых кодов
/// (TD-013: анти-hot-loop). `Retry-After` из headers пока honored внутри `fetch_snapshot_raw`
/// (на случай INITIAL-connect после IP-ban — заголовок важнее дефолта).
#[derive(Debug)]
pub(crate) enum SnapshotError {
    RateLimited { status: u16, retry_after: Duration },
    Other(anyhow::Error),
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RateLimited {
                status,
                retry_after,
            } => {
                write!(
                    f,
                    "rate-limited (status {status}), retry after {retry_after:?}"
                )
            }
            Self::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

/// `SnapshotResult` — сырая JSON-строка (snapshot) или `SnapshotError`. Передаётся из
/// `fetch_snapshot_raw` через `SnapshotFuture` в `run`, где транслируется в
/// `Result<String, u16>` для `FuturesSession::on_snapshot_result` (потеря `retry_after`
/// на этом шаге — сессия использует `default_rate_limit_cooldown(status)`, см. doc).
type SnapshotResult = Result<String, SnapshotError>;

type SnapshotFuture = Pin<Box<dyn Future<Output = (String, SnapshotResult)> + Send>>;

/// Запустить сессию Binance USDT-M futures WS+REST. Шлёт `EventKind::Md(..)` в `tx`;
/// `Sys(ConnUp(BinanceFutures))` — сразу после успешного WS-коннекта. Возвращает `Ok(())`
/// при штатном закрытии/дисконнекте/уходе получателя; `Err(_)` — при ошибке коннекта.
/// Reconnect/backoff — забота supervisor'а снаружи (emitter-not-owner, как в `venue-binance`).
///
/// TD-014: `run()` — ТОНКАЯ I/O-обёртка, делегирующая в `FuturesSession` (sync-state-машина
/// БЕЗ сети/каналов). Вся нетривиальная логика (depth-sync, Funding-emit, backoff,
/// L2Snapshot bucketizing) живёт в seam'е и покрыта RED-тестом `tests/red_live_emit.rs`.
/// «Верный рефактор текущей логики» оставил бы multi-diff-stale → оракул RED, форсит фикс.
pub async fn run(tx: mpsc::Sender<EventKind>, symbols: Vec<String>) -> anyhow::Result<()> {
    let mut streams = Vec::with_capacity(symbols.len() * 3);
    for s in &symbols {
        let lower = s.to_lowercase();
        streams.push(format!("{lower}@depth@100ms"));
        streams.push(format!("{lower}@forceOrder"));
        // TD-014 T3 FIX: PER-SYMBOL `<sym>@markPrice@1s` вместо агрегированного
        // `!markPrice@arr`. На combined endpoint Binance НЕ доставляет `!markPrice@arr`
        // вместе с per-symbol стримами (live-capture: markPrice=0 при depth=139 → 0
        // Funding в журнале). Per-symbol форма надёжна в combined-stream.
        streams.push(format!("{lower}@markPrice@1s"));
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
    let mut session = FuturesSession::new(&symbols);
    let mut pending_snapshots: FuturesUnordered<SnapshotFuture> = FuturesUnordered::new();

    // Стартовая синхронизация depth по каждому символу (REST snapshot — не ждём первого
    // diff'а, чтобы bootstrap latency был минимален; pending diff'ы буферизуются в
    // `FuturesSession::on_ws_text` пока snapshot не вернётся).
    for s in &symbols {
        let symbol = s.to_uppercase();
        pending_snapshots.push(make_snapshot_future(client.clone(), symbol, Duration::ZERO));
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
                // Транслировать `SnapshotResult` (Ok(json) / Err(RateLimited{status,..}) /
                // Err(Other)) в `Result<String, u16>` для seam'а. `retry_after` из заголовка
                // уже honored внутри `fetch_snapshot_raw`; здесь теряется (сессия
                // использует `default_rate_limit_cooldown(status)`).
                let session_result: Result<String, u16> = match result {
                    Ok(json) => Ok(json),
                    Err(SnapshotError::RateLimited { status, .. }) => Err(status),
                    Err(SnapshotError::Other(_)) => Err(500),
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
            _ = oi_interval.tick() => {
                if !poll_open_interest(&client, &symbols, &tx).await {
                    return Ok(());
                }
            }
        }
    }
}

/// Применить `SessionEffect` к I/O: `Emit` → `tx.send`; `FetchSnapshot` → пушнуть
/// future с пред-calculated задержкой. `false` если `tx` закрыт (получатель ушёл) —
/// `run` обязан вернуть `Ok(())`.
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

/// Запросить REST snapshot для символа и вернуть СЫРОЙ JSON (без парсинга). Парс и
/// reconcile делает `FuturesSession::on_snapshot_result` — иначе дублирование логики
/// и seam снова становится невидимым.
///
/// TD-013: 418 (Binance IP-ban) / 429 (rate-limit) распознаём ДО `error_for_status` и
/// преобразовываем в `SnapshotError::RateLimited { status, retry_after }` —
/// `retry_after` из `Retry-After` header (если есть) ИЛИ `default_rate_limit_cooldown(status)`.
async fn fetch_snapshot_raw(
    client: &reqwest::Client,
    symbol: &str,
) -> Result<String, SnapshotError> {
    let url = format!("{REST_DEPTH_BASE}{symbol}&limit={REST_DEPTH_LIMIT}");
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| SnapshotError::Other(e.into()))?;
    let status = response.status();
    if status.as_u16() == 418 || status.as_u16() == 429 {
        let retry_after = parse_retry_after_header(response.headers())
            .unwrap_or_else(|| default_rate_limit_cooldown(status.as_u16()));
        return Err(SnapshotError::RateLimited {
            status: status.as_u16(),
            retry_after,
        });
    }
    let text = response
        .error_for_status()
        .map_err(|e| SnapshotError::Other(e.into()))?
        .text()
        .await
        .map_err(|e| SnapshotError::Other(e.into()))?;
    Ok(text)
}

/// `Retry-After` header (RFC 7231 §7.1.3) → `Duration`. Binance использует формат
/// delta-seconds (целое число секунд); HTTP-date игнорируем (Binance не применяет).
fn parse_retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let v = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    v.parse::<u64>().ok().map(Duration::from_secs)
}

/// Дефолтный cooldown когда `Retry-After` отсутствует: 418 (IP-ban) — длинный,
/// 429 (rate-limit) — средний. Не 0 — иначе backoff не спасёт от hammering'а.
/// Используется в `fetch_snapshot_raw` (как fallback к header) И в
/// `FuturesSession::on_snapshot_result` (для Err(418/429) от run-уровня — сессия
/// не получает `retry_after`, опирается на дефолт).
fn default_rate_limit_cooldown(status: u16) -> Duration {
    match status {
        418 => Duration::from_secs(120),
        429 => Duration::from_secs(10),
        _ => Duration::from_secs(60),
    }
}

/// Сконструировать `SnapshotFuture` с заданной pre-delay задержкой. `after = ZERO` —
/// fire immediately (bootstrap/gap); `after > 0` — retry после fail/stale, внутри
/// `tokio::time::sleep(after).await` ПЕРЕД `fetch_snapshot_raw` (TD-013 wiring,
/// анти-hot-loop, §8).
fn make_snapshot_future(
    client: reqwest::Client,
    symbol: String,
    after: Duration,
) -> SnapshotFuture {
    Box::pin(async move {
        if !after.is_zero() {
            tokio::time::sleep(after).await;
        }
        let result = fetch_snapshot_raw(&client, &symbol).await;
        (symbol, result)
    })
}

/// Периодический REST-опрос `/fapi/v1/openInterest` per symbol → `MdEvent::OpenInterest`.
/// Fail-closed: HTTP/parse failure → логируем + skip конкретный символ (не паникуем,
/// polling продолжится со следующего тика — VN-I-7). НЕ идёт через `FuturesSession` —
/// OI REST polling не имеет sync-state (push-аналога нет, опрос по таймеру).
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

/// `!markPrice@arr` (single item) / `<symbol>@markPrice` (markPriceUpdate) →
/// `MdEvent{BinanceFutures, Funding}`. `rate_e8` = поле `r` (funding rate) ×1e8;
/// `ts_exch_ms` = `E` (event time); symbol = `s`. Знак `r` СОХРАНЯЕТСЯ (положит/отрицат
/// критичны для funding-breadth derive, M-06 #5). Битая/неполная форма → `None` (VN-I-7).
pub fn parse_mark_price(json: &str) -> Option<MdEvent> {
    let v: Value = serde_json::from_str(json).ok()?;
    let symbol = v.get("s")?.as_str()?.to_string();
    let rate: f64 = v.get("r")?.as_str()?.parse().ok()?;
    let ts_exch_ms = v.get("E")?.as_i64()?;
    Some(MdEvent {
        venue: Venue::BinanceFutures,
        symbol,
        payload: MdPayload::Funding {
            rate_e8: to_fixed(rate),
            ts_exch_ms,
        },
    })
}
