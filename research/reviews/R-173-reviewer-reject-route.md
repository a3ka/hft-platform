<!-- GATE-META
milestone: PR-159
audited_repo: a3ka/hft-platform
audited_base: a390da71bf4693726db63b45c695783c885f2cd5
audited_head: 92aafd3c9174428de5b4aedfb9a0aac92d035ebd
verdict: REJECT
-->

# R-173 — перепроверка §9 правки «REJECT/CONCERNS → architect» (PR #159): **REJECT**

Перепроверка уставной правки по `.claude/rules/gates.md` §9 — независимый Fable-агент со
свежим контекстом, не автор. Предмет: ветка `docs/reviewer-reject-route`, вершина
`92aafd3c9174428de5b4aedfb9a0aac92d035ebd`, база `a390da71bf4693726db63b45c695783c885f2cd5`
(= `origin/main` на момент проверки; отставания ветки от `main` НЕТ — merge-base совпадает с
`origin/main`, `git log origin/main --not HEAD | wc -l → 0`). Дата проверки —
`2026-09-05T09:47Z` (`date -u`).

## ВЕРДИКТ КОРОТКО

**REJECT — по одной блокирующей находке (Б-1), исправление — одна строка либо одна явная
оговорка.** Сама замена в двух тронутых профилях верна, выправляет старое противоречие с
`gates.md` §4 и не задевает границу C. Полномочия (токен, зона, барьер замка) в порядке.
Барьеры документа зелены на ветке и на дереве слияния одинаково.

Блокирует другое: коммит утверждает **«Правится КЛАСС, а не названное место»** и предъявляет
ноль грепом `REJECT.*→ *dev`. Ноль истинный — я его воспроизвёл, — но он снят с
ФОРМУЛИРОВКИ, а не с класса. В той же запертой зоне живёт **третий носитель того же класса,
использующий другое слово** (`FAIL` вместо `REJECT`): `.claude/agents/tester.md:98`. Он не
исправлен и не исключён явно — просто не увиден. Это ровно случай Р-3
(`docs/workflow/oracle-blindness-class-2026-08-28.md` §5: «опасна ровно та группа, которая
НЕ ВЫПИСАНА») и ровно класс §5.5 передачи 2026-09-04, на который коммит сам ссылается как на
основание правки.

Условие APPROVED — в конце файла.

## Что я прочитал (ярус A/B), чем грепал (ярус C)

Целиком с диска: `CLAUDE.md`, `.claude/rules/{gates,commit-discipline,branch-hygiene,handoff-block,scope-guard,testing}.md`,
`.claude/agents/architect.md`, `docs/04-workflow.md` (246 строк), `docs/workflow/reading-map.md` §3,
`.claude/agents/reviewer.md` и `.claude/agents/risk-critic.md` **целиком на ветке**; версия
`origin/main` отличается от ветки ровно показанным ниже диффом (`git diff origin/main HEAD --stat`
→ 2 файла, +8/−2). Все девять профилей `.claude/agents/*.md` — грепом по §Handoff и по
маршрутизации отказа (вывод ниже). Первоисточник основания —
`docs/workflow/session-handover-2026-09-04.md` §4 п.2 (`:78-82`), §5 п.5 (`:112-114`), `:257-258`.
Правило Р-3 — `docs/workflow/oracle-blindness-class-2026-08-28.md` §5 (`:122`-…).

Грепом (ярус C): `docs/workflow/*.md`, `docs/DESIGN.md`, `docs/0N-*.md`, `milestones/BACKLOG.md`,
`docs/SESSION-HANDOFF.md` — по маршрутизации отказа к dev (синонимы: `self-fix`, `SVR`,
`возвращается dev`, `→ engine-dev`, `dev-агент, чей/который`, `FAIL|REJECT|CONCERNS … → dev`).

## Предмет — дифф `a390da7..92aafd3`

```
$ git show --stat --format='%H' HEAD
92aafd3c9174428de5b4aedfb9a0aac92d035ebd
 .claude/agents/reviewer.md    | 5 ++++-
 .claude/agents/risk-critic.md | 5 ++++-
 2 files changed, 8 insertions(+), 2 deletions(-)
```

`reviewer.md:46-49` (было `:46`): `REJECT/CHANGES REQUESTED → dev-агент, который делал impl
(SVR-response цикл, не self-fix у architect)` → `→ **architect**: разбор находки — его …
через founder'а диспетчеризует dev на impl-правку. Прямой возврат dev'у ОТМЕНЁН …`.

`risk-critic.md:59-62` (было `:59`): `CONCERNS/REJECT → dev-агент, чей код в PR
(engine-dev/venue-dev), не self-fix` → `→ **architect** … по той же причине, что в
.claude/agents/reviewer.md §Handoff`.

## Находки

### Б-1 (БЛОКЕР). Третий носитель класса не исправлен и не исключён: `.claude/agents/tester.md:98`

```
$ sed -n '95,99p' .claude/agents/tester.md
## Handoff
- PASS → `reviewer` (передаёт verdict + raw stdout всех гейтов).
- FAIL по спеке (тест написан неверно/двусмысленно) → `architect` (spec issue).
- FAIL по реализации → dev-агент, который писал impl (`engine-dev`/`venue-dev`/`signal-engineer`/`research-dev`).
- Формат — Handoff-блок с §C = сырой stdout (не пересказ), §D = следующий агент.
```

**Почему это тот же класс, а не соседний.** Класс определён самим автором в теле коммита:
«Профили ПРОВЕРЯЮЩИХ маршрутизировали отказ ПРЯМО dev-агенту». Tester — проверяющий:
`gates.md` §4 числит его гейтом с собственным артефактом (таблица «гейт | артефакт», строка
`tester | Done Block + ШАГ 0`), `tester.md:19` — «PASS/FAIL вердикт. Независимая … проверка
перед reviewer». Строка `:98` маршрутизирует отказ гейта прямо dev'у. Оба признака класса
налицо; отличается только слово (`FAIL`, не `REJECT`), и именно поэтому греп автора его не
видит:

```
$ grep -rnE 'REJECT.*→ *dev' .claude/ CLAUDE.md docs/04-workflow.md; echo "exit=$?"
exit=1                                    ← ноль воспроизведён; grep молчит с exit=1, «0» не печатает

$ grep -rnE 'dev-агент, кот|dev-агент, чей|который (делал|писал) impl' .claude/ CLAUDE.md docs/0[0-9]-*.md docs/DESIGN.md docs/workflow/harness-track.md; echo "exit=$?"
.claude/agents/tester.md:98:- FAIL по реализации → dev-агент, который писал impl (`engine-dev`/`venue-dev`/`signal-engineer`/`research-dev`).
exit=0
```

**Почему это блокер, а не примечание.** Коммит не просто меняет две строки — он ЗАЯВЛЯЕТ
исправление класса и предъявляет ноль как доказательство. Ноль снят с одной выписанной
формулировки; группа, живущая под другим словом в той же зоне, не выписана — и потому не
накрыта (Р-3). Приём §5.5 той же передачи, на которую коммит ссылается: «правка по вердикту
НАЧИНАЕТСЯ грепом КЛАССА и ЗАКАНЧИВАЕТСЯ предъявлением НУЛЯ» — здесь греп был по слову, а не
по классу. После merge'а зона окажется внутренне несогласной в день приземления: reviewer
REJECT → architect, risk-critic CONCERNS → architect, critic REJECT → architect
(`critic.md:48`, было и раньше), но tester FAIL → dev. Влить это как «класс правлен» —
поместить в историю `main` ложное утверждение о состоянии зоны.

**Что здесь НЕ моё решение — названо явно.** Есть инженерный довод, по которому tester FAIL
МОЖЕТ быть вне класса: при FAIL «по реализации» оракул УЖЕ существует и УЖЕ красный, то есть
«место» и «класс» совпадают — architect'у нечего дописывать, dev делает GREEN по имеющемуся
RED. Для reviewer/risk-critic это не так: их находка — дефект, который оракулы ПРОПУСТИЛИ, и
потому там нужен разбор класса и новый RED (`gates.md` §4: «Reviewer находит проблему →
architect проектирует защиту → dev реализует»). Но записанное указание founder'а звучит
«ВЕСЬ разбор — architect'у» (`session-handover-2026-09-04.md:80-81`), и охват этого «весь»
— его воля (`gates.md` §0.1), не моя и не автора. Молчание автора о `tester.md:98` — не
решение об охвате, а невидение носителя.

**Воспроизведение:** три команды выше на `92aafd3`.

### Б-2 (NOTE, не блокирует). Новый запрет в запертой зоне без механизма и без тега `COGNITIVE-ONLY`

Обе новые строки вводят запрет: «Прямой возврат dev'у ОТМЕНЁН». Правило «обязывающее живёт
вместе с механизмом» (`reviewer.md:90-98`, `architect.md` §«Обязывающее правило…», founder
2026-08-17) называет `.claude/agents/**` поимённо и требует либо механизм в той же цепочке,
либо тег `COGNITIVE-ONLY` + одну строку, почему механизировать нельзя. Ни того, ни другого
в §Handoff нет. Смягчение: прежняя строка была ровно так же не тегирована, регресса нет;
маршрут §D живёт в переписке, и барьер здесь возможен лишь частично (кандидат: R-файл с
`verdict: REJECT` обязан в своём Handoff §D называть `architect` — `check_gate_meta.sh` уже
разбирает R-файлы). Рекомендую закрыть тем же исправляющим коммитом одной строкой; отдельного
круга не стоит.

### Б-3 (NOTE, не блокирует). Ссылка на решение — дата без документа

`reviewer.md:46-47` и `risk-critic.md:60` ссылаются на «решение founder'а 2026-09-04», не
называя, где оно записано. Единственная письменная фиксация, которую я нашёл, —
`docs/workflow/session-handover-2026-09-04.md` §4 п.2 (`:78-82`), и это пересказ («весь
разбор — architect'у»), не текст решения. `grep -rl 'весь разбор' docs/ .claude/` даёт два
файла; второй (`docs/plans/depth-delivery-architecture-2026-08-31.md:89`) — другое значение
слова, не решение. Под правилом цитаты (`reading-map.md` §3) ссылка тонкая: следующий читатель
профиля не сможет открыть источник. Рекомендую добавить путь к передаче в скобки.

## Проверки (а)–(г) — команды и вывод

### (а) Утверждения о коде — на дереве слияния и на ветке, одинаково

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | tail -2; echo exit=${PIPESTATUS[0]}
PASS  [7-RFC-PATH] путей-кандидатов … всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_design_claims.sh 2>&1 | tail -2; echo exit=${PIPESTATUS[0]}
VERDICT: PASS (0 нарушений)
exit=0
```

Расхождения прогонов нет. Дерево слияния = ветка (ветка сидит на `origin/main`).

### (б) Полномочия — токен, зона, замок, граница C

```
$ git show HEAD --format='%B' -s | grep -nE '^FOUNDER-APPROVED: .{12,}'
3:FOUNDER-APPROVED: указание founder'а 2026-09-04 — весь разбор находки гейта ведёт
   (длина причины 63 байта ≥ 12; проверка ПОКОММИТНАЯ — коммит в диапазоне один)

$ git diff --name-only origin/main HEAD | grep -v '^\.claude/agents/'; echo exit=$?
exit=1                                    ← вне .claude/agents/** файлов нет

$ grep -n -B6 'check_docs_freeze' .github/workflows/ci.yml     # форма вызова взята из CI
 98-      - name: Барьер замка процессного слоя
 99-        env:
100-          EVENT_NAME: ${{ github.event_name }}
101-          PUSH_BEFORE: ${{ github.event.before }}
102-          PR_BASE_SHA: ${{ github.event.pull_request.base.sha }}
103:        run: bash scripts/check_docs_freeze.sh

$ EVENT_NAME=pull_request PR_BASE_SHA=a390da71bf4693726db63b45c695783c885f2cd5 bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0                                    ← барьер молчит при успехе

Негатив-контроль (барьер ПАДАЕТ против сломанного, testing.md §«Целостность гейта» п.3):
$ git checkout --detach -q HEAD && printf '\n<!-- negative-control -->\n' >> .claude/agents/tester.md \
  && git add .claude/agents/tester.md && git -c core.hooksPath=/dev/null commit -q -m 'test(negative-control): tokenless zone edit' -- .claude/agents/tester.md \
  && EVENT_NAME=pull_request PR_BASE_SHA=a390da71bf4693726db63b45c695783c885f2cd5 bash scripts/check_docs_freeze.sh; echo negative-control exit=$?
negative-control exit=1
$ git checkout -q docs/reviewer-reject-route && git status --porcelain | wc -l
0                                         ← дерево возвращено, контрольный коммит отброшен (detached)

$ EVENT_NAME=pull_request PR_BASE_SHA=a390da7… bash scripts/check_protected_artifacts.sh; echo exit=$?
OK: защищённые артефакты целы на HEAD (a390da7..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=a390da7… bash scripts/check_review_fa.sh; echo exit=$?
SKIP (диапазон не трогает crates/**)
exit=0                                    ← живой инвариант FA не требуется; проверено исполнением, не со слов мандата

$ gh pr view 159 --json state,headRefOid,mergeable
{"headRefOid":"92aafd3c9174428de5b4aedfb9a0aac92d035ebd","mergeable":"MERGEABLE","state":"OPEN"}
   statusCheckRollup: 18 чеков, все SUCCESS, включая «All checks passed» (09:36:00Z)
```

Граница C (`gates.md` §0.1): правка маршрутизирует вердикты гейтов между ролями; промоушен,
веса, состав данных, фазы, деньги не задеты. Токен — аудит-след, не подпись: проверены
НАЛИЧИЕ и ФОРМА, воля founder'а мною не удостоверяется.

### (в) Связность и висячие ссылки

- Новый маршрут **согласуется** с `gates.md` §4 `:211-213`: «Граница reviewer↔architect
  (TD-011): reviewer ОПИСЫВАЕТ дефект … Reviewer находит проблему → architect проектирует
  защиту → dev реализует». Прежняя строка reviewer.md («→ dev, не self-fix у architect»)
  этому ПРОТИВОРЕЧИЛА; правка снимает противоречие, существовавшее с 2026-07-11.
- Совместимость с «architect НИКОГДА не пишет impl-код» (`04-workflow.md` §1 `:19`,
  `architect.md` §NEVER): текст явно оставляет impl за dev — «через founder'а
  диспетчеризует dev на impl-правку». Совместимо с `CLAUDE.md` «founder = диспетчер».
- `risk-critic.md:62` ссылается на `.claude/agents/reviewer.md` §Handoff — файл и секция
  существуют (`reviewer.md:44`). Висячих ссылок в дифе нет.
- `critic.md:48` уже маршрутизирует REJECT → architect; `critic.md:56` «НЕ предлагает
  architect self-fix loop на NOTE/ESCALATE» — про НЕ-отказные вердикты, к классу не относится.
- `docs/04-workflow.md` §1/§2 описывают только прямой ход цепочки (dev → tester → reviewer →
  merge), маршрута отказа там нет — противоречия не возникает. `handoff-block.md` — форма,
  маршрута не задаёт.
- Единственная внутренняя несогласность зоны после правки — `tester.md:98` (Б-1).

### (г) Правился ли класс — см. Б-1

Сводка по всем девяти профилям `.claude/agents/*.md` (`ls` → architect critic engine-dev
research-dev reviewer risk-critic signal-engineer tester venue-dev), греп §Handoff по
маршрутизации отказа: носителей «отказ гейта → dev» ПОСЛЕ правки — **один**, `tester.md:98`.
До правки было три (reviewer, risk-critic, tester), не два, как утверждает тело коммита.
Вне `.claude/agents/**` в нормативных документах (`docs/DESIGN.md`, `docs/0N-*.md`,
`.claude/rules/*.md`, `CLAUDE.md`, `milestones/BACKLOG.md`, `harness-track.md`,
`binding-requires-mechanism.md`, `reading-map.md`) носителей класса грепом по синонимам не
найдено (`exit=1`). Упоминания в `docs/workflow/session-handover-*.md` — история передач,
не норма.

## Условие APPROVED

Один дополнительный коммит на ту же ветку, с токеном `FOUNDER-APPROVED` (зона заперта),
делающий ОДНО из двух — выбор принадлежит founder'у, потому что это охват его указания
(`gates.md` §0.1), а не инженерная правота:

1. **Включить tester в класс:** `tester.md:98` → `FAIL по реализации → architect` (разбор
   его; impl — dev через founder'а), той же формулировкой и с той же причиной, что в двух
   уже правленных профилях. Тогда «класс правлен» становится истиной, а ноль предъявляется
   грепом по КЛАССУ (например `grep -rnE '(FAIL|REJECT|CONCERNS)[^\n]{0,40}→ *(`)?(dev|engine-dev|venue-dev)' .claude/`),
   а не по одному слову.
2. **Явно исключить:** одна строка в `tester.md` §Handoff, называющая, почему FAIL «по
   реализации» вне класса (оракул уже существует и красен — «место» и есть «класс»), с
   пометкой, что исключение — по слову founder'а. Молчаливое невидение носителя таким
   исключением не является.

Желательно (не условие): тем же коммитом закрыть Б-2 (тег `COGNITIVE-ONLY` + одна строка,
либо названный кандидат-механизм) и Б-3 (путь к записи решения в скобках). Мержить PR #159
я не вправе и не мержу; после исправления — повторная перепроверка §9 ТОЛЬКО диапазона
исправления (второй круг), не всей правки заново.

## Пределы этой проверки

- Я проверил НАЛИЧИЕ и ФОРМУ токена, не волю founder'а: токен подделываем, это принятое
  ограничение (`gates.md` §11).
- Указание founder'а от 2026-09-04 я читал в ПЕРЕСКАЗЕ передачи, первичной записи решения
  не нашёл (Б-3); суждение об охвате «весь» поэтому оставлено founder'у, а не вынесено мной.
- `scripts/reserve_artifact_id.sh R` дважды не завершился за 90–100 с (`exit=124`); номер
  `R-173` взят у `scripts/next_artifact_id.sh R` (первый прогон), свободен на всех ref'ах:
  `git log --all --diff-filter=A --name-only -- 'research/reviews/R-173*'` → пусто;
  `R-170..172` существуют на ветках. Гонку с параллельной сессией ловит барьер
  `check_artifact_ids.sh` постфактум — резерв необязателен (`gates.md` §12).
- Шаблон шапки из мандата (`milestone: N/A — уставная правка процессного слоя`) барьер
  `check_gate_meta.sh:440-444` ОТВЕРГАЕТ: поле обязано иметь форму `КЛАСС-НОМЕР[буква]`
  (`FAIL … не похож на идентификатор артефакта`, exit=1 на первом прогоне). Взята форма
  прецедента процессных вердиктов `milestone: PR-NN` (`R-*` с `PR-56` ×5, `PR-59` ×3) →
  `PR-159`; повторный прогон — `VERDICT: PASS — вердиктов проверено: 1`, exit=0. Мандату
  следующей перепроверки §9 стоит нести эту форму, а не `N/A`.
