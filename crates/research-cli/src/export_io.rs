//! Read-only wiring: journal stream → order-flow reducers → JSON-экспорт (M-17 OF-I-2/3/4/6).
//!
//! Это **ЭКСПОРТ-МЕХАНИЗМ**, а НЕ рантайм-путь. Никаких writer-API журнала (открытие на
//! запись/append) — read-only `journal::stream` (CT-RFC02-2, EpochFilter обязателен).
//! Структурный тест `tests/structural.rs::test_no_journal_write_path` это зафиксирует
//! (RC-I-7).
//!
//! Контракт:
//!   - L2Snapshot из журнала → `OrderBook::apply_snapshot` → `depth_series::compute` per
//!     (side, band) → `Vec<(bucket_s, depth)>` per (venue, symbol, side, band);
//!   - Trade из журнала → `orderflow::footprint_delta` / `cumulative_delta` / `footprint_bins`
//!     и `export::ohlcv_bars` per (venue, symbol, timeframe_s);
//!   - запись в `<out_dir>/<venue>/<symbol>/<kind>.json` со стабильным JSON-форматом под
//!     `code2alpha` (см. `research/exports/format.md` — экспорт-контракт).
//!
//! Детерминизм (RC-I-5): ЭКСПОРТ сериализуется через `serde_json` с ОДНИМ И ТЕМ ЖЕ
//! представлением (BTreeMap-keyed reducer → порядок стабилен); никаких wall-clock полей
//! в payload (только `ts_wall_ms` из события).

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use book::OrderBook;
use contracts::{Event, EventKind, MdPayload, Side, Venue};
use journal::EpochFilter;
use serde::Serialize;

use crate::depth_series;
use crate::export;
use crate::grid::JournalSource;
use crate::orderflow;

/// Версия экспорт-контракта (согласована с `research/exports/format.md`).
/// Инкремент = breaking change формы данных.
pub const EXPORT_SCHEMA_VERSION: u32 = 1;

/// Конфиг экспорт-выборки: какие полосы/таймфреймы считать. Дефолты — из OF-I-6/2/3/4.
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Таймфрейм для footprint/cumulative/ohlcv-бар (мс). Дефолт 1000 (1s база).
    pub timeframe_ms: i64,
    /// Полосы для depth-series (доля от mid). Дефолт: 0.001, 0.003, 0.005.
    pub depth_bands_pct: Vec<f64>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            timeframe_ms: 1_000,
            depth_bands_pct: vec![0.001, 0.003, 0.005],
        }
    }
}

/// Артефакт экспорта. Записывается на диск рядом с `format.md` под `<out_dir>`.
#[derive(Debug, Clone, Serialize)]
pub struct ExportArtifact {
    pub schema_version: u32,
    /// venue + symbol — для дедупликации на стороне фронта.
    pub venue: String,
    pub symbol: String,
    pub timeframe_ms: i64,
    /// Снимок epoch_id, попавших в выборку (для traceability).
    pub epoch_ids: Vec<String>,
    /// Границы окна в мс (wall-clock по `ts_wall_ms` событий).
    pub first_wall_ms: i64,
    pub last_wall_ms: i64,
    /// OHLCV-свечи (1s база по дефолту).
    pub ohlcv: Vec<OhlcvRow>,
    /// Footprint-дельта per бакет (скаляр).
    pub footprint_delta: Vec<DeltaRow>,
    /// Cumulative delta (running) per бакет.
    pub cumulative_delta: Vec<DeltaRow>,
    /// Per-ценовой footprint для custom-series.
    pub footprint_bins: Vec<FootprintBarRow>,
    /// Depth time-series per (side, band). Стороны и полосы — РАЗДЕЛЬНЫЕ ключи (BID ≠ ASK).
    pub depth_series: Vec<DepthSeriesRow>,
}

/// Сериализуемая обёртка над `export::OhlcvBar` (i64-поля as-is, fixed-point ×1e8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OhlcvRow {
    pub time_s: i64,
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
    pub volume: i64,
}

impl From<export::OhlcvBar> for OhlcvRow {
    fn from(b: export::OhlcvBar) -> Self {
        Self {
            time_s: b.time_s,
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeltaRow {
    pub time_s: i64,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FootprintBarRow {
    pub time_s: i64,
    pub bins: Vec<PriceBinRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PriceBinRow {
    pub price: i64,
    pub buy_vol: i64,
    pub sell_vol: i64,
    pub delta: i64,
}

impl From<orderflow::FootprintBar> for FootprintBarRow {
    fn from(bar: orderflow::FootprintBar) -> Self {
        Self {
            time_s: bar.time_s,
            bins: bar
                .bins
                .into_iter()
                .map(|b| PriceBinRow {
                    price: b.price,
                    buy_vol: b.buy_vol,
                    sell_vol: b.sell_vol,
                    delta: b.delta,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DepthSeriesRow {
    pub side: String,
    pub band_pct_e8: i64,
    pub series: Vec<DeltaRow>,
}

/// Ошибка экспорта. `RcError::Io` для read-ошибок; `RcError::CorruptInput` для journal-фейлов;
/// `RcError::Parse` для serde.
#[derive(Debug)]
pub enum ExportError {
    Io(io::Error),
    Serde(serde_json::Error),
    /// Таймфрейм или полосы некорректны — отказ до стрима.
    BadConfig(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::Io(e) => write!(f, "io: {e}"),
            ExportError::Serde(e) => write!(f, "serde: {e}"),
            ExportError::BadConfig(m) => write!(f, "bad config: {m}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<io::Error> for ExportError {
    fn from(e: io::Error) -> Self {
        ExportError::Io(e)
    }
}

impl From<serde_json::Error> for ExportError {
    fn from(e: serde_json::Error) -> Self {
        ExportError::Serde(e)
    }
}

/// Прогнать экспорт по журналу (read-only, O(1) памяти на сегмент — `journal::stream`).
///
/// Пишет JSON в `<out_dir>/<venue>/<symbol>.json`. Идемпотентен на тех же входах: одинаковый
/// `journal_dir` + `config` → байт-идентичный вывод (RC-I-5).
///
/// **Важно:** `source.filter` — ОБЯЗАТЕЛЬНО осмысленный (`OwnCaptureOnly` по умолчанию; НЕ
/// `EpochFilter::All` без явного решения). `code2alpha` фронт получает ТОЛЬКО наши captures,
/// vendor/синтетика — НЕ подмешивается молча (CT-RFC02-2/3/4).
pub fn export_to_dir(
    source: &JournalSource,
    cfg: &ExportConfig,
    out_dir: &Path,
) -> Result<Vec<PathBuf>, ExportError> {
    if cfg.timeframe_ms <= 0 {
        return Err(ExportError::BadConfig(format!(
            "timeframe_ms must be > 0, got {}",
            cfg.timeframe_ms
        )));
    }
    for &b in &cfg.depth_bands_pct {
        if b <= 0.0 || !b.is_finite() {
            return Err(ExportError::BadConfig(format!(
                "depth band must be positive finite, got {b}"
            )));
        }
    }

    let stream = journal::stream(&source.dir, source.filter.clone())?;
    let mut epoch_ids: Vec<String> = stream
        .headers()
        .iter()
        .map(|h| h.epoch_id.clone())
        .collect();
    epoch_ids.sort();
    epoch_ids.dedup();

    // Сбор trades/snapshots per (venue, symbol). Два прохода логически, но в коде —
    // один стрим-проход с накоплением в per-instrument буферы.
    //
    // Буферы — `Vec` (стрим монотонен по `ts_wall_ms` в пределах одной вендоры/символа;
    // при наличии разных вендоров порядок — стабильный по сегменту). Дедупликация не нужна:
    // events уникальны по `seq`.
    let mut buckets: HashMap<(Venue, String), InstrumentBucket> = HashMap::new();

    let mut first_wall_ms: i64 = 0;
    let mut last_wall_ms: i64 = 0;
    let mut first_seen = false;

    for ev in stream {
        let event = ev?;
        if !first_seen {
            first_wall_ms = event.ts_wall_ms;
            first_seen = true;
        }
        last_wall_ms = event.ts_wall_ms;

        let EventKind::Md(md) = &event.kind else {
            continue;
        };
        let key = (md.venue, md.symbol.clone());
        let bucket = buckets.entry(key).or_default();

        match &md.payload {
            MdPayload::Trade {
                price,
                size,
                side,
                ts_exch_ms,
            } => {
                // ts_exch_ms — биржевой; для экспорта используем ЕГО (агрегация по биржевым
                // часам — это то, что хочет видеть order-flow; ts_wall_ms — для отчёта).
                bucket.trades.push(TradeRow {
                    ts_ms: *ts_exch_ms,
                    price: *price,
                    size: *size,
                    side: *side,
                });
            }
            MdPayload::L2Snapshot {
                bids,
                asks,
                ts_exch_ms,
            } => {
                let mut book = OrderBook::new();
                book.apply_snapshot(bids, asks);
                bucket.snapshots.push((*ts_exch_ms, book));
            }
            // Прочие payloads (Funding/OI/Liquidation/MarginRate) — order-flow не нужны.
            _ => {}
        }
    }

    // Гарантируем наличие out_dir.
    fs::create_dir_all(out_dir)?;
    let mut written: Vec<PathBuf> = Vec::new();

    // Стабильный порядок записи файлов: сортировка по (venue, symbol) — иначе вывод
    // зависел бы от HashMap-итерации (RC-I-5: байт-идентичный вывод на тех же входах).
    let mut keys: Vec<&(Venue, String)> = buckets.keys().collect();
    keys.sort_by(|a, b| {
        venue_sort_key(&a.0)
            .cmp(venue_sort_key(&b.0))
            .then_with(|| a.1.cmp(&b.1))
    });

    for key in keys {
        let bucket = buckets
            .get(key)
            .expect("ключ из keys() обязан быть в HashMap");
        let (venue, symbol) = (key.0, key.1.as_str());
        let artifact = build_artifact(
            &venue,
            symbol,
            &epoch_ids,
            cfg,
            bucket,
            first_wall_ms,
            last_wall_ms,
        )?;
        let venue_dir = out_dir.join(venue_slug(&venue));
        fs::create_dir_all(&venue_dir)?;
        let path = venue_dir.join(format!("{}.json", symbol_slug(symbol)));
        let json = serde_json::to_string_pretty(&artifact)?;
        fs::write(&path, json)?;
        written.push(path);
    }

    Ok(written)
}

#[derive(Default)]
struct InstrumentBucket {
    trades: Vec<TradeRow>,
    snapshots: Vec<(i64, OrderBook)>,
}

#[derive(Debug, Clone, Copy)]
struct TradeRow {
    ts_ms: i64,
    price: i64,
    size: i64,
    side: Side,
}

fn build_artifact(
    venue: &Venue,
    symbol: &str,
    epoch_ids: &[String],
    cfg: &ExportConfig,
    bucket: &InstrumentBucket,
    first_wall_ms: i64,
    last_wall_ms: i64,
) -> Result<ExportArtifact, ExportError> {
    // OHLCV: trades без `side` (он не нужен для свечей).
    let ohlcv_input: Vec<(i64, i64, i64)> = bucket
        .trades
        .iter()
        .map(|t| (t.ts_ms, t.price, t.size))
        .collect();
    let ohlcv: Vec<OhlcvRow> = export::ohlcv_bars(&ohlcv_input, cfg.timeframe_ms)
        .into_iter()
        .map(OhlcvRow::from)
        .collect();

    // Footprint/cumulative: trades без `price` (дельта per-бар скаляр).
    let fp_input: Vec<(i64, Side, i64)> = bucket
        .trades
        .iter()
        .map(|t| (t.ts_ms, t.side, t.size))
        .collect();
    let footprint_delta: Vec<DeltaRow> = orderflow::footprint_delta(&fp_input, cfg.timeframe_ms)
        .into_iter()
        .map(|(time_s, value)| DeltaRow { time_s, value })
        .collect();
    let cumulative_delta: Vec<DeltaRow> = orderflow::cumulative_delta(&fp_input, cfg.timeframe_ms)
        .into_iter()
        .map(|(time_s, value)| DeltaRow { time_s, value })
        .collect();

    // Footprint bins: с `price` (per-ценовая матрица).
    let bins_input: Vec<(i64, i64, Side, i64)> = bucket
        .trades
        .iter()
        .map(|t| (t.ts_ms, t.price, t.side, t.size))
        .collect();
    let footprint_bins: Vec<FootprintBarRow> =
        orderflow::footprint_bins(&bins_input, cfg.timeframe_ms)
            .into_iter()
            .map(FootprintBarRow::from)
            .collect();

    // Depth series per (side, band).
    let mut depth_series_rows: Vec<DepthSeriesRow> = Vec::new();
    for &band in &cfg.depth_bands_pct {
        for side in [Side::Buy, Side::Sell] {
            let series = depth_series::compute(&bucket.snapshots, side, band, cfg.timeframe_ms);
            depth_series_rows.push(DepthSeriesRow {
                side: side_slug(side).to_string(),
                band_pct_e8: (band * 1e8).round() as i64,
                series: series
                    .into_iter()
                    .map(|(time_s, value)| DeltaRow { time_s, value })
                    .collect(),
            });
        }
    }

    Ok(ExportArtifact {
        schema_version: EXPORT_SCHEMA_VERSION,
        venue: venue_slug(venue).to_string(),
        symbol: symbol.to_string(),
        timeframe_ms: cfg.timeframe_ms,
        epoch_ids: epoch_ids.to_vec(),
        first_wall_ms,
        last_wall_ms,
        ohlcv,
        footprint_delta,
        cumulative_delta,
        footprint_bins,
        depth_series: depth_series_rows,
    })
}

#[inline]
fn side_slug(s: Side) -> &'static str {
    match s {
        Side::Buy => "bid",
        Side::Sell => "ask",
    }
}

#[inline]
fn venue_slug(v: &Venue) -> &'static str {
    match v {
        Venue::Binance => "binance",
        Venue::Hyperliquid => "hyperliquid",
        // Новые варианты (CT-I §6) — аддитивно, ничего не ломаем; fallback через Debug.
        _ => "unknown",
    }
}

/// Стабильный ключ сортировки для Venue (Venue не импл `Ord`). Привязка к slug:
/// детерминированный порядок venue на диске и в JSON-выводе.
#[inline]
fn venue_sort_key(v: &Venue) -> &'static str {
    venue_slug(v)
}

#[inline]
fn symbol_slug(s: &str) -> String {
    // Символы вроде "BTCUSDT" → "BTCUSDT" (без `/` и пр.). На будущее — если появятся
    // спецсимволы, заменим на sanitization; пока стрим гарантирует разумные имена.
    s.to_string()
}

/// Не публичный re-export — но удобный helper для main.rs.
pub use ExportConfig as Config;
/// Не публичный re-export — default-конфиг для CLI.
pub fn default_filter() -> EpochFilter {
    EpochFilter::OwnCaptureOnly
}

// Подсказка компилятору: `Event` используется только в комментариях, но тип важен для
// downstream — оставляем `use` для doc-link.
#[allow(dead_code)]
fn _event_marker(_e: &Event) {}

#[cfg(test)]
mod smoke_tests {
    //! E2E-смок: подмножество проводки journal → export_io. Без dev-deps (postcard/crc32fast) —
    //! reducer-уровень покрыт RED-тестами; здесь проверяем только что входы
    //! типизированно валидны и `export_to_dir` отдаёт корректные имена файлов на пустой эпохе.
    use super::*;

    #[test]
    fn empty_journal_yields_no_files() {
        let tmp = std::env::temp_dir().join(format!("hft-export-smoke-{}", std::process::id()));
        let journal_dir = tmp.join("journal");
        let out_dir = tmp.join("out");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&journal_dir).unwrap();

        let cfg = ExportConfig::default();
        let source = JournalSource {
            dir: journal_dir,
            filter: EpochFilter::OwnCaptureOnly,
        };
        let written = export_to_dir(&source, &cfg, &out_dir).unwrap();
        assert!(written.is_empty(), "пустой журнал → 0 файлов, не ошибка");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn bad_timeframe_rejected() {
        let tmp = std::env::temp_dir().join(format!("hft-export-smoke-bad-{}", std::process::id()));
        let journal_dir = tmp.join("journal");
        let out_dir = tmp.join("out");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&journal_dir).unwrap();
        let cfg = ExportConfig {
            timeframe_ms: 0,
            ..ExportConfig::default()
        };
        let source = JournalSource {
            dir: journal_dir,
            filter: EpochFilter::OwnCaptureOnly,
        };
        let err = export_to_dir(&source, &cfg, &out_dir).unwrap_err();
        match err {
            ExportError::BadConfig(_) => {}
            other => panic!("ожидали BadConfig, получили {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
