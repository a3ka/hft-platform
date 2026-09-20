# R-020 — PR-гейт: `feat/gate-rfc-claims` @ `1857c14` (проверки 6/7 — SHA и пути в `docs/rfc/**`)

- **Дата (UTC):** 2026-08-02
- **Роль:** reviewer
- **Ветка:** `feat/gate-rfc-claims` @ `1857c14` (2 коммита поверх базы `4ccefbc`; `main` @ `51e4023` ушёл вперёд на 11 коммитов)
- **Диф:** `scripts/verify_design_claims.sh` (+279/−1), `scripts/tests/red_verify_design_claims.sh` (+200)
- **Вердикт: CHANGES REQUESTED** — один блокер (**B-1**), пять непирающих замечаний (N-1…N-5).
- **Merge НЕ выполнен.**

---

## Блоки гейта (`gates.md` §4)

### Block-scope — PASS

`git diff --name-status origin/main...HEAD` → ровно два файла, оба в `scripts/**`. Ни
`crates/`, ни `contracts/`, ни `docs/`, ни `*/tests/` крейтов не тронуты. Оба файла —
зона architect'а (`scripts/verify_*.sh` + `*/tests/` RED-спеки, `scope-guard.md`).
Авторство обоих коммитов — `architect <architect@noreply.local>`; conventional-формат,
ссылка на предметную область в scope (`docs-gate`), без co-author трейлеров. Атомарность
соблюдена: движок и его проба — разные коммиты.

Risk-блок **не применяется**: диф не трогает `risk`/`killswitch`/`oms`/`venue-*`/`contracts`
— это shell/python-гейт над документами. Block-C не применяется: `contracts/` не тронут.

### Block-DoneBlock — см. секцию «Done Block» ниже (прогоны reviewer'а, не перенос из отчёта)

### Форма гейта (`gates.md` §3) — PASS

- `set -uo pipefail` + явная агрегация: FAIL-счётчик и `VERDICT:` печатает python-движок,
  его exit-код становится exit-кодом скрипта (проверено: PASS→0, FAIL→1).
- Конструкции `cmd && echo PASS || echo FAIL` в диффе отсутствуют.
- Финальная строка `VERDICT: PASS`/`VERDICT: FAIL (N нарушений)` соответствует exit-коду —
  подтверждено обоими прогонами.
- Setup-guard'ы `--merge-preview` (не git-репо / base-ref не резолвится / HEAD не резолвится
  / worktree не собрался / merge конфликтует) дают `FAIL [SETUP]` + exit 1, а не тихий PASS.
  Сценарии 9 и 10 self-теста это проверяют.
- Отсутствие `exec` перед `python3` (чтобы trap EXIT снёс временный worktree) —
  осознанно, прокомментировано; утечка worktree проверяется сценарием 8b.

### `--merge-preview` — PASS (урок R-013 Б-2/Б-3 соблюдён)

Проверено ЗАМЕРОМ, что режим смотрит дерево СЛИЯНИЯ, а не ветку. На ветке проверка 6
находит **2** цитаты коммитов и проверка 7 — **31** путь; в `--merge-preview origin/main`
те же проверки видят **17** и **67** соответственно (на `main` появился
`docs/rfc/CT-RFC-05-margin-inventory.md`, которого на ветке нет). То есть содержимое
merge-цели действительно читается. `canonical_refs()` добавляет `MERGE_HEAD` внутри превью
— в выводе `историю HEAD/MERGE_HEAD`, ложного FAIL на коммиты со стороны сливаемого HEAD
не возникает.

### Ложных срабатываний на merge-цели — НЕТ

`bash scripts/verify_design_claims.sh --merge-preview origin/main` → `VERDICT: PASS
(0 нарушений)`, exit=0. Гейт не покраснеет на уже смёрженных документах — блокера
«гейт ломает CI всем» нет.

Отдельно (**N-5**, не дефект диффа): прогон на ГОЛОМ дереве ветки даёт `VERDICT: FAIL
(2 нарушений)` — обе на проверке 4 (`docs/ORCHESTRATION-STATE.md:223` и `:245` ссылаются на
несуществующий `docs/rfc/CT-RFC-06-l2delta.md`). Это состояние базы ветки (`4ccefbc`),
проверка 4 существовала на `main` до диффа, и на merge-цели дефекта нет. К диффу претензии
не относится.

### Анти-плацебо — self-test КРАСНЕЕТ на сломанной реализации (мутационный контроль пройден)

Три независимые мутации движка (копии скрипта, реальный файл не трогался):

| Мутация | Что сломано | Результат self-теста |
|---|---|---|
| M1 | `git_commit_exists()` → всегда `True` | **FAIL** `сценарий RFC-SHA-fake` (ожидался FAIL про «не найден в git-объектах», пришёл другой) |
| M2 | `git_commit_is_ancestor_of_any()` → всегда `True` | **FAIL** `сценарий RFC-SHA-orphan`, exit сценария стал 0 |
| M3 | проверка `os.path.exists(candidate)` в check7 → выключена | **FAIL** `сценарий RFC-PATH-fake`, exit сценария стал 0 |

Ни одна мутация не осталась незамеченной; оракул связан с кодом и различает ПРИЧИНУ отказа
(M1 не проскочил только потому, что тест сверяет текст сообщения, а не один лишь exit-код).
Ключевой сценарий `RFC-SHA-orphan` воспроизводит реальный класс `C-044` F1 (объект
существует на несмёрженной ветке, но не ancestor) и краснеет ровно на M2 — «`cat-file -e`
без `--is-ancestor`» как плацебо действительно закрыт.

---

## B-1 (БЛОКЕР) — детектор ключуется на рукописном списке русских синонимов; на реальном корпусе он уже обойдён, и обход НЕ РЕПОРТИТСЯ

**Что.** Проверка 6 смотрит hex-токен только если в ТОМ ЖЕ параграфе есть слово-маркер из
`SHA_CONTEXT_RE = коммит\w*|merge\b|мёрж\w*|мерж\w*|\bsha\b`. Токены без маркера
**не проверяются и не считаются** — ни строкой INFO, ни числом в отчёте. Итоговая строка
пишет «все N цитат коммитов … существуют И входят в историю», где N — только проверенные.

**Где.** `scripts/verify_design_claims.sh`: `SHA_CONTEXT_RE` (объявление), `gather_sha_refs()`
(фильтр `para_has_ctx`), `check6()` (сообщения PASS/INFO — остаток нигде не учитывается).

**Симптом на РЕАЛЬНОМ корпусе (замер, merge-цель `origin/main`).** В
`docs/DESIGN.md` + `docs/rfc/*.md` — 20 hex-токенов в backtick'ах; гейт проверяет **17**,
молча пропускает **3**, из них два — нормативные утверждения о коммитах ровно в том
документе, из-за которого гейт и строился:

```
SKIP-NOCTX docs/rfc/CT-RFC-05-margin-inventory.md:77  b3a5a95  | «Это подтверждено отдельным ИСПРАВЛЕНИЕМ `b3a5a95` …»
SKIP-NOCTX docs/rfc/CT-RFC-05-margin-inventory.md:163 41d3526  | «… reviewer close-out (`41d3526`) …»
SKIP-NOCTX docs/DESIGN.md:318                          cc5197c  | строка таблицы фаз §10
ВСЕГО hex-токенов=20 проверено=17 молча пропущено=3
```

**Воспроизведение (30 секунд).** Синтетический репозиторий, один RFC с двумя ВЫДУМАННЫМИ SHA:

```
$ cat docs/rfc/CT-RFC-99-probe.md
# CT-RFC-99 — проба формы «подтверждено исправлением»
Это подтверждено отдельным исправлением `0000000deadbee` («fix(M-99): нечто»),
которого в репозитории не существует вовсе.
Здесь же close-out ревьюера (`1111111`) — тоже выдуманный.

$ bash scripts/verify_design_claims.sh $D | grep RFC-SHA
INFO  [6-RFC-SHA] в docs/DESIGN.md и docs/rfc/**.md не найдено цитат коммитов (SHA в
      контексте «коммит»/«merge»/«мёрж...») — проверка неприменима

$ sed -i 's/отдельным исправлением/отдельным коммитом/' docs/rfc/CT-RFC-99-probe.md
$ bash scripts/verify_design_claims.sh $D | grep RFC-SHA
FAIL  [6-RFC-SHA] docs/rfc/CT-RFC-99-probe.md:3: цитируется коммит `0000000deadbee` — не
      найден в git-объектах репозитория вовсе
```

Одна и та же ложь ловится или не ловится в зависимости от выбранного автором синонима.
В первом случае гейт не просто молчит — он печатает **«проверка неприменима»** на документ,
целиком состоящий из выдуманных SHA. Второй токен (`1111111`, «close-out ревьюера») не
ловится и после правки: он в соседнем параграфе, маркера там нет.

**Почему это блокер, а не долг.**
1. Это fail-open в fail-closed гейте, и он **не латентный**: форма-обход присутствует в
   текущем корпусе (2 из 17 нормативных цитат) и возникает не от злого умысла, а от обычного
   русского синонима («исправлением», «close-out», «влито», «правкой»). Прецедент TD-060
   был принят как долг именно потому, что достижимых форм замерено **0**; здесь замер даёт 2.
2. Нарушен собственный принцип скрипта, записанный в его же шапке: «каждый парсер обязан
   ЗНАТЬ, когда он ничего не нашёл, и это FAIL, а не пустой отчёт» (там же — setup-guard,
   урок M-40). Здесь парсер не знает и не сообщает: остаток нигде не появляется.
   То же — свойство (4) «Целостность гейта» (`testing.md`): гейт обязан наблюдать
   ОТСУТСТВИЕ, не только сбой.
3. Отчёт активно вводит в заблуждение: строка «все 17 цитат коммитов … существуют И входят
   в историю» будет цитироваться как «все SHA документа проверены». После merge'а этот
   PASS станет пруфом в close-out'ах — ровно тот способ, которым `C-044` и случился.

**Граница роли.** Как чинить — зона architect'а (reviewer описывает, не проектирует;
`gates.md` §4). Формулирую только требование к результату: **остаток обязан быть виден**
— каждый hex-токен SHA-формы вне фенсов либо проверен, либо явно перечислен в отчёте как
непроверенный с причиной; «не нашёл цитат» не должно печататься на документе, где
SHA-подобные токены есть. Оракул на это свойство обязан краснеть на текущей реализации
(сегодня ни один из 8 новых сценариев не краснеет — сценарий `RFC-SHA-no-context`
закрепляет пропуск как ЖЕЛАЕМОЕ поведение, не проверяя, что пропуск объявлен).

---

## Непирающие замечания

### N-1 (MAJOR-долг, не блокер) — check7: тот же молчаливый остаток на путях

Whitelist префиксов `crates|docs|scripts|research|milestones|\.claude` не покрывает
крейт-относительную форму, которой реальные RFC пользуются свободно. Замер по
`docs/rfc/*.md` на merge-цели: гейт проверяет **67** путей, молча пропускает **49**
(большая часть — законно: glob'ы, `Ord/Risk/Ctl`, одиночные `/` в прозе), но среди
пропущенных есть настоящие ссылки на файлы:

```
docs/rfc/CT-RFC-01-market-data-expansion.md:23  `contracts/src/lib.rs:46`
docs/rfc/CT-RFC-01-market-data-expansion.md:63  `recorder/src/main.rs:58`
docs/rfc/CT-RFC-02-journal-provenance.md:127    `tests/red_schema.rs`
docs/rfc/CT-RFC-04-l2delta.md:45,125            `journal/src/segments.rs`
```

Severity ниже B-1 (опечатка в пути дешевле выдуманного SHA-пруфа), но класс тот же:
остаток не объявлен. Логично закрывать вместе с B-1.

### N-2 (MINOR) — шапка обещает `docs/rfc/**.md`, код читает `os.listdir` (без рекурсии)

`check6()`/`check7()` обходят только верхний уровень `docs/rfc/`; подкаталогов сегодня нет
(`find docs/rfc -type d` → один `docs/rfc`), поэтому поведение сейчас верное. Но документ
гейта утверждает `**`, а код делает `*` — ровно тот класс расхождения «документ говорит не
то, что делает код», ради которого гейт и написан.

### N-3 (MINOR, латентно) — чисто-цифровой токен 7–40 знаков считается SHA

`[0-9a-f]{7,40}` покрывает и десятичные литералы. В проекте fixed-point ×1e8 — константа
вида `` `100000000` `` в параграфе со словом «коммит»/«merge» даст ЛОЖНЫЙ FAIL. Замером
сегодня: единственный чисто-цифровой токен в `docs/DESIGN.md`+`docs/rfc/` — `` `0999929` ``,
и это настоящий коммит. Риск латентный, срабатываний нет.

### N-4 (процесс) — RED-first: коммит реализации РАНЬШЕ коммита пробы

```
c3757bc 2026-08-02 11:16:15 feat(docs-gate): …  → scripts/verify_design_claims.sh
1857c14 2026-08-02 11:16:28 test(docs-gate): …  → scripts/tests/red_verify_design_claims.sh
```

Порядок обратный требуемому (`gates.md` §2, `testing.md`: тест — спецификация, пишется ДО).
Формально зона одна (architect владеет и гейтом, и его пробой), и мутационный контроль
показал, что оракул РАБОЧИЙ — поэтому не блокер сам по себе. Но следствие видно в B-1:
восемь новых сценариев описывают то, ЧТО ПОЛУЧИЛОСЬ (включая сценарий
`RFC-SHA-no-context`, закрепляющий слепое пятно как норму), а не то, что требовалось —
«выдуманный SHA в RFC не проходит гейт». Это ровно механизм находки F-4 из `R-005`.

### N-5 (не дефект диффа) — голое дерево ветки красное

См. выше в блоке «ложные срабатывания»: 2 FAIL проверки 4 на `docs/ORCHESTRATION-STATE.md`
приходят из базы ветки `4ccefbc`; на merge-цели их нет.

### N-6 (справочно, к TD «гейт не в CI») — исходная причина отсрочки исчезла

`R-016` N-1 отложил подключение гейта к `ci.yml` формулировкой «подключить СЕЙЧАС нельзя:
на `main` гейт красный». Замером сегодня: старый (main-овский) движок на дереве `main` —
`VERDICT: PASS (0 нарушений)` exit=0; новый в `--merge-preview origin/main` — тоже PASS.
`grep -n "verify_design_claims" .github/workflows/*.yml` по-прежнему пуст. Долг стал
исполнимым; это отдельная работа, не предмет данного PR.

---

## Done Block (прогоны reviewer'а в собственном worktree `/tmp/hft-rev-gate-rfc`)

```
$ git -C /tmp/hft-rev-gate-rfc log --format='%h %an <%ae> %s' -2
1857c14 architect <architect@noreply.local> test(docs-gate): self-test на выдуманный SHA и несуществующий путь
c3757bc architect <architect@noreply.local> feat(docs-gate): проверка существования цитируемых SHA

$ git diff --name-status origin/main...HEAD
M	scripts/tests/red_verify_design_claims.sh
M	scripts/verify_design_claims.sh

$ git diff --stat origin/main...HEAD
 scripts/tests/red_verify_design_claims.sh | 200 +++++++++++++++++++++
 scripts/verify_design_claims.sh           | 280 +++++++++++++++++++++++++++++-
 2 files changed, 479 insertions(+), 1 deletion(-)

$ bash scripts/tests/red_verify_design_claims.sh | tail -12; echo exit=$?
PASS  сценарий RFC-SHA-real (реальный SHA в docs/rfc/, C-044): гейт даёт PASS [6-RFC-SHA], VERDICT: PASS, exit=0
PASS  сценарий RFC-SHA-fake (выдуманный SHA 0000000 в docs/rfc/, C-044 F1): гейт даёт FAIL [6-RFC-SHA], exit=1
PASS  сценарий RFC-SHA-orphan (SHA — реальный git-объект вне ancestry HEAD, C-044 F1 класс): гейт даёт FAIL [6-RFC-SHA], exit=1
PASS  сценарий RFC-SHA-no-context (hex-токен без слова-маркера коммита): гейт НЕ падает, exit=0
PASS  сценарий RFC-PATH-real (реальный путь в docs/rfc/): гейт даёт PASS [7-RFC-PATH], VERDICT: PASS, exit=0
PASS  сценарий RFC-PATH-section-tail (хвост §N/::func/:NNN внутри backtick'ов отброшен): гейт даёт PASS [7-RFC-PATH], exit=0
PASS  сценарий RFC-PATH-fake (несуществующий путь в docs/rfc/, C-044 F2 класс): гейт даёт FAIL [7-RFC-PATH], exit=1
PASS  сценарий RFC-PATH-glob (glob/brace-паттерн — не литеральный путь): гейт НЕ падает, exit=0

VERDICT: PASS
exit=0
(всего 27 сценариев, FAIL — 0)

$ bash scripts/verify_design_claims.sh | tail -4; echo exit=$?     # голое дерево ветки
FAIL  [4-МЁРТВЫЕ-ФАЙЛЫ] docs/ORCHESTRATION-STATE.md:223: ссылка на `docs/rfc/CT-RFC-06-l2delta.md` — файл не существует
FAIL  [4-МЁРТВЫЕ-ФАЙЛЫ] docs/ORCHESTRATION-STATE.md:245: ссылка на `docs/rfc/CT-RFC-06-l2delta.md` — файл не существует
PASS  [6-RFC-SHA] все 2 цитат коммитов (docs/DESIGN.md + docs/rfc/**.md) существуют И входят в историю HEAD
PASS  [7-RFC-PATH] все 31 путей, процитированных в docs/rfc/**.md, существуют в дереве репозитория
VERDICT: FAIL (2 нарушений)
exit=1
(N-5: обе находки — состояние базы ветки, не диффа)

$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -4; echo exit=$?
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 123 ссылок вида docs/*.md указывают на существующие файлы
PASS  [6-RFC-SHA] все 17 цитат коммитов (docs/DESIGN.md + docs/rfc/**.md) существуют И входят в историю HEAD/MERGE_HEAD
PASS  [7-RFC-PATH] все 67 путей, процитированных в docs/rfc/**.md, существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ # мутационный контроль (копии скрипта; verify_design_claims.sh не изменялся)
$ BARRIER=…/verify_MUT_M1.sh bash scripts/tests/red_verify_design_claims.sh | grep -E '^(FAIL|VERDICT)'
FAIL  сценарий RFC-SHA-fake: ОЖИДАЛСЯ FAIL [6-RFC-SHA] на 0000000, получено (exit=1):
VERDICT: FAIL (1 нарушений)
$ BARRIER=…/verify_MUT_M2.sh bash scripts/tests/red_verify_design_claims.sh | grep -E '^(FAIL|VERDICT)'
FAIL  сценарий RFC-SHA-orphan: ОЖИДАЛСЯ FAIL [6-RFC-SHA] «НЕ входит в историю» …, получено (exit=0):
VERDICT: FAIL (1 нарушений)
$ BARRIER=…/verify_MUT_M3.sh bash scripts/tests/red_verify_design_claims.sh | grep -E '^(FAIL|VERDICT)'
FAIL  сценарий RFC-PATH-fake: ОЖИДАЛСЯ FAIL [7-RFC-PATH], получено (exit=0):
VERDICT: FAIL (1 нарушений)

$ git status --porcelain      # мутанты удалены, дерево чистое
{пусто}

$ grep -n "verify_design_claims" .github/workflows/*.yml; echo rc=$?
rc=1
```

Post-merge деплой-гейт (`gates.md` §8) **не выполнялся** — merge не делался (CHANGES REQUESTED).

---

## Что нужно для APPROVED

1. **B-1** — остаток проверки 6 обязан быть виден в отчёте; «не найдено цитат» не печатается
   на документе, где SHA-подобные токены есть. Дизайн — architect. Оракул на это свойство
   обязан краснеть на текущей реализации `1857c14`.
2. **N-1** — то же для проверки 7 (или явное обоснование, почему остаток путей допустимо
   не объявлять).
3. N-2/N-3/N-4 — по усмотрению architect'а; блокирующими не считаю.

Всё остальное (scope, форма гейта, `--merge-preview`, отсутствие ложняков на merge-цели,
анти-плацебо самого self-теста) — **проверено и принято**.
