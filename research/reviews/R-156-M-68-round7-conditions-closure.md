<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 675d8b6caa96fe0d3ee40b70078f648ecbbca9be
audited_head: 9899cf853bc64cb3f98f6dfc83e187e42a2cb088
verdict: APPROVE
-->

# R-156 — M-68 круг 7: закрытие условий `R-154`. PR-time reviewer, **APPROVED**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-30T12:05Z
**Предмет:** PR #124, `feat/M-68-rev4`, диапазон `675d8b6c..9899cf85` (75 файлов, +11021/−221).
**Предыдущий круг:** `R-154` (REJECT, `audited_head` `ebbd765f`) — мой же вердикт; этот круг
судит закрытие ЕГО условий.

**Живой инвариант тронутого модуля (`gates.md` §4, M-66).** Диф трогает `crates/gateway/**` и
`crates/gateway-serve/**`; дом — `docs/fa/viz-backend.md`. Называю `VB-I-2` («live == replay:
серия, посчитанная на live-хвосте, бит-идентична серии из replay того же окна журнала») — он
и есть тот инвариант, вокруг которого построена CLOSE-семантика каденции, и `VB-I-10`
(bounded-window snapshot, `TD-039`), который §3.1 спеки прямо запрещает ослаблять. Оба
предъявлены живыми на судимой ревизии: `git show 9899cf8:docs/fa/viz-backend.md` даёт
непрерывный ряд `VB-I-1..VB-I-11`.

---

## Поправка к мандату — ДВЕ фактические неточности, обе сняты замером

Мандат этого круга сообщал два утверждения, которых на момент начала работы не было в фактах.
Называю их не в упрёк, а потому что вердикт, принявший чужую декларацию на слово, — это
ровно тот класс, который `gates.md` §8 называет «отчёт агента — гипотеза, состояние git — факт».

**(i) «чеки 17/17 SUCCESS» — на момент старта было НЕВЕРНО.** Замер:

```
mergeStateStatus: BLOCKED
fmt + clippy + test          pending  0
fmt + clippy + test (ветка)  pending  0
агрегат «All checks passed» — ОТСУТСТВОВАЛ в списке
```

Дождался терминального статуса `gh pr checks 124 --watch` (форма из `gates.md` §8; без
`--watch` команда возвращается немедленно и «ожидание» не наступает) → `CHECKS_EXIT=0`,
17/17 `pass`, агрегат `All checks passed` — `pass`. То есть утверждение стало истинным ПОЗЖЕ,
а не было им. Блокером не является; в Done Block оба состояния сырыми.

**(ii) «токен subject-lock закрыт ранее (`a136be9`)» — носитель назван не тот.** Замер по телам
коммитов: `ALLOW-SUBJECT-CHANGE` лежит в **`c1f6649`**; `a136be9` несёт ТРИ строки
`TERMINAL-BRANCH-VERDICT` — это условие 2, класс (i), а не условие 1. Оба токена на месте,
поэтому исход не меняется, но перепутанный носитель в следующей передаче стал бы ложным следом.

---

## Block-scope — ЧИСТО

Диф против `Allowed paths` §3 спеки и таблицы запретов §3:

```
$ git diff --name-only 675d8b6 9899cf8 | grep -E 'crates/(contracts|venue-|journal|book|risk|killswitch|oms)/'
НЕТ — ни одного
$ git diff --name-only 675d8b6 9899cf8 | grep -E 'TECH-DEBT|PROJECT-STATE|09-roadmap'
НЕТ
$ git diff 675d8b6 9899cf8 -- docker-compose.yml | grep -c 'GATEWAY_BANDS'
0
```

- **`crates/contracts/**` не тронут ⇒ Block-C не применяется** (`gates.md` §4). T1 предмет не
  меняет: форма события та же, читаем то, что уже пишется (`CT-I-2`).
- **`docker-compose.yml`** — в §3 запрещена строка `GATEWAY_BANDS` (состав ВЫДАЧИ, граница C).
  Тронута она НЕ была: 0 вхождений в дифе. Все 19 добавленных строк — ручка каденции
  (`GATEWAY_DEPTH_CADENCE_MS` + `--depth-cadence-ms`) у обеих служб, задачи 22/23.
- **`docs/fa/viz-backend.md` в дифе ОТСУТСТВУЕТ** — и это правильно: §3 спеки прямо выводит его
  из зон (`C-094` B4(4)). Последствие разбираю отдельно ниже (условия 6/8).

## Block-risk — НЕ ПРИМЕНЯЕТСЯ, и это предъявлено, а не предположено

`gates.md` §5 привязывает RISK-BLOCK к путям `crates/risk|killswitch|oms|venue-*|contracts`.
Греп выше даёт по ним НОЛЬ файлов на ВСЁМ диапазоне PR, а не только в последних коммитах.
Предмет — read-only консюмер журнала (`VB-I-3`), ордер-пути не касается ни одной строкой.
Вердикт `risk-critic` для этого PR не требуется; отсутствие его в цепочке блокером не является.

## Block-commits — атомарность и RED-first

Новая работа круга — три коммита architect'а поверх `ebbd765`:

| коммит | предмет | численно |
|---|---|---|
| `c1f6649` | токен subject-lock + `N-2`(половина) + `N-3` + `N-14` | 3 файла, +24/−5 |
| `a136be9` | три токена `TERMINAL-BRANCH-VERDICT` | 0 файлов (только тело) |
| `9899cf8` | `N-2` вторая половина | 1 файл, +23/−2 |

`c1f6649` бандлит четыре находки одного вердикта. Формально это не «пять задач одним
коммитом», за которые `commit-discipline.md` предписывает авто-reject: находки гейта — не
§Tasks, subject перечисляет их поимённо, и тело даёт основание по каждой. **Принимаю, но
называю MINOR-нотой** (`N-1` ниже) — при откате пришлось бы разбирать четыре предмета вместе.

**Тесты не переписаны под реализацию.** Проверено механически, а не чтением: во ВСЕХ правках
этого круга, попавших в `crates/**`, изменённые строки — ИСКЛЮЧИТЕЛЬНО комментарии.

```
$ git show 9899cf8 -- crates/gateway/src/bin/gateway-checkpoint.rs | grep -E '^[+-][^+-]' | grep -vE '^[+-]\s*//'
НЕТ не-комментарных строк
$ git show c1f6649 -- crates/gateway/tests/ | grep -E '^[+-][^+-]' | grep -vE '^[+-]\s*(//|///)'
НЕТ — только комментарии
```

Ни одного ассерта, ни одного литерала порога, ни одной ветки поведения. Поэтому мутационный
контроль на дельту ЭТОГО круга не ставится: нейтрализовать в комментарии нечего, а мутация
кода, который круг не трогал, повторяла бы `MUT-N1`/`MUT-CLOSE`/`MUT-ORDER` из `R-154`.
Говорю это явно, чтобы «мутации не было» не читалось как пропуск процедуры.

---

## Условия `R-154` — что закрыто, ЗАМЕРОМ

`R-154` нёс **восемь** условий: пять в §Условие APPROVED и ещё три (пп. 6-8), добавленные
Приложением 2 того же вердикта. Мандат круга назвал только первые пять. Сужать собственный
гейт по чужой выписке я не вправе — сужу все восемь.

### Условие 1 — токен subject-lock. **ЗАКРЫТО.**

`c1f6649` несёт `ALLOW-SUBJECT-CHANGE`, называющий, чем `scripts/verify_M-68.sh` вырос после
проходных `A-023`/`A-024`/`A-025`/`R-130`: пять целых step-блоков (`C2`, `C3`, `C3bis`,
`C3ter`, `C4`), каждый со своим вердиктом-основанием, и замер прироста по каждой ревизии.
Существенно, что лок сработал на РОСТЕ защиты: ни одна проверка не снята. Токен — аудит-след,
не доказательство (`gates.md` §11), и он себя таковым называет.

### Условие 2 — судьба барьера. **ЗАКРЫТО АРБИТРАЖЕМ + МЕХАНИЗМОМ В `main`.**

`A-027` (DECISION, арбитр, свежий контекст) принял (1)/(2) и (3) с одной достройкой; барьер
расширен, PR #125 влит — `675d8b6` есть tip `origin/main`. Проверил СВОИМ прогоном в
**прод-форме вызова** (та, что в `ci.yml:358-363`), включая обязательное дотягивание спас-рефов:

```
$ git fetch --no-tags origin '+refs/salvage/*:refs/salvage/*'     # 78 рефов
$ EVENT_NAME=pull_request PR_BASE_SHA=675d8b6c… bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 20, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
exit=0
```

Все семь прежних `FAIL` стали `NOTE`: четыре subject-lock открыты `ALLOW-SUBJECT-CHANGE`, три
`audited_head` терминальной ветки — `TERMINAL-BRANCH-VERDICT` из `a136be9`. **Блокер `Б-1`
вердикта `R-154` снят замером, а не декларацией.**

### Условие 3 (`N-2`) — обе половины. **ЗАКРЫТО, И МОЙ ЖЕ ВЕРДИКТ ПОПРАВЛЕН ПО СУЩЕСТВУ.**

Половина A — docstring setup-guard'а `d18g` (`c1f6649`); половина B — комментарий
`gateway-checkpoint.rs:277-278` (`9899cf8`). **Замер переснял сам**, прод-форма argv взята из
`docker-compose.yml:207-226` (`--dir`/`--ckpt-dir`/`--coverage-out`/…/`--cursor=LATEST`), а не
по памяти:

```
A) флаг 2000 + env abc  : exit=2      C) без флага + env abc  : exit=2
B) флаг 2000 + env 4000 : exit=0      D) без флага + env 4000 : exit=0
```

Setup-guard: exit=2 — это ИМЕННО отказ разбора каденции, а не посторонний сбой; бинарь
печатает `GATEWAY_DEPTH_CADENCE_MS="abc" не парсится как i64 … оператор обязан задать валидное
значение`. То есть проба судит тот сценарий, который обещает.

**Принимаю поправку architect'а к `R-154`.** Мой вердикт приводил A/B/C и читал `B` как «флаг
выиграл». Прогон `D` (я его повторил) показывает: `B≡D` ровно так же, как `A≡C`, — значит из
кода возврата «какое значение победило» не наблюдаемо НИ В ОДНУ сторону, и моя формулировка
опиралась на различие, которого замер не даёт. Вывод `N-2` при этом устоял: он держится на
паре `A≡C`, и именно она названа решающей в новом комментарии. Гейт, не признающий поправку
к себе, требует от других того, чего не делает сам.

### Условие 4 (`N-3`) — исполнитель правки FA. **ЗАКРЫТО.**

`M-68:841` переадресован с **reviewer'а** (которому `docs/**` закрыт — `scope-guard.md`,
таблица владения) на **architect'а**, с маршрутом «свой круг критика по `gates.md` §9».
Норма, неисполнимая по построению, была снята — тот же класс, что `R-104` Б-1.

### Условие 5 — прогон на дереве слияния. **ИСПОЛНЕН МНОЙ, не принят на слово.**

Сначала снял топологию, потому что от неё зависит, что вообще есть «дерево слияния»:

```
$ git merge-base --is-ancestor 675d8b6 9899cf8 → YES
```

`origin/main` — ПРЕДОК вершины, то есть merge fast-forward и **дерево слияния ≡ `9899cf8`**.
Это заодно закрывает риск `strict: false` из `gates.md` §8 (зелёный чек на устаревшей базе):
базы устаревшей здесь нет — ветка вобрала `main` коммитом `8ec6754`. Прогнано на нём:
базовая тройка CI, `verify_M-68.sh`, `check_gate_meta.sh` в прод-форме,
`verify_design_claims.sh --merge-preview`, четыре харнесс-барьера. Все — в Done Block.

### Условие 7 (`N-14`) — числа док-комментария. **ЗАКРЫТО; проверено АРИФМЕТИКОЙ, независимо от прогона.**

Фикстура: `EVENTS=120`, `EVENT_STEP_MS=1000` ⇒ журнал покрывает `t0..t0+119s`. Закрытых
интервалов: `floor(119/1)=119`, `floor(119/10)=11`, `floor(119/60)=1`. Докстринг теперь
заявляет **119/11/1** и называет причину (последний интервал не закрыт, незакрытый не
эмитится — законно по `R-141` `Б-2`). Прежние 120/12/2 недостижимы ни при каком исходе.

### Условия 6 (`N-13`) и 8 (`N-9`) — НЕ в этом PR. **МОЁ УСЛОВИЕ БЫЛО НЕИСПОЛНИМО ПО ПОСТРОЕНИЮ; ПЕРЕФОРМУЛИРУЮ.**

Это единственный пункт круга, где я меняю собственное требование, поэтому разбираю его целиком.

**Факт.** `docs/fa/viz-backend.md` и `docs/DESIGN.md` в дифе PR #124 отсутствуют. Носитель
правки существует и запушен: `origin/docs/fa-viz-M-68-close`, коммит `3a88f90`
(`docs/DESIGN.md` +1, `docs/fa/viz-backend.md` +44/−7, план аудита +264). PR на него НЕ открыт,
круга критика по `gates.md` §9 он НЕ проходил.

**Почему требовать закрытия ДО merge'а нельзя — два запертых выхода, оба замерены.**

1. *Внести правку внутрь PR #124* — прямой `SCOPE VIOLATION`: §3 спеки выводит
   `docs/fa/viz-backend.md` из `Allowed paths` явным решением `C-094` B4(4). Я же обязан был
   бы это и зареджектить в Block-scope.
2. *Влить правку ДО M-68* — сделало бы FA ЛОЖНОЙ, то есть внесло бы ровно тот дефект, против
   которого написано `N-13`. Замер:
   ```
   main:   pub const GATEWAY_SCHEMA_VERSION: u32 = 8;
   branch: pub const GATEWAY_SCHEMA_VERSION: u32 = 9;
   ```
   Правка дописывает в §5 FA бамп `8→9` и снимает предусловие (б) §4 («депт-серия
   пересчитывается ТОЛЬКО в ветке `L2Snapshot`»). На базе без M-68 схема равна 8, а ветка
   `L2Delta` серию действительно не трогает — оба новых утверждения были бы неправдой.

Значит легальный порядок ровно один: **M-68 → потом FA своим кругом**. Моё условие «закрыть до
APPROVED» относилось к порядку, которого не существует. Это тот же класс, который я сам нашёл
кругом раньше как `N-3`, — норма, исполнимая только через нарушение другой нормы. Признаю за
собой и переформулирую (пункт «Остаток» ниже), а не тащу предмет на восьмой круг ради
требования, которое сам сделал невыполнимым.

**Чего эта переформулировка НЕ лечит, и я это не смягчаю.** Между merge'ем #124 и приземлением
`3a88f90` `docs/fa/viz-backend.md` в `main` содержит два ложных утверждения. Окно реально, и
машинного наблюдателя у него НЕТ: `verify_design_claims.sh --merge-preview origin/main` даёт
`VERDICT: PASS (0 нарушений)` — предусловие (б) написано прозой, под маркер `FACTS:` не
подпадает и барьером не судится. Поэтому окно уезжает **карточкой долга `TD-185`**, а не
устным обещанием, и M-68 close-out'ом не закрывается, пока карточка открыта.

---

## Находки этого круга

### `N-1` (MINOR) — четыре находки одним коммитом

`c1f6649` закрывает `Б-1`(класс ii), `N-2`(половина), `N-3` и `N-14` в одном коммите.
`commit-discipline.md` требует атомарности по §Tasks; находки вердикта под неё формально не
подпадают, а subject перечисляет их поимённо. Не блокер, но откат любой одной из четырёх
потребовал бы ручной разборки. На будущее: находка вердикта — такой же атом, как задача.

### `N-2` (NOTE) — architect писал в `crates/*/src/`, и назначил это ему МОЙ ЖЕ вердикт

`9899cf8` правит `crates/gateway/src/bin/gateway-checkpoint.rs` — путь, закрытый для architect'а
таблицей `scope-guard.md` без оговорок про комментарии. Обстоятельства смягчающие и они
фактические: правка комментарная (доказано выше механически), поведение не задето, а
исполнителем её назначил `R-154` `N-2` — то есть я. `c1f6649` при этом честно переадресовывал
эту половину engine-dev'у, и architect в теле `9899cf8` НАЗВАЛ расхождение вместо того, чтобы
его замолчать.

Дефект здесь мой, а не его: **PR-гейт не вправе поручать роли работу в зоне, которую сам же
обязан у неё реджектить.** Не блокирую. Класс — «норма, исполнимая только через нарушение
другой нормы», третье его срабатывание в этом милестоуне (`N-3` круга 6, условия 6/8 выше,
эта нота). Уезжает карточкой **`TD-186`**: у `scope-guard.md` нет ответа на вопрос, кто правит
КОММЕНТАРИЙ в чужой зоне, и каждый раз это решается вручную.

### Что проверено и дефекта НЕ найдено — называю явно

- `docker-compose.yml`: проводка каденции объявлена у ОБЕИХ служб одной переменной
  (`:226`/`:231` у писателя, `environment` у `gateway-serve`), расхождение отпечатка селектора
  недостижимо, дефолт `1000` совпадает с прод-таймфреймом ⇒ поведение прода не меняется.
- `GATEWAY_BANDS` не тронут — состав выдачи остаётся за `П-014`/M-70, граница C не задета.
- `verify_design_claims.sh --merge-preview origin/main` — `PASS (0 нарушений)`, включая
  `[H-FACTS-SHA]` (7 маркеров) и `[4-МЁРТВЫЕ-ФАЙЛЫ]` (316 ссылок).
- Работа кругов 1-6 по существу не переоткрывается (`R-154`: «принята и переоткрытию не
  подлежит»); этот круг судит ТОЛЬКО закрытие условий.

---

## ВЕРДИКТ: **APPROVED**

Восемь условий `R-154`: **шесть закрыты замером** (1, 2, 3, 4, 5, 7), **два (6, 8)
переформулированы** — их прежняя редакция требовала порядка приземления, которого не
существует, и это дефект моего вердикта, а не работы цепочки. Блокер `Б-1` (красный
`check_gate_meta`, державший merge шесть кругов) снят: мой прогон барьера в прод-форме на
дереве слияния даёт `PASS`, `exit=0`, и агрегат `All checks passed` на PR — `pass`.
Block-scope чист, Block-C не применяется, RISK-BLOCK не применяется — всё три предъявлены
грепом по диапазону, а не мнением.

**Merge выполняю** через PR (`gates.md` §8): прямой push в `main` закрыт защитой ветки.

**Остаток — с названными носителями и исполнителями, не «в рабочем порядке»:**

1. **`docs/fa-viz-M-68-close` (`3a88f90`) вливается СЛЕДУЮЩИМ**, своим PR, через круг критика
   по `gates.md` §9 (правка меняет ФОРМУ документа: снимается предусловие §4, дописывается
   бамп `8→9` в §5, определяется семейство `MD-I` в `DESIGN` §22). Исполнитель — architect,
   гейт — critic. Пока не влито — `TD-185` OPEN, и M-68 НЕ закрывается.
2. **`TD-185`** (MAJOR) — окно ложности FA между merge'ем M-68 и приземлением п.1;
   машинного наблюдателя у него нет, названо явно.
3. **`TD-186`** (MINOR) — `scope-guard.md` не отвечает, кто правит комментарий в чужой зоне.
4. **`TD-187`** (MAJOR) — `ci-aggregate-continue-on-error`, текст карточки продиктован
   `A-027` §6 (номер выдаёт механизм — `gates.md` §12, `П-022`).
5. Карточки из `R-154`, заводимые этим же close-out'ом: уточнение `TD-168` замером
   `MUT-CLOSE`, непокрытие порядка «гвард ДО сеттера» (`MUT-ORDER`), двойная материализация
   книги (`П2.8`), отсутствие сверки `gateway`↔`book::depth_within` (`N-11`), расхождение
   `fa/book.md` §7 microprice↔mid (`N-12`).

`PROJECT-STATE.md` и `TECH-DEBT.md` обновляю ПОСЛЕ merge'а, в close-out'е — обе зоны мои.

---

## Done Block (сырой stdout)

```text
$ pwd
/tmp/hft-reviewer-M-68
$ git rev-parse HEAD
9899cf853bc64cb3f98f6dfc83e187e42a2cb088
$ git status --porcelain
?? research/reviews/R-156-M-68-round7-conditions-closure.md      (этот файл)

--- ТОПОЛОГИЯ: что есть «дерево слияния» ---
$ git merge-base origin/main 9899cf8
675d8b6caa96fe0d3ee40b70078f648ecbbca9be
$ git log -1 --format='%H' origin/main
675d8b6caa96fe0d3ee40b70078f648ecbbca9be
$ git merge-base --is-ancestor 675d8b6 9899cf8; echo $?
0            ← main есть ПРЕДОК вершины ⇒ merge fast-forward ⇒ дерево слияния ≡ 9899cf8

--- БАЗОВАЯ ТРОЙКА CI (на дереве слияния) ---
$ cargo fmt --all -- --check; echo FMT_EXIT=$?
FMT_EXIT=0
$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5; echo CLIPPY_EXIT=${PIPESTATUS[0]}
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.64s
CLIPPY_EXIT=0

--- ACCEPTANCE (он же гонит cargo test --all целиком — паритет с CI, gates.md §3) ---
$ bash scripts/verify_M-68.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo VERIFY_EXIT=${PIPESTATUS[0]}
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
PASS: cargo test -p gateway --test red_depth_from_book --quiet
PASS: A состав набора — 9 оракулов (ожидалось ровно 9: d1 d2 d3 d4 d5 d7 d7b d8 d8b)
   … (шаги B/C2/C3/C3bis/C3ter/C4 — мутация C-M68-1 ИСПОЛНЯЕТСЯ, не грепается) …
PASS: C4 ложное самоописание снято — ложное «как прежний depth_within с None mid» (lib.rs:1134-1136)
PASS: D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)
PASS: cargo test -p gateway --test red_gateway_schema_version --quiet
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
PASS: cargo test -p gateway --test red_depth_provenance_by_reach --quiet
PASS: H crates/contracts не тронут
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут (судятся только изменённые строки)
PASS: J selector_fingerprint не переписан
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: PASS
VERIFY_EXIT=0
$ grep -c '^PASS' <вывод выше>   → 29        $ grep -c '^FAIL' → 0

$ cargo test --all 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=948 failed=0 (блоков: 221)
[exited with code 0]

МЕТОД-ЗАМЕТКА ПРОТИВ СЕБЯ: этот агрегат я сначала запустил ПАРАЛЛЕЛЬНО с acceptance-гейтом в
ОДНОМ дереве — оба зовут `cargo test --all`, делят один `target/` и встали в очередь на
блокировку cargo, а рядом молотили сборки соседних агентов (load ~12). Свой дубль снял,
гейт добежал, агрегат перегнал следом. Ошибка моя и стоила ~25 минут; записываю, потому что
«прогон завис» здесь имело причину в моей же организации работы, а не в предмете.

--- БАРЬЕРЫ ХАРНЕССА (прод-форма вызова, EVENT_NAME=pull_request) ---
$ git fetch --no-tags origin '+refs/salvage/*:refs/salvage/*'      # 78 спас-рефов
$ EVENT_NAME=pull_request PR_BASE_SHA=675d8b6c… bash scripts/check_gate_meta.sh
── GATE-META: диапазон 675d8b6c..HEAD, origin=a3ka/hft-platform
NOTE  A-018 / C-094 / C-138: audited_head на ТЕРМИНАЛЬНОЙ ветке — открыто TERMINAL-BRANCH-VERDICT в a136be91
NOTE  A-023 / A-024 / A-025 / R-130: subject-lock открыт ALLOW-SUBJECT-CHANGE: scripts/verify_M-68.sh
OK    merge 7ed83f18 … вердикт R-095 в дереве слияния
OK    merge c3c4537f … вердикт R-095 в дереве слияния
VERDICT: PASS — вердиктов проверено: 20, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
GATE_META_EXIT=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [3-ССЫЛКИ] все 7 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 316 ссылок вида docs/*.md указывают на существующие файлы
PASS  [H-FACTS-SHA] маркеров `FACTS:` проверено 7 — все ревизии существуют и входят в историю
PASS  [6-RFC-SHA] всего=38 проверено=38 пропущено=0
VERDICT: PASS (0 нарушений)
DESIGN_CLAIMS_EXIT=0

$ for s in check_docs_freeze check_protected_artifacts check_artifact_ids check_review_fa; do … done
check_docs_freeze            exit=0
check_protected_artifacts    exit=0
check_artifact_ids           exit=0
check_review_fa              exit=0

--- ЧЕКИ PR #124 (решение по КОДУ ВОЗВРАТА, gates.md §8) ---
НА МОМЕНТ СТАРТА КРУГА:
  mergeStateStatus: BLOCKED
  fmt + clippy + test          pending  0
  fmt + clippy + test (ветка)  pending  0
  агрегат «All checks passed» — ОТСУТСТВОВАЛ
$ gh pr checks 124 --watch >/tmp/rev-checks124.txt 2>&1; echo CHECKS_EXIT=$?
CHECKS_EXIT=0
  Artifact IDs … pass          Delivery gate … pass        Protected artifacts … pass
  Branch health … pass         Deploy catch-up … pass      Reserve IDs … pass
  Context budgets … pass       Design claims … pass        Resource oracles … pass
  Contracts gate … pass        Docs-freeze … pass          Review FA … pass
  GATE-META … pass             cargo audit … pass
  fmt + clippy + test          pass  7m20s
  fmt + clippy + test (ветка)  pass  9m43s
  All checks passed            pass  5s          ← агрегат, входящий в защиту ветки

--- ЗАМЕР `N-2` (мой, прод-форма argv из docker-compose.yml:207-226) ---
A) флаг 2000 + env abc  : exit=2      C) без флага + env abc  : exit=2
B) флаг 2000 + env 4000 : exit=0      D) без флага + env 4000 : exit=0
SETUP-GUARD (доказательство, что exit=2 — это ИМЕННО отказ разбора каденции):
gateway-checkpoint: GATEWAY_DEPTH_CADENCE_MS="abc" не парсится как i64 (invalid digit found
in string) — опечатка в `.env` …; оператор обязан задать валидное значение или unset/пусто/
пробельное для дефолта

--- КОММЕНТАРНОСТЬ ПРАВОК В crates/** (механически, не чтением) ---
$ git show 9899cf8 -- crates/gateway/src/bin/gateway-checkpoint.rs \
    | grep -E '^[+-][^+-]' | grep -vE '^[+-]\s*//'
(пусто)
$ git show c1f6649 -- crates/gateway/tests/ | grep -E '^[+-][^+-]' | grep -vE '^[+-]\s*(//|///)'
(пусто)

--- BLOCK-SCOPE (греп по ВСЕМУ диапазону PR, не по последним коммитам) ---
$ git diff --name-only 675d8b6 9899cf8 | grep -E 'crates/(contracts|venue-|journal|book|risk|killswitch|oms)/'
(пусто — ни одного)
$ git diff --name-only 675d8b6 9899cf8 | grep -E 'TECH-DEBT|PROJECT-STATE|09-roadmap'
(пусто)
$ git diff 675d8b6 9899cf8 -- docker-compose.yml | grep -c 'GATEWAY_BANDS'
0

--- УСЛОВИЯ 6/8: замер, доказывающий, что порядок заперт ---
main:   pub const GATEWAY_SCHEMA_VERSION: u32 = 8;
branch: pub const GATEWAY_SCHEMA_VERSION: u32 = 9;
$ git show origin/main:docs/DESIGN.md | grep -c 'MD-I'
0
$ git ls-remote origin 'refs/heads/docs/fa-viz-M-68-close'
3a88f908816d2ebad7fd8c21ca1fe17a9cc0ae83
$ gh pr list --state all --head docs/fa-viz-M-68-close --json number
[]        ← носитель готов и запушен, PR не открыт, круга критика §9 не проходил

--- N-14: числа докстринга проверены АРИФМЕТИКОЙ, независимо от прогона ---
EVENTS=120, шаг 1000 мс ⇒ журнал покрывает t0..t0+119 c
floor(119/1)=119   floor(119/10)=11   floor(119/60)=1     ← докстринг заявляет 119/11/1
прежние 120/12/2 недостижимы ни при каком исходе

--- TD-158: обе половины закрыты, проверено в коде ---
$ grep -n 'депт-серия остаётся snapshot-only' crates/gateway/src/lib.rs
(строки больше нет — ветка L2Delta теперь пересчитывает серию)
$ grep -n 'cadence_ms' crates/gateway/src/lib.rs | grep 'pub '
388:    pub cadence_ms: Vec<(String, Option<i64>)>,      ← последнее поле SeriesBundle (325..389)

--- ПРОД ДО MERGE'А (базовая линия для §8) ---
$ ssh …@167.233.192.131 'docker ps --format "{{.Names}} {{.Status}}"; cat …/recorder.heartbeat'
hft-gateway-serve Up 39 hours (healthy)
hft-recorder Up 39 hours (healthy)
{"events":9671593,"free_bytes":58962038784,"next_seq":407554058,"segment_index":475,
 "ts_wall_ms":1788088743229,"writable":true}
$ date -u +%s → 1788088752        ← heartbeat свежий, отставание 9 секунд
```

## Cross-references

- `research/reviews/R-154-M-68-merge-resolution-round6.md` — предыдущий круг, восемь условий
- `research/arbitration/A-027-gate-meta-refspec-observer.md` — DECISION по условию 2 + текст `TD-187` (§6)
- `research/arbitration/A-025-m68-terminal-route.md` §5.5 — маршрут M-68 (круга критика нет)
- `milestones/M-68-depth-from-book.md` §3/§3.1 — Allowed paths и запретный список
- `docs/fa/viz-backend.md` — `VB-I-2`, `VB-I-10` (названы выше), предмет `TD-185`
- `.claude/rules/gates.md` §3/§4/§5/§8/§9/§11/§12 · `.claude/rules/commit-discipline.md`
