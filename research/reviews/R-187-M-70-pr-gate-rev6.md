<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: c739a64e335e9710d1c919fd1e007f06612b047d
audited_head: cd9f73eec34188878f09fb967c73b8ec3c47a73d
verdict: APPROVE
-->

# R-187 — M-70 (полосы глубины), PR-гейт круга 6: **APPROVED**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-09-18T16:05Z
**Предмет:** `origin/docs/M-70-rev2` @ `cd9f73e` против `origin/main` @ `c739a64`.
Диапазон с моего прошлого вердикта — ОДИН коммит (`cd9f73e`), только спека.
**Предыдущие круги:** `R-170`, `R-171`, `R-172`, `R-184`, `R-186`.
**Мои деревья:** `/tmp/r187-MP` (слияние в порядке GitHub, `58ad3b6`), `/tmp/r187-ORDER-A`
(слияние в обратном порядке, `b81b3c2`), `/tmp/r186b-BRANCH` (ветка, `cd9f73e`).

**Главное одной строкой.** **Мой собственный блокер `Б-1` из `R-186` — НЕ ДЕФЕКТ ПРЕДМЕТА,
а дефект МОЕГО ВОСПРОИЗВЕДЕНИЯ.** Я собрал дерево слияния в порядке родителей, обратном
тому, в котором его собирает GitHub, и барьер `check_protected_artifacts` честно ответил на
заданный ему — другой — вопрос. Проверено мной обоими способами: **деревья бит-идентичны,
вердикты противоположны, разница исключительно в порядке родителей.** Блокер `Б-2` закрыт
по-настоящему и токеном — подтверждено мутацией. Находка `Н-1` исполнена. Merge разрешён.

---

## Б-1 из `R-186` СНЯТ — и снят не правкой, а верным воспроизведением

### Замер: один и тот же диф, два порядка, противоположные исходы

```
$ # ПОРЯДОК A — мой в R-186: база = ВЕТКА, вливаем main
$ git worktree add /tmp/r187-ORDER-A --detach origin/docs/M-70-rev2 && git merge origin/main
A на новой базе: b81b3c2 parents=cd9f73e c739a64
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) \
    bash scripts/check_protected_artifacts.sh; echo exit=$?
FAIL  milestones/M-75-heatmap-window-decoupling.md: артефакт ИСЧЕЗ с HEAD, и ни один коммит его не удалял
exit=1

$ # ПОРЯДОК B — как собирает GitHub: база = main, вливаем ВЕТКУ
$ git worktree add /tmp/r187-MP --detach origin/main && git merge origin/docs/M-70-rev2
дерево слияния: 58ad3b6 parents=c739a64 cd9f73e
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) \
    bash scripts/check_protected_artifacts.sh; echo exit=$?
exit=0

$ # СОДЕРЖИМОЕ ОДНО И ТО ЖЕ — различаются ТОЛЬКО родители:
деревья совпадают: ДА          # git rev-parse HEAD^{tree} обоих = один SHA
```

Тот же опыт на прежней базе (`6c088ba`) дал те же исходы и бит-идентичные деревья
`ee107c30875ee26c62fb4d2f34f5bf3992fc125f` — то есть эффект не зависит от того, какой
именно `main` вливается.

### Механика — почему порядок решает (`check_protected_artifacts.sh:150-154`)

`mainline` = дерево БАЗЫ плюс цепочка **ПЕРВЫХ родителей** `base..HEAD`. Fallback (поиск
удалившего коммита ВНЕ диапазона) разрешён только когда путь в этом push'е не рождался и в
магистраль не попадал. Замер обеих магистралей — мой, не рассуждение:

```
$ # ПОРЯДОК A
$ git rev-list --first-parent $BASE..HEAD | wc -l
63
$ коммитов магистрали A с путём milestones/M-75: 23

$ # ПОРЯДОК B
$ git rev-list --first-parent $BASE..HEAD --format='%h %s'
7ea507f Merge remote-tracking branch 'origin/docs/M-70-rev2' into HEAD
$ коммитов магистрали B с путём milestones/M-75: 0
```

При обратном порядке 63 коммита ветки становятся «магистралью», 23 из них написаны ДО
переезда `M-75` в архив и несут путь ⇒ путь «был в магистрали» ⇒ fallback запрещён ⇒
`removed_by` пуст ⇒ FAIL «исчез, и ни один коммит не удалял». В верном порядке магистраль —
ровно один merge-коммит, путь в ней не появлялся, fallback разрешён и **находит настоящих
удаляющих**.

### Настоящий порядок GitHub — проверен мной на шести мержах, а не принят на слово

```
$ for m in c739a64 6c088ba e1f304e c564fa0 9d852a3 3f399ec; do … done
c739a64: p1=6c088ba  (предыдущая вершина main)   p2=ddc5532  (ветка)
6c088ba: p1=e1f304e  p1 в first-parent-цепочке main: 1        p2=a2c2c83
e1f304e: p1=c564fa0  … 1                                      p2=aa49fcc
c564fa0: p1=9d852a3  … 1                                      p2=c576514
9d852a3: p1=3f399ec  … 1                                      p2=61839ca
3f399ec: p1=95d2422  … 1                                      p2=3cbf9ec
```

Во ВСЕХ шести первый родитель — прежняя вершина `main`, второй — вершина ветки. Порядок B.

**Согласен с поправкой: блокер снят верным воспроизведением.** Это стоит записать отдельно,
потому что тем же способом будут воспроизводить и следующие: **инструмент проекта уже делает
это правильно, а я делал вручную и неправильно.** `scripts/verify_design_claims.sh`
`--merge-preview` (`:178-186`) создаёт временный worktree ИЗ base-ref и вливает в него HEAD —
то есть порядок GitHub. `gates.md` §8 предписывает именно его; я в `R-186` собрал дерево
руками и получил другой вопрос вместо предписанного.

**Предел поправки назван честно:** я НЕ утверждаю, что барьер верен вообще. `TD-163` (MAJOR,
OPEN) описывает живой ложно-положительный класс того же барьера, и он не закрыт. Я утверждаю
ровно одно и ровно про этот предмет: **на дереве, которое соберёт GitHub, барьер зелен.**

---

## Б-2 из `R-186` — закрыт ПО-НАСТОЯЩЕМУ, и это токен. Мутационный контроль

`ALLOW-SUBJECT-CHANGE` в теле `cd9f73e` называет причину поимённо (три правки
`scripts/verify_M-70.sh` после `R-185`, каждая по требованию `C-226`/`C-227`, последняя
судима `C-228`). Проверил, что зелёное держится ИМЕННО ИМ, а не порядком родителей:

```
$ # МУТАЦИЯ: тело cd9f73e без строки ALLOW-SUBJECT-CHANGE (ALLOW-ARTIFACT-DELETE оставлен)
$ git commit --amend -F /tmp/mut-msg.txt        → ec8d094
$ git checkout --detach origin/main && git merge ec8d094     → 33591b9, parents=6c088ba ec8d094
$ EVENT_NAME=pull_request PR_BASE_SHA=… bash scripts/check_gate_meta.sh; echo exit=$?
FAIL  research/reviews/R-185-…md: subject-lock — после проходного вердикта (NOTE) тронут
      класс «гейт»: scripts/verify_M-70.sh
VERDICT: FAIL (1)
exit=1
```

Барьер краснеет ровно на снятие токена — значит зелёное не случайно.

---

## ЧЕСТНАЯ ПОПРАВКА к телу `cd9f73e` — `ALLOW-ARTIFACT-DELETE` барьер (1) НЕ гасит

Тело `cd9f73e` подаёт оба токена как две развязки двух барьеров. **Для `protected-artifacts`
это неверно, и ошибка исходно МОЯ:** `R-186` назвал «`ALLOW-ARTIFACT-DELETE` в коммите
диапазона» первой развязкой, автор добросовестно её исполнил. Замер:

```
$ # (а) токен В ДИАПАЗОНЕ, порядок обратный — барьер ВСЁ РАВНО красен
ORDER-A protected_artifacts exit=1
$ # (б) токен УДАЛЁН из тела, порядок GitHub — барьер ВСЁ РАВНО зелен
$ git commit --amend -F /tmp/mut2-msg.txt   # ALLOW-ARTIFACT-DELETE: 0, ALLOW-SUBJECT-CHANGE: 1
$ EVENT_NAME=pull_request … bash scripts/check_protected_artifacts.sh; echo exit=$?
NOTE  milestones/M-75-heatmap-window-decoupling.md: ALLOW-ARTIFACT-DELETE в 70ca5c8
NOTE  milestones/M-77-frame-book-continuity.md: ALLOW-ARTIFACT-DELETE в 8562a06
exit=0
```

Причина — в коде (`check_protected_artifacts.sh:359-374`): токен читается ТОЛЬКО внутри
цикла `for c in ${removed_by}`, то есть у коммитов, которые ПУТЬ УДАЛЯЛИ. В порядке A
`removed_by` пуст (сообщение так и говорит: «ни один коммит его не удалял»), цикл не
исполняется, и токен не читается ни разу. В порядке B fallback находит настоящих
удаляющих — `70ca5c8` и `8562a06`, штатные close-out'ы, — **и у них СВОИ токены на месте**.

**Итог: токен `ALLOW-ARTIFACT-DELETE` в `cd9f73e` избыточен.** По существу он правдив
(переезд в `docs/archive` штатный, утраты нет), аудит-следу не вредит, удалять ради чистоты
не требую — это стоило бы нового круга гейта ради строки, ничего не меняющей. Но **утверждение
тела коммита, что им разблокирован барьер (1), в вердикт записывается как НЕВЕРНОЕ**, чтобы
следующий, кто упрётся в этот класс, не искал развязку не там.

---

## `Н-1` из `R-186` — ИСПОЛНЕН

§5 переписан от ПОСЛЕДСТВИЯ: «усиление обязательно, если merge меняет ФОРМУ ИЛИ СОСТАВ
выдачи — независимо от того, какой настройкой вызвано», срабатывает здесь по бампу
`GATEWAY_SCHEMA_VERSION` 9→10, а не по `GATEWAY_BANDS`. Состав гейта дан таблицей из ПЯТИ
пунктов; пункт 4 — **засечь ВРЕМЯ до появления свежего `ckpt-*.bin` и сдвига
`covered_through_seq`**. Утверждения спеки о коде проверены мной НА ДЕРЕВЕ СЛИЯНИЯ, а не на
ветке:

```
$ sed -n '4196,4203p' crates/gateway/src/lib.rs
        // (3) gateway_schema_version
        let gw_v = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
        if gw_v != GATEWAY_SCHEMA_VERSION {
            return None;
$ grep -n 'pub const GATEWAY_SCHEMA_VERSION' crates/gateway/src/lib.rs
117:pub const GATEWAY_SCHEMA_VERSION: u32 = 10;
```

Оба утверждения истинны. Признаю ценность пункта 4 прямо: числа холодной пересборки нет ни у
кого, включая предыдущий бамп `M-68` — и мой собственный вердикт `R-156` его тогда не назвал.

---

## Block-scope — PASS

```
$ git show --numstat --format='' cd9f73e
34	5	milestones/M-70-depth-bands-enablement.md
```

Единственный коммит круга, единственный файл — спека, зона architect'а. `crates/**`,
`contracts/**`, `scripts/**` не тронуты. `Allowed paths` §2 не расширены.

## Block-DoneBlock — PASS. Числа сняты МОИМ прогоном на дереве слияния

## Block-C — N/A. `crates/contracts/**` и `docs/rfc/**` не тронуты; шаг `C` гейта — PASS

## Block-risk — N/A по `gates.md` §5

Диф не трогает `crates/risk|killswitch|oms|venue-*`. Предмет — read-only выдача,
order-egress отсутствует как класс; состав ЗАПИСИ не тронут (шаг `C` гейта, не моё слово).
`risk-critic` не требуется.

## Предъявление FA (`gates.md` §4, M-66) — живые ID на ДЕРЕВЕ СЛИЯНИЯ

Диапазон трогает `crates/gateway/**` и `crates/gateway-serve/**`; FA обоих —
`docs/fa/viz-backend.md`. ID проверены `grep`'ом на дереве слияния `58ad3b6`:

- **`VB-I-4`** (`docs/fa/viz-backend.md:202`) — аддитивность формы: v1-консюмер не ломается,
  смена формы ⇒ bump `GATEWAY_SCHEMA_VERSION`. Именно эта половина и делает деплой-гейт §5
  усиленным: бамп 9→10 инвалидирует живой слепок прода.
- **`VB-I-5`** (`:203`) — метка достоверности принадлежит ТОЧКЕ, а не ряду.
- **`VB-I-10`** (`:208`) — bounded-window snapshot: предел выдачи не куплен ценой памяти.
- **`VB-I-9`** (`:207`) — граница плоскостей D6: `gateway-serve` не читает/не пишет журнал.

**Ярус C — грепом по предмету** (`reading-map.md` §2; `PROJECT-STATE.md` целиком НЕ читал и
не утверждаю, что читал): `TD-163` (`TECH-DEBT.md:110`, OPEN — прямо этот барьер),
`TD-159` (`:107`), `TD-161` (`:108`), `TD-198` (`:88`), `TD-205` (`:145`), `TD-044`, `TD-021`;
`M-70` — `docs/ROADMAP.md:95` п.9.

---

## Проверка `TD-163` — деплой ПОСЛЕ merge не замрёт

`TD-163` (MAJOR) описывает ровно этот барьер в PUSH-форме: PR зелёный, а `main` после merge
краснеет, `Gate on CI (fail-closed)` валится и деплой замирает. Обязан был проверить ту
форму, которая и замораживает прод, — проверил:

```
$ # дерево слияния 58ad3b6, форма события — push, как зовёт CI на main
$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_protected_artifacts.sh; echo exit=$?
NOTE  milestones/M-75-heatmap-window-decoupling.md: ALLOW-ARTIFACT-DELETE в 70ca5c8
NOTE  milestones/M-77-frame-book-continuity.md: ALLOW-ARTIFACT-DELETE в 8562a06
exit=0
$ EVENT_NAME=push PUSH_BEFORE=… bash scripts/check_gate_meta.sh; echo exit=$?
exit=0
```

Класс `TD-163` на этом предмете НЕ воспроизводится: у GitHub-мержа первый родитель — прежняя
вершина `main`, магистраль push'а состоит из одного merge-коммита, fallback разрешён. Карточку
это не закрывает — она про ветки, чей путь попадает в магистраль иначе.

---

## Находки круга 6 — ни одна не блокирует merge

### `Н-1` (перенесено из `R-186` `Н-2`, не исправлено) — ролевая git-личность

```
$ git log -1 --format='%an <%ae>' cd9f73e
engine-dev <t@t.local>
```

Коммит `[architect]` подписан `engine-dev`. `branch-hygiene.md` п.6 прямо запрещает ролевые
`user.name`; метка в subject'е верна, аудиту вреда нет, механического барьера у правила нет.
Карточку завожу при close-out'е — `TECH-DEBT.md` моя зона.

### `Н-2` — НАБЛЮДЕНИЕ: мои worktree с НЕЗАПУШЕННЫМИ коммитами сносились во время прогона

За сессию трижды: `/tmp/hft-rev-m70-r6-A`, `/tmp/hft-rev-m70-r6-B`, `/tmp/r186b-ORDER-A|B|MUT`
исчезли посреди работы (`fatal: Unable to read current working directory`), тогда как дерево
на коммите, достижимом из `origin` (`/tmp/r186b-BRANCH`), уцелело. Все снесённые несли
ЛОКАЛЬНЫЕ merge/amend-коммиты — то есть ровно то, что `branch-hygiene.md` §Worktree lifecycle
п.6 запрещает сносить молча. **Виновника НЕ НАЗЫВАЮ — не доказал:** ни один из шести барьеров
слова `worktree` не содержит (`grep` пуст), `crontab` у пользователя нет. Замеры успели
записаться в файлы до сноса, на выводы это не повлияло. Записываю как наблюдение, требующее
отдельного замера, а не как установленный дефект.

### `Н-3` — `origin/main` сдвинулся ПОСРЕДИ моего прогона

`6c088ba` → `c739a64` (PR #191, один файл `docs/workflow/session-handover-2026-09-18.md`).
Первый комплект замеров стал снят на устаревшей базе; **пересняты все до единого на
`c739a64`**, старые в вердикт как действующие не подаются. Отмечаю потому, что это ровно тот
класс, который `gates.md` §8 называет пределом `strict: false`.

---

# Вердикт: **APPROVED**

Оба блокера `R-186` сняты: `Б-2` — токеном (мутация подтверждает), `Б-1` — тем, что его не
было: дефект был в моём воспроизведении. `Н-1` исполнен. Инженерных претензий нет третий круг
подряд; ни один пункт не возвращается engine-dev'у.

**CI — последнее слово.** Всё выше — воспроизведение барьеров в форме, которой их зовёт CI,
а не сам CI. Merge идёт по КОДУ ВОЗВРАТА `gh pr checks --watch`, а не по этому файлу.

## Done Block

```
$ pwd; git log -1 --oneline
/tmp/r187-MP
58ad3b6 Merge remote-tracking branch 'origin/docs/M-70-rev2'   (parents: c739a64 cd9f73e)

$ bash scripts/verify_M-70.sh > /tmp/MP-verify.out 2>&1; echo "VERIFY_EXIT=$?"
VERIFY_EXIT=0
$ grep -c '^PASS' /tmp/MP-verify.out; grep -c '^FAIL' /tmp/MP-verify.out; tail -1 …
42
0
VERDICT: PASS

$ # ШЕСТЬ барьеров агрегата на дереве слияния, форма pull_request
protected_artifacts exit=0
gate_meta           exit=0
docs_freeze         exit=0
artifact_ids        exit=0
review_fa           exit=0
roadmap_sync        exit=0

$ # те же в форме push (main после merge — класс TD-163)
protected_artifacts PUSH exit=0
gate_meta           PUSH exit=0

$ # обратный порядок родителей — контроль, что мой R-186 воспроизводим и ошибочен
ORDER-A protected_artifacts exit=1        ← дерево ИДЕНТИЧНО, вердикт противоположен

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0
$ bash scripts/verify_design_claims.sh; echo exit=$?      # на ветке, для сравнения
VERDICT: PASS (0 нарушений)
exit=0
```

## Cross-references

- `research/reviews/R-186-M-70-pr-gate-rev5.md` (круг 5; его `Б-1` снят здесь как мой дефект)
- `research/reviews/R-185-m70-enablement-second-opinion.md` (носитель subject-lock)
- `research/critiques/C-226`/`C-227`/`C-228` (круг `gates.md` §9 по rev10-rev12)
- `milestones/M-70-depth-bands-enablement.md` §5 (деплой-гейт, пять пунктов)
- `docs/fa/viz-backend.md` `VB-I-4`/`VB-I-5`/`VB-I-9`/`VB-I-10`
- `TECH-DEBT.md` `TD-163` (этот барьер, OPEN), `TD-159`, `TD-161`, `TD-198`
- `.claude/rules/gates.md` §4, §8 · `.claude/rules/branch-hygiene.md` п.6
