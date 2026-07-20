# M-19 — Frontend cockpit на базе `code2alpha` (ПОЗЖЕ — после готовности бэкенда)

STATUS: **PROPOSED / DEFERRED** (2026-07-20, architect). Стартует ПОСЛЕ того, как бэкенд отдаёт данные
(M-17 экспорт + M-10 отчёты + M-18 book-дельты + M-09 /metrics). Это vision-документ: **что мы хотим
отображать и реализовать**. Реализация — дорабатываем существующий `a3ka/code2alpha`
(Next.js 15 + React 19 + lightweight-charts v5 + UDF DataFeed), НЕ пишем с нуля.

## Objective

Превратить `code2alpha` в наш **research + monitoring cockpit**: свечи + order-flow (стиль Фабио) +
сигналы + бэктест-отчёты + **детерминированный replay журнала** + мониторинг системы. Фронт потребляет
данные нашего Rust-бэкенда (UDF-бары + lightweight-charts-серии, M-17 контракт); LLM-ассист — только
дизайн-тайм (НЕ в торговом цикле, наш инвариант).

## База: что в `code2alpha` уже есть (дорабатываем, не переписываем)

- `TradingChart.tsx` — свечи + наложение результатов стратегии.
- Multi-chart dashboard (iframe, обход React/SSR-грабель lightweight-charts).
- Backtest drawer (equity-curve + метрики), Monaco code-editor drawer, Chat panel, indicators.
- UDF DataFeed (`api/tradingview/route.ts`) — сейчас mock → **заменить на наш бэкенд**.

## Что хотим отображать (полный список — по областям)

### A. Ядро чарта (lightweight-charts, нативно — дёшево)
- Свечи (база 1s, клиентская агрегация до 1m/1h/D), объём-гистограмма, crosshair, мульти-таймфрейм.
- Селектор symbol/venue/дата-диапазон.

### B. Order-flow виды (набор Фабио)
| Вид | Что показывает | Feasibility | Бэкенд |
|---|---|---|---|
| **Footprint / cluster** | buy/sell объём + дельта на КАЖДОМ ценовом уровне внутри свечи | custom series v5 (**работа**) | M-17 footprint-bins |
| **Cumulative delta** | накопленная агрессия; дивергенция с ценой | нативно (LineData) | M-17 |
| **Delta-гистограмма/бар** | знаковая дельта, цвет | нативно (HistogramData) | M-17 |
| **Volume profile** | POC / value area (гориз. гистограмма по цене) | custom series | M-17/книга |
| **DOM ladder** | живая лестница лимиток, absorption-подсветка | **bespoke grid** | **M-18 (raw дельты)** |
| **Liquidity heatmap** (Bookmap-style) | тепловая карта покоящейся ликвидности во времени | **bespoke canvas/WebGL** (продвинуто) | **M-18** |
| **Tape / time&sales** | поток сделок (размер/скорость/сторона) | таблица | сделки (есть) |

### C. Слой сигналов
- Маркеры входа/выхода на чарте (стрелки), лог срабатываний.
- Панели значений сигналов (OBI, footprint-дельта, cumulative-delta дивергенция) в реальном времени.
- Тумблеры наложения нескольких сигналов; per-signal под-панели.

### D. Бэктест / research
- Отчёт R-001-стиля: equity-curve, Sharpe / **deflated Sharpe / SE / data_span_days**, **kill-screen
  вердикт** (Kill/Inconclusive/Pass с причиной), walk-forward стабильность, decay-кривая, стресс (×1.5/×2).
- Грид-хитмап параметров (Sharpe по (n_levels, theta, horizon)).
- Просмотр trials-ledger (эпохи, deflated-N).

### E. Replay журнала — НАШ дифференциатор (journal-first детерминизм)
- **Скраб/replay ЛЮБОГО исторического момента бит-идентично** (наш журнал детерминирован).
- Пошаговый проигрыш микроструктуры: book-дельты, сделки, срабатывания сигналов — событие за событием.
- Ни один retail-сервис (Bookmap/ATAS) этого не даёт над ТВОИМИ данными — у нас есть.
- Бэкенд: журнал (есть) + read-only replay-эндпоинт (отдаёт Event-поток по окну).

### F. Мониторинг системы (из /metrics + recon, M-09)
- Recon-дивергенция (best-price), глубина книги per venue/symbol, gap-доля, тишина потока, RssAnon-тренд.
- Лента алертов P0/P1/P2. Health-дэшборд (встроить из `/metrics` ИЛИ Prometheus/Grafana — развилка).

### G. Мульти-чарт дэшборд
- Несколько symbol/venue рядом (code2alpha iframe уже), синхронный crosshair/время между чартами.

### H. Инструментарий research
- Monaco-редактор для экспериментов с сигналами (есть), сохранение/загрузка layout'ов.
- LLM-чат-ассист — **ТОЛЬКО дизайн-тайм** (анализ/идеи), НЕ в торговом решении (граница A/B/C).

## Feasibility-тиры (порядок реализации по стоимости)

1. **Тир 1 (нативно lightweight-charts, дёшево):** свечи, объём, cumulative delta, delta-гистограмма,
   сигнал-маркеры, equity-curve, мульти-чарт. → нужен **M-17** (бары+серии) + **M-10** (отчёт).
2. **Тир 2 (custom series, средне):** footprint, volume profile. → **M-17** footprint-bins.
3. **Тир 3 (bespoke canvas, продвинуто):** DOM-ladder, Bookmap-хитмап. → **M-18** (raw book-дельты).
4. **Replay (E) + мониторинг (F):** параллельно, на журнале (есть) + /metrics (есть).

## Границы / инварианты

- Фронт — ТОЛЬКО потребитель данных бэкенда; **никакой торговой логики/риска на фронте** (риск — Rust
  fail-closed, M-11). LLM на фронте — дизайн-тайм.
- Экспорт-формат бэкенда версионируется (M-17 `export_schema_version`) — фронт строится против стабильного контракта.
- Bookmap-хитмап (Тир 3) — явный «может не 1:1 как Фабио» (lightweight-charts не для этого; отдельный canvas).

## Зависимости (что должно быть готово ДО старта M-19)

M-17 (order-flow сигналы + UDF/серии экспорт) → Тир 1/2. M-18 (book-дельты) → Тир 3 (DOM/heatmap).
M-10 (R-001 отчёт) → область D. M-09 /metrics (есть) → область F. Журнал (есть) → область E (replay).

## Handoff (план, ПОЗЖЕ)

Когда бэкенд отдаёт данные — founder дорабатывает `code2alpha` (frontend-dev / founder), подключив UDF
DataFeed к нашему бэкенду. Architect: экспорт-контракт (M-17) + replay-эндпоинт спека. Это НЕ Rust-milestone
цепочки critic→dev — это фронт-работа founder'а; здесь фиксируем ЧТО показать, чтобы бэкенд отдавал нужное.
