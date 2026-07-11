# M-06 — Data expansion: futures depth + OI + liquidations + funding-breadth (P1)

STATUS: 🚧 IN_PROGRESS. Authored: architect (Fable), 2026-07-11.
Предпосылка: CT-RFC-01 применён (C-003 PASS) — T1 несёт Venue::BinanceFutures +
MdPayload::{OpenInterest,Liquidation,MarginRate}. Гейты: critic не требуется (контракты
уже прошли CT-RFC-01; новый крейт venue-binance-futures — триггер §1.4, но это тонкий
адаптер по образцу venue-binance → critic по усмотрению founder'а); **reviewer обязателен**
(trogает venue-*). risk-critic НЕ требуется (ордер-путь не трогается).

## Objective
1. Вернуть workspace в компайл после CT-RFC-01 (consumer blast-radius).
2. Собрать новые данные под квант-стратегии: глубина Binance futures, funding (+breadth),
   open interest, ликвидации. funding-breadth/CVD/базис — DERIVE downstream (journal-first).

## Contract impact (T1)
**НЕ трогается** — типы уже есть (CT-RFC-01). M-06 только ПРОИЗВОДИТ/ПОТРЕБЛЯЕТ их.

## Allowed / Forbidden paths (scope-guard)
| Агент | Allowed | Forbidden |
|---|---|---|
| architect | `milestones/M-06-*.md`, `crates/*/tests/**` (RED), `crates/venue-binance-futures/` СКЕЛЕТ (Cargo.toml+lib-стаб+RED, по образцу M-04 task 1), `scripts/verify_M-06.sh` | impl-парсеры/поллер |
| venue-dev | `crates/venue-binance-futures/src/**` (fstream depth+OI+liquidations+funding парсеры), `crates/venue-binance/src/**` (если futures-режим встраивается), consumer-arm'ы в своих venue-крейтах | tests, contracts |
| engine-dev | `crates/recorder/src/**` (poller-компонент: REST cadence фикс.конфиг → журнал), `crates/sim/src/exchange.rs` (explicit ignore-arm новых payload'ов) | tests, contracts, risk |
| research-dev | `crates/research-cli/src/**` (latency_probe match-arm'ы) + `crates/derive/src/**` (funding-breadth DERIVE, отдельный крейт) | tests, contracts |
| signal-engineer | `crates/signals/src/obi.rs` (explicit ignore-arm) | tests |
| все dev | — | `crates/contracts/**`, `*/tests/**`, `scripts/**` |

## §Tasks
| # | Status | Задача | Агент | Verify |
|---|---|---|---|---|
| 1 | ⏳ | **Blast-radius fix (компайл):** MUST-FIX exhaustive — `sim/src/exchange.rs:223` (MdPayload), `research-cli/bin/latency_probe.rs:34-38` (Venue)+`:107-111` (MdPayload); EXPLICIT-ARM (сейчас wildcard, C-003 §3) — `book/src/lib.rs:190-197`, `signals/src/obi.rs:84-87`; examples dump/bands/obi_probe. Новые payload'ы — ЯВНЫЙ ignore (не молчаливый wildcard). | engine/research/signal-dev по зонам | workspace компилируется; C1-RED GREEN |
| 2 | ⏳ | `venue-binance-futures` скелет (architect) → парсеры: fstream `@depth@100ms`+`/fapi/v1/depth` (L2Snapshot, Venue::BinanceFutures); `@forceOrder` → Liquidation; `!markPrice@arr`/`premiumIndex` → Funding | architect(скелет+RED)→venue-dev | C2-RED GREEN |
| 3 | ⏳ | OI: `/fapi/v1/openInterest` (+`openInterestHist`) → MdPayload::OpenInterest | venue-dev | C3-RED GREEN |
| 4 | ⏳ | recorder-poller: REST-источники (funding all-perps `premiumIndex` 1 вызовом; OI) с фикс.cadence-конфигом → журнал с ts_exch | engine-dev | poller пишет события, cadence детерминирован |
| 5 | ⏳ | funding-breadth top-300 (%+/−) — DERIVE в крейте `derive`, НЕ T1; ранжирование по OI/volume | research-dev | breadth считается из потока Funding детерминированно |
| 6 | ⏳ | `scripts/verify_M-06.sh` exit=0 | tester | exit=0 |

MarginRate impl — **Tier-3, ОТЛОЖЕН** (нужны ключи/3rd-party; тип уже в T1, продюсер позже).

## RED-тесты (sacred, architect-only)
- **C1 `consumers_ignore_new_md_variants`** (`crates/sim/tests/`) — sim.on_event(OpenInterest/
  Liquidation/MarginRate) → НЕ паникует, НЕ создаёт fills (явный ignore, не обработка).
- **C2 `binance_futures_liquidation_side_is_liquidated_side`** (`crates/venue-binance-futures/
  tests/`) — сырой forceOrder `S=SELL` (ликвидируется LONG) → `MdPayload::Liquidation{side:Sell}`.
  **C-003 note:** side = ЛИКВИДИРУЕМАЯ сторона, НЕ агрессор — иначе CVD/liq-flow инвертирует знак.
- **C2b** futures `@depth` → L2Snapshot с Venue::BinanceFutures; глубина полос вычислима.
- **C3 `open_interest_parse`** — сырой openInterest JSON → MdPayload::OpenInterest{oi_e8}.
- **C5 `funding_breadth_derive_deterministic`** (`crates/derive/tests/`) — из фикс.
  набора Funding-событий top-N breadth детерминирован (одинаковый вход → одинаковый %).

Анти-плацебо: каждый RED падает на заглушке/до импла.

## Acceptance
`bash scripts/verify_M-06.sh; echo "exit=$?"` → `VERDICT: PASS`, exit=0. Ключ: workspace
снова компилируется + новые парсеры GREEN + funding-breadth детерминирован.

## Handoff
architect (скелет venue-binance-futures + RED) → venue-dev(2,3)‖engine-dev(1 sim,4)‖
research-dev(1 latency_probe,5)‖signal-dev(1 obi) → tester(6) → reviewer.

## Amendment 2026-07-11 (принят deep-book quality из M-05)

Из M-05 перенесено (coupled с depth-runner): **deep-book completeness / B1**.
- B1-инвариант (unit, когда venue-binance выдаст testable seam): gap-инвалидация →
  ПОЛНАЯ замена книги (нет stale-переноса). Оракул co-design с venue-dev при depth-runner (task 2).
- **limit=5000 undercount** (REST `/api/v3/depth?limit=5000` достаёт ~top-5000 уровней →
  полосы 15-60% недосчитаны на бутстрапе): решить митигацию (многостраничный REST / принять,
  diff само-заживает активные уровни / диагностик-сверка книга vs свежий REST на глубине).
  DATA-вопрос → reviewer TECH-DEBT.
## Amendment 2026-07-11 (reviewer N2/N3/N4 → architect RED-first)

Контекст: venue-dev #2/#3 APPROVED (парсеры C2/C2b/C3 GREEN, MD-only). reviewer нашёл
пробелы; architect проектирует оракулы (граница gates.md §4).

### N3 — funding-парсер (RED landed)
`parse_mark_price` (markPriceUpdate `r`→Funding) — STUB + RED `tests/red_funding.rs`
(положит. И отрицат. ставка, знак не хардкодится). Реальный вход для C5 funding-breadth.
Задача: venue-dev реализует parse_mark_price + подписку `!markPrice@arr` в runner. GREEN:
`cargo test -p venue-binance-futures --test red_funding`.

### N2 — deep-book resync/eviction (оракул СПЕЦ; унифицирует B1 из M-05)
Риск (reviewer): futures diff-sync-книга (apply_diff/handle_diff/emit_book_snapshots) —
ресинк + eviction дальних уровней БЕЗ теста → фантомная ликвидность в полосах 15-60%.
**Мой анализ:** «phantom» — СЛАБЫЙ риск (U/u-continuity не даёт пропустить cancel;
apply_snapshot = replace). Реальный тестируемый ИНВАРИАНТ — корректность пути gap:

  **INV-N2:** при разрыве непрерывности (`U != last_update_id + 1`) книга ИНВАЛИДИРУЕТСЯ
  и ПОЛНОСТЬЮ пересобирается из свежего REST-снапшота — НЕТ переноса stale-уровней
  (дальний уровень, присутствовавший до gap и ОТСУТСТВУЮЩИЙ в свежем снапшоте, удалён).

Оракул (анти-плацебо): seed-снапшот → diff добавляет уровень на ~40% от mid → gap →
свежий REST-снапшот БЕЗ него → `notional_within(Side, 0.50)` его НЕ содержит. Падает на
merge-семантике / отсутствии eviction.

**Требуемый seam (venue-dev выставляет — §4: дизайн мой, impl их):** тестируемый
book-maintainer, напр. `pub(crate) struct FuturesDepthBook { apply_snapshot(&[Level],&[Level])
= REPLACE; apply_diff(&[Level],&[Level]) size==0→remove; notional_within(Side,f64) }`.
RED `tests/red_resnapshot_futures.rs` ФИНАЛИЗИРУЮ против этого seam, когда venue-dev
выставит maintainer в runner (не проектирую вслепую против невидимой ветки — иначе
mismatch/rework только что одобренного кода). Это ДИЗАЙН оракула (инвариант+seam), не
отсрочка: как только maintainer есть — RED .rs коммичу.

Комплемент N2 — `limit=5000` undercount (completeness, DATA-вопрос, отдельно; уже в
amendment выше): N2 = корректность resync; limit=5000 = полнота глубины.

### N4 — MD-only carve-out (закреплено в gates.md §5)
`venue-*` MD-only (нет order-egress) → risk-critic НЕ нужен, reviewer подтверждает MD-only.
Правило и milestone больше не противоречат.
