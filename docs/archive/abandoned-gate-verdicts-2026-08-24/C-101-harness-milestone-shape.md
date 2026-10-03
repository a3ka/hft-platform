<!-- GATE-META
milestone: C-099
audited_repo: a3ka/hft-platform
audited_base: 10bc072c7e008bce3feee80013fb187e3436fd17
audited_head: a9b20800912410dec0f91151e01704ea65fd9d59
verdict: REJECT
-->

# C-100 — REJECT: adversary audit `milestone-shape`

> *Переименован из `C-100` в `C-101` 2026-08-18 architect'ом (арбитраж `A-010` §E).
> Правка ФОРМЫ, не содержания: два вердикта заняли номер `C-100` одновременно —
> `18b3655` (M-69) в 09:21:52 против `ec2e5e9` (харнесс) в 09:23:28, барьер
> `check_artifact_ids.sh` валит обе ветки, каждая обвиняя другую. Решение арбитра —
> переименовывается пришедший вторым; номер взят РЕЗЕРВОМ (`reserve_artifact_id.sh C`),
> а не аллокатором, потому что незарезервированный номер эту коллизию и породил.
> **Содержание вердикта не изменено ни на символ**; шапка `GATE-META` не тронута.*


## Предмет и граница

Аудирован закоммиченный harness-набор `10bc072..a9b2080`: барьер
`scripts/check_milestone_shape.sh`, его проба
`scripts/tests/red_milestone_shape.sh` и CI-проводка. `milestone: C-099` в
GATE-META — первичный critic-артефакт, на котором основан механизм; у
harness-трека отдельного M-файла намеренно нет (`docs/workflow/harness-track.md` §3).

Набор остаётся в границе harness-трека: изменены только `scripts/check_*.sh`,
`scripts/tests/red_*.sh` и `.github/workflows/ci.yml`; T-контракты и продуктовый
milestone-файл здесь не требуются.

## Вердикт: REJECT

### B-1 — кодовый блок выдаётся за обязательный раздел

`scripts/check_milestone_shape.sh:68-81` подаёт весь Markdown в `grep` и не
различает заголовок документа и строку в fenced-code/comment. Для новой спеки с
отсутствующим разделом `Allowed paths`, но с текстом `## Allowed paths` внутри
```markdown`-блока, барьер печатает `OK` и завершает работу с `exit=0`.

Это ложное зелёное ровно на обещанном инварианте формы: у dev нет реального
раздела границ, хотя барьер утверждает обратное. Авторская проба не содержит
такой фикстуры и потому не ловит уже существующий дефект реализации.

### B-2 — проба принимает substring-стаб

Проба в `scripts/tests/red_milestone_shape.sh:101-158` проверяет только удаление
строк точных заголовков. Адверсарный стаб с тем же fail-closed setup и тем же
поиском новых файлов, но с `grep -qi -- <имя раздела>` вместо проверки заголовка,
прошёл её целиком: `PASS=14 FAIL=0`, `exit=0`.

Такой стаб принимает, например, `Acceptance is described in prose` как
«Acceptance», а значит анти-плацебо требование harness-track §3/§5 не выполнено.
Факт, что текущий барьер якорит начало строки, не спасает пробу: она обязана
краснеть против данного ослабления, а не делает этого.

### B-3 — неполная спека обходит барьер через rename

`scripts/check_milestone_shape.sh:53-55` выбирает только статус `A`.
В свежем git-репозитории rename неполной `milestones/M-98-old.md` в
`milestones/M-99-renamed.md` показан Git как `R100`; барьер сообщает «новых
milestone-спек нет» и возвращает `0`. Новое имя M-99 тем самым получает форму,
которую barьер не проверял. В пробе есть лишь сценарий `modify`, но нет rename.

### B-4 — заявленный отрицательный замер не воспроизводится как проверка формы

Заявленное `e555cb4 → exit=1` / `10753df → exit=0` не предъявляет нужный
сценарий. На аудируемом `a9b2080` оба SHA не являются предками и барьер
fail-closed; на историческом `10753df` диапазон `e555cb4..HEAD` не содержит
добавленной milestone-спеки и даёт `exit=0`. Следовательно, `exit=1` не доказан
как отказ на неполной спеке, что нарушает setup-guard из `testing.md`
«Целостность гейта», свойство 3.

## Подтверждённые свойства / не блокируют сами по себе

- Замер корпуса воспроизводится: 53 `milestones/M-*.md`, из них 36 без
  `Allowed paths`.
- Не-ASCII имя корректно проходит для полной спеки и краснеет для неполной.
- Добавленная и затем удалённая в том же диапазоне спека ожидаемо не остаётся
  объектом проверки (`exit=0`); это не самостоятельная находка.
- CI-проводка корректна: `milestone-shape` есть и в `status-check.needs`, и в
  fail-closed условии. При `needs.milestone-shape.result=failure` агрегат
  завершается с `exit=1`.

## Условие повторного аудита

Architect предъявляет дополненные RED-сценарии, которые сами краснеют против:

1. псевдозаголовка в Markdown comment/fenced-code;
2. каждого обязательного имени, присутствующего только как подстрока в прозе;
3. `R100` rename неполной спеки.

Также нужен воспроизводимый отрицательный замер с реально добавленной неполной
спекой, а не с non-ancestor или пустым по `A` диапазоном. После этого повторить
мутационный контроль пробы и harness-набор.

## Done Block

```text
$ bash scripts/tests/red_milestone_shape.sh; echo "exit=$?"
PASS=14 FAIL=0 (сценариев: 14)
VERDICT: PASS
уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: 0
exit=0

$ comment/code pseudo-heading fixture; EVENT_NAME=pull_request PR_BASE_SHA=<base> bash scripts/check_milestone_shape.sh; echo "exit=$?"
=== проверяю форму: milestones/M-99-comment.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
exit=0
# expected: exit=1; «## Allowed paths» находился только внутри fenced-code.

$ prose-only Allowed-paths fixture; EVENT_NAME=pull_request PR_BASE_SHA=<base> bash scripts/check_milestone_shape.sh; echo "exit=$?"
FAIL  milestones/M-99-prose.md: отсутствует обязательный раздел «Allowed paths»
exit=1

$ BARRIER_OVERRIDE=/tmp/mshape_substring_stub.sh bash scripts/tests/red_milestone_shape.sh; echo "exit=$?"
PASS=14 FAIL=0 (сценариев: 14)
VERDICT: PASS
уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: 0
exit=0

$ git diff --name-status <rename-base> HEAD
R100	milestones/M-98-old.md	milestones/M-99-renamed.md
$ EVENT_NAME=pull_request PR_BASE_SHA=<rename-base> bash scripts/check_milestone_shape.sh; echo "exit=$?"
OK: в диапазоне <rename-base>..HEAD новых milestone-спек нет — проверять нечего
exit=0

$ non-ASCII complete fixture -> exit=0; non-ASCII incomplete fixture -> exit=1
=== проверяю форму: milestones/M-99-кириллица.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
exit=0
FAIL  milestones/M-99-кириллица.md: отсутствует обязательный раздел «Allowed paths»
exit=1

$ git ls-tree -r --name-only HEAD milestones | rg '^milestones/M-.*\.md$' | wc -l
53
$ anchored Allowed-paths count
head_without_allowed_paths=36

$ bash /tmp/hft-critic-harness-doc-integrity/scripts/check_milestone_shape.sh e555cb4  # cwd=a9b2080
FAIL  база 'e555cb4' НЕ предок HEAD — история переписана; что введено, недоказуемо
exit=1
$ bash /tmp/hft-critic-harness-doc-integrity/scripts/check_milestone_shape.sh e555cb4  # cwd=10753df
OK: в диапазоне e555cb4..HEAD новых milestone-спек нет — проверять нечего
exit=0

$ wiring check: milestone-shape in needs and in if
wiring_check: needs=present if=present
$ aggregate model with milestone-shape=failure
One or more checks failed
aggregate_model milestone-shape=failure exit=1

$ bash -n scripts/check_milestone_shape.sh; echo "exit=$?"
exit=0
$ bash -n scripts/tests/red_milestone_shape.sh; echo "exit=$?"
exit=0
$ git diff --check 10bc072 a9b2080; echo "exit=$?"
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-18T09:23Z
- Milestone: C-099 (harness-track subject `harness-doc-integrity`)
- Статус: BLOCKED — REJECT
- HEAD: a9b2080 — feat(harness): проводка milestone-shape в CI

## §B — Что я сделал
- Исполнил adversary-пробы barьера, его RED-набора и CI-агрегата.
- Подтвердил B-1…B-4 с raw output в Done Block.

## §C — Артефакты / результаты
- `research/critiques/C-100-harness-milestone-shape.md`
- Done Block: см. выше.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь REJECT C-100 на ветке feat/harness-doc-integrity. Не меняй продуктовые
  пути. Добавь RED-сценарии для fenced/comment pseudo-heading, prose-only всех
  обязательных имён и R100 rename неполной спеки; предъяви отдельный semantic
  negative fixture вместо non-ancestor/empty-A замера. Затем запусти пробу против
  честной реализации и против ослабленных стабов, закоммить и запушь набор для
  нового adversary audit.
  ```
- Push-статус: verdict commit будет отправлен на `origin/feat/harness-doc-integrity`.
- Кэш: не создавался.

## §E — Риски / открытые вопросы
- Dev/merge блокируются до устранения B-1…B-4 и нового adversary verdict.

=== END HANDOFF ===
