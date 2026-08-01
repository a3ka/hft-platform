# R-015 — `feat/contract-gates` (rev3, закрытие F-4): **APPROVED**

- Дата (UTC): 2026-08-01
- Ветка: `feat/contract-gates`, HEAD `2ca33c0`
- База: `origin/main` `d6ff609`
- Предыдущие вердикты цепочки: `R-006` (CHANGES REQUESTED — F-1/F-2), `R-012`
  (CHANGES REQUESTED — F-4)
- Роль: reviewer (PR-гейт, `.claude/rules/gates.md` §4). Прогоны — на ЧИСТОМ worktree
  `/tmp/hft-rev-cg-final` (detached `origin/feat/contract-gates`), не в общем чекауте.

## Вердикт

**APPROVED.** F-4 закрыта по существу — проверено НЕЗАВИСИМО, не по self-test'у: четыре
конструкции, которых классификатор не знает (`patternProperties`, `not`, `format`,
`minimum`), внесённые в РЕАЛЬНУЮ `crates/contracts/schema/event.schema.json` РЯДОМ с
additive-соседом (условие маскировки из R-012), теперь дают `CLASS=breaking`, а не
`additive`/`none`.

Одновременно заведена **F-5 (MAJOR, долг, НЕ блокер)**: тот же класс fail-open выжил на
ключах ВНУТРИ `_HANDLED_KEYS`, чьи правила неполны. Основание не блокировать — в §F-5.

## Block-scope — ✅

`git diff --name-only origin/main...HEAD`:

```
.github/workflows/ci.yml
research/reviews/R-006-contract-gates.md
research/reviews/R-012-contract-gates-rev2.md
scripts/contracts_validate_fixtures.py
scripts/diff_contract_schema.py
scripts/diff_contract_schema.sh
scripts/tests/red_ct_rfc_atomic.sh
scripts/tests/red_diff_contract_schema.sh
scripts/verify_contracts.sh
scripts/verify_ct_rfc_atomic.sh
```

`crates/**` — не тронут ВООБЩЕ (ни `contracts/`, ни любой другой крейт). `docs/**`,
`milestones/**`, `research/{critiques,specs,registry}` — не тронуты.

## Block-C (contract governance) — ✅ не применимо, доказано машинно

`crates/contracts/**` в дифе отсутствует ⇒ contract-RFC не требуется. Подтверждено самим
гейтом ветки:

```
$ bash scripts/verify_ct_rfc_atomic.sh origin/main
PASS  crates/contracts/src/** не тронут — атомарность CT-RFC пакета не применима

VERDICT: PASS
exit=0
```

## Block-risk — ✅ не применимо

Диф не трогает `crates/risk`, `crates/killswitch`, `crates/oms`, `crates/venue-*`,
`crates/contracts` — RISK-BLOCK (`gates.md` §5) не срабатывает, risk-critic не требуется.
Изменения — только shell/python-гейты и проводка CI.

## Block-DoneBlock — прогоны reviewer'а (сырой вывод)

```
$ bash scripts/verify_contracts.sh
PASS  S0 setup-guard (генератор+схемы+фикстуры+python-jsonschema+cargo)
PASS  S1 схема ↔ типы (regen == committed, CT-I-4)
PASS  S2 фикстуры ↔ схема (valid PASS / invalid REJECT)
PASS  S3 CT-I-1 канарейка EventKind (единственная дефиниция в contracts)
PASS  S4 cargo test -p contracts (roundtrip + RFC RED-suite GREEN)

VERDICT: PASS
exit=0

$ bash scripts/verify_ct_rfc_atomic.sh origin/main
PASS  crates/contracts/src/** не тронут — атомарность CT-RFC пакета не применима

VERDICT: PASS
exit=0

$ bash scripts/diff_contract_schema.sh origin/main
CLASS=none

PASS  схема crates/contracts/schema не изменилась между origin/main и HEAD

VERDICT: PASS
exit=0

$ bash scripts/tests/red_diff_contract_schema.sh
PASS  D1 схема не менялась — CLASS=none, PASS
PASS  D2 новое опциональное свойство — CLASS=additive, PASS
PASS  новый файл схемы — CLASS=additive, PASS
PASS  W2 новое ОБЯЗАТЕЛЬНОЕ свойство без бампа — CLASS=breaking, FAIL
PASS  D4 BREAKING + бамп SCHEMA_VERSION — PASS
PASS  W3 смена type — CLASS=breaking, FAIL
PASS  D3 удалён вариант oneOf — CLASS=breaking, FAIL
PASS  файл схемы удалён — CLASS=breaking, FAIL
PASS  F-1 РЕГРЕССИЯ-ГВАРД: $ref-цель свойства изменена — CLASS=breaking (НЕ none!), FAIL
PASS  F-1 РЕГРЕССИЯ-ГВАРД: $ref внутри items массива изменена — CLASS=breaking, FAIL
PASS  F-4 РЕГРЕССИЯ-ГВАРД: allOf $ref retarget изолированно — CLASS=breaking, FAIL
PASS  F-4 ГЛАВНАЯ РЕГРЕССИЯ: неразобранный allOf-диф НЕ маскируется соседним additive — CLASS=breaking (НЕ additive!), FAIL
PASS  F-4 maxLength-сужение НЕ маскируется соседним additive — CLASS=breaking (НЕ additive!), FAIL
PASS  D5 несуществующий base-ref — FAIL (setup-guard, fail-closed)
PASS  D6 классификатор .py отсутствует — FAIL (setup-guard, fail-closed)

VERDICT: PASS
exit=0
```

## F-4 — ✅ ЗАКРЫТА (независимая проверка, не self-test)

Self-test автора — не доказательство: он проверяет ровно те сценарии, которые автор
придумал. Прогнал СВОИ мутации на РЕАЛЬНОЙ `crates/contracts/schema/event.schema.json`,
каждая — «неизвестная классификатору конструкция + additive-сосед в том же файле»
(условие маскировки, из-за которого F-4 была заведена):

```
--- patternProperties добавлен + additive-сосед
additive definitions.Level.properties.new_opt — новое опциональное свойство
BREAKING definitions.Level.patternProperties — узел изменился, но для ключа 'patternProperties' нет правила классификатора ...
CLASS=breaking

--- not добавлен + additive-сосед
BREAKING definitions.Level.not — ... нет правила классификатора ...
CLASS=breaking

--- format сужен на поле + additive-сосед
BREAKING definitions.Level.price.format — ... нет правила классификатора ...
CLASS=breaking

--- minimum добавлен на поле + additive-сосед
BREAKING definitions.Level.size.minimum — ... нет правила классификатора ...
CLASS=breaking
```

4 из 4 — `breaking`. Механика фикса корректна: safety-net перенесён с уровня ФАЙЛА
(`classify_repo`, срабатывал только при ПУСТОМ списке Change для всего файла) на уровень
УЗЛА (`scripts/diff_contract_schema.py:217-228`), т.е. соседний распознанный additive
больше не гасит неразобранный диф.

Дополнительно `additionalProperties` расширен с «сужение до false» на ЛЮБОЕ изменение
(`scripts/diff_contract_schema.py:168-173`) — закрывает форму «схема-как-значение» и
переход `false → true`.

## F-5 (НОВАЯ, MAJOR, долг — НЕ блокер): маскировка выжила на ключах ИЗ `_HANDLED_KEYS` с неполными правилами

Fallback F-4 (`scripts/diff_contract_schema.py:217`) считает подозрительными ТОЛЬКО ключи
вне `_HANDLED_KEYS`/`_DOC_ONLY_KEYS`. Для ключей, которые классификатор считает «своими»
(`oneOf`, `items`, `type`, `required`), но разбирает лишь частично, дифф по-прежнему
теряется молча, и ровно при том же условии — additive-сосед в том же файле. Все четыре
воспроизведения — на РЕАЛЬНОЙ `event.schema.json`, вывод сырой:

| # | Мутация (+ additive-сосед) | Ожидание | Факт |
|---|---|---|---|
| A | вариант `oneOf` c ДВУМЯ `required` (ключ варианта = `None`, `diff_contract_schema.py:57-65`) меняет `type` поля | breaking | `CLASS=additive` |
| B | `items` в СПИСОЧНОЙ форме (tuple-валидация), элемент `[0]` `string→boolean` (`:178-181` берёт `items` только как `dict`) | breaking | `CLASS=additive` |
| C | узлу без `type` ДОБАВЛЕН `type` (сужение «любое → integer»; `:158` требует `type` в ОБОИХ) | breaking | `CLASS=additive` |
| D | поле убрано из `required` (`:143` смотрит только `head_req - base_req`) — в Rust это ровно `#[serde(default)]` на существующем поле | хотя бы упоминание | `CLASS=additive`, в выводе НИ СТРОКИ про это изменение |

Тот же диф БЕЗ additive-соседа даёт `CLASS=breaking` (срабатывает файловый safety-net) —
расхождение «изолированно breaking, с соседом additive» и есть подпись fail-open.

**Почему это НЕ блокирует merge (осознанное решение reviewer'а, не недосмотр):**
1. Схема **генерируется** и её идентичность типам проверяется тем же пакетом
   (`verify_contracts.sh` S1, CT-I-4) ⇒ в репо не может появиться форма, которую
   schemars не эмитит. Фактическим замером текущей схемы: вариантов `oneOf` без
   однозначного ключа — 0, `items`-списков — 0 (A и B сегодня недостижимы; это латентный,
   а не активный дефект). Для сравнения: `allOf` из F-4 в реальной схеме ПРИСУТСТВУЕТ
   (`$.definitions.ReconAudit.properties.action`) — вот почему F-4 блокировала.
2. Достижим сегодня практически только класс D (`#[serde(default)]`), причём он не ломает
   `CT-I-3` (чтение старых журналов), а бьёт по гарантиям для консюмеров.
3. Merge строго увеличивает детекцию: на `main` контрактных гейтов нет ВООБЩЕ, и ни один
   сценарий, который `main` ловит сегодня, после merge ловиться не перестаёт.
4. T1 остаётся под человеческими гейтами — contract-RFC + critic + risk-critic + reviewer
   (`gates.md` §4/§5); машинный классификатор — defense-in-depth, а не единственный барьер.

**Что это значит для читателя вывода гейта (важно, TD-011-класс):** `VERDICT: PASS`
классификатора означает «известные правила не нашли ломающего изменения», а НЕ «ломающих
изменений нет». До закрытия F-5 вывод нельзя цитировать как доказательство совместимости.

**Следующее появление этого класса в `diff_contract_schema.py` считается блокирующим.**
Дизайн фикса — зона architect (`gates.md` §4, граница reviewer↔architect: я описываю
дефект, не проектирую защиту). Занесено в `TECH-DEBT.md` как **TD-058**.

## Шум классификатора — ✅ низкий (проверено на реальной истории)

Прогнал классификатор на всех коммитах `origin/main`, менявших `crates/contracts/schema`
(каждый — против своего родителя):

```
e06e48a feat(M-35): CT-RFC-05 — MdPayload::MarginInventory  -> CLASS=additive
6af0aef feat(M-18): CT-RFC-04 — MdPayload::L2Delta T1       -> CLASS=additive
64c0a9e contract(CT-RFC-03): SysEvent::ReconDivergence      -> CLASS=additive
0835569 feat(CT-RFC-02): C-005 C1 — полный RFC-пакет        -> CLASS=additive
```

Ни одного ложного `breaking` на реальных аддитивных RFC — усиленный node-fallback не
превратил гейт в шумовой (инструмент с ложными красными отключают, и это хуже отсутствия).

## Дыра `CT-RFC-05` — ✅ признана честно, не замаскирована (перепроверено)

`docs/rfc/` содержит `CT-RFC-01..04`; файла `CT-RFC-05-*.md` нет, при этом `CT-RFC-05`
упомянут в репозитории 83 раза (`crates/contracts/src/lib.rs:24`, `CHANGELOG.md:6`,
`crates/venue-binance/src/lib.rs:807`, `research/critiques/C-024.md`, …). В скриптах ветки
**нет ни hardcode `CT-RFC-05`, ни списка исключений, ни тихого `skip`** (проверено грепом
по `scripts/**` — ни одного вхождения). Гейт не валит CI задним числом только потому, что
проверяет ДИФФ события (`push=before` / `pull_request=base.sha`), а исторический коммит
`e06e48a` уже на `main` — это структурное свойство diff-гейта, а не спрятанное исключение.
Нормативный остаток (ретро-документ `docs/rfc/CT-RFC-05-*.md` — зона architect) остаётся
долгом; заведён отдельной записью в `TECH-DEBT.md` (**TD-059**).

## Проводка CI — ✅

`.github/workflows/ci.yml`: добавлен job `contracts` (verify_contracts → база события →
verify_ct_rfc_atomic → его self-test → diff_contract_schema → его self-test) и он ВКЛЮЧЁН
в `needs`/условие `status-check` — то есть красный гейт реально валит проверку PR, а не
висит информационно. База берётся ИЗ СОБЫТИЯ (`push=before`, `PR=base.sha`) с fail-closed
на zero-SHA — тот же паттерн, что у `protected-artifacts` (блокер B1, 2026-07-11).
Оба self-test'а (`red_ct_rfc_atomic.sh`, `red_diff_contract_schema.sh`) прогоняются ТОЙ ЖЕ
проводкой — анти-плацебо самого барьера, `.claude/rules/testing.md`.

Замечание (MINOR, не блокер): `pip install --quiet jsonschema` — без пина версии; смена
поведения валидатора апстримом придёт в CI незамеченной. Внесено в TD-058 как пункт «б».

## Итог

**APPROVED** → merge в `main` (`--no-ff`) + `PROJECT-STATE.md`/`TECH-DEBT.md` + пост-мерж
деплой-гейт `gates.md` §8.
