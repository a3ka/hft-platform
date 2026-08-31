<!-- GATE-META
milestone: M-73
audited_repo: a3ka/hft-platform
audited_base: e55f2f0956f83b6169310252442ac61775b10125
audited_head: b394a830352e4569c24bdc6e9abfac2cb4b2860c
verdict: APPROVE
-->

# R-158 — M-73 (офсайт-копия по расписанию), круг 4: PR-time reviewer, **APPROVED**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-30T19:00Z
**Предмет:** `e55f2f0..b394a83` на `origin/feat/R1-offsite-schedule` (круг 4 — три коммита:
`712b96d` architect, `dda3a80` + `b394a83` engine-dev). Полный диапазон PR —
`d1221b1..b394a83`, 12 файлов.
**Номер выдан механизмом** (`gates.md` §12): `bash scripts/reserve_artifact_id.sh R` → `R-158`.

---

## ВЕРДИКТ КОРОТКО

**APPROVED.** Единственный блокер круга 3 (`R-157` `Б-5` — мёртвый сторож сбоя источника)
закрыт, и закрыт правильно: два звена — фикс дева и оракул архитектора, который этот фикс
пиннит. Я не поверил ни отчёту дева, ни отчёту тестера: **прогнал мутационный контроль сам, в
обе стороны и третьим — на несостоявшийся setup.** Все три мутации ведут себя как обязаны.

Примечание `Н-10` тоже закрыто. Требование 4 `R-157` (прод-прогон с argv в логе) выполнено
ЧАСТИЧНО — и это единственное, что я принимаю с явно названным остатком, а не молча: argv в
лог не пишется по построению обёртки, и добавление такой записи — правка вне списка задач
круга 4. Провенанс исполняемой ревизии закрывается СИЛЬНЕЕ, чем просил `R-157`: после merge
`deploy.yml` сам ставит скрипт на прод из фиксированного SHA, и первый АВТО-прогон я проверю
глазами в post-merge гейте (`gates.md` §8).

**Три новых замечания (`Н-13`..`Н-16`) не блокируют** и заводятся картами долга — они старше
этого милестоуна либо лежат в зоне architect'а.

---

## Что я прочитал, и чем греп ограничен (ярус C, `reading-map.md` §2)

**Открыто целиком:** `.claude/agents/reviewer.md` · `CLAUDE.md` + `.claude/rules/*` (gates,
testing, scope-guard, commit-discipline, branch-hygiene, handoff-block) ·
`milestones/M-73-offsite-schedule.md` · `research/reviews/R-157-M-73-offsite-schedule-r3.md`
(весь) · `scripts/tests/red_offsite_pipeline_rc.sh` (весь, 156 строк) ·
`deploy/cron.d/journal-offsite` · `deploy/cron.d/builder-prune` · `deploy/README.md` §1.4/§2 ·
`.github/workflows/deploy.yml` §Deploy via SSH · `docs/fa/ops.md` §6/§7.1 ·
`docs/fa/viz-backend.md` §таблица инвариантов.

**Ярус C — грепом, называю ЧТО искал** (`reviewer.md` startup п.6):

```
$ grep -nE '^\- \*\*TD-[0-9]+\*\*' TECH-DEBT.md | grep -iE 'offsite|cron|built-not-wired|retention|storage'
3110:- **TD-020** `retention-implemented-but-never-invoked`
$ grep -nE 'TD-020|TD-006\b|TD-124|TD-135' TECH-DEBT.md            → 12 попаданий
$ grep -niE 'offsite|M-73|П-023|builder-prune' PROJECT-STATE.md    → 0 попаданий
```

Существенное: `TD-020` — **acceptance-ворота Ф0, OPEN**, закрывается «вместе со Storage Box +
первым успешным retention apply» (`TECH-DEBT.md:1605`); `TD-006` (диск) закрывается вместе с
ним. В `PROJECT-STATE.md` офсайта нет — милестоун ещё не приземлялся, что и ожидается; строку
добавляю я при close-out.

**Предъявление FA (M-66).** Барьер прогнан МНОЙ его же проводкой на диапазоне PR:

```
$ BASE=$(git merge-base origin/main HEAD)          # d1221b1ca932d0b8e95403c2849308ed6e7b9ce2
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_review_fa.sh; echo exit=$?
research/reviews/R-151-R1-offsite-schedule.md: VB-I-1
research/reviews/R-157-M-73-offsite-schedule-r3.md: VB-I-1
research/reviews/R-151-R1-offsite-schedule.md: VB-I-3
research/reviews/R-157-M-73-offsite-schedule-r3.md: VB-I-3
exit=0
```

`SKIP` здесь НЕ наступает: диапазон PR трогает `crates/gateway/src/lib.rs` (`564f103`, правка
doc-комментария) — прод-код. Живые инварианты называю сам, открыв файлы на ревизии `b394a83`:

| ID | текст (сокращён) | место |
|---|---|---|
| `VB-I-1` | Каждый индикатор — чистый редьюсер над `journal::stream`; детерминизм-тест обязателен | `docs/fa/viz-backend.md:188` |
| `VB-I-3` | Read Gateway read-only: gateway не импортирует journal-writer/recorder-write | `docs/fa/viz-backend.md:190` |
| `OPS-I-2` | Журнал существует минимум в двух местах; удаление горячей копии — только через `ColdCopyProof` | `docs/fa/ops.md:470` |
| `OPS-I-3` | Холодная копия периодически ВОССТАНАВЛИВАЕТСЯ и читается (drill), а не только создаётся | `docs/fa/ops.md:471` |
| `OPS-I-8` | Тишина в потоке — алерт P1: «жив, но не работает» (`TD-011`/`TD-014`-класс) | `docs/fa/ops.md:476` |

`OPS-I-8` — класс закрываемой находки `Б-5`; `OPS-I-2` — то, ради чего милестоун существует;
`OPS-I-3` остаётся НЕ выполненным и после этого merge (см. `Н-16`).

`FA-WAIVER` в этом вердикте НЕ ставлю и ставить нельзя: тронутый крейт `gateway` — FA-крейт
(`viz-backend.md`), живое эхо предъявлено выше. Отмечаю отдельно: оба коммита дева круга 4
несут `FA-WAIVER: deploy-only diff …` в теле `%B` — там барьер waiver НЕ читает (`TD-165`,
`check_review_fa.sh:279` ищет строку в review-ФАЙЛЕ), и само утверждение верно для диапазона
круга 4, но неверно для диапазона PR. Вреда нет (waiver не требуется вовсе), форму см. `Н-15`.

---

## Block-scope — ПРОЙДЕН

```
$ git diff --name-only e55f2f0 b394a83
deploy/bin/journal-offsite-cron.sh
deploy/cron.d/journal-offsite
scripts/tests/red_offsite_pipeline_rc.sh
scripts/verify_M-73.sh

$ git show --numstat --format='' 712b96d     # architect
156	0	scripts/tests/red_offsite_pipeline_rc.sh
16	0	scripts/verify_M-73.sh
$ git show --numstat --format='' dda3a80     # engine-dev
9	2	deploy/bin/journal-offsite-cron.sh
$ git show --numstat --format='' b394a83     # engine-dev
4	1	deploy/cron.d/journal-offsite
```

**Владение соблюдено поимённо, а не в среднем по диапазону.** Sacred-зона
(`scripts/verify_*.sh`, `scripts/tests/**`) тронута ТОЛЬКО коммитом architect'а `712b96d`;
`deploy/**` — только коммитами engine-dev'а. Это ровно таблица Allowed paths милестоуна.
`crates/**`, `milestones/*.md`, `docker-compose.yml`, `.github/**`, `PROJECT-STATE.md`,
`TECH-DEBT.md` в диапазоне круга 4 отсутствуют. Числа `--numstat` соответствуют заявленному в
телах коммитов (`branch-hygiene.md` п.9, симметричная проверка ПОСЛЕ коммита).

**Атомарность** (`commit-discipline.md`): три задачи круга — три коммита, каждый с ссылкой
`M-73` + номером задачи + меткой роли в subject'е. Бандла нет.
**Трейлеры:** `git log --format='%H%n%b' d1221b1..b394a83 | grep -ci 'co-authored-by'` → `0`.
**Удалений защищённых артефактов:** `git diff --name-status d1221b1...b394a83 | grep '^D'` → пусто.
**Зона замка §11** (`.claude/**`, `CLAUDE.md`, `docs/04-workflow.md`) диапазоном не тронута —
`docs-freeze` неприменим.

## Block-C — N/A, основание предъявлено

`crates/contracts/**` в диапазоне отсутствует; T1 не тронут, `SCHEMA_VERSION` не бампался.
`05-contract-layer.md` §4 неприменим; §Contract impact милестоуна утверждает то же.

## Block-risk — N/A по путям

`crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**` не тронуты.
`RISK-BLOCK` (`gates.md` §5) не включается, risk-critic в цепочке не требуется. Единственное
касание `crates/` — doc-комментарий в `gateway` (read-only консюмер журнала, `VB-I-3`), без
order-egress.

---

## Требования `R-157` к кругу 4 — поштучно

### 1. `Б-5` — фикс дева — **ЗАКРЫТ**

`deploy/bin/journal-offsite-cron.sh:320-326` (`dda3a80`):

```
_st=("${PIPESTATUS[@]}")
rsync_rc=${_st[1]:-0}
find_rc=${_st[0]:-0}
```

Снимок берётся ОДНИМ массивом ДО любых присваиваний; развязка `0|141) find_rc=0` и `set -u`
не тронуты. Шесть строк комментария называют механизм дефекта — не «исправлено», а ПОЧЕМУ
предыдущая форма была мертва.

### 2. `Б-5`-оракул — архитектор — **ЗАКРЫТ, и это лучшая часть круга**

`scripts/tests/red_offsite_pipeline_rc.sh` (156 строк) + шаг `task #1ter` в `verify_M-73.sh`.
Проба ИСПОЛНЯЕТ обёртку прод-формой со стаб-`PATH`; стабятся только `find`/`rsync`/`ssh`,
`nice`/`ionice`/`flock`/`date` настоящие. Судится тройка `(exit, наличие alert, наличие
last-success)` — то есть НАБЛЮДАЕТСЯ ОТСУТСТВИЕ, а не только сбой (`testing.md`, свойство 4).

### 3. `Н-10` — шапка `cron.d` — **ЗАКРЫТ**

`deploy/cron.d/journal-offsite:20-23` (`b394a83`): `--partial` → `--partial-dir=.rsync-partial`
плюс явное свойство («обрывок живёт В ПОДКАТАЛОГЕ, а НЕ под финальным именем»). Шаг `task #2`
гейта проверяет, что голого `--partial` в живом блоке нет.

### 4. Прод-прогон исправленной ревизии с argv в логе — **ЧАСТИЧНО; остаток назван**

Замер снят МНОЙ по ssh, не принят из отчётов:

```
$ ssh … 'cat /var/lib/hft/journal-offsite.last-success; date -u'
2026-08-30T16:43:22Z                       ← НЕ 13:29, как было на круге 3
Sun Aug 30 06:49:02 PM UTC 2026
$ ssh … 'tail -40 /var/log/hft/journal-offsite.log'
…Number of created files: 1 … Total transferred file size: 1.07G bytes
2026-08-30T16:43:18Z OK duration=26s       ← прогон копирующий
…Number of regular files transferred: 0 … Total transferred file size: 0 bytes
2026-08-30T16:43:22Z OK duration=0s        ← идемпотентный повтор
$ ssh … 'ls /var/lib/hft/*.alert'          → нет alert-файлов
$ ssh … 'grep -c "DST_URL=\|HFT_CRON_PRINT_ARGV" /var/log/hft/journal-offsite.log'
0                                          ← argv-блока в логе НЕТ
```

Прогон 16:43 совпадает по времени с коммитами дева (`16:42:07`/`16:42:14`), то есть
исполнялась ревизия круга 4 — но **доказательством это не является**: argv в лог не пишется
(обёртка печатает его только под `HFT_CRON_PRINT_ARGV=1` и выходит ДО side-эффектов), а
скрипта на проде уже нет. То есть требование `R-157` в его буквальной форме исполнимо только
правкой обёртки — вне списка задач круга 4.

**Почему я принимаю это, а не реджекчу.** Merge закрывает провенанс СИЛЬНЕЕ ручного прогона:
`.github/workflows/deploy.yml:288` ставит каждый `deploy/cron.d/*` в `/etc/cron.d/hft-*` на
разрешённом SHA, а `git reset --hard` кладёт саму обёртку в `/root/hft-platform/deploy/bin/`.
После merge исполняемая ревизия равна вершине `main` по построению, а не по совпадению времён.
Проверяю это глазами в post-merge гейте (`gates.md` §8), и это условие моего APPROVED, а не
пожелание.

---

## Мутационный контроль — МОЙ, три прогона (`testing.md`)

Оракул, который никто не ломал, — декларация. Ломал я, в изолированных копиях дерева.

**Мутация 1 — прямая: вернуть дефект `Б-5`** (`_st`-снимок → чтение `PIPESTATUS` после присваивания):

```
PASS  A SIGPIPE-развязка цела: find=141 rsync=0 ⇒ exit=0, alert=нет, успех=да
PASS  B сбой приёмника виден: find=0 rsync=12 ⇒ exit=12, alert=да, успех=нет
FAIL  C сбой ИСТОЧНИКА виден: find=1 rsync=0 ⇒ ПОЛУЧЕНО exit=0, alert=нет, успех=да;
                                              ОЖИДАЛОСЬ exit=1, alert=да, успех=нет
VERDICT: FAIL (1 из 3)     EXIT=1
```

Оракул краснеет РОВНО на предмете и только на нём. Привязка к дефекту доказана.

**Мутация 2 — обратная: что пришлось ослабить рядом** (снята развязка SIGPIPE, `0|141)` → `0)`):

```
FAIL  A SIGPIPE-развязка цела: find=141 rsync=0 ⇒ ПОЛУЧЕНО exit=141, alert=да, успех=нет
PASS  B …      PASS  C …
VERDICT: FAIL (1 из 3)     EXIT=1
```

Дешёвый фикс «любой ненулевой `find` — сбой» превратил бы КАЖДУЮ успешную копию в ложную
тревогу; сценарий `A` его ловит. Второй вопрос мутационного контроля закрыт.

**Мутация 3 — несостоявшийся setup** (стаб `find` не сделан исполняемым):

```
SETUP НЕ СОСТОЯЛСЯ: стаб find не подхватился PATH (получено rc=0, ждали 141)     EXIT=1
```

Проба падает, а не зеленеет по чужой причине. Свойство 3 целостности гейта — предъявлено.

**Дополнительно проверено прод-парсером cron'а** (этого не делает ни гейт, ни обёртки — см. `Н-14`).
Важно потому, что `deploy.yml:290-294` при `crontab -n ≠ 0` ОБРЫВАЕТ деплой:

```
deploy/cron.d/journal-offsite      crontab_n_exit=0
deploy/cron.d/builder-prune        crontab_n_exit=0
deploy/cron.d/journal-retention    crontab_n_exit=0
```

Merge деплой не сломает.

---

## Механизм на пути (DoD, `gates.md` §4) — ПОДКЛЮЧЕНИЕ ЕСТЬ, и это установлено замером

`R-157` собирался заводить карту «built-not-wired». Я проверил маршрут доставки и он ЕСТЬ:

```
$ git show origin/main:.github/workflows/deploy.yml | grep -n 'for src in deploy/cron.d\|install -m 0644\|CRON INSTALL FAILED'
284:  sudo install -m 0644 "$src" "/etc/cron.d/hft-${name}"
288:  for src in deploy/cron.d/*; do
293:  echo "=== CRON INSTALL FAILED — aborting deploy ===" >&2
```

Подтверждено состоянием прода: `/etc/cron.d/` содержит `hft-journal-retention` — ровно
единственный файл, который лежит в `deploy/cron.d/` на `main`. То есть авто-установка не
теория, а работающий путь. После merge появятся `hft-journal-offsite` и `hft-builder-prune`.

**Карту «built-not-wired» я поэтому НЕ завожу** — она была бы ложной. Взамен: подключение
проверяется глазами в post-merge гейте, и пока не проверено — милестоун не закрыт.

---

## Примечания (не блокируют; заведены картами долга)

**`Н-13` (MAJOR). `deploy/README.md:126` утверждает противоположное механизму.** Текст:
«`install /etc/cron.d/...` — **ОСОЗНАННЫЙ РУЧНОЙ ШАГ с подписью founder ★**, а НЕ авто-действие
`deploy.yml`». Факт: с `26b6228` (M-48, 2026-07-29) `deploy.yml` ставит `deploy/cron.d/*`
автоматически на каждом деплое, fail-closed. Раздел написан `eb0e6cc` (2026-07-15) — он
протух через две недели и живёт ложным больше месяца.

Практическое следствие именно здесь: этот merge ВКЛЮЧИТ на проде два расписания сам, а
runbook говорит founder'у, что CI так не делает — значит «eyes-on первого АВТО-прогона»,
который тот же раздел объявляет обязательным, никто не назначит. Вреда данным нет (копия —
create-only, `--delete` запрещён и это пиннится гейтом; `builder-prune` бьёт только кэш
BuildKit), и офсайт-копия санкционирована `П-023` по существу — поэтому НЕ блокер. Но это
`TD-138`-класс в документе, который читает оператор. **Зона architect'а** (`docs`, и правка
меняет заявленную governance-модель ⇒ `gates.md` §9 триггер). Карта — `TD-177`.

**`Н-14` (MINOR). Гейт не зовёт `crontab -n`, хотя деплой на нём обрывается.**
`grep -n "crontab -n" scripts/verify_M-73.sh` → пусто. Cron-файл, который не парсится,
роняет ВЕСЬ деплой проекта (`deploy.yml:290-294`), а поймать это можно одной командой на
файл. Я проверил руками прод-парсером (выше) — сегодня зелено. Зона architect'а
(`scripts/verify_*.sh` sacred). Карта — `TD-178`.

**`Н-15` (MINOR). Форма `FA-WAIVER` у дева.** Оба коммита круга 4 кладут waiver в тело
коммита; по `TD-165` барьер читает его в review-ФАЙЛЕ, и waiver обязан называть КОНКРЕТНЫЙ
крейт, а не «deploy-only diff». Здесь waiver не требуется вовсе (FA-крейт `gateway` предъявлен
живым эхом), так что вреда ноль — но форма воспроизводится третий раз подряд и стоит одной
строки в мандате. Без карты; сказано деву в §D.

**`Н-16`. `docs/fa/ops.md:399` протух, и `OPS-I-3` не выполнен.** Строка гласит: «боевой
журнал существует минимум в ДВУХ местах. **Сегодня — в одном.**» С 29.08 копия есть (85 ГБ на
приёмнике, подтверждено логом прода выше). При этом `OPS-I-3` (restore-drill) не проводился НИ
РАЗУ — то есть «два места» есть, а доказательства читаемости холодной копии нет. Правка строки
и заведение restore-drill — зона architect'а, close-out милестоуна. Карта — `TD-179` (drill),
строка `ops.md` — architect'у в §D.

**`Н-17`. `§Tasks` милестоуна устарел:** пять задач стоят `⏳ OPEN`, хотя закрыты; задачи
круга 4 (`Б-5`, `Н-10`) в таблицу не внесены вовсе. `milestones/*.md` — зона architect'а;
merge этим не блокируется (`gates.md` §4 такого требования не содержит), но при переводе
Status → DONE таблицу привести к факту.

---

## Done Block — прогон РЕВЬЮЕРА

```
$ pwd
/tmp/hft-rev-m73r4
$ git rev-parse HEAD
b394a830352e4569c24bdc6e9abfac2cb4b2860c
$ git status --porcelain
(до создания этого файла — пусто)

$ bash scripts/verify_M-73.sh > /tmp/rev4_verify.out 2> /tmp/rev4_verify.err
EXIT=0
$ grep -c '^PASS' /tmp/rev4_verify.out ; grep -c '^FAIL' /tmp/rev4_verify.out ; wc -c < /tmp/rev4_verify.err
18
0
0
   PASS: cargo fmt --all -- --check
   PASS: cargo clippy --all-targets --all-features -- -D warnings
   PASS: cargo test --all --quiet
   … task #0 (4 шага) · task #0 наблюдение отсутствия (2) · task #0 композиция (1)
   … task #1 · task #1bis · task #1ter · task #2 · task #3 · task #4 (2) · task #5
   VERDICT: PASS

$ bash scripts/tests/red_offsite_pipeline_rc.sh; echo oracle_exit=$?
PASS  A SIGPIPE-развязка цела: find=141 rsync=0 ⇒ exit=0, alert=нет, успех=да
PASS  B сбой приёмника виден: find=0 rsync=12 ⇒ exit=12, alert=да, успех=нет
PASS  C сбой ИСТОЧНИКА виден: find=1 rsync=0 ⇒ exit=1, alert=да, успех=нет
VERDICT: PASS (3/3)
oracle_exit=0

$ BASE=$(git merge-base origin/main HEAD)
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_review_fa.sh          → exit=0 (VB-I-1, VB-I-3)
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_gate_meta.sh          → VERDICT: PASS, вердиктов 3, exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_artifact_ids.sh       → OK
$ EVENT_NAME=pull_request PR_BASE_SHA=$BASE bash scripts/check_protected_artifacts.sh → OK
```

### Состояние мира (ярус S)

```
$ gh run list --branch main --limit 3
completed success Deploy to VPS  main workflow_run 33319502798 13s   2026-08-30T15:23:33Z
completed success CI (PR #128)   main push        33319046913 9m49s 2026-08-30T15:13:42Z
$ ssh … 'docker ps --format "{{.Names}} {{.Status}}"; df -h /; cat …/recorder.heartbeat'
hft-gateway-serve Up 7 hours (healthy) · hft-recorder Up 7 hours (healthy)
/dev/sda1 150G 93G 52G 65% /
{"events":2985028,"free_bytes":55360413696,"next_seq":410795907,"segment_index":478,"writable":true}
$ git -C /root/hft-platform rev-parse --short HEAD  → 1397b99
```

`main` зелёный, деплой доехал, прод здоров, heartbeat свежий, диск 65 %.

---

## Условие APPROVED (исполняется мной же, немедленно после merge)

1. `gh pr checks --watch` зелёные → `gh pr merge --merge --delete-branch`.
2. **Post-merge деплой-гейт** (`gates.md` §8): дождаться CI + Deploy success; проверить по ssh,
   что `/etc/cron.d/hft-journal-offsite` и `/etc/cron.d/hft-builder-prune` УСТАНОВЛЕНЫ и что
   `/root/hft-platform/deploy/bin/journal-offsite-cron.sh` лежит исполняемым на вершине `main`.
3. **Eyes-on первого АВТО-прогона** в ближайшие `:22` — свежий `journal-offsite.last-success`,
   `OK` в логе, alert-файла нет, recorder не задет. Пока это не подтверждено глазами,
   милестоун не закрывается (`deploy/README.md` §2: «установлено» ≠ «работает»).
4. `PROJECT-STATE.md` + `TECH-DEBT.md` (`TD-177`/`TD-178`/`TD-179`) — мои, при close-out.

**Чего я не делаю:** не проектирую фиксы `Н-13`/`Н-14`/`Н-16` — это граница reviewer↔architect
(`gates.md` §4): я описываю дефект, защиту проектирует architect.

---

## ПОПРАВКА 2026-08-30T19:40Z — номера карточек долга были названы ЗАНЯТЫМИ

Выше, в `Н-13`/`Н-14`/`Н-16` и в разделе «Условие APPROVED», я назвал карточки `TD-177`,
`TD-178`, `TD-179`. **Все три уже заняты** другими предметами (`TD-177` — generation при
снятии подписки, `TD-178` — оракул точки входа терминальности, `TD-179` — побатчевое
продвижение курсора). Я взял номера по памяти вместо механизма — ровно то, что `gates.md` §12
запрещает: «Номер берётся ТОЛЬКО механизмом».

**Верные номера, выданные аллокатором** (`scripts/reserve_artifact_id.sh TD`, трижды):

| в тексте выше | читать |
|---|---|
| `TD-177` (`Н-13`, README против `deploy.yml`) | **`TD-191`** |
| `TD-178` (`Н-14`, гейт не зовёт `crontab -n`) | **`TD-192`** |
| `TD-179` (`Н-16`, `OPS-I-3` restore-drill) | **`TD-193`** |

Поправка дописана, а не внесена правкой по тексту, намеренно: вердикт — артефакт гейта, и
след ошибки в нём ценнее аккуратного вида. Барьер `check_artifact_ids.sh` этого класса не
ловит и по построению не может — он сторожит ДВА НОСИТЕЛЯ под одним идентификатором, а здесь
носитель один и он завёлся под верными номерами; ложная ссылка живёт в прозе вердикта.
Названо как предел, а не как оправдание.

Заведённые карточки: `TD-191` (MAJOR), `TD-192` (MINOR), `TD-193` (MAJOR).
Дополнительно этим же close-out'ом ЗАКРЫТЫ `TD-183` и `TD-184` — оба merge'ем M-73.

## ПОСТ-MERGE (§8) — исполнено, сырые строки

```
merge: PR #129 → main 2026-08-30T19:17:54Z, вершина 7581f60
$ gh pr checks 129 --watch >/dev/null 2>&1; echo CHECKS=$?     → CHECKS=0  (17/17 pass)
$ gh run watch 33330478744 --exit-status >/dev/null 2>&1; echo → CI_exit=0
$ gh run watch 33330478723 --exit-status >/dev/null 2>&1; echo → DEPLOY_exit=0
$ ssh … 'git -C /root/hft-platform rev-parse HEAD'
7581f603bddff2f18a4356f90e74e7f2f4eb70b0
$ ssh … 'ls -la /etc/cron.d/'
-rw-r--r-- 1 root root 3119 Aug 30 19:28 hft-builder-prune          ← НОВОЕ
-rw-r--r-- 1 root root 5009 Aug 30 19:28 hft-journal-offsite        ← НОВОЕ
-rw-r--r-- 1 root root 7063 Aug 30 19:28 hft-journal-retention
$ ssh … 'ls -l /root/hft-platform/deploy/bin/{journal-offsite,builder-prune}-cron.sh'
-rwxr-xr-x 1 root root 31832 Aug 30 19:28 …/journal-offsite-cron.sh
-rwxr-xr-x 1 root root  8167 Aug 30 19:28 …/builder-prune-cron.sh
$ ssh … 'sed -n "326p" …/journal-offsite-cron.sh'
_st=("${PIPESTATUS[@]}")                          ← фикс Б-5 физически на проде
$ ssh … 'docker ps --format "{{.Names}} {{.Status}}"; df -h /; ls /var/lib/hft/*.alert'
hft-gateway-serve Up (healthy) · hft-recorder Up (healthy) · / 65 % · (alert-файлов нет)
heartbeat: {"free_bytes":54431326208,"min_free_bytes":10737418240,"writable":true,…}
```

**Признание ошибки процедуры, не влияющей на исход.** Команду merge'а я выполнил как
`gh pr merge … | tail -5; echo merge_exit=$?` — это ловит exit `tail`, а не `gh`, то есть
буквально та форма, которую `gates.md` §3 запрещает («решение принимается по КОДУ ВОЗВРАТА, а
не по тексту»). `gh` при этом напечатал `could not determine current branch` (я работал в
detached-worktree), и «`merge_exit=0`» было ложным. Исход установлен фактом, а не отчётом —
`gh pr view 129 --json state,mergedAt` → `MERGED 19:17:54Z`, `git log origin/main` содержит
`7581f60`. Ветка `feat/R1-offsite-schedule` удалена отдельной командой. Записываю потому, что
вердикт, скрывающий свою процедурную ошибку, воспроизводит класс, который сам же ловит.

## EYES-ON ПЕРВОГО АВТО-ПРОГОНА — ПРОЙДЕН 2026-08-30T20:22Z

Условие моего APPROVED исполнено полностью. Прогон именно АВТОМАТИЧЕСКИЙ — вызывателем
выступил cron, а не рука; это предъявлено строкой syslog, а не выведено из свежести отметки:

```
$ ssh … 'grep journal-offsite /var/log/syslog | tail -1'
2026-08-30T20:22:01 ubuntu-8gb-fsn1-2 CRON[561596]: (root) CMD (/root/hft-platform/deploy/bin/journal-offsite-cron.sh)
$ ssh … 'cat /var/lib/hft/journal-offsite.last-success'
2026-08-30T20:22:28Z
$ ssh … 'tail -8 /var/log/hft/journal-offsite.log'
Number of files: 479 (reg: 479)      Number of created files: 1 (reg: 1)
Number of deleted files: 0           Total transferred file size: 1.07G bytes
sent 1.07G bytes  received 33 bytes  40.53M bytes/sec
2026-08-30T20:22:28Z OK duration=26s
$ ssh … 'ls -l /var/lib/hft/journal-offsite.alert'   → (alert-файла нет)
$ ssh … 'docker ps --format "{{.Names}} {{.Status}}"; df -h /'
hft-gateway-serve Up 54 minutes (healthy) · hft-recorder Up 54 minutes (healthy) · / 65 %
```

Сверка с ЗАЯВЛЕННОЙ формой (шапка `deploy/cron.d/journal-offsite`: «сегмент ≈ 1100 МБ
копируется за ≈ 27 с, при 1–2 закрытых за час»): один созданный файл, 1.07 ГБ, 26 с.
Совпадает — то есть прогноз милестоуна был замером, а не оценкой. `Number of deleted files: 0`
— положительное подтверждение отсутствия `--delete` на живом канале, а не только в argv.

**Что этим НЕ доказано, и я это называю.** Что скопированные байты ЧИТАЮТСЯ. Копия
существует и обновляется, `OPS-I-2` выполнен по букве; `OPS-I-3` (restore-drill) не
проводился ни разу — `TD-193`. Пока drill не пройден, `RETENTION_MODE` обязан оставаться
`dry-run`: порядок «копия → восстановление → удаление» необратим.

**Итог гейта:** APPROVED исполнен целиком — merge, §8, подключение к проду и первый
авто-прогон предъявлены. Милестоун НЕ закрыт: close-out (`§Tasks`, `docs/fa/ops.md:399`,
restore-drill) — зона architect'а.

=== END R-158 ===
