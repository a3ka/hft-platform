# FA — viz-backend: дериватив-слой, read-gateway, экспорт для кокпита (Слой 8)

STATUS: FA v1 (2026-07-22, architect). Планово-архитектурный документ виз-бэкенда для
**Order Flow Intelligence Terminal** (Bookmap+TPP+AI кокпит). Реализация — отдельными milestone'ами.
Источники: `docs/07-cockpit-backend-roadmap.md` (решения сессии), `docs/06-data-layer-and-storage.md`
(даталеер — на котором ЭТОТ слой стоит), `docs/05-contract-layer.md` (контракт-governance),
`research/exports/format.md` (export v1, M-17), `research/data-quality/depth-probe-*.md` (глубина книги).

---

## §1. Место слоя: ПОВЕРХ даталеера, read-only

Виз-бэкенд — **консюмер журнала**, не его писатель. Он НЕ трогает recorder/journal-writer
(`docs/06` §1 journal-first; RC-I-7). Вся связь с данными — через **`journal::stream(dir, EpochFilter)`**
(`docs/06` §5, bounded-memory итератор) и `crates/book` (реконструкция стакана). Это **Граница A**
(`docs/03` §4): всё ниже — чистые редьюсеры над потоком `Event`, без wall-clock/rand/I-O в расчёте.

**Почему так (BINDING):** journal-first + детерминизм (`DET-I-1`, `docs/06` §5, `DESIGN` §1) дают
**live == replay бесплатно** — один код редьюсеров, live кормит хвост журнала → бит-идентичный вывод.
Это фундамент replay-tutor'а и «AI видит то же, что человек».

**Граница market-плоскость ↔ app-плоскость (`docs/07` §5 D6, добавлено 2026-07-23).** Виз-бэкенд —
ТОЛЬКО market-плоскость: `gateway-serve` (транспорт, D1) держит WS, тейлит журнал, отдаёт
snapshot/frames/replay — **read-only, детерминированный, stateless по юзеру**. Он **НЕ владеет** аккаунтами,
стратегиями, чатами, настройками — это **application-плоскость** (Next.js + Postgres, зона founder'а).
Единственная связь: `gateway-serve` **ВЕРИФИЦИРУЕТ** короткоживущий подписанный JWT (выпущенный Next.js;
крейт `jsonwebtoken`, HS256/Ed25519) на WS-connect — stateless проверка подписи, БЕЗ обращения в user-БД.
**Инвариант (VB-I-9):** gateway не читает/не пишет application-БД; user-состояние (стратегии/чаты/настройки)
живёт в Postgres, НЕ в market-журнале (`DET-I-1` цел, тот же класс, что `AI-I-1`).

## §2. Дериватив-слой: индикаторы как чистые редьюсеры

Каждый виз-индикатор = функция `(&[Event] | stream) -> Series`, детерминированная:

| Индикатор | Вход (journal) | Статус |
|---|---|---|
| OHLCV, footprint-δ, **CVD**, footprint-bins, depth-series | Trade+side / L2 | ✅ есть (M-17 export v1) |
| **Heatmap / COB / Volume Bubbles** | L2Delta (M-18) / Trade | 🟡 новый (M-23) |
| **Volume Profile** (SVP/CVP/FRVP/Anchored/Composite: POC/VAH/VAL/HVN/LVN, VA%) | Trade | ❌ новый (M-24) |
| **VWAP** (session/anchored, HLC3, σ-полосы) | Trade | 🟡 M-20 (QUEUED) |
| **Liquidations/OI/Funding профили** | Liquidation/OI/Funding | 🟡 новый (M-25) |
| **TPP Bid/Ask, Delta** (полосы 1.5/3/5/8/15/30%, per side, COIN-scope) | L2 (book depth) | 🔴 data-quality gate (§4) |
| TPP TOTAL/TOTAL1-3/OTHERS, Secrets (MLSP/Margin/Ratio/Speed) | — | ⛔ `formula_pending` (founder-спека) |

**Модель сессии** (00:00 UTC anchor/reset, session templates) — общий примитив для VWAP / SVP / CVD-reset;
живёт в дериватив-слое (не в journal — это интерпретация, не факт).

## §3. Три подсистемы

```
   ┌─ A. INDICATOR ENGINE ──────────┐   ┌─ B. READ GATEWAY (live+replay) ─┐
   │ чистые редьюсеры над journal    │   │ хвост journal → редьюсеры →      │
   │ → серии (детерминированы)       │──▶│ снапшот(REST) + WS-push + replay │──▶ фронт code2alpha
   └─────────────────────────────────┘   │ read-only (RC-I-7), live==replay │   (lightweight-charts)
                                          └──────────────────────────────────┘
   ┌─ C. EXPORT CONTRACT v2 ─────────────────────────────────────────────────┐
   │ версионированная форма серий (export_schema_version), аддитивно поверх v1 │
   └──────────────────────────────────────────────────────────────────────────┘
```

- **A. Indicator Engine** — `crates/derive` + `crates/research-cli` (расширяются). Новые агрегаты — новые
  чистые редьюсеры; RED-first на каждый (детерминизм-тест обязателен).
- **B. Read Gateway** (M-22, новый крейт, напр. `crates/gateway`) — **enabling-инфра кокпита**. Тейлит журнал
  live, прогоняет редьюсеры, отдаёт: (1) снапшот при подключении, (2) инкрементальный **WS-push** (поверх
  Fastify founder'а), (3) replay (детерминированный проигрыш окна). Тяжёлые серии (heatmap/depth) — бинарные
  фреймы. **Read-only**: не пишет журнал, recorder не зависит от gateway.
- **C. Export contract v2** — `research/exports/format.md` расширяется АДДИТИВНО (bump `export_schema_version`,
  без T1-RFC — T-designate, как v1; `docs/05` §governance). Новые серии: heatmap, volume_profile, vwap,
  liq/oi/funding-profiles, tpp (с provenance §4).

## §4. Data-quality дисциплина (замер глубины — BINDING)

Урок C-020/M-10 «сначала провалидируй измеритель, потом доверяй» переносится сюда как гейт.

- **Глубина книги (TPP-полосы 3-30%):** видимы из нашей diff-книги (замер `depth-probe-*`: reach ~54-59%,
  полосы заполнены, флуктуируют как реальная ликвидность — НЕ чистый фантом TD-016). НО **валидировать
  глубже ~1.3% не против чего** (Binance капит снапшот; Tardis — тот же кап) — это inherent для всего класса
  (Bookmap/TPP тоже реконструируют из diff и доверяют).
- **Инвариант провенанса:** каждая серия по книге глубже 1.3% несёт
  `depth_band_provenance: "diff-reconstructed, validated<=1.3%"`. Фронт/AI не выдают её за биржевой факт.
- **Корректность книги — предусловие TPP** (замена «вендор y/n»): (а) эвикция мёртвых уровней (**TD-016**);
  (б) целостность при resync (ресинк к мелкому снапшоту не должен ронять восстановимые дальние полосы в 0
  как «настоящий» факт). RED-спека на venue-book — до включения TPP-полос в контракт.
- **`formula_pending`** для неподтверждённых формул (TPP TOTAL-состав, Secrets) — бэкенд НЕ выдумывает
  (бриф §27). Серия объявлена, значение — `pending`, пока founder не подпишет методологию.
- **История 2019+** — только через вендора (**Tardis.dev**, Binance spot L2 с 2019-12); НЕ для глубины, для
  временного охвата (backtest/replay). Импорт — `Vendor`-эпоха (CT-RFC-02, `docs/06` §эпохи), fail-closed
  классификация, в обучение по умолчанию не попадает.

## §5. Инварианты (RED-оракулы; sacred, architect-only)

| ID | Инвариант |
|---|---|
| VB-I-1 | Каждый индикатор — чистый редьюсер над `journal::stream`; детерминизм-тест обязателен (тот же вход → байт-идентичная серия). Нет wall-clock/rand/I-O в расчёте |
| VB-I-2 | **live == replay**: серия, посчитанная на live-хвосте, бит-идентична серии из replay того же окна журнала |
| VB-I-3 | Read Gateway read-only: grep-канарейка — gateway не импортирует journal-writer/recorder-write; recorder не зависит от gateway |
| VB-I-4 | export v2 аддитивен: старые консюмеры v1 не ломаются; форма меняется только с bump `export_schema_version` (CT-I-аналог) |
| VB-I-5 | Серия глубже 1.3% несёт `depth_band_provenance`; отсутствие поля → серия невалидна (честность измерителя) |
| VB-I-6 | Модель сессии (00:00 UTC) — единый примитив для VWAP/SVP/CVD-reset; расхождение anchor между ними запрещено (один источник) |
| VB-I-7 | `formula_pending`-серия НЕ эмитит вычисленное значение (только маркер), пока формула не подписана founder'ом |
| VB-I-8 | Volume Profile: цены без сделок НЕ выдумываются (как footprint-bins v1, C-016); POC/VAH/VAL детерминированы от разбиения |
| VB-I-9 | **Граница плоскостей (D6):** `gateway-serve` НЕ читает/не пишет application-БД (Postgres); auth = ТОЛЬКО stateless verify подписанного JWT, без user-БД-lookup. grep-канарейка: gateway не импортирует postgres/sqlx/diesel-клиент. User-состояние (стратегии/чаты/настройки) вне market-журнала (`DET-I-1`) |

## §6. Соответствие контракт-слою (`05`) и даталееру (`06`)

- **T1-ядро (`Event`/`EventKind`) НЕ трогаем** — только читаем (Граница A). Виз-контракты — T-designate в
  export/gateway-крейтах; промоушен в `crates/contracts` только при кросс-языковом консюмере (TD-008-паттерн).
- **Даталеер (`06`):** стрим-чтение (`06` §5), эпохи/provenance (CT-RFC-02), ретеншен/компакция (`06` §4) —
  виз-бэкенд их ПОТРЕБИТЕЛЬ. Replay-окно (сколько raw держим локально) — параметр ретеншена (`06` §4);
  Tardis-архив — cold/`Vendor`-эпоха. Read Gateway обязан работать на bounded-memory стриме (не `Vec<Event>`
  на 15 GB — класс TD-011).

## §7. Открытые вопросы
1. TPP `formula_pending`: состав TOTAL1/2/3/OTHERS + формулы Secrets (founder).
2. Replay-окно (дни/недели/месяцы) → ретеншен-политика + Storage Box (founder).
3. Транспорт бинарных фреймов heatmap (msgpack/protobuf) — детали M-22.
4. resync-целостность дальних полос — RED-спека на venue-book (предусловие TPP).

## §8. Cross-references
- `docs/07-cockpit-backend-roadmap.md` (решения), `docs/06-data-layer-and-storage.md` (даталеер),
  `docs/05-contract-layer.md` (governance), `docs/03-integration-contract.md` §4 (Граница A).
- `docs/fa/ai-copilot.md` (AI-слой — потребитель этого дериватив-слоя).
- `research/exports/format.md` (export v1→v2), `research/data-quality/depth-probe-*.md` (глубина).
- TD-016 (эвикция/фантом), TD-004/010 (REST-глубина).
