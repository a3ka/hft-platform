# M-07 — Strategy brain: `alpha` → `portfolio` → `strategy` (мозг стратегии)

STATUS: 🚧 PROPOSED. Authored: architect (Opus), 2026-07-13.
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
| D7 | Метрики грида после замены harness'а | `BacktestReport` несёт `fills`, `intents`, `cash_e8`, `positions`, `turnover_e8`, `equity_curve_e8` (mark-to-market по последнему mid на каждом событии с филлом). `research-cli` считает returnsᵢ = `Δequity_e8 / capital_ref_e8`, где `capital_ref_e8 = max_position_e8 × mid₀` ячейки (детерминированно, задокументировать в отчёте). Числа PnL ОТЛИЧАЮТСЯ от пилота v1 — это ожидаемо (мерили другую логику); ledger append-only, старые записи не переписываются |
| D8 | Совместимость grid-ячеек | Ячейка = `ObiParams` + опциональный блок `strategy`: `{max_position_e8=1e8, min_order_e8=1e6, intent_ttl_ms=1000, marketable_margin_bp=100, kind="taker"}` (дефолты при отсутствии) → существующие спеки `S-001` парсятся. `params_hash` меняется → это НОВЫЕ trial-записи, честно |

## Allowed / Forbidden paths (scope-guard)

| Агент | Allowed | Forbidden |
|---|---|---|
| architect | `milestones/`, `docs/fa/strategy-brain.md`, `crates/{alpha,portfolio,strategy}/{Cargo.toml,src}` — **ТОЛЬКО T2-типы + trait-сигнатуры + `todo!()`-стабы** (скелет, паттерн M-04 task 1), `crates/*/tests/**` (RED, sacred), `scripts/verify_M-07.sh`, workspace-root `Cargo.toml` (members), **carve-out A1:** `crates/sim/src/types.rs` + `lib.rs` + `Cargo.toml` — ТОЛЬКО релокация T2 `OrderIntent`/`OrderKind` (удаление определения + `pub use strategy::...` + path-dep). Обоснование: релокация T2-формы атомарна по определению — половинчатое состояние (два определения одного типа) = ровно тот дефект, против которого стоит ST-I-7; логика не пишется | любой impl-код (тела `todo!()`), `crates/research-cli/src/**` |
| engine-dev | `crates/{alpha,portfolio,strategy}/src/**` + их `Cargo.toml` (свои deps), `crates/sim/src/**` + `crates/sim/Cargo.toml` (свои deps) | `*/tests/**` (sacred), `crates/{contracts,risk,killswitch,journal,venue-*}/**`, `crates/research-cli/**`, `scripts/**`, `docs/**` |
| research-dev | `crates/research-cli/src/**` + `crates/research-cli/Cargo.toml` | всё остальное; `research/trials-ledger.jsonl` (ручная правка) |
| tester | read-only; прогон `scripts/verify_M-07.sh` | правки кода |
| reviewer | `PROJECT-STATE.md`, `TECH-DEBT.md`, merge `feat/M-07 → main` + §8 деплой-гейт | код |

## §Tasks

| # | Status | Задача | Агент | Verify |
|---|---|---|---|---|
| 1 | ✅ | Скелеты `alpha`/`portfolio`/`strategy` (T2-типы, трейты, `todo!()`), релокация `OrderIntent` (carve-out A1), workspace members, RED-suite, `verify_M-07.sh` | architect | workspace компилируется; fmt+clippy зелёные; RED-suite ПАДАЕТ (todo!) |
| 2 | ⏳ | `alpha` impl: `LinearAlpha` (комбинация весов, stale-expiry, clamp, детерминированный порядок) | engine-dev | `cargo test -p alpha` GREEN (AL-I-1..5) |
| 3 | ⏳ | `portfolio` impl: `RiskBudget` + `size()` (сайзинг, fail-safe кап, flatten, fail-closed без лимита) | engine-dev | `cargo test -p portfolio` GREEN (PF-I-1..4) |
| 4 | ⏳ | `strategy` impl: `DirectionalStrategy` (books → signals → alpha → portfolio → diff → интенты; in-flight + ttl; маркетабельная цена) | engine-dev | `cargo test -p strategy` GREEN (ST-I-1..7) |
| 5 | ⏳ | `sim::StrategyBacktest` harness: прогон `dyn Strategy` через `BacktestExchange`, мост `SimFill → FillReport`, `BacktestReport` (D3/D7) | engine-dev | `cargo test -p sim` GREEN (ST-I-8 + регрессия SM-I-*) |
| 6 | ⏳ | `research-cli/grid.rs`: снять ad-hoc harness (`OpenPosition`/`Action`), перевести ячейку на `sim::StrategyBacktest` + strategy-пайплайн; ledger/metrics/стресс-режимы семантически сохранены (D7/D8) | research-dev | `cargo test -p research-cli` GREEN; грепы T8 verify |
| 7 | ⏳ | Прогон `scripts/verify_M-07.sh` на чистом чекауте | tester | `VERDICT: PASS`, exit=0 |
| 8 | ⏳ | Review + merge `feat/M-07 → main` + post-merge §8 (CI/Deploy + VPS eyes-on: recorder НЕ должен измениться — M-07 инертен для прода) | reviewer | Done Block + §8 пруф |

Задачи 2 и 3 параллелятся; 4 зависит от 2+3; 5 от 4; 6 от 5.

## RED-тесты (sacred, architect-only)

- `crates/alpha/tests/red_alpha.rs` — AL-I-1 (детерминизм ×2), AL-I-2 (веса, конкретные
  числа), AL-I-3 (неизвестный signal_id игнорируется), AL-I-4 (stale-expiry; всё протухло
  → форкаста нет), AL-I-5 (clamp на мусорном value).
- `crates/portfolio/tests/red_portfolio.rs` — PF-I-1 (сайзинг), PF-I-2 (fail-safe кап,
  включая `i64::MAX`-edge), PF-I-3 (нет лимита → 0), PF-I-4 (позиция без форкаста → flatten).
- `crates/strategy/tests/red_strategy.rs` — ST-I-1 (diff 0→+X, +X→0), ST-I-2 (target ==
  current → нет интента), ST-I-3 (in-flight не дублирует; ttl → повтор), ST-I-4 (детерминизм),
  ST-I-5 (no-lookahead: префикс-стабильность).
- `crates/strategy/tests/structural.rs` — ST-I-6 (грепы: нет sim/venue/journal/tokio/reqwest/
  rand/SystemTime/Instant/HashMap), ST-I-7 (`OrderIntent` ровно в одном крейте).
- `crates/sim/tests/red_strategy_backtest.rs` — ST-I-8 (интеграция strategy→BacktestExchange:
  прогон ×2 бит-идентичен; позиция стратегии == нетто филлов; интенты дошли до биржи).

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
