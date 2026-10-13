<!-- GATE-META
milestone: TD-243
audited_repo: a3ka/hft-platform
audited_base: 3670d4b3e16bb99dedb3dc4c081256b70a1eedbd
audited_head: 5cbc5c8833863d8aa1b72cc7b1e29589de51debf
verdict: DECISION
-->

# A-048 — проверка маркеров runbook'а (D6b, TD-243): условие R-238 выполнено; вливать после одной точечной доделки пробы, без четвёртого круга

**Арбитр `gates.md` §0, свежий контекст.** Триггеры: §0 п.1 — два REJECT подряд одного класса
(«проверка библиотеки не пиннится пробой»: `R-236` Б-1, `R-238` Б-1) и §0 п.2 — третий круг по
предмету. Спора о фактах между сторонами нет: автор принял обе находки и закрыл их коммитом
`5cbc5c8`. Вопрос арбитру — выполнено ли условие APPROVE `R-238`, можно ли вливать, и если
нет — что ровно доделать, чтобы круга 4 не было.

**РЕШЕНИЕ (коротко).**
1. Условие APPROVE `R-238` **выполнено** и предъявлено исполнением (§1): мутант «без
   самоисключения» роняет S19, мутант «grep без `-F`» роняет S20, честная библиотека — 24/24;
   Н-1 (ложная фраза шапки) исправлена, и новая фраза верна по команде.
2. Из 40 одноточечных мутантов библиотеки (§2) все **проверки** пиннятся, кроме ОДНОЙ:
   агрегация «проверен КАЖДЫЙ извлечённый маркер» пиннится S10 только с одной стороны —
   реализация, проверяющая лишь ПОСЛЕДНИЙ маркер в отсортированном списке, проходит пробу
   24/24 и на настоящем дереве даёт ложное зелёное при подложном маркере (§3). Это та же
   форма дефекта, что и в двух REJECT'ах, но исправление — четыре строки пробы, и его форма
   проверена арбитром здесь же (§3): оно роняет обманную реализацию и не трогает честную.
3. **Доделать ровно одно** (§6, список точный) — и **вливать без нового круга адверсария**.
   Прогон мутанта `m30` → `S10b FAILED` автор предъявляет сырым выводом в теле коммита;
   позитивный контроль гоняет CI в джобе `delivery`. Прочие наблюдения (§6 «не условия») —
   на усмотрение автора, вливанию не мешают.

---

## §0 — Предмет, взят командами

```
$ git fetch origin; git rev-parse origin/harness/runbook-markers
5cbc5c8833863d8aa1b72cc7b1e29589de51debf
$ git merge-base origin/main origin/harness/runbook-markers
3670d4b3e16bb99dedb3dc4c081256b70a1eedbd
$ git rev-parse origin/main
087ba995d26ff9155bfb56ebc5432ca3e465f14b
$ git log --oneline origin/main..origin/harness/runbook-markers
5cbc5c8 fix(TD-243): R-238 — самоисключение и дословность закреплены; порог префикса, корневой tests/; шапка без ложного утверждения [architect]
3d7dae0 docs(review): R-238 — адверсарий харнесс-трека D6b круг 2 (TD-243) REJECT [architect-adversary]
83fbef8 fix(TD-243): R-236 — сценарии на каждое исключение, регистр, стражи входа, извлечение без склейки; пределы в шапке [architect]
83a149b docs(review): R-236 — адверсарий харнесс-трека D6b (TD-243) REJECT [architect-adversary]
b0bec75 feat(TD-243): D6b — маркеры runbook'а печатает код; проба 12 сценариев (A-047 §2 п.1) [architect]
$ git diff --stat 3670d4b..5cbc5c8 | tail -1
 6 files changed, 979 insertions(+)
```

Отсоединённый worktree `/tmp/hft-arb-markers`; общий чекаут не трогался. Прочитаны целиком:
`scripts/lib/runbook_markers.sh` (80 строк), `scripts/tests/red_runbook_markers.sh` (300),
`R-236`, `R-238`, `docs/workflow/harness-track.md`, `A-047` §2 (с `origin/main`), диф шага
`D6b` в `scripts/verify_delivery_M-08.sh` и шага пробы в `.github/workflows/ci.yml`;
`TECH-DEBT.md` — грепом `TD-243` (`:53`, `:220`, `:5574`). Мутанты строились арбитром заново
(40 штук, каждый — одна-две строки диффа; исключение `m29` — замена `git grep` на `grep -r`,
пять строк), а не брались из вердиктов сторон.

## §1 — Условие APPROVE `R-238`: выполнено

Условие (`R-238` §«Условие APPROVE»): два сценария пробы — маркер только в копии библиотеки;
маркер с метасимволом BRE, отличающийся от кода на этот символ, — и прогон: `m05_no_self_excl`
→ первый FAILED, `m18_regex_grep` → второй FAILED, честная библиотека — 22/22 PASS (у автора
стало 24: добавлены ещё S21/S22 по Н-2/Н-3).

```
$ diff scripts/lib/runbook_markers.sh m05_no_self_excl.sh | grep '^[<>]'
<               ':(exclude)*/tests/*' ':(exclude)tests/**' ':(exclude)scripts/lib/runbook_markers.sh' 2>/dev/null \
>               ':(exclude)*/tests/*' ':(exclude)tests/**' 2>/dev/null \
$ RUNBOOK_MARKERS_LIB=m05_no_self_excl.sh bash scripts/tests/red_runbook_markers.sh | grep -E '^FAIL|^сценариев|^VERDICT'
FAIL  S19 маркер только в файле самой проверки — ожидалось fail, rc=0
сценариев: 24, провалов: 1, каталогов фикстур после прогона: 0
VERDICT: FAIL

$ diff scripts/lib/runbook_markers.sh m09_regex_grep.sh | grep '^[<>]'
<     where=$(git -C "${root}" grep -lF -e "${p}" -- . \
>     where=$(git -C "${root}" grep -l -e "${p}" -- . \
$ RUNBOOK_MARKERS_LIB=m09_regex_grep.sh bash scripts/tests/red_runbook_markers.sh | grep -E '^FAIL|^сценариев|^VERDICT'
FAIL  S20 код отличается на метасимвол (.) — ожидалось fail, rc=0
сценариев: 24, провалов: 1, каталогов фикстур после прогона: 0
VERDICT: FAIL

$ bash scripts/tests/red_runbook_markers.sh | tail -2
сценариев: 24, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
```

Н-1 (`R-238`): фраза шапки `:29-30` теперь «сегодня runbook таких не ЦИТИРУЕТ (в коде такие
есть: `scripts/verify_M-49.sh`)». Обе половины верны по команде:
```
$ git grep -nE '===[^=]*=[^=]+===' -- . ':(exclude)*.md' ':(exclude)docs/**' ':(exclude)research/**' ':(exclude)*/tests/*' ':(exclude)tests/**' ':(exclude)scripts/lib/runbook_markers.sh'
scripts/verify_M-49.sh:210:echo "=== итог: FAIL=$FAILS ==="
$ grep -nE '===[^=`]*=[^=`]+===' deploy/README.md; echo rc=$?
rc=1
```
Н-2 → S21 (`-lt 1` роняет S21), Н-3 → `':(exclude)tests/**'` + S22 (мутант без него роняет
S22). Корневого `tests/` в репозитории нет (`ls -d tests` → отсутствует) — S22 пиннит
исключение на фикстуре, как и положено.

## §2 — Полный перечень проверок библиотеки → мутант → сценарий, который падает

Каждый мутант — одна правка `scripts/lib/runbook_markers.sh` (строки по вершине `5cbc5c8`);
проба — `scripts/tests/red_runbook_markers.sh` без изменений. Колонка «падает» — сценарии
с `FAIL` в выводе пробы (сводка `сценариев: 24, провалов: N`).

| # | проверка библиотеки (строка) | мутант | падает |
|---|---|---|---|
| 1 | страж: нет runbook'а → `return 1` (`:43`) | `m10_missing_ok` — `return 0` | S17 |
| 2 | страж: корень не git → `return 1` (`:47`) | `m11_nogit_ok` — `return 0` | S17b |
| 3 | извлечение `'===[^=`]+==='` (`:50`) | `m13_regex_old` — `[^=]*` | S18, S18b |
| 3a | — бэктик не входит в маркер | `m14_regex_nobt` — `[^=]+` | S18, S18b |
| 3b | — непустое тело (`+`, не `*`) | `m15_regex_star` — `[^=`]*` | S18b |
| 3c | — `sort -u` | `m32_no_sort_u` — убрано | **нет — эквивалентный мутант:** вердикт и rc не меняются, меняется лишь число повторов строк `ok` (на настоящем дереве: 2× `WATCHDOG INSTALL FAILED`) |
| 4 | ноль маркеров → `return 1` (`:53`) | `m12_zero_ok` — `return 0` | S8 |
| 5 | срез по `…` (`:57`) | `m16_no_ellipsis` — `p="${m}"` | S2, S12, S21 |
| 6 | срез по `<` (`:58`) | `m17_no_lt` — строка удалена | S2, S9, S12 |
| 7 | срез по `$` (`:59`) | `m18_no_dollar` — строка удалена | S2 |
| 8 | тело без ведущих `===` (`:60`) | `m19_no_strip_eq` — `body="${p}"` | S9, S21 |
| 9 | тело без ведущих пробелов (`:61`) | `m20_no_trim` — строка удалена | **нет** — эффект равен порогу 2 (см. #10b) |
| 10 | порог длины префикса `-lt 3` (`:62`) | `m21_thr0` — `-lt 0` | S9, S21 |
| 10a | — | `m22_thr1` — `-lt 1` | S21 |
| 10b | — | `m23_thr2` — `-lt 2` | **нет** — проба пиннит «≥ 2», а не «= 3»; величина порога — параметр конструкции (`R-238` Н-2), не отдельная проверка. Фикстура `=== AB… ===` против кода `=== ABSENT ===`: оригинал `miss … короче 3 знаков`, `m23`/`m20` → `ok ← deploy/bin/x.sh` |
| 11 | счётчик `miss` при коротком префиксе (`:64`) | `m27_no_thr_count` — удалён | S9, S21 |
| 12 | дословность — `-F` (`:67`) | `m09_regex_grep` — `grep -l` | S20 |
| 13 | регистр — без `-i` (`:67`) | `m08_icase` — `grep -ilF` | S16 |
| 14 | область — весь репозиторий `-- .` (`:67`) | `m07_allowlist` — `'.github/**' 'crates/**/src/**' 'deploy/bin/**'` | S15 |
| 15 | только отслеживаемые файлы (`git grep`) | `m37_untracked` — `--untracked` | S7 |
| 15a | — | `m29_fs_grep` — `grep -rlF` по ФС с теми же исключениями | S7 |
| 16 | поиск в `root`, а не в cwd (`-C "${root}"`) | `m35_grep_cwd` — `git grep` без `-C` | S2, S15 |
| 17 | исключение `*.md` (`:68`) | `m03_no_md_excl` | S3 S4 S5 S6 S7 S10 S13 S14 S16 S18 S19 S20 S22 |
| 18 | исключение `docs/**` (`:68`) | `m01_no_docs_excl` | S13 |
| 19 | исключение `research/**` (`:68`) | `m02_no_research_excl` | S5, S14 |
| 20 | исключение `*/tests/*` (`:69`) | `m04_no_tests_excl` | S6 |
| 21 | исключение `tests/**` в корне (`:69`) | `m06_no_roottests_excl` | S22 |
| 22 | самоисключение `scripts/lib/runbook_markers.sh` (`:69`) | `m05_no_self_excl` | S19 |
| 23 | `head -1` (`:70`) | `m36_no_head` — убрано | **нет — эквивалентный мутант:** `where` становится многострочным, `-n` истинно так же; на настоящем дереве отличие — лишняя строка пути (`deploy/bin/install-watchdog.sh`) в выводе `ok` |
| 24 | решение `-n "${where}"` (`:71`) | `m28_always_ok` — `if true` | 13 сценариев (как #17) |
| 25 | счётчик `miss` при ненайденном (`:75`) | `m26_no_miss_count` — удалён | 13 сценариев (как #17) |
| 26 | итог «все найдены» `[ miss -eq 0 ]` (`:79`) | `m24_any_found` — `-lt ${#markers[@]}` | S10 |
| 26a | — | `m25_always0` — `true` | 15 сценариев |
| 27 | **проверен КАЖДЫЙ маркер** (цикл `:56`) | `m31_first_only` — `sort -u \| head -1` | S10 |
| 27a | — | `m40_break_first` — `break` в конце тела цикла | S10 |
| 27b | — | **`m30_last_only` — `sort -u \| tail -1`** | **нет — проходит 24/24.** Разбор — §3 |
| 28 | обманные реализации ВНЕ конструкции (расширяют набор срезов) | `m33_cut_paren` — доп. срез по `(`; `m34_trim_trailing` — срез хвостовых пробелов префикса | **нет** — класс открыт (любой доп. символ среза); конструкция `A-047` называет три места подстановки, проба пиннит их присутствие, но не отсутствие четвёртого. Цена `m33` на настоящем дереве реальна: runbook с `(D-COMP-4)` против кода `(D-COMP-3)` → `ok`. Не условие — см. §6 |
| 29 | срез ВСЕГДА (усечение закрывающих `===`, класс `R-231` N-1) | `m39_strip_close` — `p="${p%% ===}"` | S4 |
| 29a | сравнение по укороченному префиксу | `m38_prefix8` — `p="${p:0:8}"` | S3, S4, S20 |

Итого: 40 мутантов; 32 роняют свой сценарий; 2 эквивалентных (`m32`, `m36` — вердикт не
меняется); 2 — величина порога (`m20`, `m23`; классифицировано `R-238` Н-2 как параметр);
2 — вне конструкции (`m33`, `m34`); **1 — проверка внутри конструкции, не запиненная пробой
(`m30`)**; 1 — дубль по смыслу (`m22` = Н-2, пиннится S21). Утверждение коммита `5cbc5c8`
«мутанты: 20 из 20 роняют свой сценарий» — верно для авторского набора; арбитражный набор
шире и находит ровно одну новую дыру.

## §3 — Обманная реализация в пределах конструкции: «только последний маркер»

`m30_last_only` — одна правка `:50`: `| sort -u)` → `| sort -u | tail -1)`. Реализация
извлекает, режет, ищет и агрегирует ровно по конструкции `A-047` §2 п.1 — но для ОДНОГО
маркера. Проба её не отличает от честной:

```
$ RUNBOOK_MARKERS_LIB=m30_last_only.sh bash scripts/tests/red_runbook_markers.sh | tail -2
сценариев: 24, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
```

Почему S10 слеп: его фикстура — `=== GOOD ===` и `=== MISSING ===`; в отсортированном
списке `MISSING` стоит ПОСЛЕДНИМ, и реализация, проверяющая только последний, честно печатает
`miss  === MISSING ===` и возвращает 1. S10 пиннит «только первый» (`m31`, `m40`) и не пиннит
«только последний». `expect` для `want=pass` не смотрит на вывод вовсе — S2 (три маркера) и
S15 (два) не замечают, что проверен один.

Цена на настоящем дереве — ложное зелёное ровно в исходной ситуации `TD-239`:
```
$ printf '\nУспех: ищите `=== WATCHDOG INSTALL OK ===`.\n' >> deploy/README.md
$ source scripts/lib/runbook_markers.sh; runbook_markers_check deploy/README.md . | tail -1; # честная
runbook-markers: маркеров 8, не найдено в коде 1          rc=1
$ source m30_last_only.sh; runbook_markers_check deploy/README.md .                          # обманная
ok    === план ретеншена ===  ← crates/journal/src/bin/journal-retention.rs
runbook-markers: маркеров 1, не найдено в коде 0          rc=0
$ git checkout -- deploy/README.md; git status --porcelain | wc -l
0
```
Латинский подложный маркер сортируется раньше кириллического «план ретеншена» — то есть
ЛЮБОЙ подложный маркер на латинице в сегодняшнем README проходит.

**Форма исправления проверена арбитром** на временной копии пробы (копия удалена, дерево
чистое): после `expect fail "S10 …"` — утверждение по итоговой строке того же вывода
(`${TMP}/out.$((N-1))` — брать ДО вызова `bad`, который сдвигает `N`):
```
$ diff scripts/tests/red_runbook_markers.sh scripts/tests/red_arb_tmp.sh
160a161,165
> if grep -qF 'runbook-markers: маркеров 2, не найдено в коде 1' "${TMP}/out.$((N-1))"; then
>   ok "S10b оба маркера сосчитаны, один не найден"
> else
>   bad "S10b итог не называет «маркеров 2, не найдено в коде 1» — проверены не все маркеры"; …
> fi
$ bash scripts/tests/red_arb_tmp.sh | tail -2                                    # честная
сценариев: 25, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
$ RUNBOOK_MARKERS_LIB=m30_last_only.sh  bash scripts/tests/red_arb_tmp.sh | grep -E '^FAIL|^сценариев'
FAIL  S10b итог не называет «маркеров 2, не найдено в коде 1» — проверены не все маркеры
сценариев: 25, провалов: 1, каталогов фикстур после прогона: 0
$ RUNBOOK_MARKERS_LIB=m31_first_only.sh bash scripts/tests/red_arb_tmp.sh | grep -cE '^FAIL'   → 2 (S10, S10b)
$ RUNBOOK_MARKERS_LIB=m40_break_first.sh bash scripts/tests/red_arb_tmp.sh | grep -cE '^FAIL'  → 2 (S10, S10b)
$ RUNBOOK_MARKERS_LIB=m24_any_found.sh  bash scripts/tests/red_arb_tmp.sh | grep -cE '^FAIL'   → 1 (S10)
$ rm scripts/tests/red_arb_tmp.sh; git status --porcelain | wc -l
0
```
Утверждение по итоговой строке пиннит сразу обе величины — «сосчитаны оба» и «не найден
ровно один», — поэтому закрывает и `tail -1`, и `head -1`, и `break`, и любую выборку
подмножества. Итоговая строка `runbook-markers: маркеров N, не найдено в коде M` уже
печатается библиотекой (`:78`) и уже используется `D6b` как текст `pass` — формат не новый.

Других обманных реализаций внутри конструкции арбитр не построил: проверялись укороченный
префикс (`m38` → S3/S4/S20), всегда-срез закрывающих `===` (`m39` → S4), поиск по ФС вместо
индекса (`m29` → S7), `--untracked` (`m37` → S7), поиск в cwd (`m35` → S2/S15), «хоть один
найден» (`m24` → S10), перевёрнутое решение (`m28`) — все красные.

## §4 — Настоящий runbook, прод-форма `D6b`, уборка, дерево слияния

```
$ grep -oE '===[^=`]+===' deploy/README.md | sort -u | wc -l
7
$ source scripts/lib/runbook_markers.sh; runbook_markers_check deploy/README.md .; echo rc=$?
ok    === DEPLOY FAILED — logs + rollback to … ===  ← .github/workflows/deploy.yml
ok    === DEPLOY FAILED — logs + rollback to <sha> ===  ← .github/workflows/deploy.yml
ok    === healthy (recorder + gateway-serve) — deployed … ===  ← .github/workflows/deploy.yml
ok    === WATCHDOG INSTALL FAILED ===  ← .github/workflows/deploy.yml
ok    === компакция закрытых сегментов (D-COMP-3) ===  ← crates/journal/src/bin/journal-retention.rs
ok    === отчёт ===  ← crates/journal/src/bin/journal-retention.rs
ok    === план ретеншена ===  ← crates/journal/src/bin/journal-retention.rs
runbook-markers: маркеров 7, не найдено в коде 0
rc=0

$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D6b|DELIVERY'; echo exit=${PIPESTATUS[0]}
PASS  D6b маркеры deploy/README.md печатаются кодом (runbook-markers: маркеров 7, не найдено в коде 0)
DELIVERY: PASS
exit=0
```

Дерево слияния с ТЕКУЩИМ `origin/main` (`087ba99`, ушёл от базы на 3 коммита; `git merge
--no-edit 5cbc5c8` во временном worktree, merge-коммит `cc959c6`, родители `087ba99 5cbc5c8`):
```
$ bash scripts/tests/red_runbook_markers.sh | tail -2
сценариев: 24, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D6b|DELIVERY'; echo exit=${PIPESTATUS[0]}
PASS  D6b маркеры deploy/README.md печатаются кодом (runbook-markers: маркеров 7, не найдено в коде 0)
DELIVERY: PASS
exit=0
$ printf '\nУспех: ищите `=== WATCHDOG INSTALL OK ===` в логе.\n' >> deploy/README.md
$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D6b|miss|DELIVERY'; echo exit=${PIPESTATUS[0]}
FAIL  D6b deploy/README.md велит искать в логе маркер, которого код не печатает (TD-243):
      miss  === WATCHDOG INSTALL OK ===  — префикс «=== WATCHDOG INSTALL OK ===» не печатается ни одним файлом кода
DELIVERY: FAIL (1)
exit=1
$ git checkout -- deploy/README.md; git status --porcelain | wc -l
0
$ T=${TMPDIR:-/tmp}; b=$(ls -d $T/tmp.* | wc -l); bash scripts/tests/red_runbook_markers.sh >/dev/null; a=$(ls -d $T/tmp.* | wc -l); echo before=$b after=$a delta=$((a-b))
before=6421 after=6421 delta=0
```
Уборка: печать пробы «каталогов фикстур после прогона: 0» (считается, `:296-298`) и внешний
замер — дельта 0 (`TMPDIR` здесь не `/tmp`, замер взят по фактическому `TMPDIR`).

## §5 — Перепроверка `gates.md` §9

**(а) Утверждения о коде — командой.** Шапка библиотеки: `:16-18` перечень исключений =
шести `:(exclude)` в `:68-69` (`grep -oE "':\(exclude\)[^']+'"` → `*.md docs/** research/**
*/tests/* tests/** scripts/lib/runbook_markers.sh`); `:26-28` разрыв переносом не извлекается,
соседний маркер проверяется — S18/S18b честной библиотекой; `:29-30` — §1 выше (обе половины
верны); `:31-32` ASCII `...` в README не встречается (`grep -nE '===[^=`]*\.\.\.[^=`]*==='
deploy/README.md` → rc=1); `:33` регистрозависимость — `grep -lF` без `-i` и
`deploy/README.md:130` (`grep -F '=== DEPLOY FAILED'`). Комментарий `D6b`
(`verify_delivery_M-08.sh:266`) «D6 видит лишь наличие ключевых слов» — верно: `D6`
(`:258`) — `grep -qi 'storage box\|/mnt/journal-cold'`. Шаг пробы стоит в джобе `delivery`
(`ci.yml:46-47`), джоб входит в агрегат `All checks passed` (`ci.yml:630`, `needs: […,
delivery, …]`) — утверждение `A-047` §2 п.1 «без правки агрегата» выполнено. Шапка пробы `:7`
«число сценариев СЧИТАЕТСЯ прогоном» — `N` инкрементируется в `ok`/`bad` (`:16-17`), итог
печатается из `N` (`:297`). `verify_design_claims.sh --merge-preview origin/main` → `VERDICT:
PASS (0 нарушений)`. Утверждение коммита «честная библиотека 24/24» — верно; «20 из 20
мутантов» — верно для набора автора, см. §2 итог.

**(б) Полномочия.** Диф — `scripts/lib/**`, `scripts/tests/**`, `scripts/verify_*.sh`,
`.github/workflows/ci.yml`, `research/reviews/**`: зона architect'а (`scope-guard.md`) и зона
трека (`harness-track.md` §2); `crates/**`, `contracts/**`, документы норм не тронуты — трек
применим (§4 трека: три вопроса — три «нет»). Замок §11 не задет:
`EVENT_NAME=pull_request PR_BASE_SHA=3670d4b… check_docs_freeze.sh` → exit=0.
`check_protected_artifacts.sh 3670d4b HEAD` → OK. `check_roadmap_sync.sh` в PR-форме →
`VERDICT: PASS`. Граница C не затронута. Мерж в треке делает автор-architect
(`harness-track.md` §3) — исполнитель есть.

**(в) Ссылки.** `R-236`, `R-238`, `A-047` — существуют на ветке/`origin/main`; `TD-243` на
`origin/main` (`TECH-DEBT.md`, 3 вхождения); `# shellcheck source=` в пробе (`:23`) и в verify
(`:271`) резолвятся в `scripts/lib/runbook_markers.sh`. `check_artifact_ids.sh` — прогнан после
коммита этого файла, в Done Block.

**FA:** диф вне `crates/**` — `review-fa` даёт SKIP; инвариант крейта не называется по той же
причине.

## §6 — РЕШЕНИЕ

**Условие APPROVE `R-238` выполнено.** Два REJECT'а подряд закрыты фактом, а не словом:
`m05` → S19, `-F` → S20, шапка верна по команде. Стандарт обоих кругов — `harness-track.md`
§5 п.2 «нейтрализация КАЖДОЙ проверки роняет свой сценарий» — применён арбитром к полному
перечню (§2); он держится везде, кроме одной агрегационной проверки (§3), которую стороны
не рассматривали ни в одном круге.

**Доделать ровно одно, затем вливать — без нового круга адверсария:**

1. `scripts/tests/red_runbook_markers.sh`, после `expect fail "S10 …"` (`:160`): утверждение
   `S10b` по итоговой строке вывода S10 — `grep -qF 'runbook-markers: маркеров 2, не найдено в
   коде 1' "${TMP}/out.$((N-1))"` → `ok`, иначе `bad` (имя файла вычислить в переменную ДО
   вызова `bad`: он инкрементирует `N`; тот же сдвиг индекса сидит в S18b `:242` — при провале
   `sed` укажет на несуществующий `out.*`; поправить попутно или оставить, вливанию не мешает).
   Библиотека не меняется.
2. В теле коммита — сырой вывод: мутант `| sort -u | tail -1` → `S10b FAILED`
   (`провалов: 1`); честная библиотека → `сценариев: 25, провалов: 0, VERDICT: PASS`.

Основание «без нового круга»: форма правки проверена здесь исполнением (§3) — она роняет
`m30`/`m31`/`m40` и не трогает честную реализацию; CI в джобе `delivery` гоняет позитивный
контроль на чистом чекауте при push'е; три круга подряд ловили один и тот же класс, и
четвёртый на четырёхстрочную правку стоил бы дороже предмета — ровно то, против чего заведён
трек (`harness-track.md` §1).

**Не условия (на усмотрение автора; могут ехать этим же коммитом или не ехать вовсе):**

- Порог префикса пиннится как «≥ 2», не «= 3» (`m23`, `m20`; §2 #9/#10b). Если величина 3
  важна — фикстура `=== AB… ===` против `=== ABSENT ===` → FAIL. `R-238` Н-2 уже отнёс это к
  параметрам конструкции; арбитр согласен.
- Дополнительные срезы вне конструкции (`m33` по `(`, `m34` хвостовые пробелы; §2 #28)
  пробой не исключаются, и класс открыт. Один сценарий «маркер с `(…)`, код отличается
  внутри скобок → FAIL» пиннил бы самый правдоподобный случай (настоящий README такой маркер
  содержит: `(D-COMP-3)`). Не требование: конструкция `A-047` называет три места подстановки,
  и расширять её арбитр не вправе.
- `sort -u` и `head -1` — эквивалентные мутанты (§2 #3c/#23): пиннить их нечем и незачем;
  записано, чтобы следующий круг не считал их «непокрытыми проверками».

**Несогласие сторон** не зафиксировано; решение обязательно к исполнению (`gates.md` §0).

## Done Block

```
$ pwd; git rev-parse HEAD; git status --porcelain | wc -l
/tmp/hft-arb-markers
5cbc5c8833863d8aa1b72cc7b1e29589de51debf
0

$ bash scripts/tests/red_runbook_markers.sh | tail -2
сценариев: 24, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS

$ # 40 мутантов арбитра — сводка (полная таблица — §2)
m01_no_docs_excl       провалов: 1  [S13]
m02_no_research_excl   провалов: 2  [S5 S14]
m03_no_md_excl         провалов: 13 [S3 S4 S5 S6 S7 S10 S13 S14 S16 S18 S19 S20 S22]
m04_no_tests_excl      провалов: 1  [S6]
m05_no_self_excl       провалов: 1  [S19]        ← условие R-238 (а)
m06_no_roottests_excl  провалов: 1  [S22]
m07_allowlist          провалов: 1  [S15]
m08_icase              провалов: 1  [S16]
m09_regex_grep         провалов: 1  [S20]        ← условие R-238 (б)
m10_missing_ok         провалов: 1  [S17]
m11_nogit_ok           провалов: 1  [S17b]
m12_zero_ok            провалов: 1  [S8]
m13_regex_old          провалов: 2  [S18 S18b]
m14_regex_nobt         провалов: 2  [S18 S18b]
m15_regex_star         провалов: 1  [S18b]
m16_no_ellipsis        провалов: 3  [S2 S21 S12]
m17_no_lt              провалов: 3  [S2 S9 S12]
m18_no_dollar          провалов: 1  [S2]
m19_no_strip_eq        провалов: 2  [S9 S21]
m20_no_trim            провалов: 0  VERDICT: PASS   ← величина порога (= -lt 2)
m21_thr0               провалов: 2  [S9 S21]
m22_thr1               провалов: 1  [S21]
m23_thr2               провалов: 0  VERDICT: PASS   ← величина порога
m24_any_found          провалов: 1  [S10]
m25_always0            провалов: 15
m26_no_miss_count      провалов: 13
m27_no_thr_count       провалов: 2  [S9 S21]
m28_always_ok          провалов: 13
m29_fs_grep            провалов: 1  [S7]
m30_last_only          провалов: 0  VERDICT: PASS   ← §3, условие §6 п.1
m31_first_only         провалов: 1  [S10]
m32_no_sort_u          провалов: 0  эквивалентный
m33_cut_paren          провалов: 0  VERDICT: PASS   ← вне конструкции
m34_trim_trailing      провалов: 0  VERDICT: PASS   ← вне конструкции
m35_grep_cwd           провалов: 2  [S2 S15]
m36_no_head            провалов: 0  эквивалентный
m37_untracked          провалов: 1  [S7]
m38_prefix8            провалов: 3  [S3 S4 S20]
m39_strip_close        провалов: 1  [S4]
m40_break_first        провалов: 1  [S10]

$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D6b|DELIVERY'; echo exit=${PIPESTATUS[0]}
PASS  D6b маркеры deploy/README.md печатаются кодом (runbook-markers: маркеров 7, не найдено в коде 0)
DELIVERY: PASS
exit=0

$ # дерево слияния origin/main(087ba99) + 5cbc5c8 → cc959c6
сценариев: 24, провалов: 0, каталогов фикстур после прогона: 0 / VERDICT: PASS
PASS  D6b … (runbook-markers: маркеров 7, не найдено в коде 0) / DELIVERY: PASS / exit=0
подложный маркер → FAIL D6b … miss === WATCHDOG INSTALL OK === / DELIVERY: FAIL (1) / exit=1
before=6421 after=6421 delta=0

$ EVENT_NAME=pull_request PR_BASE_SHA=3670d4b3e16bb99dedb3dc4c081256b70a1eedbd bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0
$ bash scripts/check_protected_artifacts.sh 3670d4b HEAD | tail -1
OK: защищённые артефакты целы на HEAD (3670d4b..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
$ EVENT_NAME=pull_request PR_BASE_SHA=3670d4b3e16bb99dedb3dc4c081256b70a1eedbd bash scripts/check_roadmap_sync.sh | tail -1
VERDICT: PASS
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -1
VERDICT: PASS (0 нарушений)
$ bash scripts/next_artifact_id.sh A
A-048
```
