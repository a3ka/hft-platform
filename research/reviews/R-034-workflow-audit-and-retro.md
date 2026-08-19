<!-- GATE-META
milestone: docs/workflow-audit + docs/retro-audit
audited_repo: a3ka/hft-platform
audited_base: 7a163f7187d36b3c6823561c3e45e71fd650064b
audited_head: 08ef1751c7bf3f6ae86e1777019ecb721139bfc3 (docs/workflow-audit) + 9d85c901f74a933eebb050d4c4c9186cbf962f9b (docs/retro-audit)
verdict: REJECT (docs/workflow-audit) / APPROVED (docs/retro-audit)
-->

# R-034 — PR-гейт двух doc-веток: аудит воркфлоу (REJECT) + ретро-аудит M-32/33/34 (APPROVED)

**Роль:** reviewer. **Дата:** 2026-08-05T20:42Z. **Рабочее дерево:** `/tmp/hft-rev-r034`
(detached от `origin/main` = `7a163f7`), плюс два detached-worktree на головы веток —
общий чекаут и чужие ветки не трогались (`branch-hygiene.md` §1/§2).

**ШАГ 0 — сверка репозитория пройдена** (`origin` = `https://github.com/a3ka/hft-platform.git`,
toplevel `/tmp/hft-rev-r034`, HEAD `7a163f7`). Класс инцидента `8d09e12` (гейт отработал в
чужом дереве) не воспроизведён.

## Итог одной строкой

| ветка | вердикт | почему |
|---|---|---|
| `docs/workflow-audit` (`08ef175`) | **REJECT** | замер §1.1 «Лаунчеры/обёртки: нет» ложен на собственной базе аудита (`.claude/wrappers/` — 8 файлов в дереве, `pi-dev.sh` исполняемый, адаптация einhard W1–W3). Строка несущая: из неё выведены §5 Г2, §7-5 и ранжирование §6; ошибка уже перенесена в спеку M-60 |
| `docs/retro-audit` (`9d85c90`) | **APPROVED, смержен** (`f90a5a8`) | scope чист, все выборочно проверенные утверждения о коде подтвердились построчно; коллизия номеров TD разведена reviewer'ом при merge'е |

---

# §A — `docs/workflow-audit` → **REJECT**

## A.0 Что в ветке

Один коммит `08ef175 [auditor]`, один файл, `+442/−0`:
`docs/plans/workflow-audit-2026-08-einhard-vs-hft.md`. Scope соответствует заявленному;
за зону ничего не вышло; защищённые артефакты целы (Done Block §D).

**Документ сильный, и REJECT не отменяет этого.** §2/§3/§7 — содержательное, проверяемое
сравнение; §3 (наши преимущества: вердикт-в-git под CI-барьером, арбитраж §0, анти-плацебо
пробы самих гейтов, эпистемология `testing.md`, §8-деплой-гейт) выдерживает проверку и
формулирует то, чего мы про себя не формулировали. Реджект — по одной несущей строке замера,
а не по качеству работы.

## A.1 Что проверено грепом и подтвердилось (замеры @`379e3bc` — собственная база аудита)

Проверял на дереве `379e3bc` — той базе, которую документ называет в шапке.

| утверждение §1.1 | замер | итог |
|---|---|---|
| `.claude/rules/` — 6 файлов, 949 строк | 6 файлов; 75+112+352+111+85+214 = **949** | ✅ точно |
| `CLAUDE.md` — 92 строки / 7.8 KB | `92 7793` | ✅ точно |
| `.claude/agents/` — 9 файлов, 440 строк / 43 KB | 9 / **440** / **43032 B** | ✅ точно |
| `docs/04-workflow.md` — 140 строк | 140 | ✅ |
| verify-скрипты — 41 | `scripts/verify_*.sh` = **41** | ✅ точно |
| анти-плацебо пробы — 4 | `scripts/tests/red_*.sh` = **4** | ✅ |
| Git-хуки — нет | `.githooks` отсутствует | ✅ |
| §1.4 `PROJECT-STATE` 3209 стр / 378.9 KB, `TECH-DEBT` 3103 / 386.1 KB | `3209 378858`, `3103 386087` | ✅ точно |
| Г3: `verify_design_claims.sh` вне CI | `grep -rn verify_design_claims .github/` → пусто | ✅ подтверждено |

Воспроизведение:
```
git ls-tree -r --name-only 379e3bc .claude/rules/ | wc -l
for f in $(git ls-tree -r --name-only 379e3bc .claude/rules/); do git show 379e3bc:$f; done | wc -l
git ls-tree --name-only 379e3bc scripts/ | grep -c '^scripts/verify_.*\.sh$'
```

**Вывод по корпусу замеров: он честный.** Именно поэтому находка ниже — не «аудитор всё
выдумал», а «одна строка не измерена, а предположена», и распознать это по документу нельзя,
потому что остальные строки измерены.

## A.2 F-034-1 — **BLOCKER** — §1.1: «Лаунчеры/обёртки: нет» ложно на собственной базе аудита

**Файл:строка:** `docs/plans/workflow-audit-2026-08-einhard-vs-hft.md:50`
> `| Лаунчеры/обёртки | нет | 19 шт. + dispatch-mandate.md (pi-dev.sh единый) |`

**Факт на `379e3bc` (базе, названной самим документом в строке 4):**
```
$ git ls-tree -r --name-only 379e3bc .claude/wrappers/
.claude/wrappers/README.md
.claude/wrappers/dispatch-mandate.md
.claude/wrappers/pi-dev.sh
.claude/wrappers/pi-engine-dev.sh
.claude/wrappers/pi-research-dev.sh
.claude/wrappers/pi-signal-engineer.sh
.claude/wrappers/pi-tester.sh
.claude/wrappers/pi-venue-dev.sh
$ ls -l .claude/wrappers/pi-dev.sh
-rwxrwxr-x 10119 pi-dev.sh          # исполняемый
$ git log --oneline -- .claude/wrappers/
39591f4 docs(process): F-3 R-032 — снял противоречие о git-личности ... [architect]   # 2026-08-03
19f1c4c feat(process): pi-агент лаунчеры — обвязка внешних дешёвых dev-ролей [architect]
```

Это не пустой каталог и не заготовка. `README.md` называет себя «Адаптация einhard-runtime
W1-W3», а сам скрипт делает ровно то, что §2-1 записывает einhard'у в преимущество:

| механика einhard (§2-1 аудита) | наш `pi-dev.sh` | строка |
|---|---|---|
| свежий worktree от `origin/main` / `--branch` | есть | `pi-dev.sh:75-76` |
| инжект персоны роли + `dispatch-mandate.md` в систем-промт | есть | `pi-dev.sh:119-126, 176` |
| self-cleanup worktree при выходе | есть, с защитой «не сношу, если есть незапушенное» | `pi-dev.sh:182-188` |
| 5 ролевых обёрток | симлинки `pi-{engine,venue,research}-dev`, `pi-signal-engineer`, `pi-tester` | `.claude/wrappers/` |
| W6 auto-push feat-ветки при выходе | **НЕТ** — worktree оставляется с напечатанным путём | `README.md` п.4 |
| identity-hook / `core.hooksPath` | сознательно снят (замер `A-003` #27) | `pi-dev.sh:95-97` |

**Почему это BLOCKER, а не примечание.** Строка несущая — из неё выведены три места:

1. **§5 Г2:** «У нас носителя-лаунчера нет — субагент получает текстовый мандат.» Верно
   только для Claude-native ролей (architect, reviewer, risk-critic, critic-субагент);
   для пяти внешних pi-ролей носитель есть и работает.
2. **§7-5** («перенимать НЕ надо»): «Полный лаунчер `pi-dev.sh` — их носитель отдельные
   CLI-процессы; **копировать скрипт некуда**.» Он уже скопирован и адаптирован — коммитом
   `19f1c4c`, за подписью architect'а.
3. **§6 — ранжирование.** §6 ранжирует «закрываемая цена ошибки ÷ **цена внедрения**».
   Цена внедрения механики, для которой носитель уже существует и требует расширения,
   отличается от цены механики, для которой носителя нет. Ранжирование, посчитанное на
   ложном входе, не чинится сноской — его надо пересчитать.

**Ошибка уже распространилась** — ровно тот класс, о котором предупреждает `gates.md` §9
(«ошибка в спеке не падает в CI, а тихо тиражируется»):
```
$ git show origin/feat/M-60-mechanisms:milestones/M-60-mechanisms.md | grep -n 'лаунчер'
70: ... полный лаунчер `pi-dev.sh` — у нас нет носителя (субагенты харнеса, не CLI-процессы);
```
M-60 на этом основании **вычёркивает лаунчер из пространства решений**, тогда как расширение
существующего `pi-dev.sh` (например, добавление W6 и покрытие Claude-native ролей) —
кандидат, который milestone обязан хотя бы рассмотреть.

**Отягчающее — §0 заявляет метод, которым эта строка не проверена.** §0 «Чем мерил»:
«grep по ИСПОЛНЯЕМЫМ путям (`.github/workflows/`, `.githooks/`, **`.claude/wrappers/`**,
`scripts/`)». Читая §0, следующий агент вправе считать строку 50 измеренной. Фактически этот
grep выполнялся по дереву **эталона** (см. §5 Г1, где перечислен тот же набор путей при
поиске вызовов `precommit-chain-integrity-check.sh`), а наш `.claude/wrappers/` не
перечислялся никогда. Это не фабрикация, а смешение двух деревьев в одной формулировке
метода — но последствие то же: непроверенное утверждение выглядит проверенным.

**Что требуется для APPROVED (не дизайн фикса — перечень ложных утверждений):**
1. строка 50 §1.1 — заменить на измеренное значение (что есть, сколько, что покрывает,
   чего в нём нет: W6, Claude-native роли);
2. §5 Г2 — переформулировать: носитель есть и покрывает 5 внешних pi-ролей; не покрыты
   Claude-native субагенты — то есть ровно те роли, в которых случились `C-062` (критик в
   чужом репо) и M-54 (tester в общем чекауте);
3. §7-5 — «копировать некуда» неверно; предмет переноса — недостающие W6/охват, а не скрипт;
4. §6 — пересчитать ранжирование там, где цена внедрения зависит от наличия носителя;
5. §0 — привести описание метода в соответствие с тем, что реально грепалось и по какому дереву.

## A.3 F-034-2 — **MAJOR** — §1.3: находка «мёртвый `core.hooksPath`» сегодня не воспроизводится

**Файл:строка:** `...einhard-vs-hft.md:78`
> `git config --show-origin core.hooksPath` → `file:.git/config  .githooks`

Замер сейчас — настройки нет нигде:
```
$ cd /home/nous/hft-platform && git config --show-origin --get-all core.hooksPath; echo exit=$?
exit=1
$ grep -n hooksPath .git/config; echo exit=$?
exit=1
$ cd /tmp/hft-audit-wf && git config --show-origin --get-all core.hooksPath; echo exit=$?   # дерево самого аудитора
exit=1
```
`.git/config` разделяется всеми worktree, так что «в другом дереве видно иначе» здесь
невозможно. Либо настройку сняли после аудита (тогда §6-4 исполнен и должен быть помечен
исполненным, а не предложен), либо замер был неточен изначально. Второе подряд
непроверяемое утверждение о собственном репозитории — поэтому MAJOR, а не MINOR: на §1.3
стоит рекомендация §6-4.

**Требуется:** перезамерить; если настройка снята — §1.3/§6-4 переписать как «исполнено
(кем, когда)»; если нет — привести воспроизводимую команду с деревом, в котором она видна.

## A.4 F-034-3 — MINOR — §1.1: «`04-workflow.md` не менялся с 07-10»

**Файл:строка:** `...einhard-vs-hft.md:47`. Файл менялся трижды после 07-10:
```
$ git log --format='%h %ad %s' --date=short 379e3bc -- docs/04-workflow.md
0bf5caf 2026-07-23 docs(process): 04-workflow — engine-dev крейты += gateway/gateway-serve
eb2dfa4 2026-07-15 docs(M-09): C-007 ремонт — 4 блокера critic'а
191d5ef 2026-07-14 docs(doc-gate): ремонт по C-006 (M1-M10)
70d12f7 2026-07-10 docs: initial platform design
```
Строк 140 — верно; «не менялся с 07-10» — нет. Утверждение служит тезису «конституция
стабильна», и в ослабленном виде («менялся 3 раза, последний — 07-23, правки точечные») он
сохраняется. Правится одной строкой.

## A.5 F-034-4 — MINOR — §1.2: «`ci.yml`, 5 job'ов» — их 6

`.github/workflows/ci.yml`: `build-test`, `security`, `delivery`, `protected-artifacts`,
`contracts`, `status-check` = **6**. Счёт «8 гейт-скриптов + 3 пробы» сходится (3 cargo-шага
+ 5 bash-гейтов; `cargo audit` не посчитан как гейт-скрипт) — расхождение только в числе
job'ов, вероятно `status-check` (агрегатор) не учтён. Не влияет на выводы; правится числом.

## A.6 Почему REJECT, а не «merge + TD»

Рассматривал merge с TD-записью всерьёз — за него говорит и то, что merge разблокировал бы
`F-064-1` (M-60), и то, что merge чинит одно из шести падений `verify_design_claims` на
`main` (§C.1). Отклонил по трём причинам:

1. **Ошибка не в изложении, а во ВХОДЕ ранжирования.** §6 — операционная часть документа,
   ради которой он писался; она отсортирована по цене внедрения. TD-запись рядом с
   документом не пересчитывает §6 — она лишь фиксирует, что §6 считать нельзя. Тогда merge
   вносит в `main` рекомендации, которыми запрещено пользоваться, под видом принятых.
2. **Ошибка уже исполняется.** M-60 не «может унаследовать» дефект — он его уже унаследовал
   (`M-60-mechanisms.md:70`). Merge источника в этом состоянии закрепляет производную ошибку
   как проверенную: спека будет ссылаться на артефакт в `main`.
3. **Цена реджекта мала и локальна.** Это не переписывание: одна строка таблицы, три
   производных абзаца, перезамер `core.hooksPath` и пересчёт §6 там, где он зависит от
   носителя. Корпус замеров (A.1) устоял целиком и переизмерению не подлежит.

**Что это НЕ значит:** `F-064-1` остаётся открытым, но лечится тем же кругом — как только
исправленный документ попадёт в `main`, источник замеров M-60 войдёт в проверяемую цепочку.
Разблокировка M-60 отложена на один круг, а не отменена.

**Кому:** автору-аудитору (свежий круг, тот же мандат founder'а) либо architect'у как
владельцу `docs/**`. Reviewer правку документа не вносит — `docs/plans/**` вне зоны записи
reviewer'а (`scope-guard.md`), и граница «reviewer описывает дефект, architect проектирует»
(`gates.md` §4) здесь действует буквально.

---

# §B — `docs/retro-audit` → **APPROVED** (смержен `f90a5a8`)

## B.1 Scope и проверка утверждений о коде

Один коммит `9d85c90`, три файла, `+633/−0`: `research/reviews/R-031-retro-audit.md` (516),
`TECH-DEBT.md` (+86), `PROJECT-STATE.md` (+31). Все три — зона записи reviewer'а
(`scope-guard.md`); кода, контрактов, milestone'ов, `docs/**` ветка не касается. RISK-BLOCK
не применим (`crates/risk|killswitch|oms|venue-*` не тронуты — диф пуст по `crates/`).

Утверждения R-031 о коде проверял на **дереве слияния**, не на ветке (`gates.md` §8):

| утверждение R-031 | замер на дереве слияния | итог |
|---|---|---|
| `depth_lifetime.rs:171` — рождением считается только ПЕРВОЕ появление цены | `src/depth_lifetime.rs:171`: `let new_birth = !self.states.contains_key(&l.price);`; :139 в докстринге: «`states`: price→LevelState (**НЕ удаляется** при size=0 — для фиксации fate)» | ✅ построчно |
| `crates/venue-binance-futures/src/lib.rs:1392` — проводка breadth жива | `:1392`: `for event in select_funding_emit(events, &subscribed, true)` | ✅ построчно |
| `crates/ops/src/alerts.rs:85` — алерт ловит ИСЧЕЗНОВЕНИЕ, а не схлопывание широты | `:80-86` `AlertRule{ incident:"TD-014", metric:"md_events_total", summary:"нулевая производная по kind при живом WS (Funding/Trade пропали)" }` | ✅ |

Три из трёх выборочных проверок — точны до номера строки. Это тот уровень привязки, которого
не хватило ветке §A, и он засчитан в пользу APPROVED.

## B.2 Коллизия номеров TD — разведена при merge'е

R-031 заводил долги как **TD-098/099/100** (2026-08-03). За время ожидания merge'а эти номера
заняты в `main` другими долгами параллельными сессиями:

| номер | занят в `main` | предмет |
|---|---|---|
| TD-098 | `R-030` | оракул O-1 M-56 меряет глобальный счётчик в параллельном тест-раннере |
| TD-099 | — | `gateway-serve` остаётся на 400 % CPU после отключения клиентов |
| TD-100 | `R-029`/`R-030` | цена сессии 9.8 MiB получена вычитанием не той величины покоя |
| TD-101 | снят как беспредметный (`e82767b`) — номер остаётся занятым | |
| TD-102 | `R-032` | откат A-003 не доведён до целевого объёма |

Свободны с **TD-103**. Долги R-031 перенумерованы:
`TD-098→TD-103`, `TD-099→TD-104`, `TD-100→TD-105`; ссылки внутри `R-031` (13 шт.) и
`PROJECT-STATE.md` (4 шт.) приведены к новым номерам тем же коммитом; в `TECH-DEBT.md`
добавлено примечание о перенумерации в том же стиле, что уже принят для прежних коллизий
(строки 6–16). Предмет и severity долгов не менялись.

Исторические артефакты `research/*` (включая сам текст R-031 в части, где он цитирует свои
находки) читаются по новым номерам, потому что R-031 мержится ВПЕРВЫЕ — ретроспективной
правки уже опубликованных вердиктов здесь нет.

## B.3 Самоотчёт: перенумерация чуть не задела чужие долги

Механическая замена `TD-098|099|100 → 103|104|105` по `PROJECT-STATE.md` затронула **6**
вхождений, из которых **2 принадлежали собственным долгам `main`** (`TD-098` про оракул O-1
M-56 и `TD-100` про 9.8 MiB) — их renumbering исказил бы. Поймано обязательной сверкой дифа
(`branch-hygiene.md` §9: «диф — ПОСЛЕ»), два вхождения возвращены; в коммит ушли ровно 4
правки, все — в блоках, добавленных веткой. Записываю не ради полноты: правило §9 сработало
как задумано, и это его первый предъявленный случай на doc-ветке, а не на коде.

## B.4 Содержательная оценка R-031

Вердикт годен и полезен: находка `TD-103` (метрика `cancel_fraction` слепа к перерождению
цены, смещение направлено В СТОРОНУ ВЫВОДА, на котором стоит founder-подписанное решение
M-32) — именно тот класс, который `testing.md` («оракул обязан мерить то, что обещает»,
п. 4 про насыщение) описывает как дорогой, и она подтверждена пробой на вербатим-модуле, а
не рассуждением. Поправки в `PROJECT-STATE.md` ослабляют ранее записанные утверждения, не
отзывая выводы — корректная форма для ретро-правки.

---

# §C — Находки вне обеих веток (заведены в TECH-DEBT)

## C.1 `main` КРАСЕН по `verify_design_claims.sh` — 6 нарушений, и это не видно никому

Прогон на чистом `origin/main` = `7a163f7` (до всякого merge'а):
```
FAIL [4-МЁРТВЫЕ-ФАЙЛЫ] docs/SESSION-HANDOFF.md:21 → docs/plans/workflow-audit-2026-08-einhard-vs-hft.md
FAIL [4-МЁРТВЫЕ-ФАЙЛЫ] docs/NEXT-SESSION-PROMPT.md:12,69,132 → docs/ORCHESTRATION-STATE.md
FAIL [4-МЁРТВЫЕ-ФАЙЛЫ] docs/rfc/CT-RFC-06-l2delta.md:51 → docs/ORCHESTRATION-STATE.md
FAIL [7-RFC-PATH]      docs/rfc/CT-RFC-06-l2delta.md:51 → тот же путь
VERDICT: FAIL (6 нарушений)   exit=1
```
Пять из шести — последствие `0bd8b45` (перенос `ORCHESTRATION-STATE.md` в
`docs/archive/orchestration-log-2026-07-08.md`): файл перемещён, входящие ссылки не
переписаны. Шестое — ссылка `SESSION-HANDOFF.md:21` на невлитый документ ветки §A.

**Почему это не поймали:** гейт не в CI — ровно предмет **Г3** самого аудита и его
рекомендации **6-3**. Красное состояние `main` наблюдаемо только тем, кто вручную запустит
скрипт. Заведено **TD-106**; зона правки — architect (`docs/NEXT-SESSION-PROMPT.md`,
`docs/rfc/CT-RFC-06-l2delta.md` вне зоны записи reviewer'а).

Merge `docs/retro-audit` **не ухудшил** состояние: после merge'а те же 6 нарушений, ни одного
нового (Done Block §D).

## C.2 Ссылки на `ORCHESTRATION-STATE.md` в моих файлах — поправлены

`PROJECT-STATE.md` и `TECH-DEBT.md` ссылались на перемещённый файл; пути обновлены на
`docs/archive/orchestration-log-2026-07-08.md`. В исторических вердиктах `research/**`
ссылки НЕ трогал — они правдивы для своего времени (указание founder'а, и то же следует из
`branch-hygiene.md` п.4 про аудит-трейл).

---

# §D — Done Block

```
$ git remote get-url origin && git rev-parse --show-toplevel && git log -1 --format='%h %s'
https://github.com/a3ka/hft-platform.git
/tmp/hft-rev-r034
7a163f7 docs(handoff): §0 — C-062 значился в main, фактически на невлитой ветке [architect]

$ git diff --numstat origin/main...origin/docs/workflow-audit
442	0	docs/plans/workflow-audit-2026-08-einhard-vs-hft.md

$ git diff --numstat origin/main...origin/docs/retro-audit
31	0	PROJECT-STATE.md
86	0	TECH-DEBT.md
516	0	research/reviews/R-031-retro-audit.md

# --- барьер артефактов, ПРОД-ФОРМА (как его зовёт ci.yml: событие + база из события) ---
$ EVENT_NAME=pull_request PR_BASE_SHA=379e3bc bash scripts/check_protected_artifacts.sh   # docs/workflow-audit
OK: защищённые артефакты целы на HEAD (379e3bc..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=b5d82f2 bash scripts/check_protected_artifacts.sh   # docs/retro-audit
OK: защищённые артефакты целы на HEAD (b5d82f2..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=push PUSH_BEFORE=7a163f7 bash scripts/check_protected_artifacts.sh           # дерево слияния
OK: защищённые артефакты целы на HEAD (7a163f7..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

# первый вызов барьера БЕЗ окружения дал FAIL «событие не задано — барьер зовут не так,
# как его зовёт CI»; это корректное fail-closed поведение, а не поломка: гейт, прогнанный
# не той проводкой, не прогнан (testing.md, «целостность гейта», свойство 1).

$ bash scripts/tests/red_protected_artifacts.sh | tail -2
VERDICT: PASS (20/20) — барьер держит при ТОЙ ЖЕ проводке, какой его зовёт CI
exit=0

# --- merge-preview (gates.md §8: документ правдив на ДЕРЕВЕ СЛИЯНИЯ) ---
$ bash scripts/verify_design_claims.sh                      # чистый origin/main, БАЗА
VERDICT: FAIL (6 нарушений)    exit=1     # см. §C.1 — предсуществующее состояние main

$ cd <wt docs/workflow-audit> && bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: FAIL (5 нарушений)    exit=1     # на 1 меньше базы: merge чинит SESSION-HANDOFF.md:21

$ cd <wt docs/retro-audit> && bash scripts/verify_design_claims.sh --merge-preview origin/main
FAIL [SETUP] --merge-preview: слияние origin/main + HEAD (9d85c90) КОНФЛИКТУЕТ ...
VERDICT: FAIL (1 нарушений)    exit=1     # конфликт TECH-DEBT.md = коллизия TD-номеров, §B.2

$ # после ручного разрешения конфликта, обычный режим на РЕЗУЛЬТАТЕ (как предписывает сам скрипт):
$ git merge --no-ff origin/docs/retro-audit && bash scripts/verify_design_claims.sh
VERDICT: FAIL (6 нарушений)    exit=1     # те же 6, что и на базе; НИ ОДНОГО нового

# --- код не тронут ни одной из веток ---
$ git diff origin/main..HEAD --stat -- crates/ Cargo.toml Cargo.lock scripts/ .github/
(пусто)                        exit=0

# --- push-scope (gates.md §8) ---
$ git log origin/main..HEAD --format='%h %an %s'
f90a5a8 Alex Kurz merge(docs/retro-audit): R-031 — ретро-аудит M-32/M-33/M-34 + TD-103/104/105 [reviewer]
9d85c90 reviewer  docs(review): R-031 — ретро-аудит M-32/M-33/M-34 + сверка PROJECT-STATE + процессный вывод
# чужих коммитов нет

# --- индекс ДО коммита / диф ПОСЛЕ (branch-hygiene.md §9) ---
$ git status --porcelain && git diff --cached --stat
M  PROJECT-STATE.md
M  TECH-DEBT.md
A  research/reviews/R-031-retro-audit.md
 PROJECT-STATE.md                      |  31 ++
 TECH-DEBT.md                          |  92 ++++++
 research/reviews/R-031-retro-audit.md | 516 ++++++++++++++++++++++++++++++++++
 3 files changed, 639 insertions(+)         # 0 удалений

$ git show --numstat --format='' f90a5a8
31	0	PROJECT-STATE.md
92	0	TECH-DEBT.md
516	0	research/reviews/R-031-retro-audit.md
```

Тесты/clippy/fmt не гонялись намеренно и это НЕ пробел: диф обеих веток пуст по `crates/`,
`Cargo.*`, `scripts/`, `.github/` (команда выше, exit=0) — кодовые гейты не могут измениться.
CI на push всё равно прогонит их целиком; пруф — §E.

---

# §E — Пост-merge деплой-гейт (`gates.md` §8)

**Push:** `7a163f7..55e8189 HEAD -> main` (exit=0). Отдельно вердикт положен на ветку
предмета реджекта: `08ef175..1af58cc HEAD -> docs/workflow-audit` — чтобы следующий круг
читал находки на своей ветке, а не искал их в переписке (`gates.md` §4, урок M-49).

Перед push'ем на `docs/workflow-audit` доказана МОЛЧАНИЕ автора, а не пустота
(`branch-hygiene.md` п.8): worktree `/tmp/hft-audit-wf` чист, HEAD не двигался с
`2026-08-05T00:02Z`, mtime предмета — `00:02:18`, замер в `20:47Z` ⇒ сессия аудитора
завершена ~20 ч назад.

```
$ gh run list --limit 3
completed  success  docs(review): R-034 — PR-гейт двух doc-веток; retro-audit APPROVED, w…  CI  main  push  31045724018  10m51s  2026-08-05T20:47:12Z
completed  success  docs(handoff): §0 — C-062 значился в main, фактически на невлитой вет…  CI  main  push  31000261420   9m53s  2026-08-05T11:08:32Z
completed  success  docs(audit 6-2): ORCHESTRATION-STATE убран из startup-протокола, журн…  CI  main  push  30962905160  10m57s  2026-08-05T00:19:15Z

$ gh run watch 31045724018 --exit-status
✓ All checks passed
watch_exit=0

$ gh run list --workflow=deploy.yml --limit 1
completed  success  merge(M-56): снапшот без клонирования состояния …  Deploy to VPS  main  push  30850551377  2026-08-03T20:31:02Z
# Deploy НЕ триггерился — path-фильтр (crates/**, Cargo.*), диф merge'а чисто документный.
# Значит рестарта recorder'а нет; проверка ниже подтверждает это по uptime, а не по вере.

$ ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 \
    'docker ps --format "{{.Names}} {{.Status}}"; cat .../recorder.heartbeat; date -u +%s'
hft-gateway-serve Up 2 days (healthy)
hft-recorder      Up 2 days (healthy)
{"events":16398646,"free_bytes":68840169472,"min_free_bytes":10737418240,
 "next_seq":175768396,"segment_index":188,"ts_wall_ms":1785963496420,"writable":true}
1785963500
```

**Чтение пруфа:** `Up 2 days` у обоих контейнеров = рестарта не было (ожидаемо: deploy не
запускался) · heartbeat свежий на **4 секунды** (`1785963500 − 1785963496`) · журнал растёт:
`next_seq` 159 121 674 → **175 768 396**, сегмент 167 → **188** относительно замера
2026-08-03 · `writable: true`, свободно 68.8 GB при пороге 10 GB.

Содержательный sanity свежих событий не требуется: деплой не выполнялся, поведение данных
merge не менял (диф пуст по `crates/`) — то есть условие «деплой менял парсеры/форматы»
из §8 п.2 не наступило.

# §F — Вердикт

- `docs/workflow-audit` (`08ef175`) — **REJECT.** Блокер `F-034-1` (§1.1:50), major
  `F-034-2` (§1.3:78), minor `F-034-3` (§1.1:47), `F-034-4` (§1.2). Merge не выполнен.
- `docs/retro-audit` (`9d85c90`) — **APPROVED**, смержен в `main` коммитом `f90a5a8`
  (`--no-ff`), долги перенумерованы в TD-103/104/105.
- Новый долг: **TD-106** (`main` красен по `verify_design_claims.sh`, гейт вне CI).
