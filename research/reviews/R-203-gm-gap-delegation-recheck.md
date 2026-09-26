<!-- GATE-META
milestone: R-194
audited_repo: a3ka/hft-platform
audited_base: 29b0a93a17df5d499e607be05d9a51afbea4eb8e
audited_head: 8cfca8b31583e317d2df78b98ae515d4a7316e11
verdict: NOTE
-->

# R-203 — перепроверка `gates.md` §9: `docs/gm-gap-delegation` (вершина `8cfca8b`)

**Вердикт: NOTE.** Норма «предмет гейта — ВЕТКА; вершину агент берёт командой; SHA мандата —
справка; ушла вперёд ⇒ судится вершина; не предок ⇒ СТОП» — по существу верна, единственное
место нормы выбрано правильно (`docs/04-workflow.md` §2), профили несут ссылки, а не пересказ,
смысл `TD-036`/`RN-18` (стоп при незапушенном коммите) при переписывании п.1 `tester.md`
СОХРАНЁН. Все утверждения о коде и документах подтверждены командами на дереве слияния
(§1). Полномочия в порядке (§2). Блокирующих находок нет. Три находки класса «носитель
расходится с нормой» (Б-1..Б-3) — правки в одно-два слова каждая, две из них в ЭТОМ ЖЕ
диффе; Б-1 стоит исправить до PR, потому что без неё «взятая командой вершина» может
оказаться устаревшим локальным ref'ом — тот самый класс, который норма лечит, только теперь
с ложной строкой-доказательством в Done Block'е. Находка автора о ложном красном
`check_archived_refs.sh` ПОДТВЕРЖДЕНА исполнением (§5), с дополнением о базовой линии.

Роль: architect-клон (Fable), свежий контекст. Автор правки — сторона; его рамку не
наследовал, вершину взял командой (§0), первоисточники открывал, а не читал в пересказе.

## §0. Вершина взята САМА, а не из мандата

Мандат SHA намеренно не называл — тем самым эта перепроверка исполнена в форме, которую
судит.

```text
$ git fetch origin && git rev-parse origin/docs/gm-gap-delegation
8cfca8b31583e317d2df78b98ae515d4a7316e11
$ git merge-base origin/main origin/docs/gm-gap-delegation
29b0a93a17df5d499e607be05d9a51afbea4eb8e
$ git rev-parse origin/main
29b0a93a17df5d499e607be05d9a51afbea4eb8e          # база == main: ветка не отстаёт
$ git log --oneline origin/main..origin/docs/gm-gap-delegation
8cfca8b docs(GM-GAP): ссылка на GM-11 без пути — обход ложного красного archived-refs [architect]
3aefa69 docs(GM-GAP): предмет гейта — ВЕТКА; вершину агент берёт командой [architect]
$ git diff --stat 29b0a93..8cfca8b
 .claude/agents/architect.md   |  4 ++++
 .claude/agents/critic.md      |  3 +++
 .claude/agents/reviewer.md    |  1 +
 .claude/agents/risk-critic.md |  3 +++
 .claude/agents/tester.md      | 15 ++++++++-----
 docs/04-workflow.md           | 49 +++++++++++++++++++++++++++++++++++++++++++
 6 files changed, 70 insertions(+), 5 deletions(-)
$ git worktree add /tmp/hft-arch-gmgap-recheck 8cfca8b…   # detached; общий чекаут не трогал
$ git merge-tree --write-tree origin/main HEAD >/dev/null; echo $?
0                                                  # дерево слияния чистое
$ bash scripts/check_branch_health.sh | tail -2
веток кроме main: 15; замечаний: 0
VERDICT: PASS — наблюдение состоялось
```

База совпадает с `origin/main`, поэтому «на ветке» и «на дереве слияния» — одно и то же
дерево; отдельный merge-preview прогнан всё равно (§1, первая строка).

## §1. Пункт (а) — каждое утверждение о коде и документах проверено командой

```text
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | grep -vc '^PASS'  … 
NOTE  [H-FACTS] docs/plans/contracts-current-state.md: 23 утверждений … без маркера FACTS
VERDICT: PASS (0 нарушений)
exit=0                                              # NOTE — чужой документ, не этой ветки
```

| утверждение нового текста | где | команда / первоисточник | результат |
|---|---|---|---|
| `C-232` судил `684ea89` — первый из четырёх авторских коммитов | `04-workflow.md:71-72` | `git show origin/docs/td-216-fa-depth-range:research/reviews/R-194-…md` §0: лог ветки — 5 коммитов, из них 4 авторских + `C-232`; `git show origin/docs/td-216-fa-depth-range:research/critiques/C-232-…md \| grep audited_head` → `684ea895…` | **верно** |
| тестер прогнал `a65b859` вместо фикса; артефакта гейта нет | `04-workflow.md:75-77` | `git grep -n a65b859` → только `session-handover-2026-09-21.md:40`, `next-session-bootstrap-2026-09-22.md:209`, `ROADMAP.md:109`, `R-188:39` (там `a65b859` — коммит engine-dev по `docker-compose.yml`, M-86); вердикт-файла tester'а не существует по конструкции роли | **верно, и оговорка «артефакта нет» честна** |
| `R-194` §5: расширение `is_gate_class` покрасило бы **пять** вердиктов, **четыре** — `DECISION` | `04-workflow.md:84-88` | `R-194` §5 (iii) — таблица WOULD-FAIL: 5 строк; перепроверено НЕ по таблице, а по файлам на четырёх ветках: `A-028`/`A-032` (`docs/M-73-closeout-architect`), `A-029` (`feat/M-72-…`), `A-033` (`feat/M-84-fixed-bands`) → `verdict: DECISION`; `C-232` → `verdict: NOTE` | **верно: 5 / 4** |
| `is_gate_class` в `scripts/check_gate_meta.sh` | `04-workflow.md:83` | `grep -n is_gate_class scripts/check_gate_meta.sh` → `:266` определение, `:520` вызов | **верно** |
| «`audited_head` существует и является предком `HEAD` — ровно то, что `check_gate_meta.sh` проверяет» | `04-workflow.md:92-93` | `:460` «не существует в этой истории», `:461` `merge-base --is-ancestor`, `:505` «НЕ предок HEAD» | **верно** |
| спека `M-60b`, строка GM-11 таблицы запретов | `04-workflow.md:88` | `grep -n GM-11 milestones/M-60b-gate-mechanisms.md` → `:247` «Применять subject-lock к REJECT/… — лок, красящий нормальный круг, вреднее отсутствующего (GM-11)» | **верно** |
| «он ловит правку ПОСЛЕ вердикта» | `04-workflow.md:88-90` | `check_gate_meta.sh:516-521`: `own_touched "^${ah}"` — диапазон ПОСЛЕ `audited_head` | **верно** |
| наблюдатель по образцу ВИСЯК/ДУБЛЬ в `check_branch_health.sh` | `04-workflow.md:97` | прогон §0: печатает `ВИСЯК`, `ДУБЛЬ`, `НЕИЗВЕСТНО` | **верно** |
| «новый харнесс под заморозкой `П-017` A2» | `04-workflow.md:97-98` | `docs/PENDING-SIGNATURE.md:863-1088` (`П-017`): A2 — заморозка НОВОГО харнесса; три точечные разморозки (22.08 ×2, 26.08) — поимённо, наблюдателя среди них нет | **верно** |
| `П-019` — правило на принимающей стороне, `COGNITIVE-ONLY` | `04-workflow.md:94` | `PENDING-SIGNATURE.md:1271-1336`: «Правило поставлено на ПРИНИМАЮЩЕЙ стороне намеренно — здесь оно самоисполнимо (отказ в твоих руках)» (цитируется в `tester.md:62-65`) | **верно** |
| `gates.md` §4, шапка `GATE-META` | `04-workflow.md:59` | `.claude/rules/gates.md` §4 — блок `GATE-META` с полем `audited_head` | **верно** |
| `session-handover-2026-09-21.md` §4 (а) | `04-workflow.md:76` | открыт: §4 (а) — «Сработало ДВАЖДЫ: тестер прогнал `a65b859`…; критик судил `684ea89`…» | **верно** |
| `TD-036/RN-18` — стоп при SHA не на `origin` | `tester.md:35` | `docs/archive/TECH-DEBT-triage-2026-08-17.md:625` — `TD-036` `chain-bootstrap-from-worktree-not-origin-feat`; `R-174:153` «преднамеренный СТОП по TD-036/RN-18 (SHA не на origin — нечего проверять)»; `engine-dev.md:51` — та же пара как hard-precondition push'а | **смысл сохранён**: стоп остался ровно для случая «SHA нет на origin» (`--is-ancestor` ≠ 0) |
| `git merge-base --is-ancestor <sha> origin/feat/M-NN` → не 0 ⇒ СТОП | `tester.md:36` | семантика git: exit 0 = предок; иное = не предок / не существует | **верно** |
| «инцидент M-30», «M-54» | `tester.md:38,70` | текст до правки — не этой ветки | не судится |
| commit body: «второй носитель одной нормы расходится молча (A-023 §2.4)» | тело `3aefa69` | `research/arbitration/A-023-…md:160` §2.4, `:177` «объявляется в одном машинно-находимом месте, остальной текст ССЫЛАЕТСЯ» | **верно** |

Остаточные ссылки старой нормы («совпасть с SHA из мандата», «целевой SHA») в устав-зоне:

```text
$ git grep -nP 'совпасть с SHA|SHA из мандата|целевой SHA|SHA, который просят|названн\w+ SHA|SHA мандата' \
    -- '.claude/**' docs/04-workflow.md CLAUDE.md docs/workflow/harness-track.md
.claude/agents/critic.md:47      … SHA из мандата — справка …        # НОВЫЙ текст
.claude/agents/reviewer.md:25    … SHA из мандата — справка …        # НОВЫЙ текст
.claude/agents/risk-critic.md:57 … SHA из мандата — справка …        # НОВЫЙ текст
.claude/agents/tester.md:34,38,75                                      # НОВЫЙ текст
$ git grep -nP 'SHA|rev-parse' -- '.claude/wrappers/dispatch-mandate.md' '.claude/wrappers/pi-tester.sh'
(шаблон мандата SHA не впрыскивает; pi-tester.sh:73 делает fetch — см. Б-1)
```

Старая формулировка нигде не осталась. Висячих ссылок в новом тексте нет: каждая `§N`/файл
из таблицы выше открыт и существует на дереве слияния.

## §2. Пункт (б) — полномочия

```text
$ EVENT_NAME=pull_request PR_BASE_SHA=29b0a93a… bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0                                   # молчит по конструкции; bash -x: grep FOUNDER-APPROVED → exit 0
$ git log --format=%B 29b0a93..8cfca8b | grep -c '^FOUNDER-APPROVED: .\{12,\}'
2                                        # в КАЖДОМ из двух коммитов
$ bash scripts/check_context_budgets.sh | tail -1
VERDICT: PASS — 7 файлов, 112960 B из 114900 B бюджета (запас 1940 B)
$ git diff --name-only 29b0a93..8cfca8b -- CLAUDE.md .claude/rules | wc -l
0                                        # ядро (по определению check_context_budgets.sh:47) не тронуто
$ for s in check_roadmap_sync check_gate_meta check_protected_artifacts check_artifact_ids check_review_fa; do …; done
check_roadmap_sync exit=0 VERDICT: PASS
check_gate_meta    exit=0 VERDICT: PASS — вердиктов проверено: 0 …
check_protected_artifacts exit=0 · check_artifact_ids exit=0 · check_review_fa exit=0 SKIP (не трогает crates/**)
```

- **Зона.** `docs/04-workflow.md` + `.claude/agents/*` — зона architect'а (`architect.md` §Writes);
  замок §11 — токен в каждом коммите, проверено барьером и грепом. Причина токена («делай
  свое» на план сессии) — декларация; истинность механизмом не проверяется (§11 сам это
  говорит), здесь принимается как заявленная.
- **Граница C** не задета: ни данных, ни промоушенов, ни фаз.
- **`П-017` A3.** «Ядро не тронуто» — верно по определению механизма
  (`check_context_budgets.sh:47`: «РОВНО впрыскиваемое ядро: `CLAUDE.md` + `.claude/rules/*.md`»).
  Названный предел — Н-5 ниже.
- **`П-017` A2.** Нового харнесса нет: `git diff --name-only` — только два документа-класса,
  `scripts/**` и `.github/**` не тронуты. Наблюдатель НЕ построен и назван как не построенный
  (`04-workflow.md:96-98`) — форма, которую `A-023` §2.4 предписывает для замороженного
  кандидата.
- **`binding-requires-mechanism`.** Тег `COGNITIVE-ONLY` стоит в ЗАГОЛОВКЕ секции
  (`04-workflow.md:51`), причина — отдельным абзацем (`:91-94`) и она верна: барьер видит
  только существование/предковость `audited_head`, откуда взят SHA — не видит. Правило
  «после третьего повторения проза запрещена» не сработало: класс предъявлен ДВАЖДЫ, из них с
  артефактом — один раз. Н-6 ниже — что делать на третьем.

## §3. Пункт (в) — связность: находки

### Б-1 (NOTE, исправить ДО PR) — `git fetch origin` выпал из команды в четырёх носителях

Норма п.2 (`04-workflow.md:57`): «`git fetch origin && git rev-parse origin/<ветка>`». Носители:

```text
$ grep -n 'fetch' .claude/agents/tester.md .claude/agents/critic.md .claude/agents/reviewer.md .claude/agents/risk-critic.md
(пусто)
tester.md:74       git rev-parse origin/<ветка>          # ВЕРШИНА, взятая командой; её и прогоняешь
critic.md:46       … (`git rev-parse origin/<ветка>`) …
risk-critic.md:56  … (`git rev-parse origin/<ветка>`) …
reviewer.md:25     … (`git rev-parse origin/<ветка>`) …
```

`git worktree add <path> origin/feat/M-NN` сеть не трогает, `rev-parse origin/<ветка>` читает
ЛОКАЛЬНЫЙ remote-tracking ref. Клон, стартовавший без fetch (Claude-native субагент — без
обёртки; `pi-tester.sh:73-74` fetch делает, но при отказе МОЛЧА бутстрапится от локального
ref'а с предупреждением в stderr), напечатает в ШАГ 0 устаревший SHA и назовёт его
«вершиной, взятой командой». Это тот же класс «судить прошлое», который правка лечит, — но
теперь с ложной строкой-доказательством в Done Block'е и с `audited_head`, который барьер
пропустит (предок `HEAD`). Направление отказа здесь НЕ fail-closed: `--is-ancestor` SHA
мандата на устаревшем ref'е даст «не предок ⇒ СТОП» только если ветка ушла вперёд ОТ
мандата, а если мандат сам назвал устаревший SHA — оба совпадут и прогон пройдёт.

Правка: слово `git fetch origin &&` перед `rev-parse` в четырёх местах (в `tester.md` ШАГ 0 —
отдельной строкой блока, чтобы её exit попал в Done Block).

### Б-2 (NOTE) — «печатаешь оба SHA» не имеет места в Done Block'е tester'а

`tester.md:38-39`: «Вершина УШЛА ВПЕРЁД от названного SHA — не стоп: прогоняешь вершину и
печатаешь оба SHA». `04-workflow.md:63`: «оба SHA печатаются рядом». Но ШАГ 0
(`tester.md:71-78`) — четыре строки, и ни одна не печатает SHA мандата и не предъявляет исход
`--is-ancestor`, на котором держится СТОП п.1. Условие TD-036 объявлено, а свидетельство для
него в Done Block'е не предусмотрено — reviewer не увидит, была ли проверка сделана.

Правка: пятая строка ШАГ 0 —
`git merge-base --is-ancestor <SHA мандата> origin/<ветка>; echo ancestor=$?` (0 — предок,
прогоняешь вершину; иначе — СТОП п.1). Число строк «четыре» → «пять». В `04-workflow.md` п.3
— та же команда уже названа, добавлять нечего.

### Б-3 (NOTE, вне этого диффа — назвать, не править здесь) — `gates.md` §4 несёт старый ШАГ 0

`.claude/rules/gates.md` §4, таблица артефактов: «tester | Done Block + ШАГ 0 (`pwd`,
`git log -1`, `ls` предмета)». После правки ШАГ 0 — четыре (по Б-2 — пять) строк. Это ровно
класс «второй носитель одной нормы расходится молча» (`A-023` §2.4), на который автор сам
ссылается в теле коммита. Править в ЭТОМ PR нельзя и не нужно: `gates.md` — ядро (`П-017` A3,
запас бюджета 27 B по `check_context_budgets.sh`). Достаточно заменить скобку указателем
той же длины («ШАГ 0 — `tester.md` п.2») отдельным коммитом ядра, когда ядро будут трогать по
другому поводу, — либо принять как названный остаток. Не блокер: строка §4 — сводка, норма
живёт в профиле.

### Н-1 (INFO) — «два случая», «пять / четыре» — числа правдивы

Оба случая различены честно: `C-232` — с артефактом (`R-194` §0), `a65b859` — только записка
сессии; текст так и говорит (`04-workflow.md:76-77`). Пять вердиктов / четыре `DECISION` —
перепроверено по файлам на четырёх ветках, не по таблице `R-194` (§1).

### Н-2 (INFO) — `critic.md` Startup reading п.1 — §3, указатель нормы — §2

`critic.md:42` велит читать `04-workflow.md` §3; новый указатель (`:46-48`) ведёт в §2. Не
противоречие — указатель адресный, — но критик, читающий «по списку», §2 не откроет.
Достаточно того, что есть: указатель самодостаточен (команда + развилки названы).

## §4. Пункт (г) — по существу: закрывает ли норма класс, где дыры

**Закрывает** класс «вердикт над не-вершиной при ушедшей вперёд ветке» — при условии Б-1.
Развилка п.3 (`04-workflow.md:60-64`) правильно различает три исхода (совпал / предок / не
предок). Дыры и рекомендации — рекомендациями, не правками:

1. **Мандат вовсе без SHA.** Норма делает SHA справкой, то есть НЕОБЯЗАТЕЛЬНЫМ
   (`architect.md`: «допустим только справкой»). Без справки развилка «не предок ⇒ СТОП»
   обезоружена, «оба SHA печатаются» невозможно, force-push / подмена ветки не обнаруживаются
   этой нормой вовсе (ловит только `check_gate_meta.sh:505` — и лишь если на ветке уже есть
   вердикт с `audited_head`). Рекомендация: справка ОБЯЗАТЕЛЬНА, предмет — ветка. Одно слово
   в п.1 нормы: «называет ВЕТВЬ и, справкой, вершину на момент передачи».
2. **Ref не разрешается** (ветка удалена, опечатка): `rev-parse` падает, агент без явного
   правила может «поискать похожую». Рекомендация: явная клауза «ref не разрешился ⇒ СТОП»,
   fail-closed, как у `П-019`.
3. **Ветка ушла ПОСЛЕ взятия вершины, посреди прогона.** Норма молчит; вердикт честно
   называет то, что судил, — это правильно, судить движущуюся цель нельзя. Рекомендация:
   перед записью `GATE-META` — повторный `fetch && rev-parse`; расхождение с §0 печатается
   как строка вердикта (не пересуживается). Это делает наблюдаемым то, что кандидат-наблюдатель
   покажет только после разморозки.
4. **Force-push при наличии справки** — ловится (не предок ⇒ СТОП). Верно.
5. **Тест нормы её же формой.** Эта перепроверка исполнена по норме (мандат без SHA, вершина
   взята командой, §0) — норма исполнима на принимающей стороне, что и требуется от
   `COGNITIVE-ONLY` с принимающей стороны.

### Н-4 (NOTE о МАНДАТЕ, не о ветке) — «milestone: GM-GAP» барьер отвергает

Мандат этой перепроверки велел писать в шапку `milestone: GM-GAP`. Прогон на диапазоне с
вердиктом:

```text
$ EVENT_NAME=pull_request PR_BASE_SHA=29b0a93a… bash scripts/check_gate_meta.sh
FAIL  research/reviews/R-203-…md: milestone «GM-GAP» не похож на идентификатор артефакта (КЛАСС-НОМЕР[буква])
VERDICT: FAIL (1)
```

`check_gate_meta.sh:439-444`: форма `^[A-Za-z]+-[0-9]+[a-z]?$` (`gates.md` §12) — `GM-GAP`
строка роадмапа, не идентификатор артефакта. Поставлено `milestone: R-194` — артефакт,
чьё §5 выбрало лекарство, которое ветка исполняет (прецеденты вердиктов по не-milestone
предметам: `R-149`, `C-101`, `TD-020`). Замечание к автору мандата, не к диффу: форма
шапки в мандате не была прогнана барьером — та же ось «утверждение без команды», по которой
устроена и сама правка.

### Н-5 (INFO) — граница «ядра» названа, чтобы A3 не читался шире

`check_context_budgets.sh:47,71-77` — ядро есть `CLAUDE.md` + семь `.claude/rules/*.md`.
Профили `.claude/agents/*` впрыскиваются роли (`architect.md` §Startup: «этот профиль
(впрыск)»), но в бюджет не входят. Правка добавляет в четыре профиля по указателю и
переписывает существующий п.1/ШАГ 0 tester'а — новой нормативной единицы во впрыскиваемом
ядре по определению механизма нет; по духу A3 (объём впрыска) — +26 строк в профили.
Не нарушение; названо, чтобы следующая правка профилей не ссылалась на этот PR как на
прецедент «профили — не ядро».

### Н-6 (INFO) — третье повторение класса запирает прозу

`binding-requires-mechanism`: после третьего повторения — механизм либо запись остаточного
риска. Класс — на счёте 2 (один с артефактом). Кандидат-механизм (наблюдатель) — под A2.
Рекомендация: заготовить запрос разморозки по образцу `П-017` (порог ≥3 срабатываний,
предъявленных замером) ДО третьего случая, а не после; строка `GM-GAP` роадмапа при
приземлении PR обновляется автором (ход (1) сделан, ход (2) ждёт разморозки) — `roadmap-sync`
сейчас PASS, статус-колонка — самоприземляемая правка.

## §5. Находка автора о `check_archived_refs.sh` — ПОДТВЕРЖДЕНА исполнением

```text
$ ls docs/archive/ | grep -E '^M-60'
M-60-mechanisms-umbrella-2026-08.md
$ sed -n '87p;91p' scripts/check_archived_refs.sh
    M-*.md)        id="$(printf '%s' "$art" | grep -oE '^M-[0-9]+[a-z]?')"; old="milestones/${id}" ;;
  hits=$(git grep -n -F "$old" -- "${LIVE[@]}" 2>/dev/null | grep -v 'ARCHIVED-REF-OK' || true)
$ git checkout -q 3aefa69 && bash scripts/check_archived_refs.sh; echo exit=$?
FAIL  висячая ссылка на вынесенный артефакт «M-60-mechanisms-umbrella-2026-08.md» (прежний путь «milestones/M-60»):
        docs/04-workflow.md:91:отсутствующего (`milestones/M-60b-gate-mechanisms.md` GM-11). …
VERDICT: FAIL (провалов: 1)
exit=1
$ git checkout -q 8cfca8b && bash scripts/check_archived_refs.sh; echo exit=$?
VERDICT: PASS
exit=0
$ ls milestones/M-60b-gate-mechanisms.md
milestones/M-60b-gate-mechanisms.md              # файл ЖИВ
```

Механизм дефекта ровно тот, что назван в теле `8cfca8b`: `old="milestones/M-60"` без
границы, `git grep -F` — подстрока, префикс `M-60` совпадает с живым `M-60b`. Обход
формулировкой (`8cfca8b`) законен: барьер — отдельный предмет харнесс-трека.

**Дополнение, которого в теле коммита нет.** Базовая линия УЖЕ несёт тот же дефект:

```text
$ grep -n 'milestones/M-60$' scripts/lib/archived_refs_baseline.txt | wc -l
9
$ grep -n 'milestones/M-60' scripts/lib/archived_refs_baseline.txt | head -3
14:.claude/rules/gates.md|milestones/M-60
33:docs/ROADMAP.md|milestones/M-60
41:scripts/verify_M-60b.sh|milestones/M-60
```

`gates.md` §11 ссылается на `milestones/M-60a-docs-freeze.md` (живой файл); `ROADMAP.md:109`
— на `milestones/M-60b-gate-mechanisms.md:247` (живой). Эти девять строк амнистируют не
висячие ссылки, а ЛОЖНЫЕ СРАБАТЫВАНИЯ, и правило базовой линии («протухшая строка = FAIL»,
`:64-67`) при починке префикса потребует их снять. Предмет харнесс-трека — назвать в
Handoff'е рядом с самим дефектом, чтобы фикс не остановился на регексе.

## §6. Что НЕ найдено — сказано прямо

- Противоречий нового текста `П-019` (список команд дословно), `gates.md` §0 (арбитр читает
  первоисточники), §8 (push перед handoff), §9 (перепроверка на ветке) — нет; норма
  дополняет, не пересекает.
- Утраты смысла `TD-036`: нет — стоп сохранён для «SHA нет на origin», снят только для
  «ветка ушла вперёд», где он и был ложным (§1, строка TD-036).
- Висячих ссылок, чужих файлов в диапазоне, правок вне зоны: нет (`git diff --name-only` —
  шесть файлов, все в зоне architect'а).

## §7. Done Block

```text
$ pwd
/tmp/hft-arch-gmgap-recheck
$ git rev-parse HEAD
8cfca8b31583e317d2df78b98ae515d4a7316e11
$ git status --porcelain
(пусто до записи этого файла)
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -1; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=29b0a93a17df5d499e607be05d9a51afbea4eb8e bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0
$ bash scripts/check_context_budgets.sh | tail -1
VERDICT: PASS — 7 файлов, 112960 B из 114900 B бюджета (запас 1940 B)
$ bash scripts/check_archived_refs.sh >/dev/null 2>&1; echo exit=$?
exit=0
$ bash scripts/next_artifact_id.sh R
R-203
$ RESERVE_ROLE=architect bash scripts/reserve_artifact_id.sh R
reserve: резерв R-203 взят
```

Ярус S (`gh run list`, деплой, ssh на VPS) не снимался: предмет — документы устава, прод не
задет; `check_branch_health.sh` — §0.
