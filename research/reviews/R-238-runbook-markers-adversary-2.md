<!-- GATE-META
milestone: TD-243
audited_repo: a3ka/hft-platform
audited_base: 3670d4b3e16bb99dedb3dc4c081256b70a1eedbd
audited_head: 83fbef8bfcf09c4930ee7de352bb682103c3f98d
verdict: REJECT
-->

# R-238 — адверсарий харнесс-трека, круг 2: шаг D6b «маркеры runbook'а печатает код» (TD-243)

**Вердикт: REJECT — один блокер того же класса, что `R-236` Б-1, дешёвый в снятии.** Всё,
что требовал круг 1, сделано и подтверждено исполнением: Б-1 снят (мутант «снято только
`docs/**`» роняет S13), Н-1…Н-4 и Н-6 закрыты сценариями S14–S18b, и каждый из них пиннится
своим одно-строчным мутантом; Н-5/Н-7 и разрыв маркера переносом названы пределами в шапке
библиотеки; настоящий `deploy/README.md` даёт 7 маркеров и `PASS`, `D6b` краснеет в прод-форме
на подложном маркере, фикстуры убраны (дельта каталогов 0). Но у библиотеки остались **две
проверки, которые проба не пиннит**: самоисключение собственного файла из области поиска
(`:68`) и дословность сравнения — флаг `-F` (`:66`). Снятие любой из них проходит пробу
20/20 и на фикстуре даёт ложное «ok» — направление отказа дорогое. `harness-track.md` §5 п.2
(«нейтрализация КАЖДОЙ проверки роняет свой сценарий») — условие merge'а, и именно по нему
круг 1 вынес REJECT; стандарт здесь тот же. Утверждение коммита «16 из 16 мутантов роняют
свой сценарий» верно для авторского набора и не покрывает эти две проверки.

Вердикт засчитывается и как перепроверка `gates.md` §9 по зоне `scripts/lib`,
`scripts/tests` — пункты (а)/(б)/(в) ниже.

## Предмет

```
$ git fetch origin --prune
$ git rev-parse origin/harness/runbook-markers
83fbef8bfcf09c4930ee7de352bb682103c3f98d
$ git merge-base origin/main origin/harness/runbook-markers
3670d4b3e16bb99dedb3dc4c081256b70a1eedbd
$ git rev-parse origin/main
087ba995d26ff9155bfb56ebc5432ca3e465f14b        # main ушёл на 3 коммита (PR #316, TD-242)
$ git log --oneline 83a149b..origin/harness/runbook-markers      # объём круга 2 — после R-236
83fbef8 fix(TD-243): R-236 — сценарии на каждое исключение, регистр, стражи входа, извлечение без склейки; пределы в шапке [architect]
$ git show --stat --format= 83fbef8
 scripts/lib/runbook_markers.sh       | 12 +++++-
 scripts/tests/red_runbook_markers.sh | 75 ++++++++++++++++++++++++++++++++++++
```

Отсоединённый worktree `/tmp/hft-adv2-markers`; общий чекаут не трогался. Прочитаны
целиком: библиотека и проба на вершине, диф круга 2, `R-236`, `harness-track.md` §3/§5,
`A-047` §2 (с `origin/main`); `TECH-DEBT.md` — грепом `TD-243` (`:53`, `:220`, `:5574`).
`origin/main` после базы изменил только `crates/gateway-serve/src/lib.rs` и `R-235`
(`git diff --name-only 3670d4b origin/main`) — предмет не задет; проба и `D6b` прогнаны
дополнительно на дереве слияния (`git merge --no-edit 83fbef8` поверх `origin/main` во
временном worktree): `сценариев: 20, провалов: 0`, `D6b PASS`, `DELIVERY: PASS`.

## Пункт (1) мандата — Б-1 круга 1 снят

```
$ diff scripts/lib/runbook_markers.sh m01_no_docs_excl.sh | grep '^>'
>               ':(exclude)*.md' ':(exclude)research/**' \
$ RUNBOOK_MARKERS_LIB=m01_no_docs_excl.sh bash scripts/tests/red_runbook_markers.sh | grep -E '^(FAIL|сценариев|VERDICT)'
FAIL  S13 маркер только в не-.md файле под docs/ — ожидалось fail, rc=0
сценариев: 20, провалов: 1, каталогов фикстур после прогона: 0
VERDICT: FAIL
```

## Пункт (2) мандата — Н-1…Н-4, Н-6: сценарии и их мутанты

Каждый мутант — ОДНА строка библиотеки (проверено `diff | grep -c '^>'` = 1), проба против
каждого — красная ровно там, где должна:

| находка R-236 | сценарий | мутант (одна строка) | роняет |
|---|---|---|---|
| Б-1 `docs/**` | S13 | снято `:(exclude)docs/**` | S13 |
| (по одному на исключение) `research/**` | S14 | снято `:(exclude)research/**` | S5, S14 |
| Н-1 allowlist путей | S15 | `-- '.github/**' 'crates/**/src/**' 'deploy/bin/**'` | S15 |
| Н-2 регистр | S16 | `grep -ilF` | S16 |
| Н-3 нет README / не git | S17 / S17b | `:42 return 0` / `:46 return 0` | S17 / S17b |
| Н-4 склейка через бэктик | S18, S18b | `'===[^=]+==='` (без бэктика) | S18, S18b |
| Н-4/Н-6 старое извлечение | S18, S18b | `'===[^=]*==='` | S18, S18b |
| Н-6 setext | S18b | `'===[^=`]*==='` | S18b |

Полная таблица прогона — в Done Block. Пределы Н-5 (`=` в теле), Н-7 (ASCII `...`), разрыв
переносом и регистр — названы в шапке библиотеки `:24-32`; формулировка о регистре совпадает
с командой оператора в `deploy/README.md:130` (`grep -F '=== DEPLOY FAILED'`).

## Находки

### Б-1 — две проверки библиотеки не пиннятся: самоисключение (`:68`) и дословность `-F` (`:66`)

**(а) Самоисключение собственного файла.** Шапка `:15-17` объявляет «сам этот файл» не
кодом; проба каждым `mk` копирует библиотеку в фикстуру (`red_runbook_markers.sh:33`) и
коммитит её (`:38`) — то есть ровно для того, чтобы исключение было наблюдаемо, — но ни один
сценарий не кладёт маркер в эту копию. Мутант и его цена:
```
$ diff scripts/lib/runbook_markers.sh m05_no_self_excl.sh | grep '^>'
>               ':(exclude)*/tests/*' 2>/dev/null \
$ RUNBOOK_MARKERS_LIB=m05_no_self_excl.sh bash scripts/tests/red_runbook_markers.sh | tail -2
сценариев: 20, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
-- фикстура F1: маркер ТОЛЬКО в шапке копии библиотеки (# пример маркера из TD-239: === SELF ONLY ===)
оригинал:  miss  === SELF ONLY ===  — префикс «=== SELF ONLY ===» не печатается ни одним файлом кода   rc=1
m05:       ok    === SELF ONLY ===  ← scripts/lib/runbook_markers.sh                                   rc=0
```
На настоящем дереве сегодня не несущее: единственный извлекаемый фрагмент шапки — `=== … ===`
(`grep -oE '===[^=`]+===' scripts/lib/runbook_markers.sh`), префикс пуст. Но шапка — текст об
этих самых маркерах (`TD-239`, `R-231`), и первая же процитированная в ней строка маркера
превращает мутанта в ложно-зелёный. Класс тот же, что `R-236` Б-1: исключение из области
поиска, которое проба не отличает от его отсутствия.

**(б) Дословность — флаг `-F`.** Шапка `:12` требует, чтобы префикс встречался «ДОСЛОВНО»;
механизм этого — `grep -lF`. Без `-F` `git grep` сравнивает BRE-шаблоном:
```
$ diff scripts/lib/runbook_markers.sh m18_regex_grep.sh | grep '^>'
>     where=$(git -C "${root}" grep -l -e "${p}" -- . \
$ RUNBOOK_MARKERS_LIB=m18_regex_grep.sh bash scripts/tests/red_runbook_markers.sh | tail -1
VERDICT: PASS
-- фикстура F2: README `=== retention v1.2 ... done ===`, код `echo "=== retention v1x2 XXX done ==="`
оригинал:  miss  === retention v1.2 ... done ===  — префикс «…» не печатается ни одним файлом кода   rc=1
m18:       ok    === retention v1.2 ... done ===  ← deploy/bin/x.sh                                    rc=0
```
Ни в одной фикстуре пробы нет маркера с метасимволом BRE (`.`, `*`, `[`); семь настоящих
маркеров несут только `(`, `+`, `—`, которые в BRE литеральны, — поэтому ни проба, ни S12 не
видят разницы. Форма `v1.2` / `D-COMP-3.1` внутри маркера правдоподобна (сегодняшний
`=== компакция закрытых сегментов (D-COMP-3) ===` в одном шаге от неё).

**Что требуется для снятия (две правки пробы, библиотека не меняется):**
1. сценарий «маркер только в копии библиотеки»: после `mk` —
   `printf '# пример: === SELF ONLY ===\n' >> "$d/scripts/lib/runbook_markers.sh"`, `commit`,
   страж `grep -qF 'SELF ONLY' "$d/scripts/lib/runbook_markers.sh"`,
   `expect fail … "miss  === SELF ONLY ==="`;
2. сценарий «метасимвол в маркере, код отличается на этот символ»: README
   `` `=== retention v1.2 ===` ``, код `echo "=== retention v1x2 ==="`, `expect fail … "miss  === retention v1.2 ==="`.
Предъявить прогон: `m05` → первый сценарий FAILED, `m18` → второй FAILED.

### Н-1 — утверждение шапки `:28-29` «сегодня код таких не печатает» ложно по команде

```
$ git grep -nE '===[^=]*=[^=]+===' -- . ':(exclude)*.md' ':(exclude)docs/**' ':(exclude)research/**' \
    ':(exclude)*/tests/*' ':(exclude)scripts/lib/runbook_markers.sh'
scripts/verify_M-49.sh:210:echo "=== итог: FAIL=$FAILS ==="
```
`R-236` Н-5 мерил узкую область (`.github deploy crates/*/src` → 0); шапка расширила слова до
«код» без расширения команды. Механизм не задет: `deploy/README.md` маркеров с `=` в теле не
цитирует (`grep -nE '===[^=`]*=[^=`]+===' deploy/README.md` → rc=1), и предел остаётся честно
названным. Но это утверждение о коде в документе, который читают как спецификацию, и оно
опровергается одной командой — класс `gates.md` §9 (а). Правка одной фразы: «runbook сегодня
таких не цитирует» (это и есть то, что проверено). Не блокер сам по себе; снимается вместе
с Б-1.

### Н-2 — порог длины префикса (`:61`, `-lt 3`) пиннится как существование, не как величина

Мутант `-lt 1` проходит пробу 20/20 (S9 пиннит только пустой префикс). Фикстура
`` `=== A… ===` `` против кода `=== ABSENT ===`: оригинал → `miss … префикс до подстановки
пуст`, мутант → `ok ← deploy/bin/x.sh`. Величина порога — параметр конструкции, а не
отдельная проверка; информационно. Попутно: текст отказа «префикс пуст» при префиксе из 1-2
символов неточен — косметика.

### Н-3 — `*/tests/*` не исключает каталог `tests/` в КОРНЕ репозитория — вне объёма

Фикстура с `tests/t.sh` (`grep -q "=== ROOT TESTS ===" out`) → `ok ← tests/t.sh`: ведущий
`*` в git-pathspec не матчит пустой префикс без `/`. В `hft-platform` корневого `tests/` нет
(`ls -d tests` → отсутствует); конструкция `A-047` говорит о `*/tests/*` как о крейтовых
тестах. Вне объёма; одна строка `':(exclude)tests/**'`, если автор захочет закрыть.

### Что НЕ сломалось (контроль)

- Проба против честной библиотеки: `сценариев: 20, провалов: 0, VERDICT: PASS`; на дереве
  слияния с `origin/main` — то же.
- Семь маркеров настоящего README резолвятся в `deploy.yml:301/305/308` (команды `echo`, не
  комментарии) и `journal-retention.rs`; `rc=0`.
- Мутанты, которые проба ловит (по одной строке каждый): снятие любого из `*.md` /
  `docs/**` / `research/**` / `*/tests/*`; allowlist; `-i`; каждый из трёх стражей входа
  (`:42`, `:46`, `:52` → `return 0`); три варианта регулярки извлечения; снятие любого из трёх
  срезов (`…`, `<`, `$`); порог `-lt 0`; «хоть один найден → 0»; `true` вместо итога.
  Авторские `M*`-мутанты круга 1 перепрогнаны — все красные.
- `D6b` в прод-форме: чистый README → `PASS`/`exit=0`; README + строка с
  `=== WATCHDOG INSTALL OK ===` → `FAIL … miss === WATCHDOG INSTALL OK ===`, `DELIVERY: FAIL (1)`,
  `exit=1`; `git checkout -- deploy/README.md` → `porcelain=0`.
- Уборка: `/tmp/tmp.*` до 2481 / после 2481 (дельта 0); печать пробы «каталогов фикстур
  после прогона: 0».
- Проводка CI не менялась в круге 2 (`ci.yml:46-47`, джоб `delivery`) — см. `R-236`.

## Перепроверка §9 — (а) код, (б) полномочия, (в) ссылки

**(а)** Утверждения шапки библиотеки `:24-32` сверены командами: разрыв переносом не
извлекается и соседний маркер проверяется (S18 честной библиотекой — `miss === MISSING HERE ===`,
без мусора); ASCII `...` в README не встречается (`grep -nE '===[^=]*\.\.\.[^=]*===' deploy/README.md`
→ rc=1); регистрозависимость — `grep -lF` без `-i` (`:66`) и `README:130`. Утверждение
`:28-29` о коде — ЛОЖНО (Н-1). `verify_design_claims.sh --merge-preview origin/main` →
`VERDICT: PASS (0 нарушений)`. Утверждение коммита «16 из 16» верно для своего набора, но
условие §5 п.2 говорит о каждой ПРОВЕРКЕ — две не покрыты (Б-1).

**(б)** Тронуты только `scripts/lib/**`, `scripts/tests/**` — зона architect'а
(`scope-guard.md`) и зона трека (`harness-track.md` §2). Замок §11 не задет:
`EVENT_NAME=pull_request PR_BASE_SHA=<merge-base> check_docs_freeze.sh` → `exit=0` (с
`PR_BASE_SHA=origin/main` барьер fail-closed отказывает — «база не предок HEAD»; это свойство
отставшей ветки, в PR-форме GitHub проверяет merge-коммит, где `main` — предок). Граница C
не затронута; `crates/**` не тронуты — трек применим.

**(в)** `R-236`, `A-047` существуют; `# shellcheck source=` резолвятся; `TD-243` на
`origin/main` (`TECH-DEBT.md:220`). `check_protected_artifacts.sh <merge-base> HEAD` → OK;
`check_roadmap_sync.sh` в PR-форме → `VERDICT: PASS`. `shellcheck` на хосте отсутствует и в
`ci.yml` не зовётся — не прогонялся.

**FA:** диф вне `crates/**` — `review-fa` даёт SKIP; инвариант крейта не называется по той
же причине.

## Done Block

```
$ git rev-parse origin/harness/runbook-markers origin/main; git merge-base origin/main origin/harness/runbook-markers
83fbef8bfcf09c4930ee7de352bb682103c3f98d
087ba995d26ff9155bfb56ebc5432ca3e465f14b
3670d4b3e16bb99dedb3dc4c081256b70a1eedbd

$ bash scripts/tests/red_runbook_markers.sh | tail -2
сценариев: 20, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS

$ for f in mut/m*.sh; do …; done          # каждый мутант — одна строка диффа
m01_no_docs_excl       провалов: 1 | S13
m02_no_research_excl   провалов: 2 | S5 S14
m03_no_md_excl         провалов: 10 | S3 S4 S5 S6 S7 S10 S13 S14 S16 S18
m04_no_tests_excl      провалов: 1 | S6
m05_no_self_excl       провалов: 0 | VERDICT: PASS        ← Б-1 (а)
m06_allowlist          провалов: 1 | S15
m07_icase              провалов: 1 | S16
m08_missing_ok (:42)   провалов: 1 | S17
m09_nogit_ok (:46)     провалов: 1 | S17b
m10_regex_old [^=]*    провалов: 2 | S18 S18b
m11_regex_nobt [^=]+   провалов: 2 | S18 S18b
m12_regex_star [^=`]*  провалов: 1 | S18b
m13_no_ellipsis        провалов: 2 | S2 S12
m14_no_lt              провалов: 3 | S2 S9 S12
m15_no_dollar          провалов: 1 | S2
m16_thr0               провалов: 1 | S9
m17_thr1               провалов: 0 | VERDICT: PASS        ← Н-2
m18_regex_grep (-F)    провалов: 0 | VERDICT: PASS        ← Б-1 (б)
m19_any_found          провалов: 1 | S10
m20_zero_ok (:52)      провалов: 1 | S8
m21_always0            провалов: 11 | S3 S4 S5 S6 S7 S9 S10 S13 S14 S16 S18

$ source scripts/lib/runbook_markers.sh; runbook_markers_check deploy/README.md . | tail -1; echo rc=$?
runbook-markers: маркеров 7, не найдено в коде 0
rc=0

$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D6b|DELIVERY'; echo exit=${PIPESTATUS[0]}
PASS  D6b маркеры deploy/README.md печатаются кодом (runbook-markers: маркеров 7, не найдено в коде 0)
DELIVERY: PASS
exit=0
$ printf '\nУспех: ищите `=== WATCHDOG INSTALL OK ===` в логе.\n' >> deploy/README.md; bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D6b|miss|DELIVERY'; echo exit=${PIPESTATUS[0]}
FAIL  D6b deploy/README.md велит искать в логе маркер, которого код не печатает (TD-243):
      miss  === WATCHDOG INSTALL OK ===  — префикс «=== WATCHDOG INSTALL OK ===» не печатается ни одним файлом кода
DELIVERY: FAIL (1)
exit=1
$ git checkout -- deploy/README.md; git status --porcelain | wc -l
0

$ # дерево слияния: worktree origin/main + git merge --no-edit 83fbef8
сценариев: 20, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
PASS  D6b маркеры deploy/README.md печатаются кодом (runbook-markers: маркеров 7, не найдено в коде 0)
DELIVERY: PASS
exit=0

$ ls -d /tmp/tmp.* | wc -l   # до / после пробы
2481
2481

$ git grep -nE '===[^=]*=[^=]+===' -- . ':(exclude)*.md' ':(exclude)docs/**' ':(exclude)research/**' ':(exclude)*/tests/*' ':(exclude)scripts/lib/runbook_markers.sh'
scripts/verify_M-49.sh:210:echo "=== итог: FAIL=$FAILS ==="

$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -1
VERDICT: PASS (0 нарушений)
$ EVENT_NAME=pull_request PR_BASE_SHA=3670d4b3e16bb99dedb3dc4c081256b70a1eedbd bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=3670d4b3e16bb99dedb3dc4c081256b70a1eedbd bash scripts/check_roadmap_sync.sh | tail -1
VERDICT: PASS
$ bash scripts/check_protected_artifacts.sh 3670d4b HEAD | tail -1
OK: защищённые артефакты целы на HEAD (3670d4b..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
```

## Условие APPROVE

Снять Б-1: два сценария пробы (маркер только в копии библиотеки; маркер с метасимволом BRE,
отличающийся от кода на этот символ) и предъявленный прогон: `m05_no_self_excl` → первый
FAILED, `m18_regex_grep` → второй FAILED, честная библиотека — 22/22 PASS. Н-1 — одна фраза
в шапке `:28-29`. Н-2/Н-3 — на усмотрение автора, вне условия.
