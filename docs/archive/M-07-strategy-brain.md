# M-07 — Strategy brain: `alpha` → `portfolio` → `strategy` (мозг стратегии)

STATUS: ✅ DONE (2026-07-13; reviewer APPROVED, ff-merge в `main` `5141fd9`, reviewer-коммит
`656c7ca`; CI+Deploy success; §8 eyes-on на VPS GREEN — recorder инертен; `verify_M-07.sh`
21/21 PASS, exit=0). Revision 3 (reviewer-находка: сэмплинг equity_curve → ST-I-8g/8h → задача 9).
Authored: architect (Opus), 2026-07-13.
Гейты: **critic ОБЯЗАТЕЛЕН** (`.claude/rules/gates.md` §1 триггеры: 3 новых крейта,
≥5 коммитов) → engine-dev → research-dev → tester → reviewer.
**risk-critic НЕ требуется:** milestone не трогает `crates/risk|killswitch|oms|contracts|venue-*`
(gates §5). `crates/sim/**` тронут (T2-релокация + harness) — это НЕ safety-путь: sim
не отправляет ордера на биржу, структурно не зависит от `venue-*` (SM-I-9).
Ветка: `feat/M-07` (RED до реализации на main не живёт — gates §8).

## Objective

Построить ЕДИНСТВЕННЫЙ код торговых решений: `SignalOut → Forecast → TargetPosition →
OrderIntent`, детерминированный, без I/O, который исполняют И бэктест (`sim`), И будущий
live (`runner`). Этим закрывается дыра DESIGN §1 равенства 2 (`backtest == paper == live`):
сегодня решения захардкожены в ad-hoc harness'е `research-cli/src/grid.rs` (taker-in по
`SignalOut`, taker-out по `horizon_ms`) — то есть **бэктест меряет логику, которой не
будет в live**. M-07 заменяет этот harness настоящим strategy-пайплайном.

Авторитетная FA: **`docs/fa/strategy-brain.md`** (§2 границы, §3 формы, §4–6 семантика,
§7 инварианты AL-I-1..5 / PF-I-1..4 / ST-I-1..8). DESIGN: §1 (детерминизм), §2 (слои 3–4),
§3 (T1/T2/T3).

**Явно НЕ в scope (следующие итерации, named-not-silent):** MM-котирование (двусторонние
квоты/cancel-replace — требует `oms`+`risk`, M-08+); риск-гейт `RiskApproved` (M-08);
wiring весов из `signals.json` (граница B, P3); netting/корреляции (P5).

## Contract impact (T1)

**`crates/contracts/**` НЕ трогается → Block-C не срабатывает, contract-RFC не нужен.**

`Instrument`, `Forecast`, `Position`, `TargetPosition`, `RiskBudget`, `FillReport`,
`OrderIntent`, `OrderKind` — **T2** (модульно-ограниченные, владеет крейт) per
`docs/05-contract-layer.md` §2: они НЕ пересекают границу движок↔квант-деск (деск отдаёт
`SignalOut`/`SignalSpec` и читает журнал; форкасты/таргеты/интенты — внутренняя механика
движка). 05 §2 прямо называет `Forecast`/`TargetQuotes`/`Order` примерами T2.

**Релокация T2 `OrderIntent`/`OrderKind`: `sim` → `strategy` (D3, ниже).** Wire-формат
не меняется (в журнал эти типы не пишутся; `Ord(...)` EventKind — отдельный contract-RFC
в P3). Публичный API `sim` сохранён ре-экспортом → `research-cli` компилируется без правок
импортов.

## Архитектурные решения

| # | Вопрос | Решение M-07 |
|---|---|---|
| D1 | Где живёт `OrderIntent`? | **В `strategy` (Layer 4 — продюсер формы).** `sim` (Layer 6) зависит на `strategy` и ре-экспортирует (`pub use`). Обратный порядок (тип в `sim`) заставил бы live-`runner` линковать СИМУЛЯТОР ради типа — прямое нарушение «один код решений». Направление зависимости 6→4 = вниз, онион не нарушен. Канарейка ST-I-7: `pub struct OrderIntent` определён РОВНО в одном крейте |
| D2 | Обратная связь исполнения в `strategy` | T2 `FillReport{instrument, side, price_e8, qty_e8, fee_e8, ts_mono_ns}` — `strategy` НЕ знает про `sim::SimFill` (иначе зависимость 4→6, цикл). Мост SimFill→FillReport живёт в `sim::StrategyBacktest`; в live тот же мост построит `runner` из `Ord(Fill)` |
| D3 | Как `sim` гоняет стратегию | Новый `sim::StrategyBacktest` (harness): `run(&[Event], &mut dyn Strategy) -> BacktestReport`. Порядок на событии строго: `fills = exchange.on_event(ev)` → `strategy.on_fill(...)` по каждому филлу → `intents = strategy.on_event(ev)` → `exchange.submit(...)`. Стратегия НИКОГДА не видит событие раньше биржи и не видит будущего (ST-I-5) |
| D4 | Дедупликация ордеров в полёте | `in_flight` per instrument; запись истекает по **event-time** через `intent_ttl_ms` (никакого wall-clock). Без этого стратегия шлёт интент на каждом тике, пока филл не пришёл (ST-I-3) |
| D5 | Ансамбль v1 | `LinearAlpha`: `edge = clamp(Σwᵢvᵢ/Σ\|wᵢ\|, ±1e8)`, `horizon = max(horizonᵢ)`, `confidence = доля живого веса`, stale-expiry по `horizon_ms` сэмпла (FA §4). Один сигнал — вырожденный случай той же формулы (не отдельный путь) |
| D6 | Сайзинг v1 | `target = clamp(edge·max_pos/1e8, ±max_pos)` (i128); нет лимита → target 0; позиция без форкаста → target 0 (flatten). Это **pre-risk sanity, НЕ риск-гейт** — настоящий fail-closed барьер приходит в M-08 |
| D7 | Метрики грида после замены harness'а | `BacktestReport` несёт `fills`, `intents`, `cash_e8`, `positions`, `turnover_e8`, `equity_curve_e8`. **Семантика кривой УТОЧНЕНА (rev 3, reviewer-находка на PR-гейте; оракулы ST-I-8g/8h):** РОВНО одна точка на КАЖДОЕ СОБЫТИЕ, где биржа вернула ≥1 филл (2 филла на одном событии = 1 точка, а НЕ 2), снятая ПОСЛЕ применения события к книге и учёта ВСЕХ филлов события; на бесфилловых событиях точек НЕТ; `len(equity_curve) == #уникальных seq в fills`. Привязка к накопленному числу филлов (`curve.len() < fills.len()`) ЗАПРЕЩЕНА — она добирает фантомные точки в бесфилловом хвосте → σ занижена → Sharpe завышен → отчёт → ledger → подпись founder'а. `research-cli` считает returnsᵢ = `Δequity_e8 / capital_ref_e8`, где `capital_ref_e8 = max_position_e8 × mid₀` ячейки (детерминированно, задокументировать в отчёте). Числа PnL ОТЛИЧАЮТСЯ от пилота v1 — это ожидаемо (мерили другую логику); ledger append-only, старые записи не переписываются |
| D8 | Совместимость grid-ячеек | Ячейка = `ObiParams` + опциональный блок `strategy`: `{max_position_e8=1e8, min_order_e8=1e6, intent_ttl_ms=1000, marketable_margin_bp=100, kind="taker"}` (дефолты при отсутствии) → существующие спеки `S-001` парсятся. `params_hash` меняется → это НОВЫЕ trial-записи, честно |

## Allowed / Forbidden paths (scope-guard)

| Агент | Allowed | Forbidden |
|---|---|---|
| architect | `milestones/`, `docs/fa/strategy-brain.md`, `crates/{alpha,portfolio,strategy}/{Cargo.toml,src}` — **ТОЛЬКО T2-типы + trait-сигнатуры + `todo!()`-стабы** (скелет, паттерн M-04 task 1), `crates/*/tests/**` (RED, sacred), `scripts/verify_M-07.sh`, workspace-root `Cargo.toml` (members), **carve-out A1:** `crates/sim/src/types.rs` + `lib.rs` + `Cargo.toml` — ТОЛЬКО релокация T2 `OrderIntent`/`OrderKind` (удаление определения + `pub use strategy::...` + path-dep). Обоснование: релокация T2-формы атомарна по определению — половинчатое состояние (два определения одного типа) = ровно тот дефект, против которого стоит ST-I-7; логика не пишется. **carve-out A2 (revision 2, по C-004 C2):** `crates/research-cli/src/strategy_cell.rs` (новый) + `lib.rs` (`pub mod`) + `Cargo.toml` (path-deps на мозг) — ТОЛЬКО типы/константы-дефолты/сигнатуры D7/D8 с `todo!()`-телами. Обоснование: без этих форм задача 6 гейтилась только грепами, удовлетворяемыми комментарием (C2) — RED-оракул невозможно написать против несуществующей сигнатуры | любой impl-код (тела `todo!()`), логика в `crates/research-cli/src/grid.rs` |
| engine-dev | `crates/{alpha,portfolio,strategy}/src/**` + их `Cargo.toml` (свои deps), `crates/sim/src/**` + `crates/sim/Cargo.toml` (свои deps) | `*/tests/**` (sacred), `crates/{contracts,risk,killswitch,journal,venue-*}/**`, `crates/research-cli/**`, `scripts/**`, `docs/**` |
| research-dev | `crates/research-cli/src/**` + `crates/research-cli/Cargo.toml` | всё остальное; `research/trials-ledger.jsonl` (ручная правка); `crates/research-cli/tests/**` (sacred) |
| tester | read-only; прогон `scripts/verify_M-07.sh` | правки кода |
| reviewer | `PROJECT-STATE.md`, `TECH-DEBT.md`, merge `feat/M-07 → main` + §8 деплой-гейт | код |

## §Tasks

| # | Status | Задача | Агент | Verify |
|---|---|---|---|---|
| 1 | ✅ | Скелеты `alpha`/`portfolio`/`strategy` (T2-типы, трейты, `todo!()`), релокация `OrderIntent` (carve-out A1), workspace members, RED-suite, `verify_M-07.sh` | architect | workspace компилируется; fmt+clippy зелёные; RED-suite ПАДАЕТ (todo!) |
| 2 | ✅ | `alpha` impl: `LinearAlpha` (комбинация весов, stale-expiry, clamp, детерминированный порядок) | engine-dev | `cargo test -p alpha` GREEN (AL-I-1..5) |
| 3 | ✅ | `portfolio` impl: `RiskBudget` + `size()` (сайзинг, fail-safe кап, flatten, fail-closed без лимита) | engine-dev | `cargo test -p portfolio` GREEN (PF-I-1..4) |
| 4 | ✅ | `strategy` impl: `DirectionalStrategy` (books → signals → alpha → portfolio → diff → интенты; in-flight + ttl; маркетабельная цена) | engine-dev | `cargo test -p strategy` GREEN (ST-I-1..7) |
| 5 | ✅ | `sim::StrategyBacktest` harness: прогон `dyn Strategy` через `BacktestExchange`, мост `SimFill → FillReport` (сторона/инструмент — из интента через `order_meta`), `BacktestReport` (D3/D7) | engine-dev | `cargo test -p sim` GREEN (**ST-I-8a..f**: интенты доходят; **8e спай — КАЖДЫЙ филл доложен и верно подписан**; **8f — мутация будущего не меняет прошлое**; регрессия SM-I-*) |
| 6 | ✅ | `research-cli`: (а) реализовать `strategy_cell` (D7/D8: дефолты, `cell_params_hash`, `capital_ref_e8`, `returns_from_equity`); (б) переписать `grid.rs` — снять ad-hoc harness (`OpenPosition`/`Action`), гонять ячейку через `sim::StrategyBacktest` + `DirectionalStrategy`; ledger/стресс-режимы семантически сохранены | research-dev | `cargo test -p research-cli` GREEN (**GR-I-1..7**, включая ПОВЕДЕНЧЕСКИЕ GR-I-6/7: блок `strategy` реально меняет оборот; деадбенд глушит торговлю; ledger несёт канонический хэш) + RC-I-* регрессия |
| 7 | ✅ | Прогон `scripts/verify_M-07.sh` на чистом чекауте | tester | `VERDICT: PASS`, exit=0 |
| 9 | ✅ | **(rev 3, BLOCKING)** Фикс сэмплинга `equity_curve_e8` в `sim::StrategyBacktest::run` (D7): точка привязывается к «на ЭТОМ событии был ≥1 филл», а не к `curve.len() < fills.len()`. Попутно убрать мёртвый `let _ = signed_qty;` и висячий комментарий (`strategy_backtest.rs:85-88,117,156-162`) | engine-dev | `cargo test -p sim` GREEN, включая **ST-I-8g/8h**; `verify_M-07.sh` T5b PASS |
| 8 | ✅ | Review + merge `feat/M-07 → main` + post-merge §8 (CI/Deploy + VPS eyes-on: recorder НЕ должен измениться — M-07 инертен для прода) | reviewer | Done Block + §8 пруф |

Задачи 2 и 3 параллелятся; 4 зависит от 2+3; 5 от 4; 6 от 5.

## RED-тесты (sacred, architect-only)

- `crates/alpha/tests/red_alpha.rs` — AL-I-1 (детерминизм ×2), AL-I-2 (веса, конкретные
  числа), AL-I-3 (неизвестный signal_id игнорируется), AL-I-4 (stale-expiry; всё протухло
  → форкаста нет), AL-I-5 (clamp на мусорном value).
- `crates/portfolio/tests/red_portfolio.rs` — PF-I-1 (сайзинг), PF-I-2 (fail-safe кап,
  включая `i64::MAX`-edge), PF-I-3 (нет лимита → 0), PF-I-4 (позиция без форкаста → flatten).
- `crates/strategy/tests/red_strategy.rs` — ST-I-1 (diff 0→+X, +X→0), ST-I-2 (target ==
  current → нет интента), ST-I-3 (in-flight не дублирует; ttl → повтор), ST-I-4 (детерминизм),
  ST-I-5 (prefix-stability/replay-determinism — честная формулировка per C-004 M1).
- `crates/strategy/tests/structural.rs` — ST-I-6 (грепы: нет sim/venue/journal/tokio/reqwest/
  rand/SystemTime/Instant/HashMap), ST-I-7 (`OrderIntent` ровно в одном крейте).
- `crates/sim/tests/red_strategy_backtest.rs` — ST-I-8a..d (интенты дошли до биржи;
  согласованность позиции; детерминизм при seed; префикс филлов) + **ST-I-8e (C-004 C1):
  спай-стратегия — `run()` обязан доложить КАЖДЫЙ филл и подписать `FillReport` верно
  (сторона из интента, price/qty/fee/ts из `SimFill`); падает на пропуске `on_fill`,
  выдуманных филлах и подписи «всё как Buy»** + **ST-I-8f (C-004 M1): настоящий
  no-lookahead — мутация ТОЛЬКО будущих событий среза не меняет исполнения в прошлом**.
- **`crates/research-cli/tests/red_grid_strategy.rs` (C-004 C2)** — GR-I-1..7: дефолты D8;
  fail-closed валидация блока `strategy`; `params_hash` покрывает strategy+costs;
  returns = Δequity/capital_ref (D7); capital_ref-формула; **ПОВЕДЕНЧЕСКИЕ GR-I-6/7** —
  разный `max_position_e8` обязан дать разный оборот, а деадбенд шире лимита — ноль
  интентов (ad-hoc harness с фиксированным `qty=1.0` оба валит), ledger несёт канонический
  `cell_params_hash`. Именно эти два оракула, а не грепы, гейтят задачу 6.

**Анти-плацебо.** Все RED падают на скелете (`todo!()` → panic), а не «зелены против
заглушки». Ключевые оракулы спроектированы так, чтобы наивная реализация ПАДАЛА:
PF-I-2 подаёт `edge` вне диапазона (наивный `edge·max/1e8` без clamp → превышение капа);
ST-I-3 бьёт по «шлём интент на каждом тике» (наивный diff без in-flight → 2 интента);
ST-I-5 ловит любую подсмотренную вперёд информацию (префикс ≠ префикс);
AL-I-4 ловит «сигнал живёт вечно» (наивный last-value без expiry → форкаст навсегда).

## Acceptance

`bash scripts/verify_M-07.sh; echo "exit=$?"` → `VERDICT: PASS`, exit=0
(fmt + clippy -D warnings + alpha/portfolio/strategy/sim/research-cli тесты + регрессия
contracts/journal/book/signals + структурные грепы, включая канарейку «ad-hoc harness
`OpenPosition` из grid.rs удалён» и «grid использует `StrategyBacktest`»).

## Handoff

architect → **critic** (гейт §1: 3 новых крейта, ≥5 коммитов) → engine-dev (2,3 → 4 → 5)
→ research-dev (6) → tester (7) → reviewer (8, merge + §8).

## Close-out (2026-07-13, architect)

**Результат.** Мозг стратегии построен и сквозной: `SignalOut → Forecast → TargetPosition →
OrderIntent → sim::BacktestExchange`. Ad-hoc harness из `research-cli/src/grid.rs` (taker-in
по сигналу, taker-out по horizon) УДАЛЁН — бэктест больше не меряет логику, которой не будет
в live (DESIGN §1, равенство 2). Тот же `dyn Strategy` в P3+ погонит `runner` на живом фиде.

**Пруф (reviewer, §8):** ff-merge `5141fd9` → `main`, reviewer-коммит `656c7ca`; CI + Deploy
success; §8 eyes-on на VPS GREEN — recorder инертен (поведение не изменилось; инертность
подтверждена и структурно: дерево зависимостей recorder не содержит `alpha`/`portfolio`/
`strategy`/`sim` — канарейка T10). `verify_M-07.sh` 21/21 PASS, exit=0.

**Чему научил цикл (для следующих milestone'ов):**
1. **Гейт зелёный ≠ инвариант проверен.** Дважды ловили дыру НЕ в коде, а в моём RED-suite:
   C-004 C2 (задача 6 гейтилась грепами, удовлетворяемыми комментарием) и rev 3 (форма
   `equity_curve` не ассертилась НИГДЕ — verify показывал 20/20 при неверной реализации).
   Правило: **если инвариант живёт в данных, которые кто-то ниже по потоку считает метрикой,
   оракул обязан сверять эти данные ПОЭЛЕМЕНТНО с независимым пересчётом, а не по длине/факту
   существования.**
2. **Поведенческий оракул > греп.** GR-I-6/7 падали на РЕАЛЬНЫХ числах старого harness'а
   (одинаковый оборот при `max_position` ×30; торговля при глухом деадбенде) — такое
   комментарием не обойти.
3. **Цена ошибки определяет глубину оракула.** Дефект сэмплинга кривой не ронял ничего —
   он тихо занижал σ и завышал Sharpe, а этот путь ведёт к подписи founder'а (gates §6/§7).
   Такие места гейтятся отдельной строкой в verify (T5b), а не агрегатом.

**Следствия для M-08 (risk/killswitch/oms) — обязательны к учёту при планировании:**
- `portfolio::size` — **pre-trade sanity, НЕ риск-гейт**. `PF-I-2` (кап позиции) ничего не
  гарантирует про деньги: он лишь не даёт конвейеру решений ВЫРАЗИТЬ абсурдный размер.
- Fail-closed `RiskApproved<Order>` (приватный конструктор в `crates/risk`) встаёт **МЕЖДУ**
  `strategy` и `oms`: `OrderIntent` — намерение, не разрешение. Типовой барьер (`RK-I-1`):
  venue/oms принимает ТОЛЬКО `RiskApproved<_>`; `strategy` его сконструировать не может.
- M-08 трогает `risk`/`killswitch`/`oms` → **RISK-BLOCK** (`gates.md` §5): critic + **risk-critic**
  (сильная модель) обязательны; RED-suite `RK-I-1..10` + `INTG-I-*` обязан падать на заглушках.

**Открытый долг, унаследованный отсюда:** TD-015 (несопоставимость эпох trials-ledger) —
правило чтения закреплено в `.claude/rules/gates.md` §6.3/§6.4 (пункт 0 чек-листа risk-critic)
и в амендменте к M-04 задаче 8. Кода-долга нет; долг — дисциплина чтения ledger'а.
