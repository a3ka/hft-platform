# M-17 export contract — данные для downstream-фронта (`code2alpha` + lightweight-charts v5)

**Schema version:** `export_schema_version: 1`
**Status:** стабильный контракт (M-17 OF-I-4 / M-19 / C-016)
**Owner:** `research-dev` (impl, schema doc); `architect` (T1 изменения формата — RFC)
**Консьюмер:** `code2alpha` Fastify + lightweight-charts v5 (founder-фронт; вне нашего дерева)

> **Версионирование:** breaking change формы = bump `export_schema_version`. Аддитивные
> поля (новые серии, новые полосы) — без bump, но с описанием в этой доке.

## 0. Назначение

Бэкенд HFT-платформы (M-17) отдаёт ЧИСТЫЕ ДАННЫЕ для визуализации order-flow и order-book
фронтом. **Рендер вне нашего scope** (M-19; Fastify + open-source TradingView, у founder'а
есть наработки). Наша задача — отдать корректные, детерминированные, документированные
данные в формате, который фронт УЖЕ умеет потреблять.

Граница A: всё, что ниже — **чистые редьюсеры** над потоком `Event` (Trade / L2Snapshot)
из read-only `journal::stream`. Без wall-clock, без rand, без I/O в runtime. Экспорт-
механизм — файл (`<out_dir>/<venue>/<symbol>.json`) ИЛИ read-only эндпоинт — НЕ
рантайм-путь (recorder/journal writer не трогается; RC-I-7).

## 1. Глобальная обёртка файла

Один JSON-файл на `(venue, symbol)`. Поля:

```jsonc
{
  "schema_version": 1,                  // = export_schema_version
  "venue": "binance",                  // "binance" | "hyperliquid" | "binance_futures" | ...
  "symbol": "BTCUSDT",                 // as-is из MdEvent.symbol
  "timeframe_ms": 1000,                // база для всех таймфрейм-зависимых полей
  "epoch_ids": ["own-2025-10-12"],     // provenance: какие epoch_id попали (CT-RFC02-2)
  "first_wall_ms": 1731340800000,      // начало окна (ts_wall_ms первого события)
  "last_wall_ms": 1731344400000,       // конец окна (ts_wall_ms последнего события)

  "ohlcv": [ ... OhlcvRow ... ],      // §2
  "footprint_delta": [ ... DeltaRow ... ],   // §3.1
  "cumulative_delta": [ ... DeltaRow ... ],  // §3.2
  "footprint_bins": [ ... FootprintBarRow ... ],  // §3.3 (C-016 fix)
  "depth_series": [ ... DepthSeriesRow ... ]      // §4
}
```

**Фиксированная точка:** `price`/`size`/`notional` — `i64` ×1e8 (contracts::PRICE_SCALE);
`time_s` — UTC seconds (UDF UTCTimestamp совместим); `volume` — сумма `size`.

**Детерминизм (RC-I-5):** байт-идентичный вывод на тех же `(journal, config)`. Никаких
wall-clock полей в payload (только `first_wall_ms`/`last_wall_ms` от самих событий).

## 2. OHLCV-свечи (`ohlcv`)

Под `code2alpha` DataFeed / TradingView **UDF** (`config` / `symbol_info` / `history`),
база 1s (фронт агрегирует клиентски до 1m / 1h / D). Бэкенд-агрегация — в 1s-OHLCV.

### 2.1 Тип

```jsonc
{
  "time_s": 1731340800,   // UDF UTCTimestamp (UTC seconds; начало бакета)
  "open": 6500000000000,  // i64 ×1e8 — первая цена в бакете
  "high": 6505000000000,  // i64 ×1e8 — MAX
  "low":  6495000000000,  // i64 ×1e8 — MIN
  "close": 6502000000000, // i64 ×1e8 — последняя цена
  "volume": 12345         // i64 — Σsize в бакете (НЕ число сделок)
}
```

### 2.2 Правила per бакет (sacred — `red_ohlcv.rs`)

- `open`  = **первая** цена в бакете (по `ts_ms` сделки);
- `high`  = `MAX(price)` в бакете;
- `low`   = `MIN(price)` в бакете;
- `close` = **последняя** цена в бакете (по `ts_ms`);
- `volume`= `Σsize` сделок в бакете.

Бакет: `ts_ms / timeframe_ms` (целочисленное; `timeframe_ms` — обычно 1000 для 1s-базы).
Пустой вход → пустой массив (не выдуманная свеча).

## 3. Order-flow серии

Под **lightweight-charts v5** (`LineData{time,value}` / `HistogramData{time,value,color}`).
Все `time` — UTC seconds.

### 3.1 Footprint-дельта (per-бар скаляр)

`{ time_s, value }` где `value = Σ(size | side=Buy) − Σ(size | side=Sell)`. **ЗНАКОВАЯ
агрессия** per бакет, не `|buy|+|sell|`. Сторона берётся из `MdPayload::Trade.side` (taker,
агрессор) — Binance m-flag инверсия уже применена в парсере (M-06).

### 3.2 Cumulative delta (running)

`{ time_s, value }` где `value[b] = Σ(знаковая агрессия до конца бакета b)`. **Running**,
НЕ сброс per-бакет. Знак дельты зависит от стороны агрессора — кумулята монотонно растёт
на чистых покупках, монотонно падает на чистых продажах. Дивергенция с ценой = производный
сигнал (отдельный оракул, не в этом контракте).

### 3.3 Footprint BINS (per-цена) — C-016 fix

Полный footprint для custom-series фронта (M-19 Тир2 — cluster / heatmap-чарт).
**НЕ** скалярная дельта per-бар (3.1) — это **матрица** `(bucket, price) → {buy_vol,
sell_vol, delta=buy−sell}`. Цены, по которым НЕ было сделок, НЕ выдумываются.

```jsonc
{
  "time_s": 1731340800,
  "bins": [
    { "price": 6500000000000, "buy_vol": 5, "sell_vol": 0, "delta": 5 },
    { "price": 6501000000000, "buy_vol": 0, "sell_vol": 3, "delta": -3 }
  ]
}
```

- `bins` отсортированы по `price` (BTreeMap-редьюсер);
- `delta = buy_vol − sell_vol` (ЗНАКОВАЯ, не сумма модулей);
- на одной цене `buy`/`sell` РАЗДЕЛЬНЫЕ аккумуляторы (не сливаются);
- `time_s` = начало бакета в СЕКУНДАХ.

## 4. Depth time-series (per side, per band)

Под `LineData{time,value}` для линейного графика. **BID и ASK — РАЗДЕЛЬНЫЕ серии**
(порядок рядов в `depth_series` — сначала все BID, потом все ASK, в порядке band
по возрастанию; полный порядок — стабильный).

```jsonc
{
  "side": "bid",                     // "bid" | "ask" — НЕ суммированы
  "band_pct_e8": 100000,             // 0.001 ×1e8 = 0.1% (band = 0.001)
  "series": [
    { "time_s": 1731340800, "value": 12345678 },
    { "time_s": 1731340801, "value": 12349876 }
  ]
}
```

- `band_pct_e8` — полоса в долях ×1e8 (фронт делит на 1e8 для отображения как процент);
- `value` = `book.depth_within(side, band_pct)` в `i64` (size ×1e8);
- `time_s` = начало бакета в СЕКУНДАХ;
- значение = глубина **ПОСЛЕДНЕГО** снапшота в бакете (close-семантика, детерминир.);
- пустые бакеты НЕ эмитятся (нет выдуманных точек).

## 5. Детерминизм-инварианты (sacred)

1. **Один вход → один выход (RC-I-5):** повторный запуск на тех же `(journal, config)`
   даёт байт-идентичные файлы. Никаких `std::time::SystemTime::now()` в payload.
2. **Порядок:** `ohlcv` / `footprint_delta` / `cumulative_delta` / `footprint_bins.bins` /
   `depth_series[].series` — отсортированы по `time_s` (BTreeMap-редьюсер).
3. **Стороны:** `depth_series` сначала все BID, потом все ASK (стабильный порядок полос).
4. **Per-цена:** `footprint_bins` — один bin на цену; цены без сделок не выдумываются.
5. **Пустые бакеты:** не эмитятся ни в одной серии.

## 6. Endpoints (для CI / smoke)

Файл-экспорт (рекомендуемый):

```bash
# Дериватив: per (venue, symbol) → <out_dir>/<venue>/<symbol>.json
research-cli export --journal <dir> --out <out_dir> \
  [--timeframe-ms 1000] \
  [--band 0.001 --band 0.003 --band 0.005] \
  [--filter own_capture_only]
```

Все артефакты одного экспорта — в одном каталоге; M-19 фронт читает их как статику
(`fs.readFileSync` в Fastify) ИЛИ через лёгкий read-only эндпоинт (на усмотрение
founder'а; реализация — вне этого дерева).

## 7. Не в этом контракте (out of scope M-17)

- Book-flow (absorption / DOM / iceberg) — **Phase B / M-18**; требует raw book-дельт
  + CT-RFC-04. Не вычислимо из Trade + L2Snapshot.
- Любой рендер / HTML / Canvas / WebGL — **M-19**, founder-фронт.
- Сигналы (Signal-trait) — **Граница A, signal-engineer**; trade-flow `Signal`-impl
  (footprint score, divergence) — отдельный T1, отдельный handoff.

## 8. Версионирование changelog

- **v1 (2026-07-21, M-17):** первичный экспорт-контракт. UDF 1s-бары, lightweight-charts
  depth/footprint серии, per-цена footprint bins (C-016 fix). BTreeMap-стабильность.
