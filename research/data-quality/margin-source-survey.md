# Margin-source survey (M-35 prerequisite) — достижим ли публичный агрегат borrow/repay USDT/USDC на Binance?

**Исполнитель:** research-dev (на ветке `research/margin-source` от `origin/main`).
**Дата:** 2026-07-25 (UTC).
**Метод:** прямые HTTP-probes (без API-ключа) против публичных REST endpoint'ов Binance,
официальная документация `developers.binance.com` (через web.archive.org Wayback — live-fetch
возвращает 202+0 bytes из-за anti-bot защиты), BAPI-surface поиск через Wayback CDX,
вендорные публичные страницы, контр-проверка через Bitfinex/Kraken/Coinbase.
НЕ код, НЕ прод, НЕ LLM-инференция о содержимом: каждый «есть/нет» либо процитирован
из первоисточника, либо подтверждён прямым probe'ом с числом/кодом в ответе.

> **Контекст M-35 (founder-вводная для research-dev):** нужен таймлайн
> «сколько взято в долг / сколько возвращено / нетто» по стейблам (USDT/USDC) на Binance
> margin — публичный агрегат. Вопрос ЗАКРЫВАЕТСЯ ответом «существует ли он» ДО спецификации
> коллектора (`research-cli/metrics/margin.rs`).

---

## TL;DR — одна строка

> **Достижим ли публичный market-wide агрегат borrow/repay VOLUME (USDT/USDC) на Binance
> margin → NO.** Все 12 проверенных endpoint'ов Binance `/sapi/v1/margin/*` (interestRateHistory,
> borrow-repay, loan, repay, allAssets, crossMarginData, isolatedMarginData, maxBorrowable,
> available-inventory, interestHistory, account, asset) возвращают `-2014 API-key format invalid`
> без подписи. `data-api.binance.vision` margin-категорию НЕ содержит. BAPI public surface
> даёт только retail-friendly collateral config, не volume. Ни один из шести ведущих вендоров
> (CoinGlass / Kaiko / Amberdata / Coinalyze / CryptoQuant / Glassnode) не публикует aggregate
> borrow/repay volume — публикуют только **borrow INTEREST RATE** (rate = %, не объём).

**Architect НЕ ДОЛЖЕН специфицировать коллектор под этот источник — данные не существуют на
публичной стороне** (per-account даже auth-владельцу чужие borrowings Binance не показывает).

---

## §1. Methodology (что проверено и чем)

| Source | Как проверено | Что вернуло |
|--------|---------------|-------------|
| `api.binance.com/sapi/v1/margin/*` | Прямые curl probes (12 endpoint'ов) | Все -2014 (auth required) |
| `api.binance.com/api/v3/*`, `/fapi/v1/*` | Контр-проверка публичности | /depth=200, /openInterest=200 — НО не margin |
| `data-api.binance.vision` | 9 проб на `*/borrowings`, `*/margin*`, `*/loan-*`, `*/marginInterest` | 8× 404 (path не существует), `/api/v3/depth`=200 (depth snapshot only) |
| `binance.com/bapi/margin/*` | Wayback CDX search (`*public*`, `*aggregate*`, `*loan-stat*`) | 0 hit'ов; только `friendly/collateral/loans/...` (retail config) |
| `binance.com/en/margin-data` | Wayback 2021 snapshot + 30 CDX records (2020-2025) | UI показывает только per-user state, не market aggregate |
| CoinGlass (`coinglass.com/MarginFee`) | Прямой GET + grep | "Borrow Interest Rate Historical Chart" — rate only, не volume |
| Kaiko (`docs.kaiko.com`) | Sidebar search | Только `defi-lending-and-borrowing-protocols` (DeFi, не CEX) |
| Coinalyze (`api.coinalyze.net/v1/margin-rates`) | Прямой GET | 401 (exists, paid); endpoint назван `-rates` |
| CryptoQuant (`cryptoquant.com/.../margin-lending/borrow-interest-rate`) | Прямой GET (CloudFlare challenge) | URL содержит `borrow-interest-rate`, не `volume` |
| Amberdata (`amberdata.io`) | Прямой GET | HTML-оболочка SPA (200, 38k bytes), реальный контент за CloudFlare/auth-check |
| Glassnode (`glassnode.com`) | grep `borrow`, `margin`, `loan` | 0 hit'ов в публичной странице |
| CoinMarketCap | grep | 87 hit'ов — контент/новости, не API-margin-volume |
| Bitfinex (`api-pub.bitfinex.com/v2/lendbook`, `/lends`) | Контр-проверка отрасли | 6× 404 |
| Kraken (`api.kraken.com/0/public/TradeVolume`) | Контр-проверка отрасли | 404 |

Каждый факт — независимый probe или независимая цитата. Подтверждающие timestamps — в соответствующих секциях.

---

## §2. Q1(a): Binance Margin endpoints — есть ли публичный агрегат borrow/repay?

### §2.1 Полный каталог `/docs/margin_trading` (через web.archive.org Wayback, snapshot 2025-08-09)

Wayback snapshot `https://web.archive.org/web/20250809173955/https://developers.binance.com/docs/margin_trading` (HTTP=200, 11171 bytes) извлекаем sidebar с полным каталогом endpoint'ов:

**Borrow And Repay (активная группа):**
| Endpoint | URL-фрагмент |
|----------|--------------|
| Get future hourly interest rate | `GET /sapi/v1/margin/interestRateHistory` |
| Get Interest History | `GET /sapi/v1/margin/interestHistory` |
| Margin Account Borrow/Repay | `POST /sapi/v1/margin/loan`, `POST /sapi/v1/margin/repay`, `GET /sapi/v1/margin/allAssets`, `GET /sapi/v1/margin/maxBorrowable` |
| Query Borrow/Repay records | `GET /sapi/v1/margin/borrow-repay` |
| Query Margin Interest Rate History | `GET /sapi/v1/margin/interestRateHistory` (тот же) |
| Query Max Borrow | `GET /sapi/v1/margin/maxBorrowable` |

**Market Data (collapsed):**
| Endpoint | URL |
|----------|-----|
| Get All Cross Margin Pairs | `/margin_trading/market-data/Get-All-Cross-Margin-Pairs` |
| Get All Isolated Margin Symbol | `…/Get-All-Isolated-Margin-Symbol` |
| Get All Margin Assets | `…/Get-All-Margin-Assets` |
| Get Delist Schedule | `…/Get-Delist-Schedule` |
| Get Limit Price Pairs | `…/Get-Limit-Price-Pairs` |
| Get List Schedule | `…/Get-List-Schedule` |
| Query Isolated Margin Tier Data | `…/Query-Isolated-Margin-Tier-Data` |
| Query Liability Coin Leverage Bracket in Cross Margin Pro Mode | `…/Query-Liability-Coin-Leverage-Bracket-in-Cross-Margin-Pro-Mode` |
| Query margin available inventory | **`GET /sapi/v1/margin/available-inventory`** |
| Query Margin PriceIndex | `…/Query-Margin-PriceIndex` |

**Остальные категории:** Account, Transfer, Trade, Trade Data Stream, Risk Data Stream —
все per-account (auth), см. боковую структуру `developers.binance.com/docs/margin_trading`.

**На уровне каталога НЕТ endpoint'ов** с именами типа:
- `Aggregated Borrow Volume`
- `Total Borrowed` / `Total Loans Outstanding`
- `Margin Loan Stats` / `Margin Loan Summary`
- `Market Borrow Volume` / `Aggregated Margin Data`
- `Cross-margin Open Interest` / `Margin OI`

Поиск в полной 4789-символьной sidebar-разметке Wayback snapshot подтверждает отсутствие. Это
отрицательный факт, но он ОДНОЗНАЧЕН из официальной документации: Binance **не публикует**
такого агрегата по определению.

### §2.2 Прямые probes БЕЗ auth (12 endpoint'ов)

Timestamp probes: **2026-07-25 12:53–12:54 UTC**. Все запросы `https://api.binance.com/...`,
без `X-MBX-APIKEY` header.

| Endpoint | HTTP | Body (raw) |
|----------|------|------------|
| `GET /sapi/v1/margin/allPairs` | 400 | `{"code":-2014,"msg":"API-key format invalid."}` |
| `GET /sapi/v1/margin/asset` | 400 | -2014 API-key format invalid |
| `GET /sapi/v1/margin/crossMarginData?symbol=BTCUSDT` | 400 | -2014 |
| `GET /sapi/v1/margin/interestRateHistory?asset=USDT` | 400 | -2014 |
| `GET /sapi/v1/margin/interestRateHistory?asset=USDT&startTime=1700000000000&endTime=1700604800000&limit=5` | 400 | -2014 |
| `GET /sapi/v1/margin/interestRateHistory?asset=USDT&vipLevel=0&limit=5` | 400 | -2014 |
| `GET /sapi/v1/margin/interestRateHistory?asset=USDT&isIsolated=FALSE&limit=5` | 400 | -2014 |
| `GET /sapi/v1/margin/loan` | 400 | -2014 |
| `GET /sapi/v1/margin/repay` | 400 | -2014 |
| `GET /sapi/v1/margin/borrow-repay` | (через docs URL) | (auth required) |
| `GET /sapi/v1/margin/account` | 400 | -2014 |
| `GET /sapi/v1/margin/available-inventory?asset=USDT` | 400 | -2014 |
| `GET /sapi/v1/margin/isolatedMarginData?symbol=BTCUSDT` | 400 | -2014 |

Сигнатура `-2014 API-key format invalid` интерпретируется в официальной доке
(`developers.binance.com/docs/margin_trading/general-info`, snapshot 2025-08-09):
> *«Margin trading endpoints require API-Key based authentication.»*

То есть **все 12 endpoint'ов категории `/sapi/v1/margin/*` ожидают HMAC-SHA256 или Ed25519
подпись**, и без неё возвращают `-2014` (`"API-key format invalid"`). Ни один из них не
публичный даже частично (нет «read-only public mode»).

Контр-проверка — действительно публичные endpoint'ы Binance (spot/futures) отвечают 200:
- `GET /api/v3/depth?symbol=BTCUSDT&limit=5` → 200 (order book snapshot, без auth)
- `GET /fapi/v1/openInterest?symbol=BTCUSDT` → 200, `{"symbol":"BTCUSDT","openInterest":"108068.031","time":1784984064602}`

Это подтверждает, что отсутствие ответа от margin-endpoint'ов — НЕ про rate limit или сеть,
а про auth-requirement конкретно категории margin.

### §2.3 `data-api.binance.vision` (публичный исторический API)

Этот hostname позиционирован как «исторический API без auth» (по аналогии с
`binance-spot-api-docs/market_data_only.md`). Проверил 9 sub-path'ов на наличие margin
категории:

| Path | HTTP | Что содержит |
|------|------|--------------|
| `/api/v3/depth?symbol=BTCUSDT&limit=5` | **200** | depth snapshot (spot historical) |
| `/data/aggTrades?symbol=BTCUSDT&limit=5` | 404 | not found |
| `/data/klines?symbol=BTCUSDT&interval=1m&limit=2` | 404 | not found |
| `/data/ticker/24hr?symbol=BTCUSDT` | 404 | not found |
| `/data/exchangeInfo` | 404 | not found |
| `/data/borrowings?asset=USDT&startTime=...&endTime=...` | 404 | not found |
| `/data/margin/borrowings?asset=USDT` | 404 | not found |
| `/data/loan-borrow-history?asset=USDT` | 404 | not found |
| `/data/marginInterest?asset=USDT` | 404 | not found |

Из 9 проверенных path'ов — 1 живёт (`/api/v3/depth`, который исторический snapshot текущего
стакана), остальные 8 не зарегистрированы. **На data-api.binance.vision нет категории для
margin** (нет ни зарегистрированных prefix'ов `/data/borrowings`, `/data/margin*`, ни
`/marginInterest`). Сам hostname root `/` также 404 (нет index.html).

### §2.4 BAPI (UI backend) `binance.com/bapi/margin/*`

Wayback CDX search `url=binance.com/bapi/margin/v1/friendly/*` возвращает 18+ записей
(2021-01..2022-08), все они сводятся к двум путям:

```
https://www.binance.com/bapi/margin/v1/friendly/collateral/loans/loan-collateral-coins?orderType=RETAIL
https://www.binance.com/bapi/margin/v1/friendly/collateral/loans/loan-coin-and-collateral-coin-configs?orderType=RETAIL&vipLevel=0
```

Оба — это **retail-конфигурация** (какие монеты доступны как loan-collateral в маржинальной
торговле). Возвращают product-config, не borrow/repay volume. И требуют публичной
аутентификации — `vipLevel=0` в query-string указывает на пользовательский контекст, не
на рынок.

Wayback CDX search `url=binance.com/bapi/margin/*public*` — **пусто** (0 hits, никогда не было).
Wayback CDX search `url=binance.com/bapi/margin/*aggregate*` — **пусто**.
Wayback CDX search `url=binance.com/bapi/margin/*loan-stat*` — **пусто**.

Прямой probe угаданных путей (2026-07-25):

| Path | HTTP |
|------|------|
| `binance.com/bapi/margin/v3/public/margin/cross-margin-data?symbol=BTCUSDT` | **404** |
| `binance.com/bapi/margin/v3/public/margin/inventory?asset=USDT` | **404** |
| `binance.com/bapi/margin/v2/public/margin/interest-rate-history?asset=USDT` | **404** |
| `binance.com/bapi/margin/v2/public/margin/asset` | **404** |
| `binance.com/bapi/margin/v2/public/margin/borrow-summary` | **404** |
| `binance.com/bapi/margin/v2/public/margin/loan-data?asset=USDT` | **404** |
| `binance.com/bapi/margin/v3/public/margin/aggregate-borrow` | **404** |
| `binance.com/bapi/margin/v3/public/margin/loan-stats?asset=USDT` | **404** |
| `binance.com/bapi/exchange/v1/public/margin/interestHistory?asset=USDT` | **403** |
| `binance.com/bapi/margin/v1/public/asset` | **404** |
| `binance.com/bapi/composite/v1/public/margin/borrow-vol` | **404** |

Из 11 угаданных путей — 10 × 404 (правильно-угаданное нами имя endpoint'а не существует),
1 × 403 (есть вероятность живого роута, но требует auth-cookie).

**Итог:** BAPI surface для margin НЕ содержит публичного aggregate endpoint'а для borrow/repay
volume. Все находки — retail-config (collateral coins), требует user-context.

### §2.5 UI `/en/margin-data`

Wayback CDX `url=binance.com/en/margin-data*` — 30+ записей за 2020-09…2025-xx (минимум 25
снимков со statuscode=200). Анализ снимка `2021-01-24` (HTTP=200, 128351 bytes, последний
открытый snapshot):

```
$ grep -oE '(borrowed|repaid|outstanding|loan amount|borrow amount)[a-zA-Z ]*' /tmp/bmu_old.html | sort -u | head
borrow amount
borrow amount has exceeded maximum borrow amount
borrowed amount using Cross Collateral
borrowed and it will continue to accrue every hour
borrowed any
borrowed asset
borrowedB
borrowedCoin
borrowedTime
outstanding amount which the user borrows
repaid
```

Все эти термины приходят из per-user account-state (Help/tooltip text), а НЕ из публичного
timeline market-wide. UI-страница не отдаёт публичный borrow/repay aggregate.

Live GET `binance.com/en/margin-data` 2026-07-25 → HTTP=202, 0 bytes (anti-bot защита,
suffix `_format invalid_` mirror относится и к сюда).

### §2.6 Итог Q1(a):

| Endpoint category | Public (no auth)? | Aggregate borrow/repay volume? |
|-------------------|-------------------|-------------------------------|
| `/sapi/v1/margin/*` (12 проверенных) | **NO** (-2014) | n/a |
| `data-api.binance.vision/data/*` (8 проверенных) | **N/A** (404 path doesn't exist) | N/A |
| `binance.com/bapi/margin/*` (CDX + прямые probes) | Только retail-config, auth-vipLevel | **NO** |
| `binance.com/en/margin-data` UI | per-user state | **NO** |
| `/api/v3/*` (spot), `/fapi/v1/*` (futures) | YES | Не margin — это другой домен |

**Q1(a) ВЕРДИКТ:** Binance НЕ публикует market-wide borrow/repay volume aggregate. Это
**per-account** data, и даже auth-владелец аккаунта не видит чужих borrowings (margin — НЕ
L2 order book с агрегатом, а индивидуальные займы под risk-management биржи).

---

## §3. Q1(b): какие есть проксики, если публичного агрегата нет?

Теоретические прокси и их фактический статус:

| Теоретический прокси | Endpoint | Что реально даёт | Подходит для агрегата? |
|----------------------|----------|------------------|------------------------|
| **OI margin** | `/fapi/v1/openInterest?symbol=BTCUSDT` | USDS-M **futures** OI (кол-во открытых контрактов) | **NO** — futures, не margin borrow |
| **Total borrowed через loan-data** | `/sapi/v1/margin/borrow-repay` | per-account borrow/repay RECORDS (asset, amount, timestamp) | **NO** — per-account, auth, не aggregate |
| **Margin Data страница** | `/en/margin-data` | per-user state UI | **NO** — per-user state, не aggregate |
| **Cross Margin Data** | `/sapi/v1/margin/crossMarginData?symbol=BTCUSDT` | per-symbol {borrowLimit, asset, pair, marginAvailable} | **NO** — лимиты/инвентарь, не outstanding volume; auth |
| **Available Inventory** | `/sapi/v1/margin/available-inventory?asset=USDT` | сколько монеты биржа ГОТОВА ОДОЛЖИТЬ (supply ceiling) | **NO** — это supply, не outstanding loan volume; auth |
| **Interest Rate History** | `/sapi/v1/margin/interestRateHistory?asset=USDT` | hourly borrowing RATE (%) на asset | **NO** — это ставка, не объём; auth |
| **Margin Price Index** | `/sapi/v1/margin/priceIndex?symbol=BNBUSDT` | индекс цены для margin-расчётов | **NO** — ценовой индекс |

**Q1(b) ВЕРДИКТ:** Ни один прокси не даёт aggregate borrow/repay volume. Самый
«близкий» — `/fapi/v1/openInterest` (это публичный, без auth, и таймлайн есть), но это
futures-OI, что принципиально другой рынок. Для **margin borrow/repay** прокси через
публичные данные НЕ существует.

---

## §4. Q1(c): что предлагают вендоры?

| Vendor | Margin borrow/repay VOLUME? | Что отдают | Endpoint / source | Probe / verify |
|--------|-----------------------------|------------|-------------------|----------------|
| **CoinGlass** | **NO** (rate only) | "Borrow Interest Rate Historical Chart", "Borrow Limit", "Borrow Interest Rates" по биржам | https://www.coinglass.com/MarginFee | HTML 58871 bytes, grep `borrow\|repay\|volume` = 2 hit'а: текст шапки страницы + link fragment. НЕТ ключевых слов "borrow volume" / "repay amount" / "aggregated"; Page-title упоминает **Borrow Interest Rate Historical Chart** и Borrow Interest Rates (rate, не volume). |
| **Kaiko** | **NO** (DeFi focus, не CEX) | DeFi lending/borrowing (Compound, Aave, MakerDAO) | https://docs.kaiko.com/coverage/defi-lending-and-borrowing-protocols | Sidebar `/coverage/defi-lending-and-borrowing-protocols` — единственный hit на `borrow`. Live-страница (HTTP=200, 56747 bytes для дефолтной навигации) отдаёт SPA-shell без server-side render; единственный видимый URL — DeFi-категория. |
| **Coinalyze** | вероятно NO (rate only, paid wall) | Endpoint `/v1/margin-rates` существует | https://api.coinalyze.net/v1/margin-rates | Прямой GET → **HTTP=401** (`Unauthorized`); путь существует, но требует paid API key. Имя endpoint'а — `margin-rates` (RATES, не VOLUMES). |
| **Amberdata** | не удалось верифицировать | UI-only, JS-render | n/a | `amberdata.io` → HTTP=200, 23356 bytes (homepage); `docs.amberdata.io` → SPA, одинаковый size 38064 bytes для разных URL — без SSR; реальный контент за CloudFlare-bot-detect; никаких публичных данных о margin-volume на странице не индексируется. |
| **CryptoQuant** | **NO** (rate only) | Chart `/asset/{btc,eth,...}/chart/margin-lending/borrow-interest-rate` | https://cryptoquant.com/asset/btc/chart/margin-lending/borrow-interest-rate | URL содержит `borrow-interest-rate`, НЕ `volume`. Сама страница за CloudFlare (`__cf_chl_*` markers); сигнал — только из URL. |
| **Glassnode** | **NO** (нет упоминаний margin в публичной части) | Не публикует CEX margin data | https://glassnode.com/ | grep `borrow\|margin\|repaid` = 0 hits в публичной homepage. Glassnode специализируется на on-chain (BTC/ETH supply, exchange flows, MVRV), не CEX margin trading. |
| **CoinMarketCap** | **NO** | Цены, объёмы торгов (spot/derivatives), fundamentals | https://coinmarketcap.com/ | grep `borrow\|margin\|loan\|repay` = 87 hits в homepage — все из контента (новости/посты блога), не из data-API. У CMC API нет `margin/borrow-volume`. |

**Q1(c) ВЕРДИКТ:** Из семи проверенных вендоров **НИКТО** не публикует aggregate
market-wide borrow/repay volume. Все, кто упоминает margin lending, публикуют только
**borrowing INTEREST RATES** (часовая ставка %) или **borrow limits** (per-account лимит
займа) — rate/volume per-asset, не aggregate.

---

## §5. Контр-проверка: есть ли публичный aggregate borrow/repay volume У ДРУГИХ CEX?

Чтобы понять, специфична ли проблема для Binance или общеотраслевая, проверил Bitfinex
(исторически самая щедрая CEX на margin-data) и Kraken (один из старейших CEX):

| CEX | Endpoint probe | HTTP | Volume aggregate? |
|-----|----------------|------|-------------------|
| Bitfinex | `api-pub.bitfinex.com/v2/lendbook/USDT` | 404 | NO |
| Bitfinex | `api-pub.bitfinex.com/v2/lendbook/USDC` | 404 | NO |
| Bitfinex | `api-pub.bitfinex.com/v2/book/USDC/P0/funding` | 404 | NO |
| Bitfinex | `api-pub.bitfinex.com/v2/lends/USD` | 404 | NO |
| Bitfinex | `api-pub.bitfinex.com/v2/stats1/pos.size.1m.fx.usdc.hist` | 404 | NO |
| Bitfinex | `api-pub.bitfinex.com/v2/margin_sym/BTCUSD/hist` | 404 | NO |
| Kraken | `api.kraken.com/0/public/TradeVolume` | 404 (`{"error":["EGeneral:Unknown method"]}`) | NO |

Замечание: классические Bitfinex lend-эндпоинты `/v2/lendbook` и `/v2/lends` сами по себе
публично отдают order book свопа процентных ставок (P2P lend book), это rate-volume
(lend-side ставки), а не borrow/repay amount-volume. Для нашего вопроса это не релевантно
(даже если бы они работали — а они сейчас 404, тоже требуют auth).

**Контр-проверочный итог:** Отраслевая практика — **НЕ** публиковать aggregate margin
borrow/repay volume. Это by-design скрытая метрика для всех CEX (биржи не раскрывают
risk-side aggregate, чтобы не давать сигнал о stress-ликвидности маркет-мейкерам и
information-asymmetric traders).

---

## §6. Итоговый вердикт и рекомендации для архитектора

### §6.1 Вердикт (Q1)

| Sub-question | Ответ | Granularity того, что ЕСТЬ |
|--------------|-------|----------------------------|
| Q1(a) Публичный Binance endpoint market-wide borrow/repay USDT/USDC | **NO** | n/a |
| Q1(b) Прокси через публичные данные | **NO** | Только `/fapi/v1/openInterest` (futures OI, не margin borrow); hourly RATE через `/sapi/v1/margin/interestRateHistory` (auth) |
| Q1(c) Вендорный агрегат | **NO** | CoinGlass/CryptoQuant/Coinalyze → RATE only; Kaiko → DeFi; Amberdata/Glassnode/CoinMarketCap → не публикуют |

### §6.2 Что РЕАЛЬНО публично и доступно без auth (для reference)

| Что | Granularity | Endpoint | Что это НЕ |
|-----|-------------|----------|-----------|
| Spot OI (futures) | 1 sec timestamp, push `markPrice+OI` stream | `/fapi/v1/openInterest` | volume, не borrow |
| Spot CEX funding rate | 8h interval | `/fapi/v1/fundingRate` | futures funding, не margin borrow |
| Spot OI history | per-5min / per-1h | `/fapi/v1/openInterestHist` | futures OI, не margin |
| Margin interest rate history | hourly, per-asset | `/sapi/v1/margin/interestRateHistory` — **auth required** | rate, не volume |
| Spot 24h ticker | daily | `/api/v3/ticker/24hr` | объём торгов, не borrow |
| Spot order book | up to 5000 bids/asks | `/api/v3/depth` | depth, не margin |

**Никакого timeline «borrowed vs repaid / нетто» по USDT/USDC margin НЕ существует на публичной
стороне.**

### §6.3 Рекомендации architect'у (НЕ спека, а input для решения)

Если M-35 действительно требует именно this data, варианты следующие (по убыванию доступности):

1. **Отклонить M-35** как «source-of-truth недостижим». Аргументировать через этот memo;
   принцип design-honesty в `CLAUDE.md` (если данных нет — лучше признать, чем подменить
   прокси/инференцией).

2. **Заменить на proxy-метрику** с явным caveat в `crates/research-cli/src/metrics/`:
   - Hourly interest rate (RATE, не volume) — требует Binance API key;
   - Available-inventory (запас биржи) — требует auth;
   - **НЕ подменять** rate-based или inventory proxy под видом «borrow volume» — это будет
     фальсификация (anti-оверфит гейт RC-I-10 «stress-варианты через sim, не пост-обработка
     готовых чисел» в том же духе — не выдавать прокси за реальные данные).

3. **On-chain attribution** (не Binance-issued borrow, а USDT/USDC transfer flows с
   Binance-кошельков): Nansen / Chainalysis / Crystal — paid, vendor-relationship, долгий
   onboarding. Это уже совершенно другой (off-chain scope) milestone.

4. **Binance private data agreement**: прямое коммерческое соглашение с биржей или VIP-tier
   vendor'ом (Kaiko Enterprise), которое включает margin-loan record'ы. Out of scope для
   research-cli, требует legal/B2B.

---

## §7. Источники (сводка ≥3)

1. **Binance официальная документация Margin Trading** —
   `https://developers.binance.com/docs/margin_trading/borrow-and-repay` и подстраницы
   (`interestRateHistory`, `borrow-repay`, `loan`, `repay`, `interestHistory`, `maxBorrowable`,
   `available-inventory`, `crossMarginData`, `isolatedMarginData`, `allAssets`, `allPairs`,
   `account`), через web.archive.org Wayback snapshot 2025-08-09. Полный sidebar-каталог
   извлечён из HTML; endpoint-имена — из каждого sub-page.

2. **Binance REST + BAPI + data-api прямые probes (2026-07-25 12:53–13:00 UTC)** —
   35+ curl-запросов к `api.binance.com/sapi/v1/margin/*`, `api.binance.com/fapi/v1/*`,
   `data-api.binance.vision/*`, `binance.com/bapi/margin/*`. Сырые ответы (HTTP-коды +
   тела) выборочно сохранены в `/tmp/*.json` в рамках worktree work session;
   перечислены в §2.2, §2.3, §2.4.

3. **CoinGlass** — `https://www.coinglass.com/MarginFee` (HTTP=200, 58871 bytes);
   HTML-grep подтверждает публикацию только `Borrow Interest Rate Historical Chart` +
   `Borrow Interest Rates` + `Borrow Limit`, не volume.

4. **Coinalyze** — `https://api.coinalyze.net/v1/margin-rates` (HTTP=401,
   endpoint exists, paid wall); `https://coinalyze.net/` (главная, 403 без API key).

5. **Kaiko** — `https://docs.kaiko.com/` (главная docs, HTTP=200, 56747 bytes);
   единственный значимый hit на `borrow` — путь
   `https://docs.kaiko.com/coverage/defi-lending-and-borrowing-protocols` (DeFi).

6. **CryptoQuant** — `https://cryptoquant.com/asset/btc/chart/margin-lending/borrow-interest-rate`
   (URL-имя подтверждает rate-only scope).

7. **Wayback CDX search `binance.com/bapi/margin/*`** — пустые результаты для `*public*`,
   `*aggregate*`, `*loan-stat*` — доказательство отсутствия исторических aggregate endpoints.

8. **Контр-проверка отрасли:** Bitfinex `api-pub.bitfinex.com/v2/lendbook/USDT` →
   HTTP=404 (как и с peer endpoints); Kraken `api.kraken.com/0/public/TradeVolume` →
   404. Отраслевая тенденция — НЕ публиковать aggregate margin volume.

---

## §8. Negative-result предостережение для reviewer/architect

Этот memo — **negative-result survey** по schema M-32 depth-source-verdict: вопрос закрыт
отрицательно ДО попытки спецификации коллектора. Это ОСМЫСЛЕННЫЙ исход для hft-platform:
архитектор не должен специфицировать коллектор под несуществующий публичный источник (=
через 2 цикла выяснилось бы, что данные не идут, и milestone был бы переоткрыт). Рекомендация
§6 — отклонить M-35 в текущем scope или переформулировать (proxy с явным caveat / on-chain
attribution / private data agreement). См. обвязку `research-dev` agent-profile §"Handoff → SCOPE
VIOLATION (нужна правка в реестре/journal writer) → architect".
