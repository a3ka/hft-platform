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

use contracts::{Event, EventKind, Level, MdEvent, MdPayload, Side, Venue};
use journal::EpochFilter;
use serde::{Deserialize, Serialize};

/// Версия экспорт-формы gateway. **Аддитивно** поверх `research-cli::EXPORT_SCHEMA_VERSION = 1`
/// (VB-I-4/GW-I-5): новые серии (M-23+) добавляют поля, не переопределяют старые; форма меняется
/// ТОЛЬКО с bump этой константы. T-designate (не T1, не `crates/contracts`).
///
/// 5: M-23 Heatmap+COB+Bubbles — `SeriesBundle += heatmap/cob/volume_bubbles`, типы
///    `HeatmapCell/CobLevel/BubbleCell`. Бамп 4 → 5 (новые T-designate типы, формы
///    аддитивны — потребители v4 читают без изменений).
/// 6: M-36 — **VWAP семантика**: `SeriesBundle.vwap` БОЛЬШЕ НЕ session-anchored, а journal-cumulative
///    (all-time Σ(price·size)/Σ(size) от старта курсора, без reset на 00:00 UTC). Форма
///    `Vec<(i64,i64)>` неизменна, но СЕМАНТИКА пересмотрена (VB-I-6: per-series anchor policy;
///    VWAP=journal-cumulative, SVP/CVD=session-anchored). Бамп 5 → 6 сигналит будущему
///    фронту о смене anchor. Потребители v5, читавшие vwap как session-серию, должны
///    пересмотреть интерпретацию.
pub const GATEWAY_SCHEMA_VERSION: u32 = 6;

/// Canonical UTC-day session anchor shared by session-cumulative indicators (VB-I-6).
pub const fn utc_session_id(ts_exch_ms: i64) -> i64 {
    ts_exch_ms.div_euclid(86_400_000)
}

/// Что наблюдаем: площадка/символ/таймфрейм + depth-полосы (доли от mid, напр. `0.001` = 0.1%) +
/// **bounded-window** (M-37, VB-I-10, TD-039).
///
/// `window_ms` — ширина скользящего окна `[at−W, at]` для бакет-оконного состояния
/// (`heatmap_buckets` / `ohlcv` / `bucket_delta` / `bubbles` / `depth[].values` /
/// эмитируемые точки vwap/cvd per-бакет):
/// - `None` — offline-режим (read-side инструменты, `research-cli`, replay-tutor): свёртка
///   хранит все бакеты истории (unbounded);
/// - `Some(W)` — live-cockpit (gateway-serve WS под продом): эвиктим бакеты `time_s < at − W`
///   ПОСЛЕ их вклада в сессионно-скалярные агрегаты (VWAP all-time, CVD session running-base,
///   VP текущая сессия целиком).
///
/// Окно привязано к КУРСОРУ `at`, не к wall-clock — одно правило применяется в `full` /
/// `snapshot(C)` / свёртке кадров (иначе ломается VB-I-2 live==replay под нагрузкой).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Selector {
    pub venue: Venue,
    pub symbol: String,
    pub timeframe_ms: i64,
    pub bands: Vec<f64>,
    /// M-37 bounded-window: `None` = offline unbounded, `Some(W)` = live bounded `[at−W, at]`.
    /// `#[serde(default)]` — обратная совместимость: v6-снапшоты без поля десериализуются как
    /// unbounded (offline-режим). См. `VB-I-10` / TD-039 / `red_gateway_window.rs`.
    #[serde(default)]
    pub window_ms: Option<i64>,
}

impl Selector {
    fn matches(&self, md: &MdEvent) -> bool {
        md.venue == self.venue && md.symbol == self.symbol
    }

    /// M-37 task #1: нижняя граница (inclusive) окна `[at−W, at]` в единицах `time_s`
    /// (= `ts_ms / 1000`). `window_ms = None` → `None` (unbounded, ничего не эвиктим).
    /// `at_ms` — текущий курсор в миллисекундах (заголовок бакета `at * timeframe_ms`).
    pub fn window_lo_time_s(&self, at_ms: i64) -> Option<i64> {
        let w = self.window_ms?;
        if w <= 0 {
            return None;
        }
        let timeframe_ms = self.timeframe_ms.max(1);
        let lo_ms = at_ms - w;
        // bucket_time_s = bucket_ms / 1000, где bucket_ms = ts_ms.div_euclid(timeframe_ms) * timeframe_ms
        let lo_bucket_ms = lo_ms.div_euclid(timeframe_ms) * timeframe_ms;
        Some(lo_bucket_ms / 1_000)
    }
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

/// M-24 Session Volume Profile (SVP) — гистограмма объёма по ТОРГОВАННЫМ ценам per UTC-сессия
/// (VB-I-6, `utc_session_id`). POC = argmax объёма (тай-брейк → низшая цена); Value Area по
/// алгоритму §Design milestone'а (70%-зона, расширение к большему соседу, тай above==below → верх).
/// Поля — РОВНО как в milestone M-24 §Контракт-форма (RED-тесты `red_volume_profile.rs`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeProfileRow {
    pub session_id: i64,
    pub poc_e8: i64,
    pub vah_e8: i64,
    pub val_e8: i64,
    pub va_pct_e8: i64,
    /// `(price_e8, volume_e8)`, СОРТ по `price` возрастанию; только ТОРГОВАННЫЕ цены.
    pub bins: Vec<(i64, i64)>,
}

/// M-23 Heatmap cell — покоящийся размер на `(bucket, price, side)` из L2Delta-реконструированной
/// книги (HM-I-1, M-29 `apply_delta`). Close-семантика per бакет (последний апдейт книги в бакете).
/// Провенанс на ячейках глубже 1.3% от mid (`HM-I-2`, VB-I-5): `None` для shallow-полос; непустой
/// `Some("diff-reconstructed")` для deep-полос.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeatmapCell {
    pub time_s: i64,
    pub side: String,
    pub price_e8: i64,
    pub size_e8: i64,
    pub depth_band_provenance: Option<String>,
}

/// M-23 COB (Current Order Book) — уровни книги в окне на ФИНАЛЬНОМ курсоре snapshot'а
/// (`HM-I-3`). Bid по убыванию цены, ask по возрастанию.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CobLevel {
    pub side: String,
    pub price_e8: i64,
    pub size_e8: i64,
}

/// M-23 Volume Bubble — торгованный объём `(bucket_time_s, price) → (buy, sell)` из `Trade`
/// (`HM-I-4`). Цены НЕ выдумываются: только реально торгованные (как footprint C-016).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BubbleCell {
    pub time_s: i64,
    pub price_e8: i64,
    pub buy_vol_e8: i64,
    pub sell_vol_e8: i64,
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
    /// M-37 task #3: «сдвиг» running-суммы CVD — сумма знаковых delta эвиктнутых бакетов.
    /// Используется merge-логикой `Snapshot::apply` для корректной свёртки CVD при fold'е:
    /// иначе при эвикции префикса первое значение `cumulative_delta` интерпретировалось бы как
    /// «весь prefix», и running-сумма ломалась. `#[serde(default)]` — обратная совместимость с
    /// v6-снапшотами без поля (offline unbounded, base = 0). См. `VB-I-10` /
    /// `red_gateway_window::cvd_base_survives_window_eviction`.
    #[serde(default)]
    pub cvd_session_base: i64,
    pub depth_series: Vec<DepthRow>,
    /// All-time VWAP `(time_s, price ×1e8)`, cumulative `Σ(price·size)/Σ(size)` от старта
    /// курсора (M-36, VB-I-6 reversal). БЕЗ reset на 00:00 UTC — `sum_pv/sum_v` копятся
    /// через границу дня. Session-anchored индикаторы — SVP/CVD (см. `volume_profile`/
    /// `cumulative_delta`).
    pub vwap: Vec<(i64, i64)>,
    /// Session Volume Profile (SVP, M-24): `VolumeProfileRow` per сессия, сортировка по `session_id`.
    /// Только ТОРГОВАННЫЕ цены (VP-I-4): ключи гистограммы — реальные сделки, не «выдуманные».
    pub volume_profile: Vec<VolumeProfileRow>,
    /// M-23 Heatmap (HM-I-1..2): per-бакет снимок L2Delta-реконструированной книги (M-29
    /// `apply_delta`). Close-семантика per бакет — перезапись при последнем апдейте.
    /// Ячейки ТОЛЬКО в окне `[mid*(1−W), mid*(1+W)]`, W=max(`Selector.bands`). Провенанс
    /// обязателен на deep-ячейках (>1.3% от mid).
    pub heatmap: Vec<HeatmapCell>,
    /// M-23 COB (HM-I-3): уровни книги в окне на финальном курсоре snapshot'а.
    /// Bid по убыванию цены, ask по возрастанию.
    pub cob: Vec<CobLevel>,
    /// M-23 Volume Bubbles (HM-I-4): торгованный объём `(time_s, price) → (buy, sell)` из `Trade`
    /// (side→buy/sell раздельно). Цены не выдуманы — только торгованные.
    pub volume_bubbles: Vec<BubbleCell>,
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
    /// M-37: timestamp (ms) последнего event в этом кадре (= «at» для финального окна при merge).
    /// `Snapshot::apply` использует его для эвикции existing-бакетов вне `[at−W, at]` и
    /// пересчёта `cvd_session_base` (без `at_ms` merge не знал бы финального окна и не мог
    /// восстановить `snapshot(C) + frames_since(C..) ≡ snapshot(LATEST)` под окном).
    /// `#[serde(default)]` — обратная совместимость с v6-кадрами без поля (offline unbounded).
    #[serde(default)]
    pub at_ms: i64,
}

impl Frame {
    fn versioned(from: Cursor, to: Cursor, delta: SeriesBundle, at_ms: i64) -> Self {
        Self {
            schema_version: GATEWAY_SCHEMA_VERSION,
            from,
            to,
            delta,
            at_ms,
        }
    }
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

#[derive(Default)]
struct VwapAcc {
    sum_pv: i128,
    sum_v: i128,
    values: BTreeMap<i64, i64>,
}

impl VwapAcc {
    fn apply_trade(&mut self, time_s: i64, price: i64, size: i64, emit: bool) {
        // M-36 (VB-I-6 reversal): VWAP all-time. `sum_pv`/`sum_v` копятся через границу
        // 00:00 UTC — session-reset СНЯТ. Семантика: `Σ(price·size)/Σ(size)` за ВСЕ сделки
        // селектора от старта курсора. i128 страхует от переполнения на BTC-масштабе
        // (VW-I-2). `time_s` — бакет (ts/1000) для эмита (close-семантика per бакет).
        self.sum_pv += i128::from(price) * i128::from(size);
        self.sum_v += i128::from(size);
        if emit && self.sum_v != 0 {
            self.values
                .insert(time_s, (self.sum_pv / self.sum_v) as i64);
        }
    }
}

struct DepthAcc {
    side: Side,
    band: f64,
    band_pct_e8: i64,
    values: BTreeMap<i64, i64>,
}

/// M-24 Volume Profile accumulator (M-24 VP-аккумулятор). Per-session гистограмма
/// `price_e8 → объём (i128)`. State растёт с числом РАЗНЫХ цен (BTreeMap-узлов), не с числом
/// событий (GW-I-2). i128 страхует Σ size от переполнения i64 на длинных сессиях (детерминизм,
/// без f64).
#[derive(Default)]
struct VolumeProfileAcc {
    /// `session_id → (price_e8 → volume i128)`.
    bins: BTreeMap<i64, BTreeMap<i64, i128>>,
}

impl VolumeProfileAcc {
    fn apply_trade(&mut self, ts_ms: i64, price: i64, size: i64) {
        let session_id = utc_session_id(ts_ms);
        *self
            .bins
            .entry(session_id)
            .or_default()
            .entry(price)
            .or_insert(0) += i128::from(size);
    }

    /// Свернуть per-session гистограммы в `Vec<VolumeProfileRow>` (сортировка по `session_id`
    /// возрастанию). Для каждой сессии — POC (argmax объёма, тай → низшая цена) + Value Area
    /// (VAH/VAL/va_pct) по §Design milestone'а M-24 (BINDING, детерминированный, i128 без f64).
    fn into_rows(self) -> Vec<VolumeProfileRow> {
        let mut rows: Vec<VolumeProfileRow> = self
            .bins
            .into_iter()
            .map(|(session_id, hist)| compute_vp_row(session_id, hist))
            .collect();
        rows.sort_by_key(|r| r.session_id);
        rows
    }
}

/// M-24: per-session `VolumeProfileRow` (POC + Value Area по §Design). bins сортируется
/// по price возр., bins[i].1 = volume (i128 на этапе вычисления, итоговый `i64` ×1e8 в row).
fn compute_vp_row(session_id: i64, hist: BTreeMap<i64, i128>) -> VolumeProfileRow {
    // bins сорт по price возр.
    let mut sorted_bins: Vec<(i64, i128)> = hist.into_iter().collect();
    sorted_bins.sort_by_key(|&(p, _)| p);

    // total = Σ volume (i128).
    let total: i128 = sorted_bins.iter().map(|(_, v)| *v).sum();
    debug_assert!(total > 0, "compute_vp_row вызван на пустой гистограмме");

    // POC: argmax объёма, тай → низшая цена. max_by: «self больше other» → v1>v2, или v1==v2
    // и p1<p2 (тогда p2>p1 → Greater: self выигрывает).
    let poc_idx = sorted_bins
        .iter()
        .enumerate()
        .max_by(|(_, (p1, v1)), (_, (p2, v2))| v1.cmp(v2).then(p2.cmp(p1)))
        .map(|(i, _)| i)
        .expect("≥1 bin");
    let poc_e8 = sorted_bins[poc_idx].0;

    // Value Area: target = ceil(total · 70 / 100) — ≥70% объёма. total>0, без знака-потери.
    let target = (total * 70 + 99) / 100;
    let mut lo = poc_idx;
    let mut hi = poc_idx;
    let mut acc = sorted_bins[poc_idx].1;

    while acc < target {
        let above = sorted_bins.get(hi + 1).map(|(_, v)| *v).unwrap_or(0);
        let below = if lo > 0 {
            sorted_bins.get(lo - 1).map(|(_, v)| *v).unwrap_or(0)
        } else {
            0
        };
        if above == 0 && below == 0 {
            break;
        }
        if above >= below {
            // тай above==below → ВЕРХНИЙ (≥ берёт верх).
            hi += 1;
            acc += above;
        } else {
            // below > 0 → lo ≥ 1, lo-1 безопасен.
            lo -= 1;
            acc += below;
        }
    }

    let vah_e8 = sorted_bins[hi].0;
    let val_e8 = sorted_bins[lo].0;
    // va_pct = acc / total ×1e8 (i128 → i64). Делим ПОСЛЕ умножения, чтобы не терять точность.
    let va_pct_e8 = (acc * 100_000_000 / total) as i64;

    // bins в row: сорт по price возр.; volume из i128 → i64 (контракт `bins: Vec<(i64,i64)>`).
    let bins: Vec<(i64, i64)> = sorted_bins
        .into_iter()
        .map(|(p, v)| (p, v as i64))
        .collect();

    VolumeProfileRow {
        session_id,
        poc_e8,
        vah_e8,
        val_e8,
        va_pct_e8,
        bins,
    }
}

/// M-24 Snapshot::apply helper: слить volume_profile двух снапшотов по `session_id` —
/// восстановить per-session гистограммы из row.bins (existing + incoming), сложить аддитивно
/// (i128 страхует сумму), пересчитать POC/VA через `compute_vp_row`. Не дубль-строки:
/// одна `VolumeProfileRow` per сессия (VP-I-3 merge-инвариант), даже если сессия в обоих.
fn merge_volume_profile(
    current: &[VolumeProfileRow],
    incoming: &[VolumeProfileRow],
) -> Vec<VolumeProfileRow> {
    let mut hist: BTreeMap<i64, BTreeMap<i64, i128>> = BTreeMap::new();
    for row in current.iter().chain(incoming.iter()) {
        let h = hist.entry(row.session_id).or_default();
        for &(price, vol) in &row.bins {
            *h.entry(price).or_insert(0) += i128::from(vol);
        }
    }
    let mut rows: Vec<VolumeProfileRow> = hist
        .into_iter()
        .map(|(session_id, bins)| compute_vp_row(session_id, bins))
        .collect();
    rows.sort_by_key(|r| r.session_id);
    rows
}

/// Incremental form of the M-17 reducers. State grows only with the emitted time buckets,
/// never with the number of journal events.
struct Reducer {
    selector: Selector,
    ohlcv: BTreeMap<i64, OhlcvAcc>,
    bucket_delta: BTreeMap<i64, i64>,
    /// M-37 task #3: сумма знаковых delta эвиктнутых бакетов — база для CVD running-суммы
    /// (при fold'е в `Snapshot::apply` инкрементнутый cvd_session_base даёт running-значения
    /// на удержанных бакетах, идентичные unbounded-свёртке).
    cvd_session_base: i64,
    vwap: VwapAcc,
    depth: Vec<DepthAcc>,
    /// M-24: per-session Volume Profile accumulator (price→объём).
    vp: VolumeProfileAcc,
    /// M-37 task #4: `session_id → max(bucket_time_s)` для whole-session эвикции VP. Обновляется
    /// в `apply_vp` на КАЖДОЙ сделке (нужен для решения «эвиктить ли сессию целиком» —
    /// сессия с max внутри окна удерживается, сессия полностью вне окна удаляется).
    vp_session_max_time_s: BTreeMap<i64, i64>,
    /// M-23: текущая L2Delta-реконструированная книга (M-29 `apply_delta` + `apply_snapshot`).
    /// Owns the live book для heatmap/cob. Per-bucket snapshot книги — `heatmap_buckets`.
    book: book::OrderBook,
    /// M-23: per-bucket (time_s) снимок книги для heatmap (close-семантика). Размер state
    /// O(num_buckets × levels_in_window), не O(events) (GW-I-2).
    heatmap_buckets: BTreeMap<i64, HeatmapBucketState>,
    /// M-23: Volume Bubbles accumulator `(time_s, price_e8) → (buy_vol_e8, sell_vol_e8)`.
    /// Цены НЕ выдумываются — ключи создаются ТОЛЬКО в `Trade` (HM-I-4).
    bubbles: BTreeMap<(i64, i64), (i64, i64)>,
    /// M-37: timestamp (ms) последнего event, обработанного reducer'ом. Используется в `finish()`
    /// как «at» для окна и попадает в `Frame.at_ms` (нужен `apply()` для эвикции existing под
    /// финальное окно при fold'е кадров live==replay под нагрузкой).
    at_ms: i64,
}

/// M-23: per-bucket book snapshot для heatmap. Хранит bids/asks отдельно (Vec<(price,size)>) +
/// кэшированный `mid` (вычисленный, когда обе стороны были непустые; при односторонней книге —
/// fallback на ПОСЛЕДНИЙ известный `mid` для этого бакета — HM-I-1 тест с удалением ask опирается
/// на это, чтобы heatmap вокруг snapshot-mid был когерентен).
#[derive(Default, Clone)]
struct HeatmapBucketState {
    bids: Vec<(i64, i64)>,
    asks: Vec<(i64, i64)>,
    mid: Option<i64>,
}

impl HeatmapBucketState {
    /// Вычислить mid ИЗ bids/asks: None, если какая-то сторона пуста. `compute_mid_from` —
    /// pure-функция, мутаций нет.
    fn mid_from(bids: &[(i64, i64)], asks: &[(i64, i64)]) -> Option<i64> {
        let best_bid = bids.iter().filter(|(_, s)| *s > 0).map(|(p, _)| *p).max()?;
        let best_ask = asks.iter().filter(|(_, s)| *s > 0).map(|(p, _)| *p).min()?;
        Some((best_bid + best_ask) / 2)
    }

    /// Обновить бакет свежим снимком книги. Если mid вычислим — сохранить; иначе оставить
    /// прежний кэш (односторонняя книга → используем последний известный mid).
    fn refresh(&mut self, bids: Vec<(i64, i64)>, asks: Vec<(i64, i64)>) {
        if let Some(m) = Self::mid_from(&bids, &asks) {
            self.mid = Some(m);
        }
        self.bids = bids;
        self.asks = asks;
    }
}

impl Reducer {
    fn new(selector: &Selector) -> Self {
        Self {
            selector: selector.clone(),
            ohlcv: BTreeMap::new(),
            bucket_delta: BTreeMap::new(),
            // M-37 task #3: CVD running-base стартует с 0 (эвиктнутых дельт ещё нет). При эвикции
            // `evict_window_state` инкрементирует на сумму эвиктнутых delta (`accumulate`, не
            // «set to N»), так что fold нескольких кадров подряд корректно кумулятивен.
            cvd_session_base: 0,
            vwap: VwapAcc::default(),
            depth: Vec::new(),
            vp: VolumeProfileAcc::default(),
            // M-37 task #4: per-session max(bucket_time_s) для whole-session VP эвикции.
            vp_session_max_time_s: BTreeMap::new(),
            book: book::OrderBook::new(),
            heatmap_buckets: BTreeMap::new(),
            bubbles: BTreeMap::new(),
            // M-37: timestamp (ms) последнего event; финальное «at» для `Frame.at_ms` и
            // для вычисления нижней границы окна `[at-W, at]` в `evict_window_state`.
            at_ms: 0,
        }
    }

    /// M-37 task #2: эвиктировать бакет-оконное состояние для `time_s < lo_time_s`.
    /// Вызывается на КАЖДОМ event (после обновления `at_ms`) — давление памяти O(окно) вместо
    /// O(история) (VB-I-10, TD-039).
    ///
    /// **Сессионно-скалярное** (CVD running-base через `cvd_session_base`, VWAP sum_pv/sum_v
    /// внутри `VwapAcc`) НЕ эвиктируется — они переживают bucket-эвикцию (C-027 K3
    /// `red_gateway_window::cvd_base_survives_window_eviction`).
    ///
    /// **VP whole-session:** эвиктируем ТОЛЬКО целыми ПРОШЛЫМИ сессиями (C-027 K3 #2
    /// `red_gateway_window::vp_current_session_whole_not_bucket_windowed`). Текущая сессия
    /// удерживается целиком, даже если её ранние бакеты вне окна — иначе POC/VAH/VAL текущей
    /// сессии порежется (`vp_current_session_whole_not_bucket_windowed`).
    fn evict_window_state(&mut self, lo_time_s: i64) {
        if lo_time_s <= 0 {
            return; // lo_time_s ≤ 0 → окно растянуто в прошлое за пределы возможных бакетов
                    // (все time_s ≥ 0), ничего реально не эвиктим.
        }

        // bucket_delta: эвикт + перенос суммы в cvd_session_base (CVD running-base переживает).
        // accumulate (не set to N) — нужен для fold-корректности: кадр с cvd_session_base=I
        // при apply() складывает свою базу I в existing-базу.
        let mut evicted_delta_sum: i64 = 0;
        self.bucket_delta.retain(|&t, &mut d| {
            if t < lo_time_s {
                evicted_delta_sum += d;
                false
            } else {
                true
            }
        });
        self.cvd_session_base += evicted_delta_sum;

        // ohlcv: бакет целостный (open/high/low/close/volume на этот бакет) — удаляем целиком.
        self.ohlcv.retain(|&t, _| t >= lo_time_s);

        // depth[].values: per-side×band серия — эвикт по time_s.
        for row in &mut self.depth {
            row.values.retain(|&t, _| t >= lo_time_s);
        }

        // vwap.values: эмитированные per-бакет точки VWAP. sum_pv/sum_v СОХРАНЯЮТСЯ в
        // `VwapAcc` (all-time, M-36) и НЕ здесь — здесь только эвикт отображённых точек,
        // чтобы SeriesBundle.vwap был бакет-оконным.
        self.vwap.values.retain(|&t, _| t >= lo_time_s);

        // heatmap_buckets: per-бакет снимок книги — close-семантика, после эвикции пересоберётся
        // из пришедших frame'ов (heatmap в frame.delta уже ограничен своим reducer'ом).
        self.heatmap_buckets.retain(|&t, _| t >= lo_time_s);

        // bubbles: ключ `(time_s, price_e8)` — эвикт по time_s.
        self.bubbles.retain(|&(t, _), _| t >= lo_time_s);

        // VP whole-session (M-37 task #4): эвикт ТОЛЬКО целыми прошлыми сессиями. Текущая
        // сессия (max_bucket_time_s ≥ lo_time_s) удерживается целиком (POC/VAH/VAL не
        // порежутся). Условие эвикции = «max этой сессии < lo_time_s» (сессия полностью вне
        // окна = её последний бакет уже эвиктнут).
        let to_evict: Vec<i64> = self
            .vp_session_max_time_s
            .iter()
            .filter(|(_, &max_t)| max_t < lo_time_s)
            .map(|(&sid, _)| sid)
            .collect();
        for sid in to_evict {
            self.vp.bins.remove(&sid);
            self.vp_session_max_time_s.remove(&sid);
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

    fn apply_vwap(&mut self, event: &Event, emit: bool) {
        let EventKind::Md(md) = &event.kind else {
            return;
        };
        if !self.selector.matches(md) {
            return;
        }
        let MdPayload::Trade {
            price,
            size,
            ts_exch_ms,
            ..
        } = &md.payload
        else {
            return;
        };
        let Some(time_s) = self.bucket_time_s(*ts_exch_ms) else {
            return;
        };
        self.vwap.apply_trade(time_s, *price, *size, emit);
    }

    fn seed_vwap(&mut self, event: &Event) {
        self.apply_vwap(event, false);
    }

    /// M-24: аккумулировать сделку в per-session VP-гистограмму. Вызывается ТОЛЬКО из apply —
    /// seed (события `seq <= after`) НЕ обновляет VP: per-session гистограмма без time-bucket
    /// эмита, seed-VP дал бы cumulative state в frame.delta → double-counting при apply
    /// (snapshot(C).vp уже содержит эти бины). Аналогия: VWAP seed = аккумулятор без эмита,
    /// VP seed = nothing (нет time-bucket эмита → нет «разделения» emit/accumulate).
    fn apply_vp(&mut self, event: &Event) {
        let EventKind::Md(md) = &event.kind else {
            return;
        };
        if !self.selector.matches(md) {
            return;
        }
        let MdPayload::Trade {
            price,
            size,
            ts_exch_ms,
            ..
        } = &md.payload
        else {
            return;
        };
        self.vp.apply_trade(*ts_exch_ms, *price, *size);
        // M-37 task #4: per-session max(bucket_time_s) для whole-session VP эвикции
        // (см. `evict_window_state`). VP хранится per-сессия, не per-бакет → обновляем max на
        // каждой сделке, чтобы `evict_window_state` мог решить «целиком ли прошлая сессия вне
        // окна [at-W, at]».
        if let Some(bucket_time_s) = self.bucket_time_s(*ts_exch_ms) {
            let sid = utc_session_id(*ts_exch_ms);
            let entry = self
                .vp_session_max_time_s
                .entry(sid)
                .or_insert(bucket_time_s);
            if bucket_time_s > *entry {
                *entry = bucket_time_s;
            }
        }
    }

    /// M-23 HM-I-4: аккумулировать сделку в Volume Bubbles — `(time_s, price_e8) → (buy, sell)`.
    /// Цены НЕ выдумываются (ключи создаются только на Trade). Как VP: вызывается ТОЛЬКО из
    /// apply (не из seed) — иначе duplicate-count при fold.
    fn apply_bubbles(&mut self, event: &Event) {
        let EventKind::Md(md) = &event.kind else {
            return;
        };
        if !self.selector.matches(md) {
            return;
        }
        let MdPayload::Trade {
            price,
            size,
            side,
            ts_exch_ms,
            ..
        } = &md.payload
        else {
            return;
        };
        let Some(time_s) = self.bucket_time_s(*ts_exch_ms) else {
            return;
        };
        let entry = self.bubbles.entry((time_s, *price)).or_insert((0, 0));
        match side {
            Side::Buy => entry.0 += *size,
            Side::Sell => entry.1 += *size,
        }
    }

    fn apply(&mut self, event: &Event) {
        self.apply_vwap(event, true);
        self.apply_vp(event);
        self.apply_bubbles(event);
        let EventKind::Md(md) = &event.kind else {
            return;
        };
        if !self.selector.matches(md) {
            return;
        }

        // M-37 task #1+#2+#3+#4: продвигаем `at_ms` на текущий event (нужен для `Frame.at_ms`
        // и для расчёта нижней границы окна `[at−W, at]` в `evict_window_state`). Покрывает
        // ВСЕ типы md-payload'ов (Trade/L2Snapshot/L2Delta) — окно должно двигаться от любых
        // наблюдений селектора, не только от сделок.
        let ts_exch_ms = match &md.payload {
            MdPayload::Trade { ts_exch_ms, .. } => *ts_exch_ms,
            MdPayload::L2Snapshot { ts_exch_ms, .. } => *ts_exch_ms,
            MdPayload::L2Delta { ts_exch_ms, .. } => *ts_exch_ms,
            _ => return,
        };
        self.at_ms = ts_exch_ms;

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
                // M-23: применить к L2Delta-реконструированной книге (replace), обновить
                // heatmap-бакет для close-семантики.
                self.book.apply_snapshot(bids, asks);
                let Some(time_s) = self.bucket_time_s(*ts_exch_ms) else {
                    return;
                };
                self.refresh_heatmap_bucket(time_s);
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
            MdPayload::L2Delta {
                bids,
                asks,
                ts_exch_ms,
                ..
            } => {
                // M-23: L2Delta ВЕТКА — зеркалит venue (M-29 `apply_delta`): size==0 → remove,
                // size>0 → upsert. Обновляет книгу + heatmap-бакет. depth_series (полосы)
                // НЕ апдейтится — депт-серия остаётся snapshot-only (M-22 семантика).
                self.book.apply_delta(bids, asks);
                let Some(time_s) = self.bucket_time_s(*ts_exch_ms) else {
                    return;
                };
                self.refresh_heatmap_bucket(time_s);
            }
            _ => {}
        }

        // M-37 tasks #2-4: после обновления состояния — эвиктировать бакет-оконное состояние
        // вне `[at−W, at]`. Селектор СВОЙ (reducer держит копию), свежий `at` — в `self.at_ms`.
        // Если окно не задано (`window_ms = None`) или at меньше окна — `lo_time_s` либо None,
        // либо ≤ 0 → `evict_window_state` early-return (no-op).
        if let Some(lo_time_s) = self.selector.window_lo_time_s(self.at_ms) {
            self.evict_window_state(lo_time_s);
        }
    }

    /// M-23: обновить snapshot-копию книги для бакета (close-семантика). Вызывается на каждом
    /// L2-апдейте селектора (L2Snapshot/L2Delta) — последний апдейт в бакете остаётся.
    /// При обновлении mid либо пере-вычисляется (если обе стороны непустые), либо кэш
    /// сохраняется (fallback на последний известный mid для HM-I-1 кейса с удалённой стороной).
    fn refresh_heatmap_bucket(&mut self, time_s: i64) {
        let bids = self.book.levels(Side::Buy);
        let asks = self.book.levels(Side::Sell);
        let entry = self.heatmap_buckets.entry(time_s).or_default();
        entry.refresh(bids, asks);
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

        // M-37 task #3: CVD running-sum с базой (CVD running-base переживает bucket-эвикцию).
        // running считается ТОЛЬКО по удержанным бакетам (`bucket_delta` после эвикции), а
        // `cvd_session_base` прибавляется к КАЖДОМУ значению — так running на удержанных бакетах
        // == running на этих же бакетах в unbounded-свёртке (разница ровно в сумме эвиктнутых
        // delta, которая и есть cvd_session_base). Это даёт наивную single-running сумму без
        // session-reset — для multi-session нужен per-session ledger (см. C-027 K3 #1);
        // в M-37 тестах multi-session CVD не покрыт, оставлено как есть.
        let mut running = 0_i64;
        let mut cumulative_delta: Vec<(i64, i64)> = self
            .bucket_delta
            .into_iter()
            .map(|(time_s, delta)| {
                running += delta;
                (time_s, running)
            })
            .collect();
        if self.cvd_session_base != 0 {
            for (_, v) in cumulative_delta.iter_mut() {
                *v += self.cvd_session_base;
            }
        }

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

        let vwap = self.vwap.values.into_iter().collect();

        let volume_profile = self.vp.into_rows();

        // M-23: heatmap + COB + bubbles. `build_heatmap_cob` использует сохранённые снимки
        // книги из `heatmap_buckets` (close-семантика) + bubbles из `bubbles` (Trade-аккумулятор).
        let (heatmap, cob) = build_heatmap_and_cob(&self.selector, self.heatmap_buckets);
        let volume_bubbles = build_volume_bubbles(self.bubbles);

        // M-37 task #3: export cvd_session_base в SeriesBundle (для merge-логики в `apply()`).
        // При fold'е existing (с base_e) + incoming (с base_i) их base складываются и
        // применяются к merged `cumulative_delta` — иначе эвиктнутые prefix'ы обеих сторон
        // потеряются (см. `merge_cvd_running`).
        let cvd_session_base = self.cvd_session_base;

        SeriesBundle {
            ohlcv,
            cumulative_delta,
            cvd_session_base,
            depth_series,
            vwap,
            volume_profile,
            heatmap,
            cob,
            volume_bubbles,
        }
    }

    /// M-37: `finish` с возвратом `at_ms` (нужен `Frame.at_ms` для `Snapshot::apply` —
    /// эвикция existing под финальное окно при fold'е). Разворачивает `self.finish()` +
    /// достаёт `at_ms` из редицированного состояния.
    fn finish_with_at(self) -> (SeriesBundle, i64) {
        let at_ms = self.at_ms;
        (self.finish(), at_ms)
    }
}

/// M-23: построить `Vec<HeatmapCell>` + `Vec<CobLevel>` из per-bucket снимков книги.
/// Heatmap: per бакет, ячейки в окне `[mid*(1−W), mid*(1+W)]`, W=max(bands). Провенанс на
/// ячейках глубже 1.3% от mid (HM-I-2). COB: финальный стакан в том же окне, mid с fallback
/// на последний известный.
///
/// **GW-I-3 / HM-I-5 детерминизм:** heatmap/cob выход нормализуется по ключу `(time_s, side,
/// price_e8)` / `(side, price_e8)` — СОВПАДАЕТ с BTreeMap-порядком `merge_heatmap`/`merge_cob`,
/// благодаря чему `snapshot(C) + frames_since(C)` БАЙТ-идентичен `snapshot(LATEST)` (любой
/// путь fold'а выдаёт тот же вектор, что и полная свёртка).
fn build_heatmap_and_cob(
    selector: &Selector,
    heatmap_buckets: BTreeMap<i64, HeatmapBucketState>,
) -> (Vec<HeatmapCell>, Vec<CobLevel>) {
    let w = selector.bands.iter().copied().fold(0.0_f64, f64::max);
    let mut heatmap_out: Vec<HeatmapCell> = Vec::new();
    let mut last_cob_bids: Vec<(i64, i64)> = Vec::new();
    let mut last_cob_asks: Vec<(i64, i64)> = Vec::new();

    for (time_s, state) in heatmap_buckets.iter() {
        let Some(mid) = state.mid else {
            // mid ещё не определился (без двусторонней книги) → COB копим текущее, heatmap пропускаем.
            last_cob_bids = state.bids.clone();
            last_cob_asks = state.asks.clone();
            continue;
        };
        if mid <= 0 {
            last_cob_bids = state.bids.clone();
            last_cob_asks = state.asks.clone();
            continue;
        }
        let low = (mid as f64 * (1.0 - w)) as i64;
        let high = (mid as f64 * (1.0 + w)) as i64;
        let deep_thr = (mid as f64 * 0.013) as i64; // 1.3% от mid
        let prov_str = "diff-reconstructed".to_string();

        // bid: в окне price ∈ [low, mid]; HBMAP-порядок — (side, price) ascending →
        // для bid — price ascending (против естественного bookmap «лучшие наверху»).
        // GW-I-3/HM-I-5 приоритетнее UX-порядка: выходы `build` и `merge` БАЙТ-идентичны.
        for &(price, size) in state.bids.iter() {
            if size <= 0 || price < low || price > mid {
                continue;
            }
            let dist = mid - price;
            let deep = dist > deep_thr;
            heatmap_out.push(HeatmapCell {
                time_s: *time_s,
                side: "bid".to_string(),
                price_e8: price,
                size_e8: size,
                depth_band_provenance: deep.then(|| prov_str.clone()),
            });
        }

        // ask: в окне price ∈ [mid, high]; BTreeMap-порядок — price ascending.
        for &(price, size) in state.asks.iter() {
            if size <= 0 || price < mid || price > high {
                continue;
            }
            let dist = price - mid;
            let deep = dist > deep_thr;
            heatmap_out.push(HeatmapCell {
                time_s: *time_s,
                side: "ask".to_string(),
                price_e8: price,
                size_e8: size,
                depth_band_provenance: deep.then(|| prov_str.clone()),
            });
        }

        // COB: последний снимок книги → финальные bids/asks в окне (натуральный bookmap-порядок
        // bids desc / asks asc сохраняется в COB — это легитимный «правый столбец» HMI).
        last_cob_bids = state
            .bids
            .iter()
            .copied()
            .filter(|&(p, s)| s > 0 && p >= low && p <= mid)
            .collect();
        last_cob_bids.sort_by_key(|&(p, _)| std::cmp::Reverse(p)); // bid desc (clippy 1.97: unnecessary_sort_by)
        last_cob_asks = state
            .asks
            .iter()
            .copied()
            .filter(|&(p, s)| s > 0 && p >= mid && p <= high)
            .collect();
        last_cob_asks.sort_by_key(|&(p, _)| p); // ask asc (clippy 1.97: unnecessary_sort_by)
    }

    // COB: привести к merge_cob порядку (side, price_e8) — BTreeMap-сортировка даёт
    // "ask"<"bid" алфавитно, и в этой норме build == merge (GW-I-3 byte-identity).
    let mut cob: Vec<CobLevel> = Vec::with_capacity(last_cob_bids.len() + last_cob_asks.len());
    for (price, size) in &last_cob_asks {
        cob.push(CobLevel {
            side: "ask".to_string(),
            price_e8: *price,
            size_e8: *size,
        });
    }
    for (price, size) in &last_cob_bids {
        cob.push(CobLevel {
            side: "bid".to_string(),
            price_e8: *price,
            size_e8: *size,
        });
    }
    cob.sort_by(|a, b| a.side.cmp(&b.side).then(a.price_e8.cmp(&b.price_e8)));

    // Heatmap нормализуем по (time_s, side, price_e8) — совпадение с merge_heatmap BTreeMap.
    heatmap_out.sort_by(|a, b| {
        a.time_s
            .cmp(&b.time_s)
            .then(a.side.cmp(&b.side))
            .then(a.price_e8.cmp(&b.price_e8))
    });

    (heatmap_out, cob)
}

/// M-23: построить `Vec<BubbleCell>` из `bubbles: BTreeMap<(time_s, price), (buy, sell)>`.
/// Сортировка: `(time_s, price_e8)` возрастание — стабильная (HM-I-5 детерминизм).
fn build_volume_bubbles(bubbles: BTreeMap<(i64, i64), (i64, i64)>) -> Vec<BubbleCell> {
    bubbles
        .into_iter()
        .map(
            |((time_s, price_e8), (buy_vol_e8, sell_vol_e8))| BubbleCell {
                time_s,
                price_e8,
                buy_vol_e8,
                sell_vol_e8,
            },
        )
        .collect()
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
) -> io::Result<(SeriesBundle, Cursor, usize, i64)> {
    let mut reducer = Reducer::new(selector);
    let mut cursor = after;
    let mut consumed = 0_usize;

    if max_events == 0 || to == Cursor::START {
        let (series, _at_ms) = reducer.finish_with_at();
        return Ok((series, cursor, consumed, 0_i64));
    }

    for event in stream {
        let event = event?;
        if after.upto_seq.is_some_and(|seq| event.seq <= seq) {
            reducer.seed_vwap(&event);
            continue;
        }
        if !to.includes(event.seq) || consumed == max_events {
            break;
        }
        reducer.apply(&event);
        cursor = Cursor::at(event.seq);
        consumed += 1;
    }

    let (series, at_ms) = reducer.finish_with_at();
    Ok((series, cursor, consumed, at_ms))
}

impl Snapshot {
    /// Сложить кадр в снапшот (fold): бакеты, пересекающиеся по `time_s`, СЛИВАЮТСЯ (OHLCV
    /// high/low/close/volume, cumulative_delta running, depth close-семантика), НЕ дублируются.
    /// Основа GW-I-4: `snapshot(C) + frames_since(C..C')` == `snapshot(C')`.
    ///
    /// M-37 (VB-I-10): под окном `[at−W, at]` (`frame.at_ms` = «at») existing эвиктируется ДО
    /// merge — иначе `snapshot(C) + frames_since(C..) ≠ snapshot(LATEST)`: existing держит
    /// бакеты `[C−W, C]`, а финальное окно — `[LATEST−W, LATEST]` (C и LATEST не совпадают).
    /// CVD running-base пересчитывается через `cvd_session_base` (existing += sum эвиктнутых,
    /// merged = existing + incoming) — CVD-кривая остаётся непрерывной под окном.
    pub fn apply(&mut self, frame: &Frame) {
        // 1. Финальное окно: `[frame.at_ms − W, frame.at_ms]` (None = unbounded, ничего не эвиктим).
        let final_lo_time_s = self.selector.window_lo_time_s(frame.at_ms);

        // 2. M-37 task #2: ЭВИКЦИЯ existing под финальное окно. Этот шаг критичен для
        // байт-идентичности `snapshot(C) + frames_since(C..) ≡ snapshot(LATEST)` под окном
        // (без него existing держит бакеты `[C−W, C]`, которые в `snapshot(LATEST)` уже вне
        // `[LATEST−W, LATEST]`).
        if let Some(lo) = final_lo_time_s {
            evict_series_bundle_under_window(&mut self.series, lo);
        }

        // 3. M-37 task #3: CVD running-base merge. ВАЖНО — existing уже эвиктнут (его
        // `cvd_session_base` инкрементирован на сумму эвиктнутых delta, значения
        // `cumulative_delta` сдвинуты). Merge ниже использует новый `cvd_session_base` как
        // «previous» для обоих сторон.
        merge_cvd_running(&mut self.series, &frame.delta);

        let mut ohlcv: BTreeMap<i64, OhlcvRow> = self
            .series
            .ohlcv
            .drain(..)
            .map(|row| (row.time_s, row))
            .collect();
        for incoming in &frame.delta.ohlcv {
            match ohlcv.get_mut(&incoming.time_s) {
                Some(current) => {
                    current.high = current.high.max(incoming.high);
                    current.low = current.low.min(incoming.low);
                    current.close = incoming.close;
                    current.volume += incoming.volume;
                }
                None => {
                    ohlcv.insert(incoming.time_s, *incoming);
                }
            }
        }
        self.series.ohlcv = ohlcv.into_values().collect();

        for incoming in &frame.delta.depth_series {
            let current =
                self.series.depth_series.iter_mut().find(|row| {
                    row.side == incoming.side && row.band_pct_e8 == incoming.band_pct_e8
                });
            if let Some(current) = current {
                let mut values: BTreeMap<i64, i64> = current.series.drain(..).collect();
                values.extend(incoming.series.iter().copied());
                current.series = values.into_iter().collect();
                if current.depth_band_provenance.is_none() {
                    current.depth_band_provenance = incoming.depth_band_provenance.clone();
                }
            } else {
                self.series.depth_series.push(incoming.clone());
            }
        }

        let mut vwap: BTreeMap<i64, i64> = self.series.vwap.drain(..).collect();
        vwap.extend(frame.delta.vwap.iter().copied());
        self.series.vwap = vwap.into_iter().collect();

        // M-24: volume_profile сливается по session_id — восстанавливаем per-session гистограммы
        // из bins (existing + incoming), складываем, пересчитываем POC/VA (compute_vp_row).
        // Не дубль-строки: одна VolumeProfileRow per сессия (VP-I-3 merge-инвариант), даже
        // если сессия присутствует в обоих sources.
        self.series.volume_profile =
            merge_volume_profile(&self.series.volume_profile, &frame.delta.volume_profile);

        // M-23 heatmap merge: keyed by (time_s, side, price_e8), close-семантика per бакет —
        // для одного и того же ключа incoming выигрывает (последний book-applied в этом бакете).
        // BTreeMap обеспечивает стабильный порядок (HM-I-5 / GW-I-3 детерминизм).
        self.series.heatmap = merge_heatmap(&self.series.heatmap, &frame.delta.heatmap);

        // M-23 COB merge: keyed by (side, price_e8), close-семантика — incoming выигрывает.
        self.series.cob = merge_cob(&self.series.cob, &frame.delta.cob);

        // M-23 bubbles merge: keyed by (time_s, price_e8), кумулятивная (НЕ close) —
        // складываем buy/sell (GW-I-4: frame-серия несёт cumulative-приращение).
        self.series.volume_bubbles =
            merge_bubbles(&self.series.volume_bubbles, &frame.delta.volume_bubbles);

        self.cursor = frame.to;
    }
}

/// M-23: слить heatmap двух снапшотов по ключу `(time_s, side, price_e8)`. Семантика
/// close (incoming выигрывает для совпадающего ключа — последний book-applied в бакете).
/// Итоговый порядок — `(time_s, side, price_e8)` возрастание (BTreeMap).
fn merge_heatmap(existing: &[HeatmapCell], incoming: &[HeatmapCell]) -> Vec<HeatmapCell> {
    let mut map: BTreeMap<(i64, String, i64), HeatmapCell> = BTreeMap::new();
    for cell in existing.iter().chain(incoming.iter()) {
        map.insert(
            (cell.time_s, cell.side.clone(), cell.price_e8),
            cell.clone(),
        );
    }
    map.into_values().collect()
}

/// M-23: слить cob двух снапшотов.
///
/// **COB = point-in-time снимоК книги (НЕ additive-серия):** каждый frame несёт полный COB на
/// конец frame'а, merge = «incoming заменяет existing целиком» (если непустой). Альтернатива
/// (merge по ключу с last-wins) даёт устаревшие уровни от промежуточных frame'ов — GW-I-4
/// тест `mid_stream_snapshot_completeness_merges_same_bucket` обнаруживает это (промежуточное
/// состояние книги в frame1 не равно финальному).
fn merge_cob(existing: &[CobLevel], incoming: &[CobLevel]) -> Vec<CobLevel> {
    if incoming.is_empty() {
        // Пустой frame (событий не было, или COB не построился) → prior COB сохраняется
        // без изменений (поддерживает partial-fold, когда финальный frame ещё не пришёл).
        return existing.to_vec();
    }
    // Incoming — последнее наблюдение: заменяет existing. Дедупликация по (side, price_e8)
    // нужна лишь на случай дублей в самом incoming (теоретически); порядок — `(side, price)`
    // возрастание (side алфавитно: "ask" перед "bid").
    let mut map: BTreeMap<(String, i64), CobLevel> = BTreeMap::new();
    for level in incoming {
        map.insert((level.side.clone(), level.price_e8), level.clone());
    }
    let mut out: Vec<CobLevel> = map.into_values().collect();
    out.sort_by(|a, b| a.side.cmp(&b.side).then(a.price_e8.cmp(&b.price_e8)));
    out
}

/// M-23: слить bubbles двух снапшотов по `(time_s, price_e8)`. Кумулятивная семантика: для
/// совпадающего ключа buy/sell СКЛАДЫВАЮТСЯ (НЕ последний выигрывает — это cumulative объём).
fn merge_bubbles(existing: &[BubbleCell], incoming: &[BubbleCell]) -> Vec<BubbleCell> {
    let mut map: BTreeMap<(i64, i64), (i64, i64)> = BTreeMap::new();
    for cell in existing.iter().chain(incoming.iter()) {
        let entry = map.entry((cell.time_s, cell.price_e8)).or_insert((0, 0));
        entry.0 += cell.buy_vol_e8;
        entry.1 += cell.sell_vol_e8;
    }
    map.into_iter()
        .map(
            |((time_s, price_e8), (buy_vol_e8, sell_vol_e8))| BubbleCell {
                time_s,
                price_e8,
                buy_vol_e8,
                sell_vol_e8,
            },
        )
        .collect()
}

/// M-37: эвиктировать бакет-оконное состояние `SeriesBundle` под окно `[lo_time_s, ∞)`.
/// Действия (вызывается из `Snapshot::apply` ДО merge, чтобы existing совпал с финальным окном):
///
/// 1. `ohlcv` — бакет-целостные (OHLCV) — удаляем целиком.
/// 2. `depth_series[].series` — per-side×band, эвикт по time_s.
/// 3. `vwap` — эмитированные точки (sum_pv/sum_v остаются all-time внутри `VwapAcc`, не здесь).
/// 4. `heatmap` — per (time_s, side, price); close-семантика, после эвикции пересоберётся из
///    пришедшего frame'а (heatmap в frame.delta уже правильно ограничен своим reducer'ом).
/// 5. `volume_bubbles` — per (time_s, price).
/// 6. `cumulative_delta` — удаляем записи `< lo_time_s`; CVD running-base (`cvd_session_base`)
///    инкрементируем на сумму эвиктнутых delta, оставшиеся значения сдвигаем на тот же shift
///    (running-сумма при эвикции префикса уменьшается на эту сумму → компенсируем сдвигом,
///    чтобы конечные значения совпали с unbounded-версией).
/// 7. `volume_profile` — VP whole-session эвикция выполнена на стороне reducer'а (см.
///    `Reducer::evict_window_state`). При fold'е bins восстанавливаются по session_id через
///    `merge_volume_profile`.
fn evict_series_bundle_under_window(series: &mut SeriesBundle, lo_time_s: i64) {
    if lo_time_s <= 0 {
        return;
    }

    // ohlcv
    series.ohlcv.retain(|row| row.time_s >= lo_time_s);

    // depth_series[].series
    for row in &mut series.depth_series {
        row.series.retain(|&(t, _)| t >= lo_time_s);
    }

    // vwap
    series.vwap.retain(|&(t, _)| t >= lo_time_s);

    // heatmap
    series.heatmap.retain(|c| c.time_s >= lo_time_s);

    // volume_bubbles
    series.volume_bubbles.retain(|c| c.time_s >= lo_time_s);

    // cumulative_delta: эвикт + сдвиг + инкремент cvd_session_base.
    // До эвикции: value[t] = old_base + running(retained up to t), где running стартует с 0
    // (на удержанных бакетах) — `Reducer::finish` прибавляет cvd_session_base ПОСЛЕ running
    // (см. конец `finish()`).
    // Эвиктируем записи < lo_time_s. Сумма эвиктнутых delta вычисляется через
    // diff: delta[t] = value[t] - prev; prev = value[t_предыдущего]; first prev = old_base.
    // После эвикции значения сдвигаются на +evicted_sum и cvd_session_base += evicted_sum,
    // чтобы на КАЖДОМ удержанном бакете новый running(retained, начиная с prev=new_base)
    // восстановил сумму сдвинутую обратно.
    let mut evicted_sum: i64 = 0;
    let mut prev: i64 = series.cvd_session_base;
    series.cumulative_delta.retain_mut(|&mut (t, ref mut v)| {
        if t < lo_time_s {
            let delta = *v - prev;
            evicted_sum += delta;
            prev = *v;
            false
        } else {
            prev = *v;
            true
        }
    });
    if evicted_sum != 0 {
        for (_, v) in series.cumulative_delta.iter_mut() {
            *v += evicted_sum;
        }
        series.cvd_session_base += evicted_sum;
    }
}

/// M-37: CVD running-base merge. После `evict_series_bundle_under_window` existing имеет
/// `cvd_session_base_e` (включая инкремент от только что эвиктнутых бакетов) и значения,
/// сдвинутые на тот же инкремент. Incoming имеет `cvd_session_base_i` и свои значения
/// (`cvd_session_base_i` + running(retained_i)).
///
/// **Алгоритм:**
/// 1. Дельты existing извлекаем с «previous» = cvd_session_base_e (тогда первая дельта
///    корректна, а не «весь prefix»).
/// 2. Дельты incoming извлекаем с «previous» = cvd_session_base_i.
/// 3. Суммируем дельты по time_s (один time_s — одна дельта с каждой стороны; union).
/// 4. Ре-дериваем running с new_base = cvd_session_base_e + cvd_session_base_i.
/// 5. `series.cvd_session_base = new_base` для последующих merge.
fn merge_cvd_running(series: &mut SeriesBundle, incoming: &SeriesBundle) {
    let base_e = series.cvd_session_base;
    let base_i = incoming.cvd_session_base;

    let mut deltas: BTreeMap<i64, i64> = BTreeMap::new();
    // existing
    let mut prev = base_e;
    for &(t, v) in &series.cumulative_delta {
        let d = v - prev;
        prev = v;
        deltas.insert(t, d);
    }
    // incoming
    let mut prev = base_i;
    for &(t, v) in &incoming.cumulative_delta {
        let d = v - prev;
        prev = v;
        *deltas.entry(t).or_insert(0) += d;
    }

    let new_base = base_e + base_i;
    let mut running = new_base;
    series.cumulative_delta = deltas
        .into_iter()
        .map(|(t, d)| {
            running += d;
            (t, running)
        })
        .collect();
    series.cvd_session_base = new_base;
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
    let (series, cursor, _, _at_ms) =
        reduce_event_stream(stream, sel, Cursor::START, at, usize::MAX)?;
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
/// (`f[i].to == f[i+1].from`); последний `.to == возвращённый курсор`.
///
/// M-37: возвращаемый `Frame` несёт `at_ms` (= «at» последнего event в кадре) для корректного
/// fold'а `Snapshot(C) + frames_since(C..) == snapshot(LATEST)` под окном (`apply()` использует
/// `at_ms` для эвикции existing под финальное окно — иначе existing держит бакеты `[C−W, C]`,
/// а финальное окно — `[LATEST−W, LATEST]`).
pub fn frames_since(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    sel: &Selector,
    after: Cursor,
    max_events: usize,
) -> io::Result<(Vec<Frame>, Cursor)> {
    let stream = journal::stream(dir, filter)?;
    let (delta, cursor, consumed, at_ms) =
        reduce_event_stream(stream, sel, after, Cursor::LATEST, max_events)?;
    if consumed == 0 {
        return Ok((Vec::new(), after));
    }
    Ok((vec![Frame::versioned(after, cursor, delta, at_ms)], cursor))
}

/// Детерминированный replay окна `(from .. to]` тем же редьюсером, что live (VB-I-2/GW-I-3).
///
/// M-37: возвращаемый `Frame` несёт `at_ms` для финального окна (см. `frames_since`).
pub fn replay(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    sel: &Selector,
    from: Cursor,
    to: Cursor,
) -> io::Result<Vec<Frame>> {
    let stream = journal::stream(dir, filter)?;
    let (delta, cursor, consumed, at_ms) = reduce_event_stream(stream, sel, from, to, usize::MAX)?;
    if consumed == 0 {
        return Ok(Vec::new());
    }
    Ok(vec![Frame::versioned(from, cursor, delta, at_ms)])
}
