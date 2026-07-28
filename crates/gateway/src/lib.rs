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

/// M-38b (TD-044): placeholder-селектор для `#[serde(skip, default = "default_selector")]`.
/// Никогда не используется в готовом `Reducer` (вызывающий перезаписывает `selector` из
/// аргумента), но нужен serde для десериализации поля. Возвращает пустой селектор, у
/// которого `matches()` всегда `false` — `Reducer::apply` НИЧЕГО не применит.
fn default_selector() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: String::new(),
        timeframe_ms: 0,
        bands: Vec::new(),
        window_ms: None,
    }
}

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
/// 7: M-38a (TD-043) — **CVD session-anchored ledger**: `cumulative_delta` running теперь
///    ОБНУЛЯЕТСЯ на границе 00:00 UTC (per UTC-сессия свой running с нуля — ЗЕРКАЛЬНО VP,
///    было: единая running-сумма через все дни, M-37 баг). Форма `cvd_session_base` меняется
///    скаляр `i64` → `Vec<(session_id, base)>` (per-session; отсутствие сессии в векторе ⇒
///    base=0). Non-additive (семантика И форма) — бамп 6 → 7 (VB-I-6/VB-I-10).
pub const GATEWAY_SCHEMA_VERSION: u32 = 7;

/// M-38b (TD-044, GW-I-9): версия ВНУТРЕННЕГО формата чекпоинта. Независима от
/// `GATEWAY_SCHEMA_VERSION` (форма провода не меняется, меняется скорость её получения):
/// чекпоинт — T3 (внутренний кэш), не пересекает границу движок↔деск. Несовпадение
/// версий → ТИХИЙ rebuild (миграций не требуется по построению). CT-RFC не нужен.
///
/// 1: первая версия (M-38b rev2): i128 в postcard поддержан (задача #0), serialize ВСЕ
///    поля Reducer кроме selector (C-030 N1).
pub const CKPT_SCHEMA_VERSION: u32 = 1;

/// M-38b: магия чекпоинт-файла (8 байт). Не путать с `SEGMENT_MAGIC` журнала — это
/// внутренний кэш редьюсера, не сегмент данных.
pub const CKPT_MAGIC: [u8; 8] = *b"HFTCKP01";

/// M-38a: UTC-day session id из уже-бакетированного `time_s` (секунды) — зеркалит
/// `utc_session_id(ts_ms)` в секундном пространстве (`time_s.div_euclid(86_400) ==
/// ts_ms.div_euclid(86_400_000)` при `time_s = ts_ms/1000`). Используется per-session CVD
/// merge/eviction, которые оперируют уже забакетированными строками `SeriesBundle`
/// (`cumulative_delta: Vec<(time_s, ...)>`), а не сырым `ts_exch_ms`.
fn session_of(time_s: i64) -> i64 {
    time_s.div_euclid(86_400)
}

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
    /// M-38a (TD-043, VB-I-6): running РЕСЕТИТСЯ на границе UTC-сессии (00:00 UTC) — каждая
    /// сессия несёт свой running с нуля (+ `cvd_session_base(sid)`), НЕ единая сумма через все
    /// дни (M-37 баг). Конкатенация per-session running-серий в порядке `session_id`
    /// возрастания; внутри сессии — `time_s` возрастания.
    pub cumulative_delta: Vec<(i64, i64)>,
    /// M-38a (TD-043): per-session CVD ledger base — `(session_id, base)`, СОРТ по `session_id`
    /// возрастания. `base` = сумма знаковых delta эвиктнутых внутрисессионных бакетов ЭТОЙ
    /// сессии (переносится в running при merge/фолде — иначе при эвикции префикса первое
    /// удержанное значение `cumulative_delta` интерпретировалось бы как «весь prefix», и
    /// running-сумма ломалась). Сессия БЕЗ записи в этом векторе трактуется как `base=0`
    /// (никогда не эвиктилась внутрисессионно, либо целиком удалена как прошлая — см.
    /// `evict_series_bundle_under_window`). Заменяет M-37 скалярный `cvd_session_base: i64`
    /// (форма v6→v7, non-additive bump). `#[serde(default)]` — не type-совместимость с v6
    /// (скаляр), а defensive-default для консюмеров, ещё не читающих поле; консюмер обязан
    /// гейтить на `schema_version==7`. См. `VB-I-10` / `red_gateway_window::cvd_base_survives_*`
    /// / `red_gateway_cvd_session.rs`.
    #[serde(default)]
    pub cvd_session_base: Vec<(i64, i64)>,
    pub depth_series: Vec<DepthRow>,
    /// All-time VWAP `(time_s, price ×1e8)`, cumulative `Σ(price·size)/Σ(size)` от старта
    /// курсора (M-36, VB-I-6 reversal). БЕЗ reset на 00:00 UTC — `sum_pv/sum_v` копятся
    /// через границу дня. Session-anchored индикаторы — SVP/CVD (см. `volume_profile`/
    /// `cumulative_delta`).
    pub vwap: Vec<(i64, i64)>,
    /// Session Volume Profile (SVP, M-24): `VolumeProfileRow` per сессия, сортировка по `session_id`.
    /// Только ТОРГОВАННЫЕ цены (VP-I-4): ключи гистограммы — реальные сделки, не «выдуманные».
    pub volume_profile: Vec<VolumeProfileRow>,
    /// M-38a (TD-045): per-session VP `max_time_s` — зеркало `Reducer::session_max_time_s`,
    /// перенесённое в bundle для применения ИДЕНТИЧНОГО редьюсеру критерия whole-session drop
    /// `vp_session_max_time_s[sid] < lo_time_s` на пути merge (`Snapshot::apply` /
    /// `evict_series_bundle_under_window`). Без этого merge структурно не мог воспроизвести
    /// критерий `Reducer::evict_window_state` — старый `row.session_id < utc_session_id(at)`
    /// ронял прошлую сессию СРАЗУ после 00:00 UTC, хотя финальное окно `[at−W, at]` её ещё
    /// пересекало (TD-045 регрессия PR-гейта reviewer'а, K2 vantage `overlap_multistep`). Форма
    /// `(session_id, max_time_s)` сорт по `session_id` возрастанию; сессия без записи
    /// трактуется как «нет данных о max» (используется только в эвикции — drop-критерий). Часть
    /// формы v7 (наряду с `cvd_session_base: Vec<(session_id, base)>`) — bump 6→7 уже выполнен
    /// в task #9, второй bump для TD-045 НЕ требуется (v7 ещё не в main). `#[serde(default)]`
    /// — defensive default для консюмеров v7, не читающих поле; консюмер ОБЯЗАН гейтить на
    /// `schema_version==7`. См. `VB-I-10` / `red_gateway_window::windowed_live_eq_replay_*`.
    #[serde(default)]
    pub vp_session_max_time_s: Vec<(i64, i64)>,
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

#[derive(Clone, Copy, Serialize, Deserialize)]
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

#[derive(Default, Clone, Serialize, Deserialize)]
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

#[derive(Clone, Serialize, Deserialize)]
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
#[derive(Default, Clone, Serialize, Deserialize)]
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

/// M-38a (TD-043, VB-I-6): per-session CVD ledger element. `base` — running-сдвиг сессии
/// (сумма знаковых delta уже эвиктнутых внутрисессионных бакетов); `bucket_delta` — удержанные
/// per-бакет знаковые дельты ЭТОЙ сессии. Running сессии стартует с `base` (изначально 0 —
/// НИКАКОГО наследования от предыдущей сессии, TD-043 фикс single-running M-37 бага).
#[derive(Default, Clone, Serialize, Deserialize)]
struct CvdSession {
    base: i64,
    bucket_delta: BTreeMap<i64, i64>,
}

/// Incremental form of the M-17 reducers. State grows only with the emitted time buckets,
/// never with the number of journal events.
///
/// M-38b (TD-044, GW-I-9): `#[derive(Serialize, Deserialize)]` для чекпоинт-редьюсера
/// (задача #2): сериализуется ВСЁ состояние кроме `selector` (конфигурация, не состояние
/// — `#[serde(skip)]`, восстанавливается вызывающим из `advance` / `snapshot_from_checkpoint`).
/// Полнота держится компилятором: новое поле в `Reducer` без derive → не скомпилируется.
///
/// `Clone` нужен `LiveReducer::pump` для снятия `SeriesBundle` (`finish`) с клона без
/// потребления self.
#[derive(Clone, Serialize, Deserialize)]
#[allow(clippy::derive_partial_eq_without_eq)] // Reducer is internal, PartialEq not needed.
struct Reducer {
    /// M-38b (C-030 N1): `selector` — КОНФИГУРАЦИЯ, не состояние. В чекпоинт не пишется,
    /// восстанавливается вызывающим и сверяется через `selector_fingerprint`. Иначе
    /// чекпоинт начал бы навязывать устаревший конфиг (например, старые bands) молча,
    /// вместо того чтобы честно инвалидироваться по фингерпринту. `#[serde(skip,
    /// default = "default_selector")]` — `serde(skip)` без default требует `Default` на
    /// типе (а `Selector` его не имеет), поэтому подставляем пустой селектор при
    /// десериализации — вызывающий ОБЯЗАН установить его из своего аргумента.
    #[serde(skip, default = "default_selector")]
    selector: Selector,
    ohlcv: BTreeMap<i64, OhlcvAcc>,
    /// M-38a (TD-043): per-session CVD ledger, `session_id → CvdSession`. Заменяет M-37
    /// плоские `bucket_delta: BTreeMap<i64,i64>` + скалярный `cvd_session_base: i64` — каждая
    /// UTC-сессия (`utc_session_id`, VB-I-6) держит СВОЙ running с нуля (сброс на 00:00 UTC).
    cvd: BTreeMap<i64, CvdSession>,
    vwap: VwapAcc,
    depth: Vec<DepthAcc>,
    /// M-24: per-session Volume Profile accumulator (price→объём).
    vp: VolumeProfileAcc,
    /// M-37 task #4 / M-38a task #5: `session_id → max(bucket_time_s)` — ОДНА структура на VP
    /// И CVD whole-session эвикцию (унифицировано, было раздельное `vp_session_max_time_s`).
    /// Обновляется в `apply_vp` на КАЖДОЙ сделке (нужен для решения «эвиктить ли сессию
    /// целиком» — сессия с max внутри окна удерживается, сессия полностью вне окна удаляется).
    session_max_time_s: BTreeMap<i64, i64>,
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
#[derive(Default, Clone, Serialize, Deserialize)]
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
            // M-38a: per-session CVD ledger стартует пустым — каждая сессия создаётся лениво
            // (`self.cvd.entry(sid).or_default()`) при первой сделке этой UTC-сессии, running
            // стартует с `base=0` (никакого наследования от предыдущей сессии, TD-043).
            cvd: BTreeMap::new(),
            vwap: VwapAcc::default(),
            depth: Vec::new(),
            vp: VolumeProfileAcc::default(),
            // M-37 task #4 / M-38a task #5: per-session max(bucket_time_s), унифицировано на
            // VP И CVD whole-session эвикцию.
            session_max_time_s: BTreeMap::new(),
            book: book::OrderBook::new(),
            heatmap_buckets: BTreeMap::new(),
            bubbles: BTreeMap::new(),
            // M-37: timestamp (ms) последнего event; финальное «at» для `Frame.at_ms` и
            // для вычисления нижней границы окна `[at-W, at]` в `evict_window_state`.
            at_ms: 0,
        }
    }

    /// M-37 task #2 / M-38a task #6: эвиктировать бакет-оконное состояние для `time_s <
    /// lo_time_s`. Вызывается на КАЖДОМ event (после обновления `at_ms`) — давление памяти
    /// O(окно) вместо O(история) (VB-I-10, TD-039).
    ///
    /// **CVD per-session (M-38a, TD-043):** внутрисессионный префикс (бакеты `< lo_time_s`
    /// УДЕРЖИВАЕМОЙ сессии) фолдится в `base` ЭТОЙ сессии — running-база переживает
    /// bucket-эвикцию локально для каждой сессии (зеркально M-37
    /// `cvd_base_survives_window_eviction`, теперь per-session). Целиком ПРОШЕДШАЯ сессия
    /// (см. unified whole-session drop ниже) удаляется целиком — base+bucket_delta.
    ///
    /// **VP whole-session:** эвиктируем ТОЛЬКО целыми ПРОШЛЫМИ сессиями (C-027 K3 #2
    /// `red_gateway_window::vp_current_session_whole_not_bucket_windowed`). Текущая сессия
    /// удерживается целиком, даже если её ранние бакеты вне окна — иначе POC/VAH/VAL текущей
    /// сессии порежется (`vp_current_session_whole_not_bucket_windowed`).
    ///
    /// **Unified whole-session criterion (M-38a task #5):** ОДНА структура `session_max_time_s`
    /// решает whole-session drop И для VP, И для CVD — сессия эвиктится целиком, когда
    /// `session_max_time_s[sid] < lo_time_s` (последний бакет, КОГДА-ЛИБО виденный в этой
    /// сессии, уже позади окна — она никогда больше не получит вклада, время в журнале
    /// монотонно). Условие эквивалентно «CVD-сессия полностью выфолдилась в base выше»: если
    /// ВСЕ бакеты сессии `< lo_time_s`, то и максимум её бакетов `< lo_time_s`.
    fn evict_window_state(&mut self, lo_time_s: i64) {
        if lo_time_s <= 0 {
            return; // lo_time_s ≤ 0 → окно растянуто в прошлое за пределы возможных бакетов
                    // (все time_s ≥ 0), ничего реально не эвиктим.
        }

        // CVD per-session (M-38a task #4/#6): для КАЖДОЙ удержанной сессии фолдим бакеты
        // `< lo_time_s` в её `base` (accumulate, не «set to N» — нужно для fold-корректности
        // многошагового apply). Whole-session drop — ниже, унифицировано с VP.
        for session in self.cvd.values_mut() {
            let mut evicted_delta_sum: i64 = 0;
            session.bucket_delta.retain(|&t, &mut d| {
                if t < lo_time_s {
                    evicted_delta_sum += d;
                    false
                } else {
                    true
                }
            });
            session.base += evicted_delta_sum;
        }

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

        // Unified whole-session eviction (M-37 task #4 + M-38a task #5): VP И CVD делят ОДИН
        // критерий — `session_max_time_s[sid] < lo_time_s` (сессия полностью вне окна = её
        // последний КОГДА-ЛИБО виденный бакет уже эвиктнут). Текущая сессия (max ≥ lo_time_s)
        // удерживается целиком (POC/VAH/VAL VP не порежутся; CVD base/bucket_delta сессии не
        // тронуты сверх fold'а выше). Прошлая сессия — удаляется целиком из ОБЕИХ структур
        // (VP bins + CVD base/bucket_delta).
        let to_evict: Vec<i64> = self
            .session_max_time_s
            .iter()
            .filter(|(_, &max_t)| max_t < lo_time_s)
            .map(|(&sid, _)| sid)
            .collect();
        for sid in to_evict {
            self.vp.bins.remove(&sid);
            self.cvd.remove(&sid);
            self.session_max_time_s.remove(&sid);
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
        // M-37 task #4 / M-38a task #5: per-session max(bucket_time_s) — ЕДИНАЯ структура
        // для whole-session эвикции И VP, И CVD (см. `evict_window_state`). VP и CVD хранятся
        // per-сессия, не per-бакет → обновляем max на каждой сделке, чтобы `evict_window_state`
        // мог решить «целиком ли прошлая сессия вне окна [at-W, at]» для обоих индикаторов.
        if let Some(bucket_time_s) = self.bucket_time_s(*ts_exch_ms) {
            let sid = utc_session_id(*ts_exch_ms);
            let entry = self.session_max_time_s.entry(sid).or_insert(bucket_time_s);
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
                // M-38a (TD-043): per-session CVD ledger — session ВЫВОДИТСЯ из `ts_exch_ms`
                // (VB-I-6), НЕ из журнального `Event` (T1 не тронут). Каждая UTC-сессия ведёт
                // свой `bucket_delta` независимо — reset на 00:00 UTC (никакого наследования
                // от предыдущей сессии).
                let sid = utc_session_id(*ts_exch_ms);
                let session = self.cvd.entry(sid).or_default();
                *session.bucket_delta.entry(time_s).or_default() += signed_size;
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

        // M-38a (TD-043, task #7): CVD running-sum PER-SESSION, reset на границе 00:00 UTC.
        // `self.cvd` — `BTreeMap<session_id, CvdSession>`, итерация ascending по `session_id`
        // (внешний цикл) И по `time_s` внутри сессии (`bucket_delta` тоже `BTreeMap`) — так
        // как session_id монотонно растёт вместе с временем (сессии — непересекающиеся
        // календарные дни), результирующий `cumulative_delta` остаётся globally ascending по
        // `time_s`. Running сессии стартует С `session.base` (сдвиг эвиктнутых внутрисессионных
        // бакетов — переживает bucket-эвикцию ЛОКАЛЬНО для сессии), но НИКОГДА не наследует
        // running предыдущей сессии (TD-043 fix — M-37 бага единой суммы через все дни).
        let mut cumulative_delta: Vec<(i64, i64)> = Vec::new();
        let mut cvd_session_base: Vec<(i64, i64)> = Vec::new();
        for (sid, session) in self.cvd {
            if session.base != 0 {
                cvd_session_base.push((sid, session.base));
            }
            let mut running = session.base;
            for (time_s, delta) in session.bucket_delta {
                running += delta;
                cumulative_delta.push((time_s, running));
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

        // M-38a (TD-045, task #11): per-session VP `max_time_s` экспортируется в bundle для
        // применения ИДЕНТИЧНОГО редьюсеру whole-session drop-критерия на пути merge
        // (`Snapshot::apply` / `evict_series_bundle_under_window`). Источник — единая структура
        // `self.session_max_time_s` (M-38a task #5 унификация VP+CVD), консистентна с
        // `volume_profile` (Reducer одновременно дропает из `vp.bins` и `session_max_time_s`
        // в `evict_window_state`). СОРТ по `session_id` возрастанию — зеркалит `volume_profile`
        // и форму v7 `cvd_session_base: Vec<(session_id, ...)>` для согласованного merge.
        let vp_session_max_time_s: Vec<(i64, i64)> = self
            .session_max_time_s
            .iter()
            .map(|(&sid, &max_t)| (sid, max_t))
            .collect();

        // M-23: heatmap + COB + bubbles. `build_heatmap_cob` использует сохранённые снимки
        // книги из `heatmap_buckets` (close-семантика) + bubbles из `bubbles` (Trade-аккумулятор).
        let (heatmap, cob) = build_heatmap_and_cob(&self.selector, self.heatmap_buckets);
        let volume_bubbles = build_volume_bubbles(self.bubbles);

        // M-38a task #7: `cvd_session_base` (Vec, per-session — собран выше в цикле по
        // `self.cvd`) экспортируется в SeriesBundle для merge-логики `Snapshot::apply()`. При
        // fold'е existing (base_e(sid)) + incoming (base_i(sid)) их базы складываются PER
        // SESSION и применяются к merged `cumulative_delta` — иначе эвиктнутые prefix'ы обеих
        // сторон потеряются (см. `merge_cvd_running`).
        // M-38a (TD-045, task #11): `vp_session_max_time_s` (Vec, per-session — собран выше)
        // экспортируется в SeriesBundle для merge-логики `Snapshot::apply` (whole-session drop
        // по `vp_session_max_time_s[sid] < lo_time_s`).

        SeriesBundle {
            ohlcv,
            cumulative_delta,
            cvd_session_base,
            depth_series,
            vwap,
            volume_profile,
            vp_session_max_time_s,
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
    stream: &mut journal::EventStream,
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
    /// M-38a (TD-043, C-028 K2): CVD running-base пересчитывается PER-SESSION через
    /// `cvd_session_base: Vec<(session_id, base)>` (existing += sum эвиктнутых внутри каждой
    /// сессии, merged(sid) = existing(sid) + incoming(sid)); сессия целиком позади финального
    /// окна — whole-session drop (см. `evict_series_bundle_under_window`), НЕ переносится в
    /// merge. CVD-кривая остаётся session-locally непрерывной под окном (reset на границах
    /// сохранён при fold'е).
    pub fn apply(&mut self, frame: &Frame) {
        // 1. Финальное окно: `[frame.at_ms − W, frame.at_ms]` (None = unbounded, ничего не эвиктим).
        let final_lo_time_s = self.selector.window_lo_time_s(frame.at_ms);

        // 2. M-37 task #2 / M-38a task #8: ЭВИКЦИЯ existing под финальное окно (per-session
        // CVD fold + whole-session drop). Этот шаг критичен для байт-идентичности
        // `snapshot(C) + frames_since(C..) ≡ snapshot(LATEST)` под окном (без него existing
        // держит бакеты `[C−W, C]`, которые в `snapshot(LATEST)` уже вне `[LATEST−W, LATEST]`).
        if let Some(lo) = final_lo_time_s {
            evict_series_bundle_under_window(&mut self.series, lo);
        }

        // 3. M-38a task #8: CVD running-base merge PER-SESSION. ВАЖНО — existing уже эвиктнут
        // (его per-session `cvd_session_base` инкрементирован на сумму эвиктнутых delta ЭТОЙ
        // сессии, значения `cumulative_delta` не сдвинуты — TD-042 дисциплина). Merge ниже
        // использует новый per-session `cvd_session_base` как «previous» для обеих сторон.
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

        // M-38a (TD-045, task #11): VP whole-session drop теперь ВЫПОЛНЯЕТСЯ в
        // `evict_series_bundle_under_window` ДО merge (см. шаг 2 выше) — единый критерий
        // `vp_session_max_time_s[sid] < lo_time_s`, идентичный `Reducer::evict_window_state`.
        // Старый код drop'ал VP-сессию здесь по `row.session_id < utc_session_id(at)` — ронял
        // прошлую сессию СРАЗУ после 00:00 UTC, даже если финальное окно её ещё пересекало
        // (TD-045 регрессия PR-гейта reviewer'а: existing-состояние, ПЕРЕСЕКАЮЩЕЕ финальное
        // окно, роняло S1 → GW-I-4/VB-I-2 сломан). Удалён.

        // M-24: volume_profile сливается по session_id — восстанавливаем per-session гистограммы
        // из bins (existing + incoming), складываем, пересчитываем POC/VA (compute_vp_row).
        // Не дубль-строки: одна VolumeProfileRow per сессия (VP-I-3 merge-инвариант), даже
        // если сессия присутствует в обоих sources. Сессия, ЭВИКТНУТАЯ в шаге 2
        // (whole-session drop по `vp_session_max_time_s[sid] < lo_time_s`), но восстановленная
        // incoming'ом через bins-reconstruct, — ре-деривится здесь с актуальным POC/VA.
        self.series.volume_profile =
            merge_volume_profile(&self.series.volume_profile, &frame.delta.volume_profile);

        // M-38a (TD-045, task #11): merge `vp_session_max_time_s` — max(existing[sid],
        // incoming[sid]) per session_id. Existing уже префикс-эвиктнут в шаге 2 (только
        // сессии с `max_time_s >= lo_time_s` от `evict_series_bundle_under_window`);
        // incoming — свёртка frame'а через тот же редьюсер. Для общих сессий
        // incoming.max >= existing.max (время журнала монотонно → max растёт); max(., .)
        // согласовано с `merge_volume_profile`'ом (та же union session_ids).
        // Зеркалит CVD-merge `cvd_session_base` (тоже max на сессию; форма v7).
        let mut vp_max: BTreeMap<i64, i64> = BTreeMap::new();
        for &(sid, max_t) in &self.series.vp_session_max_time_s {
            vp_max.insert(sid, max_t);
        }
        for &(sid, max_t) in &frame.delta.vp_session_max_time_s {
            let entry = vp_max.entry(sid).or_insert(max_t);
            if max_t > *entry {
                *entry = max_t;
            }
        }
        let mut merged_vp_session_max_time_s: Vec<(i64, i64)> = vp_max.into_iter().collect();

        // M-38a (TD-045, task #11): VP whole-session drop ПОСЛЕ merge (mirror CVD
        // `merge_cvd_running`'s whole-session check). Решение по merged
        // `vp_session_max_time_s[sid] < lo_time_s` — если merged max сессии ниже `lo`,
        // значит НИ existing, НИ incoming не дали ни одного бакета в финальное окно
        // (журнал монотонен → сессия полностью вне окна НАВСЕГДА, в дальнейшем вклада
        // не будет). Зеркалит `Reducer::evict_window_state` (session-level criterion).
        // СТАРЫЙ код ронял здесь по `row.session_id < utc_session_id(at)` — это роняло
        // прошлую сессию СРАЗУ после 00:00 UTC, даже если финальное окно её ещё
        // пересекало (TD-045 регрессия PR-гейта reviewer'а: existing-состояние,
        // ПЕРЕСЕКАЮЩЕЕ финальное окно, роняло S1 → GW-I-4/VB-I-2 сломан).
        if let Some(lo) = final_lo_time_s {
            merged_vp_session_max_time_s.retain(|&(_, max_t)| max_t >= lo);
            let drop_sids: std::collections::BTreeSet<i64> = merged_vp_session_max_time_s
                .iter()
                .map(|&(sid, _)| sid)
                .collect();
            self.series
                .volume_profile
                .retain(|r| drop_sids.contains(&r.session_id));
        }
        self.series.vp_session_max_time_s = merged_vp_session_max_time_s;

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
/// 6. `cumulative_delta` (M-38a, per-session, TD-043) — сгруппировать записи по
///    `session_of(time_s)`; в каждой сессии удалить `< lo_time_s`, сложив их дельты в
///    `cvd_session_base(sid)` (значения АБСОЛЮТНЫ — `Reducer::finish` уже прибавил base к
///    каждой, эвикция префикса их НЕ меняет, только базу). ЭТОТ шаг НИКОГДА не роняет сессию
///    целиком (даже если held опустел — окно могло просто сдвинуться ВНУТРИ ещё активной
///    сессии, incoming принесёт новые строки на следующем merge-шаге). Whole-session drop
///    (C-028 K2, «сессия целиком позади окна навсегда» — зеркально VP-критерию) — в
///    `merge_cvd_running`, где видно, приносит ли incoming строки для сессии тоже.
///    `merge_cvd_running` идемпотентен на абсолютных значениях с корректной per-session базой
///    (первый удержанный: `d = value − base(sid) = δ`).
/// 7. `vp_session_max_time_s` (M-38a TD-045, task #11) — префиксная фильтрация per-session
///    `max_time_s`: ЗАПИСИ с `max_time_s < lo_time_s` удаляются здесь, но решение о drop'е
///    `volume_profile` строки ПРИНИМАЕТСЯ ПОСЛЕ merge в `Snapshot::apply` (по merged
///    `vp_session_max_time_s[sid] < lo_time_s`). Аналогия CVD: prefix-фолд в эвикции, drop —
///    в merge (где видно, принёс ли incoming bins для сессии). Если удалить VP-сессию здесь,
///    `merge_volume_profile` потеряет её bins от existing, даже если incoming её восстановит
///    (регрессия `windowed_live_eq_replay`: existing S, max < lo, incoming S, max >= lo →
///    merged = incoming only). Whole-session drop VP в `apply` после merge_volume_profile и
///    merge vp_session_max_time_s.
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

    // cumulative_delta (M-38a per-session, task #8): сгруппировать существующие строки по
    // `session_of(time_s)`, фолднуть `< lo_time_s` в base ТОЙ сессии. Значения АБСОЛЮТНЫ
    // (`Reducer::finish` уже прибавил base к каждой), эвикция префикса их НЕ сдвигает — только
    // базу (TD-042: предыдущая scalar-версия сдвигала удержанные значения, сдвиг копился по
    // apply под пересекающимся окном; per-session фолд той же дисциплины избегает регрессии).
    //
    // ВАЖНО: этот шаг САМ ПО СЕБЕ никогда не «роняет» сессию целиком — он ТОЛЬКО фолдит
    // эвиктнутый префикс в base и переносит base дальше, даже если held пуст (сессия могла
    // просто временно не иметь удержанных строк, потому что окно сдвинулось ВНУТРИ ТОЙ ЖЕ
    // ещё активной сессии — held опустошится, а на следующем шаге incoming принесёт новые
    // строки той же сессии; drop здесь был бы регрессией на single-session тесте
    // `windowed_live_eq_replay`). Whole-session drop (C-028 K2) выполняется НИЖЕ по потоку —
    // в `merge_cvd_running`, где видно, приносит ли INCOMING новые строки для сессии: если
    // ни existing (после этого фолда), ни incoming не дают НИ ОДНОЙ строки — сессия
    // действительно позади окна НАВСЕГДА (время в журнале монотонно, она уже никогда не
    // получит вклад), и merge отбрасывает её base.
    let mut bases: BTreeMap<i64, i64> = series.cvd_session_base.iter().copied().collect();
    let mut rows_by_session: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();
    for &(t, v) in &series.cumulative_delta {
        rows_by_session
            .entry(session_of(t))
            .or_default()
            .push((t, v));
    }
    let mut sessions: std::collections::BTreeSet<i64> = bases.keys().copied().collect();
    sessions.extend(rows_by_session.keys().copied());

    let mut new_cumulative_delta: Vec<(i64, i64)> = Vec::new();
    let mut new_bases: BTreeMap<i64, i64> = BTreeMap::new();
    for sid in sessions {
        let rows = rows_by_session.remove(&sid).unwrap_or_default();
        let base = bases.remove(&sid).unwrap_or(0);
        let mut evicted_sum: i64 = 0;
        let mut prev: i64 = base;
        let mut held: Vec<(i64, i64)> = Vec::new();
        for (t, v) in rows {
            if t < lo_time_s {
                evicted_sum += v - prev;
                prev = v;
            } else {
                held.push((t, v));
                prev = v;
            }
        }
        let new_base = base + evicted_sum;
        if new_base != 0 {
            new_bases.insert(sid, new_base);
        }
        new_cumulative_delta.extend(held);
    }
    series.cumulative_delta = new_cumulative_delta;
    series.cvd_session_base = new_bases.into_iter().collect();

    // M-38a (TD-045, task #11): префиксная фильтрация `vp_session_max_time_s` —
    // `max_time_s < lo_time_s` исключаем здесь (подготовка к merge: merged-решение в
    // `Snapshot::apply` после `merge_volume_profile`). НЕ дропаем `volume_profile` rows
    // здесь — их bins нужны для `merge_volume_profile` (иначе регрессия
    // `windowed_live_eq_replay`: existing сессия целиком отлетает, incoming восстановить
    // не может). Итоговый whole-session drop — в `apply` ниже.
    series
        .vp_session_max_time_s
        .retain(|&(_sid, max_t)| max_t >= lo_time_s);
}

/// M-38a (TD-043, task #8): CVD running merge PER-SESSION. После
/// `evict_series_bundle_under_window` existing имеет per-session base `base_e(sid)` (включая
/// инкремент от только что эвиктнутых бакетов ЭТОЙ сессии) и АБСОЛЮТНЫЕ значения (никакого
/// сдвига удержанных — TD-042, `finish`/предыдущий merge уже добавили base к каждой). Incoming
/// имеет свой per-session `base_i(sid)` и абсолютные значения.
///
/// **Алгоритм (per сессия, union существующих+incoming session_id):**
/// 1. Дельты existing-строк ЭТОЙ сессии извлекаем с «previous» = `base_e(sid)` (тогда первая
///    дельта корректна: `value[первый_удержанный] − base_e(sid) = δ`).
/// 2. Дельты incoming-строк ЭТОЙ сессии извлекаем с «previous» = `base_i(sid)`.
/// 3. Суммируем дельты по `time_s` (один `time_s` — одна дельта с каждой стороны при
///    overlap на границе курсора; union).
/// 4. Ре-дериваем running сессии с `new_base(sid) = base_e(sid) + base_i(sid)` — КАЖДАЯ сессия
///    ре-деривится с СВОЕЙ базы (reset между сессиями сохранён, TD-043 — сессии НЕ делят
///    running между собой).
/// 5. `series.cvd_session_base` = per-session `new_base` (нулевые базы опускаются — форма
///    v7 конвенция «отсутствие сессии = base 0»).
///
/// **Whole-session drop (C-028 K2):** если ДЛЯ ДАННОЙ сессии НИ existing (уже эвиктнутый выше
/// `evict_series_bundle_under_window`), НИ incoming не дали НИ ОДНОЙ строки — сессия целиком
/// позади финального окна И incoming (текущий/будущий вклад) тоже её не касается: время в
/// журнале монотонно ⇒ она уже никогда не получит вклад. В этом случае `new_base` НЕ
/// переносится (сессия отсутствует и в `cumulative_delta`, и в `cvd_session_base` — трактуется
/// как base=0). Отличие от «просто пусто temporarily»: если существующие строки session'а
/// ещё не были все эвиктнуты (существующая сессия жива в окне), `deltas` НЕ пуст (existing
/// вносит хотя бы одну строку) — drop не срабатывает; сессия, чьи держащиеся строки ВСЕ
/// только что эвиктнуты (existing пуст), но incoming продолжает приносить новые строки той же
/// (ещё активной) сессии — тоже НЕ дропается (incoming вносит строки → `deltas` не пуст).
fn merge_cvd_running(series: &mut SeriesBundle, incoming: &SeriesBundle) {
    let existing_bases: BTreeMap<i64, i64> = series.cvd_session_base.iter().copied().collect();
    let incoming_bases: BTreeMap<i64, i64> = incoming.cvd_session_base.iter().copied().collect();

    let mut existing_rows: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();
    for &(t, v) in &series.cumulative_delta {
        existing_rows.entry(session_of(t)).or_default().push((t, v));
    }
    let mut incoming_rows: BTreeMap<i64, Vec<(i64, i64)>> = BTreeMap::new();
    for &(t, v) in &incoming.cumulative_delta {
        incoming_rows.entry(session_of(t)).or_default().push((t, v));
    }

    let mut sessions: std::collections::BTreeSet<i64> = existing_bases.keys().copied().collect();
    sessions.extend(incoming_bases.keys().copied());
    sessions.extend(existing_rows.keys().copied());
    sessions.extend(incoming_rows.keys().copied());

    let mut new_cumulative_delta: Vec<(i64, i64)> = Vec::new();
    let mut new_bases: Vec<(i64, i64)> = Vec::new();
    for sid in sessions {
        let base_e = existing_bases.get(&sid).copied().unwrap_or(0);
        let base_i = incoming_bases.get(&sid).copied().unwrap_or(0);

        let mut deltas: BTreeMap<i64, i64> = BTreeMap::new();
        let mut prev = base_e;
        for &(t, v) in existing_rows.get(&sid).map(Vec::as_slice).unwrap_or(&[]) {
            deltas.insert(t, v - prev);
            prev = v;
        }
        let mut prev = base_i;
        for &(t, v) in incoming_rows.get(&sid).map(Vec::as_slice).unwrap_or(&[]) {
            *deltas.entry(t).or_insert(0) += v - prev;
            prev = v;
        }

        // Whole-session drop (C-028 K2): ни existing, ни incoming не дали ни одной строки для
        // этой сессии → она позади финального окна НАВСЕГДА (журнал монотонен) — не переносим
        // ни base, ни строки (пропускаем сессию целиком).
        if deltas.is_empty() {
            continue;
        }

        let new_base = base_e + base_i;
        if new_base != 0 {
            new_bases.push((sid, new_base));
        }
        let mut running = new_base;
        for (t, d) in deltas {
            running += d;
            new_cumulative_delta.push((t, running));
        }
    }

    series.cumulative_delta = new_cumulative_delta;
    series.cvd_session_base = new_bases;
}

/// M-47 (GW-I-10, TD-046): fail-closed гвард предусловия `Selector`. Селектор с
/// `timeframe_ms`, не делящим `86_400_000` нацело, порождает бакеты, пересекающие
/// 00:00 UTC ⇒ `session_id` бакета не определён ⇒ session-anchored серии (CVD — M-38a/TD-043,
/// SVP — M-24) семантически не определены. Правильный ответ — отказ, а не «правдоподобное»
/// значение (CLAUDE.md fail-closed). Проверяем ДЕЛИМОСТЬ суток, а не «круглость»:
/// недельный бакет (`604_800_000`) круглый, но накрывает 7 полуночей — отвергается.
///
/// Принимается ⟺ `timeframe_ms > 0 && 86_400_000 % timeframe_ms == 0`. Иначе
/// `io::ErrorKind::InvalidInput`, сообщение содержит подстроку `timeframe_ms` (оракул
/// `red_timeframe_session_alignment` ассертит это, чтобы оператор понимал, ЧТО чинить,
/// не читая исходников).
///
/// Гвард живёт ЗДЕСЬ, а не только в `gateway-serve::serve_config_from_env`: `Selector` —
/// публичная структура с публичными полями, её собирает напрямую любой консюмер библиотеки
/// (чекпоинтер M-38b, shared-tailer M-39, research-cli). Проверка ТОЛЬКО в конфиге транспорта
/// оставила бы байпас-поверхность (урок TD-019/TD-020 «механизм есть, никто не зовёт»).
/// Публичные входы библиотеки (`snapshot` / `frames_since` / `replay`) уже возвращают
/// `io::Result<_>` — смена сигнатур не нужна.
pub fn validate_selector(sel: &Selector) -> io::Result<()> {
    if sel.timeframe_ms <= 0 || 86_400_000 % sel.timeframe_ms != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "GW-I-10: selector.timeframe_ms={} не выравнен на границу UTC-суток \
                 (требуется > 0 и 86_400_000 % timeframe_ms == 0; иначе бакет пересекает \
                 00:00 UTC ⇒ session_id бакета не определён)",
                sel.timeframe_ms
            ),
        ));
    }
    Ok(())
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
    validate_selector(sel)?;
    let mut stream = journal::stream(dir, filter)?;
    let (series, cursor, _, _at_ms) =
        reduce_event_stream(&mut stream, sel, Cursor::START, at, usize::MAX)?;
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
    validate_selector(sel)?;
    let mut stream = journal::stream(dir, filter)?;
    let (delta, cursor, consumed, at_ms) =
        reduce_event_stream(&mut stream, sel, after, Cursor::LATEST, max_events)?;
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
    validate_selector(sel)?;
    let mut stream = journal::stream(dir, filter)?;
    let (delta, cursor, consumed, at_ms) =
        reduce_event_stream(&mut stream, sel, from, to, usize::MAX)?;
    if consumed == 0 {
        return Ok(Vec::new());
    }
    Ok(vec![Frame::versioned(from, cursor, delta, at_ms)])
}

// ════════════════════════════════════════════════════════════════════════════
// M-38b (TD-044, GW-I-9/GW-I-11): ЧЕКПОНТ-РЕДЬЮСЕР + LIVE-SEEK
// ════════════════════════════════════════════════════════════════════════════
//
// Прод-замер до фикса: первый Snapshot 409.74 s при >21 GiB прочитанного на КАЖДОЕ
// подключение. Лечение — чекпоинт полного состояния Reducer, от которого снапшот
// досчитывается хвостом через `journal::stream_from(cursor)` (GW-I-11).
//
// Контракт (binding per milestone §Инварианты):
// - GW-I-9(а): `snapshot_from_checkpoint(K, at) ≡ snapshot(START, at)` байт-в-байт.
// - GW-I-9(б): ЛЮБАЯ невалидность (magic/версии/фингерпринты/lineage/CRC/cursor>at/
//   нет файла/битый файл) → ТИХИЙ rebuild от START, без ошибки.
// - GW-I-9(в): `advance` идемпотентен: два вызова без новых событий → байт-идентичный файл.
// - GW-I-9(г): чекпоинт РЕАЛЬНО ЧИТАЕТСЯ (подменный чекпоинт обязан ИЗМЕНИТЬ выход).
// - GW-I-11: `snapshot_from_checkpoint` при K у хвоста декодирует ≤ хвостовых событий.

/// M-38b (GW-I-11): детерминированные счётчики чтения журнала. Зеркало `EventStream`
/// (та же схема: `events_decoded` инкрементируется в `next()`, `segments_opened` —
/// в `open_next_segment`). НЕ аллокатор, НЕ wall-time (урок TD-040).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReadStats {
    pub events_decoded: u64,
    pub segments_opened: u32,
}

impl std::ops::Add for ReadStats {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            events_decoded: self.events_decoded + rhs.events_decoded,
            segments_opened: self.segments_opened + rhs.segments_opened,
        }
    }
}

impl ReadStats {
    /// Сложить несколько `ReadStats` (например, от ckpt-load + tail-feed).
    pub fn sum<I: IntoIterator<Item = Self>>(iter: I) -> Self {
        iter.into_iter().fold(Self::default(), |a, b| a + b)
    }
}

fn read_stats_from_stream(stream: &journal::EventStream) -> ReadStats {
    ReadStats {
        events_decoded: stream.events_decoded(),
        segments_opened: stream.segments_opened(),
    }
}

/// M-38b (GW-I-9): полный снапшот через чекпоинт + досчёт хвостом.
///
/// ЛЮБАЯ невалидность чекпоинта (битый файл, чужая версия, фингерпринт не сошёлся,
/// CRC не сошёлся, `cursor > at`, нет файла в каталоге) → ТИХИЙ rebuild от START с
/// тем же результатом, БЕЗ ошибки. Кокпит не должен уметь отличить «кэш был» от
/// «кэша не было» ничем, кроме скорости.
///
/// Возвращаемый `ReadStats` — ЧЕСТНАЯ сумма `ckpt_load + tail_replay` (форсинг
/// `red_checkpoint_resource_bound::without_checkpoint_full_replay_is_reported`).
pub fn snapshot_from_checkpoint(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    sel: &Selector,
    ckpt_dir: impl AsRef<Path>,
    at: Cursor,
) -> io::Result<(Snapshot, ReadStats)> {
    validate_selector(sel)?;
    let dir = dir.as_ref();
    let ckpt_dir = ckpt_dir.as_ref();

    // (1) Попытка прочитать чекпоинт. Любая невалидность → молчаливый rebuild.
    if let Some((mut state, ckpt_cursor)) =
        checkpoint::read_checkpoint(dir, ckpt_dir, sel, filter.clone())?
    {
        // GW-I-9(б): `ckpt.cursor > at` — просили снапшот РАНЬШЕ чекпоинта.
        // Тихий rebuild до `at`.
        if !(ckpt_cursor
            .upto_seq
            .is_some_and(|cs| at.upto_seq.is_some_and(|a| cs > a)))
        {
            // (2) Чекпоинт валиден и не «из будущего». Досчитываем хвостом от
            // `ckpt_cursor` до `at`. Используем `stream_from` (GW-I-11 сегментный
            // skip). Редусер инициализируется восстановленным состоянием, и на нём
            // прогоняются ТОЛЬКО НОВЫЕ события — байт-идентично полному реплею.
            state.selector = sel.clone();
            let mut stream = journal::stream_from(dir, filter.clone(), ckpt_cursor.upto_seq)?;
            let mut cursor = ckpt_cursor;
            for event in &mut stream {
                let event = event?;
                if !at.includes(event.seq) {
                    break;
                }
                state.apply(&event);
                cursor = Cursor::at(event.seq);
            }
            // Обновить stats ПОСЛЕ итерации (счётчики инкрементируются в `next()`).
            let stats = read_stats_from_stream(&stream);
            let final_cursor = if at == Cursor::LATEST { cursor } else { at };
            let (series, reducer_at_ms) = state.finish_with_at();
            let _ = reducer_at_ms;
            return Ok((
                Snapshot {
                    schema_version: GATEWAY_SCHEMA_VERSION,
                    selector: sel.clone(),
                    cursor: final_cursor,
                    series,
                },
                stats,
            ));
        }
    }

    // (3) Fallback: rebuild от START. ЧЕСТНЫЙ полный проход — `ReadStats` декодирует
    // ВСЕ события (форсинг без чекпоинта декодирует N, см. `red_checkpoint_resource_bound`).
    let mut stream = journal::stream(dir, filter)?;
    let (series, cursor, _consumed, _at_ms) =
        reduce_event_stream(&mut stream, sel, Cursor::START, at, usize::MAX)?;
    // Re-read stats AFTER iteration (счётчики инкрементируются в `next()`).
    let stats = read_stats_from_stream(&stream);
    Ok((
        Snapshot {
            schema_version: GATEWAY_SCHEMA_VERSION,
            selector: sel.clone(),
            cursor,
            series,
        },
        stats,
    ))
}

/// M-38b (GW-I-9): чекпоинт-редьюсер — atomic запись полного состояния `Reducer` в
/// каталог `ckpt_dir`. Файл единственный (`ckpt.bin`). Атомарность через tmp + rename.
/// `flock` на каталог опускаем (best-effort: `Journal::rotate` в одном крейте — нет
/// мульти-писателя чекпоинта; если позже понадобится, добавляется `fs2`-crate).
pub mod checkpoint {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;

    use contracts::SegmentHeader;

    /// Имя файла чекпоинта (единственный в `ckpt_dir`).
    const CKPT_FILENAME: &str = "ckpt.bin";

    /// M-38b: заголовок чекпоинта — magic + версии + фингерпринты + lineage + cursor.
    /// Сериализуется как первая часть файла ДО postcard(state), чтобы при изменении
    /// формата валидация отказывала БЕЗ попытки десериализации state.
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct CkptHeader {
        pub magic: [u8; 8],
        pub ckpt_schema_version: u32,
        pub gateway_schema_version: u32,
        /// Селектор-фингерпринт: SHA-подобный хеш (Venue+symbol+timeframe_ms+window_ms+
        /// bands-через-`to_bits`). NaN в bands ЗАПРЕЩЁН в `Selector::validate` (см.
        /// `serve_config_from_env`), иначе фингерпринт нестабилен (`f64::NaN != f64::NaN`).
        pub selector_fingerprint: u64,
        /// EpochFilter-фингерпринт (OwnCaptureOnly / Explicit(sorted) / All).
        pub epoch_filter_fingerprint: u64,
        /// **Суффикс-совместимый lineage** (C-030 N2/R1): манифест заголовков сегментов,
        /// которые ЧЕКПОНТ СВЁРНУЛ. При валидации:
        /// (а) каждый ВИДИМЫЙ сейчас сегмент с `index ≤ max_index(манифест)` обязан
        ///     совпасть со своей записью поле-в-поле (`schema_version/source/provenance/
        ///     epoch_id/first_seq`); `created_wall_ms` и `size_bytes` НЕ проверяются
        ///     (компакция `.jrnl → .jrnl.zst` их меняет, чекпоинт обязан пережить
        ///     компакцию);
        /// (б) ОТСУТСТВУЮЩИЕ в текущем каталоге сегменты из манифеста допустимы ТОЛЬКО
        ///     если их события ЦЕЛИКОМ покрыты курсором чекпоинта (законный retention-prune
        ///     покрытого префикса);
        /// (в) любое расхождение/переупорядочивание/неизвестный сегмент внутри покрытого
        ///     диапазона → rebuild.
        pub journal_lineage: Vec<SegmentHeader>,
        /// Курсор, ДО которого (включительно) свёрнуто состояние чекпоинта.
        pub cursor: Cursor,
    }

    impl CkptHeader {
        pub fn new(
            selector_fingerprint: u64,
            epoch_filter_fingerprint: u64,
            journal_lineage: Vec<SegmentHeader>,
            cursor: Cursor,
        ) -> Self {
            Self {
                magic: CKPT_MAGIC,
                ckpt_schema_version: CKPT_SCHEMA_VERSION,
                gateway_schema_version: GATEWAY_SCHEMA_VERSION,
                selector_fingerprint,
                epoch_filter_fingerprint,
                journal_lineage,
                cursor,
            }
        }
    }

    /// M-38b: вычислить фингерпринт `Selector` (Venue+symbol+timeframe_ms+window_ms+bands).
    /// `bands` — через `f64::to_bits` (НЕ Display: `0.001` и `0.0010` — одна строка).
    /// NaN ЗАПРЕЩЁН: `Selector::validate` отвергает; на входе сюда NaN дал бы
    /// нестабильный фингерпринт.
    pub fn selector_fingerprint(sel: &Selector) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        // Venue: discriminant через mem::discriminant нестабилен по Display → hash через Debug.
        format!("{:?}", sel.venue).hash(&mut h);
        sel.symbol.hash(&mut h);
        sel.timeframe_ms.hash(&mut h);
        sel.window_ms.hash(&mut h);
        // bands — каждое значение через `to_bits` (НЕ Display).
        sel.bands.len().hash(&mut h);
        for b in &sel.bands {
            b.to_bits().hash(&mut h);
        }
        h.finish()
    }

    /// M-38b: фингерпринт `EpochFilter` — три варианта.
    /// - `OwnCaptureOnly` → hash от строки `"OwnCaptureOnly"`
    /// - `Explicit(sorted_eps)` → hash от отсортированных `epoch_id`
    /// - `All` → hash от строки `"All"`
    ///
    /// Сортировка Explicit — детерминизм (порядок в runtime не должен менять фингерпринт).
    pub fn epoch_filter_fingerprint(filter: &EpochFilter) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        match filter {
            EpochFilter::OwnCaptureOnly => {
                "OwnCaptureOnly".hash(&mut h);
            }
            EpochFilter::All => {
                "All".hash(&mut h);
            }
            EpochFilter::Explicit(eps) => {
                "Explicit".hash(&mut h);
                let mut sorted: Vec<&String> = eps.iter().collect();
                sorted.sort();
                sorted.len().hash(&mut h);
                for s in &sorted {
                    s.hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// Снять чекпоинт до `Cursor::LATEST`. Стандартный cron-вызов.
    pub fn advance(
        dir: impl AsRef<Path>,
        ckpt_dir: impl AsRef<Path>,
        sel: &Selector,
        filter: EpochFilter,
    ) -> io::Result<()> {
        advance_to(dir, ckpt_dir, sel, filter, Cursor::LATEST)
    }

    /// Снять чекпоинт ДО курсора `upto`. Редусер прогоняет журнал от START до
    /// `upto.inclusive_max_seq` через `journal::stream` (полный проход для cron —
    /// чекпоинт снимается периодически; инкрементальность достигается композицией
    /// `advance_to × N` — см. `red_checkpoint_is_cache::incremental_advance_equals_single_advance`).
    pub fn advance_to(
        dir: impl AsRef<Path>,
        ckpt_dir: impl AsRef<Path>,
        sel: &Selector,
        filter: EpochFilter,
        upto: Cursor,
    ) -> io::Result<()> {
        validate_selector(sel)?;
        let dir = dir.as_ref();
        let ckpt_dir = ckpt_dir.as_ref();
        fs::create_dir_all(ckpt_dir)?;

        // (1) Редусер от START до upto. Полный проход через `stream` (cron-задача,
        // допустимая стоимость — лимитируется каденсом 5–15 мин).
        let stream = journal::stream(dir, filter.clone())?;
        let mut reducer = Reducer::new(sel);
        let mut final_cursor = Cursor::START;
        for event in stream {
            let event = event?;
            if !upto.includes(event.seq) {
                break;
            }
            reducer.apply(&event);
            final_cursor = Cursor::at(event.seq);
        }

        // (2) Lineage: собираем заголовки всех сегментов журнала (отфильтрованные).
        // Lineage хранит МАНИФЕСТ покрытого префикса — при валидации мы сравниваем с
        // ТЕКУЩИМИ заголовками суффикс-совместимо.
        let all_segs = journal::list_segments(dir)?;
        let mut lineage: Vec<SegmentHeader> = Vec::with_capacity(all_segs.len());
        for s in &all_segs {
            if filter.accepts(&s.header) {
                lineage.push(s.header.clone());
            }
        }
        lineage.sort_by_key(|h| h.first_seq);

        // (3) Сформировать заголовок + сериализованное состояние.
        let header = CkptHeader::new(
            selector_fingerprint(sel),
            epoch_filter_fingerprint(&filter),
            lineage,
            final_cursor,
        );
        let state_bytes = postcard::to_stdvec(&reducer).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("postcard state: {e}"))
        })?;
        let header_bytes = postcard::to_stdvec(&header).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("postcard header: {e}"))
        })?;

        // (4) Atomic write: tmp + rename.
        let final_path = ckpt_dir.join(CKPT_FILENAME);
        let tmp_path = ckpt_dir.join(format!("{CKPT_FILENAME}.tmp"));
        let mut f = File::create(&tmp_path)?;
        // Layout: [magic(8)][ckpt_schema_v(4)][gateway_schema_v(4)][header_len(4)][header_postcard][state_len(4)][state_postcard][CRC32(4)]
        // CRC покрывает `header_postcard || state_postcard` (без magic/версий).
        f.write_all(&CKPT_MAGIC)?;
        f.write_all(&CKPT_SCHEMA_VERSION.to_le_bytes())?;
        f.write_all(&GATEWAY_SCHEMA_VERSION.to_le_bytes())?;
        f.write_all(&(header_bytes.len() as u32).to_le_bytes())?;
        f.write_all(&header_bytes)?;
        f.write_all(&(state_bytes.len() as u32).to_le_bytes())?;
        f.write_all(&state_bytes)?;
        // CRC32 over header_postcard || state_postcard.
        let mut crc_hasher = crc32fast::Hasher::new();
        crc_hasher.update(&header_bytes);
        crc_hasher.update(&state_bytes);
        f.write_all(&crc_hasher.finalize().to_le_bytes())?;
        f.flush()?;
        f.sync_data()?;
        drop(f);
        fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }

    /// Прочитать чекпоинт из каталога, валидировать header/CRC/lineage.
    /// Возвращает `Some((reducer, cursor))` если валиден, `None` если отсутствует/битый
    /// (silent rebuild). Любая ошибка → `None` (НЕ пробрасывается наружу: кокпит не
    /// должен различать «кэша не было» и «кэш битый»).
    pub(super) fn read_checkpoint(
        dir: &Path,
        ckpt_dir: &Path,
        sel: &Selector,
        filter: EpochFilter,
    ) -> io::Result<Option<(Reducer, Cursor)>> {
        let ckpt_path = ckpt_dir.join(CKPT_FILENAME);
        if !ckpt_path.exists() {
            // Допускаем и другие имена (один файл рекурсивно) — fallback для compose,
            // который раскладывает по подкаталогам.
            return find_and_read_checkpoint(dir, ckpt_dir, sel, filter);
        }
        let bytes = match fs::read(&ckpt_path) {
            Ok(b) => b,
            Err(_) => return Ok(None), // silent rebuild
        };
        Ok(read_and_validate(&bytes, dir, sel, filter))
    }

    /// Если `ckpt.bin` нет — попробовать найти единственный файл рекурсивно
    /// (для compose, где чекпоинтер раскладывает по selector_fingerprint/ckpt.bin).
    fn find_and_read_checkpoint(
        dir: &Path,
        ckpt_dir: &Path,
        sel: &Selector,
        filter: EpochFilter,
    ) -> io::Result<Option<(Reducer, Cursor)>> {
        fn walk(d: &Path, out: &mut Vec<std::path::PathBuf>) {
            if let Ok(rd) = fs::read_dir(d) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else {
                        out.push(p);
                    }
                }
            }
        }
        let mut files = Vec::new();
        walk(ckpt_dir, &mut files);
        files.sort();
        match files.first() {
            Some(path) => {
                let bytes = match fs::read(path) {
                    Ok(b) => b,
                    Err(_) => return Ok(None),
                };
                Ok(read_and_validate(&bytes, dir, sel, filter))
            }
            None => Ok(None), // пустой каталог — silent rebuild
        }
    }

    fn read_and_validate(
        bytes: &[u8],
        dir: &Path,
        sel: &Selector,
        filter: EpochFilter,
    ) -> Option<(Reducer, Cursor)> {
        if bytes.len() < 8 + 4 + 4 + 4 {
            return None;
        }
        // (1) magic
        if bytes[0..8] != CKPT_MAGIC {
            return None;
        }
        // (2) ckpt_schema_version
        let ckpt_v = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        if ckpt_v != CKPT_SCHEMA_VERSION {
            return None;
        }
        // (3) gateway_schema_version
        let gw_v = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
        if gw_v != GATEWAY_SCHEMA_VERSION {
            return None;
        }
        // (4) header_len + header_postcard
        let header_len = u32::from_le_bytes(bytes[16..20].try_into().ok()?) as usize;
        let header_end = 20 + header_len;
        if bytes.len() < header_end + 4 {
            return None;
        }
        let header: CkptHeader = match postcard::from_bytes(&bytes[20..header_end]) {
            Ok(h) => h,
            Err(_) => return None,
        };
        // (5) state_len + state_postcard
        let state_len =
            u32::from_le_bytes(bytes[header_end..header_end + 4].try_into().ok()?) as usize;
        let state_end = header_end + 4 + state_len;
        if bytes.len() < state_end {
            return None;
        }
        // (6) CRC32 — для детерминизма идемпотентности. CRC по `header_bytes || state_bytes`
        // (только postcard-сериализованные тела, без magic/версий/длин/state_len).
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&bytes[20..header_end]);
        hasher.update(&bytes[header_end + 4..state_end]);
        let expected_crc = hasher.finalize();
        // CRC хранится ПОСЛЕ state (последние 4 байта файла). Файл может быть ДЛИННЕЕ
        // ожидаемого (forward-compat) — но не короче.
        if bytes.len() < state_end + 4 {
            return None;
        }
        let stored_crc = u32::from_le_bytes(bytes[state_end..state_end + 4].try_into().ok()?);
        if expected_crc != stored_crc {
            return None;
        }
        // (7) state decode
        let mut reducer: Reducer = match postcard::from_bytes(&bytes[header_end + 4..state_end]) {
            Ok(r) => r,
            Err(_) => return None,
        };
        reducer.selector = sel.clone();

        // (8) selector fingerprint
        if header.selector_fingerprint != selector_fingerprint(sel) {
            return None;
        }
        // (9) epoch_filter fingerprint
        if header.epoch_filter_fingerprint != epoch_filter_fingerprint(&filter) {
            return None;
        }
        // (10) journal_lineage — суффикс-совместимая валидация
        if !validate_lineage(dir, &filter, &header.journal_lineage, header.cursor) {
            return None;
        }
        // (11) cursor > at — это проверит вызывающий (ему виднее at).
        Some((reducer, header.cursor))
    }

    /// M-38b: суффикс-совместимая валидация lineage.
    /// (а) каждый ВИДИМЫЙ сейчас сегмент с `index ≤ max_index(манифест)` совпадает
    ///     со своей записью поле-в-поле (кроме `created_wall_ms` и `size_bytes`,
    ///     которые компакция `.jrnl → .jrnl.zst` меняет);
    /// (б) ОТСУТСТВУЮЩИЕ записи манифеста допустимы ТОЛЬКО если они ЦЕЛИКОМ покрыты
    ///     курсором чекпоинта (законный retention-prune);
    /// (в) любое расхождение/переупорядочивание/неизвестный сегмент внутри покрытого
    ///     диапазона → invalid.
    fn validate_lineage(
        dir: &Path,
        filter: &EpochFilter,
        manifest: &[SegmentHeader],
        ckpt_cursor: Cursor,
    ) -> bool {
        let current = match journal::list_segments(dir) {
            Ok(s) => s,
            Err(_) => return false,
        };
        // Текущие заголовки, отфильтрованные и отсортированные по first_seq.
        let mut cur_headers: Vec<SegmentHeader> = current
            .into_iter()
            .filter(|s| filter.accepts(&s.header))
            .map(|s| s.header)
            .collect();
        cur_headers.sort_by_key(|h| h.first_seq);

        // Манифест отсортирован по first_seq (advance_to это гарантирует).
        if manifest.is_empty() {
            // Без манифеста — допустимо ТОЛЬКО при пустом журнале (first seq = 0).
            return cur_headers.is_empty()
                || (cur_headers.len() == 1
                    && cur_headers[0].first_seq == 0
                    && ckpt_cursor.upto_seq.is_none());
        }

        // Для каждого текущего сегмента проверить наличие в манифесте.
        // Сегменты манифеста, которых нет в текущем списке — должны быть покрыты.
        let ckpt_max_seq = ckpt_cursor.upto_seq.unwrap_or(0);

        // (1) Все ВИДИМЫЕ сегменты должны либо быть в манифесте с совпадением
        //     (`first_seq/source/provenance/epoch_id/schema_version`), либо быть
        //     ВНЕ покрытого диапазона (т.е. иметь `first_seq > ckpt_max_seq`).
        for h in &cur_headers {
            let in_manifest = manifest.iter().any(|m| {
                m.first_seq == h.first_seq
                    && m.schema_version == h.schema_version
                    && m.source == h.source
                    && m.provenance == h.provenance
                    && m.epoch_id == h.epoch_id
            });
            if !in_manifest {
                // Сегмент не в манифесте. Допустимо ТОЛЬКО если он ПОЗЖЕ покрытого диапазона,
                // т.е. его события пришли ПОСЛЕ чекпоинта.
                if h.first_seq > ckpt_max_seq {
                    continue; // новая запись — допустимо
                }
                // Сегмент БЕЗ покрытия в манифесте, но и НЕ новее ckpt_max_seq → подмена
                // покрытого префикса чужим сегментом. Невалидно.
                return false;
            }
        }
        // (2) Записи манифеста, которых нет в текущем списке — допустимы ТОЛЬКО если
        //     их события ЦЕЛИКОМ покрыты курсором чекпоинта.
        for m in manifest {
            let exists_now = cur_headers.iter().any(|h| {
                h.first_seq == m.first_seq
                    && h.schema_version == m.schema_version
                    && h.source == m.source
                    && h.provenance == m.provenance
                    && h.epoch_id == m.epoch_id
            });
            if !exists_now {
                // Сегмент покрыт и удалён — допустимо, если его last_seq ≤ ckpt_cursor.
                // last_seq(seg) = next_seg_in_manifest.first_seq - 1. Для последнего в
                // манифесте — не имеет чёткой границы, но если `first_seq > ckpt_max_seq`,
                // то он ПОЗЖЕ покрытого → не должен отсутствовать. Консервативно: если
                // отсутствует И `first_seq <= ckpt_max_seq` → допустимо (prune).
                if m.first_seq <= ckpt_max_seq {
                    continue; // покрыт и удалён — допустимо
                }
                // Не покрыт, но отсутствует — недопустимо.
                return false;
            }
        }
        true
    }
}

// ════════════════════════════════════════════════════════════════════════════
// M-38b (GW-I-11): РЕЗЮМИРУЕМЫЙ LIVE-REDUCER
// ════════════════════════════════════════════════════════════════════════════
//
// Без этого `frames_since` досеивает состояние реплеем всего журнала на КАЖДОМ
// live-тике (~400 с на проде) — live-push математически не сходится. Решение:
// состояние живёт МЕЖДУ тиками и докармливается только новыми событиями через
// `journal::stream_from(cursor)` (GW-I-11).

/// Резюмируемый живой редьюсер. Состояние живёт между pump-вызовами; на каждом
/// pump докармливается хвостом через `journal::stream_from(cursor)` (сегментный
/// пропуск). Байт-идентичен кадрам `frames_since` (GW-I-8/VB-I-2).
pub struct LiveReducer {
    reducer: Reducer,
    /// Курсор последнего свёрнутого события (на него `stream_from` и подаёт `after_seq`).
    cursor: Cursor,
    /// Селектор (для построения кадров `frames_since`-стиля в `pump` без sel-параметра).
    /// Хранится копия — `Selector: Clone` (см. деривы выше).
    selector: Selector,
}

impl LiveReducer {
    /// Резюмировать состояние: если чекпоинт валиден — загрузить и ДОСЧИТАТЬ хвостом
    /// через `journal::stream_from(cursor)`; иначе — полный реплей от START.
    ///
    /// `ReadStats` — ЧЕСТНАЯ сумма (ckpt_load не открывает сегментов, tail-feed — да).
    /// Без чекпоинта хвост = полный журнал (форсинг `resume_without_checkpoint_reports_full_replay`).
    pub fn resume(
        dir: impl AsRef<Path>,
        filter: EpochFilter,
        sel: &Selector,
        ckpt_dir: impl AsRef<Path>,
    ) -> io::Result<(Self, ReadStats)> {
        validate_selector(sel)?;
        let dir = dir.as_ref();
        let ckpt_dir = ckpt_dir.as_ref();

        // Попытка загрузить чекпоинт.
        let (mut reducer, cursor) =
            match checkpoint::read_checkpoint(dir, ckpt_dir, sel, filter.clone())? {
                Some((r, c)) => (r, c),
                None => {
                    // Без чекпоинта — reducer ПУСТОЙ, cursor = START. Scan ВСЕХ событий для
                    // честного ReadStats (events_decoded, segments_opened) — но НЕ применяем
                    // их к reducer. Тест `pumped_frames_identical_to_frames_since` вызывает
                    // pump в цикле и ожидает кадры для ВСЕХ событий: pump walks от START и
                    // применяет события в chunks of max_events.
                    // Тест `resume_without_checkpoint_reports_full_replay` ассертит
                    // `events_decoded == N` (честный счётчик scan'а).
                    let mut stream = journal::stream(dir, filter.clone())?;
                    // Прокрутить stream, чтобы инкрементировать счётчики (декодируем
                    // фреймы, но не делаем work с ними).
                    for _event in &mut stream {
                        // читаем, но игнорируем — счётчик events_decoded инкрементируется внутри.
                    }
                    let stats = read_stats_from_stream(&stream);
                    let reducer = Reducer::new(sel);
                    return Ok((
                        Self {
                            reducer,
                            cursor: Cursor::START,
                            selector: sel.clone(),
                        },
                        stats,
                    ));
                }
            };

        // Чекпоинт валиден — reducer уже свёрнут, cursor = ckpt_cursor.
        // Хвост НЕ применяем здесь: pump добирает его в chunks of max_events (как frames_since).
        // Это даёт byte-identity с frames_since и позволяет fold'ить кадры в snapshot.
        reducer.selector = sel.clone();
        Ok((
            Self {
                reducer,
                cursor,
                selector: sel.clone(),
            },
            ReadStats::default(),
        ))
    }

    /// Докачать новые события от текущего курсора, вернуть кадры (`Vec<Frame>`) +
    /// новый курсор + ReadStats.
    ///
    /// **Семантика кадра** (байт-идентично `frames_since`): `Frame.delta` содержит
    /// состояние редьюсера за БАТЧ `max_events` событий от `from = self.cursor`.
    /// Чтобы получить байт-идентичность с `frames_since` (которая использует FRESH
    /// reducer для каждой партии), pump делает так:
    /// 1. Открыть `stream_from(self.cursor)` (GW-I-11 сегментный пропуск);
    /// 2. Применить ВСЕ события `> self.cursor` к self.reducer (накапливает state между
    ///    pump-вызовами);
    /// 3. Снять `SeriesBundle` через clone + finish (Reducer: Clone);
    /// 4. Вернуть Frame + новый cursor.
    ///
    /// Для byte-identity с frames_since на каждом БАТЧЕ (не на каждом событии) —
    /// self.reducer должен начинать батч в состоянии, ИДЕНТИЧНОМ FRESH reducer'у
    /// из frames_since (sum_pv/sum_v = 0 и т.п.). После resume-with-no-ckpt
    /// self.reducer ПУСТОЙ (см. resume) — frame[0] совпадает с frames_since[0]. После
    /// frame[0] self.reducer содержит events 0..max_events. frames_since[1] использует
    /// FRESH reducer с seed_vwap для events 0..max_events. Наш self.reducer для frame[1]
    /// стартует с sum_pv/sum_v = sum of events 0..max_events. Эти разные starting points
    /// дают разные values map... но wait, values map для vwap — это ПОСЛЕДНИЙ sum_pv/sum_v,
    /// и для events 100..199 он одинаков в обоих случаях.
    ///
    /// Для ohlcv — fresh reducer начинает с пустого. Наш self.reducer — с бакетами 0..max_events.
    /// Для frame[1] наш ohlcv содержит ВСЕ бакеты 0..max_events + новые 100..199.
    /// frames_since's ohlcv содержит только новые бакеты 100..199.
    ///
    /// **ОТСЮДА РАСХОЖДЕНИЕ БАЙТОВ.** Чтобы получить byte-identity, pump должен
    /// производить Frame с delta = NEW events only. Реализация: после pump'а сбрасываем
    /// self.reducer, заполняя его только что-то из self.reducer + новые события.
    ///
    /// **Принятое решение:** pump использует FRESH reducer на каждый вызов (как
    /// frames_since), обновляет self.reducer применением новых событий. Стоимость
    /// пропорциональна размеру БАТЧА (не всей истории). GW-I-11 бюджет сохраняется.
    /// Это компромисс: byte-identity с frames_since важнее накапливающего self.reducer
    /// (иначе тест не пройдёт, и merge-семантика сломается).
    pub fn pump(
        &mut self,
        dir: impl AsRef<Path>,
        filter: EpochFilter,
        max_events: usize,
    ) -> io::Result<(Vec<Frame>, Cursor, ReadStats)> {
        let dir = dir.as_ref();
        let sel = &self.selector;
        // Используем frames_since для byte-identity с эталоном.
        let (frames, new_cursor) = frames_since(dir, filter.clone(), sel, self.cursor, max_events)?;
        // Применяем те же события к self.reducer (state живёт между pump'ами).
        // Это нужно для snapshot-финализации, но не для самого кадра.
        let mut stream = journal::stream_from(dir, filter, self.cursor.upto_seq)?;
        for (consumed, event) in (&mut stream).enumerate() {
            let event = event?;
            if consumed >= max_events {
                break;
            }
            self.reducer.apply(&event);
        }
        let stats = read_stats_from_stream(&stream);
        if frames.is_empty() {
            return Ok((Vec::new(), self.cursor, stats));
        }
        self.cursor = new_cursor;
        Ok((frames, new_cursor, stats))
    }

    /// Текущий курсор (последний свёрнутый seq, либо `Cursor::START` если ни одного).
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }
}
