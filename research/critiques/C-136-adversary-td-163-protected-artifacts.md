<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: ffb1d0231073f358d78ba7a0c49e1d4e3f8381f1
audited_head: 28b950026faf0900a63d987d25047e77f937f9ae
verdict: REJECT
-->

# C-136 — адверсарий TD-163: диагноз верен, ложное красное починено и предъявлено на обоих открытых PR — но fallback открывает ДОКАЗАННОЕ ложное ЗЕЛЁНОЕ: древний `ALLOW-ARTIFACT-DELETE` благословляет evil merge / `-s ours` воссозданного пути. Один блокер, условие снятия узкое

**Предмет:** PR #75, ветка `harness/td-163-protected-artifacts`, единственный коммит
`28b9500` (`scripts/check_protected_artifacts.sh` +25, `scripts/tests/red_protected_artifacts.sh` +61, только добавления).
**Роль:** адверсарий харнесс-трека (`docs/workflow/harness-track.md` §3), СВЕЖИЙ контекст,
Fable (названа в мандате). НЕ автор правки. Read-only на `scripts/**`; все мутации — в
копиях `/tmp/adv-*`.
**Рамка (мандат founder'а):** блокирует ТОЛЬКО ложное зелёное/красное, предъявленное
исполнением на построенной мной фикстуре. «Можно покрыть шире» — шум, не находка.

---

## §1. Диагноз подтверждён исполнением на реальных данных — ложное красное было и починено

Merge PR #66 (`origin/docs/two-barriers-step1-2026-08-23`) в `origin/main` (`ffb1d02`)
собран в отдельном дереве (`4d15e98`), барьеры запущены push-формой:

```
$ EVENT_NAME=push PUSH_BEFORE=ffb1d02... bash /tmp/adv-old-barrier.sh   # git show origin/main:scripts/check_protected_artifacts.sh
old exit=1
FAIL-строк: 24, NOTE-строк: 0
FAIL  milestones/M-04-research-core.md: артефакт ИСЧЕЗ с HEAD, и ни один коммит его не удалял

$ EVENT_NAME=push PUSH_BEFORE=ffb1d02... bash <ветка>/scripts/check_protected_artifacts.sh
new exit=0
FAIL-строк: 0, NOTE-строк: 24
NOTE  milestones/M-04-research-core.md: ALLOW-ARTIFACT-DELETE в bd357df
```

Числа тела коммита (24 FAIL / 24 NOTE) сошлись. PR #32 (`origin/feat/M-69-window-guard`)
той же процедурой: merge собран, новый барьер `exit=0` в **обеих** формах события
(`EVENT_NAME=push` и `EVENT_NAME=pull_request`). Утверждения о форк-точках проверены:

```
$ git merge-base --is-ancestor bd357df origin/docs/two-barriers-step1-2026-08-23; echo $?  → 1 (НЕ предок)
$ git merge-base --is-ancestor bd357df origin/feat/M-69-window-guard; echo $?              → 1 (НЕ предок)
```

Инцидент реален и уже случился (ограничитель трека): `gh run list --branch main` →
`completed failure … Merge pull request #29 … CI … 2026-08-23T23:36:52Z` + Deploy
failure/skipped; то же — сырым выводом в `R-118` §10 (прочитан из
`origin/main:research/reviews/R-118-M-65-a015-step6.md`).

## §2. БЛОКЕР Б-1 — ложное ЗЕЛЁНОЕ: fallback благословляет исчезновение ВОССОЗДАННОГО артефакта древним токеном

**Где:** `scripts/check_protected_artifacts.sh:166-169` (fallback), в связке с циклом
`ok=` `:171-187`, который не отличает «древнее объяснение пред-базового исчезновения» от
«индульгенции на исчезновение нового содержимого».

**Существо.** Fallback ищет объясняющий коммит по всей истории `HEAD`, когда диапазон
удаления не содержит. Комментарий `:163-165` и тело коммита утверждают: «Evil merge этим НЕ
прощается: путь, живший лишь на ветке и выброшенный merge'ем, не удалён НИГДЕ в истории —
`removed_by` остаётся пустым». **Это неверно для пути, чьё ИМЯ уже встречалось в истории и
было удалено с токеном:** fallback находит ТО, ДРЕВНЕЕ удаление и благословляет им
СЕГОДНЯШНЮЮ потерю нового файла. Пространство отравленных имён конкретно: один `bd357df`
удалил **26** защищённых путей (замер: `git show bd357df --name-status | awk '$1=="D" && (защищённые)' | wc -l` → 26), ALLOW-коммитов в истории `origin/main` — 5, и каждый будущий
архивный переезд расширяет это множество навсегда.

**Фикстура E1** (evil merge; setup-guard'ы по образцу P21 — все три состоялись):

```
init -b main; base с milestones/M-01.md
git rm milestones/M-01.md; commit  # тело: ALLOW-ARTIFACT-DELETE: архивный переезд по норме  ← ДРЕВНИЙ токен
commit filler
checkout -b feat; воссоздать milestones/M-01.md («живёт только на ветке»); commit
checkout main; before=HEAD
git merge --no-commit --no-ff feat; git rm --cached milestones/M-01.md; commit  # evil merge, БЕЗ токена
```

setup-guard: путь жив на `feat`, отсутствует на HEAD, диапазон `before..HEAD` без D/R — `1` (ок).

```
--- СТАРЫЙ барьер (origin/main): exit=1   ← ловил
--- НОВЫЙ барьер (28b9500):
NOTE  milestones/M-01.md: ALLOW-ARTIFACT-DELETE в c9534ac
OK: защищённые артефакты целы на HEAD (...)
exit=0                                    ← ЛОЖНОЕ ЗЕЛЁНОЕ
```

**Фикстура E2** — то же с `merge -s ours` и путём `research/critiques/C-001.md` (вердикт
критика нового круга под старым именем): старый барьер `exit=1`, новый —
`NOTE research/critiques/C-001.md: ALLOW-ARTIFACT-DELETE в 6717f26`, `exit=0`.

**Это регрессия ядра барьера, а не унаследованный пробел:** дореформенный барьер оба случая
валил (класс P1/P14 — то, ради чего барьер существует), новый молчит. Прод-форма вызова
(env-проводка CI) соблюдена в обоих прогонах. Сценарий прод-правдоподобен: «разархивировать»
спеку — значит воссоздать её ровно под старым путём; после этого любой evil merge / `-s ours`
уносит её беззвучно. Тот же механизм благословляет и подмену типа по такому пути (цикл `ok=`
отрабатывает до FAIL-ветки «артефакт подменён») — отдельно не прогонялось, корень один.

**Дискриминатор дыры измерен и чист.** Легитимный случай TD-163 — путь УНАСЛЕДОВАН веткой
(в диапазоне нет A-коммита); дыра — путь ДОБАВЛЕН в диапазоне:

```
E1:        git log --full-history --diff-filter=A before..HEAD -- milestones/M-01.md → [ad451b4]  (есть)
PR#66 sim: git log --full-history --diff-filter=A ffb1d02..4d15e98 -- milestones/M-04-research-core.md → []  (пусто)
```

P21 и P22 обе стороны дискриминатора не задевают (в P21 путь тоже унаследован, в P22
fallback не достигается) — сужение fallback'а по этому признаку не ломает ни фикс ложного
красного, ни существующие сценарии. Дизайн фикса — зона architect'а (`gates.md` §4).

**Условие снятия Б-1 (узкое, «чинить то, что есть»):**
1. fallback не смеет объяснять исчезновение содержимого, ПОЯВИВШЕГОСЯ в диапазоне
   (или эквивалентное сужение по выбору architect'а), при сохранении зелёного P21 и
   NOTE-исхода на симуляции PR #66;
2. в пробу добавлен E1-класс сценарий (setup-guard'ами: древний токен-предок базы, путь
   воссоздан В диапазоне, диапазон без D/R, путь отсутствует на HEAD ⇒ deny) — красный
   против барьера `28b9500`, зелёный против исправленного; `-s ours`-вариант — по решению
   architect'а (корень один);
3. утверждение «Evil merge этим НЕ прощается» в комментарии барьера скорректировано — в
   текущей форме оно ложно и станет основанием чьего-то будущего решения.

## §3. P21/P22 честны — обе стороны и все пять setup-guard'ов

Прогон на вершине (прод-форма, из корня worktree): `VERDICT: PASS (22/22)`, 2.2 с.

Анти-плацебо в обе стороны, обе мутации из тела коммита воспроизведены:

```
BARRIER=<origin/main-версия>      → FAIL P21 …, VERDICT: FAIL (1)
BARRIER=<вся-история-вместо-диапазона> → FAIL P22 …, VERDICT: FAIL (1)
```

Каждый setup-guard сломан по отдельности (правки КОПИИ пробы; барьер настоящий):

| излом | результат |
|---|---|
| A: P21 путь мёртв и на ветке | `FAIL P21 SETUP НЕ СОСТОЯЛСЯ…`, VERDICT: FAIL (1) |
| B: P21 путь восстановлен на HEAD | то же |
| C: P21 объяснение НЕ предок базы (`before=HEAD~1`) | то же |
| D: P22 воссоздания нет (путь мёртв на базе) | `FAIL P22 SETUP НЕ СОСТОЯЛСЯ…`, VERDICT: FAIL (1) |
| E: P22 второго удаления нет (диапазон пуст) | то же |

Каждый guard кричит своим сообщением; молчаливых нет.

## §4. Прежние сценарии: мутационный контроль по блокам

Шесть мутаций КОПИЙ барьера, суита против каждой:

| мутация | исход |
|---|---|
| M3: `existed` без деревьев диапазона | FAIL P11, P12 — пиннится |
| M4: `head_file_ok` принимает любой объект | FAIL P14, P15, P16 — пиннится |
| M5: токен всегда «найден» | FAIL P2, P3, P6, P11, P12, P14, P19, P20, P22 — пиннится (9) |
| M1: снят die zero-SHA | выжил — zero-SHA ловится СЛЕДУЮЩЕЙ проверкой («база отсутствует в истории»); неверного исхода не построить, слой избыточен по конструкции |
| M2: снят die «база не предок HEAD» | выжил — в P9-фикстуре исчезнувший артефакт валит гейт другой веткой; неверный исход требует force-push с целыми артефактами — предсуществующая избыточность слоёв, предметом не тронута |
| M6: снят `--full-history` в диапазонном поиске | выжил — НОВЫЙ fallback маскирует потерю тем же итогом (P18: rename находится полноисторийным поиском). Опора P18 сместилась с диапазонного флага на fallback; неверного исхода не построено |

По рамке мандата M1/M2/M6 — не блокеры и не NOTE: неверный исход на прод-форме не предъявлен.
Фиксируются здесь как результат обязательной программы, а не как требования.

## §5. verify_M-60b.sh — полный прогон

Полный прогон на вершине ветки (`/tmp/hft-adv-c136`, свежее дерево), фоном, лог целиком:

```
$ bash scripts/verify_M-60b.sh; echo "exit=$?"
PASS  P red_protected_artifacts: зелёная (22 исполнено — счёт из её собственного счётчика)
PASS  P red_docs_freeze: зелёная (27 исполнено — счёт из её собственного счётчика)
PASS  P red_artifact_ids: зелёная (51 исполнено — счёт из её собственного счётчика)
PASS  P red_commit_paths: зелёная (8 исполнено — счёт из её собственного счётчика)
PASS  CI cargo fmt --check
PASS  CI cargo clippy -D warnings
PASS  CI cargo test --all (замер rev3 1531.74s)
VERDICT: PASS
exit=0
```

Итог по логу: `grep -c '^PASS'` = 60, `grep -c '^FAIL'` = 0. Счёт declared/executed
сошёлся: проба заявляет 22/22, verify считает «22 исполнено» из её собственного счётчика.

## §6. Числа тела коммита — каждое своим прогоном

| утверждение | мой замер |
|---|---|
| старый барьер на PR#66-sim: exit=1, 24 FAIL | exit=1, `grep -c '^FAIL'` = 24 |
| новый: exit=0, 0 FAIL, 24 NOTE | exit=0, 0 / 24 |
| вершина: PASS (22/22) | `VERDICT: PASS (22/22)` |
| откат к main → FAIL P21 (1) | воспроизведено |
| безусловный fallback → FAIL P22 (1) | воспроизведено мутацией «вся история» (заявленная форма) |
| `bd357df` не предок обеих PR-веток | exit=1 обе |
| numstat 25/0 · 61/0 | `git show 28b9500 --numstat` → 25/0, 61/0 |
| PR #75 чеки | `gh pr checks 75` → все pass, вкл. `Protected artifacts` и `All checks passed` |

## §7. Ограничители трека

- Только харнесс-файлы, только добавления (86 строк на инцидент, реально сорвавший доставку
  дважды) — рост оправдан предметом.
- Барьер называет УЖЕ СЛУЧИВШИЙСЯ инцидент: CI failure + Deploy skipped, `gh run list` выше.
- Согласованное изменение в двух местах: барьер + проба, один коммит.
- Зона трека: не `crates/**`, не нормы, не путь к деньгам — маршрут применим.

## Done Block

```
$ cd /tmp/adv-c136-mergeSim && EVENT_NAME=push PUSH_BEFORE=ffb1d02... bash /tmp/adv-old-barrier.sh; echo old-exit=$?
old-exit=1   (FAIL-строк 24)
$ EVENT_NAME=push PUSH_BEFORE=ffb1d02... bash /tmp/hft-adv-c136/scripts/check_protected_artifacts.sh; echo new-exit=$?
new-exit=0   (FAIL 0, NOTE 24); pull_request-форма exit=0; PR#32-sim exit=0

$ [E1] СТАРЫЙ exit=1 · НОВЫЙ: NOTE milestones/M-01.md: ALLOW-ARTIFACT-DELETE в c9534ac → exit=0  ← БЛОКЕР
$ [E2] СТАРЫЙ exit=1 · НОВЫЙ: NOTE research/critiques/C-001.md: ALLOW-ARTIFACT-DELETE в 6717f26 → exit=0

$ bash scripts/tests/red_protected_artifacts.sh            → VERDICT: PASS (22/22)
$ BARRIER=<old> …                                          → FAIL P21, VERDICT: FAIL (1)
$ BARRIER=<вся-история> …                                  → FAIL P22, VERDICT: FAIL (1)
$ guard-break A/B/C/D/E                                    → каждый: SETUP НЕ СОСТОЯЛСЯ, VERDICT: FAIL (1)
$ мутации M3/M4/M5                                         → FAIL (2)/(3)/(9) — пиннятся
$ bash scripts/verify_M-60b.sh; echo exit=$?               → VERDICT: PASS, exit=0 (60 PASS / 0 FAIL, cargo test 1531.74s)

Фикстуры убраны: каталогов до=2720, после=1014 (остаток — не мои; свои /tmp/adv-fix-*,
копии барьера/пробы, merge-sim worktree — удалены, git worktree remove --force выполнен).
```
