<!-- GATE-META
milestone: A-010
audited_repo: a3ka/hft-platform
audited_base: 1a0270e34827c0a5997d746e8d9b6f34744d12a4
audited_head: 1a0270e34827c0a5997d746e8d9b6f34744d12a4
verdict: NOTE
-->

# R-095 — перепроверка удаления и вливания веток: NOTE при условиях У-1…У-6

**Роль:** независимый architect-клон, свежий контекст, read-only на общем чекауте
(`/home/nous/hft-platform` не тронут; работа в `/tmp/hft-rc-verdict`, `/tmp/hft-rc-preview`).
**Дата (UTC):** 2026-08-19
**Предмет:** `docs/plans/branch-inventory-2026-08-19.md` — живёт в `main` на
`1a0270e34827c0a5997d746e8d9b6f34744d12a4`; `audited_base == audited_head`, потому что
предмет уже влит (PR #38) и судится в том виде, в каком его читает следующая сессия.

**Почему `milestone: A-010`, и где это НЕ точно.** Разбор веток исполняет §B
`docs/plans/plan-branches-and-ci-2026-08-19.md`; управляющие решения по кластерам M-65/M-66
и по процессной ветке даёт арбитраж `A-010` (§A, §B, §I-4), на который инвентарь ссылается в
Cross-references. Оговорка, которую я обязан назвать: §B САМОГО `A-010` посвящён M-66
(«красный acceptance из-за флака в чужом крейте»), а не инвентарю веток — совпадение буквы
секции случайно. Привязка к `A-010` верна как к управляющему решению, не как к секции.

**Метод.** Каждый пункт, от которого зависит НЕОБРАТИМОЕ действие, воспроизведён своей
командой на `origin/main` = `1a0270e`. Слияния проверялись на ДЕРЕВЕ СЛИЯНИЯ
(`gates.md` §8, `strict:false`), а не на ветке и не по `gh pr checks`. Отчёты двух
адверсариев приняты как ГИПОТЕЗЫ: шесть их утверждений подтвердились, два оказались неполными
(§6).

**Предъявление FA (M-66).** Предмет трогает `crates/research-cli/**` (черновик `salvage/M-59`),
`crates/journal/**`, `crates/gateway/**`, `crates/research-cli/**` (тест-файлы PR #28). Живые
инварианты на этой ревизии: `RC-I-2` (trials-ledger append-only, `docs/fa/research-cli.md:198`),
`JR-I-2` (`seq` строго монотонен без дыр, `docs/fa/journal.md:112`), `VB-I-5`
(серия глубже 1.3 % несёт `depth_band_provenance`, `docs/fa/viz-backend.md:119`).
`RC-I-2` — не украшение: именно он определяет, чем является перенос 276 записей ledger'а в
архив (§2.2).

---

## §1. Вердикт числом

| действие | предмет | вердикт |
|---|---|---|
| удалить | `salvage/M-59-research-dev-uncommitted` | **РАЗРЕШЕНО** при У-1 |
| удалить | `feat/M-10-rebased` | **РАЗРЕШЕНО** при У-1, У-2 |
| удалить | `feat/M-60-mechanisms` | **РАЗРЕШЕНО** при У-1, У-2, **У-3** (уникальный груз) |
| влить | `docs/founder-decisions-2026-08-17` (PR #19) | **РАЗРЕШЕНО** — первым, без условий |
| влить | `docs/M-60c-plan` (PR нет) | **РАЗРЕШЕНО** — без условий, после создания PR |
| влить | `docs/P-012-decision` (PR #18) | **ЗАПРЕЩЕНО в одиночку** — У-4 (воспроизведён красный GATE-META) |
| влить | `fix/resource-oracle-barrier` (PR #28) | **РАЗРЕШЕНО при У-5** (гейт трека §5 п.5 не выполнен) |
| маршрут | кто приземляет #28 | харнесс-трек (architect), но не «по §5 без остатка» — §4 |

Постфактум-проверка четырёх уже удалённых веток — **подтверждена полностью** (§2.4).

---

## §2. Удаление — по каждой ветке

### 2.1 `salvage/M-59-research-dev-uncommitted` (`dc646cb`) — РАЗРЕШЕНО

Уникальный вклад ветки — РОВНО один файл, новых файлов нет:

```
$ git diff --stat origin/main...origin/salvage/M-59-research-dev-uncommitted
 crates/research-cli/src/depth_lifetime.rs | 177 ++++++++++++++++++++----------
$ git diff --diff-filter=A --name-only origin/main...origin/salvage/M-59-research-dev-uncommitted
(пусто)
$ git rev-list --left-right --count origin/main...origin/salvage/M-59-research-dev-uncommitted
434	1
```

Предмет прошёл гейт и влит: `git merge-base --is-ancestor 61f452e origin/main` → exit=0;
`61f452e` = «Merge branch 'feat/M-59-lifetime-memory' — M-59: граница памяти per-life
анализатора (R-083 APPROVED, круг 3)». Файл `research/reviews/R-083-M-59-rev3.md` в `main`,
строка 3: «**Вердикт: APPROVED.** Блокеров нет». Close-out `41ba13c` («TD-107/TD-108
закрыты») — предок `main`, exit=0; `TD-107`/`TD-108` в `TECH-DEBT.md` больше не заводятся.

**Проверка на потерю — содержимым, а не путём.** Влитая версия и черновик решают ОДИН класс
(завершённая жизнь стиралась перерождением цены до атрибуции) РАЗНЫМИ структурами:
`main` — `pending_completed: VecDeque<(i64, Fate)>` + `flush_pending_completed`
(`:201`, `:241`); черновик — `pending_bumps: BTreeMap<i64, Vec<Fate>>` + `flush_delayed_states`
(`:199`, `:241`). Обе покрывают множественность жизней одной цены. Влитая при этом несёт
ИЗМЕРЕННУЮ границу — `:173` «замер `R-076` — ×4.00 на одностороннем потоке против ×1.00 на
пути с mid», тогда как черновик на `:175` пишет «НЕ ИЗМЕРЕНА». Черновик — превзойдённая
параллельная попытка, а не источник уникальной защиты.

### 2.2 `feat/M-10-rebased` (`51c21dc`) — РАЗРЕШЕНО

Уникальных путей 17 (`git diff --name-status origin/main...origin/feat/M-10-rebased`);
отставание 1197. Покрытие проверено ПОФАЙЛЬНО, а не по совпадению имён:

**(а) Восемь файлов кода → `crates-research-cli.patch`.** Проверка сильнее, чем
`git apply --check`: патч ПРИМЕНЁН на сегодняшнем `origin/main` в чистом worktree, и результат
сверен с версиями ветки —

```
$ git apply /tmp/rc_m10.patch ; echo APPLY_EXIT=$?
APPLY_EXIT=0
IDENTICAL  crates/research-cli/src/{grid,ledger,main,report,types}.rs
IDENTICAL  crates/research-cli/tests/{red_killscreen,red_research,red_stack_honesty}.rs
mismatches=0
```

Это закрывает и живую ссылку `docs/07-cockpit-backend-roadmap.md:28` («RED-спека закоммичена
на `feat/M-10-rebased`, вернёмся при возобновлении сигналов»): сама спека переживает удаление
ветки побайтно, ломается только адресат.

**(б) Восемь артефактов → архив, 0 удалённых строк.** Для каждой пары «ветка → архив» диф
даёт `deleted_lines=0`; добавлено 18–27 строк архивной врезки:

```
milestones/M-10-r001-obi-killscreen.md → M-10-r001-obi-killscreen.md : deleted=0 added=18
research/critiques/C-019-M-10.md       → C-019-M-10.md               : deleted=0 added=18
research/critiques/C-020-M-10.md       → C-020-M-10.md               : deleted=0 added=18
research/data-quality/gaps-own-*.json  → gaps-own-2026-07.json       : deleted=0 added=0
research/decisions/D-001-*.md          → D-001-*.DRAFT-UNSIGNED.md   : deleted=0 added=27
research/reports/R-001-obi-trackA.json → R-001-obi-trackA.json       : deleted=0 added=0
research/reports/R-001-obi-trackA.md   → R-001-obi-trackA.md         : deleted=0 added=18
scripts/verify_M-10.sh                 → verify_M-10.sh              : deleted=0 added=23
```

Переименование `D-001` в `…DRAFT-UNSIGNED.md` содержимого не трогает и повышает честность:
подписи founder'а под ним не было, а `research/decisions/` — зона founder-подписи
(`scope-guard.md`).

**(в) 276 записей `trials-ledger` — перепроверено лично (прямое требование мандата).**

```
$ git show origin/main:research/trials-ledger.jsonl | wc -l                                   → 4
$ git show origin/feat/M-10-rebased:research/trials-ledger.jsonl | wc -l                      → 280
$ git show origin/main:docs/archive/M-10-.../trials-ledger-delta.jsonl | wc -l                → 276
$ git show origin/main:docs/archive/M-10-.../trials-ledger-delta.jsonl | md5sum
d8a2a9845c3d5323556236e10476ab2b
$ git show origin/feat/M-10-rebased:research/trials-ledger.jsonl | tail -276 | md5sum
d8a2a9845c3d5323556236e10476ab2b
$ diff <(git show origin/main:research/trials-ledger.jsonl) \
       <(git show origin/feat/M-10-rebased:research/trials-ledger.jsonl | head -4); echo $?
0
```

То есть ledger ветки = ledger `main` (4 строки, побайтно) + архивная дельта (276, побайтно).
Предупреждение аудита 13.08 §8 п.3 было верно ДЛЯ СВОЕГО МОМЕНТА и снято архивацией.
**`RC-I-2` при этом НЕ нарушен и не обойдён:** инвариант запрещает удаление/перезапись
записей живого ledger'а, а не запрещает хранить чужую эпоху отдельным файлом. Отдельное
хранение здесь ПРАВИЛЬНО по `gates.md` §6 п.3 (эпохи ledger'а несопоставимы): влив 276 записей
мёртвого OBI Трека A в живой ledger, мы бы испортили знаменатель deflated-Sharpe живых
сигналов. Это — довод ЗА нынешнюю раскладку, а не оговорка к ней.

Итого 8 + 8 + 1 = 17, покрытие полное.

### 2.3 `feat/M-60-mechanisms` (`f0e915b`) — РАЗРЕШЕНО ТОЛЬКО ПРИ У-3

Уникальных путей 23; отставание 430. Двух путей нет в `main` под своими именами
(`milestones/M-60-mechanisms.md`, `scripts/verify_M-60.sh`) — оба в `docs/archive/` под
архивными именами, это подтверждается. Но **проверка «21 путь существует в `main`»
(`git cat-file -e`) — проверка ПУТИ, а не содержимого**, и метод самого инвентаря
(«поглощение доказывается ancestry либо СРАВНЕНИЕМ СОДЕРЖИМОГО, а НЕ совпадением путей»)
требует второй. Я её выполнил:

```
$ # для каждого из 21: git diff --numstat origin/main:<p> origin/feat/M-60-mechanisms:<p>
research/{arbitration/A-005, critiques/C-062,C-064,C-065,C-066,C-067,C-068,
          reviews/R-041,R-042}                     SAME  (9 артефактов гейтов — идентичны)
scripts/tests/red_docs_freeze.sh                   SAME
.claude/rules/gates.md            DIFF +5  -130    ← branch-only строки ПРОТУХЛИ
.github/workflows/ci.yml          DIFF +2  -187    ← агрегат на 6 джобов (сейчас 13)
milestones/{M-60a,M-60b,M-60c}    DIFF +2/+46/+64  ← черновики 06.08, переписаны в main
milestones/BACKLOG.md             DIFF +22 -158
scripts/{check_docs_freeze,tests/red_context_budgets,tests/red_gate_meta}.sh  DIFF (старее)
docs/09-roadmap-v2.md             DIFF +14 -0      ← ★ ЧИСТОЕ ДОБАВЛЕНИЕ, в main НЕТ НИЧЕГО
```

Branch-only строки `gates.md` — это дословно «self-push автора» и «branch protection на этом
репозитории недоступен (403, private + free plan)»: утверждения, ОТМЕНЁННЫЕ 2026-08-15.
Их потеря — благо. Черновики M-60a/b/c и таблица BACKLOG — превзойдённые редакции.

**Находка (★).** `docs/09-roadmap-v2.md` несёт на ветке 14 строк, которых в `main` нет ВОВСЕ
(`+14 -0`, то есть `main` ничего взамен не даёт) — раздел «### Процессный трек — именованная
цепочка, а не «фон» (2026-08-06)» с нормативным утверждением «Трек **не занимает фазовый
слот** и не конкурирует с Ф0–Ф6». Поиск по всему дереву `main`:

```
$ git grep -ln "Процессный трек" origin/main -- .
docs/plans/M-60-reconciliation-2026-08-13.md      ← и только он, как УПОМИНАНИЕ
$ git grep -ln "фазовый слот" origin/main -- .
(пусто)
```

Решение по этому грузу существует и я его нашёл — `docs/plans/M-60-reconciliation-2026-08-13.md:61`:
«дельты `docs/09-roadmap-v2.md` (+14), `milestones/BACKLOG.md` (+19) | 33 | «Процессный трек»
как именованная очередь — идея верная, статусные строки протухли; **писать заново**». То есть
решение — ПЕРЕПИСАТЬ, и оно **не исполнено**: раздела в `main` нет.

Внегитовый дубль, на который ссылается `SESSION-HANDOFF.md:27`, эти дельты НЕ содержит:

```
$ ls /home/nous/salvage-2026-08-14/M-60-branch-cargo/
C-062-M-60-mechanisms.md  M-60b-gate-mechanisms.md  M-60c-corpus-cleanup.md
README.txt  red_context_budgets.sh  red_gate_meta.sh  SOURCE-COMMIT.txt
```

Пять из семи файлов этого дубля уже лежат в `main` по своим путям — дубль устарел ровно в той
части, где он избыточен, и молчит в той, где он единственный. Удаление ветки без У-3
уничтожает последнюю копию 14+22 строк.

Отдельно подтверждаю находку §5 п.5 инвентаря и добавляю к ней третий адрес:
`milestones/M-60b-gate-mechanisms.md:348` и `milestones/M-60c-corpus-cleanup.md:241` — оба
дословно «**Ветка `feat/M-60-mechanisms` НЕ УДАЛЯЕТСЯ** (`C-083` F-083-2)»; третий —
`docs/SESSION-HANDOFF.md:27` «**НЕ УДАЛЯТЬ.** `C-083` F-083-2 доказал: остаётся уникальный
не-RED груз (зонт `M-60-mechanisms.md`, `verify_M-60.sh`, **дельты роадмапа/BACKLOG**)».
Из четырёх названных там предметов архивация покрыла ДВА. Строка живёт в §0bis, помеченном
ИСТОРИЧЕСКИМ, — но §0bis входит в Ярус A обязательного чтения (`CLAUDE.md`), и её содержание
сегодня подтверждается замером, а не опровергается им.

### 2.4 Четыре уже удалённые ветки — постфактум, ПОДТВЕРЖДЕНО

Все четыре SHA живы в локальной базе; проверка велась в ОБЕ стороны — предковство и явное
отсутствие односторонних коммитов:

```
$ git merge-base --is-ancestor 6082336 b623470                             → exit=0
$ git merge-base --is-ancestor 6082336 origin/fix/M-65-battery-recalibration → exit=0
$ git merge-base --is-ancestor b623470 origin/fix/M-65-battery-recalibration → exit=0
$ git merge-base --is-ancestor 0fd7380 origin/fix/M-65-battery-recalibration → exit=0
$ git merge-base --is-ancestor 3fb009b origin/docs/M-67-rev2               → exit=0
$ git rev-list --count 6082336 ^origin/fix/M-65-battery-recalibration      → 0
$ git rev-list --count b623470 ^origin/fix/M-65-battery-recalibration      → 0
$ git rev-list --count 0fd7380 ^origin/fix/M-65-battery-recalibration      → 0
$ git rev-list --count 3fb009b ^origin/docs/M-67-rev2                      → 0
```

`0fd7380` добавлен мной: это вершина `feat/M-65-race-fix` по остаточному ref'у `allrefs/`,
отличная от названного в мандате `b623470`. Обе поглощены — заявление устояло на более
широком наборе, чем предъявленный.

**Поправка к формулировке.** `test/depth-from-book` (`12d906f`) предком **НЕ является**:

```
$ git merge-base --is-ancestor 12d906f origin/feat/M-68-depth-from-book → exit=1
$ git rev-list --count 12d906f ^origin/feat/M-68-depth-from-book        → 1
```

Заявлено было не предковство, а идентичность содержимого — и она держится:

```
$ git diff --stat origin/feat/M-68-depth-from-book...12d906f
 crates/gateway/tests/red_depth_from_book.rs | 249 +++  (единственный путь)
$ md5sum <(git show 12d906f:…) <(git show origin/feat/M-68-depth-from-book:…)
7e445d2a9e3eb507df6bd02db460f942   7e445d2a9e3eb507df6bd02db460f942
```

Различать эти два основания важно: предковство переживает любую последующую правку потомка,
идентичность содержимого — нет.

---

## §3. Вливание — по каждому PR, на ДЕРЕВЕ СЛИЯНИЯ

Конфликтность (`git merge-tree --write-tree` против `origin/main` = `1a0270e`):

```
origin/docs/founder-decisions-2026-08-17 → exit=0     origin/docs/M-60c-plan          → exit=0
origin/docs/P-012-decision               → exit=0     origin/fix/resource-oracle-barrier → exit=0
origin/docs/founder-decisions-2026-08-17 × origin/docs/P-012-decision → exit=1  (КОНФЛИКТ)
```

Порядок `R-091` У-6 подтверждён. Конфликт — единственный хунк `docs/PENDING-SIGNATURE.md`
667..861, сторона #18 в нём ПУСТА: #19 вставляет новые секции там, где #18 переписывает
соседнюю. Ручной сплайс (взять сторону HEAD) сохраняет ОБА предмета — проверено:
после резолва в файле присутствуют и `## П-012 — ЗАКРЫТО … РЕШЕНИЕМ АРБИТРА` (#18), и
`П-011`-амендмент / `П-013` / `П-014` / `П-015` (#19).

### 3.1 PR #19 `docs/founder-decisions-2026-08-17` — РАЗРЕШЕНО, первым

Диф: `docs/PENDING-SIGNATURE.md` +191, `R-090` +436, `R-091` +515; отставание **0**
(ветка вобрала `main`, `merge` даёт fast-forward, `--no-ff` — чистый merge-коммит).
На дереве слияния (`--no-ff`, `8ce249d`):

```
check_gate_meta.sh 1a0270e HEAD      → NOTE (ALLOW-SUBJECT-CHANGE), VERDICT: PASS   exit=0
check_docs_freeze.sh 1a0270e HEAD    → exit=0
check_protected_artifacts.sh         → OK, exit=0
verify_design_claims.sh              → VERDICT: PASS (0 нарушений)  exit=0
```

**У-1/У-3/У-4/У-5 `R-091` — перепроверены лично, все закрыты** коммитом `6c19e09`:
У-3 — литерал `"diff-reconstructed, validated<=1.3%"` присутствует целиком (`:621`);
У-4 — якоря `:360-361` (`:594`) и `:398` (`:596`), `---` перед `## П-013` (`:726-728`),
«со сохранением» не встречается ни разу, есть «с сохранением» (`:652`);
У-5 — дополнение про `gap-resync` на `:597-599` и `:610-611` с адресами `:248,263,302,333`
и `state.book = None` на `:259`;
У-1 — сноска `:742-743` называет носитель `docs/M-67-rev2` и переживший merge статус второй
ветки. Оба адресата M-67 помечены (`:747`, `:756`).

### 3.2 `docs/M-60c-plan` — РАЗРЕШЕНО, без условий (PR ещё не создан)

Один новый файл, 459 строк, отставание 182. На дереве слияния (`36a0e13`):
`gate_meta` PASS exit=0 · `docs_freeze` exit=0 · `protected` OK exit=0 ·
`design_claims` PASS exit=0 · `check_artifact_ids.sh` (прод-форма, `EVENT_NAME=pull_request`)
→ «ни один новый артефакт не введён», exit=0.

### 3.3 PR #18 `docs/P-012-decision` — ЗАПРЕЩЕНО В ОДИНОЧКУ (У-4)

`gh pr checks 18` → все `pass`, exit=0. **Это зелёное устарело** (`gates.md` §8,
`strict:false`; класс `TD-135`): прогон снят на базе, старее нынешней на 42 коммита.
На дереве слияния с сегодняшним `main` (`d62c49a`):

```
$ bash scripts/check_gate_meta.sh 1a0270e34827c0a5997d746e8d9b6f34744d12a4 HEAD
FAIL  research/arbitration/A-009-export-contract-ceremony.md: subject-lock — после проходного
      вердикта (DECISION) тронут класс «гейт»: .claude/rules/gates.md
      .github/workflows/{branch-build,ci,deploy}.yml scripts/tests/red_deploy_catchup.sh
      выход из лока — строка «ALLOW-SUBJECT-CHANGE: <причина>» в теле коммита диапазона
VERDICT: FAIL (1)   GM_EXIT=1
```

Причина — механическая и проверена чтением `scripts/check_gate_meta.sh:249-259`: диапазон
диффа берётся от `audited_head` вердикта (`9b888b1`) до `HEAD`, а `main` после 17.08
независимо трогал класс «гейт». Синхронизация ветки с `main` этого НЕ лечит: новые коммиты
`main` попадают в тот же диапазон.

**Развязка, найденная замером, а не рассуждением, и она же — причина, по которой порядок
`У-6` несущий.** После merge'а #19 токен `ALLOW-SUBJECT-CHANGE` из тела `6c19e09` попадает в
диапазон и открывает лок ДЛЯ ВСЕХ проходных вердиктов сразу, включая `A-009`:

```
$ # дерево: main + #19 + #18 (конфликт разрешён вручную), 1df3a19
NOTE  research/arbitration/A-009-…: subject-lock открыт явным ALLOW-SUBJECT-CHANGE
NOTE  research/reviews/R-091-…:     subject-lock открыт явным ALLOW-SUBJECT-CHANGE
VERDICT: PASS — вердиктов проверено: 3     GM_EXIT=0
```

То есть очерёдность #19→#18 отделяет зелёный `main` от красного, и держится это на ТОКЕНЕ,
написанном для ДРУГОГО вердикта. Причина в теле `6c19e09` («правки класса «гейт» пришли ИЗ
`origin/main` вбиранием») для `A-009` фактически верна, но авторства под неё нет. Отсюда У-4.

Прочие гейты #18 на дереве слияния зелёные: `docs_freeze` exit=0, `protected` OK exit=0,
`design_claims` PASS exit=0, `check_artifact_ids.sh` (прод-форма) exit=0.

### 3.4 PR #28 `fix/resource-oracle-barrier` — РАЗРЕШЕНО ПРИ У-5

Диф: `ci.yml`, четыре тест-файла (`crates/{gateway,journal,research-cli}/tests/**`),
`C-095`, `check_resource_oracles.sh`, `red_resource_oracles.sh`; отставание 22.
На дереве слияния всё зелёное, включая то, что `gh pr checks` не покрывает:

```
gate_meta PASS exit=0 · docs_freeze exit=0 · protected OK exit=0 · design_claims PASS exit=0
cargo fmt --all -- --check                                   → FMT_EXIT=0
cargo clippy -p gateway -p journal -p research-cli --all-targets --all-features -- -D warnings
                                                             → CLIPPY_EXIT=0
cargo test (4 изменённых оракула)  passed=16 failed=0 (блоков: 4)  → TEST_EXIT=0
bash scripts/tests/red_resource_oracles.sh   VERDICT: PASS (16/16)  PROBE_EXIT=0
bash scripts/check_resource_oracles.sh       VERDICT: PASS (оракулов 12)  BAR_EXIT=0
```

Агрегат CI после merge'а — одно тело, 13 джобов, `resource-oracles` внутри; ни один джоб
`main` не потерян (сверка списков `^  [a-z0-9-]+:$`: 12 в `main` + 1 новый).

**Честность «признанного предела» — проверена своими стабами, не пересказом.** Я построил
четыре фикстуры независимо от `C-097` и прогнал барьер в прод-форме `ROOT=<фикстура>`:

```
pos (обычный процессный счётчик + два #[test])  → FAIL, exit=1   ← позитивный контроль держит
r2  (static CUR: OnceLock<AtomicUsize>)         → PASS, exit=0   ← пропущен, как и заявлено
r8  (счётчик + два #[tokio::test])              → PASS, exit=0   ← пропущен, как и заявлено
r10 (счётчик + thread_local! в КОММЕНТАРИИ)     → PASS, exit=0   ← пропущен, как и заявлено
```

Шапка барьера (`scripts/check_resource_oracles.sh:49-71`) и тело `a071496` перечисляют ровно
шесть классов `C-097` §1.2 (R2, R3, R4, R8, R9, R10) — предел назван честно и не занижен.
Вердикт `C-097` REJECT в `a071496` снятым НЕ объявляется; влитие оформлено как
founder-override с названным обоснованием.

**Мутационный контроль — свой, отличный от адверсарийного.** Снял из регулярки `procwide`
поддержку `pub static` и квалифицированного пути типа (это пиннит RO-16 — сценарий ПОЗЖЕ
удалённого раннего агрегатора, то есть ровно то, что чинил H-1):

```
проба после мутации:  сценариев: 16  PASS: 15  FAIL: 1   VERDICT: FAIL (1 из 16)  exit=1
проба после отката:   сценариев: 16  PASS: 16  FAIL: 0   VERDICT: PASS (16/16)    exit=0
```

Фикс H-1 реален, а не заявлен.

**Находка (блокирующая до У-5): гейт трека §5 п.5 НЕ выполнен, и H-11 не закрыт ничем.**
`harness-track.md` §5 п.5 требует «замер **«каталогов ПОСЛЕ прогона»** предъявлен числом».
Проба печатает размер РЕЕСТРА, то есть число СОЗДАННЫХ каталогов — это и есть находка
`C-097` H-11 (MAJOR, предмет B). Воспроизведено ломанием уборки:

```
$ # cleanup() { :; }  — уборка обезврежена; TMPDIR=/home/nous/.cache/paxio-tmp
before=2509
каталогов-фикстур в реестре: 16 (уборка — trap EXIT)
сценариев: 16   PASS: 16   FAIL: 0
VERDICT: PASS (16/16)      probe_exit=0            ← проба НЕ ЗАМЕТИЛА
after=2526   leaked=17
```

Сломанная уборка оставляет 17 каталогов, а проба остаётся зелёной — «наблюдает сбой, но не
ОТСУТСТВИЕ» (`testing.md`, целостность гейта, свойство 4). `mktemp -d` по-прежнему без
префикса (`red_resource_oracles.sh:35`).

При этом `C-097` §6 требует: «MAJOR (снимаются либо получают TD-карточку с severity и
владельцем): H-4…H-11». Проверка:

```
$ git show origin/main:TECH-DEBT.md | grep -c "C-097"   → 0
```

Ни фикса, ни карточки. Founder-override в `a071496` покрывает ШЕСТЬ ПРОПУЩЕННЫХ КЛАССОВ
СТАБОВ и **не упоминает H-11** — то есть предел, признанный решением, и предел, оставшийся
непризнанным, здесь разные. Мусор в общем `TMPDIR` — не гипотеза: на момент замера там уже
лежало 2 509 каталогов, а класс «10 400 каталогов и диск на 100 %» назван в самом
`harness-track.md` §5 п.5.

---

## §4. Кто вправе приземлить #28 — маршрут против §4 `gates.md`

**Ответ: харнесс-трек, то есть architect своим PR. Но не «по §5 без остатка» — см. У-5.**

Основания, каждое проверено открытием файла:

1. `gates.md` §4 привязан к MILESTONE: «Reviewer обязателен для ЛЮБОГО **milestone'а**,
   тронувшего код/контракты/риск/докс». У #28 нет ни milestone-файла, ни §Tasks, ни
   `verify_M-NN.sh` — это не милестоун, а трек-работа.
2. `harness-track.md` — отдельный founder-авторизованный (2026-08-15) маршрут; его §5
   перечисляет гейт merge'а и reviewer'а в него не включает, а §6 называет это ПРИЗНАННЫМ
   пределом («здесь предмет видят две роли»), не скрывает.
3. Текстовое противоречие внутри самого трека РАЗРЕШАЕТСЯ ИМ ЖЕ. §4 говорит «`crates/**` без
   исключений», но §6 в кандидате на механизацию пишет: «проверка, что PR трека не трогает
   `crates/**` (**кроме `*/tests/**`**), `contracts/**` и перечисленные документы норм».
   Плюс собственный признак §4 — «если код запускается на VPS или его результат попадает в
   журнал» — тест-файлы ему не удовлетворяют.
4. Адверсарий трека уже вынес это решение по существу: `C-097` §6 дословно — «Маршрут
   остаётся харнесс-треком: предмет — `scripts/**` и `.github/workflows/**`, `crates/**`
   затронуты только в `tests/`, полный milestone-цикл не требуется (`harness-track.md` §4,
   три вопроса дают «нет»)».

**Где инвентарь неправ.** `docs/plans/branch-inventory-2026-08-19.md` §3 п.4 утверждает:
«Требуется reviewer-гейт с вердиктом-ФАЙЛОМ (`gates.md` §4 UNCONDITIONAL) — merge без него
нарушение независимо от очевидности дифа». По МАРШРУТУ это неверно: §4 адресован милестоуну,
а трек — не милестоун, и адверсарий трека постановил обратное.

**Но вывод инвентаря («нужен reviewer») случайно попадает в цель по ДРУГОЙ причине, и её надо
назвать прямо.** `C-097` §6 требует TD-карточку с severity и владельцем на непокрытые MAJOR;
`TECH-DEBT.md` — **reviewer-owned**, architect в него не пишет (`scope-guard.md`,
профиль architect'а). Значит закрыть H-11 карточкой architect физически не вправе: у него
остаётся ровно один путь — ПОЧИНИТЬ H-11 (пять строк: считать каталоги после прогона,
`mktemp -d -t hft-ro.XXXXXX`), и тогда reviewer не нужен. Выбор между «починить» и «карточка»
— это и есть У-5.

Правку текста `harness-track.md` §4 («без исключений» против §6) я НЕ делаю и делать не
вправе в вердикте: это правка НОРМЫ, идёт через `gates.md` §9. Заношу как У-6.

---

## §5. Что я пытался опровергнуть и не смог

1. **Искал у `feat/M-10-rebased` хоть один файл, теряемый при удалении.** Проверил все 17
   путей: 8 воспроизводятся патчем ПОБАЙТНО (применил и сверил, а не ограничился
   `--check`), 8 архивированы с `deleted_lines=0`, ledger сошёлся по md5 с двух сторон.
   Не нашёл ничего.
2. **Пытался поймать патч `crates-research-cli.patch` на дрейфе** — месяц с последнего
   коммита ветки. `git apply` на сегодняшнем `main` прошёл, и результат оказался ИДЕНТИЧЕН
   версиям ветки по всем восьми файлам. Гипотеза «патч протух» опровергнута.
3. **Пытался найти в черновике `salvage/M-59` защиту, которой нет во влитой версии.**
   Сравнил файлы напрямую с сегодняшним `main` (не через merge-base — там легко получить
   ложный вывод): обе редакции закрывают один класс, влитая дополнительно несёт измеренную
   границу. Не нашёл.
4. **Пытался опровергнуть предковство четырёх удалённых веток** обратной проверкой
   (`rev-list --count X ^потомок`), добавив пятую вершину `0fd7380`, которой в мандате не
   было. Все нули. Заявление устояло на более широком наборе.
5. **Пытался доказать, что порядок #19→#18 — косметика.** Опровергнут собой же: #18 в
   одиночку роняет GATE-META (exit=1), в паре после #19 — PASS. Порядок несущий.
6. **Пытался поймать «признанный предел» на занижении** — построил свои стабы R2/R8/R10 плюс
   позитивный контроль. Все три пропускаются ровно как заявлено, позитивный ловится. Шапка
   барьера честна.
7. **Пытался доказать, что фикс H-1 — декларация.** Мутировал регулярку `procwide` (иной шов,
   чем у предыдущего адверсария): проба покраснела на своём сценарии, exit=1. Фикс держит.
8. **Пытался доказать, что диф #28 в `crates/*/tests/` ослабляет проверяемую границу.**
   Прогнал четыре оракула на дереве слияния — 16 passed / 0 failed; `fmt` и `clippy` на трёх
   крейтах — exit=0. Не нашёл.
9. **Пытался найти потерю джоба в агрегате `ci.yml` после merge'а #28** (класс `eaab0e0`).
   Сверил списки джобов `main` и дерева слияния: 12 + 1, `needs` перечисляет все 13. Не нашёл.

---

## §6. Где я расхожусь с адверсариями

| адверсарий утверждал | мой замер |
|---|---|
| «21 файл `feat/M-60-mechanisms` присутствует на `main` (`git cat-file -e`)» | Проверка ПУТИ, а не содержимого. Content-diff даёт **чистое добавление +14 −0** в `docs/09-roadmap-v2.md`, которого в `main` нет нигде (§2.3). Условие удаления добавлено — У-3 |
| «`salvage/M-59` — удалять, condition N/A» | Живая строка `SESSION-HANDOFF.md:149` в §0 (Ярус A): «**Salvage-ветки в `origin` (не мержить, держать до решения)**: `salvage/M-59-research-dev-uncommitted`». Условие У-1 распространяется и на неё |
| «PR #19 GATE-META на дереве слияния — FAIL» (снято позже) | На сегодняшнем `origin/main` — **PASS**, exit=0, воспроизведено с `--no-ff`. Находка была верна для своего момента |
| «конфликт #19×#18 — чисто позиционный, содержательного решения не требует» | Позиционный — да. Но `П-015` («ДЕЛЕГИРОВАНО АРБИТРУ… вердикт `research/arbitration/A-*-export-contract-ceremony.md`») и `П-012` («ЗАКРЫТО РЕШЕНИЕМ АРБИТРА») после сплайса стоят в файле в обратном порядке. Не блокер, но связность после резолва проверяется глазами, а не `-X ours` |
| «#28: C-097 REJECT оформлен, шесть классов названы честно — подтверждаю» | Подтверждаю тоже. Но **H-11 (MAJOR) не закрыт ни фиксом, ни TD-карточкой**, и через него не выполнен `harness-track.md` §5 п.5 — воспроизведено ломанием уборки, 17 утёкших каталогов при зелёной пробе (§3.4). Ни один адверсарий этого не проверил |
| «право architect'а мержить #28 — прямого нарушения не нашёл» | Согласен по МАРШРУТУ и добавляю недостающее звено: TD-карточку, которой `C-097` §6 закрывает MAJOR, architect писать не вправе (`TECH-DEBT.md` reviewer-owned). Отсюда развилка У-5 |

---

## §7. Условия (обязательны к исполнению до необратимого действия)

**У-1 — сохранение перед сносом (`branch-hygiene.md` п.6).** До `git push origin --delete`
для КАЖДОЙ из трёх веток положить рядом патч и вершину:
`git diff origin/main...<ветка> > /home/nous/salvage-2026-08-19/<ветка>.patch` +
`git rev-parse <ветка> >> SOURCE-COMMITS.txt`. После удаления remote-ref'а объекты
перестают быть достижимы и уходят в gc; ни одна из трёх веток сейчас не удерживается
worktree'ом (`git worktree list` — совпадений нет).

**У-2 — синхронизация живых адресатов ОДНИМ движением с удалением.** Заменить имя мёртвой
ветки на `docs/archive/M-10-obi-killscreen-retired-2026-07/` в: `docs/07-cockpit-backend-roadmap.md:28`
и `:193`; `milestones/BACKLOG.md:21`; `docs/SESSION-HANDOFF.md:127` («Удержано намеренно,
ждёт решения founder'а» — решение принято 17.08), `:101` («`feat/M-60-mechanisms` (не
мержить)»), `:149` (salvage-ветки «держать до решения»). Механического барьера на висячее
ИМЯ ВЕТКИ нет: `verify_design_claims.sh` на сегодняшнем `main` даёт PASS при всех шести
ссылках — проверка когнитивная, и это говорится, а не подразумевается.

**У-3 — уникальный груз `feat/M-60-mechanisms` (блокирующее).** До удаления сделать ОДНО из:
(а) перенести в `main` 14 строк `docs/09-roadmap-v2.md` (раздел «Процессный трек»), исполнив
предписание `M-60-reconciliation-2026-08-13:61` «писать заново» — тогда удаление чисто; либо
(б) положить `git diff origin/main:docs/09-roadmap-v2.md origin/feat/M-60-mechanisms:docs/09-roadmap-v2.md`
и то же для `milestones/BACKLOG.md` в `/home/nous/salvage-2026-08-19/` и ЗАПИСАТЬ явно, что
текст выбрасывается сознательно. Одновременно пометить «условие `C-083` F-083-2 выполнено
17.08 (PR #12)» в `milestones/M-60b-gate-mechanisms.md:348` и `M-60c-corpus-cleanup.md:241` —
снимать чужое нормативное условие молчаливым удалением строки нельзя.

**У-4 — PR #18 (блокирующее).** Мержить ТОЛЬКО после #19 и ТОЛЬКО перепроверив
`check_gate_meta.sh <sha main> HEAD` на дереве слияния НЕПОСРЕДСТВЕННО перед merge'ем
(`gh pr checks` для этого не годится — зелёное снято на базе −42). Правильная форма — свой
`ALLOW-SUBJECT-CHANGE: <причина>` в коммите синхронизации #18, а не заимствование токена
#19: тогда #18 перестаёт зависеть от очерёдности. Конфликт `PENDING-SIGNATURE.md`
разрешать ручным сплайсом (не `-X ours/theirs`), сохранив и `П-012`(#18), и
`П-011`/`П-013`/`П-014`/`П-015`(#19), и прогнать `gate_meta` + `design_claims` на результате
ДО коммита резолва.

**У-5 — PR #28 (блокирующее), одно из двух:**
(а) починить H-11 — печатать «каталогов ПОСЛЕ прогона» (замер до/после, не размер реестра),
`mktemp -d -t hft-ro.XXXXXX`, и предъявить прогон со сломанной уборкой, дающий КРАСНОЕ
(`C-097` §6 п.5, `harness-track.md` §5 п.5). Тогда трек закрывается автором, reviewer не
нужен; **либо**
(б) reviewer заводит TD-карточку на H-11 (severity MAJOR, владелец) — путь, которым
`C-097` §6 разрешает не чинить MAJOR; architect этого сделать не вправе.
До (а) или (б) merge #28 означает влитие с невыполненным пунктом гейта СОБСТВЕННОГО трека —
что отличается от founder-override'а `a071496`: тот покрывает шесть классов стабов и H-11 не
называет.

**У-6 — не в этом круге, но заводится сейчас.** `docs/workflow/harness-track.md` §4
(«`crates/**` без исключений») противоречит §6 («кроме `*/tests/**`») внутри одного файла.
Правка НОРМЫ — маршрут `gates.md` §9, отдельным PR с критиком.

---

## Done Block

```text
$ git -C /tmp/hft-rc-verdict rev-parse HEAD
1a0270e34827c0a5997d746e8d9b6f34744d12a4

$ bash scripts/reserve_artifact_id.sh R
reserve: попытка 1/8 — R-095 ← 394bf17cf0ce03b4518e0ddf65ae7d639d9c7ea5
R-095
RESERVE_EXIT=0

# ── УДАЛЕНИЕ ────────────────────────────────────────────────────────────────────────
$ git diff --stat origin/main...origin/salvage/M-59-research-dev-uncommitted
 crates/research-cli/src/depth_lifetime.rs | 177 ++++++-----
$ git diff --diff-filter=A --name-only origin/main...origin/salvage/M-59-…   (пусто)
$ git merge-base --is-ancestor 61f452e origin/main; echo exit=$?                  exit=0

$ git diff --name-status origin/main...origin/feat/M-10-rebased | wc -l           17
$ git apply /tmp/rc_m10.patch; echo APPLY_EXIT=$?                          APPLY_EXIT=0
  IDENTICAL ×8   mismatches=0
$ md5sum: архивная дельта == tail-276 ветки     d8a2a9845c3d5323556236e10476ab2b (обе)

$ git diff --name-only origin/main...origin/feat/M-60-mechanisms | wc -l          23
$ git diff --numstat origin/main:docs/09-roadmap-v2.md \
                     origin/feat/M-60-mechanisms:docs/09-roadmap-v2.md            14  0
$ git grep -ln "фазовый слот" origin/main -- .                                (пусто)

$ git merge-base --is-ancestor 6082336 b623470; echo exit=$?                      exit=0
$ git rev-list --count 3fb009b ^origin/docs/M-67-rev2                                  0
$ git merge-base --is-ancestor 12d906f origin/feat/M-68-depth-from-book; echo exit=$?  exit=1
$ md5sum обеих версий red_depth_from_book.rs   7e445d2a9e3eb507df6bd02db460f942 (обе)

# ── ВЛИВАНИЕ, дерево слияния (база 1a0270e) ─────────────────────────────────────────
$ merge-tree: #19→0  #18→0  M-60c→0  #28→0 ; #19×#18→1 (CONFLICT PENDING-SIGNATURE.md)

#19  (--no-ff 8ce249d): gate_meta PASS exit=0 · docs_freeze exit=0 · protected exit=0
                        design_claims PASS exit=0
#18  (d62c49a):         gate_meta FAIL (1) exit=1  ← БЛОКЕР
                        docs_freeze exit=0 · protected exit=0 · design_claims PASS exit=0
                        artifact_ids (прод-форма) exit=0
#19+#18 (1df3a19):      gate_meta PASS exit=0 (2×NOTE ALLOW-SUBJECT-CHANGE)
M-60c (36a0e13):        gate_meta PASS · docs_freeze 0 · protected 0 · design_claims PASS
                        artifact_ids (прод-форма) exit=0
#28  (5713eb4/3cdf901): gate_meta PASS · docs_freeze 0 · protected 0 · design_claims PASS
                        cargo fmt --all -- --check                    FMT_EXIT=0
                        cargo clippy -p gateway -p journal -p research-cli
                          --all-targets --all-features -- -D warnings CLIPPY_EXIT=0
                        cargo test (4 оракула)  passed=16 failed=0    TEST_EXIT=0
                        red_resource_oracles.sh  VERDICT: PASS (16/16)  exit=0
                        check_resource_oracles.sh VERDICT: PASS (12)    exit=0

# ── АНТИ-ПЛАЦЕБО: свои стабы против барьера #28 (ROOT=<фикстура>) ───────────────────
pos  обычный процессный счётчик + 2×#[test]   FAIL   exit=1   ← позитивный контроль
r2   static CUR: OnceLock<AtomicUsize>        PASS   exit=0   ← пропуск (заявлен)
r8   счётчик + 2×#[tokio::test]               PASS   exit=0   ← пропуск (заявлен)
r10  счётчик + thread_local! в комментарии    PASS   exit=0   ← пропуск (заявлен)

# ── МУТАЦИИ ────────────────────────────────────────────────────────────────────────
M1  procwide: снята поддержка 'pub static'/квалиф. пути
    → сценариев: 16  PASS: 15  FAIL: 1   VERDICT: FAIL (1 из 16)   exit=1
    откат → сценариев: 16  PASS: 16  FAIL: 0  VERDICT: PASS (16/16) exit=0
M2  cleanup() { :; }  (уборка фикстур обезврежена)
    before=2509 → after=2526  leaked=17
    проба: VERDICT: PASS (16/16)  probe_exit=0   ← НЕ ЗАМЕТИЛА (C-097 H-11)
$ git show origin/main:TECH-DEBT.md | grep -c "C-097"                                  0

# ── ГИГИЕНА ────────────────────────────────────────────────────────────────────────
$ df -h /   → 81 % (порог --reclaim 85 % не достигнут)
$ git -C /home/nous/hft-platform branch --show-current   main   (общий чекаут не тронут)
```
