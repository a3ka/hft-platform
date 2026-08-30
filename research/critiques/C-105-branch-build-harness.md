<!-- GATE-META
milestone: A-010
audited_repo: a3ka/hft-platform
audited_base: 4878be19184b0284b228e48741840b04223a9028
audited_head: 7575d38ce16ec01b8065fa2f76d3a7c63aa4ddfd
verdict: REJECT
-->

# C-105 — адверсарий набора «ветка собирается в CI» (branch-build): REJECT

**Поле `milestone: A-010` — оговорка.** У харнесс-трека нет M-файла по построению
(`docs/workflow/harness-track.md` §3: «отдельный milestone на каждый механизм» убран), а
управляющее решение по этому набору — арбитраж `A-010` §F/§J. Поэтому предметом гейта назван
он, а не несуществующий `M-NN`. Отдельно ниже (§2, B-11) показано, что самого `A-010` нет ни
на предмете, ни в `origin/main`.

**Роль:** адверсарий записи (`harness-track.md` §3/§6). Свёл три независимые линзы, каждую
находку воспроизвёл СВОЕЙ командой, добавил семь собственных атак, которых не строила ни одна
линза. Предмет не правил: диф этого коммита — один файл вердикта.

**Дерево:** `/tmp/hft-adv-verdict`, detached на `7575d38` (`origin/feat/harness-branch-build`,
PR #34). `origin/main` = `4878be1`; предмет от `main` НЕ отстаёт
(`git rev-list --count 7575d38..origin/main` → `0`), поэтому merge-preview совпадает с веткой.

---

## §0. Вердикт

**REJECT.** Основание — не «мало проверок», а три отдельных класса, каждый предъявлен
исполнением:

1. **Merge-блокирующее свойство набора снимается молча, и барьер, написанный ровно чтобы это
   наблюдать, остаётся зелёным.** Три независимые формы: `|| true` после вызова (B-1),
   `env:` на вызывающем шаге (B-2), закомментированное условие агрегата (B-3). Заявленная
   цель B7 — «барьер наблюдает СОБСТВЕННУЮ проводку» (`check_branch_build.sh:12-14`) — не
   достигнута ни в одной из них.
2. **Сам сборщик обезвреживается семью формами при `VERDICT: PASS`** (B-4…B-9): фильтр
   `paths`, `branches-ignore: '*/*'` (глушит 30 живых веток из 31 — замер ниже), динамический
   `if`, `env: RUSTFLAGS`, `defaults.run.shell`, `needs:` на фантом, `permissions: write-all`
   на новом джобе.
3. **Барьер печатает ЛОЖНОЕ утверждение о предмете.** На фикстуре с `permissions: write-all`
   он выводит дословно `ok B9: … прав на запись не запрошено`. Это хуже отсутствующей
   проверки — тем же аргументом, каким автор сам переписал «ложный честно названный предел»
   первой редакции (`check_branch_build.sh:41-45`).

Набор при этом НЕ вреден: прод он не задевает (проверено, §3), несущую процедуру merge не
ломает, позитивный контроль и мутационный контроль у пробы честные. REJECT — про недостаточность
барьера, а не про вредность механизма.

---

## §1. Мой прогон гейта трека (`harness-track.md` §5) — базовое состояние

Прогнано лично на `7575d38`, не принято на слово.

```
$ bash scripts/check_branch_build.sh; echo exit=$?
ok    B1: .github/workflows/branch-build.yml на месте и разбирается
ok    B2: `on.push.branches-ignore` = ['main'] — ветки собираются, `main` исключён
ok    B3: состав `build-test` совпадает с `ci.yml` шаг-в-шаг (6 шагов)
ok    B4: джоб `build-test` и все 6 шагов не обезврежены
ok    B5: `ci.yml` `on.push.branches` = ['main'] — отклонённая правка не просочилась
ok    B6: `deploy.yml`: push=['main'], paths не накрывают предмет, workflow_run=['CI']
ok    B7: `branch-build-parity` зовёт барьер и пробу, стоит в `status-check.needs` и участвует в его условии
ok    B8: `concurrency.group` = «branch-build-${{ github.ref }}» — по ref и не пересекается с deploy
ok    B9: имена джобов не пересекаются с `ci.yml`; прав на запись не запрошено

VERDICT: PASS — ветка собирается тем же составом, что `main`; агрегат, прод и merge-гейт не задеты.
exit=0

$ bash scripts/tests/red_branch_build.sh --battery   (хвост)
ok         M2-B2 … M9-B9              kill-set совпал (7/9/4/2/3/4/3/2 сцен.)
сценариев исполнено: 39  ok: 39  FAIL: 0
каталогов red-branchbuild-* до: 0, после уборки: 0
VERDICT: PASS
PROBE_EXIT=0
dirs_before=18004  dirs_after=18004      ← замер каталогов /tmp вокруг прогона
```

**Мой мутационный контроль (не батарея автора).** Мутация ВНУТРИ хелпера, а не удаление вызова
проверки: `invokes()` → безусловный `return True`.

```
$ bash scripts/tests/red_branch_build.sh   (против мутанта)
FAIL       S32-echo-не-вызов   барьер вернул exit=0 — стаб ПРОПУЩЕН
сценариев исполнено: 39  ok: 38  FAIL: 1
VERDICT: FAIL
PROBE_EXIT_vs_mutant=1
```

Проба этот класс ЛОВИТ. Позитивный контроль, анти-плацебо стабами, мутационный контроль и
уборка фикстур (§5 п.1/2/5) — выполнены честно; претензий к ПРОБЕ как к пробе нет. Все находки
ниже — про то, чего барьер не проверяет вовсе, поэтому и проба не может это пиннить.

---

## §2. Находки (все воспроизведены МОЕЙ командой на немодифицированном барьере)

Общая форма воспроизведения: три YAML-файла предмета копируются в фикстуру, мутируется один,
барьер зовётся прод-формой через задокументированную ручку
`BRANCH_BUILD_ROOT=<фикстура> bash scripts/check_branch_build.sh`.

### B-1 (БЛОКЕР) — `|| true` после вызова: merge-блокирующий джоб всегда зелён, B7 не видит

`scripts/check_branch_build.sh:187-208` (`invokes`), `:202` (`re.split(r"&&|\|\||;|\|", body)`),
`:206` (`seg.startswith(needle)`), потребитель — `:378-383`.

`invokes()` ищет needle в ПОЗИЦИИ КОМАНДЫ и на этом останавливается. Дописанный `|| true`
отрезается сплитом; первый сегмент по-прежнему `bash scripts/check_branch_build.sh`.

```
$ sed -i 's#run: bash scripts/check_branch_build.sh$#run: bash scripts/check_branch_build.sh || true#' <fix>/ci.yml
$ grep -n 'check_branch_build.sh' <fix>/ci.yml
361:        run: bash scripts/check_branch_build.sh || true
$ BRANCH_BUILD_ROOT=<fix> bash scripts/check_branch_build.sh
ok    B7: `branch-build-parity` зовёт барьер и пробу, стоит в `status-check.needs` и участвует в его условии
VERDICT: PASS — …
REAL_EXIT=0
```

Симметрично для вызова ПРОБЫ (`red_branch_build.sh --battery || true`) — `REAL_EXIT=0`.

**Почему блокер.** `branch-build-parity` — единственный джоб набора, входящий в
`status-check.needs` (`ci.yml:374`) и в fail-closed условие (`ci.yml:378`), то есть
единственное, что реально блокирует merge. Одна дописка обезвреживает и барьер, и пробу
разом, при этом сам барьер сообщает «проводка в порядке».

**Проба этого класса не несёт.** `grep -n 'check_branch_build.sh' scripts/tests/red_branch_build.sh`
→ строки 2, 30, 334; единственный сценарий на проводку с подменой команды — S32 (`echo` вместо
вызова). `grep -n '|| true'` → одна строка 307, и она про `cargo test --all || true` в ПРЕДМЕТЕ,
а не про вызов барьера в `ci.yml`.

### B-2 (БЛОКЕР) — `env:` на вызывающем шаге подменяет корень, который судит барьер

`scripts/check_branch_build.sh:370-404` — `check_b7` не читает `env` нигде;
`grep -vE '^\s*#' scripts/check_branch_build.sh | grep -c '"env"'` → **0**.

```
$ python3 …  # вставить перед run: в джоб branch-build-parity
        env:
          BRANCH_BUILD_ROOT: /opt/frozen-golden-snapshot
        run: bash scripts/check_branch_build.sh
$ BRANCH_BUILD_ROOT=<fix> bash scripts/check_branch_build.sh
VERDICT: PASS — …
REAL_EXIT=0
```

`BRANCH_BUILD_ROOT` — задокументированная ручка самого барьера (`check_branch_build.sh:47-49`:
«прод-путь её не задаёт»). Ничто не проверяет, что прод-путь её действительно не задаёт. В
реальном ране барьер судил бы подставленный корень, а `branch-build.yml` рабочего дерева мог
бы отсутствовать или быть сломан как угодно.

### B-3 (БЛОКЕР, моя находка) — B7 проверяет участие в агрегате ПОДСТРОКОЙ, нарушая собственную доктрину

`scripts/check_branch_build.sh:395-398`:

```python
    guard = "\n".join(st["run"] for st in steps_of(gate) if isinstance(st.get("run"), str))
    if f"needs.{WIRE_JOB}.result" not in guard:
```

Против собственной шапки, `:16-17`:

> `ДЕВЯТЬ ИНВАРИАНТОВ, каждый по РАЗБОРУ YAML, не грепом (греп зелен и против
> закомментированной строки, и против имени в соседнем echo)`

Минимальная форма — условие закомментировано ВНУТРИ того же `run:`-блока:

```
$ # в ci.yml: убрать ' || "${{ needs.branch-build-parity.result }}" != "success"' из if,
$ #           оставить его же строкой '# TODO вернуть: …'
$ BRANCH_BUILD_ROOT=<fix> bash scripts/check_branch_build.sh
ok    B7: `branch-build-parity` зовёт барьер и пробу, стоит в `status-check.needs` и участвует в его условии
VERDICT: PASS — …
REAL_EXIT=0
```

Вторая форма — условие вырезано, имя оставлено в соседнем `echo "справка: needs.branch-build-parity.result …"`:
тот же `ok B7`, `REAL_EXIT=0`.

**Почему это отдельный блокер, а не дубль B-1.** Здесь снимается не вызов барьера, а
ЕДИНСТВЕННОЕ свойство, ради которого весь набор заведён в `ci.yml`: что красное
`branch-build-parity` роняет `All checks passed`. Сценарий пробы S31 (`red_branch_build.sh:329-331`)
вырезает литерал ЦЕЛИКОМ, поэтому пиннит присутствие подстроки где угодно в тексте шага, а не
участие в условии.

### B-4 (БЛОКЕР) — `on.push.paths` / `paths-ignore` предмета не читаются: механизм мёртв при идеальном B2

`scripts/check_branch_build.sh:223-260` (`check_b2`). Ключ `paths` читается в файле ровно один
раз — `:353`, и это `deploy.yml`, не предмет.

```
$ # branch-build.yml: on.push.branches-ignore: [main] + paths: ['this-path-does-not-exist-anywhere/**']
ok    B2: `on.push.branches-ignore` = ['main'] — ветки собираются, `main` исключён
VERDICT: PASS — …
REAL_EXIT=0

$ # то же с paths-ignore: ['**']
ok    B2: …  VERDICT: PASS  REAL_EXIT=0
```

`paths` и `branches-ignore` комбинируются GitHub через AND: обычный коммит фича-ветки не
трогает несуществующий путь ⇒ сборка не рождается НИКОГДА, а B2 рапортует именно про тот ключ,
который выглядит безупречно. В пробе оси нет: единственное совпадение `paths` —
S24-deploy-paths (`red_branch_build.sh:283-286`), про `deploy.yml`.

### B-5 (БЛОКЕР, моя находка) — catch-all список B2 не покрывает `*/*`: глушится 30 веток из 31

`scripts/check_branch_build.sh:251`: `catchall = [p for p in ignore if p in ("**", "*", "**/**")]`.

```
$ # branch-build.yml: branches-ignore: [main, '*/*']
VERDICT: PASS — …
REAL_EXIT=0

$ git ls-remote --heads origin | sed 's#refs/heads/##'
всего веток: 31
с ровно одной косой (глушит '*/*'): 30
без косой: 1   → main
```

То есть конфигурация, проходящая барьер с `ok B2`, отключает сборку для ВСЕХ реально
существующих веток проекта (`feat/*`, `docs/*`, `fix/*`, `engine-dev/*`, `test/*`), оставляя
живым только `main`, который и так исключён. Это ровно тот исход, против которого B2 написан
(«механизм мёртв при формально верном ключе», `:253-254`), в форме, которую он не ловит.

### B-6 (БЛОКЕР, моя находка) — `static_false()` ловит только литералы: динамический `if` обезвреживает джоб

`scripts/check_branch_build.sh:181-185`:

```python
def static_false(cond):
    return isinstance(cond, (bool, str)) and str(cond).strip().lower() in ("false", "${{ false }}", "0",)
```

```
$ # branch-build.yml, джоб build-test: if: github.event_name == 'schedule'
VERDICT: PASS — …
REAL_EXIT=0
```

Событие workflow — исключительно `push` (`branch-build.yml:52-54`), `schedule` не наступает
никогда. Джоб не исполнится ни разу; B4 при этом сообщает «джоб … и все 6 шагов не
обезврежены». Семантически это то же самое, что `if: false` (S18), только через выражение.

### B-7 (БЛОКЕР) — B9 проверяет права только у workflow и у `build-test`; новый джоб с `write-all` проходит

`scripts/check_branch_build.sh:453`:

```python
    for node, where in ((bb, "workflow"), (job_of(bb, REF_JOB) or {}, f"джоб {REF_JOB}")):
```

```
$ # в branch-build.yml добавлен ПЕРВЫМ джоб:
$ #   exfil: {name: "Extra job (nothing to see here)", runs-on: ubuntu-latest,
$ #           permissions: write-all, steps: [{run: echo "elevated token available here"}]}
ok    B3: состав `build-test` совпадает с `ci.yml` шаг-в-шаг (6 шагов)
ok    B9: имена джобов не пересекаются с `ci.yml`; прав на запись не запрошено
VERDICT: PASS — …
REAL_EXIT=0
$ BRANCH_BUILD_ROOT=<fix> bash scripts/check_branch_build.sh | grep -ci exfil
0
```

Барьер не упоминает добавленный джоб ни одной строкой и при этом ПЕЧАТАЕТ ложное утверждение
«прав на запись не запрошено». Проверка клэша имён (`:436-437`) перечисляет все джобы предмета
и потому от нового джоба защищает; проверка ПРАВ — нет. Триггер — push любой не-main ветки,
то есть `GITHUB_TOKEN` с записью выдаётся на каждый push кого угодно, кто может создать ветку.

### B-8 (СУЩЕСТВЕННО) — паритет B3 не читает `env:` ни на одном из трёх уровней

`scripts/check_branch_build.sh:148-172` (`norm_steps`) сравнивает только `uses`+`with` и текст
`run:`. `env` не читается нигде (счёт выше — 0).

```
$ # три отдельные фикстуры: env на уровне WORKFLOW / ДЖОБА / ШАГА clippy
$ #   RUSTFLAGS: "--cap-lints allow"
ok    B3: состав `build-test` совпадает с `ci.yml` шаг-в-шаг (6 шагов)
VERDICT: PASS — …   (все три)
REAL_EXIT=0
```

Что `--cap-lints allow` действительно нейтрализует `-D warnings`, я проверил ИСПОЛНЕНИЕМ, а не
документацией — на отдельном крейте с нарушением `clippy::ptr_arg`:

```
$ cargo clippy --all-targets --all-features -- -D warnings ; echo
error: could not compile `clippyprobe` (lib) due to 1 previous error
REAL_EXIT_no_flags=101

$ RUSTFLAGS="--cap-lints allow" cargo clippy --all-targets --all-features -- -D warnings
    Finished `dev` profile … in 0.04s
REAL_EXIT_cap_lints=0
```

Это ровно тот тихий дрейф, который B3 объявляет своей целью: «Если он разойдётся, „ветка
собирается“ станет означать МЕНЬШЕ, чем `main`, — и это тихо» (`branch-build.yml:47-49`).

### B-9 (СУЩЕСТВЕННО) — вне поля зрения барьера: `defaults`, `runs-on`, `needs`, `strategy`

`grep -vE '^\s*#' scripts/check_branch_build.sh | grep -c` → `defaults`=0, `runs-on`=0,
`strategy`=0; `needs` читается один раз (`:390`) и только у `status-check` в `ci.yml`, не у
джоба предмета.

```
$ # defaults: {run: {shell: cat {0}}}  — штатная кастомная форма GitHub: скрипт печатается, не исполняется
VERDICT: PASS   REAL_EXIT=0
$ # build-test: needs: [nonexistent-phantom-job] — ссылка на несуществующий job ID
VERDICT: PASS   REAL_EXIT=0
$ # runs-on: [self-hosted, ghost-runner-never-registered]
VERDICT: PASS   REAL_EXIT=0
```

`needs` на фантом и ghost-runner — статически проверяемые дефекты, семантически равные
`if: false`, который B4 ловит. Оговорка честная: РАНТАЙМ-последствие каждой из трёх форм на
живом GitHub я не проверял (это стоило бы прогонов на чужих ветках); проверено и предъявлено
одно — что барьер эти ключи не читает вовсе. Для `defaults.run.shell` и `runs-on` частично
работает названный предел барьера («судит ТЕКСТ конфигурации, а не факт прогона»,
`:32-35`), но `needs:` — чистая текстовая проверка, которую он мог бы делать и не делает.

### B-10 (СУЩЕСТВЕННО) — отменённая сборка ветки НЕ наблюдается ничем: `gh pr checks` печатает `fail`, возвращает `0`

Воспроизведено НА ЖИВОМ РЕПОЗИТОРИИ, на моём одноразовом PR #36
(ветка `test/adv-verdict-cancel` от `7575d38` + один пустой коммит `6894658`; PR закрыт,
ветка удалена — см. §5).

```
$ git push origin HEAD:test/adv-verdict-cancel ; gh run cancel <Branch build run 32178998597>
$ gh pr create --draft --base main --head test/adv-verdict-cancel …   → PR #36
$ gh api …/commits/68946582…/check-runs
name=fmt + clippy + test (ветка)  status=completed  conclusion=cancelled
$ gh pr checks 36
All checks passed                                 pass   4s
fmt + clippy + test                               pass   9m21s
fmt + clippy + test (ветка)                       fail   5s      ← отменённый
… остальные 12 — pass
EXIT_36=0            (три прогона подряд: 0, 0, 0)

$ gh pr checks 36 --watch >/dev/null 2>&1; CHECKS=$?    # ровно форма gates.md §8
CHECKS=0
==> процедура §8 сказала бы: МЕРЖУ

контроль (настоящий failure, PR #35): EXIT_35=1
```

Я предсказывал обратное (что `cancelled` даст exit=1) — исполнение показало, что предсказание
неверно. Маскировка специфична именно для `cancelled`: `failure` распознаётся корректно.

**Что это значит.** Заявление шапки «`НЕ входит в агрегат` ⇒ merge не блокирует, процедура §8
цела» (`branch-build.yml:35-36`) верно. Но обратная сторона не названа и не покрыта ничем:
сигнал «ветка НЕ собралась» не наблюдает ни branch protection (чек не required), ни
`gh pr checks` (отменённое маскируется), ни барьер (он судит текст, не прогоны). Это ровно
свойство 4 целостности гейта — «наблюдает сбой, не наблюдает ОТСУТСТВИЕ» (`testing.md`), то
самое, которым автор обосновывает существование самого барьера (`check_branch_build.sh:7-14`),
применённое на уровень выше и не закрытое. Отмена здесь не экзотика по конструкции:
`cancel-in-progress: true` (`branch-build.yml:63`) делает отмену штатным событием этого
workflow, а B-16 повышает вероятность отменить руками не тот прогон.

### B-11 (СУЩЕСТВЕННО) — три висячие ссылки, и `verify_design_claims.sh` их структурно не видит

```
$ git cat-file -e 7575d38:docs/plans/ci-branch-trigger-probe-2026-08-18.md   → НЕТ
$ git cat-file -e 7575d38:docs/plans/plan-branches-and-ci-2026-08-19.md      → НЕТ
$ git cat-file -e origin/main:… (оба)                                        → НЕТ
$ git ls-tree -r --name-only 7575d38 research/arbitration/ | grep A-010      → пусто
$ git ls-tree -r --name-only origin/main research/arbitration/ | grep A-010  → пусто
$ git merge-base --is-ancestor origin/docs/session-handover-2026-08-19 origin/feat/harness-branch-build; echo $?
1
$ git merge-base --is-ancestor e350a779 7575d38; echo $?      # коммит, добавивший A-010
1
```

Оба `docs/plans/*` живут только на `origin/docs/session-handover-2026-08-19`, `A-010` — только
на `origin/docs/A-010-arbitration-2026-08-18`; ни одна из веток не предок предмета. Цитируются
эти пути в `branch-build.yml:5-6` (замер и «План §A»), `branch-build.yml:14` и `ci.yml:357`
(`A-010 §F`), и в теле коммита `7575d38` (первая строка: «План §A (docs/plans/…)»).

Гейт молчит:

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [7-RFC-PATH] путей-кандидатов (токены со слэшем в backtick'ах, docs/rfc/**.md): …
VERDICT: PASS (0 нарушений)      REAL_EXIT=0
```

CHECK 7 сканирует `docs/rfc/**.md`; `.github/workflows/*.yml` в область не входит. Класс —
тот самый, что 11.08 сработал трижды и вписан в `gates.md` §9 (`C-075` M1, `R-047` §2(i),
`R-049` B-1). После merge обоснование набора станет непроверяемым навсегда.

### B-12 (СУЩЕСТВЕННО) — замер в шапке ложен: 415 против «130», и «НОЛЬ» — не ноль

`branch-build.yml:8` и дословный повтор в `ci.yml:356-357`: «Эмпирика: 130 push-прогонов, ВСЕ
по `main`; настоящих push-прогонов на ветках — НОЛЬ».

```
$ gh api "repos/a3ka/hft-platform/actions/runs?event=push&per_page=100" --paginate \
    --jq '.workflow_runs[] | "\(.name)\t\(.head_branch)"' | sort | uniq -c | sort -rn
    415 CI	main
     91 Deploy to VPS	main
      3 Branch build	test/adv-lens3-cancel-probe     ← круг адверсария (линза 3)
      1 .github/workflows/ci.yml	fix/resource-oracle-barrier   ← push-прогон НА ВЕТКЕ
      1 Build and Deploy to VPS	main
      1 Branch build	feat/harness-branch-build
$ … --jq '.workflow_runs[].id' | wc -l
512
```

Направление довода замер не меняет (даже усиливает), но документ, чей собственный тезис —
«замер, а не ощущение», расходится в 3,2 раза и утверждает абсолют «НОЛЬ», который не ноль.
Числа впечатаны в два файла и в тело коммита.

### B-13 (ЗАМЕЧАНИЕ) — «M-69 прошёл ДВА круга критика»: кругов было три

```
$ git log --all --format='%H %ai %s' | grep -E 'C-(099|100|104)'
e38f800 2026-08-18 00:25:40  docs(M-69): C-099 — plan-time REJECT [critic]
18b3655 2026-08-18 09:18:59  docs(M-69): C-100 — plan-time REJECT [critic]
16fff45 2026-08-18 15:42:47  docs(M-69): C-104 — plan-time REJECT [critic]
$ git log -1 --format='%aI' 7575d38     → 2026-08-18T19:11:29+00:00
```

К моменту коммита кругов было ТРИ, и на третьем ветка уже собиралась (PR #32 создан
09:32:12, два прогона с `clippy: success` к 15:42 существовали). Утверждение стало неточным
между вторым и третьим кругом; см. §4 — часть заявленного линзой 2 по этому пункту я НЕ
подтвердил.

### B-14 (ЗАМЕЧАНИЕ) — цена удвоения самого дорогого джоба нигде не названа

На одном SHA `7575d38` (`gh pr checks 34`, сырой вывод):

```
fmt + clippy + test          pass  9m31s   .../runs/32175467563/…   ← ci.yml, event=pull_request
fmt + clippy + test (ветка)  pass  7m38s   .../runs/32175372301/…   ← branch-build.yml, event=push
```

Масштаб: 30 живых не-main веток в `origin`, 8 открытых PR (без моего одноразового), 144
коммита вне `main` за 7 суток. Отклонённая альтернатива (`push.branches: ['**']`) отвергнута по
КОРРЕКТНОСТИ, а не по цене, — поэтому это не противоречие, а пробел: набор, чья доктрина
«пределы, названные явно» (`check_branch_build.sh:31`), свою системную цену не называет.

### B-15 (ЗАМЕЧАНИЕ) — гомоглиф проходит проверку клэша имён B9

```
$ # branch-build.yml: name: Аll checks passed   (кириллическая А, U+0410)
ok    B9: имена джобов не пересекаются с `ci.yml`; прав на запись не запрошено
VERDICT: PASS   REAL_EXIT=0
```

Понижаю относительно линзы 1 сознательно: подделки required-контекста здесь я предъявить НЕ
смог и на живом GitHub не проверял — required-чек сверяется точной строкой, и гомоглиф ей,
по всей видимости, не удовлетворит. Остаётся текстуальная ложность декларации B9 и визуально
неотличимый двойник в списке `gh pr checks` — усилитель B-10 вместе с B-16.

### B-16 (ЗАМЕЧАНИЕ) — два почти одноимённых чека разной силы

Из одного вывода `gh pr checks 34`:

```
Branch build (ветка собирается тем же составом, что main)   pass  42s     ← ci.yml, В агрегате
fmt + clippy + test (ветка)                                 pass  7m38s   ← workflow «Branch build», ВНЕ агрегата
```

Статическая parity-проверка за 42 s носит имя, начинающееся с «Branch build»; собственно сборка
ветки называется иначе и живёт в workflow с именем «Branch build». Оператор, отменяющий
«дублирующий» прогон, с разумной вероятностью отменит сборку — и создаст B-10.

### B-17 (ЗАМЕЧАНИЕ, побочный эффект круга, НЕ предмета) — остаток предыдущей линзы в `origin`

```
$ git ls-remote origin 'refs/heads/test/*'
42b23eb0…  refs/heads/test/adv-lens3-cancel-probe
$ git log -1 --format='%s' 42b23eb
chore: adversarial lens3 probe — intentional fmt violation (throwaway, will be reverted)
```

Линза 3 отчиталась «коммит 42b23eb, позже удалённый вместе с веткой» — ветка в `origin` жива, с
намеренно сломанным `fmt`, ответвлена от предмета. Отчёт агента — гипотеза, git — факт
(`gates.md` §8). Не удаляю: ветка не моя (`branch-hygiene.md` п.7) — передаю founder'у.

---

## §3. Что я пытался сломать и НЕ СМОГ

Без этого раздела вердикт адверсария недействителен.

1. **Дотянуться сборкой ветки до прода.** Три отдельные попытки, все отбиты:
   `deploy.yml` `on.push.branches` = `['main']`; `workflow_run.workflows` = `['CI']`, «Branch build»
   там нет; фильтр `paths` деплоя предмет не накрывает — B6 честно проверяет все три и краснеет
   на каждой (S22/S23/S24 в моём прогоне пробы — `ok`). Коллизия concurrency на уровне
   WORKFLOW тоже отбита (`branch-build-${{ github.ref }}` ≠ `deploy-main`, B8 `ok`).
   *Оговорка:* job-level `concurrency: {group: deploy-main, cancel-in-progress: true}` барьер
   пропускает (B8 читает только workflow-уровень) — но предъявить, что GitHub реально отменил
   бы этим идущий деплой, я не могу без прогона на живом деплое, а его я не трогаю. Поэтому
   это НЕ вынесено отдельной находкой, а названо здесь: проверки нет, последствие не доказано.
2. **Сломать пробу как пробу.** Мутация хелпера `invokes()` → `return True` (вне восьми
   заявленных мутантов батареи) — проба поймала: `FAIL S32-echo-не-вызов`, `VERDICT: FAIL`,
   exit=1. Гипотеза «проба слепа ко всему, что вне `battery()`» опровергнута.
3. **Найти течь фикстур.** `каталогов red-branchbuild-* до: 0, после уборки: 0`; независимый
   замер каталогов `/tmp` вокруг прогона: `18004` → `18004`. Реестр в файле + `trap EXIT`
   работают; база снимается ДО создания `WORK`, поэтому страж не вакуумен.
4. **Найти ложный позитивный контроль.** `P0-честная` и `P1-on-закавычен` зелены, все 37
   стабов красные, и красные ПО СВОЕЙ проверке (четвёртый аргумент `expect_red` пиннит
   причину, а не только код) — сверено построчно, а не по итоговой строке.
5. **Обойти замок §11.** `bash scripts/check_docs_freeze.sh origin/main 7575d38` → exit=0:
   `.github/workflows/**` и `scripts/**` в зону `in_zone()` (`check_docs_freeze.sh:63`)
   действительно не входят, `FOUNDER-APPROVED` не требуется. Заявление коммита верно.
6. **Найти неаддитивную правку `ci.yml`.** `needs` агрегата на ветке — строгое надмножество
   `needs` на `origin/main` (+`branch-build-parity`, ни один член не убран); условие
   fail-closed расширено тем же членом. Заявление «правка строго аддитивна» верно.
7. **Найти ложную ссылку на приём `deploy_catchup.py` A6.** `grep -n 'A6' scripts/deploy_catchup.py`
   → строки 737-748, приём существует и описан так, как заявлено.
8. **Опровергнуть «`branches: [main]` с первого коммита».** Почти верно и не стоит находки:
   `25fe2c8` — ВТОРОЙ коммит репозитория (первый, `70d12f7`, — docs-only), но именно он вводит
   `ci.yml`, и `push.branches: [main]` в нём с рождения файла (`git show 25fe2c8:.github/workflows/ci.yml`).
9. **Найти регресс `design-claims` на дереве слияния.** `verify_design_claims.sh --merge-preview
   origin/main` → `VERDICT: PASS (0 нарушений)`, exit=0; предмет от `main` не отстаёт вовсе.

**Отдельно — наблюдение, не находка.** Механизм не действует на ветках, не содержащих сам файл:
моя первая проба ответвилась от `origin/main`, и `Branch build` не запустился ни разу
(`gh run list --branch … --json name` → пусто, 12 попыток за 48 с). Это штатное поведение
GitHub, но означает, что до merge покрытие ограничено потомками этой ветки, а после merge —
ветками, созданными/обновлёнными после точки слияния. В шапке, перечисляющей «чего этот
workflow не даёт» (`branch-build.yml:39-43`), этого нет.

---

## §4. Что заявлено линзами и МНОЙ НЕ ПОДТВЕРЖДЕНО

1. **Линза 2, F4, часть «б»: «ветка КОМПИЛИРОВАЛАСЬ во всех трёх раундах, значит „ни разу не
   собравшись“ буквально ложно».** НЕ воспроизводится. `gh pr list --search M-69` →
   PR #32 создан `2026-08-18T09:32:12Z`; вердикты `C-099` (00:25:40) и `C-100` (09:18:59)
   вынесены ДО него, а все три прогона — `event: pull_request`, самый ранний `09:32:14`. К
   моменту первых двух кругов на ветке не было ни одного завершённого прогона, то есть
   формулировка «прошёл два круга, ни разу не собравшись» для тех двух кругов ВЕРНА.
   Подтверждена только недосчитанность кругов (B-13); утверждение о ложности — нет.
2. **Линза 1, F5: severity «СУЩЕСТВЕННО» для гомоглифа.** Слепота B9 воспроизведена, но
   заявленный риск («риск путаницы у людей и агентов, читающих статус») — единственное, что я
   могу предъявить; обхода merge-гейта нет. Понижено до ЗАМЕЧАНИЯ (B-15).
3. **Линза 1, F4: severity «СУЩЕСТВЕННО» для `needs: [phantom]` как самостоятельной находки.**
   Слепота воспроизведена и вошла в B-9, но как отдельный класс не выделена: рантайм-последствие
   («джоб неразрешим/не стартует») на живом GitHub я не проверял, а без этого утверждение о
   вреде остаётся декларацией — той же природы, что предел, который автор себе уже запретил.
4. **Линза 3, F2: «альтернатива отвергнута именно по цене, а цена дублирования не сопоставлена».**
   Факт удвоения подтверждён (B-14), причинная связка — нет: `branch-build.yml:17-30` отвергает
   `push.branches: ['**']` по КОРРЕКТНОСТИ (zero-SHA, force-push, два прогона на одном SHA), а
   не по цене. Формулировка линзы приписывает документу довод, которого в нём нет.
5. **Линза 3, F1, формулировка «в ТОЧКЕ, где это единственный раз важно».** Сам факт
   (`EXIT=0` при `cancelled`) подтверждён и усилен — я воспроизвёл его на СВОЁМ PR #36 и
   отдельно на форме с `--watch`. Но подана находка была как «маскировка в автоматическом
   merge»; проверка показала, что маскируется не сбой (настоящий `failure` даёт exit=1), а
   ОТСУТСТВИЕ завершения — это другой класс, и в B-10 он назван своим именем.

---

## §5. Ответ по `gates.md` §9 — (а) / (б) / (в)

**(а) Каждое утверждение О КОДЕ — командой на дереве слияния.** Предмет от `origin/main`
(`4878be1`) не отстаёт (`git rev-list --count 7575d38..origin/main` → 0), поэтому дерево
слияния = ветка; `verify_design_claims.sh --merge-preview origin/main` → `PASS (0 нарушений)`,
exit=0. Проверены поимённо: «состав дословно повторяет `build-test`» — верно (B3 `ok`, 6
шагов); «правка `ci.yml` строго аддитивна» — верно (§3 п.6); «зона вне замка §11» — верно
(§3 п.5); «приём взят у `deploy_catchup.py` A6» — верно (§3 п.7). **Ложны:** замер
«130 / НОЛЬ» (B-12); «ДВА круга критика» (B-13). **Не резолвятся на дереве слияния:** три
цитируемых документа (B-11).

**(б) Полномочия.** Зона правки — `.github/workflows/**` + `scripts/**`, харнесс-трек
(`harness-track.md` §2), полным циклом идти не обязана: прод-процессом не исполняется, норм не
меняет, journal/позицию/деньги испортить не может — все три вопроса §4 дают «нет». Замок §11
не задет (проверено исполнением). Граница C не затронута. Маршрут соблюдён: адверсарий со
свежим контекстом обязателен и вот он. Полномочия — в порядке.

**(в) Связность и висячие ссылки.** Внутренние перекрёстки набора связны: `branch-build.yml`
↔ `check_branch_build.sh` ↔ `red_branch_build.sh` ↔ `ci.yml` (`branch-build-parity`) — каждая
ссылка резолвится, имена джобов совпадают, `SUT`/`PROBE`/`WIRE_JOB` указывают на существующее.
Ссылки на `testing.md` (целостность гейта, свойство 4), `harness-track.md` §5, `TD-086`,
`TD-106`/`TD-062`, `C-062`, `C-096` B-4 — резолвятся. Висят три (B-11), и их не ловит ни один
существующий гейт.

---

## §6. Что требуется для снятия REJECT (минимально)

Блокеры B-1…B-7 закрываются проверками, каждая из которых — несколько строк разбора уже
загруженного YAML; каждая обязана прийти со СВОИМ сценарием пробы и своим мутантом батареи:

| # | проверка | сценарий пробы |
|---|---|---|
| B-1 | `invokes()` обязана требовать, чтобы сегмент был ПОСЛЕДНИМ в строке, либо отдельно валить `\|\| true` / `\|\| :` / `; true` после needle | стаб `bash scripts/check_branch_build.sh \|\| true` — и такой же для пробы |
| B-2 | шаг и джоб `branch-build-parity` не несут `env` с `BRANCH_BUILD_ROOT` | стаб с `env:` на шаге |
| B-3 | участие в fail-closed условии — по РАЗБОРУ строки условия (needle внутри `if [[ … ]]`, не в комментарии и не в `echo`), а не подстрокой всего `run:` | два стаба: закомментировано; вынесено в `echo` |
| B-4 | `on.push` предмета не несёт `paths`/`paths-ignore` | два стаба |
| B-5 | catch-all список дополнить `*/*`, `**/*`, `*/**` — либо перевернуть: список РАЗРЕШЁННЫХ исключений | стаб `branches-ignore: [main, '*/*']` |
| B-6 | `if` на джобе/шаге допустим только из белого списка (пусто либо явно перечисленное) — всё прочее FAIL | стаб `if: github.event_name == 'schedule'` |
| B-7 | `writes()` перечисляет ВСЕ джобы предмета, а не workflow + `build-test` | стаб «новый джоб с `permissions: write-all`» |

B-8/B-9 — тем же приёмом (`env` в паритет B3; `defaults`, `runs-on`, `needs` предмета — в B4).
B-10 требует не барьера, а решения: либо `branch-build-parity` дополнительно проверяет ФАКТ
свежего завершённого прогона на ветке, либо предел «отменённая сборка не наблюдается ничем»
пишется в шапку явно, рядом с остальными. B-11/B-12/B-13 — правка текста: втянуть цитируемые
документы в предмет либо перестать на них ссылаться, и привести числа к замеру.

---

## Done Block

```
$ pwd; git rev-parse --short HEAD; git log -1 --format='%s'
/tmp/hft-adv-verdict
7575d38
feat(harness): ветка собирается в CI — сборщик + барьер паритета + проба [architect]

$ bash scripts/check_branch_build.sh >/dev/null 2>&1; echo exit=$?
exit=0

$ bash scripts/tests/red_branch_build.sh --battery 2>&1 | tail -3
сценариев исполнено: 39  ok: 39  FAIL: 0
каталогов red-branchbuild-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main >/dev/null 2>&1; echo exit=$?
exit=0

$ bash scripts/check_docs_freeze.sh origin/main 7575d38 >/dev/null 2>&1; echo exit=$?
exit=0

$ # 15 фикстур-атак, каждая: BRANCH_BUILD_ROOT=<fix> bash scripts/check_branch_build.sh
$ # REAL_EXIT: B-1a=0 B-1b=0 B-2=0 B-3a=0 B-3b=0 B-4a=0 B-4b=0 B-5=0 B-6=0 B-7=0
$ #            B-8(workflow/job/step)=0,0,0  B-9(defaults/needs/runs-on)=0,0,0  B-15=0
$ # мутация invokes()->True против пробы: PROBE_EXIT=1 (FAIL S32-echo-не-вызов)

$ gh pr checks 36 --watch >/dev/null 2>&1; echo CHECKS=$?     # отменённая сборка ветки
CHECKS=0
$ gh pr checks 35 >/dev/null 2>&1; echo EXIT=$?               # контроль: настоящий failure
EXIT=1

$ gh pr close 36 ; git push origin --delete test/adv-verdict-cancel
✓ Closed pull request #36
 - [deleted]         test/adv-verdict-cancel

$ bash scripts/reserve_artifact_id.sh C
C-105       (дубль C-106, взятый параллельным прогоном того же аудита, снят --release)
```

**Уборка:** мой одноразовый PR #36 закрыт, ветка `test/adv-verdict-cancel` удалена из `origin`;
фикстуры лежат вне репозитория (scratchpad) и в дерево не попадали (`git status --porcelain`
пуст до коммита этого файла); кэш `target/` моего worktree убран при сдаче
(`branch-hygiene.md`, Worktree lifecycle п.3).
