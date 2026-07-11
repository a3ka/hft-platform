# hft-platform — Master Rules (CLAUDE.md)

Операционные правила проекта. **Источник правды по архитектуре — `docs/DESIGN.md`**
(+ `docs/00-06`, `docs/fa/*`). Этот файл — как мы РАБОТАЕМ; DESIGN — что мы строим.
Порядок чтения при старте агента: `DESIGN.md` → `docs/04-workflow.md` →
`docs/05-contract-layer.md` → релевантный `docs/fa/<module>.md`.

## Операционные принципы (BINDING)

- **Пользователь — оркестрационный диспетчер.** Агенты не вызывают друг друга; передача
  через Handoff-блоки (`.claude/rules/handoff-block.md`).
- **Journal-first + детерминизм.** Всё — событие в упорядоченном журнале; `DET-I-1`
  (бит-идентичный replay) sacred. В доменном коде — никакого недетерминизма (нет
  wall-clock/`rand()`/итерации по HashMap без сортировки в редьюсерах).
- **RED-first TDD обязателен.** Architect пишет падающий тест ПЕРВЫМ; тест — спецификация;
  dev делает GREEN. Тест, зелёный против заглушки, — дефект (анти-плацебо).
- **Риск fail-closed.** `RK-I-1..10` sacred: ордер только через `RiskApproved`; байпас-поверхности
  не существует; неизвестный вход → reject; отказ инфраструктуры → торговля стоит.
- **LLM НЕ в горячем торговом цикле.** LLM влияет на рантайм только на дизайн-тайме через
  границы A/B/C (`docs/03-integration-contract.md`) с подписью founder'а.
- **Атомарные коммиты** (одна задача = ≥1 коммит, ссылка на milestone/task).
- **Done Block** (сырой stdout гейтов) перед «готово»; **acceptance-скрипт — реальный гейт**
  (`set -euo pipefail`, exit≠0 на FAIL).
- **Auto-push только при зелёных гейтах.** Любое касание `risk`/`killswitch`/`oms`/`venues`/
  `contracts` → обязательный reviewer + **risk-critic**.
- **Push ≠ конец цикла: прод живёт на VPS.** После push в `main` — post-merge
  деплой-гейт (`.claude/rules/gates.md` §8): дождаться CI+Deploy success + проверить
  VPS по ssh (контейнер healthy, heartbeat свежий); пруф — в close-out отчёт.
  Milestone не закрывается поверх красного/непроверенного прода.

## Делегирование и маршрутизация моделей (экономия)

| Роль | Модель-класс | Зона |
|---|---|---|
| architect (Fable) | дорогая — экономно | архитектура, `contracts` T1, RED-тесты, verify, sacred |
| **risk-critic** | сильная (не экономим) | safety-путь + отчёты стратегий (асимметричная цена ошибки) |
| reviewer | сильная | PR-гейт: scope, Done Block, contract Block-C, риск-инварианты |
| critic | средняя | plan-time гейт (триггеры в `.claude/rules/gates.md`) |
| engine/venue/signal/research-dev | кодовая дешёвая/средняя | impl по milestone + RED |
| explore/tester | дешёвая | разведка / прогон тестов |

## Scope-guard (кратко; полное — `.claude/rules/scope-guard.md`)

- Квант-агенты пишут ТОЛЬКО `crates/signals/` + `research/`.
- `crates/risk`, `crates/killswitch`, `crates/contracts` (T1-типы), `*/tests/` (RED-спеки),
  `scripts/verify_*.sh` — **sacred** (architect-only). Выход за зону → `!!! SCOPE VIOLATION
  REQUEST !!!` + стоп.
- `contracts/` (T1) меняется только через contract-RFC (`CT-I-2`).

## Commit protocol

Conventional commit: `type(scope): subject`. Ссылка на milestone/task. Без co-author трейлеров.

## Cross-references
- `docs/DESIGN.md` — мастер-архитектура (§-структура, инварианты, роадмап)
- `docs/04-workflow.md` — operating model (роли, milestone-цикл, гейты)
- `docs/05-contract-layer.md` — T1 governance
- `docs/fa/*.md` — per-module Functional Architecture (спека каждого крейта)
- `PROJECT-STATE.md` (reviewer-owned) — что реализовано
- `TECH-DEBT.md` (reviewer-owned) — открытый долг
