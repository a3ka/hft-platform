<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: c49fbc46da10acb3089ed0ad45ccb0785556d7e1
audited_head: 4a575f2ee3d586744c0d768d6d6a90e7ffde8ea9
verdict: REJECT
-->

# R-186 — M-70 (полосы глубины), PR-гейт круга 5: **REJECTED (merge механически невозможен)**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-09-18T15:05Z
**Предмет:** `c49fbc4..4a575f2` на `origin/docs/M-70-rev2` — всё, что легло поверх ревизии,
судимой `R-184`.
**Мандат:** вердикт tester'а PASS (7/7 команд, `verify_M-70.sh` exit=0, 994/994, рендер
`GATEWAY_BANDS: "0.001"`).
**Предыдущие круги:** `R-170` (маршрут), `R-171` (REJECT: форма ломала `VB-I-4`), `R-172`
(REJECT: `DB-I-4c` не сторожит эвикцию), `R-184` (REJECT: merge = включение полос без подписи;
rev10/rev11 без круга критика).
**Моё дерево:** `/tmp/hft-rev-m70-r5`, detached `4a575f2`, своя сборка (чужой `target/` не
переиспользован). Дерево слияния — `/tmp/hft-rev-m70-mp`, `9697400`.

**Главное одной строкой.** **ОБА блокера `R-184` закрыты, и каждый предъявлен МОИМ замером, а
не отчётом** — включая два мутационных контроля. Инженерных претензий к работе у меня НЕТ.
**Merge всё равно отказан, и снова ни одна причина не является дефектом кода:** на ДЕРЕВЕ
СЛИЯНИЯ два барьера из агрегата `All checks passed` КРАСНЫ (`Б-1`, `Б-2`), то есть branch
protection физически не пропустит PR. Оба — ложные срабатывания по существу и настоящие по
механике; чинятся в зоне architect'а, не dev'а.

---

## Block-scope — PASS

Разделение зон проверено ПОКОММИТНО по собственному диапазону ветки (`--not origin/main`), а
не по итоговому диффу:

```
$ for c in $(git log 4a575f2 --format='%h' --not origin/main); do ... git show --numstat ...
4a575f2 [architect]   milestones/M-70-depth-bands-enablement.md            20/1
c0e744d [critic]      research/critiques/C-228-M-70-c227-closure.md       108/0
2ec574d [architect]   scripts/verify_M-70.sh                              51/15
812c309 [critic]      research/critiques/C-227-M-70-c226-closure.md       127/0
0a42885 [architect]   milestones/M-70-depth-bands-enablement.md            28/6
9ff366a [architect]   scripts/verify_M-70.sh                              16/14
1adb7ac [critic]      research/critiques/C-226-M-70-rev10-rev12.md        186/0
e6f2e73 [architect]   milestones/M-70-depth-bands-enablement.md            83/53
```

**Ни один коммит круга 5 не трогает `crates/**` вообще** — ни `src/`, ни `tests/`. Круг целиком
процессно-гейтовый: спека, шаг гейта, три вердикта критика. `scripts/verify_M-70.sh` — зона
architect'а (`scope-guard.md`), и её правит architect, а не dev. Чистый диф предмета против
`main` остаётся ровно `Allowed paths` спеки §2 — новых путей круг не добавил.

## Block-DoneBlock — PASS. Числа отчёта совпали с прогоном

`R-172` Б-3 убил круг 3 за `exit=0` в отчёте при `exit=1` у гейта. Класс не повторился:

```
$ bash scripts/verify_M-70.sh > /tmp/r5-verify.out 2>&1; echo "VERIFY_EXIT=$?"
VERIFY_EXIT=0
$ grep -c '^PASS' /tmp/r5-verify.out; grep -c '^FAIL' /tmp/r5-verify.out
42
0
$ tail -1 /tmp/r5-verify.out
VERDICT: PASS
```

Отличие от прогона tester'а снято: он гонял со своим `target/`, я собирал своё дерево с нуля.
Числа сошлись поимённо.

## Block-C — N/A, предъявлено шагом гейта

`crates/contracts/**` и `docs/rfc/**` не тронуты; contract-RFC не требуется. Шаг `C` гейта —
`PASS` (сырая строка в `/tmp/r5-verify.out`).

## Block-risk — N/A по `gates.md` §5

Диф не трогает `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**`.
Предмет — read-only выдача, order-egress отсутствует как класс. Состав ЗАПИСИ не тронут — это
тоже шаг гейта, а не моё слово. `risk-critic` не требуется.

## Предъявление FA (`gates.md` §4, M-66) — живые инварианты на ЭТОЙ ревизии

Диф ветки трогает `crates/gateway/**` и `crates/gateway-serve/tests/**`; FA обоих —
`docs/fa/viz-backend.md` (`check_review_fa.sh:190-199`: `gateway`→`VB`, `gateway-serve`→`GS`).
Названы ID, ЖИВЫЕ на `4a575f2`, а не по памяти:

- **`VB-I-5`** (`docs/fa/viz-backend.md:203`) — метка достоверности принадлежит ТОЧКЕ, а не
  ряду; форма v10, `series_provenance` снимается ТЕМ ЖЕ наблюдением книги, что `series[i]`.
  Это инвариант, который задача 4 исполняет, а `DB-I-4d` сторожит.
- **`VB-I-10`** (`:208`) — bounded-window snapshot: предел выдачи не куплен ценой предела
  памяти. Шаг `task #8` исполнил существующие оракулы окна (2 теста, `PASS`).
- **`VB-I-4`** (`:202`) — аддитивность формы: v1-потребитель не ломается, смена формы ⇒ bump
  `GATEWAY_SCHEMA_VERSION` 9→10. Обе половины исполнены (задачи 6/6b).
- **`GS-I-1`** — `gateway-serve` остаётся тонкой read-only IO-оболочкой: диф в крейте
  ограничен тестом доставки, прод-код транспорта не тронут.

**Ярус C предъявляю грепом** (`reading-map.md` §2) — я эти файлы пишу, поэтому называю, что
искал: `TD-159` (`TECH-DEBT.md:107`, OPEN), `TD-161` (`:108`, OPEN), `TD-198` (`:88`, OPEN),
`TD-199` (`:140`, закрыт), `TD-204` (`:144`, закрыт), `TD-205` (`:145`, OPEN); `M-70` в
`docs/ROADMAP.md`. `PROJECT-STATE.md` целиком не читал и не утверждаю, что читал.

---

# Закрытие находок `R-184` — обе предъявлены МОИМ замером

## `R-184` Б-1 (граница C) — **ЗАКРЫТ вариантом B, и это ЗАМЕР, а не согласие с доводом**

`R-184` установил: merge ⇒ Deploy ⇒ семь полос включены пользователям одним действием, потому
что прод берёт дефолт из compose. Развязка B («дефолт вернуть к `0.001`») исполнена
(`705c0aa`). **Я проверил не текст, а РЕНДЕР — то, что увидит прод:**

```
$ env -u GATEWAY_BANDS GATEWAY_JWT_SECRET=dummy docker compose config | grep GATEWAY_BANDS
      GATEWAY_BANDS: "0.001"
$ git show origin/main:docker-compose.yml | grep -n 'GATEWAY_BANDS:'
136:      GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001}
```

Дефолт ветки и дефолт `main` СОВПАДАЮТ. И состояние прода — ssh, а не память:

```
$ ssh … 'docker inspect hft-gateway-serve --format "{{range .Config.Env}}{{println .}}{{end}}" | grep -i bands'
GATEWAY_BANDS=0.001
$ ssh … 'grep -n GATEWAY_BANDS .env || echo "(нет в .env)"'
(нет в .env)
```

**Следствие, и оно решающее:** merge НЕ меняет состав полос, выдаваемых пользователям.
Граница C этим merge'ем не пересекается, подписи на момент не требуется. Довод `R-185` §3 я не
принимал на слово — он подтверждён тем, что дефолт по обе стороны merge'а один и тот же.

**Страж отложенного решения — НЕ плацебо, проверено мутацией.** Гейт обязан краснеть на флипе
дефолта, иначе вариант B держится на вежливости. Воспроизвёл обход, которым `C-227` убил
вторую редакцию предиката:

```
$ # docker-compose.yml: GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001},0.02
$ env -u GATEWAY_BANDS … docker compose config | grep GATEWAY_BANDS
      GATEWAY_BANDS: 0.001,0.02
$ bash scripts/verify_M-70.sh; echo exit=$?
FAIL: task #7 — ОТРЕНДЕРЕННЫЙ прод-дефолт НЕ РОВНО узкий: '0.001,0.02', требуется ровно '0.001'
VERDICT: FAIL (2)
exit=1
$ # код возвращён:
$ git status --porcelain
{пусто}
```

**Второй мутационный контроль — композиция писателя и читателя чекпоинта.** Шаг `task #7`
судит ТОЛЬКО блок `gateway-serve`; блок `gateway-checkpoint` живёт под профилем `ops` и в
`docker compose config` не рендерится вовсе — то есть предикатом НЕ покрыт. Комментарий гейта
утверждает, что связку держит поведенческий оракул `c3ter`. **Я это утверждение проверил
исполнением, а не прочтением:**

```
$ # docker-compose.yml: - --bands=${GATEWAY_BANDS:-0.02}   (расходится ТОЛЬКО чекпоинтер)
$ cargo test -p gateway --test red_checkpoint_bin_prod_argv 2>&1 | tail -3
failures:
    c3ter_writer_and_reader_agree_on_checkpoint
test result: FAILED. 7 passed; 1 failed
```

Оракул выводит селектор читателя ИЗ `environment:`-блока `gateway-serve`
(`red_checkpoint_bin_prod_argv.rs:464-494`) и гоняет настоящий бинарь с argv из `command:`.
Дыра, которую я предполагал, закрыта — но закрыта ПОВЕДЕНИЕМ, а не грепом, и это верно.

## `R-184` Б-2 (круг критика по rev10/rev11) — **ИСПОЛНЕН**

`C-226` судит ТРИ ревизии одним предметом и называет их поимённо: `af16be5` (rev10),
`c8e9f4a` (rev11), `e6f2e73` (rev12) — то есть требование `R-184` («предмет круга — обе
ревизии вместе, не последняя») выполнено буквально. Цепочка: `C-226` REJECT → `C-227` REJECT →
`C-228` NOTE, все три закоммичены на ветку как файлы. Предмет закрыт.

---

# Блокеры круга 5 — оба МЕХАНИЧЕСКИЕ, оба вне кода

## Б-1 — БЛОКЕР. `protected-artifacts` КРАСЕН на дереве слияния ⇒ агрегат красен

```
$ cd /tmp/hft-rev-m70-mp && git merge --no-edit origin/main     # дерево слияния, 9697400
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) \
    bash scripts/check_protected_artifacts.sh; echo exit=$?
FAIL  milestones/M-75-heatmap-window-decoupling.md: артефакт ИСЧЕЗ с HEAD, и ни один коммит
      его не удалял (значит его выбросил MERGE — evil merge / -s ours / rename внутри мержа)
exit=1
```

**По существу утраты НЕТ, и я это проверил, а не предположил:**

```
$ git cat-file -e origin/main:milestones/M-75-heatmap-window-decoupling.md; echo $?   → 1 (нет и в main)
$ git ls-tree -r --name-only origin/main -- docs/archive | grep M-75
docs/archive/M-75-heatmap-window-decoupling.md
docs/archive/verify_M-75.sh
$ git ls-tree -r --name-only 4a575f2 -- docs/archive | grep M-75      → те же два файла
```

`M-75` закрыт и ПЕРЕЕХАЛ в `docs/archive/` штатным close-out'ом (`70ca5c8`) — по обе стороны
merge'а файлы на месте. Барьер видит исчезновение ПУТИ `milestones/M-75-*` через merge и не
умеет следовать за переездом.

**Но merge от этого не становится возможным.** Джоб `protected-artifacts` входит в `needs:`
агрегата (`ci.yml:473` блок `All checks passed`), защита `main` требует зелёный агрегат
(`gates.md` §8), значит PR физически не сольётся. Ложность срабатывания — не право его
обойти.

**Развязки, обе в зоне architect'а (`scripts/**` — sacred), выбор не мой:** (а) `ALLOW-ARTIFACT-DELETE: <причина>`
в теле коммита диапазона — аудит-след, честно называющий, что переезд осознан; (б) научить
барьер следовать переезду `milestones/* → docs/archive/*`, поскольку close-out ИМЕННО это и
делает штатно, и класс повторится на каждом следующем milestone'е.

## Б-2 — БЛОКЕР. `gate-meta` КРАСЕН на дереве слияния: subject-lock от `R-185`

```
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) \
    bash scripts/check_gate_meta.sh; echo exit=$?
FAIL  research/reviews/R-185-m70-enablement-second-opinion.md: subject-lock — после
      проходного вердикта (NOTE) тронут класс «гейт»: scripts/verify_M-70.sh
VERDICT: FAIL (1)
exit=1
```

Механика (`check_gate_meta.sh:516-532`): вердикт с ПРОХОДНЫМ исходом запирает класс «гейт» от
своего `audited_head` и дальше; открывает лок только `ALLOW-SUBJECT-CHANGE:` в теле коммита
ТОГО ЖЕ диапазона. `R-185` несёт `verdict: NOTE` при `audited_head=692aac2`, а после этой
ревизии `scripts/verify_M-70.sh` правился ТРИЖДЫ (`76de24d`, `9ff366a`, `2ec574d`).

**По существу правки санкционированы:** каждая сделана ПО ТРЕБОВАНИЮ критика, и последняя
(`2ec574d`) судима `C-228` (`audited_head=2ec574d`, NOTE). Барьер этого не видит: он не знает
отношения «позднейший вердикт перекрывает более ранний» и судит каждый файл отдельно.

**Корень, который стоит назвать, потому что он повторится:** `R-185` — не вердикт гейта, а
ВТОРОЕ МНЕНИЕ консультанта, вызванное founder'ом. Он надел шапку `GATE-META` с проходным
исходом и тем самым запер класс «гейт» на предмете, круги гейта по которому продолжались. Это
не ошибка автора `R-185` (шапка требуется от вердикта любого гейта — `gates.md` §4), а
незакрытый случай в конструкции: у консультации нет своего исхода, и она вынуждена
маскироваться под проходной.

**Развязки, обе в зоне architect'а:** (а) `ALLOW-SUBJECT-CHANGE: <причина>` в теле коммита
диапазона; (б) отличить консультацию от вердикта гейта — отдельный исход в словаре
`GATE-META`, не запирающий subject-lock.

---

# Находки — не блокеры merge'а, но обязаны быть сняты ДО close-out'а

## Н-1 — усиленный деплой-гейт §5 привязан к событию, которого при варианте B НЕ происходит

Спека §5 усиливает деплой-гейт условием «**Смена `GATEWAY_BANDS` меняет ПОВЕДЕНИЕ ДАННЫХ на
проде**». Вариант B `GATEWAY_BANDS` не меняет ⇒ по букве §5 усиление не применяется, и merge
уезжает на прод под тремя liveness-проверками, которые §5 сама называет недостаточными.
**Между тем поведение прода merge МЕНЯЕТ, и вот замер, а не опасение:**

```
$ ssh … 'for f in …/gateway-ckpt/_data/ckpt-*.bin; do printf "%s " $(basename $f); od -An -tu4 -j8 -N8 $f; done'
ckpt-2a00318f774d9689.bin           2          8
ckpt-b0f1ed89ec2ec142.bin           2          9        ← ЖИВОЙ, снят 18.09 14:15
```

`read_and_validate` шаг (3) (`crates/gateway/src/lib.rs:4198-4202`) отвергает файл при
`gw_v != GATEWAY_SCHEMA_VERSION`. После merge'а константа станет `10`, значит **живой
чекпоинт прода становится недействительным в момент деплоя**, и до следующего прогона cron
`gateway-serve` реплеит журнал с головы на каждом подключении — класс `TD-044`/`R-029`,
против которого строились `M-38b`/`M-48`/`M-54`.

Поведение САМО ПО СЕБЕ корректно и оракулом покрыто (`red_checkpoint_bootstrap_truncated.rs:737`,
`stale_schema_version_checkpoint_rebuilds_silently_and_overwrites`; спека §3ter это называет).
Дефект не в коде, а в том, что **условие усиленного деплой-гейта не перечитали после
переопределения предмета на вариант B**. Это ПЯТЫЙ носитель класса «правлю названный
экземпляр, не грепнув класс» в этом предмете — четыре первых нашла сама rev12.1.

**Что требуется:** §5 переформулировать от ПОСЛЕДСТВИЯ, а не от переменной («merge меняет
форму провода и инвалидирует чекпоинт ⇒ деплой-гейт усилен»), и в close-out назвать холодный
пересбор ожидаемым переходным состоянием с указанием, когда cron вернёт тёплый старт. Зона
architect'а (спека), не dev'а.

## Н-2 — ролевая git-личность выставлена локально, вопреки `branch-hygiene.md` п.6

```
$ git config --global user.name       → Alex K
$ git config --local  user.name       → engine-dev          ← в ОБЩЕМ чекауте
$ git log 4a575f2 --format='%an' --not origin/main | sort -u → engine-dev
```

Все восемь коммитов круга подписаны `engine-dev`, включая коммиты critic'а и architect'а
(роль при этом верно указана меткой в subject'е). `branch-hygiene.md` п.6 прямо запрещает:
«Ролевые `user.name` не выставляются» — и запрещает по замеру, который уже был: все 14
worktree несли подпись `reviewer` независимо от того, кто в них работал. Здесь класс
воспроизвёлся зеркально. Вреда аудиту не нанесено (метки в subject'ах верны и различают
роли), но носитель ложного признака жив и будет тиражироваться копированием worktree.
Механического барьера у правила нет. Заведу карточкой при close-out'е — `TECH-DEBT.md` моя
зона.

## Н-3 — `C-226` и `C-227` суть два REJECT подряд по ОДНОЙ причине; арбитр по `gates.md` §0 не созывался

Причина у обоих дословно одна: предикат судит ФРАГМЕНТ текста вместо ЗНАЧЕНИЯ, которое увидит
потребитель (`C-226` B-1 — не разбирает дефолт; `C-227` B-1 — разбирает, но теряет суффикс за
скобками). `gates.md` §0 триггер 1: «Два REJECT подряд по одной и той же причине — не жди
третьего». Арбитр созван не был; третий круг (`C-228`) сошёлся.

**Записываю как NOTE, а не как блокер, и называю почему:** вред, который триггер
предотвращает (несходимость), не наступил — круги были продуктивны и каждый сузил предикат к
прод-форме. Но сам факт пропуска триггера фиксирую: в `docs/ROADMAP.md` п.19 уже числится
предмет с «8 кругов REJECT без созыва арбитра — нарушение `gates.md` §0 само по себе», то есть
класс в проекте рецидивный, и молчать о его втором носителе нельзя.

---

# Условие APPROVED — ровно два пункта, оба вне кода, оба к `architect`

1. **`protected-artifacts` зелен на дереве слияния** — переезд `M-75` в `docs/archive/` либо
   назван токеном `ALLOW-ARTIFACT-DELETE` в коммите диапазона, либо барьер научен следовать
   переезду. Предъявление — прогон `check_protected_artifacts.sh` на дереве слияния, `exit=0`.
2. **`gate-meta` зелен на дереве слияния** — subject-lock `R-185` снят токеном
   `ALLOW-SUBJECT-CHANGE` в коммите диапазона либо конструкцией, отличающей консультацию от
   проходного вердикта гейта. Предъявление — тот же прогон, `exit=0`.

Плюс, НЕ блокируя merge, но ДО close-out'а: `Н-1` (переформулировать условие усиленного
деплой-гейта §5 от последствия, а не от переменной).

**Ни один пункт не возвращается engine-dev'у: его работа принята целиком и второй круг
подряд.** Все три — зона architect'а (`scripts/**` и спека — sacred).

**Предел моего предсказания назван честно:** я предъявил красное прогоном ТЕХ ЖЕ барьеров на
дереве слияния в форме, которой их зовёт CI (`EVENT_NAME=pull_request`,
`PR_BASE_SHA=origin/main`, HEAD = merge-коммит). Это воспроизведение CI, а не сам CI. PR я не
открывал: открывать PR ради красного агрегата, причина которого уже измерена, — шум.

---

## Done Block

```
$ pwd; git log -1 --oneline
/tmp/hft-rev-m70-r5
4a575f2 spec(M-70): врезка статусов приведена к прогону 18.09 — снимок 16.09 протух [architect]

$ git status --porcelain
{пусто — обе мутации возвращены}

$ bash scripts/verify_M-70.sh > /tmp/r5-verify.out 2>&1; echo "VERIFY_EXIT=$?"
VERIFY_EXIT=0
$ grep -c '^PASS' /tmp/r5-verify.out; grep -c '^FAIL' /tmp/r5-verify.out; tail -1 /tmp/r5-verify.out
42
0
VERDICT: PASS

$ # МУТАЦИЯ A — обход C-227 в дефолте gateway-serve
$ bash scripts/verify_M-70.sh; echo exit=$?
FAIL: task #7 — ОТРЕНДЕРЕННЫЙ прод-дефолт НЕ РОВНО узкий: '0.001,0.02'
FAIL: cargo test --all --quiet
VERDICT: FAIL (2)
exit=1

$ # МУТАЦИЯ B — расходится ТОЛЬКО дефолт gateway-checkpoint
$ cargo test -p gateway --test red_checkpoint_bin_prod_argv 2>&1 | tail -3
failures:
    c3ter_writer_and_reader_agree_on_checkpoint
test result: FAILED. 7 passed; 1 failed

$ # ДЕРЕВО СЛИЯНИЯ /tmp/hft-rev-m70-mp @ 9697400 (слияние чистое, конфликтов 0)
$ for s in protected_artifacts gate_meta docs_freeze artifact_ids review_fa roadmap_sync; do
>   EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) bash scripts/check_$s.sh >/dev/null 2>&1; echo "$s exit=$?"; done
protected_artifacts exit=1        ← БЛОКЕР Б-1
gate_meta           exit=1        ← БЛОКЕР Б-2
docs_freeze         exit=0
artifact_ids        exit=0
review_fa           exit=0
roadmap_sync        exit=0

$ gh run list --branch main --limit 2
completed success Deploy to VPS  main workflow_run 35267000453
completed success Merge PR #189  CI   main push    35266148403
$ ssh … 'git rev-parse --short HEAD; docker ps --format "{{.Names}} {{.Status}}"'
3f399ec                                  ← верно: deploy.yml фильтрован по путям, #189 — docs-only
hft-gateway-serve Up 28 hours (healthy)
hft-recorder      Up 28 hours (healthy)
```

## Cross-references

- `research/reviews/R-184-M-70-pr-gate-rev4.md` (круг 4, два блокера — оба закрыты выше)
- `research/reviews/R-185-m70-enablement-second-opinion.md` (второе мнение; носитель `Б-2`)
- `research/critiques/C-226`/`C-227`/`C-228` (круг `gates.md` §9 по rev10/rev11 — исполнен)
- `milestones/M-70-depth-bands-enablement.md` §1 (цель переопределена), §3sexies (а), §5
- `docs/fa/viz-backend.md` `VB-I-4`/`VB-I-5`/`VB-I-10`, `GS-I-1`
- `docs/PENDING-SIGNATURE.md` `П-014`, `П-029` п. 4
- `TECH-DEBT.md` `TD-159`, `TD-161`, `TD-198`, `TD-205`
- `.claude/rules/gates.md` §0 (арбитр), §4 (PR-гейт), §8 (агрегат и деплой), §9 (документы)
- `.claude/rules/branch-hygiene.md` п.6 (`Н-2`)
