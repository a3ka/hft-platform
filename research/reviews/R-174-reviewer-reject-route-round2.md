<!-- GATE-META
milestone: PR-159
audited_repo: a3ka/hft-platform
audited_base: cf3b22cca44b62c7540c30bdde10583da333294f
audited_head: 0870a0d8fc15621c6f1fbb9e41e52491b7d06efe
verdict: APPROVE
-->

# R-174 — перепроверка §9, круг 2: исполнение условия `R-173` (PR #159): **APPROVED**

Независимый Fable-агент со свежим контекстом, не автор правки и не автор круга 1.
Предмет — ТОЛЬКО диапазон исправления `cf3b22c..0870a0d` (один коммит), как предписал
`R-173` §«Условие APPROVED». Решённое кругом 1 (замена «REJECT → architect» в `reviewer.md`
и `risk-critic.md`, полномочия, барьеры) не переоткрывалось: нового факта против него не
найдено. Дата — `2026-09-05T12:24Z…12:33Z` (`date -u`, две отметки ниже). Ветка сидит ровно на
`origin/main`: `git merge-base HEAD origin/main` = `a390da71…` = `origin/main`; дерево слияния
и ветка совпадают.

## ВЕРДИКТ КОРОТКО

**APPROVED.** Условие круга 1 исполнено по существу: founder выбрал вариант 1 («включить
tester в класс»), `tester.md:98` переведён на `architect`, две соседние строки §Handoff
различимы по ПОСТ-ДЕЙСТВИЮ architect'а, а не только по слову. Четвёртый носитель класса,
`risk-critic.md:58` (CONCERNS по backtest-отчёту → `signal-engineer`/`backtest-runner`), НЕ
переведён и оформлен явным исключением с причиной — и причина **истинна по первоисточникам**:
`docs/02-quant-desk.md` §1 прямо говорит «Fable не участвует в рутине цикла вообще», а
`docs/DESIGN.md` §7 перечисляет состав квант-деска без architect'а. Пятого носителя в зоне
грепом класса (шесть форм, одна из них многострочная) — нет. Б-2/Б-3 закрыты. Барьеры зелены
на явной базе в CI-форме, `verify_design_claims` — на ветке и на дереве слияния одинаково.

Пять примечаний (N-1…N-5) — не блокируют merge, но два из них (N-1, N-2) требуют ОДНОЙ
строки в СЛЕДУЮЩЕМ процессном коммите, и почему их нельзя было закрыть в этом — названо.
Одно решение остаётся founder'у и названо явно (§«Что не моё решение»).

## Что я прочитал (целиком), чем грепал

Целиком с диска: `CLAUDE.md`, `.claude/rules/{gates,commit-discipline,branch-hygiene,handoff-block,scope-guard,testing}.md`,
`.claude/agents/architect.md`, `research/reviews/R-173-reviewer-reject-route.md` (весь),
`.claude/agents/{reviewer,risk-critic,tester}.md` на `0870a0d` (весь текст, `cat -n`),
`docs/DESIGN.md` §6 (`:201-225`) и §7 (`:227-240`), `docs/02-quant-desk.md` §1 (`:11-26`),
§3 (`:42-67`), §4 (`:68-81`), `docs/workflow/session-handover-2026-09-04.md` §4 (`:74-88`),
`docs/workflow/oracle-blindness-class-2026-08-28.md` §5 Р-3 (`:59-89`),
`docs/workflow/reading-map.md` §3, `docs/04-workflow.md` §1-§2 (`:19-52`, `:159`),
`.claude/agents/critic.md` §Handoff (`:47-56`), `.github/workflows/ci.yml` (формы вызова барьеров),
`scripts/check_gate_meta.sh` (что он разбирает: `:389-390`, `:567-572`).

Грепом (ярус C): вся зона `.claude/agents/*.md` (9 профилей), `.claude/rules/*.md`,
`CLAUDE.md`, `docs/04-workflow.md` — по КЛАССУ «отказ проверяющего → dev-роль», шесть форм
(вывод в §(2) ниже).

## Предмет — диапазон `cf3b22c..0870a0d`

```
$ git show 0870a0d --stat --format=''
 .claude/agents/reviewer.md    |  8 +++++++-
 .claude/agents/risk-critic.md | 15 ++++++++++++++-
 .claude/agents/tester.md      | 11 ++++++++++-
 3 files changed, 31 insertions(+), 3 deletions(-)
$ git diff --name-only cf3b22c HEAD | grep -v '^\.claude/agents/'; echo "outside-zone grep exit=$?"
outside-zone grep exit=1                       ← вне .claude/agents/** файлов нет
```

## (1) Б-1 — закрыт по существу

`tester.md:97-102` на `0870a0d`:

```
97: - FAIL по спеке (тест написан неверно/двусмысленно) → `architect` (spec issue).
98: - FAIL по реализации → **`architect`** (решение founder'а 2026-09-05: разбор ЛЮБОГО отказа
99:   гейта ведёт architect, исключений нет). Он называет, что чинить, и через founder'а
100:  диспетчеризует dev (`engine-dev`/`venue-dev`/`signal-engineer`/`research-dev`) на
101:  impl-правку. Прямой возврат dev'у ОТМЕНЁН тем же решением, что в
102:  `.claude/agents/reviewer.md` и `.claude/agents/risk-critic.md` §Handoff.
```

**Не схлопываются.** Обе строки называют одного адресата, но различаются тем, что architect
ДЕЛАЕТ дальше: `:97` — «spec issue», он правит спеку/оракул сам (его зона `*/tests/`,
`milestones/`); `:98-101` — «называет, что чинить, и через founder'а диспетчеризует dev на
impl-правку». Классификация tester'ом («по спеке» / «по реализации») остаётся содержательной
информацией Handoff'а §D, а не выбором адресата. Согласуется с `gates.md:211-213` («Reviewer
находит проблему → architect проектирует защиту → dev реализует») и с `04-workflow.md:38-49`
(схема цепочки маршрута отказа не содержит — противоречия нет). Совместимо с «architect
НИКОГДА не пишет impl-код»: impl оставлен dev'у явно (`:100-101`).

Тонкость `:99` «исключений нет» — см. N-1.

## (2) Четвёртый носитель и форма исключения — главный предмет круга

### Истинна ли причина — по первоисточникам, открытым мною

`risk-critic.md:59-65` утверждает: отказ по backtest-отчёту — исследовательский цикл
Границы A, у architect'а там нет зоны, маршрут через него сделал бы его звеном, где он
ничего не решает.

- `docs/DESIGN.md:229-233` (§7): роли квант-деска — `hypothesis-researcher`, `signal-engineer`,
  `backtest-runner`, `risk-critic`, `portfolio-analyst`, **founder — единственная подпись**.
  Architect'а в составе деска НЕТ.
- `docs/02-quant-desk.md:23-26` (§1), дословно: **«Экономия: Fable не участвует в рутине цикла
  вообще; дорогая модель только у risk-critic»**. Это не «нет зоны», это прямое проектное
  решение — Fable вне петли деска по цене.
- `docs/02-quant-desk.md:42-66` (§3): цикл гипотезы — п.5 «Критика. risk-critic … KILL /
  CONCERNS / PASS» → п.6 «Решение founder'а №1». Следующий шаг после CONCERNS в цикле —
  доработка исполнителем п.2-4 (signal-engineer / backtest-runner), architect'а в цикле нет.
- `docs/DESIGN.md:206-208` (§6, Граница A): «агенты пишут только `crates/signals/**` +
  `research/**`».

**Вывод: причина истинна, и первоисточник даёт довод СИЛЬНЕЕ, чем записал автор** (см. N-4).

### Тот же класс или другой

Класс, ради которого принято решение 04.09 (`session-handover-2026-09-04.md:78-82`,
`reviewer.md:49-50`): dev чинил названное МЕСТО, а не КЛАСС, потому что находка гейта —
дефект, который ОРАКУЛЫ ПРОПУСТИЛИ, и нужен разбор класса + новый RED — работа architect'а.
У backtest-CONCERNS другой предмет и другой контур защиты от «починки места»: гипотеза с
пре-регистрацией, глобальный trials-ledger (каждая попытка +1 → deflated Sharpe падает),
KILL навсегда в карточке (`02-quant-desk.md:70-81` §4, `DESIGN.md:235-240`). Здесь «класс»
пиннится ledger'ом и критиком, а не RED-оракулом architect'а. **Другой класс** — суждение
инженерное; охват решения founder'а — его воля (`gates.md` §0.1), см. ниже.

### Достаточна ли форма исключения

Круг 1 требовал: «Молчаливое невидение исключением не является». Стало НЕ молчаливым:
`risk-critic.md:59` — заголовок жирным «ЯВНОЕ ИСКЛЮЧЕНИЕ … (записано, а не умолчано)»,
`:60-64` — причина с ссылками на первоисточники, `:65` — как найдено (грепом класса при
исполнении `R-173` Б-1) и статус («вынесено founder'у на подтверждение»). Тело коммита
`0870a0d` повторяет то же. Форма достаточна. Замечание к её будущему состоянию — N-2.

### Пятого носителя нет — греп по классу, шесть форм

Зона `Z` = `.claude/agents/*.md` (9 файлов: architect critic engine-dev research-dev
reviewer risk-critic signal-engineer tester venue-dev — `ls`), `.claude/rules/*.md`,
`CLAUDE.md`, `docs/04-workflow.md`. Греп — GNU grep 3.11 (`/bin/grep`; под именем `grep`
в среде стоит ugrep, он упёрся в лимит сложности регэкспа — `exit=2`, показано ниже, чтобы
ноль не был принят за «не запускали»).

```
$ /bin/grep -nE '(FAIL|REJECT|CONCERNS|KILL|CHANGES REQUESTED).{0,80}→.{0,20}(dev|engine-dev|venue-dev|signal-engineer|research-dev|backtest-runner)' $Z
.claude/agents/risk-critic.md:58:- Backtest-отчёт: … CONCERNS → `signal-engineer`/`backtest-runner` для доработки. …
exit=0                                          ← единственное совпадение — явно исключённая строка

$ /bin/grep -nE '→ *\*{0,2}`?(dev|engine-dev|venue-dev|signal-engineer|research-dev|backtest-runner)' $Z
.claude/agents/risk-critic.md:58:…             ← та же строка
.claude/rules/gates.md:213:находит проблему → architect проектирует защиту → dev реализует.
docs/04-workflow.md:159:**Plan-time (critic) — триггеры (иначе architect→dev напрямую):** …
exit=0
   gates.md:213 — dev получает работу ОТ architect'а (прямой ход нового маршрута), не отказ.
   04-workflow.md:159 — диспетч плана architect→dev, не отказ гейта. Оба — не класс.

$ /bin/grep -nEi '(возвращ|отда[её]т|верн[иу]).{0,40}(dev|engine-dev|venue-dev|signal-engineer|research-dev)' $Z; echo exit=$?
exit=1
$ for f in $Z; do perl -0777 -ne 'while(/((?:возвращ|отда[её]т|верн[иу])[\s\S]{0,60}?(?:engine-dev|venue-dev|signal-engineer|research-dev|dev\x27|dev-агент))/g){…print}' "$f"; done
.claude/agents/tester.md: верни dev'          ← многострочно: tester.md:35-36 «СТОП, верни / dev'у запушить»
   Это преднамеренный СТОП по TD-036/RN-18 (SHA не на origin — нечего проверять), а не
   вердикт FAIL по предмету: класс «отказ гейта по находке» не задет. Выписано, не умолчано (Р-3).

$ /bin/grep -n 'self-fix' $Z
.claude/agents/critic.md:56:… НЕ предлагает architect self-fix loop на NOTE/ESCALATE.
exit=0                                          ← про НЕ-отказные вердикты; круг 1 уже разобрал
$ /bin/grep -nE 'SVR' $Z; echo exit=$?
exit=1
$ /bin/grep -rnE 'dev-агент, кот|dev-агент, чей|который (делал|писал) impl' $Z; echo exit=$?
exit=1                                          ← носитель круга 1 (tester.md:98) исчез
$ /bin/grep -nE 'для доработки' $Z
.claude/agents/risk-critic.md:58:…             ← та же исключённая строка
```

Сводка по девяти профилям после `0870a0d`: маршрутов «отказ проверяющего → dev-роль» — **один**,
`risk-critic.md:58`, и он исключён ЯВНО. Утверждение коммита «единственное оставшееся
совпадение — явно исключённая строка» воспроизведено.

## (3) Б-2 — закрыт; кандидат назван честно; утверждение о `check_gate_meta.sh` истинно

Во всех трёх профилях (`reviewer.md:51-55`, `risk-critic.md:71-75`, `tester.md:103-107`):
тег `COGNITIVE-ONLY` со ссылкой на `gates.md` §11, строка «почему механизма нет» («Маршрут §D
живёт в переписке, а CI видит только артефакты» — истинно), кандидат с оговоркой «НАЗВАН и не
выдаётся за сделанный». Утверждение «`scripts/check_gate_meta.sh` эти файлы уже разбирает»:

```
$ grep -nE 'research/(reviews|critiques|arbitration)|Handoff|§D' scripts/check_gate_meta.sh
389:verdict_changes="$(git diff --name-status --diff-filter=AM "${BASE}" HEAD -- \
390:  research/critiques research/reviews research/arbitration 2>/dev/null \
567:  done < <(git ls-tree -r --name-only "${c}" -- research/reviews 2>/dev/null \
```

Барьер разбирает файлы `research/{critiques,reviews,arbitration}` — утверждение ИСТИННО.
Секций `Handoff`/`§D` он не разбирает (совпадений нет) — то есть кандидат действительно
только кандидат, и автор это сказал. Точность формулировки кандидата — N-3.

## (4) Б-3 — закрыт

`reviewer.md:47`, `risk-critic.md:68`: путь `docs/workflow/session-handover-2026-09-04.md` §4 п.2.

```
$ awk '/^## §?4/,/^## §?5/' docs/workflow/session-handover-2026-09-04.md | sed -n '6,9p'
2. **Процессная правка в запертой зоне.** `.claude/agents/reviewer.md` §Handoff говорит
   «REJECT → dev-агент», что расходится с указанием founder'а от 04.09 («весь разбор —
   architect'у»). Нужен токен `FOUNDER-APPROVED` (`gates.md` §11). …
```

Существует, содержит то, на что ссылается (пересказ указания — как и констатировал круг 1;
первичной записи, отличной от пересказа, в репозитории нет, и профиль честно ссылается на
то, что есть).

## (5) Полномочия диапазона — покоммитно, в CI-форме

```
$ for c in $(git rev-list cf3b22c..HEAD); do git show -s --format='%B' $c | grep -nE '^FOUNDER-APPROVED: .{12,}'; done
3:FOUNDER-APPROVED: решение founder'а 2026-09-05 — разбор ЛЮБОГО отказа гейта ведёт
   (коммит в диапазоне один; причина 102 байта ≥ 12)

$ grep -n -B6 'run: bash scripts/check_docs_freeze.sh' .github/workflows/ci.yml | grep -E 'EVENT_NAME|PUSH_BEFORE|PR_BASE_SHA|run:'
100:          EVENT_NAME: ${{ github.event_name }}
101:          PUSH_BEFORE: ${{ github.event.before }}
102:          PR_BASE_SHA: ${{ github.event.pull_request.base.sha }}
103:        run: bash scripts/check_docs_freeze.sh

$ BASE=$(git rev-parse origin/main)   # = a390da71bf4693726db63b45c695783c885f2cd5 = merge-base = база PR #159 (baseRefName main)
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0                                          ← барьер молчит при успехе
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_protected_artifacts.sh; echo exit=$?
OK: защищённые артефакты целы на HEAD (a390da7..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_review_fa.sh; echo exit=$?
SKIP (диапазон не трогает crates/**)
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_gate_meta.sh 2>&1 | tail -1
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_artifact_ids.sh 2>&1 | tail -1
OK: ни один коммит диапазона a390da7..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```

Контроль fail-closed самих барьеров (попутно): при пустой `PR_BASE_SHA` все пять отказали
(`FAIL база события пуста`, exit 1/2) — барьеры не проходят молча на неустановленной базе.

Граница C (`gates.md` §0.1): диапазон маршрутизирует вердикты между ролями; промоушен,
веса, состав данных, фазы, деньги не задеты. Токен — аудит-след, не подпись: проверены
НАЛИЧИЕ и ФОРМА, воля founder'а мною не удостоверяется.

```
$ gh pr view 159 --json state,headRefOid,mergeable,baseRefName
OPEN head=0870a0d8fc15621c6f1fbb9e41e52491b7d06efe mergeable=MERGEABLE base=main
```

## (6) Утверждения о коде — ветка и дерево слияния

```
$ bash scripts/verify_design_claims.sh 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
VERDICT: PASS (0 нарушений)
exit=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
VERDICT: PASS (0 нарушений)
exit=0
```

Расхождения нет (дерево слияния = ветка).

## Примечания — не блокируют, но названы с местом и правкой

### N-1. `tester.md:99` «исключений нет» противоречит `risk-critic.md:59` буквально

`gates.md` §6 называется «Анти-оверфит гейт», и risk-critic на backtest-отчёте — гейт с
артефактом `C-NNN`. Значит «разбор ЛЮБОГО отказа гейта … исключений нет» (`tester.md:98-99`,
та же фраза — в токене `FOUNDER-APPROVED`) буквально ложно ровно на одно исключение, которое
тот же коммит записывает в `risk-critic.md:59`. Почему не блокер: маршрутные строки всех
четырёх носителей однозначны и не противоречат друг другу; расхождение — в ОБОСНОВАНИИ
одной из них; оба текста явны и взаимно видны; тело коммита раскрывает оба. Почему нельзя
было закрыть в этом коммите: окончательная формулировка зависит от того, подтвердит ли
founder исключение, — этого решения ещё нет. Правка (одна строка, следующим процессным
коммитом, после решения founder'а): `tester.md:99` → «…ведёт architect; единственное
исключение — backtest-отчёт risk-critic'а, `risk-critic.md` §Handoff» ЛИБО, при отказе от
исключения, `risk-critic.md:58` → `architect`.

### N-2. `risk-critic.md:65` «вынесено founder'у на подтверждение» устареет в день merge'а

Если founder подтверждает исключение merge'ем, строка остаётся в `main` как вечно
«ожидающая». Правка тем же следующим коммитом: «подтверждено founder'ом <дата>» либо снять
строку и исключение целиком (см. N-1).

### N-3. Кандидат-механизм скопирован дословно в три профиля, но точен только для reviewer'а

`verdict: REJECT обязан в §D называть architect` — у reviewer'а отказ = `REJECT`, верно.
У risk-critic'а отказ на safety-пути = `CONCERNS` (`risk-critic.md:66`; `KILL` терминален) —
кандидат, как записан, его не накрыл бы. У tester'а вердикт-файла НЕТ вовсе
(`tester.md:24` «Ничего в репозитории — READ-ONLY», `disallowedTools: Write, Edit`; вердикт —
`FAIL`, не `REJECT`), то есть кандидат к tester'у неприменим по конструкции, и честная строка
«почему механизма нет» там сильнее: артефакта, который CI мог бы прочитать, у роли не
существует. Утверждение о `check_gate_meta.sh` при этом истинно (§(3)). Правка: в
`risk-critic.md` — `verdict: REJECT|CONCERNS`; в `tester.md` — заменить кандидат на
«у роли нет артефакта в репозитории — кандидата нет, остаток `COGNITIVE-ONLY` целиком».

### N-4. Цитата `scope-guard.md` в исключении неточна; сильный первоисточник не назван

`risk-critic.md:63`: «зоны в которых у architect'а НЕТ (`scope-guard.md`: `crates/signals/**`
+ `research/**` — квант-деск)». `scope-guard.md` говорит, что квант-агенты пишут ТОЛЬКО туда;
он НЕ говорит, что architect'а там нет: `*/tests/` (везде) и `scripts/verify_*` — architect'а,
а `research/{critiques,reviews,arbitration}` пишут гейты. Истинно про ПРЕДМЕТ отказа (карточка
гипотезы, отчёт, грид, код сигнала), не про пути буквально. Точный и более сильный источник —
`docs/02-quant-desk.md` §1 `:24-25`: «Fable не участвует в рутине цикла вообще», плюс
`DESIGN.md` §7 `:229-233` (состав деска). Правка — заменить ссылку.

### N-5. Непарный бэктик — две строки

```
$ /bin/grep -n 'п\.2`' .claude/agents/*.md
.claude/agents/reviewer.md:47:  … §4 п.2`; охват
.claude/agents/risk-critic.md:68:  … §4 п.2`); impl-правку он
```

Открывает code-span до следующего бэктика; в raw-тексте, который читают агенты, безвреден;
в рендере ломает разметку строки. Орфографический класс — правится тем же коммитом, что N-1.

## Что здесь НЕ моё решение — названо явно

Подтверждение исключения для `risk-critic.md:58` — охват указания founder'а, граница §0.1.
Я установил: причина истинна по первоисточникам, класс инженерно другой, форма явная. Волю
founder'а я не удостоверяю. Если founder ИСКЛЮЧЕНИЕ ОТКЛОНЯЕТ, PR #159 в текущем виде влить
нельзя: сначала `risk-critic.md:58` → `architect` тем же порядком (токен, зона), и это
пересмотрит проектную оговорку `02-quant-desk.md` §1 «Fable не участвует в рутине цикла»
(правка формы `docs/0N-*` → критик по `gates.md` §9). Если ПОДТВЕРЖДАЕТ — APPROVED действует
как есть, а N-1/N-2 закрываются одной строкой в следующем процессном коммите.

## Пределы этой проверки

- Скоуп — диапазон исправления, не вся правка: так предписал круг 1, и нового факта против
  его решений я не нашёл. Цену маршрута «каждый FAIL tester'а → Fable» круг 1 уже назвал,
  founder выбрал охват — не переоткрываю.
- Токен проверен на НАЛИЧИЕ и ФОРМУ; истинность причины — когнитивно (`gates.md` §11).
- `scripts/next_artifact_id.sh R` не завершился за 60 с (`exit=124`; тот же симптом — `R-173`
  §«Пределы»); повторный запуск в фоне без лимита вернул `R-174`, `exit=0`. Параллельно номер
  снят вручную тем же правилом, что у аллокатора (максимум+1 по `refs/heads` ∪ `refs/remotes/origin`):
  `git for-each-ref refs/heads refs/remotes/origin | … git ls-tree … research/reviews | grep -oE 'R-[0-9]{3}' | sort -u | tail -1` → `R-173`;
  `git log --all --diff-filter=A --name-only -- 'research/reviews/R-174*'` → пусто. Гонку с
  параллельной сессией ловит `check_artifact_ids.sh` постфактум (`gates.md` §12).
- **Шаблон шапки из мандата барьер ОТВЕРГ дважды**, и побеждает барьер (та же норма, что
  `reviewer.md` TD-165: механизм в агрегате, мандат — проза):
  ```
  $ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_gate_meta.sh 2>&1 | grep -E 'R-174|VERDICT'
  FAIL  research/reviews/R-174-…: milestone «PR-159 (круг 2)» не похож на идентификатор артефакта (КЛАСС-НОМЕР[буква])
  FAIL  research/reviews/R-174-…: verdict «APPROVED» вне перечня (REJECT NOTE APPROVE PASS CONCERNS KILL ESCALATE DECISION)
  VERDICT: FAIL (2) …   exit=1
  ```
  Первое круг 1 уже предсказал (`R-173` §«Пределы»: «мандату следующей перепроверки стоит
  нести форму `PR-NN`, а не `N/A`») — мандат круга 2 добавил к `PR-159` суффикс «(круг 2)»
  и снова не прошёл. Второе новое: мандат предписал `verdict: APPROVED | REJECT`, перечень
  барьера — `APPROVE`. Шапка приведена к `milestone: PR-159`, `verdict: APPROVE`; слово
  «APPROVED» в прозе оставлено — оно из `gates.md` §4 (вердикт reviewer'а), а машинное поле
  берёт форму перечня `check_gate_meta.sh`. Повторный прогон — в Handoff §C. Мандату
  следующей перепроверки §9 стоит нести ОБА поля в форме перечня, а не в форме прозы.
