# M-17 — Order-flow сигналы Phase A (trade-flow) + экспорт данных (бэкенд; из УЖЕ собираемых данных)

STATUS: **PROPOSED** (2026-07-20, architect). Doc-гейт §9 Class A. Founder дал «go»
(«собирать данные для визуализации и создания сигналов по стратегии Fabio»).

## Objective

Order-flow скальпинг (метод Fabio) делится на **trade-flow** (агрессия сделок) и **book-flow**
(динамика лимиток: absorption/DOM). **Критическая находка architect'а: сторону АГРЕССОРА мы УЖЕ
пишем** (`MdPayload::Trade.side` = taker-сторона, Binance m-флаг инверсия). Значит **trade-flow
order-flow вычислим из данных, что уже собираем — без изменения захвата и без T1.**

Phase A (ТОЛЬКО БЭКЕНД): реализовать trade-flow сигналы (footprint-дельта, cumulative delta,
per-price агрессия, imbalance) как чистые редьюсеры + **ЭКСПОРТ производных серий** в документированном
формате (JSON/series) для downstream-визуализации. **Визуализация — вне scope M-17:** её реализует
founder отдельно (Fastify-сервер + open-source TradingView, есть наработки); наша задача — отдать
корректные ДАННЫЕ, не рисовать. Book-flow (absorption) — Phase B (M-18, требует raw-delta + CT-RFC-04).

## Contract impact (T1) — НЕТ

Сигналы читают существующие `Trade` + `L2Snapshot`. Виз — экспорт производных серий. Новых T1 нет.

## Инварианты (RED, sacred)

| ID | Инвариант |
|---|---|
| OF-I-1 | **Детерминизм (Граница A):** сигнал = чистый редьюсер над потоком `Event` до текущего момента; НЕТ доступа к будущему, НЕТ wall-clock/rand. RED: одинаковый Event-поток → одинаковый выход (два прогона) |
| OF-I-2 | **Footprint-дельта верна:** для бара/цены `delta = Σ(aggressive_buy_size) − Σ(aggressive_sell_size)` из `Trade.side`. RED: фикстура сделок с известными сторонами → точная дельта; сторона НЕ перепутана (buy↔sell) |
| OF-I-3 | **Cumulative delta монотонно накапливает** знаковую агрессию; дивергенция с ценой — производный сигнал. RED: последовательность → точная кумулята; сброс окна детерминирован |
| OF-I-4 | **Экспорт-контракт честен (данные для downstream-виза):** экспорт footprint несёт (цена, buy_vol, sell_vol, delta) per bin в СТАБИЛЬНОМ документированном формате; агрегация НЕ теряет сторону и НЕ выдумывает уровни, которых не было (та же дисциплина, что C1/эвикция). RED: асимметр. вход → корректные bins. Рендер — не наша забота, но данные обязаны быть точны и самодостаточны |
| OF-I-5 | **Trade-flow ≠ book-flow (честная граница):** absorption/iceberg/DOM НЕ вычислимы из trade+snapshot (нужны raw book-дельты). Phase A их НЕ заявляет; они — M-18 |
| OF-I-6 | **Depth time-series per (side, band):** для каждой площадки/символа/стороны (BID/ASK ОТДЕЛЬНО)/полосы (`RECON_BANDS` + глубже для Binance) — временной ряд суммарной глубины `depth_within(side, band)` по таймфреймам (1s база → 1m/1h/… агрегация). Экспорт как `LineData{time,value}` серии для линейного графика. RED: BID и ASK РАЗДЕЛЕНЫ (не суммированы/не перепутаны); полоса корректна (0.3% ≠ 0.5%); агрегация в бакет таймфрейма детерминирована; чистый редьюсер над `L2Snapshot` (Phase A, БЕЗ raw-дельт) |

## Allowed / Forbidden paths

- `crates/signals/src/**` (новые trade-flow сигналы: `footprint.rs`/`cumdelta.rs`; семья S-002+) — **signal-engineer** (Граница A).
- `research/specs/S-002-*.md`, `research/hypotheses/H-*.md` (пре-регистрация trade-flow гипотез) — signal-engineer.
- `crates/research-cli/src/**` (экспорт footprint/delta серий в JSON/series для downstream-виза) — **research-dev**.
- `research/exports/**` (документированный формат экспорта — схема + примеры для founder-фронта) — research-dev.
- `*/tests/**` (OF-I-* RED), `scripts/verify_M-17.sh`, milestone — **architect**.
- **ВНЕ scope M-17:** любой рендер/HTML/чарт — это founder-фронт (Fastify + open-source TradingView), отдельно.
- **Forbidden:** `crates/{risk,killswitch,oms,journal,recorder,venue-*,contracts}`; сигнал с I/O или доступом к будущему; промоушен сигнала (Граница B/C — founder-подпись).

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | OF-I-* RED (`crates/signals/tests/red_orderflow.rs` детерминизм+дельта; `research-cli/tests/red_footprint_export.rs` экспорт-данные) | architect | RED падает без impl; достижим |
| 2 | ⏳ | `verify_M-17.sh` | architect | exit=0 на GREEN |
| 3 | ⏳ | Пре-регистрация: `H-*-orderflow` + `S-002-footprint` (критерии фальсификации ДО бэктеста) | signal-engineer | карточка + spec |
| 4 | ⏳ | Реализация trade-flow сигналов (footprint-дельта, cumulative delta, per-price imbalance) — чистые редьюсеры | signal-engineer | OF-I-1/2/3 GREEN |
| 5 | ⏳ | Экспорт footprint/delta серий в документированном формате (JSON/series: цена, buy_vol, sell_vol, delta per bin/бар) + схема+пример в `research/exports/` для founder-фронта | research-dev | OF-I-4 GREEN; экспорт корректен и стабилен (рендер — вне scope) |
| 6 | ⏳ | (опц.) прогон trade-flow сигнала через M-10 kill-screen | research-dev | вердикт по пре-рег критериям |
| 7 | ⏳ | **Depth time-series (OF-I-6):** редьюсер `depth_within(side,band)` над `L2Snapshot` → ряд per (venue,symbol,side,band,timeframe); экспорт `LineData` серий для линейного графика (BID/ASK раздельно) | research-dev | OF-I-6 GREEN; экспорт per side/band корректен |

## Гейты

- critic (новый milestone §9). Сигналы = Граница A (signal-engineer), детерминизм-тест обязателен.
- **risk-critic N/A для ИМПЛА сигнала** (нет safety/order-path); но БЭКТЕСТ-ОТЧЁТ (task 6) — анти-оверфит §6 + risk-critic (как M-10).
- Экспорт-формат — стабильный контракт для downstream-фронта (версионируется); рендер вне scope.

## Экспорт-контракт (под готовый фронт founder'а — `code2alpha`, lightweight-charts v5)

Формат НЕ изобретаем — целим в то, что фронт УЖЕ умеет (проверено architect'ом по `code2alpha`):

- **Свечи:** TradingView **UDF** (`config`/`symbol_info`/`history`) с базой **1s** (`type:'second',
  baseInterval:1`) — фронт агрегирует клиентски до 1m/1h/D. Наш бэкенд агрегирует trades/snapshots в
  1s-OHLCV. Прецедент: `hft-core-rs-` `/v1/bars`.
- **Order-flow серии (наш вклад):** в shape lightweight-charts v5 — `LineData{time,value}`
  (cumulative delta), `HistogramData{time,value,color}` (footprint-дельта/бар, цвет=знак),
  сигналы — маркеры/серия. `time` = UTC seconds. Full per-price footprint-bins отдаём как данные;
  их custom-series рендер — фронт-работа (вне scope).
- **Depth-серии (OF-I-6, per side/band):** `LineData{time,value}` per (venue,symbol,side,band,timeframe)
  — глубина `depth_within(side,band)`; запрос `?venue&symbol&side&band&resolution&from&to`. Фронт рисует
  N линий (BID/ASK × полосы) на панели. Value = глубина в конце бакета таймфрейма (детерминир.).
- **Механизм подачи:** лёгкий read-only эндпоинт ИЛИ файл-экспорт (impl research-dev) — НЕ
  детерминированный рантайм-путь (recorder не трогается).

Стабильность: формат версионируется (`export_schema_version`); внутреннее вычисление меняем, не ломая фронт.

## Связь с роадмапом

Phase A (этот) — trade-flow, из существующих данных, СЕЙЧАС. Phase B (**M-18**, CT-RFC-04 `L2Delta` +
venue-dev raw-delta захват) — book-flow (absorption/DOM). Вместе = полный order-flow «как Fabio».
OBI (M-10) — простейший depth-сигнал, ортогонален (book-imbalance, не trade-flow).

## Handoff (план)

critic → signal-engineer (пре-рег + trade-flow сигналы) + research-dev (экспорт данных) → (опц.) risk-critic на отчёт.
Architect: OF-I-* RED + verify.
