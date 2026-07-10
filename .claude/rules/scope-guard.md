# Scope Guard — обязателен для каждого агента

Источник: `docs/04-workflow.md` §1 (роли/зоны) + §3 (scope-guard гейт) + `docs/DESIGN.md`
§2 (слои) + §6 (граница A). Зона — по путям. Нарушение = откат изменений.

## Таблица владения

| Агент | РАЗРЕШЕНО | ЗАПРЕЩЕНО (всё остальное) |
|---|---|---|
| **architect** (Fable) | `docs/`, `contracts/` T1-типы (только через contract-RFC, `.claude/rules/gates.md`), `*/tests/` (RED-спеки), `scripts/verify_*.sh`, `milestones/M-NN-*.md` | ЛЮБОЙ impl-код (`crates/*/src/`, `research/*/src/`), билд-конфиги (`Cargo.toml` deps секции) |
| **signal-engineer** | ТОЛЬКО `crates/signals/**` + `research/specs/`, `research/hypotheses/` (по назначению) | `journal/venues/book/oms/risk/killswitch/portfolio/strategy` — запрещены безусловно (граница A, `docs/03-integration-contract.md` §4) |
| **engine-dev** (journal/book/oms/sim/runner/alpha/portfolio/strategy) | `crates/{journal,book,oms,sim,runner,alpha,portfolio,strategy}/src/**` (impl, НЕ типы) | `crates/risk/`, `crates/killswitch/`, `crates/contracts/` (T1), другие крейты вне своего списка |
| **venue-dev** | `crates/venue-*/src/**` (адаптеры бирж) | всё вне `venue-*`; `risk`/`killswitch` |
| **research-dev** | `crates/research-cli/src/**`, `research/reports/` (генерация) | `crates/{risk,killswitch,journal,oms}`, `research/trials-ledger.json` (append-only механизм — не ручная правка) |
| **risk-critic** | ТОЛЬКО `research/critiques/*.md` (вердикт-файлы) | ничего в коде; не пишет milestone'ы |
| **critic** | ТОЛЬКО `research/critiques/` verdict-файлы (не milestone'ы) | `crates/`, `docs/`, `research/registry/` |
| **tester** | read-only на код; отчёт в чат/handoff | никаких правок кода |
| **reviewer** | `PROJECT-STATE.md`, `TECH-DEBT.md`, PR-комменты | всё остальное |

## SACRED (architect-only, безусловно, для ВСЕХ dev-агентов)

- `crates/risk/**`, `crates/killswitch/**` — риск-слой; ни один dev-агент не пишет сюда,
  включая свои же тесты (`RK-I-1..10`, `docs/DESIGN.md` §4).
- `crates/contracts/**` (T1-типы: `Event`/`EventKind`, `SignalRegistry` entry, `Ctl(ParamChange)`,
  `SignalSpec`, `ValidationReport`, `TrialsLedger` entry, `Decision`) — только через
  contract-RFC, см. `.claude/rules/gates.md` + `docs/05-contract-layer.md` §4.
- `*/tests/` (везде) — RED-спеки architect'а. dev НЕ правит тест, даже если "тест неправильный".
- `scripts/verify_*.sh` — acceptance-гейты architect'а.
- `research/registry/signals.json` (граница B) — движок читает, деск пишет ТОЛЬКО через
  подписанное решение (`docs/03-integration-contract.md` §2, `INTG-I-2`).
- `research/decisions/D-NNN.md` — пишет ТОЛЬКО founder (Ed25519-подпись).

## Milestone-файлы — carve-out статус-колонки

Dev-агент МОЖЕТ править ТОЛЬКО колонку Status в §Tasks активного milestone'а
(`⏳ OPEN → 🚧 IN_PROGRESS → ✅ DONE`), названного в его инвокации. Любая другая правка
`milestones/M-NN-*.md` (Objective/Allowed paths/RED-тесты/Acceptance-скрипт/Handoff) —
architect-only.

## Билд-конфиги (Cargo.toml) — shared-access правило

Каждый engine-dev/venue-dev/research-dev МОЖЕТ добавлять СВОИ зависимости в
`[dependencies]` своего крейта. ЗАПРЕЩЕНО: удалять/менять чужие зависимости, править
workspace-root `Cargo.toml`, менять lint/test-конфиг. Reviewer проверяет:
`git diff <crate>/Cargo.toml` показывает ТОЛЬКО добавления агента.

## Формат SCOPE VIOLATION REQUEST

Если работа блокируется правкой вне своей зоны — НЕ трогать файл. Написать ровно:

```
!!! SCOPE VIOLATION REQUEST !!!
Agent: [твоя роль]
Current task: [milestone/task]
File I need to change: [полный путь]
Owner: [architect | другой агент]
What change: [точное описание]
Why I cannot proceed without it: [конкретная причина]
!!! END SCOPE VIOLATION REQUEST !!!
```

Затем **STOP и WAIT** — architect (или founder через founder-подпись, если это
`risk`/`killswitch`/`contracts`) решает.

## Три жёстких правила

1. **Не трогай чужой код.** Увидел баг вне своей зоны — сообщи через SCOPE VIOLATION
   REQUEST, не чини.
2. **Не трогай код вне текущей задачи.** Работающий протестированный код из прошлого
   milestone'а не улучшается по инициативе dev-агента.
3. **Нет спецификации → нет работы.** Нет RED-теста и нет verify-скрипта → не реализуй.
   Есть RED-тест без скрипта → реализуй по тесту. Нет RED-теста, но есть FAIL-скрипт →
   реализуй по скрипту.

## Тесты — sacred

Файлы `*/tests/*` — спецификация architect'а. Правка теста dev-агентом = не "фикс", а
нарушение. Странный тест → SCOPE VIOLATION REQUEST, не самостоятельная правка.

## Cross-references

- `docs/04-workflow.md` §1 (роли), §3 (scope-guard в гейтах)
- `docs/DESIGN.md` §2 (слои/крейты), §4 (RK-I-*), §6 (границы A/B/C)
- `docs/03-integration-contract.md` §4 (граница A — код сигнала)
- `.claude/rules/gates.md` (contract-RFC + risk-block)
