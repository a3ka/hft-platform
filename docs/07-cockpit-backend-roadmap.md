# 07 — Order Flow Intelligence Terminal: бэкенд-роадмап + журнал решений

> **STATUS:** LIVING DOC (architect-owned). Заведён 2026-07-22 как фиксация решений сессии, чтобы
> НЕ ПОТЕРЯТЬ контекст пивота. Это и роудмап, и decision-log. Формальные FA/DESIGN/контракт-правки
> идут отдельными доками (см. §12), этот — якорь.
>
> **Цель продукта (founder, 2026-07-22):** собрать ПРАВИЛЬНЫЕ ДАННЫЕ + бэкенд-агрегаты + стабильный
> экспорт → фронт `code2alpha` рисует **Bookmap-подобный кокпит** (heatmap ликвидности, footprint, CVD,
> Volume Profile, VWAP) + **TPP-метрики** (Bid/Ask, Delta по %-полосам) + **встроенный AI-копилот**,
> который в реальном времени по выбранной стратегии (напр. Fabio-style) говорит «что делаем и почему».
> Рабочее имя: **Order Flow Intelligence Terminal** (бриф UX/UI — у founderّа).

---

## §1. Пивот (что изменилось этой сессией)

- **Сигналы/альфы/стратегии — на паузу.** M-10 (OBI kill-screen) закрыт негативно (D-001, OBI мёртв);
  стек-фиксы C-020 A/B/C/D **заморожены** (RED-спека закоммичена на `feat/M-10-rebased`, вернёмся при
  возобновлении сигналов). Торговый стек **risk/killswitch/oms (M-11), runner/подпись (M-12)** — отложены.
- **Новый фокус:** ДАННЫЕ + виз-бэкенд + AI-копилот. «Инфраструктура → данные → потом сигналы/торговля».
- **Источники:** оставляем изначальные **Binance + Hyperliquid**. **OKX убран** (ранее рассматривался —
  отменён). Новые venue добавляем постепенно ПОТОМ.
- **Фронт — founder** (`a3ka/code2alpha`, Next15 + lightweight-charts v5 + Fastify). **Мы — бэкенд +
  стабильный экспорт-контракт.** M-19 (frontend cockpit) — зона founder'а.
- Визуально: смесь **Bookmap + Trading Platform Pro (TPP)**. Референсы: `tmp/design_screens/`
  (Bookmap-скрины, TPP-скрины, `BID_ASK_*.pdf`, `TradingPlatformPro_Datasets.pdf`, funding — стратегия,
  отложена). Не копировать код/ассеты конкурентов — оригинальная система (бриф §27).

## §2. Продуктовая логика (информационная архитектура и AI)

Интерфейс и AI ведут пользователя по 4 вопросам (Fabio/Auction Market Theory):
**Market State → Location → Aggression → Price Response → Trigger → Invalidation.**
Это же — каркас вывода AI-копилота (§7).

## §3. Архитектурный спайн (делает всё когерентным — BINDING)

1. **Journal-first, чистые детерминированные редьюсеры.** Каждый индикатор (heatmap, footprint, CVD, VP,
   VWAP, TPP, семантические события) = функция над потоком `Event` из `journal::stream` (Граница A,
   read-only). Без wall-clock/rand/I-O в расчёте.
2. **Live == Replay.** Live-путь и replay гоняют ОДИН код редьюсеров; live просто кормит хвост журнала →
   бит-идентичный вывод (замена ручного «скриншот каждые 30с»). Даёт replay-tutor (бриф §18) бесплатно.
3. **AI вне горячего цикла + вне детерминированного ядра.** LLM недетерминирован → его выводы живут в
   ОТДЕЛЬНОМ audit-store, НЕ в market-журнале (DET-I-1 цел). Наш инвариант «LLM не в горячем торговом цикле».
4. **T1-ядро (`Event`/`EventKind`) не трогаем** — только читаем. Виз-контракты — аддитивные/T-designate (§9).

## §4. Что УЖЕ есть (не переделываем)

- **M-17** (DONE): export-контракт `research/exports/format.md` (`export_schema_version=1`): OHLCV,
  footprint-дельта, **cumulative delta (CVD)**, footprint-bins (per-цена buy/sell/delta), depth-series
  (per side, per band). Редьюсеры order-flow в `crates/{derive,research-cli}`.
- **M-18** (DONE): L2Delta capture (инкрементальный стакан) — сырьё для heatmap.
- **recorder** 24/7 в проде: Binance spot+futures, HL; ротация+компакция+ретеншен-доставка живут; recon (M-09).
- Экспорт-механизм сейчас — **БАТЧ-файл** `<out>/<venue>/<symbol>.json` (read-only, RC-I-7).

## §5. Решения этой сессии (D1–D5 + данные + AI)

| # | Решение | Статус |
|---|---|---|
| **D1 Транспорт** | **WebSocket поверх Fastify** (у founder уже заложено переключение на WS). Снапшот-при-подключении + инкрементальный push + replay-контролы. Heatmap/depth — бинарные фреймы, остальное JSON. | ✅ принято |
| **D2 TPP scope** | Строим **COIN-scope** (BTC/ETH) сейчас; **TOTAL/TOTAL1-3/OTHERS** — потом (нужен универсум монет + масштаб сбора). «Всё как у TPP» = принято как цель. | ✅ принято |
| **D2 TPP формула** | Файлов ХВАТАЕТ для heatmap/VP/VWAP/CVD/footprint/Bid-Ask/Delta — строю сам. **Нужно от founder ТОЛЬКО:** (а) состав `TOTAL1/2/3/OTHERS`; (б) формулы «Secrets» — **MLSP, Margin, Ratio, Speed, Market Diff** (их методик в файлах НЕТ). До этого — `formula_pending`. | ⏳ ждёт founder (частично) |
| **D2 TPP ДАННЫЕ** | Bid/Ask/Delta требуют глубины **3/5/8/15/30% от mid**. REST Binance кап ~1.3% (BTC spot) / 0.09-0.26% (futures) — сам по себе НЕ годится (= наш TD-004/010). Diff-книга уходит глубже. **НО дальние полосы = потенциальный ФАНТОМ (TD-016, OPEN).** См. §6. | 🔴 БЛОКЕР (TD-016) |
| **D3 AI размещение** | **Отдельный бэкенд-сервис `ai-copilot`** вне детерминированного ядра, зовёт внешний **мультимодальный LLM-API**, стримит на фронт через WS + пишет Audit-Log. Работает непрерывно без открытого UI. Модель-агностичен. | ✅ принято (провайдер — ждёт founder) |
| **D4 MVP** | **MVP-1 (без блокеров, строим сейчас):** Read Gateway + heatmap + volume bubbles + COB + Volume Profile (SVP/CVP/FRVP) + CVD (session-reset 00:00 UTC) + VWAP. Цель — **Binance BTCUSDT**. **MVP-2:** TPP COIN→TOTAL, Event Engine, AI. | ✅ принято |
| **D5 История** | **Tardis.dev** для истории с 2019 (Binance Spot BTCUSDT `incremental_book_L2` с 2019-12-01; futures с 2020-02). HL в 2019 НЕ существовал. Replay-окно (raw локально) — TBD; Storage Box — когда окно перерастёт диск. Бэкфил — отдельно (M-16-класс). | ✅ направление (бюджет — ждёт founder) |

## §6. Данные — открытый узел (depth-probe + TD-016)

**Depth-probe (research-dev, `research/data-quality/depth-probe-binance.md`, ветка `research/depth-probe`):**
- Подтвердил: REST-снапшот мелкий (BTC spot 1.3%, futures 0.09-0.26%) — для TPP не годится.
- Измерил: наша diff-книга **достигает 50-59% от mid**. Отсюда memo сделал вывод «полосы 3-30% вычислимы,
  вендор не нужен».
- **Architect НЕ ПРИНЯЛ этот вывод (переоценён).** Memo измерил ДОСЯГАЕМОСТЬ, но НЕ КАЧЕСТВО дальних полос.
  **Reach ≠ реальная ликвидность.**

**Почему (BINDING):** наш reviewer уже задокументировал эти полосы как **фантом — TD-016 (OPEN)**:
> «мёртвые уровни… попадают в L2Snapshot и в полосы OBI 6-60%. Дальние полосы содержат ФАНТОМНУЮ
> ликвидность, которой на бирже нет… отличить [сходимость от лика] может ТОЛЬКО recon с биржей (P2.5).»

Значит числа memo в полосах 15/30% — вероятно тот самый фантом. Валидировать полосы 3-30% против Binance
**невозможно** (REST даёт максимум 1.3%, глубже эталона у биржи нет).

**Корректный вывод (заменяет §7 memo):**
1. Источник (diff-поток) — вероятно достаточен для LIVE-глубины → вендор для живого фида, скорее всего, НЕ нужен.
2. **Значения полос 3-30% СЕЙЧАС НЕДОСТОВЕРНЫ** (TD-016). Строить TPP band-sums на них = строить на
   загрязнённой ликвидности (тот же урок, что C-020/M-10: сначала провалидируй измеритель).
3. **История 2019 — Tardis нужен** независимо (diff-книга работает только вперёд).

**⇒ TD-016 повышается из «наблюдения» в БЛОКЕР TPP.** Нужен: (а) `depth_probe.rs` — staleness/per-snapshot/
полосы 0.5-1% (различает реальную полосу от фантома); (б) фикс TD-016 (recon-эвикция дальних полос).

## §7. AI-копилот — архитектура (5 слоёв, разделение)

Заменяет ручной «скриншот в ChatGPT каждые 30с» на непрерывный автоконтекст.

1. **Дериватив-слой (детерминированный, наш):** все индикаторы — те же числа, что видит UI (единый источник
   правды: AI и человек видят ОДНО).
2. **Event Engine (наш):** визуальные паттерны → структурированные события: absorption, exhaustion, breakout,
   CVD-divergence, TPP «Перевёртыш»/«Слипание». **LLM не смотрит на пиксели** — паттерн отдаётся как факт.
3. **Strategy Definition (контракт):** «Fabio» и др. = versioned-конфиг (какие индикаторы важны + каркас
   решения + инвалидация). Стратегий даём сколько угодно (бриф §15).
4. **AI-Context Composer (наш):** каждые N сек / на событии собирает компактный снапшот
   `{market_state, location, aggression, price_response, liquidity, tpp, active_events[], strategy_state}`.
5. **LLM-сервис (ОТДЕЛЬНЫЙ, недетерминированный):** `context + strategy + events` → структурный вывод
   (Market State/Location/Aggression/Price Response/Interpretation/Confirmation/**Invalidation**/Confidence/
   Evidence, бриф §14.2). Опционально мультимодально (скрин) — как усиление, не основа. → Audit-Log.

**AI по умолчанию read-only** (не шлёт ордера; §14.4/§23 брифа). Автоисполнение — отдельный будущий режим.

## §8. Milestone-последовательность (виз-first)

Фундамент DONE: M-17 export · M-18 L2Delta · export v1.

**Трек A — источник/качество данных:**
- **[БЛОКЕР] TD-016 фикс** — recon-эвикция дальних полос (иначе TPP-полосы фантомны). Предваряется `depth_probe.rs`.
- **Tardis-импорт** (история 2019+) — research-only, для replay/бэктеста.

**Трек B — виз-примитивы (строятся на текущих данных, MVP-1):**
- **M-22 — Read Gateway** (WS live + snapshot + replay). Enabling-инфра, критический путь.
- **M-23 — Heatmap + COB + Volume Bubbles** серии (из L2Delta).
- **M-20 — VWAP** (session-anchored, HLC3, reset 00:00 UTC) — расчёхлить из QUEUED.
- **M-24 — Volume Profile** (SVP/CVP/FRVP/Anchored/Composite: POC/VAH/VAL/HVN/LVN, VA%) + модель сессии.
- **M-25 — Liquidations/OI/Funding профили** (правые колонки).

**Трек C — MVP-2:**
- **TPP COIN** (Bid/Ask/Delta по полосам, BTC/ETH) — ПОСЛЕ TD-016 фикса; TOTAL — после спеки универсума.
- **M-26 — Event Engine** (семантические события → UI + AI + alerts).
- **M-27 — AI-Context Service + Audit Log** (+ внешний `ai-copilot` LLM-сервис).

**Сходятся → M-19 Frontend cockpit** (founder).

**Отложено целиком:** M-11 (risk/ks/oms), M-12 (runner/подпись), M-13 (HL depth), M-14 (DET-I-1 полный),
M-10 стек-фиксы (сигналы), funding-стратегия.

## §9. Соответствие контрактному слою (`05-contract-layer.md`)

- **export v2** — новые серии (heatmap/VP/VWAP/CVD-session/TPP) АДДИТИВНО, bump `export_schema_version`,
  без T1-RFC (T-designate в export-слое, как v1).
- **AI-Context / Event / Strategy / Audit** — новые версионированные контракты; T-designate в своём крейте,
  промоушен в `crates/contracts` только при кросс-языковом консюмере (как TD-008).
- **T1-ядро не трогаем** — только читаем журнал (Граница A). Контрактная чистота сохранена.

## §10. Открытые вопросы (ждут founder)

1. **TPP `formula_pending`:** состав `TOTAL1/2/3/OTHERS` + формулы Secrets (MLSP/Margin/Ratio/Speed/Market Diff).
2. **LLM-провайдер** (Claude/GPT-4o-класс, мультимодальный) + где крутить инференс (наш сервис — принято; подтвердить модель).
3. **Tardis бюджет** (стоимость/лицензия — architect соберёт сравнение) + окно replay (дни/недели/месяцы) + Storage Box сейчас/потом.
4. **Масштаб универсума монет** для TPP TOTAL (десятки-сотни символов → инфра/диск).
5. **MVP-1 набор** подтверждён; первый символ — Binance BTCUSDT.

## §11. Дисциплина, перенесённая на виз-бэкенд

- Детерминизм редьюсеров sacred (live==replay); RED-first на каждый агрегат.
- **Data-quality gate:** ни один агрегат по книге не «готов», пока не провалидирован против recon/staleness
  (урок TD-016 + C-020/M-10 «сначала измеритель, потом доверие»).
- `formula_pending` для неподтверждённых формул (бриф §27 — бэкенд не выдумывает).
- AI-выводы аудируются; AI read-only по умолчанию; вне DET-I-1 журнала.

## §12. Следующие действия (порядок)

1. **`crates/book/examples/depth_probe.rs`** — staleness/per-snapshot/полосы 0.5-1% (закрывает вопрос «фантом ли полосы 3-30%»). ← СЕЙЧАС.
2. **TD-016 → блокер TPP** (recon-эвикция) — спека после depth_probe чисел.
3. Уставные доки: `DESIGN.md` (фаза «Виз-бэкенд + AI»), `docs/fa/viz-backend.md`, `docs/fa/ai-copilot.md`,
   `05-contract-layer.md` (governance), `BACKLOG.md` (виз-first).
4. Спека **M-22 (Read Gateway)** + параллельно M-20/M-23.
5. Tardis — сравнение вендоров (цена/покрытие/лицензия) → founder-решение.

## §13. Cross-references
- `research/data-quality/depth-probe-binance.md` (ветка `research/depth-probe`) — замер глубины.
- `research/exports/format.md` — export-контракт v1.
- `TECH-DEBT.md` TD-016 (фантом дальних полос), TD-004/010 (REST-глубина).
- `feat/M-10-rebased` — замороженная RED-спека сигналов (C-020 A/B/C/D) + D-001 (OBI KILL).
- `tmp/design_screens/` — Bookmap/TPP референсы + брифы.
- `milestones/{M-16,M-17,M-18,M-19,M-20,M-21}.md`.
