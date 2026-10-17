<!-- GATE-META
milestone: TD-243
audited_repo: a3ka/hft-platform
audited_base: 3670d4b3e16bb99dedb3dc4c081256b70a1eedbd
audited_head: b0bec7547196b7d478b91ec6f34c937d6f8caa10
verdict: REJECT
-->

# R-236 — адверсарий харнесс-трека: шаг D6b «маркеры runbook'а печатает код» (TD-243)

**Вердикт: REJECT — один блокер, дешёвый в снятии.** Конструкция `A-047` §2 п.1 реализована
верно по существу: семь маркеров настоящего `deploy/README.md` резолвятся в `deploy.yml` и
`journal-retention.rs`, шаг `D6b` реально краснеет на подложном маркере, девять из десяти
проверок библиотеки пиннятся пробой, фикстуры убраны. Но одна проверка — исключение `docs/**`
из области поиска — **не пиннится ни одним сценарием**: мутант без неё проходит пробу 12/12,
а на настоящем дереве подтверждает маркер архивным скриптом из `docs/archive/`. Это прямое
нарушение гейта трека (`harness-track.md` §5 п.2: «нейтрализация КАЖДОЙ проверки роняет свой
сценарий»), и утверждение коммита «мутанты (10) — каждый роняет свой сценарий» для этой
проверки ложно: авторский мутант `M5-docs.sh` снимал `docs/**` и `research/**` ОДНОВРЕМЕННО и
ловился файлом `research/r.txt`.

Вердикт засчитывается и как перепроверка `gates.md` §9 (зона `scripts/lib`, `scripts/tests`,
`.github/workflows`) — пункты (а)/(б)/(в) в §«Перепроверка §9».

## Предмет

- Ветка `origin/harness/runbook-markers`, вершина и база взяты командами:
  ```
  $ git fetch origin; git rev-parse origin/harness/runbook-markers
  b0bec7547196b7d478b91ec6f34c937d6f8caa10
  $ git merge-base origin/main origin/harness/runbook-markers
  3670d4b3e16bb99dedb3dc4c081256b70a1eedbd      # = origin/main, диапазон — один коммит
  ```
- Диф: `.github/workflows/ci.yml` (+2), `scripts/lib/runbook_markers.sh` (+69),
  `scripts/tests/red_runbook_markers.sh` (+185), `scripts/verify_delivery_M-08.sh` (+15).
- Свой worktree `/tmp/hft-adv-markers`; общий чекаут не трогался. Прочитаны целиком:
  `harness-track.md`, `A-047` §2 (с `origin/main`), библиотека, проба, диф verify/CI;
  `TECH-DEBT.md` — грепом `TD-243` (`:53`, `:220`, `:5574`).

## Находки

### Б-1 — исключение `docs/**` не пиннится пробой; мутант без него зелёный 12/12

`scripts/lib/runbook_markers.sh:57` исключает четыре класса путей: `*.md`, `docs/**`,
`research/**`, `*/tests/*`. Проба пиннит три из них: `*.md` — `deploy/OTHER.md` (S5, `:106`),
`research/**` — `research/r.txt` (S5, `:107`), `*/tests/*` — S6. Для `docs/**` единственный
файл фикстуры — `docs/notes.md` (S5, `:105`), который уже снимается исключением `*.md`.

Мутант (одна правка, снято только `':(exclude)docs/**'`):
```
$ diff scripts/lib/runbook_markers.sh m1_no_docs_excl.sh | grep '^[<>]'
<               ':(exclude)*.md' ':(exclude)docs/**' ':(exclude)research/**' \
>               ':(exclude)*.md' ':(exclude)research/**' \
$ RUNBOOK_MARKERS_LIB=m1_no_docs_excl.sh bash scripts/tests/red_runbook_markers.sh | tail -2
сценариев: 12, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
```

Это не теоретическая дыра — исключение несущее на настоящем дереве:
```
$ git ls-files docs research | grep -vc '\.md$'
58
$ git grep -oE '===[^=]*===' -- docs research ':(exclude)*.md' | wc -l
15          # docs/archive/verify_M-40.sh:45 «=== M-40 acceptance ===» и т.д.
```
README, цитирующий маркер из архивного скрипта, против оригинала и мутанта (README передан
параметром, корень — настоящее дерево ветки):
```
-- оригинал:
miss  === M-40 acceptance ===  — префикс «=== M-40 acceptance ===» не печатается ни одним файлом кода
rc=1
-- m1 (без docs/**):
ok    === M-40 acceptance ===  ← docs/archive/verify_M-40.sh
rc=0
```
Авторский набор мутантов этого не видел по построению: `M5-docs.sh` (scratchpad автора)
снимает `docs/**` **и** `research/**` разом и падает на S5 благодаря `research/r.txt`.

**Воспроизведение:** команды выше из корня worktree ветки.
**Что требуется для снятия:** сценарий (или строка в S5), где маркер лежит ТОЛЬКО в не-`.md`
файле под `docs/**` — например `put "$d" docs/archive/old_verify.sh 'echo "=== ONLY IN DOCS ==="'`,
со стражем `tracked`. Один `put`, без изменений библиотеки. Желательно — по одному сценарию
на каждое исключение, чтобы бандл не повторился.

### Н-1 — allowlist путей вместо «весь репозиторий минус документы» проходит 12/12

`A-047` §2 п.1 требует область поиска «весь репозиторий минус документы». Проба пиннит
позитив тремя точками: `deploy/bin/*.sh` (S1), `.github/workflows/*.yml` (S2, S4),
`crates/**/src/**` (S11); настоящие маркеры (S12) резолвятся ровно в эти же три класса
(`deploy.yml`, `journal-retention.rs`, `deploy/bin/install-watchdog.sh`). Мутант:
```
>     where=$(git -C "${root}" grep -lF -e "${p}" -- '.github/**' 'crates/**/src/**' 'deploy/bin/**' \
сценариев: 12, провалов: 0 … VERDICT: PASS
```
Маркер, печатаемый `scripts/*.sh`, `deploy/*.sh` (не `bin/`), `Dockerfile`, `docker-compose.yml`,
стал бы ложным FAIL. Не блокер: позитив любая проба сэмплирует; но один сценарий с печатью из
«неожиданного» пути (например `scripts/ops/x.sh`) закрыл бы ровно ту ошибку, от которой арбитр
предостерёг architect'а.

### Н-2 — регистронезависимый поиск проходит 12/12; на настоящем дереве даёт ложное «ok»

```
$ diff … m3_icase.sh | grep '^>'
>     where=$(git -C "${root}" grep -ilF -e "${p}" -- . \
$ RUNBOOK_MARKERS_LIB=m3_icase.sh bash scripts/tests/red_runbook_markers.sh | tail -1
VERDICT: PASS
```
README с `=== watchdog install failed ===` (нижний регистр), корень — настоящее дерево:
оригинал → `miss`, мутант → `ok ← .github/workflows/deploy.yml`. Оператор копирует маркер в
`grep -F` — регистрозависимый; расхождение регистром даёт тот же пустой grep, что и `TD-239`.
«Дословно» из конструкции включает регистр; сценарий «отличается только регистром → FAIL»
стоит три строки.

### Н-3 — стражи входа (нет README / не git-дерево) не пиннятся

`runbook_markers.sh:30-37` — два `return 1`. Мутант `return 0` на отсутствующем README
проходит пробу 12/12 (нет сценария с отсутствующим файлом или не-git корнем). В `D6b` путь
фиксирован (`${ROOT}/deploy/README.md`) и README проверяется раньше по `D6`, так что в
прод-форме дыра прикрыта соседом; но как свойство библиотеки — не пиннится.

### Н-4 — извлечение сцепляет закрывающий `===` разорванного маркера с открывающим следующего

`runbook_markers.sh:39` — `grep -oE '===[^=]*==='` построчно. Настоящий README содержит
ЧЕТЫРЕ цитаты маркера, разорванные переносом (`deploy/README.md:110-111`, `:117-118`,
`:127-128`, `:131-132`); сегодня каждая закрывается на строке, где нет второго маркера, и
`D6b` зелёный. Стоит хвосту разорванного маркера оказаться на одной строке с другим маркером:
```
$ printf 'Строка `=== WATCHDOG INSTALL\nOK ===` и `=== отчёт ===`.\n' > f1.md
$ runbook_markers_check f1.md .
miss  ===` и `===  — префикс «===` и `===» не печатается ни одним файлом кода
runbook-markers: маркеров 1, не найдено в коде 1
```
Три эффекта разом: мусорный маркер (ложное красное), подложный разорванный маркер НЕ проверен,
честный `=== отчёт ===` ПРОГЛОЧЕН. Направление отказа — красное (дешёвый исход по
`harness-track.md` §2 п.1), поэтому не блокер. Разрыв переносом конструкция `A-047` не
оговаривает — обработка разорванных маркеров **вне объёма**; но сцепка лечится в один символ
(`[^=\x60]*` — бэктик не бывает внутри маркера) и как минимум заслуживает строки в шапке
библиотеки среди названных пределов.

### Н-5 — маркер со знаком `=` в теле не извлекается никогда → молча не проверяется

Тот же `[^=]*`: `=== deployed sha=abc ===` на отдельной строке не попадает в список маркеров;
при наличии других маркеров `D6b` зелёный (`маркеров 1, не найдено 0, rc=0` на фикстуре с
двумя строками). Сегодня код таких маркеров не печатает:
`git grep -cE '===[^=]*=[^=]+===' -- .github deploy crates/*/src` → 0 файлов. Латентно;
форма `key=value` внутри маркера правдоподобна (`=== deployed sha=… ===`). Вне объёма
конструкции; назвать пределом.

### Н-6 — setext-подчёркивание `======` принимается за маркер

`======` матчится как `===` + пусто + `===`; `body` после среза ведущих `===` равен `===`
(длина 3 ≥ порога `:51`) и ищется в коде:
```
ok    ======  ← scripts/verify_alerting.sh
```
Ложное «ok» (или ложное красное, не окажись `======` в коде). Настоящий README setext не
использует. Лечится `[^=]+` вместо `[^=]*` в `:39`.

### Н-7 — ASCII `...` не считается местом подстановки

`:46` режет только по `…` (U+2026). `=== DEPLOY FAILED — logs + rollback to ... ===` → `miss`
целиком. README последователен в `…` (`:122`, `:152`), шапка библиотеки говорит
«многоточие» без уточнения. Информационно; одна оговорка в шапке.

### Что НЕ сломалось (позитивный и мутационный контроль подтверждён)

- Проба против честной библиотеки: `сценариев: 12, провалов: 0, VERDICT: PASS`.
- Мои мутанты, которые проба ловит: без `*/tests/*` → S6; без `…`-среза → S2+S12;
  без `research/**` → S5; «хоть один найден → 0» → S10. Авторские `M2-lt`, `M3-dollar`,
  `M4-md`, `M6-tests`, `M7-untracked`, `M8-zero`, `M9-emptyprefix`, `M10-always0`
  перепрогнаны — каждый роняет свой сценарий (вывод в Done Block).
- Формы цитирования без дефекта: кавычки вместо бэктиков и маркер без обрамления (F4 — оба
  `ok`); `LC_ALL=C` на фикстуре с `…`/`—` и на настоящем README (`маркеров 7, не найдено 0`);
  `=== A === B ===` — извлекается `=== A ===`, хвост отброшен (ожидаемо); коллизия префикса
  `=== DEPLOY … ===` ↔ `=== DEPLOY FAILED …` — врождённое свойство префиксного сравнения,
  названное в `A-047`, не находка.
- Отказ засчитывается только по названной причине (`expect`, проба `:48`) — мутант
  «краснеет на всём» (`M10`-класс) ловится семью сценариями.

## Пункт (3) мандата — D6b краснеет в прод-форме; проводка CI

Временная правка `deploy/README.md` в своём worktree (добавлена строка с
`=== WATCHDOG INSTALL OK ===`), затем `git checkout -- deploy/README.md`:
```
baseline:  PASS  D6b маркеры deploy/README.md печатаются кодом (runbook-markers: маркеров 7, не найдено в коде 0)
           DELIVERY: PASS   exit=0
broken:    FAIL  D6b deploy/README.md велит искать в логе маркер, которого код не печатает (TD-243):
                 miss  === WATCHDOG INSTALL OK ===  — префикс «=== WATCHDOG INSTALL OK ===» не печатается ни одним файлом кода
                 runbook-markers: маркеров 8, не найдено в коде 1
           DELIVERY: FAIL (1)   exit=1
restored:  git status --porcelain | wc -l → 0
```
Проводка: шаг пробы стоит в джобе `delivery` (`ci.yml:46-47`) после `verify_delivery_M-08.sh`;
`delivery` входит в `needs` агрегата `All checks passed` (`ci.yml:630`) и в его условие
(`:634`) — агрегат уже пробуется поджобно (`:644`). `actions/checkout@v4` даёт git-дерево,
`git grep` по отслеживаемым файлам работает на shallow-клоне; фикстуры пробы — собственные
`git init` с локальным `user.*`. Проводка корректна. Предел (не находка): при красном
`verify_delivery` шаг пробы не выполняется — джоб и так красный.

## Пункт (4) — уборка фикстур

Один `mktemp -d` на прогон, `trap EXIT INT TERM` (`:19-20`), явный `rm -rf` в конце (`:180`),
число печатается прогоном. Замер снаружи (глобальный `/tmp` у хоста грязный, поэтому дельта):
```
каталогов /tmp/tmp.* до прогона: 2481   после: 2481   (дельта 0)
печать пробы: каталогов фикстур после прогона: 0
```
§5 п.5 выполнен. Оговорка: «реестр в ФАЙЛЕ» реализован одним корневым каталогом — для 12
фикстур это эквивалент.

## Перепроверка §9 — (а) утверждения о коде, (б) полномочия, (в) ссылки

**(а)** Ветка = `origin/main` + 1 коммит, merge-base = `origin/main`, значит дерево ветки и
есть дерево слияния. Проверено командами на нём: семь маркеров и их источники (вывод
библиотеки); `ci.yml:630/634` — `delivery` в `needs` и в условии агрегата;
`bash scripts/verify_design_claims.sh --merge-preview origin/main` → `VERDICT: PASS (0 нарушений)`.
Утверждения шапки библиотеки (`:10-17`) о срезе префикса и четырёх исключениях совпадают с
кодом `:46-58`. Утверждение коммита «каждый мутант роняет свой сценарий» — ЛОЖНО для проверки
`docs/**` в отдельности (Б-1).

**(б)** Тронуты только `scripts/lib/**`, `scripts/tests/**`, `scripts/verify_*.sh`,
`.github/workflows/**` — зона architect'а (`scope-guard.md`) и зона трека
(`harness-track.md` §2). Замок §11 не задет: `in_zone` в `check_docs_freeze.sh:61-66` не
включает `.github/workflows`; барьер в PR-форме (`EVENT_NAME=pull_request PR_BASE_SHA=<main>`)
→ exit=0. Граница C не затронута. Код прод-процесса (`crates/**/src`) не тронут — предмет
проходит три вопроса §4 трека.

**(в)** `A-047-td-239-runbook.md`, `R-231-TD-239-watchdog-runbook-rev2.md` существуют;
`scripts/tests/red_runbook_markers.sh` существует; директивы `# shellcheck source=` резолвятся
(`lib/runbook_markers.sh` от `scripts/`, `../lib/runbook_markers.sh` от `scripts/tests/`);
`TD-243` заведена (`TECH-DEBT.md:220`). `check_protected_artifacts.sh origin/main HEAD` → OK;
`check_roadmap_sync.sh` в PR-форме → PASS. Висячих ссылок нет.

**FA:** диф не трогает `crates/**` — барьер `review-fa` даёт SKIP; инвариант крейта не
называется по той же причине (вне `crates/**` требование когнитивное, предъявлять нечего).

## Done Block

```
$ git rev-parse origin/harness/runbook-markers; git merge-base origin/main origin/harness/runbook-markers
b0bec7547196b7d478b91ec6f34c937d6f8caa10
3670d4b3e16bb99dedb3dc4c081256b70a1eedbd

$ bash scripts/tests/red_runbook_markers.sh | tail -2
сценариев: 12, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS

$ for f in mut/*.sh; do RUNBOOK_MARKERS_LIB=$f bash scripts/tests/red_runbook_markers.sh | grep -E '^(сценариев|VERDICT)' | tr '\n' ' '; done
m1_no_docs_excl    сценариев: 12, провалов: 0  VERDICT: PASS      ← Б-1
m2_allowlist       сценариев: 12, провалов: 0  VERDICT: PASS      ← Н-1
m3_icase           сценариев: 12, провалов: 0  VERDICT: PASS      ← Н-2
m4_missing_ok      сценариев: 12, провалов: 0  VERDICT: PASS      ← Н-3
m5_no_tests_excl   провалов: 1 (S6)            VERDICT: FAIL
m6_no_ellipsis     провалов: 2 (S2, S12)       VERDICT: FAIL
m7_any             провалов: 1 (S10)           VERDICT: FAIL
m8_no_research     провалов: 1 (S5)            VERDICT: FAIL
M2-lt / M3-dollar / M4-md / M5-docs / M6-tests / M7-untracked / M8-zero / M9-emptyprefix / M10-always0 (авторские)
                   VERDICT: FAIL каждый (S2+S9+S12 / S2 / S3-S7,S10 / S5 / S6 / S7 / S8 / S9 / 7 сценариев)

$ source scripts/lib/runbook_markers.sh; runbook_markers_check deploy/README.md .; echo rc=$?
… runbook-markers: маркеров 7, не найдено в коде 0
rc=0
$ LC_ALL=C bash -c 'source scripts/lib/runbook_markers.sh; runbook_markers_check deploy/README.md . | tail -1'
runbook-markers: маркеров 7, не найдено в коде 0

$ bash scripts/verify_delivery_M-08.sh | grep -E 'D6b|DELIVERY'; echo exit=$?         # чистый README
PASS  D6b маркеры deploy/README.md печатаются кодом (runbook-markers: маркеров 7, не найдено в коде 0)
DELIVERY: PASS
exit=0
$ printf '\nУспех: ищите `=== WATCHDOG INSTALL OK ===` в логе.\n' >> deploy/README.md; bash scripts/verify_delivery_M-08.sh | grep -E 'D6b|DELIVERY'; echo exit=$?
FAIL  D6b deploy/README.md велит искать в логе маркер, которого код не печатает (TD-243):
DELIVERY: FAIL (1)
exit=1
$ git checkout -- deploy/README.md; git status --porcelain | wc -l
0

$ ls -d /tmp/tmp.* | wc -l   # до / после пробы
2481
2481

$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -1
VERDICT: PASS (0 нарушений)
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) bash scripts/check_roadmap_sync.sh | tail -1
VERDICT: PASS
$ bash scripts/check_protected_artifacts.sh origin/main HEAD | tail -1
OK: защищённые артефакты целы на HEAD (3670d4b..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
```

## Условие APPROVE

Снять Б-1: сценарий, пиннящий исключение `docs/**` отдельно от `*.md` (не-`.md` файл под
`docs/**`, со стражем `tracked`), и предъявить прогон: мутант `m1_no_docs_excl` → этот
сценарий FAILED. Н-1…Н-7 — на усмотрение автора; Н-4/Н-5/Н-7 как минимум назвать пределами в
шапке библиотеки, если не чинятся. Расширение конструкции (обработка разорванных маркеров,
`=` в теле) — вне объёма этого круга.
