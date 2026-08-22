# C-069 — M-61 artifact ids plan-time critic

**Дата:** 2026-08-07T10:57Z  
**Роль:** critic  
**Ветка:** `origin/docs/M-61-artifact-ids` @ `538284c`  
**Предмет:** `milestones/M-61-artifact-ids.md`  
**Вердикт:** **REJECT**

## Pre-flight

Этот круг сознательно НЕ проверяет RED-набор и `verify_M-61.sh`: по мандату founder'а они
пишутся после проверки перечня осей §4.2 и инварианта §4.1. Проверял только грамматику
инварианта, значения осей, §5 запреты и slug-определение §3.

Самореференция номера verdict-файла проверена первой:

```text
$ git for-each-ref --format='%(refname)' refs/remotes/origin | while read ref; do git ls-tree -r --name-only "$ref" 2>/dev/null; done | sed -n 's#^research/critiques/C-\([0-9][0-9][0-9]\).*#C-\1#p' | sort -u | tail -5
C-064
C-065
C-066
C-067
C-068
```

Следующий critic artifact — `C-069`; имя этого файла корректно.

## Verdict justification

§4.1/§4.2 пока не задают конечную грамматику, по которой можно написать пробу без
переписывания после первого же dev-круга. Два блокера являются находками категории (ii):
нужны структурные оси, без которых текущие реальные примеры одновременно требуют
красного и зелёного исхода. Один блокер категории (i) — недостающее значение уже
заявленной области поиска.

## Находки

### F1 — Нет оси "проверяемый срез": абсолютный инвариант конфликтует с запретом переименовывать старые коллизии

**Severity:** BLOCKER  
**Категория:** (ii) новая ось — `проверяемый срез / временной статус коллизии`  
**Минимальные значения:** `предсуществующая задокументированная коллизия baseline` (легитимно для текущего repo-gate) · `новая/усиленная коллизия этим diff` (обязана краснеть)

Evidence:

- `milestones/M-61-artifact-ids.md:71` формулирует абсолютный результат: "Ни один номер
  артефакта не обозначает ДВА РАЗНЫХ ПРЕДМЕТА".
- `milestones/M-61-artifact-ids.md:129` одновременно запрещает переименование существующих
  коллизий и говорит, что механизм предотвращает **НОВЫЕ** коллизии.
- Фактический repo уже содержит старые разные предметы под одним номером; это не теория:

```text
$ git for-each-ref --format='%(refname)' refs/remotes/origin | while read ref; do git ls-tree -r --name-only "$ref" | rg '^(milestones/M-[0-9]|research/(critiques/C|reviews/R|arbitration/A)-[0-9]).*\.md$'; done | sort -u | awk 'BEGIN{FS="/"} {base=$NF; if (match(base,/^(M|C|R|A)-[0-9]+[a-z]?/)) {key=substr(base,RSTART,RLENGTH); sub(/[a-z]$/,"",key); files[key]=files[key] " " $0; count[key]++}} END{for(k in count) if(count[k]>1) print k " |" files[k]}' | sort -V
C-018 | research/critiques/C-018-M-18-risk.md research/critiques/C-018-rev4.md
C-024 | research/critiques/C-024-M-28.md research/critiques/C-024.md
C-058 | research/critiques/C-058-addendum-critic2.md research/critiques/C-058-scale-architecture.md
M-38 | milestones/M-38-roadmap.md milestones/M-38a-cvd-session-ledger.md milestones/M-38b-checkpoint-reducer.md
M-46 | milestones/M-46-order-flow-indicators.md milestones/M-46-read-path-probe.md
M-60 | milestones/M-60-mechanisms.md milestones/M-60a-docs-freeze.md milestones/M-60b-gate-mechanisms.md milestones/M-60c-corpus-cleanup.md
R-035 | research/reviews/R-035-M-57.md research/reviews/R-035-M-58-rev2.md
R-038 | research/reviews/R-038-M-59.md research/reviews/R-038-M-60a-arbiter-trigger.md research/reviews/R-038-branch-hygiene.md
```

Почему это блокирует: если `check_artifact_ids.sh` проверяет абсолютный результат, он будет
красным на текущем репозитории навсегда. Если он молча grandfather'ит старые коллизии через
hardcode, это ровно "мешок случаев", запрещённый `A-005` §2 поправка 1. Ось должна быть в
таблице §4.2 до написания пробы: проверяется не "есть ли повтор вообще", а "появился ли
новый другой предмет относительно проверяемого среза".

Suggested fix: добавить в §4.2 структурную ось `проверяемый срез / временной статус` с
легитимным значением `предсуществующая задокументированная коллизия baseline` и красным
значением `новая/усиленная коллизия этим diff`; §4.1 переформулировать от результата
проверяемого diff, а не от вечной абсолютной истории.

### F2 — "Предмет = slug" не может одновременно зеленить C-058 и краснить C-018/C-024

**Severity:** BLOCKER  
**Категория:** (ii) новая ось — `носитель идентичности предмета`  
**Минимальные значения:** `slug имени файла` · `метаданные/шапка verdict ("Предмет"/"Контекст")` · `milestone-family suffix a/b/c` · `slug отсутствует`

Evidence:

- `milestones/M-61-artifact-ids.md:56` задаёт предмет как "часть имени файла после
  `<ПРЕФИКС>-<NNN>-`".
- `milestones/M-61-artifact-ids.md:91` требует зеленить `второй критик того же предмета`.
- Реальный законный пример C-058 имеет разные filename-slug'и, а одинаковый предмет живёт в
  содержимом:

```text
$ for path in milestones/M-38-roadmap.md milestones/M-38a-cvd-session-ledger.md milestones/M-38b-checkpoint-reducer.md research/critiques/C-058-scale-architecture.md research/critiques/C-058-addendum-critic2.md; do base=${path##*/}; printf '%s -> ' "$base"; printf '%s\n' "$base" | sed -E 's/^(M|C|R|A)-[0-9]+-?(.*)\.md$/subject=\2/'; done
M-38-roadmap.md -> subject=roadmap
M-38a-cvd-session-ledger.md -> subject=a-cvd-session-ledger
M-38b-checkpoint-reducer.md -> subject=b-checkpoint-reducer
C-058-scale-architecture.md -> subject=scale-architecture
C-058-addendum-critic2.md -> subject=addendum-critic2
```

```text
$ nl -ba research/critiques/C-058-scale-architecture.md | sed -n '1,5p'
     1	# C-058 — Аудит `docs/plans/scale-architecture-decision.md` (Redis vs in-process проекция)
     4	**Предмет:** `docs/plans/scale-architecture-decision.md` @ `feat/scale-architecture` HEAD `5b7b554`

$ nl -ba research/critiques/C-058-addendum-critic2.md | sed -n '1,6p'
     1	# C-058 — Addendum (второй критик, независимый прогон)
     3	**Контекст.** Founder запустил меня как замену первому критику на этот же предмет
     4	(`docs/plans/scale-architecture-decision.md`), предполагая, что первый завис.
     5	первый критик ожил и уже закоммитил `research/critiques/C-058-scale-architecture.md`
```

Также есть реальные slugless artifact-файлы:

```text
$ git for-each-ref --format='%(refname)' refs/remotes/origin | while read ref; do git ls-tree -r --name-only "$ref" | rg '^(milestones/|research/(critiques|reviews|arbitration)/).*\.md$'; done | sort -u | awk -F/ 'function class_of(path, base) { if (path ~ /^milestones\// && base ~ /^M-[0-9]/) return "M"; if (path ~ /^research\/critiques\// && base ~ /^C-[0-9]/) return "C"; if (path ~ /^research\/reviews\// && base ~ /^R-[0-9]/) return "R"; if (path ~ /^research\/arbitration\// && base ~ /^A-[0-9]/) return "A"; return ""; } { base=$NF; cls=class_of($0, base); if (cls != "" && base !~ "^" cls "-[0-9]+[a-z]?-.+\\.md$") print $0; }' | sort -V
research/critiques/C-003.md
research/critiques/C-024.md
research/critiques/C-025.md
research/critiques/C-028.md
research/critiques/C-029.md
research/critiques/C-030.md
research/critiques/C-031.md
research/critiques/C-032.md
research/critiques/C-033.md
research/critiques/C-034.md
research/critiques/C-035.md
```

Почему это блокирует: простая slug-реализация красит законный `C-058`, а реализация "любой
addendum/rev того же номера легитимен" зеленит незаконный `C-018-rev4` или `C-024.md`. Это
не значение существующей оси "законная множественность": чтобы решить "тот же предмет или
другой", нужна отдельная структурная ось про носитель идентичности предмета.

Suggested fix: §3 должен назвать нормализованную функцию `subject_id` по классу артефакта:
для обычных файлов — filename slug; для second-critic/addendum — явный `Предмет`/`Контекст`
из шапки; для split milestone — family id `M-38`, `M-60` плюс declared split; для slugless
старых `C-NNN.md` — named sentinel, который участвует в collision check, а не пропускается.

### F3 — Ось 3 говорит origin-only, а §3 требует origin ∪ local heads

**Severity:** BLOCKER  
**Категория:** (i) новое значение известной оси 3 `Область поиска максимума`: `локальный refs/heads/* участвует в выдаче/проверке`

Evidence:

- `milestones/M-61-artifact-ids.md:51-55` требует перечислять объединение
  `refs/remotes/origin/*` и `refs/heads/*`.
- `milestones/M-61-artifact-ids.md:71-72` и `milestones/M-61-artifact-ids.md:90` фиксируют
  invariant/легитимный случай как origin-only: "в объединении всех `refs/remotes/origin`" и
  "`максимум по всем origin-ref'ам`".
- В текущем worktree один и тот же milestone-ref есть и локально, и в origin; будущая
  реализация должна иметь один нормативный ответ, а не два:

```text
$ git for-each-ref --format='%(refname) %(objectname:short)' refs/heads refs/remotes/origin | rg 'docs/M-61-artifact-ids'
refs/heads/docs/M-61-artifact-ids 538284c
refs/remotes/origin/docs/M-61-artifact-ids 538284c
```

Почему это блокирует: A-005 требует выводить значения из предмета. Здесь предмет прямо
содержит локальные heads как тестовую и рабочую форму, но таблица §4.2 не делает это
значением оси. Реализация "только origin" будет бесполезна для локальной фикстуры; реализация
"origin+heads" не соответствует §4.1/§4.2 и не будет проверена как отдельное значение.

Suggested fix: заменить axis-3 legit на `максимум по refs/remotes/origin ∪ refs/heads` и
добавить красное значение `локальный head с занятым номером отсутствует в origin`.

### F4 — Acceptance N уже неверен для `M`: текущий origin содержит `M-61`, значит следующий свободный `M-62`

**Severity:** MAJOR  
**Категория:** (i) значение оси 3 `сам артефакт проверяемой ветки уже входит в область максимума`

Evidence:

- `milestones/M-61-artifact-ids.md:142` требует, чтобы `next_artifact_id.sh` на сегодняшнем
  репозитории печатал `M-61`.
- Но `origin/docs/M-61-artifact-ids` уже содержит `milestones/M-61-artifact-ids.md`:

```text
$ git for-each-ref --format='%(refname)' refs/remotes/origin | while read ref; do git ls-tree -r --name-only "$ref" | rg '^milestones/M-61.*\.md$' | sed "s#^#$ref #"; done
refs/remotes/origin/docs/M-61-artifact-ids milestones/M-61-artifact-ids.md

$ tmp=$(mktemp); git for-each-ref --format='%(refname)' refs/remotes/origin | while read ref; do git ls-tree -r --name-only "$ref" | rg '^(milestones/M-[0-9]|research/(critiques/C|reviews/R|arbitration/A)-[0-9]).*\.md$'; git show "$ref:TECH-DEBT.md" 2>/dev/null | rg -o 'TD-[0-9]+' || true; done > "$tmp"; for p in M TD R C A; do max=$(rg -o "${p}-[0-9]+" "$tmp" | sed -E "s/${p}-//" | sort -n | tail -1); printf '%s max=%s\n' "$p" "$max"; done; rm "$tmp"
M max=61
TD max=111
R max=039
C max=068
A max=005
```

Почему это блокирует: `TD-112`, `R-040`, `C-069`, `A-006` следуют из текущего максимума, но
`M-61` нет. Либо шаг N должен исключать "собственный artifact under review" специальным
правилом, либо ожидание должно быть `M-62` после появления branch в origin. Сейчас acceptance
невыполнимо без ad hoc исключения.

Suggested fix: разделить "номер milestone был выдан до коммита" и "следующий свободный номер
после коммита"; для verify на текущем дереве ожидать `M-62` либо добавить формальную
self-exclusion semantics в §4.2/§6.

### F5 — §5 запрещает ослаблять protected-artifacts script, но не запрещает ломать его CI-проводку

**Severity:** NOTE  
**Категория:** (iii) соседний инвариант, не находка о полноте осей

Evidence:

- `milestones/M-61-artifact-ids.md:39` разрешает dev менять `.github/workflows/ci.yml`.
- `milestones/M-61-artifact-ids.md:133` запрещает только ослаблять `check_protected_artifacts.sh`
  или его пробу.
- Текущий `ci.yml` держит соседний барьер не только скриптом, но и проводкой:
  `.github/workflows/ci.yml:47-68` job `protected-artifacts`,
  `.github/workflows/ci.yml:131-135` `status-check.needs` и guard.

Почему это важно: M-61 будет добавлять новый job/needs рядом с существующими. Реализация
может сохранить `red_protected_artifacts.sh` 20/20, но удалить `protected-artifacts` из
`status-check.needs` или ослабить guard, и §5 этого не запрещает. Это не самостоятельная
причина REJECT при наличии F1-F4, но это прямой gap в "запретном списке".

Suggested fix: добавить запрет "не удалять и не ослаблять существующие CI jobs/needs/guard для
`protected-artifacts`, `contracts`, `build-test`, `security`, `delivery`"; acceptance W/P должен
доказывать, что новая проводка аддитивна к старой.

## Проверки, которые PASS

- Заявленный список повторов по именам воспроизводится по origin-ref'ам: `R-038×3`, `R-035`,
  `M-46`, `C-018`, `C-024` действительно существуют как разные имена; `M-38`, `M-60`, `C-058`
  действительно существуют как заявленные кандидаты законной множественности.
- `C-069` как номер этого verdict-файла корректен: текущий максимум critic-файлов — `C-068`.
- Contract impact = N/A: ветка добавляет только `milestones/M-61-artifact-ids.md`; T1/`crates/contracts`
  не затронуты.

## Рекомендованное действие

Вернуть architect'у на правку спеки до написания RED-набора:

1. Добавить axis `проверяемый срез / временной статус коллизии` или эквивалентную baseline/diff
   семантику.
2. Добавить axis `носитель идентичности предмета` и формальную `subject_id` функцию.
3. Привести axis 3 к `origin ∪ heads` и добавить local-head значение.
4. Исправить acceptance N/S для `M-61` self-reference.
5. Расширить §5 запреты на CI-проводку соседних барьеров.

## Done Block

```text
$ git status --short
?? research/critiques/C-069-M-61-artifact-ids.md

$ git log -1 --oneline
538284c docs(M-61): спека — номер артефакта выдаётся механизмом, а не памятью (TD-111) [architect]

$ git diff --name-status origin/main..HEAD
A	milestones/M-61-artifact-ids.md

$ git for-each-ref --format='%(refname)' refs/remotes/origin | while read ref; do git ls-tree -r --name-only "$ref" 2>/dev/null; done | sed -n 's#^research/critiques/C-\([0-9][0-9][0-9]\).*#C-\1#p' | sort -u | tail -1
C-068
```
