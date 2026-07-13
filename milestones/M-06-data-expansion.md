# M-06 — Data expansion: futures depth + OI + liquidations + funding-breadth (P1)

STATUS: ✅ DONE (close-out 2026-07-13; reland #4 APPROVED 9272a89 + MERGED 1504d8b + §8 LIVE-GREEN). Authored: architect (Fable), 2026-07-11.
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
| 1 | ✅ | **Blast-radius fix (компайл):** MUST-FIX exhaustive — `sim/src/exchange.rs:223` (MdPayload), `research-cli/bin/latency_probe.rs:34-38` (Venue)+`:107-111` (MdPayload); EXPLICIT-ARM (сейчас wildcard, C-003 §3) — `book/src/lib.rs:190-197`, `signals/src/obi.rs:84-87`; examples dump/bands/obi_probe. Новые payload'ы — ЯВНЫЙ ignore (не молчаливый wildcard). | engine/research/signal-dev по зонам | workspace компилируется; C1-RED GREEN |
| 2 | ✅ | `venue-binance-futures` скелет (architect) → парсеры: fstream `@depth@100ms`+`/fapi/v1/depth` (L2Snapshot, Venue::BinanceFutures); `@forceOrder` → Liquidation; `!markPrice@arr`/`premiumIndex` → Funding | architect(скелет+RED)→venue-dev | C2-RED GREEN |
| 3 | ✅ | OI: `/fapi/v1/openInterest` (+`openInterestHist`) → MdPayload::OpenInterest | venue-dev | C3-RED GREEN |
| 4 | ✅ | recorder-poller: REST-источники (funding all-perps `premiumIndex` 1 вызовом; OI) с фикс.cadence-конфигом → журнал с ts_exch | engine-dev | poller пишет события, cadence детерминирован |
| 5 | ✅ | funding-breadth top-300 (%+/−) — DERIVE в крейте `derive`, НЕ T1; ранжирование по OI/volume | research-dev | breadth считается из потока Funding детерминированно |
| 6 | ✅ | `scripts/verify_M-06.sh` exit=0 | tester | exit=0 |

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

## Amendment 2026-07-12 (TD-013 — resync backoff; #4 откачен §8)

Инцидент: #4 смёржен → §8 eyes-on поймал 418-ban (recorder hammering Binance) → REVERT
(main b74449c inert-safe). Корень: `venue-binance-futures/src/lib.rs` — snapshot-fail и
stale ветки НЕМЕДЛЕННО `pending_snapshots.push(make_snapshot_future(...))` без задержки →
при 418/429 hot-loop.

**Дизайн (architect, §4 — venue-dev реализует):** чистая политика `Backoff` (per-symbol):
`new()` / `next_delay(&mut self, retry_after: Option<Duration>) -> Duration` (детерминир.:
exp база × attempt, cap; НЕ меньше `retry_after` из 418/429 Retry-After/cooldown) / `reset()`
(на success). Джиттер применяет async-вызывающий (I/O-boundary), НЕ эта политика (тестируема).

**Wiring (venue-dev):** resync-путь консультирует `Backoff` — на fail/stale вычислить
delay + ЖДАТЬ его перед re-push (delayed re-push / sleep); на успешном снапшоте `reset()`.
Honor Retry-After. INITIAL-connect 418-толерантность (остаточный IP-cooldown после ban).

**Оракул:** `tests/red_backoff.rs` (compile-RED, seam `Backoff`). Тестирует ПОЛИТИКУ (чистую);
reviewer верифицирует WIRING (путь реально ждёт, не только конструирует Backoff); §8 —
что hammering'а в проде нет (тот же паттерн seam+§8, что J1). risk-critic НЕ нужен (MD-only,
read-side REST).

**TD-013 (correctness/rate) ≠ TD-012 (completeness/limit=1000)** — разные фиксы.
Цепочка реленда: architect(RED, здесь) → venue-dev(Backoff+wire) → engine-dev(reland #4 = re-apply 2eee4bf) → tester(#6).

## Amendment 2026-07-12 (TD-014 — futures runner live-emit; #4 reland §8-rejected)

Инцидент: #4 reland прошёл TD-013 (backoff live-OK, нет hot-loop), но §8 REJECT — в live-журнале
0 BinanceFutures L2Snapshot + 0 Funding (только ConnUp + OpenInterest), логи вечно "depth
continuity gap / snapshot stale vs buffered diffs, refetching".

**Анализ (architect):** (a) `handle_snapshot` reconcile-loop применяет буфер-diff'ы, но stale-чек
(`u_first > book.last_update_id + 1`) сравнивает с last_update_id, который, вероятно, НЕ двигается
при apply → 2-й contiguous diff вечно "stale" → книга никогда не синкается → 0 L2. (b) Funding из
!markPrice@arr не эмитится/starve'ится resync-циклом → 0 Funding. Оба НЕВИДИМЫ юнит-тестам:
`handle_diff/handle_snapshot/emit_book_snapshots` ходят в сеть (reqwest) и шлют в tx напрямую.

**Дизайн (architect, §4 — venue-dev реализует):** тестируемый seam `FuturesSession` —
sync-state-машина БЕЗ сети/каналов:
```
pub struct FuturesSession;                      // wraps states + symbol_set
pub enum SessionEffect { Emit(MdEvent), FetchSnapshot { symbol: String, after: Duration } }
FuturesSession::new(&[String]) -> Self
  .on_ws_text(&mut self, text: &str) -> Vec<SessionEffect>            // depth/forceOrder/markPrice
  .on_snapshot_result(&mut self, symbol: &str, Result<String, u16>) -> Vec<SessionEffect>  // Ok(json)/Err(http)
  .tick(&mut self) -> Vec<SessionEffect>                              // 1с: emit bounded L2Snapshot
```
`run()` становится тонкой I/O-оболочкой, ДЕЛЕГИРУЮЩЕЙ в `FuturesSession` (live == tested — иначе
дефект снова невидим). FIX: двигать `last_update_id` при apply (multi-diff sync); Funding из
markPrice эмитить НЕЗАВИСИМО от состояния книги (не starve).

**Оракул:** `tests/red_live_emit.rs` (compile-RED): 2 contiguous diff'а + snapshot(L=100) → sync +
эмит L2; markPrice во время resync → Funding; 418 → backoff. Анти-плацебо: верный рефактор
ТЕКУЩЕЙ логики оставляет multi-diff-stale → RED, форсит фикс.

**Acceptance:** red_live_emit GREEN + red_backoff/parse/funding/resnapshot GREEN + fmt/clippy/
workspace + (после reland #4) verify_M-06 PASS + reviewer §8 LIVE: BinanceFutures L2Snapshot +
Funding + OpenInterest в журнале, no hot-loop, heartbeat fresh, seq continuous, CPU/MEM norm,
restarts=0. TD-014 (liveness/emit) ≠ TD-013 (rate/backoff) ≠ TD-012 (completeness/limit=1000).
risk-critic НЕ нужен (MD-only). Цепочка: architect(RED, здесь) → venue-dev(seam+fix) → engine-dev(reland #4) → tester(#6).

## Amendment 2026-07-13 (M-06 CLOSE-OUT — DONE)

**M-06 ЗАКРЫТ.** Reland #4 APPROVED (reviewer `9272a89`) + MERGED (`1504d8b`) + §8 LIVE-GREEN;
tester #6 PASS на чистом чекауте (`verify_M-06.sh` exit=0; fmt/clippy/workspace GREEN; worktree clean).
Все §Tasks ✅.

**TD-014 (futures liveness) — CLOSED** после 3 итераций §8 eyes-on (unit-green ≠ prod-green):
- **T1** recovery-эмит L2 (recovery-снапшот дропал все buffered diff'ы → `last_event_time_ms=0` → 0 L2).
- **T2** continuity: Binance USDT-M FUTURES чейнит diff'ы через `pu == last_update_id`, не спот
  `u_first == last+1` — устранил 311-gap resync-churn (sparse L2, 429).
- **T4** Funding: WS mark-price НЕ доставляется (эмпирика architect'а — live WS-capture точного URL
  адаптера: 400 depth / 0 markPrice, и sandbox, и прод-VPS) → **пивот на REST `/fapi/v1/premiumIndex`
  poll** (тот же proven механизм, что OpenInterest) → Funding>0 live. (T3 per-symbol WS — тупик,
  эмпирически доказал недоставку.)

Урок (зафиксирован в `.claude/rules/gates.md §8b` + `testing.md`): для sacred I/O-путей local-green +
Deploy-success ≠ рабочий прод; §8 eyes-on решающ. Транспорт-доставка биржи не юнит-тестируема —
диагностируется live-capture'ом.

Связанные долги (reviewer-owned, в бэклоге): TD-012 (futures REST depth `limit=1000` undercount дальних
полос); MarginRate impl — Tier-3.

**Следующее:** M-04 задача 8 (формальный прогон OBI) — теперь РАЗБЛОКИРОВАНА: journal integrity (M-05)
+ futures MD live (M-06) готовы. Осталось накопить данные полной книги → грид/walk-forward/отчёт.
