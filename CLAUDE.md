# hft-platform — Master Rules (CLAUDE.md)

Операционные правила проекта. **Источник правды по архитектуре — `docs/DESIGN.md`**
(+ `docs/00-06`, `docs/fa/*`). Этот файл — как мы РАБОТАЕМ; DESIGN — что мы строим.
Порядок чтения при старте агента: `DESIGN.md` → `docs/04-workflow.md` →
`docs/05-contract-layer.md` → релевантный `docs/fa/<module>.md`.

## ⚠️ STARTUP-ПРОТОКОЛ — ВЫПОЛНЯЕТСЯ ПЕРВЫМ, ВСЕГДА (BINDING, founder 01.08)

**Это первое действие в новой сессии и первое действие после сжатия/обновления контекста.**
Не «когда понадобится», не «если задача сложная» — всегда, до любой работы.

### Для architect (и любой роли, ведущей сессию)

1. `CLAUDE.md` (этот файл) → `.claude/rules/*.md` ВСЕ (gates, testing, scope-guard,
   commit-discipline, branch-hygiene, handoff-block) → `.claude/agents/<своя роль>.md`.
2. `docs/SESSION-HANDOFF.md` — состояние проекта между сессиями.
3. **`docs/ORCHESTRATION-STATE.md`** — что запущено, куда пушит, что делать с результатом.
4. `docs/PENDING-SIGNATURE.md` — что ждёт founder'а (границу C не переступать).
5. `docs/DESIGN.md` → релевантный `docs/fa/<модуль>.md`.
6. `PROJECT-STATE.md` + `TECH-DEBT.md` — что сделано и какой долг открыт.

### Для КАЖДОГО запускаемого субагента

**Мандат обязан начинаться с блока «Startup-протокол»** — перечнем файлов, которые агент
читает ДО работы: `CLAUDE.md`, релевантные `.claude/rules/*`, свой профиль
`.claude/agents/<роль>.md`, и предметные документы задачи. Агент без загруженного протокола
не знает ни зон ответственности (scope-guard), ни формата Done Block, ни правил push —
и нарушает их не по злому умыслу, а по неведению. Проверено практикой: все инциденты
«коммит завис в worktree», «вердикт вернулся одним словом», «правка ушла за зону» —
следствие мандата без протокола.

### Почему это вынесено в начало файла

Контекст сессии исчерпывается и сжимается; при сжатии первым теряется именно то, что было
прочитано в начале и давно не упоминалось — то есть правила. Роль продолжает работать,
«помня» задачу и забыв дисциплину. Явный протокол в шапке главного файла — единственное,
что переживает любое сжатие: он перечитывается заново.

## Операционные принципы (BINDING)

- **Пользователь — оркестрационный диспетчер.** Агенты не вызывают друг друга; передача
  через Handoff-блоки (`.claude/rules/handoff-block.md`).
- **Journal-first + детерминизм.** Всё — событие в упорядоченном журнале; `DET-I-1`
  (бит-идентичный replay) sacred. В доменном коде — никакого недетерминизма (нет
  wall-clock/`rand()`/итерации по HashMap без сортировки в редьюсерах).
- **RED-first TDD обязателен.** Architect пишет падающий тест ПЕРВЫМ; тест — спецификация;
  dev делает GREEN. Тест, зелёный против заглушки, — дефект (анти-плацебо).
- **АРТЕФАКТ, А НЕ РОЛЬ** (закреплено 2026-08-03 по `R-031`, `C-059`). Гейт считается
  пройденным только если предъявлен ФАЙЛ: вердикт критика `research/critiques/C-NNN.md`,
  отчёт reviewer'а `research/reviews/R-NNN.md`, Done Block с exit-кодами. «Агент роли X
  отработал» — не доказательство: роль подтверждается git-личностью, которая ставится один
  раз при создании worktree и не переустанавливается при смене роли в цепочке. Три milestone'а
  (M-32/33/34) уехали в прод без единого артефакта гейта именно так — и это было
  НЕНАБЛЮДАЕМО, пока вердикт reviewer'а жил в переписке. Проверка стоит одной команды:
  `ls research/reviews/ research/critiques/ | grep M-NN`. Полное правило —
  `.claude/rules/gates.md` §10.
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
| critic | средняя — **но сильная** на raw-гейте (см. ниже) | plan-time гейт (триггеры в `.claude/rules/gates.md`) |
| engine/venue/signal/research-dev | кодовая дешёвая/средняя | impl по milestone + RED |
| explore/tester | дешёвая | разведка / прогон тестов |

**RAW-гейт — critic на сильной модели (решение founder'а 2026-08-02).** Если milestone меняет
**раскладку/формат журнала** (шардирование, схема сегментов, `seq`-пространство, эпохи) ИЛИ
`contracts` T1 — critic поднимается на сильную модель, как risk-critic. Причина та же
асимметрия: ошибка в safety-пути стоит депозита, ошибка в раскладке журнала обнаруживается
через месяцы и стоит переписывания всего накопленного (`docs/PENDING-SIGNATURE.md` П-005).
Детали и формулировка триггера — `.claude/rules/gates.md` §1.

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
