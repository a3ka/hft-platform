<!-- GATE-META
milestone: M-48
audited_repo: a3ka/hft-platform
audited_base: 6f628cb04b6cbf42aacd85b94f32ad7fbd21e934
audited_head: b5683355d88061e6445c6620e5acd0e4d133789d
verdict: REJECT
-->

# R-180 — PR-гейт ветки `harness/aggregate-holds` (6 коммитов): достижимость артефакта покрытия + сторож агрегата CI

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL).
**Дата:** 2026-09-15.
**Предмет:** `origin/harness/aggregate-holds` @ `b568335`, база `6f628cb` (merge-base с `origin/main` @ `4196ff7`).
**Вердикт: REJECT.** Один блокер (F-1, MAJOR, путь физического удаления данных) и один
блокер-по-форме (F-2, ложная посылка в обоснованиях прод-конфига). Пять коммитов из шести —
годные и ценные; блокирует ОДИН — `bc00f99`.

---

## Block-scope

| коммит | пути | зона по `scope-guard.md` | вердикт |
|---|---|---|---|
| `13c48dd` [architect] | `scripts/check_retention_composition.sh`, `scripts/tests/red_retention_composition.sh` | architect: `scripts/{check_*.sh}` + пробы — своя | OK |
| `9ab00fa` [engine-dev] | `docker-compose.yml` (volumes `journal-retention`) | carve-out engine-dev: «корневой `docker-compose.yml` — только объявление операторских ручек СВОИХ сервисов» | OK |
| `bc00f99` [engine-dev] | `docker-compose.yml` (`/cold` → `:ro`) | тот же carve-out | зона OK, **содержание — блокер F-1/F-2** |
| `569ea36` [engine-dev] | `.github/workflows/ci.yml` (новый джоб) | харнесс-проводка | OK |
| `a5abec7` [architect] | `.github/workflows/ci.yml` (условие агрегата) | харнесс-проводка | OK |
| `b568335` [architect] | `scripts/check_ci_aggregate.sh`, `scripts/tests/red_ci_aggregate.sh` | architect: барьер + проба | OK |

**Мандат тестера описал скоуп НЕПОЛНО** и это важно, а не придирка: в §D сказано
«только `.github/workflows/` + `scripts/check_*.sh` + `scripts/tests/red_*.sh`». В дифе есть
ТРЕТИЙ носитель — корневой `docker-compose.yml`, то есть прод-конфигурация сервиса, который
УДАЛЯЕТ сегменты журнала. Именно в нём и лежат оба блокера. Скоуп, названный неверно,
направляет ревью мимо предмета.

**Block-C (contracts):** `crates/contracts/**` не тронут — contract-RFC не требуется.
**Риск-блок (`gates.md` §5):** `crates/risk|killswitch|oms|venue-*` не тронуты ⇒ risk-critic по
ПУТЯМ не обязателен. См., однако, F-1: предмет лежит на пути НЕОБРАТИМОГО удаления данных, и
это класс, для которого §5 и заведён по существу, а не по списку каталогов.
**FA (`gates.md` §4, M-66):** `crates/**` не тронуты ⇒ `check_review_fa.sh` даёт `SKIP`
(`:57`), `FA-WAIVER` не требуется. Инвариант всё равно называю, потому что предмет — семантика
журнала: **`JR-I-2`** (`docs/fa/journal.md:112`) — «`seq` строго монотонен без дыр; разрыв при
чтении → abort (не «пропустить»)». Удаление горячего сегмента, чья «холодная копия» не
существует вне контейнера, производит ровно такой разрыв — и делает его необратимым.
Отдельная находка F-5: в `docs/fa/journal.md` НЕТ ни одного инварианта про cold-copy/prune
(живые — `JR-I-1..12`, первый свободный `JR-I-13`), хотя типовой барьер `ColdCopyProof`
описан в коде как «зеркало `RiskApproved`».

---

## F-1 (БЛОКЕР, MAJOR) — `--cold` указывает НЕ на монтирование: `ColdCopyProof` выдаётся против копии, живущей на эфемерном слое контейнера

**Что.** Обёртка передаёт бинарю путь ХОСТА, а compose монтирует холодное хранилище в ДРУГОЙ
каталог внутри контейнера:

- `deploy/bin/journal-retention-cron.sh:30` — `JOURNAL_COLD_DIR="${JOURNAL_COLD_DIR:-/mnt/journal-cold}"`;
- `deploy/bin/journal-retention-cron.sh:74` — `--cold "${JOURNAL_COLD_DIR}"`;
- `docker-compose.yml` (сервис `journal-retention`) — `"${JOURNAL_COLD_DIR:-/mnt/journal-cold}:/cold:ro"`, то есть внутри контейнера хранилище лежит в `/cold`;
- `/etc/cron.d/hft-journal-retention` на проде — `JOURNAL_COLD_DIR=/mnt/journal-cold`.

Итог: бинарю сказано `--cold /mnt/journal-cold`, а по этому пути внутри контейнера НЕТ
монтирования. Замер на проде (сырой вывод в Done Block ниже):

```
ls: cannot access '/mnt/journal-cold': No such file or directory
drwxr-xr-x 2 root root 4096 Jul 14 18:43 /cold
```

**Почему это опасно, а не косметика.** `crates/journal/src/segments.rs` (`verify_cold_copy`):
первым вызовом идёт `fs::create_dir_all(cold_root)?`, затем при отсутствии `dst` —
`File::create(&dst)` и побайтовое копирование, затем сверка sha256 **только что созданной
копии с источником** — она, разумеется, совпадает — и выдача `ColdCopyProof`. На эфемерном
слое контейнера все три операции УДАЮТСЯ; предъявлено исполнением на проде:

```
MKDIR-OK
ЗАПИСЬ-НА-ЭФЕМЕРНЫЙ-СЛОЙ-УДАЛАСЬ
-rw-r--r-- 1 root root 8192 Sep 15 16:47 /mnt/journal-cold/fake-cold-copy
```

`ColdCopyProof` документирован как ТИПОВОЙ барьер: «удалить невыгруженный сегмент невозможно
ВЫРАЗИТЬ в этом API — это типовой барьер, а не дисциплина оператора (тот же приём, что
`RiskApproved<Order>` в риск-слое)». В прод-композиции этот барьер **обходится конфигурацией**:
proof выдаётся против копии, которая исчезает вместе с контейнером.

**Что сегодня держит удаление — и почему на это нельзя опираться.** Не proof, а ДВА
независимых монтирования: `journal-data:/journal:ro` (то есть `prune_segment` →
`fs::remove_file` получает EROFS) и `RETENTION_MODE=dry-run` в cron'е (в `DryRun`
`execute_retention` не копирует и не удаляет — `segments.rs:3878`). Оба — внешние по отношению
к инварианту: одна правка режима, и proof становится единственным сторожем, а он уже пробит.

**Почему это стало РЕАЛЬНО достижимо именно этой веткой.** До `9ab00fa` каждый кандидат
отклонялся ДО обращения к cold-хранилищу: прод-лог — `checkpoint coverage artifact absent
(fail-closed)` на всех 18 последних строках, `offloaded: 0 pruned: 0 freed_bytes: 0`. Ветка
(верно!) снимает этот отказ — и первый же `--mode apply`, ради которого работа и делается
(`TECH-DEBT.md:3679`: «`TD-020` остаётся OPEN до первого успешного `apply`»), пойдёт по коду,
где proof ничего не доказывает.

**Почему это не поймал ни один гейт — механизм, а не невнимательность.** Оракул
`scripts/verify_delivery_M-08.sh:289-290` НОРМАЛИЗУЕТ оба пути в один sandbox-каталог:

```
a=${a//\/mnt\/journal-cold/${sb}/cold}
a=${a//\/cold/${sb}/cold}
```

то есть фикстура своими руками устраняет ровно то расхождение, которое проверяет. Плюс
`verify_delivery_M-08.sh:62` — это `grep -qE '/cold' docker-compose.yml`, то есть проверка по
ТЕКСТУ. Это тот же класс, который ветка справедливо называет в шапке своего же барьера:
«проверка должна быть по ВЫЗОВУ, а не по тексту» (`testing.md`).

**Чего требую (проектирует architect, `gates.md` §4 — reviewer описывает, не проектирует).**
(i) привести `--cold` и точку монтирования к одному пути ВНУТРИ контейнера; (ii) расширить
`check_retention_composition.sh` второй парой — `--cold` ↔ монтирование холодного хранилища
(барьер уже умеет ровно эту проверку для `/ckpt`, нужен второй вызов той же логики);
(iii) RED-оракул на границу носителя: `verify_cold_copy` против пути, который НЕ является
монтированием, обязан отказывать, а не выдавать proof.

## F-2 (БЛОКЕР) — обоснование `bc00f99` ложно по прод-форме: у локальной холодной копии НЕТ того писателя, на которого ссылается коммит

Тело `bc00f99` и комментарий, попадающий в прод-конфиг, утверждают:

> «единственный ПИСАТЕЛЬ холодной копии — `journal-offsite-cron.sh` (deploy/bin); retention её
> только ЧИТАЕТ … `deploy/bin/journal-offsite-cron.sh` НЕ ТРОНУТ: он пишет в `/mnt/journal-cold`
> НА ХОСТЕ, до монтирования».

Замер опровергает это дважды:

1. `deploy/bin/journal-offsite-cron.sh:90` — `DST_URL="${JOURNAL_OFFSITE_DST:-u659392-sub1@u659392-sub1.your-storagebox.de:journal/}"`, и на проде `/etc/cron.d/hft-journal-offsite` задаёт `JOURNAL_OFFSITE_DST=u659392-sub1@…your-storagebox.de:journal/`. Это rsync на УДАЛЁННЫЙ Storage Box по SSH; в `/mnt/journal-cold` offsite не пишет НИЧЕГО и никогда не писал.
2. `/mnt/journal-cold` на прод-хосте ПУСТ, mtime `Jul 14 18:43` — то есть с момента создания каталога в него не попало ни одного файла.

Следствие: `:ro` не «закрепляет конструкцией» существующее разделение писателей — оно убирает
ЕДИНСТВЕННОГО писателя локальной холодной копии, которым был сам `verify_cold_copy`. Это
делает вилку: либо (а) `--cold` остаётся рассогласованным и proof выдаётся против эфемерной
копии (F-1), либо (б) `--cold` починят на `/cold`, и тогда при `:ro` копию создать НЕКОМУ ⇒
`prune` невозможен НИКОГДА ⇒ `freed_bytes: 0` навсегда, то есть ровно тот симптом, который
ветка взялась лечить. Обе ветви плохи, и выбор между ними сделан не будет, пока конфиг несёт
ложную посылку.

Класс — `TD-138`: документ (здесь — комментарий в прод-конфиге и тело коммита) обосновывает
инвариант механизмом, которого на этом пути не существует. Правка `bc00f99` либо снимается,
либо переписывается по замеру и сопровождается решением по вилке (a)/(b).

## F-3 (NOTE) — сторож агрегата слеп к ТРЕТЬЕЙ двери: джоб, забытый в `needs` целиком

`check_ci_aggregate.sh` сверяет ДВА списка внутри блока агрегата: `needs` ↔ `needs.*.result`.
Но добавление джоба требует правки ТРЁХ мест (определение джоба, `needs`, условие), и шапка
барьера называет только два: «Добавление джоба требует правки ДВУХ мест». Джоб, определённый в
`ci.yml` и не попавший в `needs` ВОВСЕ, барьером не виден.

Предъявлено живым состоянием файла, а не мутацией: в `ci.yml` **20** джобов, в `needs` — **19**;
двадцатый — `branch-health` (`:488`). Барьер при этом печатает `PASS  все 19 джоб(ов) …`.
В данном случае исключение ЗАКОННО и обосновано в самом `ci.yml:495-505` (наблюдатель, не
барьер; `A-010` §H, `A-011` §1.6) — но для барьера законное исключение и забытый джоб
неразличимы, а проба `red_ci_aggregate.sh` (S1-S4) этот сценарий не покрывает.

Не блокер: барьер делает то, что заявлено заголовком, и мутационный контроль это подтверждает.
Требование — назвать предел в секции «ПРЕДЕЛ НАЗВАН» самого скрипта (зона architect'а), а не
оставлять его подразумеваемым; механизация (белый список наблюдателей + сверка со ВСЕМИ
джобами файла) — кандидат, решение architect'а.

## F-4 (NOTE) — «task #1/#2/#3» СТОЛКНУЛИСЬ с нумерацией §Tasks того милестоуна, к которому относятся

`commit-discipline.md`: «Обязательная ссылка на milestone/task в теле или subject». Subject'ы
`fix(harness): task #1 …`, `… task #2 …`, `feat(ci): task #3 …` номеруют задачи, не называя
милестоун, — и это не формальность. Предмет относится к `M-48`
(`milestones/M-48-checkpoint-bootstrap-and-ops.md`, задача #6 — ровно «путь артефакта обязан
СОВПАДАТЬ с `--coverage-out`… канарейка КОМПОЗИЦИИ двух обёрток»), а в §Tasks этого же
милестоуна номера 1-3 заняты СОВСЕМ другим: `advance_to`, `history_start_seq`,
сужение fail-loud. Читатель `git log` отобразит «task #2» на чужую строку таблицы.

Зона при этом чистая: `M-48` §Allowed paths прямо перечисляет `docker-compose.yml` и
`deploy/**` за engine-dev'ом (`:100-107`) — Block-scope претензий не имеет.

## F-5 (NOTE) — `docs/fa/journal.md` не знает инвариантов cold-copy/prune

Живые — `JR-I-1..12` (`:111-218`), первый свободный — `JR-I-13` (`:221`). Ни один не про
холодную копию и не про удаление горячего сегмента, при том что в коде типовой барьер
`ColdCopyProof` объявлен зеркалом `RiskApproved`. Инвариант, живущий только в комментарии кода,
ослабляется молча — тот самый довод, по которому `gates.md` §9 требует критика на правки FA.
Зона architect'а; фиксирую как пробел, не как блокер этой ветки.

---

## Что в ветке ГОДНО (и почему это стоит приземлить отдельно от `bc00f99`)

1. **`13c48dd` + `9ab00fa` + `569ea36` — достижимость `/ckpt` закрыта по делу.** Дефект был
   реален (прод-лог: 18 отказов `checkpoint coverage artifact absent`, `freed_bytes: 0` при
   живом артефакте `509966298`), барьер ловит его по ДОСТИЖИМОСТИ тома, а не по совпадению
   строк, и проведён в CI тем же PR — то есть RED не живёт в `main` (`gates.md` §8).
2. **`a5abec7` — закрытие живой дыры в ОБЯЗАТЕЛЬНОМ чеке защиты ветки.** `secret-material`
   («ключи не живут в репозитории») и `roadmap-sync` стояли в `needs`, но их результат не
   читался: агрегат печатал «All checks passed» при их падении. Проверено чтением условия до и
   после: 19 из 19 читаются.
3. **`b568335` — сторож на класс, с предъявленным анти-плацебо.** Мутация моей рукой:
   снятие `roadmap-sync` из условия → `VERDICT: FAIL (1)` с именем джоба, возврат → `PASS`.
   Сторож включён в ОБА списка агрегата, то есть его собственное падение держит merge.

Эти пять коммитов я готов одобрить БЕЗ изменений, как только `bc00f99` снят или переписан.

---

## F-6 (БЛОКЕР ПО МАРШРУТУ) — адверсарного вердикта на оба новых барьера НЕТ

`harness-track.md` §3 ставит в маршрут **обязательного адверсария со свежим контекстом**, и §3
«Что сохранено» объясняет, почему без него трек становится дырой: «автор себя не проверяет: на
M-60 автор объявил `F-086-1` закрытым … Независимый агент построил [стаб], и набор оказался
зелёным против него». Вердикт-артефакт там же назван сохранённым требованием.

Замер по репозиторию:

```
$ grep -rl "check_retention_composition\|check_ci_aggregate\|red_ci_aggregate\|red_retention_composition" research/
(ничего)
```

Ноль файлов. У СОСЕДНЕГО барьера того же класса адверсарий есть —
`research/critiques/C-206-rollout-composition-adversary.md`, — то есть норма в проекте живая и
исполнялась месяц назад на точно таком же предмете. Здесь её место занял прогон тестера,
который §3 прямо не считает заменой («роль tester отдельным шагом» убрана ИМЕННО потому, что
прогон делает автор, а ломает — адверсарий).

Что это стоило практически: F-3 в этом вердикте — ровно та находка, которую производит
адверсарий, а не прогон. Прогон 7/7 зелёных её не видит по построению: барьер зелен, проба
зелена, а третья дверь не заперта.

## F-7 (NOTE, маршрут) — правка прод-конфига поехала облегчённым треком

`harness-track.md` §4 задаёт тест на попадание в трек: «три вопроса подряд, все три должны дать
"нет": исполняется ли это прод-процессом? меняет ли это норму, а не инструмент? может ли ошибка
здесь испортить journal/позицию/деньги?». Корневой `docker-compose.yml` в части сервиса
`journal-retention` — это argv и монтирования ПРОД-процесса, который удаляет сегменты журнала;
первый и третий вопросы дают «да». По §4 такая правка идёт полным циклом §2, а не треком, и
попадает под RAW-гейт `gates.md` §1 (класс 1 — «механизм удаления/ретеншена данных», критик на
СИЛЬНОЙ модели), обоснованный там же асимметрией горизонта: «ошибка в раскладке журнала
проявляется через МЕСЯЦЫ и стоит переписывания всего накопленного».

Ни критика, ни risk-critic'а в цепочке нет (см. F-6 — вердиктов на предмет ноль). Это не
придирка к форме: именно непроверенная посылка прод-комментария и есть F-2, а «второй парой»,
которую не проверил никто, оказался путь физического удаления.

Границы C (`gates.md` §0.1) правка НЕ пересекает: состав записываемых и удаляемых данных не
меняется, режим остаётся `dry-run`, меняется достижимость артефакта. Подпись founder'а
требуется не на эту правку, а на первый `--mode apply` — и вот его включать до закрытия F-1
нельзя.

---

## Done Block

Все команды — в чистом worktree `/tmp/hft-reviewer-agg` на `b568335` (detached, `origin/harness/aggregate-holds`).

```
$ git log -1 --format='%H %h %s'
b5683355d88061e6445c6620e5acd0e4d133789d b568335 harness: сторож агрегата — каждый needs обязан влиять на исход [architect]

$ git status --porcelain
(пусто)

$ git merge-base origin/main origin/harness/aggregate-holds
6f628cb04b6cbf42aacd85b94f32ad7fbd21e934

$ git diff --stat origin/main...origin/harness/aggregate-holds
 .github/workflows/ci.yml                   |  31 +++++++-
 docker-compose.yml                         |  20 +++++-
 scripts/check_ci_aggregate.sh              |  74 +++++++++++++++++++
 scripts/check_retention_composition.sh     | 101 ++++++++++++++++++++++++++
 scripts/tests/red_ci_aggregate.sh          |  63 +++++++++++++++++
 scripts/tests/red_retention_composition.sh | 110 +++++++++++++++++++++++++++++
 6 files changed, 396 insertions(+), 3 deletions(-)

$ cargo fmt --all -- --check; echo exit=$?
fmt exit=0

$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3
    Checking recorder v0.0.0 (/tmp/hft-reviewer-agg/crates/recorder)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.07s
clippy exit=0

$ cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=970 failed=0 (блоков: 228)
exit=0

$ bash scripts/check_ci_aggregate.sh; echo exit=$?
PASS  все 19 джоб(ов) из needs влияют на исход агрегата
PASS  в условии нет джобов, отсутствующих в needs

VERDICT: PASS — списки зависимостей и проверяемых результатов совпадают
exit=0

$ bash scripts/check_retention_composition.sh; echo exit=$?
PASS  производитель объявляет артефакт покрытия: /ckpt/covered_through_seq
PASS  потребитель просит ТОТ ЖЕ путь: /ckpt/covered_through_seq
PASS  потребитель монтирует ТОТ ЖЕ том «gateway-ckpt» в /ckpt — артефакт достижим

VERDICT: PASS — путь артефакта покрытия достижим потребителем, а не только назван
exit=0

$ bash scripts/tests/red_ci_aggregate.sh; echo exit=$?
PASS  S1 списки совпадают ⇒ барьер ЗЕЛЁН
PASS  S2 джоб в needs без проверки результата ПОЙМАН — дефект, найденный в ci.yml 2026-09-09
PASS  S3 проверка джоба, отсутствующего в needs, ПОЙМАНА — агрегат читал бы пустой результат
PASS  S4 отсутствие агрегата ⇒ SETUP-ОТКАЗ (rc=2), а не тихий зелёный
VERDICT: PASS (4/4)
exit=0

$ bash scripts/tests/red_retention_composition.sh; echo exit=$?
PASS  S1 честная конфигурация ⇒ барьер ЗЕЛЁН (иначе он запрещал бы верное)
PASS  S2 потребитель без монтирования ПОЙМАН — ровно дефект, найденный на проде 2026-09-08
PASS  S3 РАЗНЫЕ тома при совпадающем пути ПОЙМАНЫ — то, что сравнение строк не видит
PASS  S4 отсутствие производителя артефакта ПОЙМАНО
PASS  S5 расхождение путей производителя и потребителя ПОЙМАНО
VERDICT: PASS (5/5)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=6f628cb0… bash scripts/check_protected_artifacts.sh; echo exit=$?
OK: защищённые артефакты целы на HEAD (6f628cb..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=6f628cb0… bash scripts/check_artifact_ids.sh; echo exit=$?
OK: ни один новый артефакт не введён (диапазон 6f628cb..HEAD)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=6f628cb0… bash scripts/check_gate_meta.sh; echo exit=$?
VERDICT: PASS — вердиктов проверено: 0, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=6f628cb0… bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | tail -2
PASS  [7-RFC-PATH] путей-кандидатов … всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0
```

### Мутационный контроль сторожа агрегата (моей рукой, не по отчёту автора)

```
$ cp .github/workflows/ci.yml /tmp/ci-mut.yml   # + снятие roadmap-sync из условия
$ CI_WORKFLOW=/tmp/ci-mut.yml bash scripts/check_ci_aggregate.sh; echo exit=$?
FAIL  1 джоб(ов) в needs НЕ ВЛИЯЮТ на исход агрегата — их падение
      игнорируется, агрегат печатает «All checks passed», защита ветки пускает merge:
        · roadmap-sync
      Лечится добавлением в условие: || "${{ needs.<job>.result }}" != "success"
PASS  в условии нет джобов, отсутствующих в needs

VERDICT: FAIL (1)
exit=1
```

### Замер прод-формы (`testing.md` — «форма прода снимается ЗАМЕРОМ»), основание F-1/F-2

```
$ ssh … root@167.233.192.131 'grep -vE "^#" /etc/cron.d/hft-journal-retention'
JOURNAL_COLD_DIR=/mnt/journal-cold
RETENTION_RETAIN_DAYS=14
RETENTION_MODE=dry-run
7 4 * * * root /root/hft-platform/deploy/bin/journal-retention-cron.sh
*/15 * * * * root flock -n /var/lock/hft-gateway-checkpoint.lock /root/hft-platform/deploy/bin/gateway-checkpoint-cron.sh

$ ssh … 'grep -vE "^#" /etc/cron.d/hft-journal-offsite | grep DST'
JOURNAL_OFFSITE_DST=u659392-sub1@u659392-sub1.your-storagebox.de:journal/

$ ssh … 'ls -la /mnt/journal-cold'
total 8
drwxr-xr-x 2 root root 4096 Jul 14 18:43 .
drwxr-xr-x 3 root root 4096 Jul 14 18:43 ..

$ ssh … 'grep -n "cold=" /var/log/hft/journal-retention.log | tail -1'
24545:  cold=/mnt/journal-cold

$ ssh … 'tail -4 /var/log/hft/journal-retention.log'
=== отчёт ===
  mode=DryRun
  offloaded: 0  pruned: 0  failed: 0
  freed_bytes: 0

$ ssh … 'docker compose run --rm --no-deps --entrypoint sh journal-retention -c "ls -ld /cold /mnt/journal-cold"'
ls: cannot access '/mnt/journal-cold': No such file or directory
drwxr-xr-x 2 root root 4096 Jul 14 18:43 /cold

$ ssh … 'docker compose run --rm --no-deps --entrypoint sh journal-retention -c "mkdir -p /mnt/journal-cold && echo MKDIR-OK; dd if=/dev/zero of=/mnt/journal-cold/fake-cold-copy bs=1k count=8 && ls -l /mnt/journal-cold"'
MKDIR-OK
ЗАПИСЬ-НА-ЭФЕМЕРНЫЙ-СЛОЙ-УДАЛАСЬ
-rw-r--r-- 1 root root 8192 Sep 15 16:47 /mnt/journal-cold/fake-cold-copy
```

### Ярус C — что искал грепом (`reading-map.md` §2; целиком эти файлы не читаются)

- `TECH-DEBT.md`: `TD-020`, `TD-048`, `TD-193`, `TD-202`, `retention|ретеншен|cold|холод|prune`.
  Найдено и использовано: `TD-020` `retention-implemented-but-never-invoked` (`:3599`, OPEN до
  первого успешного `apply` — `:3679`), `TD-006` (`:3739`, закрывается вместе с `TD-020`),
  `TD-193` (`:132`, восстановление копии не проверялось НИ РАЗУ — прямо рядом с F-1/F-2).
- `docs/PENDING-SIGNATURE.md`: `retention|удал` — П-005 (состав данных, граница C; к этой
  ветке не применяется: режим удаления не меняется, меняется достижимость артефакта).
- `docs/fa/journal.md`: `JR-I-` — живые `JR-I-1..12`, свободен `JR-I-13`; про cold/prune нет
  (основание F-5).

---

## Вердикт

**REJECTED.** Merge не выполнен; ветка не вливалась. Условие APPROVED: снять или переписать по
замеру `bc00f99` (F-2) и закрыть F-1 — композиция `--cold` ↔ монтирование, расширение
`check_retention_composition.sh` второй парой и RED на границу носителя. F-6 — блокер по маршруту (адверсарий
харнесс-трека обязателен и отсутствует). F-3/F-4/F-5/F-7 — NOTE, merge не держат.

**Отдельно, вне этой ветки:** F-1 — живой прод-дефект, а не дефект правки. Он заводится
карточкой `TECH-DEBT` (зона reviewer'а) независимо от судьбы ветки, чтобы не исчез вместе с
ней, и `--mode apply` не включается до его закрытия.
