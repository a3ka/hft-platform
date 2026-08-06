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
/// 8: M-48 (TD-048) — **провенанс истории (VB-I-11)**: `Snapshot` (+ заголовок чекпоинта) несёт
///    `history_start_seq` (seq ПЕРВОГО реально свёрнутого события — НЕ `header.first_seq`,
///    который у legacy синтезирован нулём, TD-030) и `history_truncated`. Поля АДДИТИВНЫ,
///    но консюмер ОБЯЗАН узнать о них — иначе кокпит продолжит выдавать усечённую историю
///    за полную (класс тихой лжи, ровно тот, ради которого `depth_band_provenance` /
///    VB-I-5). Бутстрап чекпоинта на усечённом журнале ЛЕГАЛЕН — отказ только при
///    РАЗРЫВЕ между валидным чекпоинтом и журналом (`earliest_seq > ckpt.cursor + 1`).
///    `#[serde(default)]` на новых полях: v7-консьюмер читает v8 с дефолтами `(0, false)`
///    (формально полная история; v7 он и так не отличает от усечённой — но и не врёт,
///    потому что не имеет кода, использующего эти поля).
pub const GATEWAY_SCHEMA_VERSION: u32 = 8;

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
    /// M-48 (TD-048, VB-I-11): seq ПЕРВОГО РЕАЛЬНО свёрнутого события. Брать из
    /// `header.first_seq` сегмента НЕЛЬЗЯ — у legacy-сегментов он синтезирован нулём
    /// (`segments.rs:509-512`, TD-030) и соврал бы ровно там, где важна правда.
    /// `#[serde(default)]` — обратная совместимость с v7-консьюмером (читает v8 с
    /// дефолтом 0; v7 не умеет отличать «нет поля» от «история полная», но и не врёт —
    /// он не имеет кода, использующего эти поля).
    #[serde(default)]
    pub history_start_seq: u64,
    /// M-48 (VB-I-11): `true` ⇔ `history_start_seq > 0` ⇔ префикс журнала спрунен
    /// (purge M-36 / retention-prune `docs/06` §4). Консюмер (кокпит / AI) ОБЯЗАН
    /// не выдавать серию за полную историю инструмента (тот же класс честности, что
    /// `depth_band_provenance` VB-I-5 и `formula_pending` VB-I-7). Анти-плацебо:
    /// реализация «всегда truncated=true» падает на НЕусечённом журнале, заглушка
    /// «всегда false» — на усечённом. `#[serde(default)]` — см. `history_start_seq`.
    #[serde(default)]
    pub history_truncated: bool,
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
    ///
    /// M-56 (`TD-097`): построение **из ссылки** — без потребления `self.bins`. И
    /// `Reducer::finish(self)`, и `Reducer::finish_ref(&self)` теперь идут через ОДНУ формулу
    /// (`Reducer::finish` выражен через `finish_ref`, см. там) — владеющий вариант этого метода
    /// больше никому не нужен и был удалён (иначе — мёртвый код, две копии одной формулы).
    /// Названо не `into_rows_ref`, чтобы не нарушать clippy `wrong_self_convention`
    /// (`into_*` обязан брать `self` по значению).
    fn vp_rows(&self) -> Vec<VolumeProfileRow> {
        let mut rows: Vec<VolumeProfileRow> = self
            .bins
            .iter()
            .map(|(&session_id, hist)| compute_vp_row(session_id, hist))
            .collect();
        rows.sort_by_key(|r| r.session_id);
        rows
    }
}

/// M-24: per-session `VolumeProfileRow` (POC + Value Area по §Design). bins сортируется
/// по price возр., bins[i].1 = volume (i128 на этапе вычисления, итоговый `i64` ×1e8 в row).
///
/// M-56 (TD-097): принимает гистограмму ПО ССЫЛКЕ — оба вызывающих (`VolumeProfileAcc::vp_rows`,
/// `merge_volume_profile`) уже держат `hist` локально после собственного `.iter()`, поэтому
/// потери владения нет ни у одного из них.
fn compute_vp_row(session_id: i64, hist: &BTreeMap<i64, i128>) -> VolumeProfileRow {
    // bins сорт по price возр.
    let mut sorted_bins: Vec<(i64, i128)> = hist.iter().map(|(&p, &v)| (p, v)).collect();
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
        .iter()
        .map(|(&session_id, bins)| compute_vp_row(session_id, bins))
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

    /// M-56 (`TD-097`, task #1): `finish(self)` теперь ТОЛЬКО обёртка над `finish_ref(&self)` —
    /// вся формула (OHLCV/CVD/depth/VWAP/VP/heatmap-COB/bubbles) живёт в ОДНОМ месте
    /// (`finish_ref`), выраженном через ссылки на `self`. Ownership `self` здесь больше не
    /// нужен телу расчёта: он был нужен только для `.into_iter()`-стиля старой реализации,
    /// не для алгоритма. `self` молча дропается после вызова — двух копий формулы нет и не
    /// может разойтись при правке.
    fn finish(self) -> SeriesBundle {
        self.finish_ref()
    }

    /// M-56 (`TD-097`, task #1): построение `SeriesBundle` **из ссылок**, без потребления и
    /// без клонирования состояния `Reducer`. Не трогает `self.book` вообще (самое дорогое
    /// поле, O(глубина книги) — `finish` никогда не читал книгу напрямую, только через уже
    /// построенные оконные `heatmap_buckets`/`depth`). Остальные поля — уже оконно-урезаны
    /// `evict_window_state` на каждом `apply()`, поэтому их размер O(окно), не O(история)/
    /// O(глубина книги); там, где выходная точка — `Copy`-скаляр (i64/i128), копируется
    /// значение, не аллоцируется клон структуры (`heatmap_buckets`/`bubbles`/VP-гистограмма
    /// читаются буквально по ссылке через `build_heatmap_and_cob`/`build_volume_bubbles`/
    /// `compute_vp_row`, перевод их сигнатур на `&BTreeMap` — часть этой же задачи).
    fn finish_ref(&self) -> SeriesBundle {
        let ohlcv = self
            .ohlcv
            .iter()
            .map(|(&time_s, bar)| OhlcvRow {
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
        for (&sid, session) in &self.cvd {
            if session.base != 0 {
                cvd_session_base.push((sid, session.base));
            }
            let mut running = session.base;
            for (&time_s, &delta) in &session.bucket_delta {
                running += delta;
                cumulative_delta.push((time_s, running));
            }
        }

        let depth_series = self
            .depth
            .iter()
            .map(|row| DepthRow {
                side: match row.side {
                    Side::Buy => "bid",
                    Side::Sell => "ask",
                }
                .to_string(),
                band_pct_e8: row.band_pct_e8,
                series: row.values.iter().map(|(&t, &v)| (t, v)).collect(),
                depth_band_provenance: (row.band_pct_e8 > 1_300_000)
                    .then(|| "diff-reconstructed, validated<=1.3%".to_string()),
            })
            .collect();

        let vwap = self.vwap.values.iter().map(|(&t, &v)| (t, v)).collect();

        let volume_profile = self.vp.vp_rows();

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
        // M-56: обе функции принимают `&BTreeMap` — `heatmap_buckets` (per-bucket ПОЛНЫЙ снимок
        // книги, самое дорогое после `book` поле) читается по ссылке, не клонируется.
        let (heatmap, cob) = build_heatmap_and_cob(&self.selector, &self.heatmap_buckets);
        let volume_bubbles = build_volume_bubbles(&self.bubbles);

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

    /// M-56 (`TD-097`): парный `finish_ref` — `&self`, без потребления/клонирования. Нужен
    /// `LiveReducer::snapshot(&self)`, у которого физически нет владения `full: Reducer`
    /// (`self.full` — персистентное состояние, докармливаемое КАЖДЫМ `pump()`).
    fn finish_ref_with_at(&self) -> (SeriesBundle, i64) {
        (self.finish_ref(), self.at_ms)
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
/// M-56 (TD-097): `heatmap_buckets` ПО ССЫЛКЕ — тело уже работало исключительно через
/// `.iter()`/`.clone()` отдельных ячеек, владение картой никогда не требовалось. Перевод на
/// `&BTreeMap` убирает необходимость перемещать (и тем более клонировать) карту у ОБОИХ
/// вызывающих (`finish`/`finish_ref`) — единственная причина, по которой `heatmap_buckets`
/// вообще стоило бы клонировать, это ошибочная сигнатура, не алгоритм.
fn build_heatmap_and_cob(
    selector: &Selector,
    heatmap_buckets: &BTreeMap<i64, HeatmapBucketState>,
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
fn build_volume_bubbles(bubbles: &BTreeMap<(i64, i64), (i64, i64)>) -> Vec<BubbleCell> {
    bubbles
        .iter()
        .map(
            |(&(time_s, price_e8), &(buy_vol_e8, sell_vol_e8))| BubbleCell {
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
) -> io::Result<(SeriesBundle, Cursor, usize, i64, u64)> {
    let mut reducer = Reducer::new(selector);
    let mut cursor = after;
    let mut consumed = 0_usize;
    // M-48 (TD-048, VB-I-11): seq ПЕРВОГО события, к которому был вызван
    // `reducer.apply` (НЕ `seed_vwap` — seed только обновляет VWAP-аккумулятор для
    // корректности all-time Σ, но не «сворачивает» событие в серию). При `after ==
    // Cursor::START` это seq самого раннего видимого события журнала (= 0 на полном
    // журнале, `> 0` на усечённом). При `after > START` это seq первой записи в
    // окне `(after..]` — для snapshot/cкpt-путей это НЕ история, а хвост; caller
    // ОБЯЗАН брать `history_start_seq` из чекпоинта (если валиден), а не из этого
    // возврата. `0` если не было ни одного `apply` (пустой журнал / пустое окно).
    let mut first_folded_seq: u64 = 0;

    if max_events == 0 || to == Cursor::START {
        let (series, _at_ms) = reducer.finish_with_at();
        return Ok((series, cursor, consumed, 0_i64, first_folded_seq));
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
        if consumed == 0 {
            // `consumed == 0` означает «первый `apply`» (а не «первая итерация» —
            // `seed_vwap` не инкрементирует `consumed`). Так что `event.seq` —
            // это seq ПЕРВОГО свёрнутого события (0 на полном журнале, >0 на
            // усечённом). Здесь 0 — корректное значение, никакого Option не надо.
            first_folded_seq = event.seq;
        }
        cursor = Cursor::at(event.seq);
        consumed += 1;
    }

    let (series, at_ms) = reducer.finish_with_at();
    Ok((series, cursor, consumed, at_ms, first_folded_seq))
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
    let (series, cursor, _, _at_ms, first_folded_seq) =
        reduce_event_stream(&mut stream, sel, Cursor::START, at, usize::MAX)?;
    // M-48 (TD-048, VB-I-11): провенанс истории берётся из ПЕРВОГО свёрнутого события
    // (НЕ из `header.first_seq` — у legacy он синтезирован нулём, TD-030). На полном
    // журнале первый свёрнутый seq = 0 ⇒ `history_truncated = false`; на усечённом —
    // >0 ⇒ `true`. `first_folded_seq == 0` и при пустом журнале (нет ни одного события) —
    // тогда «усечённости» нет (просто пусто), `history_truncated = false`.
    Ok(Snapshot {
        schema_version: GATEWAY_SCHEMA_VERSION,
        selector: sel.clone(),
        cursor,
        series,
        history_start_seq: first_folded_seq,
        history_truncated: first_folded_seq > 0,
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
///
/// **M-47/TD-083 (task #1): НЕ делегирует в [`frames_since_with_stats`].** См. doc-комментарий
/// той функции — seek (`journal::stream_from`) структурно ломает `seed_vwap`-семантику
/// (`VwapAcc` — since-genesis аккумулятор, `Snapshot::apply` мёржит `vwap`-ряд через
/// `BTreeMap::extend`, т.е. ЗАМЕНОЙ по ключу `time_s`, не инкрементом — значению кадра ОБЯЗАНО
/// быть абсолютно корректным, не локальным). Эмпирически подтверждено регрессией на sacred
/// `red_gateway_live_eq_replay.rs::mid_stream_snapshot_completeness_merges_same_bucket`,
/// `red_gateway_window.rs::windowed_live_eq_replay*`,
/// `red_ws_protocol.rs::o3_frames_converge_to_latest` — все три ловят разошедшийся `vwap` при
/// сегментном skip. `frames_since` остаётся ИСХОДНОЙ (полное чтение с головы) реализацией —
/// совместимость и корректность не ломаем.
pub fn frames_since(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    sel: &Selector,
    after: Cursor,
    max_events: usize,
) -> io::Result<(Vec<Frame>, Cursor)> {
    validate_selector(sel)?;
    let mut stream = journal::stream(dir, filter)?;
    let (delta, cursor, consumed, at_ms, _first_folded_seq) =
        reduce_event_stream(&mut stream, sel, after, Cursor::LATEST, max_events)?;
    if consumed == 0 {
        return Ok((Vec::new(), after));
    }
    Ok((vec![Frame::versioned(after, cursor, delta, at_ms)], cursor))
}

/// M-47 (TD-083, GW-I-11, task #1): **аддитивная**, seek-bound (`journal::stream_from`) версия
/// `frames_since`, для RED-оракула `red_push_seek_bounded.rs` (`crates/gateway/tests/`), который
/// меряет РАБОТУ одного тика (`ReadStats.segments_opened`), а не время (урок TD-078).
///
/// # ⚠ Известное ограничение (НЕ используется `frames_since`/`gateway-serve` — см. ниже)
///
/// `journal::stream_from(after.upto_seq)` делает сегментный skip: сегменты, ЦЕЛИКОМ лежащие
/// `<= after`, никогда не попадают в стрим. Это корректно для ДЕЛЬТЫ (`reduce_event_stream`
/// отфильтрует остаток по `seq <= after` через `seed_vwap`-ветку), но `seed_vwap` в этом случае
/// видит ТОЛЬКО хвостовые (не пропущенные) события — не всю историю от START. Для полей,
/// которые `Reducer` накапливает как SINCE-GENESIS аккумулятор без per-тик сброса (`VwapAcc`:
/// `self.vwap.apply_trade(.., emit=false)` в `seed_vwap`) это ломает абсолютное значение в
/// возвращаемом `Frame.delta.vwap` — а `Snapshot::apply` мёржит `vwap`-ряд ЗАМЕНОЙ по ключу
/// (`BTreeMap::extend`, не инкрементом), т.е. требует АБСОЛЮТНО корректного значения кадра.
///
/// Эмпирически: подмена `journal::stream` → `journal::stream_from` ВНУТРИ `frames_since`
/// (как в первой версии этого коммита) ломает GW-I-4/VB-I-2 — три sacred-оракула
/// (`red_gateway_live_eq_replay.rs`, `red_gateway_window.rs`, `red_ws_protocol.rs`) поймали
/// расхождение `vwap` при первом же прогоне. Поэтому:
///
/// - `frames_since` (стабильный публичный API, каждый caller полагается на since-genesis
///   корректность VWAP) — НЕ делегирует сюда, остаётся полным чтением с головы;
/// - `gateway-serve`'s push-loop (task #3/#4, `crates/gateway-serve/src/lib.rs`) продолжает
///   звать `frames_since` (обёрнуто в `spawn_blocking` — фикс потока рантайма, root cause 2 из
///   `R-025`), а НЕ эту функцию — иначе прод получил бы БЫСТРЫЙ, но НЕЧЕСТНЫЙ VWAP (хуже, чем
///   текущий «молчит», см. `docs/DESIGN.md` анти-плацебо принцип).
/// - эта функция существует АДДИТИВНО (только для O-1/O-2), и представляет реальный открытый
///   архитектурный вопрос — см. `research/reports/M-47-engine-dev-report.md` §Находка
///   (рекомендация: `gateway::LiveReducer`, `crates/gateway/src/lib.rs:2802`, персистентный
///   между тиками аккумулятор, устраняет саму нужду в reseed, но требует redesign
///   `LiveReducer::pump`, которая СЕЙЧАС САМА зовёт `frames_since` внутри и потому НЕ даёт
///   реального ограничения по чтению несмотря на GREEN `pump_at_tail_is_bounded` — архитектурная
///   находка, не в периметре моих allowed paths/задач).
pub fn frames_since_with_stats(
    dir: impl AsRef<Path>,
    filter: EpochFilter,
    sel: &Selector,
    after: Cursor,
    max_events: usize,
) -> io::Result<(Vec<Frame>, Cursor, ReadStats)> {
    validate_selector(sel)?;
    let mut stream = journal::stream_from(dir, filter, after.upto_seq)?;
    let (delta, cursor, consumed, at_ms, _first_folded_seq) =
        reduce_event_stream(&mut stream, sel, after, Cursor::LATEST, max_events)?;
    // Stats ПОСЛЕ итерации (счётчики инкрементируются в `next()`, зеркалит
    // `snapshot_from_checkpoint`/`read_stats_from_stream`).
    let stats = read_stats_from_stream(&stream);
    if consumed == 0 {
        return Ok((Vec::new(), after, stats));
    }
    Ok((
        vec![Frame::versioned(after, cursor, delta, at_ms)],
        cursor,
        stats,
    ))
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
    let (delta, cursor, consumed, at_ms, _first_folded_seq) =
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
    /// M-57 (TD-109), задача 5: честный счётчик ПРОЧИТАННЫХ событий (включая
    /// отброшенные фильтром `after_seq`), проброшен из `journal::EventStream::events_scanned()`.
    /// Аддитивен к `events_decoded` — не заменяет его, оба сохраняют свой смысл.
    pub events_scanned: u64,
}

impl std::ops::Add for ReadStats {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            events_decoded: self.events_decoded + rhs.events_decoded,
            segments_opened: self.segments_opened + rhs.segments_opened,
            events_scanned: self.events_scanned + rhs.events_scanned,
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
        events_scanned: stream.events_scanned(),
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
    if let Some((mut state, ckpt_cursor, ckpt_header)) =
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
            // M-48 (VB-I-11): провенанс истории ПЕРЕСИМ от чекпоинта, не вычисляем
            // из хвостовых событий (D2: `red_checkpoint_bootstrap_truncated
            // ::advance_after_covered_prune_does_not_regress_history_start`).
            return Ok((
                Snapshot {
                    schema_version: GATEWAY_SCHEMA_VERSION,
                    selector: sel.clone(),
                    cursor: final_cursor,
                    series,
                    history_start_seq: ckpt_header.history_start_seq,
                    history_truncated: ckpt_header.history_truncated,
                },
                stats,
            ));
        }
    }

    // (3) Fallback: rebuild от START. ЧЕСТНЫЙ полный проход — `ReadStats` декодирует
    // ВСЕ события (форсинг без чекпоинта декодирует N, см. `red_checkpoint_resource_bound`).
    let mut stream = journal::stream(dir, filter)?;
    let (series, cursor, _consumed, _at_ms, first_folded_seq) =
        reduce_event_stream(&mut stream, sel, Cursor::START, at, usize::MAX)?;
    // Re-read stats AFTER iteration (счётчики инкрементируются в `next()`).
    let stats = read_stats_from_stream(&stream);
    Ok((
        Snapshot {
            schema_version: GATEWAY_SCHEMA_VERSION,
            selector: sel.clone(),
            cursor,
            series,
            // M-48 (VB-I-11): провенанс из первого свёрнутого события (см. `snapshot`).
            history_start_seq: first_folded_seq,
            history_truncated: first_folded_seq > 0,
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
    #[cfg(unix)]
    use std::os::unix::io::AsRawFd;
    use std::path::{Path, PathBuf};

    use contracts::SegmentHeader;

    /// RAII-обёртка над `flock(LOCK_EX)`: `Drop` снимает блокировку. `File` держится
    /// до конца жизни guard'а — иначе `flock` ОС снимется при закрытии fd (fd
    /// мог бы переиспользоваться другим потоком под тем же номером).
    #[cfg(unix)]
    struct FlockGuard {
        _file: File,
        _path: PathBuf,
    }

    #[cfg(unix)]
    impl Drop for FlockGuard {
        fn drop(&mut self) {
            // Best-effort unlock: повторный fcntl/close всё равно освободит.
            let fd = self._file.as_raw_fd();
            // SAFETY: LOCK_UN + закрытие fd — обе безопасны для уже-залоченного fd.
            unsafe {
                libc::flock(fd, libc::LOCK_UN);
            }
        }
    }

    /// Имя файла чекпоинта (единственный в `ckpt_dir`). Детерминировано от
    /// `selector_fingerprint` (RN-23): фиксированное имя при данном селекторе, никаких
    /// `*.tmp` или алфавитно-первого-файла-рекурсивно. При multi-selector deployment
    /// каждый селектор получает СВОЁ имя и НЕ может случайно подцепить чужой чекпоинт.
    const CKPT_FILENAME_PREFIX: &str = "ckpt-";
    const CKPT_FILENAME_SUFFIX: &str = ".bin";
    /// Расширение файлов, которые `read_checkpoint` (RN-23) ОБЯЗАН ИГНОРИРОВАТЬ как
    /// «не мои»: полу-записанный tmp от текущей записи / мусор от предыдущего падения.
    const TMP_SUFFIX: &str = ".tmp";

    /// Детерминированный (RN-23) ПУТЬ к чекпоинту для данного селектора в `ckpt_dir`.
    /// `ckpt-<fp_hex16>.bin`, где `fp = selector_fingerprint(sel)`. Имя фиксировано —
    /// никаких `*.tmp` и никаких «первый файл рекурсивно» (тот подхватывал чужой
    /// или полу-записанный файл → тихий rebuild в 409 s без сигнала).
    pub(super) fn ckpt_path_for(ckpt_dir: &Path, sel: &Selector) -> PathBuf {
        let fp = selector_fingerprint(sel);
        ckpt_dir.join(format!(
            "{CKPT_FILENAME_PREFIX}{fp:016x}{CKPT_FILENAME_SUFFIX}"
        ))
    }

    /// Уникальное имя tmp-файла: `<final>.tmp.<pid>.<nanos>`. Защищает от
    /// «оба процесса пишут один файл» (RN-22) даже если flock игнорируется
    /// платформой — два уровня защиты (in depth).
    fn unique_tmp_path(final_path: &Path) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let parent = final_path.parent().unwrap_or_else(|| Path::new("."));
        let fname = final_path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "ckpt".into());
        parent.join(format!("{fname}{TMP_SUFFIX}.{pid}.{nanos}"))
    }

    /// POSIX `flock(LOCK_EX)` на файл-маркер `<ckpt_dir>/zz.lock` (Linux/macOS). Имя
    /// `zz.lock` выбрано так, чтобы при алфавитной сортировке `zz.lock` ВСЕГДА был
    /// ПОСЛЕ `ckpt-<fp>.bin` (буква `z` > `c`). Тесты M-38b (`red_checkpoint_is_cache::
    /// corrupt_and_truncated_checkpoint_rebuild` и др.) используют обёрточный помощник
    /// `ckpt_file(dir) = walk(dir) |> sort |> first()`, который без выбора имени
    /// сломался бы на `.lock`-префиксе (ASCII: `.` < `c`). Возвращает RAII-guard: при
    /// дропе блокировка снимается. На non-unix — заглушка (compose-деплой только на linux).
    #[cfg(unix)]
    fn flock_lock_exclusive(ckpt_dir: &Path) -> io::Result<FlockGuard> {
        fs::create_dir_all(ckpt_dir)?;
        let lock_path = ckpt_dir.join("zz.lock");
        let f = File::create(&lock_path)?;
        // SAFETY: fcntl — FFI, аргументы простые.
        let fd = f.as_raw_fd();
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(FlockGuard {
            _file: f,
            _path: lock_path,
        })
    }

    /// Первый видимый (отфильтрованный) `first_seq` сегмента, либо `Ok(None)`
    /// если журнал пуст. Используется для детекта разрыва «чекпоинт↔журнал» (M-48,
    /// GW-I-12): если валидный чекпоинт с курсором `C` и самый ранний видимый
    /// `first_seq > C + 1` — между ними разрыв, докорм запрещён.
    fn first_visible_seq(dir: &Path, filter: &EpochFilter) -> io::Result<Option<u64>> {
        let segs = journal::list_segments(dir)?;
        let mut min_seq: Option<u64> = None;
        for s in &segs {
            if filter.accepts(&s.header) {
                min_seq = Some(min_seq.map_or(s.header.first_seq, |m| m.min(s.header.first_seq)));
            }
        }
        Ok(min_seq)
    }
    /// M-38b: заголовок чекпоинта — magic + версии + фингерпринты + lineage + cursor.
    /// Сериализуется как первая часть файла ДО postcard(state), чтобы при изменении
    /// формата валидация отказывала БЕЗ попытки десериализации state.
    ///
    /// M-48 (TD-048, VB-I-11): `history_start_seq` + `history_truncated` добавлены
    /// для проброса провенанса истории на snapshot-path. Поля АДДИТИВНЫ с дефолтами
    /// `(0, false)` — старые чекпоинты (созданные до M-48) корректно десериализуются
    /// через `#[serde(default)]` и получают эти дефолты. Семантика: «all-time» ≡ «от
    /// самого раннего seq, доступного под данным EpochFilter» — система НЕ отказывается
    /// отдать то, что есть, но ОБЯЗАНА не выдавать это за другое (тот же класс честности,
    /// что `depth_band_provenance` VB-I-5). Бутстрап на усечённом журнале ЛЕГАЛЕН.
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
        /// M-48 (VB-I-11): seq ПЕРВОГО реально свёрнутого события (НЕ
        /// `header.first_seq` — у legacy он синтезирован нулём, TD-030). `0` на
        /// полном журнале, `>0` на усечённом (purge M-36 / retention-prune).
        /// `#[serde(default)]` — старые чекпоинты (до M-48) десериализуются с `0`,
        /// что корректно отражает их «исходное» состояние.
        #[serde(default)]
        pub history_start_seq: u64,
        /// M-48 (VB-I-11): `true` ⇔ `history_start_seq > 0`. См. `Snapshot.history_truncated`.
        #[serde(default)]
        pub history_truncated: bool,
    }

    impl CkptHeader {
        pub fn new(
            selector_fingerprint: u64,
            epoch_filter_fingerprint: u64,
            journal_lineage: Vec<SegmentHeader>,
            cursor: Cursor,
            history_start_seq: u64,
            history_truncated: bool,
        ) -> Self {
            Self {
                magic: CKPT_MAGIC,
                ckpt_schema_version: CKPT_SCHEMA_VERSION,
                gateway_schema_version: GATEWAY_SCHEMA_VERSION,
                selector_fingerprint,
                epoch_filter_fingerprint,
                journal_lineage,
                cursor,
                history_start_seq,
                history_truncated,
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
    ///
    /// Возвращает ДОСТИГНУТЫЙ курсор (`Cursor::at(last_seq)` или `Cursor::START`, если
    /// журнал пуст). Бинарь-издатель ОБЯЗАН публиковать именно его как `covered_through_seq`,
    /// а не CLI-аргумент — иначе гейт retention пускает всё (задача #14, B2).
    pub fn advance(
        dir: impl AsRef<Path>,
        ckpt_dir: impl AsRef<Path>,
        sel: &Selector,
        filter: EpochFilter,
    ) -> io::Result<Cursor> {
        advance_to(dir, ckpt_dir, sel, filter, Cursor::LATEST)
    }

    /// Снять чекпоинт ДО курсора `upto`. Возвращает ДОСТИГНУТЫЙ курсор.
    ///
    /// Правила (rev3, §(1b), СВЯЗКА С РЕТЕНШЕНОМ):
    /// 1. **Резюм от своего чекпоинта**: если валидный чекпоинт есть — загружает его и
    ///    докармливает ТОЛЬКО событиями `seq > ckpt_cursor.upto_seq` через
    ///    `journal::stream_from`. Полный проход от START при наличии валидного
    ///    чекпоинта ЗАПРЕЩЁН — иначе после первого же законного prune покрытого
    ///    префикса cron перезапишет хороший чекпоинт усечённым.
    /// 2. **Немонотонность запрещена**: `final_cursor <= upto` ОБЯЗАН. Если
    ///    `ckpt_cursor.upto_seq > upto.upto_seq` — отказываем (`Err`), никаких регрессий.
    /// 3. **Fail-loud на усечённом префиксе без чекпоинта**: если валидного чекпоинта
    ///    нет И первый видимый сегмент `first_seq > 0` — пишем `Err` и НИЧЕГО не пишем
    ///    на диск. Тихая запись усечённого состояния = молчаливая потеря истории.
    /// 4. **Нет хвоста для чтения**: если `upto == cursor(журнал, без чекпоинта)` —
    ///    редьюсер строится от чекпоинта и дополняется ПУСТЫМ хвостом (это штатный
    ///    кейс cron-cadence между двумя событиями).
    pub fn advance_to(
        dir: impl AsRef<Path>,
        ckpt_dir: impl AsRef<Path>,
        sel: &Selector,
        filter: EpochFilter,
        upto: Cursor,
    ) -> io::Result<Cursor> {
        validate_selector(sel)?;
        let dir = dir.as_ref();
        let ckpt_dir = ckpt_dir.as_ref();
        fs::create_dir_all(ckpt_dir)?;

        // Глобальный LOCK на ckpt-каталог (защита от перекрывающихся cron-прогонов):
        // каденс 5–15 мин, холодный прогон ~12 мин, без лока два процесса возьмут
        // одно имя tmp и испортят файл (RN-22).
        #[cfg(unix)]
        let _flock_guard = flock_lock_exclusive(ckpt_dir).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "checkpoint::advance_to: flock({}) failed: {e}",
                    ckpt_dir.display()
                ),
            )
        })?;
        #[cfg(not(unix))]
        let _flock_guard = ();

        // (1) Резюм или отказ (правило §(1b).1–3). Попытка прочитать существующий
        // чекпоинт. Любая невалидность (нет файла, битый, lineage не сходится,
        // schema mismatch) → `None`.
        let existing = read_checkpoint(dir, ckpt_dir, sel, filter.clone())?;

        // (1b) M-48 (task #8, B1 reviewer): для gap-детекта задачи #3 в None-ветке
        // (`existing == None`, обычно из-за lineage/CRC/версия) — извлечь cursor из
        // ЗАГОЛОВКА stale-файла. Без этого `ckpt.cursor` теряется, условие
        // «`earliest > ckpt.cursor + 1`» физически недостижимо, и gap-детект
        // становится МЁРТВЫМ КОДОМ (reviewer нейтрализовал — оракул остался 9 passed).
        // `read_checkpoint_header` парсит magic + ckpt_v + postcard-заголовок и
        // возвращает cursor ДАЖЕ у непригодного к использованию файла. Если файл —
        // мусор (не парсится даже до заголовка) → None, и gap-детекту в None-ветке
        // действительно нечего детектировать (там корректно «тихий rebuild»).
        let header_only_cursor: Option<Cursor> = if existing.is_none() {
            let path = ckpt_path_for(ckpt_dir, sel);
            if path.exists() {
                read_checkpoint_header(&path).map(|h| h.cursor)
            } else {
                None
            }
        } else {
            None
        };

        // (2) Два сценария:
        //   A. Чекпоинт ВАЛИДЕН → резюм от `ckpt_cursor`.
        //      Докорм только `seq > ckpt_cursor.upto_seq` (GW-I-11 сегментный skip).
        //      Дополнительно проверяем правило немонотонности (п.2):
        //      `ckpt_cursor.upto_seq <= upto.upto_seq` (иначе Err).
        //      Дополнительно (M-48, GW-I-12 суженный fail-loud): если самый ранний
        //      доступный seq журнала `> ckpt_cursor.upto_seq + 1` — между ними
        //      ОБЯЗАН быть разрыв (события спрунены и не свёрнуты ни в чекпоинт,
        //      ни в журнал). Докорм «поверх дырки» дал бы состояние, которое
        //      не соответствует ни одной реальной истории. → `Err` И ничего не
        //      пишем (C-032 R3: сравнение байтов ckpt-каталога до/после).
        //      Стык `earliest == ckpt_cursor.upto_seq + 1` — штатный, не разрыв
        //      (законный prune покрытого префикса; `red_checkpoint_bootstrap_truncated
        //      ::contiguous_boundary_is_not_a_gap` давит off-by-one здесь).
        //   B. Чекпоинта НЕТ → bootstrap ОТ START.
        //      Раньше здесь стоял безусловный fail-loud (`first_seq > 0` → Err):
        //      прод-форма TD-048 (purge M-36 необратимо удалил segment-00000000,
        //      condition истинно НАВСЕГДА ⇒ чекпоинт не поднимается никогда).
        //      M-48: «all-time» ≡ «от самого раннего seq, доступного под данным
        //      EpochFilter» — bootstrap на усечённом журнале ЛЕГАЛЕН. Провенанс
        //      (`history_start_seq`/`history_truncated`) берётся из ПЕРВОГО
        //      реально свёрнутого события и пишется в `CkptHeader` ниже.
        let (mut reducer, base_cursor, history_start_seq, history_truncated): (
            Reducer,
            Cursor,
            u64,
            bool,
        ) = match existing {
            Some((r, ckpt_cursor, ckpt_header)) => {
                // Монотонность (C-030 rev3 D2, пин по двум каналам в C-034 R2):
                // возврат Err И байты ckpt-каталога не меняются.
                if let (Some(c), Some(u)) = (ckpt_cursor.upto_seq, upto.upto_seq) {
                    if c > u {
                        return Err(io::Error::other(format!(
                            "checkpoint::advance_to: чекпоинт уже покрывает seq={c}, \
                             запрошено сжатие к seq={u} — регрессия покрытия \
                             запрещена (правило немонотонности §(1b).2)"
                        )));
                    }
                }
                // M-48 (GW-I-12 суженный fail-loud): разрыв «чекпоинт↔журнал».
                // Семантика: стык `earliest == ckpt_cursor.upto_seq + 1` — НЕ
                // разрыв (законный prune покрытого префикса). Всё, что `> ckpt+1`
                // — разрыв, и докорм недопустим (иначе мы перезапишем чекпоинт
                // состоянием, не соответствующим ни одной реальной истории).
                // `first_visible_seq` смотрит ТОЛЬКО на `header.first_seq`, который
                // у legacy синтезирован нулём (TD-030) — это НЕ ловушка здесь, потому
                // что legacy-сегмент либо (а) пропущен `stream_from` → попадёт в
                // хвост и не повлияет на ckpt_cursor (мы добавляем события строго
                // `> ckpt_cursor.upto_seq`), либо (б) не пропущен (нет `next_seg`)
                // → тогда его `first_seq=0` интерпретируется как «до ckpt_cursor»,
                // что НЕ блокирует (рановато, не разрыв). Реальная проверка —
                // first_seq ПЕРВОГО видимого сегмента с валидным (не legacy) заголовком;
                // `first_visible_seq` для нашего инварианта достаточно: если он
                // ≤ ckpt_cursor, разрыва нет. Сценарий-ловушка — когда legacy —
                // единственный оставшийся сегмент: `first_visible_seq=0` и
                // разрыва не детектируется, но `stream_from` применит ВСЕ его
                // события, в т.ч. `> ckpt_cursor`, и чекпоинт будет валидно
                // расширен — это ШТАТНЫЙ случай «бутстрап на legacy-only журнале».
                if let Some(first) = first_visible_seq(dir, &filter).ok().flatten() {
                    if first > 0 && first.saturating_sub(1) > ckpt_cursor.upto_seq.unwrap_or(0) {
                        // first > ckpt + 1 ⟹ разрыв.
                        return Err(io::Error::other(format!(
                            "checkpoint::advance_to: разрыв между чекпоинтом \
                             (cursor={}) и журналом (earliest first_seq={}): события \
                             между ними спрунены и не свёрнуты ни во что. Докорм \
                             поверх дырки дал бы состояние, не соответствующее ни одной \
                             реальной истории. Безопасно только если \
                             earliest == cursor + 1 (стык после законного prune \
                             покрытого префикса) — см. GW-I-12. Ничего не пишем; \
                             требуется ручное вмешательство (cold storage + rebuild \
                             по нему или подтверждение, что префикс действительно \
                             покрыт курсором чекпоинта).",
                            ckpt_cursor
                                .upto_seq
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "START".to_string()),
                            first
                        )));
                    }
                }
                // M-48 (VB-I-11): провенанс истории ПЕРЕСИМ от старого чекпоинта
                // (D2 из C-030 rev3: `advance_after_covered_prune_does_not_regress_history_start`
                // — после законного prune покрытого префикса чекпоинт по-прежнему
                // заявляет историю с seq 0). Новые хвостовые события (>ckpt_cursor)
                // НЕ смещают `history_start_seq` вправо.
                (
                    r,
                    ckpt_cursor,
                    ckpt_header.history_start_seq,
                    ckpt_header.history_truncated,
                )
            }
            None => {
                // M-48 (B2, task #9): stale чекпоинт-файл — КЭШ (GW-I-9б), не ошибка.
                // Любая невалидность (lineage не сошёлся после truncate, CRC не совпал,
                // чужая версия схемы, мусор) → ТИХИЙ rebuild и ПЕРЕЗАПИСЬ файла.
                // Прежняя ветка «`Err` с предложением `rm ckpt-*.bin`» была
                // ВОСПРОИЗВОДСТВОМ TD-048 на другом входе: на проде `first_visible_seq > 0`
                // НАВСЕГДА (purge M-36 необратим), поэтому после любого будущего
                // бампа схемы (v5/v6/v7/v8 — бампы рутинны) чекпоинтер не поднимался
                // бы до ручного вмешательства, а фича оставалась инертной с
                // зелёными гейтами. Теперь этого класса не существует.
                //
                // Gap-детект в None-ветке (B1, task #8): если заголовок stale-файла
                // парсится (см. `read_checkpoint_header`, B1) — у нас ЕСТЬ `ckpt_cursor`
                // для проверки разрыва. Семантика та же, что в Some-ветке: стык
                // `earliest == ckpt_cursor + 1` — НЕ разрыв, всё `> ckpt_cursor + 1` —
                // разрыв, и докорм поверх дырки дал бы состояние, не соответствующее
                // ни одной реальной истории. → `Err` И ничего не пишем (C-032 R3:
                // байты ckpt-каталога не меняются). Это та единственная защита,
                // которая остаётся после #9 (silent rebuild для stale).
                //
                // Анти-плацебо (B1 review): спец-проверка в None-ветке ОБЯЗАНА быть
                // load-bearing. До #9 эта ветка удовлетворялась старой «stale → Err»
                // веткой и gap-детект был МЁРТВЫМ КОДОМ; после #9 без header-only
                // cursor-а нейтрализация спец-проверки в None-ветке НЕ ломала бы
                // оракул `gap_between_checkpoint_and_journal_is_loud` (фикстура:
                // valid ckpt + truncate deep → existing=None из-за lineage, и без
                // header_only_cursor gap-детект в None-ветке тоже не находит курсор).
                // После #8+9 нейтрализация спец-проверки в None-ветке ЛОМАЕТ оракул.
                if let Some(ckpt_cursor) = header_only_cursor {
                    if let Some(first) = first_visible_seq(dir, &filter).ok().flatten() {
                        if first > 0 && first.saturating_sub(1) > ckpt_cursor.upto_seq.unwrap_or(0)
                        {
                            return Err(io::Error::other(format!(
                                "checkpoint::advance_to: разрыв между чекпоинтом \
                                 (cursor={}) и журналом (earliest first_seq={}): события \
                                 между ними спрунены и не свёрнуты ни во что. Докорм \
                                 поверх дырки дал бы состояние, не соответствующее ни одной \
                                 реальной истории. Безопасно только если \
                                 earliest == cursor + 1 (стык после законного prune \
                                 покрытого префикса) — см. GW-I-12. Ничего не пишем; \
                                 требуется ручное вмешательство (cold storage + rebuild \
                                 по нему или подтверждение, что префикс действительно \
                                 покрыт курсором чекпоинта).",
                                ckpt_cursor
                                    .upto_seq
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| "START".to_string()),
                                first
                            )));
                        }
                    }
                }
                (Reducer::new(sel), Cursor::START, 0_u64, false)
            }
        };

        // (3) Докорм хвостом: только `seq > base_cursor.upto_seq` (A) или всё (B).
        // Используем `stream_from` (GW-I-11 сегментный skip — НЕ читаем уже
        // покрытый префикс).
        let mut final_cursor = base_cursor;
        let mut first_folded_seq: u64 = history_start_seq; // None-ветка: остаётся 0; Some: уже выставлен
        let mut consumed = 0_usize;
        let mut stream = journal::stream_from(dir, filter.clone(), base_cursor.upto_seq)?;
        for event in &mut stream {
            let event = event?;
            if !upto.includes(event.seq) {
                break;
            }
            reducer.apply(&event);
            consumed += 1;
            // M-48 (VB-I-11): первый `apply` в None-ветке — это первое реально
            // свёрнутое событие журнала. `base_cursor == START` гарантирует, что
            // его seq — самый ранний доступный (`first_visible_seq`, корректный
            // даже при legacy — см. `journal::stream_from` / TD-030 защиту).
            // В Some-ветке `first_folded_seq` уже равен `history_start_seq` из
            // старого чекпоинта (D2 сохраняем). Детектируем «первый `apply`» по
            // счётчику `consumed == 1` (НЕ по значению 0 — это амбигуозно: 0
            // корректное значение seq на полном журнале, см.
            // `advance_after_covered_prune_does_not_regress_history_start`).
            if base_cursor == Cursor::START && consumed == 1 {
                first_folded_seq = event.seq;
            }
            final_cursor = Cursor::at(event.seq);
        }
        let (history_start_seq, history_truncated) = if base_cursor == Cursor::START {
            // None-ветка: провенанс из первого свёрнутого события.
            (first_folded_seq, first_folded_seq > 0)
        } else {
            // Some-ветка: провенанс УЖЕ из старого чекпоинта, не меняем.
            (history_start_seq, history_truncated)
        };

        // (4) Lineage: собираем заголовки ТЕКУЩИХ видимых сегментов. Раньше мы
        // делали то же — этот блок поведенчески не изменился.
        let all_segs = journal::list_segments(dir)?;
        let mut lineage: Vec<SegmentHeader> = Vec::with_capacity(all_segs.len());
        for s in &all_segs {
            if filter.accepts(&s.header) {
                lineage.push(s.header.clone());
            }
        }
        lineage.sort_by_key(|h| h.first_seq);

        // (5) Сформировать заголовок + сериализованное состояние.
        let header = CkptHeader::new(
            selector_fingerprint(sel),
            epoch_filter_fingerprint(&filter),
            lineage,
            final_cursor,
            history_start_seq,
            history_truncated,
        );
        let state_bytes = postcard::to_stdvec(&reducer).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("postcard state: {e}"))
        })?;
        let header_bytes = postcard::to_stdvec(&header).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("postcard header: {e}"))
        })?;

        // (6) Atomic write: tmp + rename. Имя tmp УНИКАЛЬНО (pid + nanos),
        // чтобы при перекрытии cron-прогонов (RN-22) НЕ было «оба пишут в один файл»
        // — даже если flock пропущен (защита в глубину).
        let final_path = ckpt_path_for(ckpt_dir, sel);
        let tmp_path = unique_tmp_path(&final_path);
        {
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
        }
        // rename tmp → final. Если `final_path` уже существует — `rename`
        // атомарно заменит (POSIX гарантирует атомарность для одной ФС).
        fs::rename(&tmp_path, &final_path)?;
        Ok(final_cursor)
    }

    /// Прочитать чекпоинт из каталога, валидировать header/CRC/lineage.
    /// Возвращает `Some((reducer, cursor))` если валиден, `None` если отсутствует/битый
    /// (silent rebuild).
    ///
    /// RN-23: имя файла ДЕТЕРМИНИРОВАНО от `selector_fingerprint` (`ckpt-<fp>.bin`).
    /// НИКАКОГО «первый файл рекурсивно» — алфавитно-первый подхватил бы `*.tmp`
    /// (полу-записанный) или чужой чекпоинт при multi-selector deployment, оба давали
    /// тихий rebuild 409 s без сигнала. Если файл отсутствует — `None`, штатный
    /// silent rebuild downstream'ом.
    pub(super) fn read_checkpoint(
        dir: &Path,
        ckpt_dir: &Path,
        sel: &Selector,
        filter: EpochFilter,
    ) -> io::Result<Option<(Reducer, Cursor, CkptHeader)>> {
        let ckpt_path = ckpt_path_for(ckpt_dir, sel);
        if !ckpt_path.exists() {
            return Ok(None); // отсутствует — silent rebuild; никакой рекурсии
        }
        let bytes = match fs::read(&ckpt_path) {
            Ok(b) => b,
            Err(_) => return Ok(None), // silent rebuild
        };
        Ok(read_and_validate(&bytes, dir, sel, filter))
    }

    /// M-48 (task #8, B1 reviewer): прочитать ТОЛЬКО заголовок чекпоинта из файла.
    /// Парсит magic + `ckpt_schema_version` + postcard-заголовок (offsets 0..20+header_len).
    /// Возвращает `Some(CkptHeader)` ДАЖЕ если состояние непригодно к использованию —
    /// другая версия `gateway_schema_version`, чужой фингерпринт, несовпавший lineage,
    /// CRC не совпал, state не десериализуется. Заголовок самодостаточен и хранит
    /// `cursor` + `history_start_seq`, которые нужны gap-детекту (#3) даже когда
    /// чекпоинт признан непригодным.
    ///
    /// **Зачем отдельная функция (B1).** До M-48 `decode_checkpoint`/`read_and_validate`
    /// возвращали `None` при ЛЮБОМ расхождении (включая устаревшую версию схемы). Это
    /// означало, что gap-детект задачи #3 физически недостижим на фикстуре с усечением
    /// глубже курсора: `read_checkpoint` отдавал `None`, `ckpt_cursor` терялся,
    /// условие «`earliest > ckpt.cursor + 1`» не выполнялось ни разу. Reviewer это
    /// подтвердил, нейтрализовав спец-проверку — тест оставался зелёным за счёт ЧУЖОЙ
    /// ветки («stale чекпоинт → Err», закрыто в задаче #9).
    ///
    /// Без `dir`, без `sel`, без `filter` — заголовок не зависит от runtime-окружения.
    ///
    /// НЕ проверяет (намеренно): `selector_fingerprint`, `epoch_filter_fingerprint`,
    /// `journal_lineage` (требуют `dir`), CRC (покрывает `header || state` — без
    /// state CRC не вычислить), state_postcard decode. Это работа `read_checkpoint`.
    ///
    /// Возвращает `None` ТОЛЬКО если файл структурно нечитаем: меньше magic, нет
    /// magic, несовпадение `ckpt_schema_version` (формат файла, несовместим с текущим
    /// парсером), `postcard::from_bytes` заголовка упал. Это уже класс «на диске мусор,
    /// а не чекпоинт» — там gap-детекту действительно нечего детектировать, корректное
    /// поведение «тихий rebuild без gap-проверки» (см. `advance_to` None-ветку).
    ///
    /// NB: устаревший `gateway_schema_version` (`bytes[12..16]` НЕ равен текущему
    /// `GATEWAY_SCHEMA_VERSION`) — это **именно тот кейс, ради которого функция
    /// написана**. Не отвергаем; это часть GW-I-9б «файл с прошлой версией схемы —
    /// КЭШ, не ошибка». Свежий bump схемы на проде (v5/v6/v7/v8 — рутинны) не должен
    /// выводить чекпоинт из строя.
    pub(super) fn read_checkpoint_header(path: &Path) -> Option<CkptHeader> {
        let bytes = fs::read(path).ok()?;
        if bytes.len() < 8 + 4 + 4 + 4 {
            return None;
        }
        if bytes[0..8] != CKPT_MAGIC {
            return None;
        }
        let ckpt_v = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        if ckpt_v != CKPT_SCHEMA_VERSION {
            return None;
        }
        // bytes[12..16] — `gateway_schema_version` СПЕЦИАЛЬНО НЕ сверяем с текущей
        // (см. шапку функции). Stale-файл от прежней версии — основной use-case.
        let _gw_v = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
        let header_len = u32::from_le_bytes(bytes[16..20].try_into().ok()?) as usize;
        let header_end = 20 + header_len;
        if bytes.len() < header_end {
            return None;
        }
        postcard::from_bytes(&bytes[20..header_end]).ok()
    }

    fn read_and_validate(
        bytes: &[u8],
        dir: &Path,
        sel: &Selector,
        filter: EpochFilter,
    ) -> Option<(Reducer, Cursor, CkptHeader)> {
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
        Some((reducer, header.cursor, header))
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
// M-38b (GW-I-11) / M-53 (TD-083 rev2): РЕЗЮМИРУЕМЫЙ LIVE-REDUCER
// ════════════════════════════════════════════════════════════════════════════
//
// Без этого `frames_since` досеивает состояние реплеем всего журнала на КАЖДОМ
// live-тике (250 мс, ~400 с на проде) — live-push математически не сходится. Решение:
// состояние живёт МЕЖДУ тиками и докармливается только новыми событиями через
// `journal::stream_from(cursor)` (GW-I-11).
//
// M-53 rev2 (TD-083, после architect'ского замера): первая версия `pump` ДЕЛЕГИРОВАЛА
// построение кадра в `frames_since` («используем frames_since для byte-identity с
// эталоном») — то есть КАЖДЫЙ pump всё равно читал журнал с головы (root cause 1 никуда
// не делся), а `ReadStats` считались с ДРУГОГО (действительно bounded) прохода — оракул
// `pump_at_tail_is_bounded` мерил не ту работу, которая строила кадр.
//
// Настоящее решение: единственное состояние, которое ОБЯЗАНО пережить между тиками, —
// `VwapAcc.sum_pv`/`sum_v` (since-genesis аккумулятор без per-тик сброса, `Reducer::
// apply_vwap`). Всё остальное (ohlcv/cvd/vp/heatmap/book/bubbles) естественно
// «дельта-only» на СВЕЖЕМ `Reducer` за тик — именно так уже ведёт себя `frames_since`
// (её fresh reducer тоже стартует пустым на каждый вызов, `seed_vwap` эти поля не трогает).
// Перенося ТОЛЬКО `sum_pv`/`sum_v` между тиками, получаем ТУ ЖЕ арифметику, что дал бы
// `seed_vwap` над всей историей, — без O(история) прохода по событиям на каждый тик.
// Математическое следствие: кадры `pump` байт-идентичны кадрам `frames_since` НЕ потому,
// что `pump` зовёт `frames_since` (тавтология, которую architect поймал в TD-083), а
// потому, что оба считают ОДНО и то же — проверено И сравнением с `frames_since`
// (`pumped_frames_identical_to_frames_since`), И НЕЗАВИСИМО, полным реплеем
// (`td083_pumped_frames_fold_into_full_replay_snapshot`).

/// Резюмируемый живой редьюсер. Персистентно между `pump`-вызовами живёт ТОЛЬКО
/// vwap-аккумулятор (`sum_pv`/`sum_v`, since-genesis, GW-I-11/TD-083) — это то, что
/// докармливает КАЖДЫЙ следующий batch-кадр. Каждый `pump` строит кадр на СВЕЖЕМ
/// `Reducer` (как `frames_since`) — стоимость пропорциональна ПРИРАЩЕНИЮ
/// (`journal::stream_from(cursor)`, сегментный пропуск), а не длине журнала.
pub struct LiveReducer {
    /// since-genesis `Σ(price·size)`/`Σ(size)` — единственное состояние без per-тик
    /// сброса. `values`-карта НЕ переносится между тиками (остаётся пустой на старте
    /// каждого `pump`, как у свежего `Reducer` внутри `frames_since`) — иначе кадр нёс бы
    /// лишние (хоть и корректные) точки прошлых тиков, отличные по составу от эталона.
    vwap: VwapAcc,
    /// Курсор последнего свёрнутого события (на него `stream_from` подаёт `after_seq`).
    cursor: Cursor,
    /// Селектор (для построения кадров `frames_since`-стиля в `pump` без sel-параметра).
    selector: Selector,
    /// M-54 (`TD-093(б)`): полное накопленное состояние, источник `snapshot()`.
    /// Роль ОТДЕЛЬНАЯ от batch-`Reducer'а в `pump()` (тот остаётся СВЕЖИМ на каждый
    /// batch — не трогается, sacred `red_frames_seek_bound.rs` требует именно такой
    /// семантики для кадров wire-протокола). `full` — ПЕРСИСТЕНТНЫЙ `Reducer`, который
    /// `pump()` кормит КАЖДЫМ событием напрямую (`full.apply(event)`), в ТОМ ЖЕ порядке
    /// и с ТОЙ ЖЕ per-event оконной эвикцией (`evict_window_state`), какую делает
    /// `snapshot_from_checkpoint`'s хвостовой цикл — то есть побайтово та же арифметика,
    /// что и независимый реплей, только растянутая по нескольким `pump()`-вызовам вместо
    /// одного прохода. Смежный вариант («мёржить уже посчитанные Frame-дельты через
    /// `Snapshot::apply` раз в batch») ОТБРОШЕН: эвикция окна там срабатывала бы раз на
    /// ~`max_events` (batch), а не раз на событие — на несимметричном по времени хвосте
    /// (в частности, при событии с более ранним `ts_exch_ms`, чем у соседей — не
    /// придуманный случай, реальный urgent-фикс задним числом) это оставляет лишние
    /// записи, не эвиктнутые вовремя (эмпирически поймано оракулом O-2 на кандидате).
    full: Reducer,
    /// Провенанс истории `full` — берётся ОДИН раз из чекпоинта в `resume()` (или из
    /// первого свёрнутого события на no-checkpoint пути) и не меняется `pump()`: хвост
    /// не может расширить или сузить то, что уже спрунено ДО чекпоинта.
    full_history_start_seq: u64,
    full_history_truncated: bool,
    /// M-57 (круг 2, TD-109): курсор хвоста активного сегмента в ПАМЯТИ сессии —
    /// `seg_idx`/`last_seq`/`pos`. Передаётся в `journal::stream_from_at` при каждом
    /// `pump()` и обновляется по `EventStream::tail_hint()` после прохода. Решает
    /// обе находки PR-гейта `R-035`: (`F-035-1`) hint не зависит от записываемой
    /// поверхности каталога, (`F-035-2`) hint — per-session, а не per-catalog.
    /// `None` — первый `pump()` или после валидационного отката (ротация / запрошены
    /// события из уже прочитанной зоны).
    tail_hint: Option<journal::TailHint>,
}

impl LiveReducer {
    /// Резюмировать состояние: если чекпоинт валиден — забрать vwap-аккумулятор ИЗ
    /// восстановленного `Reducer` (`checkpoint::advance_to` уже накопил его честным
    /// `apply`-проходом от START при построении чекпоинта, O(1) здесь); иначе — единственный
    /// ОЖИДАЕМЫЙ полный реплей (`resume_without_checkpoint_reports_full_replay`), нужный
    /// ровно один раз при подключении клиента, не на каждый последующий тик.
    pub fn resume(
        dir: impl AsRef<Path>,
        filter: EpochFilter,
        sel: &Selector,
        ckpt_dir: impl AsRef<Path>,
    ) -> io::Result<(Self, ReadStats)> {
        validate_selector(sel)?;
        let dir = dir.as_ref();
        let ckpt_dir = ckpt_dir.as_ref();

        if let Some((r, cursor, header)) =
            checkpoint::read_checkpoint(dir, ckpt_dir, sel, filter.clone())?
        {
            // Чекпоинт валиден: `r.vwap.sum_pv`/`sum_v` — уже честная since-genesis сумма
            // (advance_to накопил её реальным `Reducer::apply` от START). `values` чекпоинта
            // — окно прошлых эмитов, НЕ переносим (см. doc-комментарий `LiveReducer`).
            let sum_pv = r.vwap.sum_pv;
            let sum_v = r.vwap.sum_v;
            // M-54: `full` — САМ восстановленный `Reducer` (уже полное состояние всех
            // серий на `cursor`, не только vwap) — O(1) здесь, второго чтения журнала
            // нет. Хвост докормит его `pump()` тем же `apply()`, каким `advance_to`
            // построил его исходно.
            let full = r;
            return Ok((
                Self {
                    vwap: VwapAcc {
                        sum_pv,
                        sum_v,
                        values: BTreeMap::new(),
                    },
                    cursor,
                    selector: sel.clone(),
                    full,
                    full_history_start_seq: header.history_start_seq,
                    full_history_truncated: header.history_truncated,
                    // M-57 (TD-109): `cursor` уже на хвосте — hint обязан быть
                    // заполнен ПЕРВЫМ ЖЕ pump'ом через `EventStream::tail_hint()`.
                    // Прямо здесь его вычислить нельзя: `EventStream` ещё не построен.
                    tail_hint: None,
                },
                ReadStats::default(),
            ));
        }

        // Без чекпоинта: `cursor` остаётся `START` — аккумулятор ОБЯЗАН остаться нулевым,
        // а не засеянным: первый же `pump()` естественно построит его (и все остальные
        // акумуляторы) через свой обычный `apply()`-проход С НУЛЯ, начиная от START (та же
        // логика, что у `frames_since(after == START)`). Предварительный seed здесь задвоил
        // бы сумму — `pump()` применил бы те же события ЕЩЁ РАЗ поверх уже засеянных.
        //
        // Проход по журналу здесь нужен ТОЛЬКО чтобы `ReadStats` честно отразил реальную
        // цену catch-up (форсинг `resume_without_checkpoint_reports_full_replay`: счётчик,
        // который всегда мал, обесценивает оракулы бюджета) — декодируем и отбрасываем.
        // M-54 (TD-093(б)): ОДНО поле НЕ отбрасываем — seq первого ВИДИМОГО события
        // (провенанс истории, VB-I-11). Без чекпоинта `resume()` — единственное место,
        // где `snapshot()` мог бы узнать, что журналу спрунен префикс: жёсткий
        // `0`/`false` соврал бы о честности (регрессия поймана `o6_pruned_journal_
        // is_honestly_marked`, `crates/gateway-serve/tests/red_ws_honesty_sessions.rs` —
        // сценарий БЕЗ чекпоинта на журнале с удалённым первым сегментом).
        let mut stream = journal::stream(dir, filter)?;
        let mut first_seq: Option<u64> = None;
        for event in &mut stream {
            let event = event?;
            if first_seq.is_none() {
                first_seq = Some(event.seq);
            }
        }
        let stats = read_stats_from_stream(&stream);
        let history_start_seq = first_seq.unwrap_or(0);
        Ok((
            Self {
                vwap: VwapAcc::default(),
                cursor: Cursor::START,
                selector: sel.clone(),
                // M-54: без чекпоинта ничего ещё не свёрнуто в `full` (курсор START,
                // ничего не эвиктнуто) — согласовано с `cursor: Cursor::START` выше;
                // sacred `red_frames_seek_bound.rs` требует, чтобы РАБОТУ по наполнению
                // сделал последующий `pump()`, не `resume()` (см. doc-комментарий
                // `LiveReducer`, rev2 M-54 оракулов).
                full: Reducer::new(sel),
                full_history_start_seq: history_start_seq,
                full_history_truncated: history_start_seq > 0,
                // M-57 (TD-109): первый `pump()` заполнит hint из `EventStream::tail_hint()`.
                tail_hint: None,
            },
            stats,
        ))
    }

    /// Докачать новые события от текущего курсора, вернуть кадры (`Vec<Frame>`) + новый
    /// курсор + `ReadStats`.
    ///
    /// M-53 (TD-083 rev2): кадр строится ИЗ СВОЕГО состояния — `frames_since` здесь НЕ
    /// вызывается. Единственный проход: `journal::stream_from(self.cursor)` (сегментный
    /// пропуск, GW-I-11) на СВЕЖИЙ `Reducer`, чей vwap-аккумулятор засеян ИЗ персистентного
    /// `self.vwap` (O(1), без re-seed по истории). `ReadStats` этого ЕДИНСТВЕННОГО прохода —
    /// честная мера ВСЕЙ работы тика (task 2b: раньше `stats` брались с другого,
    /// не отражающего реальную стоимость построения кадра, прохода).
    ///
    /// События `seq <= self.cursor`, попавшие в стрим из-за сегментной гранулярности
    /// (сегмент, содержащий курсор, `stream_from` отдаёт целиком, не разрезая на событии),
    /// пропускаются: они уже учтены в `self.vwap` предыдущим тиком — повторное применение
    /// задвоило бы аккумулятор.
    pub fn pump(
        &mut self,
        dir: impl AsRef<Path>,
        filter: EpochFilter,
        max_events: usize,
    ) -> io::Result<(Vec<Frame>, Cursor, ReadStats)> {
        let dir = dir.as_ref();

        // Один тик обязан ДРЕНИРОВАТЬ ВЕСЬ доступный на момент вызова backlog (а не только
        // первые `max_events`) — иначе `pump` возвращал бы РОВНО ОДИН кадр за вызов, и
        // «докачать всё» потребовалось бы СТОЛЬКО вызовов, сколько `backlog/max_events`, что
        // ломает композицию «resume() без чекпоинта → N/max_events тиков» (TD-083 O-A:
        // `resume` без чекпоинта стартует с backlog = ВЕСЬ журнал, и первый же вызов обязан
        // покрыть его целиком, разбив на batch'и по `max_events`). `max_events` ограничивает
        // размер КАЖДОГО кадра (bounded-memory одного batch'а), не число кадров за вызов.
        //
        // M-57 (круг 2, TD-109): используем `stream_from_at` с in-memory hint'ом, а не
        // `stream_from`. `stream_from` читает hint из файлового sidecar'а `journal.tail-offset`
        // ВНУТРИ каталога журнала — на проде (gateway-serve, том `:ro`) этот файл
        // НИКОГДА не появляется, и `stream_from` всегда делает full scan (`F-035-1`).
        // In-memory hint живёт в `self.tail_hint` (`None` на первом тике), обновляется
        // после каждого прохода через `EventStream::tail_hint()` и естественно per-session
        // (`F-035-2`).
        let mut stream =
            journal::stream_from_at(dir, filter, self.cursor.upto_seq, self.tail_hint)?;
        let mut frames: Vec<Frame> = Vec::new();
        let mut cursor = self.cursor;
        let mut batch_from = self.cursor;
        let mut batch = Reducer::new(&self.selector);
        batch.vwap.sum_pv = self.vwap.sum_pv;
        batch.vwap.sum_v = self.vwap.sum_v;
        let mut batch_consumed = 0_usize;

        for event in &mut stream {
            let event = event?;
            if self.cursor.upto_seq.is_some_and(|seq| event.seq <= seq) {
                continue; // уже учтено предыдущим тиком (сегментная гранулярность стрима)
            }
            if max_events == 0 {
                break;
            }
            if batch_consumed == max_events {
                // Закрыть текущий batch кадром; начать следующий СВЕЖИМ reducer'ом, засеянным
                // ИЗ persistent-аккумулятора (тот уже обновлён последним apply ниже) — та же
                // арифметика, что дал бы независимый `frames_since`-вызов на этой границе.
                let (delta, at_ms) = batch.finish_with_at();
                frames.push(Frame::versioned(batch_from, cursor, delta, at_ms));
                batch_from = cursor;
                batch = Reducer::new(&self.selector);
                batch.vwap.sum_pv = self.vwap.sum_pv;
                batch.vwap.sum_v = self.vwap.sum_v;
                batch_consumed = 0;
            }
            batch.apply(&event);
            // M-54: `full` кормится КАЖДЫМ событием НАПРЯМУЮ, в том же порядке, что и
            // `batch` — та же per-event оконная эвикция (`Reducer::apply` →
            // `evict_window_state`), что у независимого реплея. Никакого чтения журнала:
            // событие уже декодировано этой же итерацией `stream` (та единственная
            // работа, которую меряет `ReadStats`/O-1) — здесь только CPU-применение
            // ко ВТОРОМУ in-memory аккумулятору, не второй проход по диску.
            self.full.apply(&event);
            cursor = Cursor::at(event.seq);
            batch_consumed += 1;
            // Персистентный аккумулятор обновляется СРАЗУ (не только в конце вызова) — так
            // следующий batch внутри ЭТОГО ЖЕ pump() стартует с корректной суммой.
            self.vwap.sum_pv = batch.vwap.sum_pv;
            self.vwap.sum_v = batch.vwap.sum_v;
        }
        let stats = read_stats_from_stream(&stream);
        // M-57 (TD-109): забираем hint СВОЕЙ сессии из стрима. Если за тик НЕ было
        // ни одного декодированного события в активном сегменте (backlog пуст,
        // `finished` сразу), `tail_hint()` вернёт `None` — корректно, следующий
        // тик либо найдёт новые события и заполнит hint, либо увидит ротацию
        // (старый hint не подойдёт по seg_idx — откат к full scan + новый hint).
        // M-57 (TD-109): забираем hint СВОЕЙ сессии из стрима. Если за тик НЕ было
        // ни одного декодированного события в активном сегменте (backlog пуст,
        // `stream.tail_hint()` вернёт `None`), НЕ сбрасываем старый hint — он всё ещё
        // указывает на корректную байтовую позицию активного сегмента и валиден,
        // пока активный сегмент тот же (т.е. не было ротации). При ротации
        // `resolve_active_start_offset` отбросит старый hint (несовпадение `seg_idx`)
        // и сделает full scan + новый hint из обновлённого стрима — естественно.
        self.tail_hint = stream.tail_hint().or(self.tail_hint);

        if batch_consumed > 0 {
            let (delta, at_ms) = batch.finish_with_at();
            frames.push(Frame::versioned(batch_from, cursor, delta, at_ms));
        }

        self.cursor = cursor;
        Ok((frames, cursor, stats))
    }

    /// Текущий курсор (последний свёрнутый seq, либо `Cursor::START` если ни одного).
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// M-54 (`TD-093(б)`, task #1): отдать ТЕКУЩЕЕ накопленное состояние как `Snapshot`,
    /// БЕЗ чтения журнала. Сигнатура строго `(&self) -> Snapshot` — ни `dir`, ни `filter`:
    /// у метода физически нет доступа к журналу, поэтому второй проход по хвосту
    /// невозможен по построению (тот же типовой приём, что `RK-I-1`: барьер держит
    /// компилятор, не соглашение). `full` наполняется `resume()` (чекпоинт-ветка, O(1) —
    /// уже восстановленный `Reducer`) и каждым `pump()` (`full.apply(event)` per-event) —
    /// здесь только `finish_ref_with_at()` (M-56, `TD-097`) — свёртка накопленного состояния
    /// в `SeriesBundle`, БЕЗ обращения к диску И без клонирования `self.full` (клон целого
    /// редьюсера — включая книгу целиком, ≈20 MiB на проде — дал +404 ms константы на каждом
    /// подключении при ЛЮБОМ backlog'е, R-029 §C / TD-097). `finish_ref_with_at(&self)` строит
    /// серии из ссылок на персистентный `self.full`, не потребляя и не мутируя его.
    pub fn snapshot(&self) -> Snapshot {
        let (series, _at_ms) = self.full.finish_ref_with_at();
        Snapshot {
            schema_version: GATEWAY_SCHEMA_VERSION,
            selector: self.selector.clone(),
            cursor: self.cursor,
            series,
            history_start_seq: self.full_history_start_seq,
            history_truncated: self.full_history_truncated,
        }
    }
}
