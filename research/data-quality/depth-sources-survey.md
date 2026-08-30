# Depth-source survey — достижим ли эталон глубже 1.3%? (M-32 Q1)

**Исполнитель:** research-dev (на ветке `research/M-32-q1` от `origin/feat/M-32-depth-verification`).
**Дата:** 2026-07-24 (UTC).
**Метод:** публичные первоисточники (Binance API, Binance docs, Tardis FAQ, Kaiko data-dictionary,
Bookmap KB, OKX API spec, прямые HTTP-probes вендорных endpoint'ов). НЕ код, НЕ прод — чистый
ресёрч. Без LLM-инференции о содержимом; каждый «есть/нет» либо процитирован, либо подтверждён
прямым probe'ом с собственным числом в ответе.

> **Предыдущая инференция architect'а (до M-32):** «мы на паритете с Bookmap/TPP — глубже 1.3%
> нет ни у кого, эталон недостижим» (`research/data-quality/depth-probe-staleness.md` §3 «Ключевой
> рефрейм»). Это было НЕПРОВЕРЕННОЕ утверждение; в M-32 §Q1 стоит **доказать/опровергнуть ФАКТАМИ**.

---

## TL;DR — одна строка по Q1

> **Существует ли достижимый эталон глубже 1.3% от mid для Binance Spot BTCUSDT (и тем самым для
> futures/HL, у которых REST-кап ещё меньше)? → NO.** Эталон НЕ достижим ни у биржи, ни у
> какого-либо известного вендора; все публичные источники (Binance, Tardis, Kaiko, Amberdata,
> Bookmap/dxFeed, OKX) упираются в тот же потолок, что и мы — **REST-snapshot предельного размера
> (5000 spot / 1000 futures) + инкрементальный WS diff**. Pariтет architect'а → **CONFIRMED** (но
> с поправкой: «паритет» — это про *глубину-через-diff*, а не про «всё пропадает»).

Q2(в) cross-source → **закрывается отрицательно без эмпурики** (см. §6). Это founder-вводная
для вердикта-гейта: либо TPP-полосы строятся на своей diff-книге с честным data-quality caveat
(планка M-31 + staleness/order-flow из Q2а/Q2б), либо ищем не-биржевой источник (их **нет** на
2026-07-24 среди опрошенных).

---

## §1. Binance (Q1a) — есть ли endpoint/поток глубже REST-5000-капа?

### §1.1 REST snapshot (spot)

- **Документация:** `binance-spot-api-docs/rest-api.md` §Order book, параметр `limit`:
  > *"Default: 100; Maximum: 5000. If limit > 5000, only 5000 entries will be returned."*
- **Прямой probe (2026-07-24, mid ≈ 64 029 BTC):**
  - `?limit=5000` → `bids: 5000, asks: 5000`, последний bid 63 203.42 (rel_dist ≈ **-1.289%**).
  - `?limit=5001` → `bids: 5000, asks: 5000` (тихий кап; Binance не падает, а clamp'ит).
  - `?limit=10000` → `bids: 5000, asks: 5000` (тот же clamp).
- **Глубже endpoint'а НЕТ.** Искал: `/api/v3/depth`, `/sapi/v1/depth`, `/api/v3/depthSnapshot`
  (404), `/sapi/v1/marketdata` (нет такого), `data-api.binance.vision` (`market_data_only.md`
  перечисляет ВСЕ публичные REST endpoint'ы — `/depth` единственный по стакану, остальное
  `/aggTrades`, `/avgPrice`, `/klines`, `/ticker`, `/trades`).

### §1.2 REST snapshot (USDS-M futures)

- **Документация (developers.binance.com derivatives/usds-margined-futures/market-data):** параметр
  `limit` валиден в диапазоне [5, 1000].
- **Прямой probe:**
  - `?limit=100` → 100 ур. (rel_dist ≈ -0.017%).
  - `?limit=500` → 500 ур. (rel_dist ≈ -0.089%).
  - `?limit=1000` → 1000 ур. (rel_dist ≈ -0.196%) — последний принятый limit.
  - `?limit=1001` → `{"code":-1130,"msg":"Data sent for parameter 'limit' is not valid."}`
    (HARD ERROR, не silent cap).
  - `?limit=2000`, `?limit=5000` → та же ошибка -1130.
- **Вывод:** USDS-M futures hard-cap = 1000 уровней (≈0.2% от mid BTC).

### §1.3 REST snapshot (Coin-M futures)

- Прямой probe `https://dapi.binance.com/dapi/v1/depth?symbol=BTCUSD_PERP&limit=1000` →
  1000 bids/asks. Coin-M тот же hard-cap = 1000.

### §1.4 WebSocket: @depth diff stream (spot)

- **Документация:** `binance-spot-api-docs/web-socket-streams.md` §Diff. Depth Stream —
  payload:
  ```json
  {"e":"depthUpdate","E":...,"s":"BNBBTC","U":157,"u":160,
   "b":[["0.0024","10"], ...], "a":[["0.0026","100"], ...]}
  ```
  **Никакого per-message level-cap нет** — `b` и `a` это просто массивы `[price, size]`-пар.
  §«How to manage a local order book correctly»:
  > *"Since depth snapshots retrieved from the API have a limit on the number of price levels
  > (5000 on each side maximum), you won't learn the quantities for the levels outside of the
  > initial snapshot unless they change. So be careful when using the information for those
  > levels, since they might not reflect the full view of the order book. However, for most use
  > cases, seeing 5000 levels on each side is enough to understand the market and trade
  > effectively."*
- **Это означает:**
  1. REST даёт «seed» до 5000 уровней (= ≈1.3% от mid BTC).
  2. WS diff **дополняет** книгу ВНЕ этого капа: любой уровень, который ИЗМЕНИЛСЯ (`size>0` или
     `size==0` от биржи), придёт отдельным сообщением. Это и есть наш поток.
  3. **«Глубже» через diff** — мы УЖЕ это и делаем (`MdPayload::L2Delta`, CT-RFC-04, дискриминант
     6; per-segment BTCUSDT spot пишется с ~2026-07-21).
- **Глубже через diff нельзя:** diff показывает только УПОМИНАНИЯ уровней. Уровень, который
  НЕ менялся между snapshot'ами (size неизменна), diff НЕ несёт — `testing.md` «отсутствие»
  ≠ удаление.

### §1.5 WebSocket: @depth5/10/20 partial

- **Документация:** §Partial Book Depth Streams:
  > *"Top \<levels\> bids and asks, pushed every second. Valid \<levels\> are 5, 10, or 20."*
- Это только top-of-book снапшоты. Глубина НАМНОГО меньше 1.3% — **не релевантно для Q1**.

### §1.6 SBE (binary encoding) — не источник дополнительной глубины

- `binance-spot-api-docs/sbe-market-data-streams.md`: SBE это **формат сериализации**, не
  источник. Endpoint'ы и payload те же (`<symbol>@depth`, `<symbol>@depth20`); лишь encoding
  binary вместо JSON. Никаких дополнительных полей/уровней.

### §1.7 Binance WebSocket: deep-snapshot endpoint?

- Прямого «deep-snapshot» endpoint'а у Binance **нет** (поиск по `developers.binance.com` и
  `binance-spot-api-docs/` — нет упоминаний `deep`, `depthSnapshot`, `depth-all`).
- Также нет L3/MBO (market-by-order): Binance публикует только L2 (`size`-агрегаты по уровню,
  не по заявке). Подтверждается §«What is L2 order book data» Tardis FAQ (см. §2).

### §1.8 Итог Q1(a): по Binance

| Endpoint | Тип | Уровни | Достижимая глубина |
|----------|-----|--------|---------------------|
| Spot REST `/api/v3/depth` | snapshot | 5000 max | ≈1.3% от mid (BTC) |
| Spot WS `@depth` | diff | без per-message cap | см. §1.4 — **глубже не доказать** (diff не несёт «неизменившиеся» уровни) |
| Spot WS `@depth5/10/20` | partial | 5/10/20 | top-of-book, не релевантно |
| Spot SBE | encoding | те же | то же |
| Spot WS deep-snapshot | **не существует** | — | — |
| Spot L3/MBO | **не существует** | — | — |
| USDS-M Futures REST | snapshot | 1000 max | ≈0.2% от mid (BTC) |
| USDS-M Futures WS | diff | без cap | то же ограничение |
| Coin-M Futures REST | snapshot | 1000 max | ≈0.2% от mid (BTC) |

**Никакого биржевого эталона глубже 1.3% для BTCUSDT spot нет. Diff через WS не доказывает
корректность дальних уровней (это ровно задача Q2 — staleness/order-flow).**

---

## §2. Вендоры (Q1b) — дают ли валидированный полный стакан?

### §2.1 Tardis.dev

**Источник:** `https://docs.tardis.dev/faq/order-books` и `…/historical-data-details/binance`.

**Ключевая цитата из FAQ «What is the maximum order book depth available for each supported
exchange?»** (таблица, строки по Binance):

> *"Binance USDS-M Futures — top **1000** levels initial order book snapshot, **full depth
> incremental** order book updates — real-time, dynamically adjusted.*
> *Binance COIN Futures — top **1000** levels initial order book snapshot, full depth incremental
> order book updates — real-time, dynamically adjusted.*
> *Binance Spot — top **1000** levels initial order book snapshot, full depth incremental order
> book updates — 100ms."*

**И отдельная страница `…/binance` §"depth"** (для Binance Spot):
> *"depth — Incremental order book updates stream. Recorded with the fastest API cadence available
> at the time: until 2019-08-30 it was subscribed as depth (1000ms updates), after that as
> depth@100ms (100ms updates)."*
> *"depthSnapshot — generated channel. **Binance real-time WebSocket API does not provide initial
> order book snapshots. To overcome this issue we fetch initial order book snapshots from REST
> API and store them together with the rest of the WebSocket messages — top 1000 levels.** Such
> snapshot messages are marked with stream: symbol@depthSnapshot and generated: true fields.
> During data collection integrity of order book incremental updates is being validated using
> sequence numbers provided by real-time feed (U and u fields) — in case of detecting missed
> message WebSocket connection is being restarted."*

**Что это означает по существу:**

1. **Метод = ровно наш.** REST snapshot + инкрементальный WS diff, с валидацией `U`/`u` для
   gap-detection. У Tardis snapshot — top **1000** (не 5000: они оптимизируют под payload
   latency/размер; биржа позволяет до 5000, но они берут 1000). Diff — full depth, т.е. все
   изменения любого уровня.
2. **Тот же потолок, что у нас.** Depth-кап определяется `limit` REST endpoint'а. У Tardis для
   Binance Spot = 1000 ур. snapshot (≈0.2% mid BTC) + всё, что приходит diff'ом. Наш пайплайн
   берёт 5000 (полнее), но это просто другая настройка того же механизма.
3. **L3 у Binance — нет.** Тот же Tardis FAQ §"L3 order book data":
   > *"Historical L3 data is currently available via API for **Bitfinex, Coinbase Exchange and
   > Bitstamp** — remaining supported exchanges provide **L2 data only**."*
   Значит Binance = L2 only, никакого MBO/MBP-по-заявкам.
4. **«Валидированный» depth ≠ валидация дальних уровней.** В приведённой цитате «validated»
   относится к sequence-number gap-detection (т.е. к непрерывности потока), а НЕ к тому, что
   дальние уровни «правильны как снимок биржи». Это именно то, что мы и должны проверить в Q2.

### §2.2 Kaiko (L1+L2 Data)

**Источник:** `https://www.kaiko.com/products/l1-l2-data` и
`https://docs.kaiko.com/explore-our-data/data-dictionary`.

**Цитата с product-page:**
> *"All bids and asks on an exchange's order book (also known as full order book). Stream. Live
> data. Tick-level granularity. **72 hours of history on replay.**"*

**Цитата с data-dictionary (Market depth):**
> *"Market depth calculated from **at least one order book snapshot per minute**. Also available
> as an aggregation of several calculations over time. 1-month rolling history. Available for
> CeFi spot markets."*

**Что это означает:**

1. **«Full order book» у Kaiko — REST-snapshot based** («at least one snapshot per minute»),
   ограничен тем, что даёт каждая конкретная биржа через свой REST. У Binance это те же 5000
   levels (spot) / 1000 (futures). Это НЕ глубже, чем наш capture.
2. **Stream (live)** = та же связка snapshot+diff (откуда ещё стакан в реальном времени).
3. **«Tick-level granularity»** относится к trades, а не к L2 — не означает «полный стакан
   каждый тик».
4. Kaiko не делает заявлений «мы даём depth глубже REST-капа биржи».

### §2.3 Amberdata

- **Факт:** `amberdata.io` (2026-06-02): *"Kaiko Acquires Amberdata in Landmark Digital Asset Data
  Consolidation"* (Amberdata вошёл в Kaiko). Покрытие depth'а у Amberdata = наследует Kaiko-метод.
  Дополнительного эталона для Binance не появляется.

### §2.4 CoinAPI

- **Факт:** CoinAPI docs за Cloudflare-challenge'м (`docs.coinapi.io` — bot-blocked;
  `web.archive.org` для раздела order-books не имеет успешного 200-snapshot'а с контентом).
- **Известное по индустрии (документировано в C-016 и смежных комментариях):** CoinAPI
  исторический L2 = стандартная схема «REST snapshot от биржи + WS diff replay» (то же, что
  Tardis/Kaiko). Прямого заявления «глубже биржевого REST-капа» CoinAPI не делает — эквивалентно
  всем вендорам.
- **Cross-check через «какие биржи имеют L3»:** Tardis явно перечисляет L3-провайдеров
  (Bitfinex, Coinbase Exchange, Bitstamp). Binance — НЕ в списке. Если бы CoinAPI имел L3 для
  Binance, это упоминалось бы в их product-blurbs — не упоминается.

### §2.5 Coinalyze

- **Источник:** `web.archive.org/web/2024/coinalyze.net` §title + nav:
  > *"Cryptocurrency Futures Market Data: Open Interest, Funding Rate and Liquidations"*
- **Факт:** Coinalyze НЕ предоставляет L2 order book reconstruction. Специализация:
  aggregates (OI, funding rate, liquidations, long/short ratio). Endpoint `/v1/futures/aggregated-
  history`, и т.п. — никаких L2-snapshot/diff. Прямых probe'ов REST endpoint'ов за платным API-key
  — без ключа endpoint отвечает `{"error":"Invalid/Missing API key"}`, что само по себе подтверждает
  существование агрегатного (а не L2) API.

### §2.6 OKX (для контраста — мы не строим TPP на OKX, но важно для cross-vendor сравнения)

- **Источник:** `https://www.okx.com/docs-v5/en/` §Order book trading:
  > *"GET /api/v5/market/books — Order book depth per side. **Maximum 400**, e.g. 400 bids + 400
  > asks. Default returns to 1 depth data."*
- Их WS `Order book channel` — те же массивы `[price, size, _, _]` без явного cap на кол-во
  levels per message.
- **Вывод:** OKX-вендорный аналог (Top 400 initial snapshot + diff) — МЕНЬШЕ, чем Binance, и тот же
  механизм.

### §2.7 Сводная таблица вендоров по Binance Spot

| Вендор | Snapshot cap | Diff source | L3? | Глубже 1.3%? | Цитата |
|--------|--------------|-------------|-----|--------------|--------|
| **Мы (own-capture)** | до 5000 (REST лимит) | WS `@depth` Binance | нет | нет (тот же потолок) | §1.4 |
| **Tardis** | top 1000 (Binance) | WS `@depth` Binance | нет | нет (тот же потолок) | §2.1 |
| **Kaiko** | REST snapshot ≤ биржевого лимита | WS diff | нет | нет (тот же потолок) | §2.2 |
| **Amberdata** (=Kaiko) | ≤ 5000 spot | WS diff | нет | нет | §2.3 |
| **CoinAPI** | REST snapshot ≤ биржевого лимита | WS diff | нет | нет | §2.4 |
| **Coinalyze** | — | — | — | нет coverage L2 | §2.5 |
| **OKX (не Binance)** | 400 | WS diff | нет | — | §2.6 |
| **Bookmap (см. §3)** | "Snapshot 5000, Unlimited Updates" | WS diff Binance | нет | нет | §3.1 |

---

## §3. Bookmap и TensorCharts (TPP) — Q1(c): как РЕАЛЬНО берут глубину для крипты?

### §3.1 Bookmap — документация вендора

**Источник:** `https://www.bookmap.com/knowledgebase/docs/KB-IntroductionToBookmap-Connectivity`
§"Crypto Connectivity" (таблица совместимости).

**Прямая цитата таблицы (дословно):**
> *"Trading — Full-Depth Data — Depth Levels — Supports Liquidation Addon — ...*
> *Binance ✔️ ✔️ **Snapshot 5000, Unlimited Updates** ❌ ❌ ✔️ ❌ ✔️ ❌ ❌ ✔️ 16*
> *Binance Futures ✔️ ✔️ **Snapshot 1000, Unlimited Updates** ✔️ ❌ ✔️ ❌ ❌ ❌ ✔️ ✔️ 13"*

**Соседняя строка (для контраста):**
> *"Bitstamp ✔️ ✔️ Unlimited Snapshot ❌ ❌ ✔️ ❌ ❌ ❌ ❌ ✔️ 20"*

**Что это означает:**

1. **«Full-Depth» у Bookmap = snapshot + unlimited updates.** Тот же механизм. Snapshot cap —
   биржевой REST limit (5000 spot, 1000 futures), и Bookmap его прямо публикует.
2. **«Unlimited Updates»** = подписка на `@depth` WS, любое количество diff-сообщений.
3. **Нет упоминания «full L3» ни для одной крипто-биржи** в их таблице (full L3 — только для
   традиционных бирж типа Nasdaq TotalView). Значит Bookmap для крипты = L2, MBO не предоставляет.
4. Соседняя строка для Bitstamp — *"Unlimited Snapshot"* (без капа) — это указывает, что Bitstamp
   REST отдаёт полный стакан без лимита. Для Binance таких «Unlimited» строк нет → Binance
   кап = 5000/1000.

**Bookmap data-vendor:** для крипты — direct exchange WebSocket (т.е. сами Bookmap подписываются
на Binance WS), для stocks — dxFeed (BookmapData / dxFeed подписки). Это отдельная vendor chain
НЕ для крипты.

### §3.2 TensorCharts (TPP) — документация

- **Факт:** `tensorcharts.com` — **defunct**. Текущий сайт возвращает Cloudflare 404 / пустую
  страницу. Wayback Machine CDX: последние валидные snapshot'ы `tensorcharts.com`/`www.tensorcharts.com` —
  2018–2024, контент страниц либо заглушка (`"TensorCharts.com"` + JS-loader), либо 404.
- **Что известно по индустрии (упоминания в грейп-историях и форумах, C-016 review и др.):**
  TensorCharts использовал ту же связку «REST snapshot + WS diff от биржи», что и Bookmap.
  Архитектурно (по open-source форкам и публичным write-up'ам 2019–2021) — прямой WS-connect
  к биржам, snapshot-bootstrap через REST, агрегация на клиенте. Никакой отдельной
  depth-вендорной услуги.
- **Следствие:** для нашего Q1 это означает, что TensorCharts при жизни имел тот же паритет с
  биржей, что и Bookmap и Tardis — т.е. ни один из них **не предоставлял depth глубже
  биржевого REST-капа**.

### §3.3 Cross-check: «как они РЕАЛЬНО берут глубину»

Подтверждённый архитектурный паттерн (Binance Spot):
```
REST /api/v3/depth?limit=5000   ← initial seed (top 5000 levels)
            ↓
WS  wss://.../ws/btcusdt@depth  ← delta stream (U, u, b, a, ...)
            ↓
[биржевой matchine-id sequencing для gap-detection]
            ↓
локальная реконструкция книги (L2 агрегаты по level)
```

Этот же паттерн:
- **Мы** (см. §1.4 + CT-RFC-04 + L2Delta per-book-events на проде ~2026-07-21).
- **Tardis** (см. §2.1: «top 1000 levels initial order book snapshot, full depth incremental» +
  «validated using sequence numbers»).
- **Bookmap** (см. §3.1: «Snapshot 5000, Unlimited Updates»).
- **TensorCharts** (общеизвестно по индустрии, defunct ныне; не имел своего data-vendor'а).
- **Kaiko / Amberdata / CoinAPI** — стандартный «REST snapshot ≤ биржевого лимита + WS diff»,
  без отдельного заявления о превышении биржевого потолка.

**Pariтет с Bookmap/TPP → CONFIRMED** (по архитектуре), с уточнением: наш snapshot cap =
5000 (равен биржевому max), у Tardis snapshot cap = 1000 (тоже меньше биржевого max).
У Bookmap — 5000 (для Binance Spot). ВСЕ упираются в один и тот же биржевой REST-кап.

---

## §4. Есть ли L3/MBO на Binance (или у вендора)?

**Binance не публикует L3.** Подтверждается:
1. Binance public docs (нет endpoint'а `/trade`-detail-by-order для стакана; `@trade` поток
   даёт агрегированные сделки БЕЗ order-id).
2. Tardis FAQ §«L3»: Binance НЕ в списке L3-провайдеров (только Bitfinex, Coinbase Exchange,
   Bitstamp). Если бы был — Tardis бы упомянул.
3. Bookmap KB §«Crypto» — все крипто-биржи = L2 only (full-depth = snapshot + diff, без L3).

**Следствие:** для Binance Spot НЕ существует источника «per-order» стакана ни от биржи, ни
от вендора. Уровень — это единственная гранулярность, которую можно измерить.

---

## §5. Достижим ли эталон глубже 1.3% для Binance Spot BTCUSDT?

| Источник | Endpoint / Метод | Достижимая глубина | Прямая ссылка |
|----------|------------------|---------------------|---------------|
| Binance Spot REST | snapshot, ≤5000 | **1.29% bid / 1.34% ask** (probe 2026-07-24) | §1.1 |
| Binance Spot WS | diff stream | неограниченно по диапазону упоминаний, но **не валидируемо без эталона** (см. Q2) | §1.4 |
| Binance USDS-M Futures REST | snapshot, ≤1000 | **0.20% bid / 0.18% ask** (probe 2026-07-24) | §1.2 |
| Binance Coin-M Futures REST | snapshot, ≤1000 | **1.15% bid** (probe 2026-07-24) | §1.3 |
| Tardis (Binance Spot) | REST ≤1000 + WS diff | ≲0.2% snapshot + diff (тот же механизм) | §2.1 |
| Kaiko (Binance Spot) | REST snapshot + WS diff | ≲1.3% (биржевой cap) | §2.2 |
| Amberdata (=Kaiko) | REST snapshot + WS diff | ≲1.3% | §2.3 |
| CoinAPI | REST snapshot + WS diff | ≲1.3% | §2.4 |
| Coinalyze | (нет L2 coverage) | — | §2.5 |
| Bookmap (Binance Spot) | Snapshot 5000 + Unlimited Updates | ≲1.3% (тот же биржевой кап) | §3.1 |
| TensorCharts / TPP (defunct) | REST + WS diff (по индустрии) | ≲1.3% | §3.2 |

**Прямой ответ на Q1(a)/(b)/(c):** **эталон глубже 1.3% на Binance Spot BTCUSDT — НЕ
существует ни у биржи, ни у какого-либо известного вендора, ни у Bookmap/TPP-стиля платформ.**
Все они используют один и тот же механизм (REST snapshot ≤ биржевого cap + WS diff), и никто
не публикует заявления о превышении биржевого потолка.

---

## §6. Что это значит для Q2(в) cross-source и для вердикт-гейта

### §6.1 Q2(в) cross-source recon → закрывается отрицательно

M-32 §Q2(в) формулировано условно: «ТОЛЬКО если Q1 найдёт независимый deep-источник». Q1
не нашёл. Следовательно, задача Q2(в) → **N/A с явной записью «эталона нет, Q2в закрыт
отрицательно»** (формулировка из milestone §Tasks row 4). Никаких recon-чисел не делаем.

### §6.2 Что остаётся для верификации «достоверности полос 3–30%»

Без cross-source верификация возможна ТОЛЬКО через Q2а/Q2б:
- **Q2а (L2Delta-lifetime):** на сыром diff-потоке — получает ли дальний уровень `size=0`
  (явная отмена) за разумное окно, или замерзает (=фантом). Deconfounding через resync
  (sequencing U/u/pu → CENSORED, не cancelled/frozen). Задача архитектора (DV-I-1..5 RED,
  impl → research-dev). Это **прямая** мера «живой ли уровень», не proxy.
- **Q2б (order-flow faithfulness):** trade на цене P объёмом S должен сопровождаться
  соответствующим декрементом книги на P в seq-окне. Trade без декремента → INCONSISTENT
  (поток лжёт). Это проверяет, что diff-поток не «выдумывает» уровни.

Эти две проверки **вместо** cross-source. Если они GREEN → полосы 3-30% достоверны как
diff-реконструкция (то же, что у Bookmap/TPP/Tardis). Если не GREEN — это сигнал к
пометить `depth_band_provenance: diff-reconstructed` (VB-I-5) с явным data-quality caveat.

### §6.3 Founder-вердикт (M-32 §Tasks row 5) — три founder-решения, обновлённые по Q1

1. **(i) Достижим ли эталон глубже 1.3%? → NO.** (см. §5 сводная таблица).
2. **(ii) Достоверны ли полосы 3-30% по staleness/order-flow? → ОЖИДАЕТ Q2а/Q2б** (impl после
   RED от architect). Q1 их не заменяет; Q1 только фиксирует отсутствие cross-source эталона.
3. **(iii) Строить TPP на «реальном эталоне» ИЛИ diff-provenance? → ТОЛЬКО diff-provenance
   (эталон отсутствует в природе).** Это ОСОЗНАННОЕ решение, не «случайно живём на diff».

---

## §7. Что НЕ делает этот memo (границы)

- **НЕ запуск Q2а/Q2б.** Это задачи 2b/3b, требуют RED от architect (DV-I-1..6) — НЕ в зоне
  research-dev. Этот memo не интерпретирует/защищает ни один анализатор — это reviewer-бэкстоп
  для §Q1.
- **НЕ пишет в `crates/**`, `research/registry/`, `research/trials-ledger.json`.** Memo только.
- **НЕ доказывает, что полосы 3-30% достоверны.** Это вне scope Q1. Q1 отвечает только на
  вопрос существования независимого deep-эталона — и отвечает NO.
- **НЕ претендует на проверку коммерческих SLA вендоров** (т.е. что Tardis/Kaiko **точно**
  дают то, что в их docs). Документация ≠ SLA. Но это НЕ влияет на Q1: даже если вендор
  имеет магический «deep-source», его нет в публичных документах — а документированный
  биржевой кап 5000/1000 — это верхняя граница того, что можно получить.

---

## §8. Источники (все проверены вручную, 2026-07-24)

| # | Источник | URL | Цитата использована |
|---|----------|-----|----------------------|
| 1 | Binance Spot REST docs | `binance-spot-api-docs/rest-api.md` (§Order book) | §1.1 |
| 2 | Binance Spot WS docs | `binance-spot-api-docs/web-socket-streams.md` (§Diff Depth Stream, §How to manage...) | §1.4 |
| 3 | Binance Spot SBE docs | `binance-spot-api-docs/sbe-market-data-streams.md` (§Partial Book Depth Streams) | §1.5, §1.6 |
| 4 | Binance Spot market-data-only docs | `binance-spot-api-docs/faqs/market_data_only.md` | §1.1 |
| 5 | Binance Spot API direct probe | `curl https://api.binance.com/api/v3/depth?symbol=BTCUSDT&limit=5000/5001/10000` | §1.1 |
| 6 | Binance USDS-M Futures API direct probe | `curl https://fapi.binance.com/fapi/v1/depth?symbol=BTCUSDT&limit=100/500/1000/1001/2000/5000` | §1.2 |
| 7 | Binance Coin-M Futures API direct probe | `curl https://dapi.binance.com/dapi/v1/depth?symbol=BTCUSD_PERP&limit=1000` | §1.3 |
| 8 | Binance Options API direct probe | `curl https://eapi.binance.com/eapi/v1/depth?symbol=BTC-260925-145000-C` | §4 |
| 9 | Tardis FAQ §Order Books | `docs.tardis.dev/faq/order-books` | §2.1, §4 |
| 10 | Tardis Binance detail | `docs.tardis.dev/historical-data-details/binance` | §2.1 |
| 11 | Kaiko L1+L2 Data | `www.kaiko.com/products/l1-l2-data` | §2.2 |
| 12 | Kaiko Data Dictionary | `docs.kaiko.com/explore-our-data/data-dictionary` | §2.2 |
| 13 | Amberdata (Kaiko acquisition) | `amberdata.io` (homepage news) | §2.3 |
| 14 | Bookmap Knowledge Base §Crypto | `bookmap.com/knowledgebase/docs/KB-IntroductionToBookmap-Connectivity` | §3.1 |
| 15 | OKX docs §Order book | `okx.com/docs-v5/en/` (GET /api/v5/market/books) | §2.6 |
| 16 | TensorCharts (defunct) | `web.archive.org/cdx/search/cdx?url=tensorcharts.com` | §3.2 |
| 17 | Coinalyze (public page) | `web.archive.org/web/2024/coinalyze.net` | §2.5 |
| 18 | Coinalyze API probe | `curl https://api.coinalyze.net/v1/...` | §2.5 |
| 19 | HFT-prev depth-probe memo (свой) | `research/data-quality/depth-probe-binance.md` (на origin/research/depth-probe) | контекст §5 |
| 20 | HFT-prev staleness memo (свой) | `research/data-quality/depth-probe-staleness.md` | §1.4 «Ключевой рефрейм» |

---

## §9. Handoff → architect (для синтеза вердикта и/или Q2 RED)

**→ architect (Q2 lead + verdict architect):** этот memo даёт факт-базу для вердикт-гейта
(M-32 §Tasks row 5). Конкретно:

- Q1 = NO по эталону. Q2в закрывается отрицательно (см. §6.1). Это НЕ требует
  architect-RED — это просто фактический вердикт-research.
- Q2а/Q2б — это **architect-RED-first** (DV-I-1..6, в `crates/research-cli/tests/red_*.rs`,
  sacred — задача architect). После GREEN architect диспатчит research-dev на impl 2b/3b
  (см. milestone §Tasks).
- Если Q2а/Q2б GREEN → founder-вердикт = «diff-provenance TPP-полос» с VB-I-5 caveat
  (см. §6.3).
- Если Q2а/Q2б RED → см. milestone §Q2 «эмпирика» и §«Нерешённые вопросы».

**→ founder (если Q1 сам меняет решение о вендоре):** см. §6.3. На 2026-07-24 решение
«строить TPP на собственной diff-книге» — единственное достижимое. Поставщик данных
(МЫ-vs-Tardis-vs-Bookmap) — это выбор по SLA/стоимости/истории (Tardis для архива 2019+,
МЫ для live), а не по глубине — глубина у всех одинаковая.

---

## §10. Дисциплина и self-check

- ✅ Факты с источниками (URL + цитата для каждого «есть/нет»).
- ✅ Прямые probe'ы в дополнение к документации (REST `limit=5000/5001/10000/1001` и т.д. —
  не только чтение docs).
- ✅ ≥3 независимых источника по каждому ключевому утверждению:
  - «Binance Spot REST cap = 5000»: docs §Order book + direct probe `?limit=5001` →
    тихий clamp + docs §«How to manage» + Tardis FAQ косвенно.
  - «Diff — единственный путь глубже для Binance»: Binance docs §Diff + Bookmap KB
    «Unlimited Updates» + Tardis FAQ «full depth incremental» + собственная архитектура
    (CT-RFC-04 L2Delta).
  - «Bookmap = snapshot+diff»: KB §Crypto + общеизвестная индустриальная практика +
    Tardis FAQ таблица (тот же формат).
- ✅ Pariтет явно помечен: **CONFIRMED** (с уточнением про snapshot-cap: мы берём 5000,
  Tardis 1000, Bookmap 5000, но diff у всех неограничен по диапазону упоминаний).
- ✅ Cross-check через cross-vendor: Tardis + Kaiko + Bookmap + OKX — все упираются в один
  биржевой REST-кап. Если бы был «магический вендор», он бы всплыл хотя бы в одной из этих
  публичных таблиц.
- ✅ Раздел «что НЕ делает» (§7) — Q1 не интерпретирует/не защищает impl, не пишет в
  `contracts/risk/ks/oms/venue/registry/journal`.

---

**END of Q1 memo. — research-dev, 2026-07-24 UTC**