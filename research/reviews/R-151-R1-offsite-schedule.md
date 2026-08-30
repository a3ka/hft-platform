<!-- GATE-META
milestone: TD-020
audited_repo: a3ka/hft-platform
audited_base: d1221b1ca932d0b8e95403c2849308ed6e7b9ce2
audited_head: e990f0a596c1bf812be4662603c47d95512387bc
verdict: REJECT
-->

# R-151 — `R1` (offsite-расписание + сторож BuildKit-кэша): PR-time reviewer, **REJECTED**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-29T17:05Z
**Предмет:** `d1221b1..e990f0a` на `origin/feat/R1-offsite-schedule` — 7 коммитов engine-dev'а:
офсайт-копия журнала по расписанию, сторож кэша сборок, снятие чужого литерала из
`crates/gateway/src/lib.rs`, переписанный §1 `deploy/README.md`.

**Вердикт: REJECT (CHANGES REQUESTED).** Ни одна находка не угрожает уже существующим данным:
инвариант «rsync без `--delete`» **держится** (проверено honest-грепом и мутацией, см. Н-1),
ручная копия 29.08 остаётся валидной, локальный диск ничего не теряет. Блокирующее — три
УТВЕРЖДЕНИЯ, которые файл делает о себе и которые ложны на этой же ревизии, плюс один флаг
rsync, возвращающий ровно тот дефект, который шапка скрипта объявляет недопустимым.

Все четыре правки лежат в зоне engine-dev'а и стоят одного круга.

---

## Что я прочитал и чем греп ограничен (ярус C, `reading-map.md` §2)

- Ярус A: `CLAUDE.md` + `.claude/rules/*` (впрыск), профиль `reviewer`, `docs/04-workflow.md`
  §1/§2, `docs/workflow/reading-map.md`, `docs/PENDING-SIGNATURE.md` — **индекс** `grep '^## П-'`
  + тело `П-023` (на `origin/main`: на базе ветки его ещё нет).
- Ярус B: `deploy/README.md`, `deploy/cron.d/*`, `deploy/bin/*`, `.github/workflows/deploy.yml`,
  `scripts/deploy_catchup.py`, `scripts/verify_M-48.sh`, `crates/ops/src/bin/ops-watchdog.rs`,
  `docs/fa/viz-backend.md`.
- Ярус C — **грепом, предметы названы:** `TECH-DEBT.md` по `offsite|storage.?box|builder.?prune|
  BuildKit|CRON_JOBS|last-success` (нашёл `TD-020` — офсайт-бэкап + restore-drill, OPEN;
  `TD-006`; карточек про сторож кэша и про охват watchdog'а НЕТ);
  `docs/SESSION-HANDOFF.md` по `R1|offsite|storagebox`. **`TECH-DEBT.md` целиком не читал**
  (1 015 KB вместе с `PROJECT-STATE.md` — заведомо ложное «прочитал»).
- **FA тронутого модуля (M-66).** Диф трогает `crates/gateway/src/lib.rs` ⇒ барьер
  `check_review_fa.sh` требует ЖИВОЙ инвариант из `docs/fa/viz-backend.md` (префикс `VB`):
  **`VB-I-3`** (`docs/fa/viz-backend.md:190` — «Read Gateway read-only: grep-канарейка —
  gateway не импортирует journal-writer/recorder-write») и **`VB-I-1`** (там же:188 — чистый
  редьюсер, детерминизм-тест обязателен). Правка задачи #3 — комментарий над
  `enforce_response_limit`; ни одно из двух свойств ею не задето, что предъявляется дифом
  (`git diff … -- crates/gateway/src/lib.rs` = 9 строк, все внутри `///`-докблока).

---

## Block-scope — ПРОЙДЕН

```
$ git diff --stat d1221b1..e990f0a
 crates/gateway/src/lib.rs          |   9 +-
 deploy/README.md                   |  96 ++++++++----
 deploy/bin/builder-prune-cron.sh   | 116 +++++++++++++
 deploy/bin/journal-offsite-cron.sh | 245 +++++++++++++++++++++++++++
 deploy/cron.d/builder-prune        |  39 +++++
 deploy/cron.d/journal-offsite      |  54 ++++++
 6 files changed, 533 insertions(+), 26 deletions(-)
```

`deploy/**` — зона engine-dev'а (`scope-guard.md`, строка engine-dev: «+ `deploy/**`
(ops/деплой-механика: cron, Dockerfile, compose — НЕ секреты)»); `crates/gateway/src/**` —
там же. Не тронуты: `crates/{risk,killswitch,contracts,oms,venue-*}`, `*/tests/**`,
`scripts/verify_*.sh`, `milestones/*.md`, `PROJECT-STATE.md`, `TECH-DEBT.md`,
`docker-compose.yml`. Секретов в дифе нет (ключ — путь `/root/.ssh/storagebox`, не содержимое).

**Санкция founder'а есть и проверена в первоисточнике.** `П-023` (`origin/main`,
`docs/PENDING-SIGNATURE.md:1661`) прямо называет строку «копия повторяется по расписанию —
❌ НЕТ, зона engine-dev (`deploy/cron.d/`)». Граница C не нарушена: ретеншен остаётся в
`dry-run`, локальное удаление не начато, состав данных не меняется. Диф это соблюдает
(`RETENTION_MODE` не тронут — проверено: `deploy/cron.d/journal-retention` в дифе отсутствует).

**Block-C (contracts):** правок `crates/contracts/**` нет — N/A.
**RISK-BLOCK (`gates.md` §5):** `risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic
не требуется, N/A. Подтверждаю явно, а не по словам handoff'а: `git diff --name-only` выше.
**Атомарность:** 7 коммитов, каждый называет задачу; три `fix(R1): task #1` — правки багов,
пойманных ручным прогоном, а не бандл. Претензий нет.

---

## Блокирующие находки

### Б-1. Расписание `7 * * * *` СОВПАДАЕТ с ретеншеном 04:07 — три ложных утверждения подряд

`deploy/cron.d/journal-offsite:50-54`:

> `# Раз в час, :07 — … 04:07 — retention dry-run), :07 каждого часа не накладывается на оба`

`deploy/cron.d/journal-offsite:54` — `7 * * * * root …`; `deploy/cron.d/journal-retention:54` —
`7 4 * * * root …`. Ежесуточно в 04:07 обе записи срабатывают ОДНОВРЕМЕННО. Проверка:

```
$ grep -nE '^[0-9*]' deploy/cron.d/journal-retention
54:7 4 * * * root /root/hft-platform/deploy/bin/journal-retention-cron.sh
58:50 3 * * * root /root/hft-platform/deploy/bin/journal-compaction-cron.sh
79:*/15 * * * * root flock -n /var/lock/hft-gateway-checkpoint.lock …
$ grep -nE '^[0-9*]' deploy/cron.d/journal-offsite
54:7 * * * * root /root/hft-platform/deploy/bin/journal-offsite-cron.sh
```

Второе ложное утверждение — в `deploy/cron.d/builder-prune:37-38`: «после … офсайт-копии (:07
каждого часа, ближайший выше — 04:07)» — сам файл ЗНАЕТ про 04:07 и всё равно называет
пересечение отсутствующим в соседнем файле.

Третье, и оно хуже двух первых, — `deploy/bin/journal-offsite-cron.sh:97-100`:

> `# Lock — отдельный файл, чтобы cron-строки retention/compaction/offsite НЕ конкурировали`
> `# … rsync может читать сжатый сегмент одновременно с compaction — flock обеспечивает`
> `# сериализацию на уровне всего скрипта`

Отдельный lock-файл даёт РОВНО ОБРАТНОЕ: `flock -n /var/lock/hft-journal-offsite.lock`
сериализует offsite сам с собой и ни с чем больше. Ни `journal-retention-cron.sh`, ни
`journal-compaction-cron.sh` этот файл не берут (`grep -n 'flock\|LOCK'
deploy/bin/journal-retention-cron.sh` → пусто; в crontab flock есть только у
`gateway-checkpoint`). Утверждение о механизме, которого нет, — класс `TD-138`.

**Наблюдаемое следствие уже сегодня:** компакция (03:50) сверяет sha256 и УДАЛЯЕТ сырой
оригинал (`crates/journal/src/bin/journal-retention.rs:33-34,55-57`). rsync, читающий
`segment-N.jrnl` в момент его удаления, вернёт 24 → скрипт поднимет ALERT
(`journal-offsite-cron.sh:229-233`) на штатной операции. Это ложная тревога на сторожевом
пути — то, из-за чего сторожей перестают читать.

**Следствие ЗАВТРА, и оно уже назначено `П-023`:** следующий шаг — включение ретеншена
`apply`. Тогда в 04:07 удаляющий локальные сегменты процесс и копирующий их процесс идут
одновременно и без общей блокировки.

**Требуется:** развести расписание (минута, отличная от `:07`) ИЛИ ввести общий lock;
и в обоих случаях привести три комментария к тому, что делает код.

### Б-2. `--partial` кладёт ОБРЫВОК под настоящим именем в единственную офсайт-копию

`deploy/bin/journal-offsite-cron.sh:217` (и argv-контракт, строка 134) — `--partial` без
`--partial-dir`. Семантика rsync: при обрыве частично переданный файл ОСТАЁТСЯ на приёмнике
**под финальным именем**. То есть на Storage Box'е появляется `segment-00000464.jrnl`, который
выглядит целым и обрывается на середине, — до следующего часового тика.

Это ровно тот дефект, который шапка этого же файла объявляет недопустимым
(`journal-offsite-cron.sh:42-44`): «копия зафиксирует обрывок, выглядящий как целый файл — на
вид валидно, при попытке replay'а — тихая потеря хвоста». Активный сегмент от него защищён
mtime-фильтром; оборванная передача — нет.

Цена названа не мной, а решением founder'а: `П-023` — «снапшоты коробки ❌ выключены… одна наша
ошибка сносит и оригинал, и копию», и `R1b` restore-drill ещё не проводился. Восстановление
будет читать ИМЕННО эту копию.

**Требуется:** `--partial-dir=<скрытый каталог>` (частичные данные лежат вне видимого дерева и
доезжают на следующем тике) либо явный отказ от `--partial` с обоснованием. Проектирование
защиты — не моя зона (`gates.md` §4, граница reviewer↔architect); я называю дефект и его
воспроизведение: оборвать канал в середине передачи сегмента, посмотреть `ls -l` на приёмнике.

### Б-3. Runbook §1.4 предписывает оператору ФОРМУ, которая в этой же сессии доказанно ломается

`deploy/README.md:101`:

> `в ssh://u659392-sub1@u659392-sub1.your-storagebox.de:23/journal/`

Это форма, из-за которой был коммит `cf686af` («use user@host:path rsync form, not ssh:// URL
with -e»), и шапка скрипта её прямо запрещает (`journal-offsite-cron.sh:72-75`: «`ssh://`-URL в
паре с `-e` даёт „ssh ssh://…“, rsync 3.4.1, замер: `Could not resolve hostname ssh`»).
Runbook — то, что оператор копирует в терминал в четыре утра. Здесь он копирует известный баг.

**Требуется:** привести §1.4 к `user@host:path`, как в коде.

---

## Находки-примечания (не блокируют merge, но обязаны быть исправлены или записаны долгом)

### Н-1. Канарейка `--delete` из Done Block'а — ПЛАЦЕБО. Проверено мутацией

Handoff предъявляет как доказательство:

```
grep -E '^[^#]--delete' deploy/bin/*.sh deploy/cron.d/*   → 0 hits, exit=1
```

Регексп требует РОВНО ОДИН символ до `--delete` в начале строки. В реальном вызове rsync флаги
идут с отступом в шесть пробелов, поэтому канарейка не увидит `--delete`, даже когда он там
есть. Мутационный контроль (`testing.md` §«Мутационный контроль»), выполнен мной:

```
$ sed -i 's/--archive --partial --human/--archive --partial --delete --human/' /tmp/mut-offsite.sh
$ grep -n -- '--delete --human' /tmp/mut-offsite.sh
217:      --archive --partial --delete --human-readable --stats \
$ grep -E '^[^#]--delete' /tmp/mut-offsite.sh; echo "dev_canary_exit=$?"
dev_canary_exit=1          # НЕ НАШЁЛ, хотя --delete стоит в живых аргументах
$ grep -nE '^[[:space:]]*--delete|[[:space:]]--delete' /tmp/mut-offsite.sh; echo "honest_exit=$?"
217:      --archive --partial --delete --human-readable --stats \
honest_exit=0
```

**Сам инвариант ДЕРЖИТСЯ** — это я проверил отдельно и честным грепом: все пять вхождений
`--delete` на ревизии `e990f0a` — в комментариях (`journal-offsite-cron.sh:36,38,39,119`,
`cron.d/journal-offsite:19`), в аргументах rsync его нет. Дефектно ДОКАЗАТЕЛЬСТВО, а не
предмет. Шапка скрипта (строка 39) предлагает ту же плацебо-проверку следующему читателю —
её надо переписать вместе с канарейкой.

### Н-2. Предполётный ssh ходит на ЗАШИТЫЙ хост, а rsync — на `DST_URL`

`journal-offsite-cron.sh:178-180` — литерал `u659392-sub1@u659392-sub1.your-storagebox.de`,
тогда как цель настраиваема (`JOURNAL_OFFSITE_DST`, строка 76) и печатается в argv-контракте.
Переопределение цели (второй субаккаунт, стенд restore-drill'а) даёт зелёный pre-flight против
СТАРОГО хоста и падение rsync после него. Композиция «producer→consumer», которую §D handoff'а
предъявляет как проверенную, здесь разорвана. Порт 23 зашит в трёх местах тем же образом.

### Н-3. `pipefail` + SIGPIPE: комментарий утверждает обратное тому, что делает bash

`journal-offsite-cron.sh:209-213` — «мы используем `set -uo pipefail`, но pipefail не ловит
SIGPIPE от rsync». Ловит: при `pipefail` статус конвейера — код самой правой упавшей команды,
и падение `find` (141) видно, когда rsync завершился нулём. Практическое следствие — ложный
ALERT и ненулевой exit при успешной копии. Класс уже назван в корпусе на этой неделе
(`П-024`, круг 7: «SIGPIPE при `pipefail`, барьер мерил окружение вместо инварианта»).

### Н-4. Маркеры `*.alert` / `*.last-success` двух новых заданий не читает НИКТО

`journal-offsite-cron.sh:235-239` и `builder-prune-cron.sh:44-49` объявляют, что
`*.last-success` «детектирует „cron молча не запускался“». Наблюдатель в проекте один —
`crates/ops/src/bin/ops-watchdog.rs:49`:

```
const CRON_JOBS: &[&str] = &["compaction", "gateway-checkpoint", "retention"];
```

`journal-offsite` и `builder-prune` в списке отсутствуют; `deploy/README.md` §5 перечисляет те
же два задания. Смягчающее обстоятельство, которое я проверил замером, а не предположил:
watchdog на проде НЕ УСТАНОВЛЕН вовсе — в `/var/lib/hft/` нет ни `watchdog.last-success`, ни
`watchdog.state.json`, то есть сегодня не наблюдается НИ ОДНО задание, и это ждёт `П-003`
(токены телеграма). Поэтому — не блокер, а долг: карточку заведу в `TECH-DEBT.md` при
close-out (severity MAJOR, «built-not-wired» по `gates.md` §4 DoD), потому что ценность
бэкапа определяется тем, узнаём ли мы о его молчании.

### Н-5. Доставка cron-файлов на прод ЕДЕТ ЗАЙЦЕМ на правке комментария в `crates/`

`.github/workflows/deploy.yml:32-45` — `on.push.paths` содержит `crates/**`, `Cargo.*`,
`Dockerfile`, `docker-compose.yml`, `.github/workflows/deploy.yml`. **`deploy/**` там нет.**
Добор (`scripts/deploy_catchup.py:121-137`, `push_paths(wf)`) берёт тот же список из
`deploy.yml`, то есть решение `DEPLOY/SKIP` считается по той же дельте.

Практически: ЭТОТ merge доедет до прода — в нём есть `crates/gateway/src/lib.rs` (задача #3).
Но доедет он по касательной: убери задачу #3, и merge с двумя новыми cron-файлами оказался бы
инертен на проде — ровно класс `TD-048` / B3 M-48, на который ссылаются шапки этих же файлов.
Долг записываю; расширение `paths` — не правка ревьюера.

### Н-6. Предмет прошёл БЕЗ спеки, БЕЗ acceptance-гейта и БЕЗ оракула точки входа

`ls milestones/ | grep -i 'R1'` → пусто; `scripts/verify_R1.sh` не существует. У предмета нет
и ЗАКОННОГО идентификатора: «R1» — обозначение из `П-023`, а не `КЛАСС-НОМЕР` по `gates.md`
§12, и барьер `scripts/check_gate_meta.sh:349-354` его отвергает (проверено исполнением:
`FAIL … milestone «R1» не похож на идентификатор артефакта`). Поэтому шапка `GATE-META` этого
вердикта привязана к `TD-020` — карточке долга «offsite-бэкап + restore-drill», которую работа
и закрывает наполовину (вторая половина — `R1b` restore-drill, не проводился). Соседние
cron-обёртки такой оракул имеют — `scripts/verify_M-48.sh:36-64` исполняет
`HFT_CRON_PRINT_ARGV=1` для двух обёрток, сверяет ключи argv и композицию producer↔consumer.
Для двух новых обёрток такой проверки в репозитории нет: контракт `HFT_CRON_PRINT_ARGV`
поддержан (я его исполнил, вывод ниже), но ничто не удержит его от тихой пропажи.

Это НЕ вина engine-dev'а: `milestones/*.md` и `scripts/verify_*.sh` — sacred-зона architect'а
(`scope-guard.md`), dev не имеет права их писать, а tester без verify-скрипта сводится к
`cargo test`, что дев и прогнал сам. Констатирую разрыв маршрута и адресую его architect'у;
на merge не влияет.

### Н-7. Ротация логов заявлена и не существует

`deploy/cron.d/journal-retention:18` — «лог: … ротация — logrotate, см. `deploy/README.md`»;
конфигурации logrotate в репозитории нет (`grep -rn logrotate deploy/ .github/` → две строки,
обе — упоминания в комментариях). Новый почасовой лог с `--stats` добавляет ещё один растущий
файл на диск, дефицит которого и есть причина этой работы. Мелочь, но записываю.

---

## Block-DoneBlock — прогон РЕВЬЮЕРА (не пересказ handoff'а)

Всё ниже снято мной на дереве слияния `e990f0a + origin/main` (`gates.md` §8: `strict:false`,
ветка отстаёт от `main` на 16 коммитов, поэтому проверка — на merge-preview, а не на ветке).

```
$ git merge-base origin/main origin/feat/R1-offsite-schedule
d1221b1ca932d0b8e95403c2849308ed6e7b9ce2
$ git rev-list --count d1221b1..origin/main
16
$ git merge --no-commit --no-ff origin/main
Automatic merge went well; stopped before committing as requested      exit=0

$ bash -n deploy/bin/journal-offsite-cron.sh ; echo exit=$?      → exit=0
$ bash -n deploy/bin/builder-prune-cron.sh   ; echo exit=$?      → exit=0
$ HFT_CRON_PRINT_ARGV=1 bash deploy/bin/journal-offsite-cron.sh  → VARS/FIND/RSYNC, exit=0
$ HFT_CRON_PRINT_ARGV=1 bash deploy/bin/builder-prune-cron.sh    → VARS/DOCKER,      exit=0
$ git grep -c 'scripts/verify_M-71' -- crates/ deploy/           → 0 hits, exit=1   (задача #3 ✅)
$ git ls-files -s deploy/bin/*.sh
100755 … builder-prune-cron.sh
100755 … journal-offsite-cron.sh                                  (executable ✅)

$ cargo fmt --all -- --check                        → FMT_EXIT=0
$ cargo clippy --workspace --all-targets -- -D warnings → CLIPPY_EXIT=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -2
PASS  [7-RFC-PATH] … всего=274 проверено=182 пропущено=92 — все существуют
VERDICT: PASS (0 нарушений)                          exit=0
```

```
$ cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=919 failed=0 (блоков: 216)
```
Совпадает с числом из §C handoff'а — прогон дева воспроизведён на дереве слияния, а не принят
на слово.

### Состояние мира (ярус S) — снято, а не предположено

```
$ gh run list --branch main --limit 3
completed success  Deploy to VPS  main  33262855290  17s   2026-08-29T16:23:57Z
completed success  CI (PR #119)   main  33262362689  10m36s 2026-08-29T16:13:19Z
$ ssh root@167.233.192.131 'docker ps --format "{{.Names}} {{.Status}}"; df -h /; ls /etc/cron.d; ls /var/lib/hft'
hft-gateway-serve Up 21 hours (healthy)
hft-recorder      Up 21 hours (healthy)
/dev/sda1  150G  90G  55G  63% /
e2scrub_all  hft-journal-retention
compaction.last-success  gateway-checkpoint.last-success
journal-offsite.last-success (2026-08-29 16:02)  retention.last-success
$ ssh … 'cd /root/hft-platform && git rev-parse --short HEAD'   → eb1c20a  (main = 37a6358, +16 докс-коммитов)
```

`journal-offsite.last-success` на проде — след ручного прогона дева, cron ещё не установлен
(в `/etc/cron.d/` только `hft-journal-retention`). Это согласуется с §C handoff'а.

---

## Что требуется для APPROVED (круг 2)

1. **Б-1** — развести `journal-offsite` и `journal-retention` во времени (или общий lock) И
   привести три комментария (`cron.d/journal-offsite:50-53`, `cron.d/builder-prune:37-38`,
   `bin/journal-offsite-cron.sh:97-100`) к тому, что делает код.
2. **Б-2** — `--partial-dir` (или отказ от `--partial`), чтобы обрывок не лежал под финальным
   именем в единственной копии.
3. **Б-3** — `deploy/README.md:101`: форма `user@host:path`, а не `ssh://…:23/…`.
4. **Н-1** — переписать канарейку `--delete` на ту, что краснеет против мутации (и поправить
   её текст в шапке скрипта, строка 39). Предъявить в Done Block ОБА прогона: против чистого
   файла и против мутированного.
5. **Н-2/Н-3** — предполётный ssh выводить из `DST_URL`; снять неверное утверждение про
   SIGPIPE и решить, что делать с ложным ALERT'ом.

Н-4/Н-5/Н-6/Н-7 — долг и маршрут, в круг 2 не входят; карточки заведу в `TECH-DEBT.md`
(зона reviewer'а) при close-out предмета.

**Чего я НЕ прошу:** переделывать частоту, полосу, `nice/ionice`, `flock -n`-семантику,
обоснование `until=336h` и структуру env — они обоснованы числами и замерами, и это сильная
часть работы. Сторож BuildKit-кэша (задача #2) содержательных претензий не имеет вовсе:
запреты названы явно, `docker image/volume/system prune` не используются (проверено грепом),
setup-валидация fail-loud.

=== END R-151 ===
