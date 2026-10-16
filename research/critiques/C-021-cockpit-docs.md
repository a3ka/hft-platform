# C-021 — Critic Verdict — cockpit backend + AI copilot docs

**UTC:** 2026-07-22T16:37Z  
**Agent:** critic  
**Branch audited:** `origin/docs/cockpit-roadmap` at `4f4bf1c`  
**Worktree:** `/tmp/hft-critic-cockpit` (detached from `origin/docs/cockpit-roadmap`)  
**Verdict:** NOTE

## Verdict Justification

Ядро виз-бэкенда и AI-копилота согласовано с текущими `03/05/06/DESIGN`: слой стоит поверх
journal-first даталеера, T1 `Event/EventKind` не трогает, AI остаётся read-only и вне
детерминированного market-журнала. Блокирующих противоречий для продолжения `DESIGN.md` / `05`
/ `BACKLOG` / M-22 не нашёл, но есть три advisory-уточнения, которые нужно пронести в следующую
архитектурную правку.

## Pre-flight

Коммит-цепочка проверена:

```text
4f4bf1c docs(fa): viz-backend + ai-copilot FA (Слои 8-9) под кокпит
17eee98 research(depth-probe): фантом-тест дальних полос — инструмент + вывод
a6e5c50 docs(07): cockpit backend roadmap + decision-log (сессия 2026-07-22)
```

Scope over `HEAD~3..HEAD`: PASS.

```text
5 files changed, 634 insertions(+)
A crates/book/examples/depth_probe.rs
A docs/07-cockpit-backend-roadmap.md
A docs/fa/ai-copilot.md
A docs/fa/viz-backend.md
A research/data-quality/depth-probe-staleness.md
```

T1/contracts scope: PASS. `crates/contracts/**` and T1 schema files are untouched in the audited
range.

Tool checks:

```text
cargo clippy -p book --example depth_probe -- -D warnings
Finished `dev` profile ... target(s) in 4.72s
exit=0

cargo fmt --all -- --check
exit=0

cargo run --release -p book --example depth_probe -- /tmp/m10-vps-journal
Finished `release` profile ... target(s) in 11.82s
...
Binance/BTCUSDT max_reach% BID p10/p50/p90 = 48.15/54.10/55.45; ASK = 52.92/58.10/59.63
BinanceFutures/BTCUSDT max_reach% BID p10/p50/p90 = 53.97/57.98/59.46; ASK = 52.77/58.29/59.80
Hyperliquid/BTC max_reach% BID/ASK p50 = 0.03/0.03
```

## Requested Checks

### 1. Даталеер / bounded memory

PASS. `docs/fa/viz-backend.md:13-16` фиксирует read-only consumer model через
`journal::stream(dir, EpochFilter)` и `crates/book`; `docs/fa/viz-backend.md:98-101`
прямо запрещает `Vec<Event>` на 15 GB. Это согласовано с `docs/06-data-layer-and-storage.md:82-92`
и текущим `journal::stream`/`EpochFilter` API.

`depth_probe.rs` использует `journal::read_all` (`crates/book/examples/depth_probe.rs:41`), но
сам себя помечает как offline diagnostic (`crates/book/examples/depth_probe.rs:13-14`), не writer
и не production gateway. Это приемлемо как разовый диагностический пример; нельзя переносить этот
паттерн в M-22 Read Gateway.

### 2. Contract-clean

PASS with NOTE. Новые документы держат T1 ядро read-only: `docs/07-cockpit-backend-roadmap.md:44`,
`docs/07-cockpit-backend-roadmap.md:140-146`, `docs/fa/viz-backend.md:96-101`,
`docs/fa/ai-copilot.md:64-71`. Это соответствует `docs/05-contract-layer.md:46-61`.

NOTE: термин `Event` для Event Engine / AI contract конфликтует с уже занятым T1 `Event`.
В следующей governance-правке `05` лучше закрепить имя `SemanticEvent` или `AiEvent`, иначе M-26/M-27
можно прочитать как приглашение создать второй `Event` рядом с T1.

### 3. VB-I / AI-I реализуемость и анти-плацебо

PASS. VB-I-1..8 (`docs/fa/viz-backend.md:81-92`) и AI-I-1..7
(`docs/fa/ai-copilot.md:52-62`) выражены как тестируемые invariants: deterministic reducer,
live==replay, read-only grep canaries, additive export versioning, provenance field, no
formula computation under `formula_pending`, no order-egress, audit-log required.

Нужное уточнение для M-22: `docs/fa/viz-backend.md:24` пишет `(&[Event] | stream) -> Series`.
Для unit fixtures это нормально, но production path должен быть только bounded `journal::stream`.
В M-22 acceptance стоит явно разделить: slice API allowed for small tests, gateway/replay path must
stream with explicit `EpochFilter`.

### 4. Depth-probe / data-quality вывод

PASS with wording NOTE. Новый вывод корректно удерживает середину: полосы 3-30% видимы из diff-книги,
но не доказаны как биржевой факт глубже validated `<=1.3%`; планка перенесена на корректность книги
(`TD-016` evict + resync integrity), а Tardis оставлен для истории, не для "магической" глубины
(`docs/07-cockpit-backend-roadmap.md:92-95`, `research/data-quality/depth-probe-staleness.md:30-38`,
`docs/fa/viz-backend.md:66-79`).

NOTE: `research/data-quality/depth-probe-staleness.md:15` говорит, что у Hyperliquid "полосы >=3% пусты".
По stdout probe пусты именно дальние shells `3-5%`, `5-8%`, ...; cumulative bands `3%/5%/...` не пусты,
а насыщаются теми же top-20 уровнями. Следующая редакция memo должна использовать "shells >=3% пусты"
или "cumulative bands saturated by near-touch book", чтобы не сломать semantics TPP band sums.

### 5. Dependencies / contradictions / scope

PASS. Документы честно оставляют founder-decisions открытыми:
`formula_pending`, LLM provider, Tardis budget, replay window, universe size
(`docs/07-cockpit-backend-roadmap.md:148-154`, `docs/fa/viz-backend.md:103-107`,
`docs/fa/ai-copilot.md:73-77`). Это не скрытые implementation dependencies.

Есть один non-blocking hygiene issue: `docs/07-cockpit-backend-roadmap.md:69` и
`docs/07-cockpit-backend-roadmap.md:174` ссылаются на `research/data-quality/depth-probe-binance.md`,
которого нет на этой ветке. Если это внешний источник с `research/depth-probe`, так и оставить с
явной "external branch ref"; если это source-of-truth для следующих агентов, импортировать или заменить
на текущий `depth-probe-staleness.md`.

## Findings

### NOTE-1 — Rename semantic events before M-26/M-27

- Evidence: `docs/07-cockpit-backend-roadmap.md:144-145`,
  `docs/fa/ai-copilot.md:28-31`, `docs/fa/ai-copilot.md:70-71`.
- Impact: "Event" already means T1 `contracts::Event`. Reusing the noun for Event Engine output can
  create a false local-T1 or accidental `EventKind` pressure.
- Action: In the planned `05-contract-layer.md` governance section, name the new form
  `SemanticEvent`/`AiEvent` and state it is T-designate until a cross-language consumer requires
  promotion.

### NOTE-2 — Pin production streaming in M-22

- Evidence: `docs/fa/viz-backend.md:24` allows `&[Event] | stream`; `docs/fa/viz-backend.md:100-101`
  correctly bans `Vec<Event>` on 15 GB.
- Impact: A dev can over-index on the pure reducer signature and build a gateway from materialized
  history.
- Action: M-22 Read Gateway acceptance should require `journal::stream(dir, EpochFilter::...)`,
  bounded memory, and a grep/test canary against `journal::read_all` in gateway/replay production paths.

### NOTE-3 — Clarify Hyperliquid cumulative-band wording

- Evidence: `research/data-quality/depth-probe-staleness.md:15`; reproduced stdout shows HL shells
  beyond 3% are zero while cumulative bands are non-zero and saturated.
- Impact: Not blocking for Binance-first MVP, but imprecise wording can mislead later TPP COIN
  decisions.
- Action: Say "HL deep shells >=3% are empty; cumulative bands above reach contain only near-touch
  top-20 liquidity."

## Recommended Next Action

Proceed with architect continuation:

1. Add the remaining Class-A doc edits: `DESIGN.md` phase row, `docs/05-contract-layer.md`
   T-designate/export-v2/AI-context governance, and `milestones/BACKLOG.md` viz-first reorder.
2. Carry NOTE-1..3 into that edit set.
3. Author M-22 Read Gateway with explicit bounded streaming + `EpochFilter` acceptance.
4. Then route the completed Class-A doc set through reviewer before ACTIVE/main.

## Confidence

High. I read the requested docs, local critic/scope/branch/gate rules, the audited diff, current
contract and data-layer sources, relevant TD references, and executed the new `depth_probe` example.
