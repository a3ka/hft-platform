//! M-22 Read Gateway — read-only консюмер журнала (Граница A, `docs/fa/viz-backend.md` §3B).
//!
//! Отдаёт фронту `code2alpha` три вещи над ОДНИМ кодом детерминированных редьюсеров:
//! `snapshot(at)` — полная свёртка серий окна `[start .. at]`; `frames_since(after)` —
//! инкрементальные кадры за событиями `seq > after` (live-push); `replay(from, to)` —
//! детерминированный проигрыш окна `(from .. to]`.
//!
//! **live == replay** (VB-I-2): один редьюсер, разный источник хвоста.
//!
//! ЭТОТ ФАЙЛ (architect, sacred): ТОЛЬКО T-designate контракт-типы + СИГНАТУРЫ с
//! `unimplemented!()`-телами (RED-bootstrap нового крейта). Тела — engine-dev (M-22 tasks 3-5)
//! ОБЯЗАН строить на `journal::stream(dir, EpochFilter)` (bounded); `journal::read_all`/
//! материализация `Vec<Event>` в этом крейте ЗАПРЕЩЕНЫ (C-021 NOTE-2; GW-I-2).

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use contracts::{Event, EventKind, Level, MdPayload, Side, Venue};
use journal::EpochFilter;
use serde::{Deserialize, Serialize};

/// Версия экспорт-формы gateway. **Аддитивно** поверх `research-cli::EXPORT_SCHEMA_VERSION = 1`
/// (VB-I-4/GW-I-5): новые серии (M-23+) добавляют поля, не переопределяют старые; форма меняется
/// ТОЛЬКО с bump этой константы. T-designate (не T1, не `crates/contracts`).
pub const GATEWAY_SCHEMA_VERSION: u32 = 2;

/// Что наблюдаем: площадка/символ/таймфрейм + depth-полосы (доли от mid, напр. `0.001` = 0.1%).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Selector {
    pub venue: Venue,
    pub symbol: String,
    pub timeframe_ms: i64,
    pub bands: Vec<f64>,
}

/// Монотонный read-курсор в тотальном порядке журнала (`Event.seq`).
///
/// `upto_seq = None` (`START`) — ничего не свёрнуто; `Some(s)` — включены события `seq <= s`.
/// `snapshot(at)` включает `seq <= at`; `frames_since(after)` включает `seq > after`; вместе на
/// одном `s` они дают полное непересекающееся покрытие (основа GW-I-3/GW-I-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cursor {
    pub upto_seq: Option<u64>,
}

impl Cursor {
    /// Ничего не свёрнуто (пустая серия).
    pub const START: Cursor = Cursor { upto_seq: None };
    /// До текущего хвоста журнала (свернуть всё, что есть).
    pub const LATEST: Cursor = Cursor {
        upto_seq: Some(u64::MAX),
    };
    /// Курсор «включительно до `seq`».
    pub fn at(seq: u64) -> Cursor {
        Cursor {
            upto_seq: Some(seq),
        }
    }
    /// Входит ли `seq` в `[start .. self]`.
    pub fn includes(&self, seq: u64) -> bool {
        match self.upto_seq {
            None => false,
            Some(s) => seq <= s,
        }
    }
}

/// OHLCV-строка (зеркалит export v1 §2; `i64` ×1e8, `time_s` — UTC seconds).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OhlcvRow {
    pub time_s: i64,
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
    pub volume: i64,
}

/// Depth time-series per (side, band) (зеркалит export v1 §4). BID/ASK — РАЗДЕЛЬНЫЕ серии.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DepthRow {
    /// `"bid"` | `"ask"` — не суммируются.
    pub side: String,
    /// Полоса в долях ×1e8 (0.001 ×1e8 = 100000 = 0.1%).
    pub band_pct_e8: i64,
    /// `(time_s, depth ×1e8)` close-семантика per бакет.
    pub series: Vec<(i64, i64)>,
    /// Провенанс полос глубже 1.3% от mid (VB-I-5/GW-I-6): непустой для deep-полос; `None`
    /// допустим ТОЛЬКО для полос ≤1.3% (валидированного эталона). Отсутствие на deep-серии → snapshot невалиден.
    pub depth_band_provenance: Option<String>,
}

/// Bundle серий — v1-подмножество (M-22). M-23+ добавляют поля АДДИТИВНО (heatmap/vwap/vp).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SeriesBundle {
    pub ohlcv: Vec<OhlcvRow>,
    /// Running cumulative delta `(time_s, знаковая агрессия до конца бакета)` (export v1 §3.2).
    pub cumulative_delta: Vec<(i64, i64)>,
    pub depth_series: Vec<DepthRow>,
}

/// Полная детерминированная свёртка окна `[start .. cursor]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub selector: Selector,
    /// Курсор, ДО которого (включительно) свёрнута серия.
    pub cursor: Cursor,
    pub series: SeriesBundle,
}

/// Инкрементальный кадр: приращение серий за `(from .. to]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub schema_version: u32,
    pub from: Cursor,
    pub to: Cursor,
    pub delta: SeriesBundle,
}

#[derive(Clone, Copy)]
struct OhlcvAcc {
    open: i64,
    high: i64,
    low: i64,
    close: i64,
    volume: i64,
}

impl OhlcvAcc {
    fn new(price: i64, size: i64) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume: size,
        }
    }

    fn update(&mut self, price: i64, size: i64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += size;
    }
}

struct DepthAcc {
    side: Side,
    band: f64,
    band_pct_e8: i64,
    values: BTreeMap<i64, i64>,
}

/// Incremental form of the M-17 reducers. State grows only with the emitted time buckets,
/// never with the number of journal events.
struct Reducer {
    selector: Selector,
    ohlcv: BTreeMap<i64, OhlcvAcc>,
    bucket_delta: BTreeMap<i64, i64>,
    depth: Vec<DepthAcc>,
}

impl Reducer {
    fn new(selector: &Selector) -> Self {
        Self {
            selector: selector.clone(),
            ohlcv: BTreeMap::new(),
            bucket_delta: BTreeMap::new(),
            depth: Vec::new(),
        }
    }

    fn bucket_time_s(&self, ts_ms: i64) -> Option<i64> {
        let timeframe_ms = self.selector.timeframe_ms;
        if timeframe_ms <= 0 {
            return None;
        }
        let bucket = ts_ms.div_euclid(timeframe_ms);
        Some(bucket.checked_mul(timeframe_ms).map_or(0, |ms| ms / 1_000))
    }

    fn apply(&mut self, event: &Event) {
        let EventKind::Md(md) = &event.kind else {
            return;
        };
        if md.venue != self.selector.venue || md.symbol != self.selector.symbol {
            return;
        }

        match &md.payload {
            MdPayload::Trade {
                price,
                size,
                side,
                ts_exch_ms,
            } => {
                let Some(time_s) = self.bucket_time_s(*ts_exch_ms) else {
                    return;
                };
                self.ohlcv
                    .entry(time_s)
                    .and_modify(|bar| bar.update(*price, *size))
                    .or_insert_with(|| OhlcvAcc::new(*price, *size));
                let signed_size = match side {
                    Side::Buy => *size,
                    Side::Sell => -*size,
                };
                *self.bucket_delta.entry(time_s).or_default() += signed_size;
            }
            MdPayload::L2Snapshot {
                bids,
                asks,
                ts_exch_ms,
            } => {
                let Some(time_s) = self.bucket_time_s(*ts_exch_ms) else {
                    return;
                };
                if self.depth.is_empty() {
                    for &band in &self.selector.bands {
                        for side in [Side::Buy, Side::Sell] {
                            self.depth.push(DepthAcc {
                                side,
                                band,
                                band_pct_e8: (band * 1e8).round() as i64,
                                values: BTreeMap::new(),
                            });
                        }
                    }
                }
                for row in &mut self.depth {
                    row.values
                        .insert(time_s, depth_within(bids, asks, row.side, row.band));
                }
            }
            _ => {}
        }
    }

    fn finish(self) -> SeriesBundle {
        let ohlcv = self
            .ohlcv
            .into_iter()
            .map(|(time_s, bar)| OhlcvRow {
                time_s,
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
                volume: bar.volume,
            })
            .collect();

        let mut running = 0_i64;
        let cumulative_delta = self
            .bucket_delta
            .into_iter()
            .map(|(time_s, delta)| {
                running += delta;
                (time_s, running)
            })
            .collect();

        let depth_series = self
            .depth
            .into_iter()
            .map(|row| DepthRow {
                side: match row.side {
                    Side::Buy => "bid",
                    Side::Sell => "ask",
                }
                .to_string(),
                band_pct_e8: row.band_pct_e8,
                series: row.values.into_iter().collect(),
                depth_band_provenance: (row.band_pct_e8 > 1_300_000)
                    .then(|| "diff-reconstructed, validated<=1.3%".to_string()),
            })
            .collect();

        SeriesBundle {
            ohlcv,
            cumulative_delta,
            depth_series,
        }
    }
}

fn depth_within(bids: &[Level], asks: &[Level], side: Side, band: f64) -> i64 {
    let best_bid = bids
        .iter()
        .filter(|level| level.size > 0)
        .map(|level| level.price)
        .max();
    let best_ask = asks
        .iter()
        .filter(|level| level.size > 0)
        .map(|level| level.price)
        .min();
    let (Some(best_bid), Some(best_ask)) = (best_bid, best_ask) else {
        return 0;
    };
    let mid = (best_bid + best_ask) / 2;
    match side {
        Side::Buy => {
            let threshold = (mid as f64 * (1.0 - band)) as i64;
            bids.iter()
                .filter(|level| level.size > 0 && level.price >= threshold)
                .map(|level| level.size)
                .sum()
        }
        Side::Sell => {
            let threshold = (mid as f64 * (1.0 + band)) as i64;
            asks.iter()
                .filter(|level| level.size > 0 && level.price <= threshold)
                .map(|level| level.size)
                .sum()
        }
    }
}

fn reduce_event_stream(
    stream: impl Iterator<Item = io::Result<Event>>,
    selector: &Selector,
    after: Cursor,
    to: Cursor,
    max_events: usize,
) -> io::Result<(SeriesBundle, Cursor, usize)> {
    let mut reducer = Reducer::new(selector);
    let mut cursor = after;
    let mut consumed = 0_usize;

    if max_events == 0 || to == Cursor::START {
        return Ok((reducer.finish(), cursor, consumed));
    }

    for event in stream {
        let event = event?;
        if after.upto_seq.is_some_and(|seq| event.seq <= seq) {
            continue;
        }
        if !to.includes(event.seq) || consumed == max_events {
            break;
        }
        reducer.apply(&event);
        cursor = Cursor::at(event.seq);
        consumed += 1;
    }

    Ok((reducer.finish(), cursor, consumed))
}

impl Snapshot {
    /// Сложить кадр в снапшот (fold): бакеты, пересекающиеся по `time_s`, СЛИВАЮТСЯ (OHLCV
    /// high/low/close/volume, cumulative_delta running, depth close-семантика), НЕ дублируются.
    /// Основа GW-I-4: `snapshot(C) + frames_since(C..C')` == `snapshot(C')`.
    ///
    /// engine-dev (M-22 task #4). Тело-заглушка — RED.
    pub fn apply(&mut self, _frame: &Frame) {
        unimplemented!("M-22 task #4 (engine-dev): fold Frame.delta into snapshot (bucket-merge)")
    }
}

/// Полная свёртка `[start .. at]` через bounded `journal::stream`. Read-only (GW-I-1).
///
/// engine-dev (M-22 task #3): ОБЯЗАН читать через `journal::stream(dir, filter)` (bounded,
/// прецедент `research-cli::data_quality`/`grid::run_grid_streamed`). `read_all`/`Vec<Event>` — ЗАПРЕЩЕНЫ (GW-I-2).
pub fn snapshot(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    sel: &Selector,
    at: Cursor,
) -> io::Result<Snapshot> {
    let stream = journal::stream(dir, filter)?;
    let (series, cursor, _) = reduce_event_stream(stream, sel, Cursor::START, at, usize::MAX)?;
    Ok(Snapshot {
        schema_version: GATEWAY_SCHEMA_VERSION,
        selector: sel.clone(),
        cursor,
        series,
    })
}

/// Кадры за событиями `seq > after` (batched, ≤ `max_events` событий за вызов), свёрнутые тем же
/// редьюсером; возвращает НОВЫЙ курсор (последний свёрнутый `seq`, либо `after` если новых нет).
///
/// **Bounded-memory (GW-I-2):** «пропуск» к `after` идёт СТРИМОМ (`journal::stream`), БЕЗ
/// материализации истории в `Vec<Event>` — память O(1) по размеру журнала, не по `after`.
/// `max_events` кап делает выход ограниченным → клиент пампит вызовами до сходимости курсора
/// (live-push). **Курсор-контракт (GW-I-8):** первый кадр `.from == after`; кадры контигуальны
/// (`f[i].to == f[i+1].from`); последний `.to == возвращённый курсор`. engine-dev (M-22 task #4).
pub fn frames_since(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    sel: &Selector,
    after: Cursor,
    max_events: usize,
) -> io::Result<(Vec<Frame>, Cursor)> {
    let _ = (dir.as_ref(), filter, sel, after, max_events);
    unimplemented!("M-22 task #4 (engine-dev): bounded journal::stream tail → frames + new cursor")
}

/// Детерминированный replay окна `(from .. to]` тем же редьюсером, что live (VB-I-2/GW-I-3).
/// engine-dev (M-22 task #4).
pub fn replay(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    sel: &Selector,
    from: Cursor,
    to: Cursor,
) -> io::Result<Vec<Frame>> {
    let _ = (dir.as_ref(), filter, sel, from, to);
    unimplemented!("M-22 task #4 (engine-dev): deterministic replay window → frames")
}
