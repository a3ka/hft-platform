<!-- GATE-META
milestone: M-73
audited_repo: a3ka/hft-platform
audited_base: 99e83c333e108958858adf3ac195feee845c7f92
audited_head: 0dacd039bfdccc43d9bcffc7fcdb0f352fbf74d1
verdict: REJECT
-->

# R-155 — M-73 (офсайт-копия по расписанию), круг 2: PR-time reviewer, **REJECTED**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-30T11:40Z
**Предмет:** `99e83c3..0dacd03` на `origin/feat/R1-offsite-schedule` — 7 коммитов:
`ff1b546` (architect: спека `M-73` + `scripts/verify_M-73.sh`) и шесть engine-dev'а,
закрывающих пять пунктов `R-151`.

**Номер вердикта выдан механизмом** (`gates.md` §12): `bash scripts/reserve_artifact_id.sh R`
→ `R-155`. `R-152` НЕ использован — занят `research/reviews/R-152-harness-shape-backlog-recheck-r2.md`
в `main`; мандат называл его ошибочно.

---

## ВЕРДИКТ КОРОТКО

**Инженерная работа круга 2 выполнена и проверена мной поштучно — все пять пунктов `R-151`
закрыты, и закрыты честно.** Я подтвердил это не чтением, а СВОИМ мутационным контролем: на
каждую из пяти находок я сломал ровно её и убедился, что гейт краснеет (таблица ниже). Плацебо
круга 1 предъявлено мёртвым на том же файле. Это сильный круг, и его не следует читать как
«работа плохая».

**REJECT стоит по двум пунктам, и оба узкие:**

1. **`Б-1` закрыт НЕ ПОЛНОСТЬЮ.** Расписание разведено верно, но переписанный комментарий —
   тот самый, который находка требовала «привести к тому, что делает код», — содержит НОВОЕ
   ложное утверждение о механизме: «retention/compaction берут СВОИ lock-файлы». Их у них
   ноль. Правка — две строки.
2. **У предмета нет НИ ОДНОГО поведенческого доказательства**, а изменилось ровно то, что
   поведение и определяет. Требую ручной прогон на проде до APPROVED.

Ни одна находка не угрожает уже существующим данным: инвариант «rsync без `--delete`» держится
и теперь ЗАЩИЩЁН работающей канарейкой; ручная копия 29.08 (77 ГБ) валидна; локальный диск
ничего не теряет.

---

## Что я прочитал и чем греп ограничен (ярус C, `reading-map.md` §2)

- **Ярус A:** `CLAUDE.md`, `.claude/rules/*` (все шесть), профиль `.claude/agents/reviewer.md`,
  `docs/04-workflow.md` §1/§2/§3 — с диска.
- **Ярус B:** `milestones/M-73-offsite-schedule.md`, `scripts/verify_M-73.sh`,
  `research/reviews/R-151-R1-offsite-schedule.md` (целиком, 351 строка),
  `deploy/{README.md,bin/*,cron.d/*}`, `.github/workflows/deploy.yml`,
  `scripts/deploy_catchup.py`.
- **Ярус C — ТОЛЬКО грепом, предметы называю:** `TECH-DEBT.md` по
  `TD-020|TD-048|offsite|[Ss]torage.?[Bb]ox|builder.?prune|CRON_JOBS` (живая карточка —
  **`TD-020`** `retention-implemented-but-never-invoked`, OPEN, acceptance-ворота Ф0,
  «offsite-бэкап + restore-drill не проводился НИ РАЗУ»; карточек про сторож кэша и про охват
  watchdog'а по-прежнему НЕТ) и по `continue-on-error|All checks passed` (карточки под находку
  `C-184` `Б-2` НЕТ — завожу при close-out). **`TECH-DEBT.md` и `PROJECT-STATE.md` целиком не
  читал** — вместе 1 015 KB ≈ 226 k токенов, «прочитал» было бы заведомо ложным утверждением.

**FA (M-66).** Диф НЕ трогает `crates/**`, барьер это подтверждает исполнением, а не моим
словом:

```
$ bash scripts/check_review_fa.sh 99e83c33 0dacd039
SKIP (диапазон не трогает crates/**)
FA_EXIT=0
```

`FA-WAIVER` дева в `6173534` присутствует и причину несёт. Придирка (не находка): он написан
как `FA-WAIVER: crates/<name> — …`, то есть с ПЛЕЙСХОЛДЕРОМ вместо конкретного крейта —
профиль требует называть конкретный. Здесь это безвредно, потому что waiver'у нечего waive'ить
(барьер и так `SKIP`), но форма — «токен на предъявителя», которую профиль запрещает.

---

## Block-scope — ПРОЙДЕН

```
$ git diff --stat 99e83c33..0dacd039
 deploy/README.md                    |   5 +-
 deploy/bin/journal-offsite-cron.sh  | 166 ++++++++++++++++++++++++++++-----
 deploy/cron.d/builder-prune         |   6 +-
 deploy/cron.d/journal-offsite       |  12 +--
 milestones/M-73-offsite-schedule.md |  99 +++++++++++++++++++++
 scripts/verify_M-73.sh              | 158 ++++++++++++++++++++++++++++++
 6 files changed, 411 insertions(+), 35 deletions(-)
```

`Allowed paths` M-73 соблюдены: `deploy/{bin,cron.d,README.md}` — engine-dev;
`milestones/M-73-*.md` + `scripts/verify_M-73.sh` — architect, и они пришли ОТДЕЛЬНЫМ
коммитом `ff1b546` под меткой `[architect]`, а не размазаны по dev-коммитам.

**Проверено, что dev не залез в sacred** — покоммитно, а не по итоговому диффу:

```
$ for c in dfcd9dc 5dd41fd dc23359 c33e0cf 6173534 0dacd03; do
    git show --name-only --format='' $c | grep -E '^(milestones/|scripts/|crates/)'; done
{пусто}                                   exit=1 — ни одного касания
$ git diff --name-only 99e83c33..0dacd039 | grep -E '^(crates/|docker-compose.yml|PROJECT-STATE.md|TECH-DEBT.md|\.github/)'
{пусто}                                   exit=1 — Forbidden paths не тронуты
```

**Block-C (contracts):** `crates/contracts/**` не тронут — N/A, проверка тривиальна и
предъявлена строкой выше.
**RISK-BLOCK (`gates.md` §5):** `risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic
не требуется. Подтверждаю дифом, а не словами handoff'а.
**Атомарность:** 6 dev-коммитов на 5 задач; два из них (`c33e0cf`, `6173534`) — task #4 и его
немедленное исправление, что дисциплине не противоречит (одна задача = ≥1 коммит). Каждый
subject несёт `M-73` + номер задачи + метку роли. Бандлов нет.

---

## Block-DoneBlock — обязательная проверка мандата ПРОЙДЕНА

Мандат: оба прогона мутационного контроля обязаны быть в теле `6173534`, иначе REJECT.

```
$ git log -1 --format='%B' 6173534 | grep -nE 'Half 1 \(clean\)|Half 2 \(mutated\)'
13:  Half 1 (clean):  PASS  — в RSYNC-блоке нет --delete
15:  Half 2 (mutated): PASS  — канарейка REDDENS против мутации
grep_exit=0
```

Присутствуют оба, плюс третья строка `Setup-guard: PASS`. Тело коммита содержательно
описывает, ПОЧЕМУ понадобился `6173534`: мутация в `verify_M-73.sh` целила в первое вхождение
`--archive`, а им оказывался комментарий в шапке скрипта — то есть setup-guard проходил, а
анти-плацебо ломалось. Дев поймал это САМ и починил, назвав класс. Это ровно то поведение,
которого требует `testing.md`.

---

## Проверка пяти пунктов `R-151` — МОИМ мутационным контролем, а не чтением

Гейт `verify_M-73.sh` я исполнил в прод-форме, затем для каждой находки сломал ровно её в
изолированной копии дерева и посмотрел, краснеет ли гейт. Из копии сняты ТОЛЬКО три
`chk "cargo …"` (паритет с CI, ~15 мин); все проверки предмета — дословно оригинальные.

Базовая линия на чистом дереве: **13 PASS, 0 FAIL, `VERDICT: PASS`**.

| # | находка | мутация | гейт | итог |
|---|---|---|---|---|
| `Б-1` | расписание | `22 * * * *` → `7 * * * *` (возврат коллизии с retention 04:07) | `FAIL`, провалено 1 | ✅ краснеет |
| `Б-2` | `--partial-dir` | `--partial-dir=.rsync-partial` → `--partial` | `FAIL`, провалено 1 | ✅ краснеет |
| `Б-3` | runbook | возврат `ssh://…@…:23/journal/` в `README.md:101` | `FAIL`, провалено 1 | ✅ краснеет |
| `Н-1` | канарейка `--delete` | `--delete` внедрён в ЖИВОЙ rsync-вызов (отступ 6 пробелов) | `FAIL`, провалено 1 | ✅ краснеет |
| `Н-2` | предполётный ssh | зашитый литерал хоста вместо вывода из `DST_URL` | `FAIL`, провалено 1 | ✅ краснеет |

**Ключевое доказательство по `Н-1`** — плацебо круга 1 предъявлено мёртвым НА ТОМ ЖЕ файле,
где новая канарейка срабатывает:

```
$ HFT_CRON_PRINT_ARGV=1 deploy/bin/journal-offsite-cron.sh | grep -A2 '^RSYNC:'
RSYNC:
nice -n ${NICE_LEVEL} ionice -c ${IONICE_CLASS} -n ${IONICE_LEVEL} rsync \
  --archive --partial-dir=.rsync-partial --delete --human-readable --stats \

$ grep -E '^[^#]--delete' deploy/bin/journal-offsite-cron.sh
placebo_exit=1        # канарейка КРУГА 1 НЕ ВИДИТ --delete в живых аргументах

$ bash scripts/gate-nocargo.sh | grep -E '^(FAIL|VERDICT)'
FAIL: ! rsync_block \
VERDICT: FAIL         # канарейка КРУГА 2 видит
```

`Н-3` (SIGPIPE) проверен чтением, а не мутацией — это снятие ложного утверждения, а не
механизм: `pipefail` снят целиком (`set -u`, строка 77), и комментарий теперь описывает
ФАКТИЧЕСКУЮ причину («ловил 141 `find`'а на штатном SIGPIPE от rsync'а и алертил ложно»).
Утверждение стало истинным. Принято.

**Отдельно отмечаю качество гейта.** `verify_M-73.sh` содержит развязку, которую я обязан
назвать, потому что она нетривиальна и сделана правильно: setup-guard сравнивает ВЫВОД
`rsync_block` до и после мутации, а не текст файла. Именно это отличает наблюдение канала от
наблюдения строк и закрывает класс, на котором подорвалась первая редакция. Шапка гейта честно
называет и собственный ретроспективный характер, и предел («судит argv и тексты, но не
поведение на реальном канале»).

---

## Механизм несущего пути (`gates.md` §4 DoD) — ПРОЙДЕН, и я едва не записал сюда ложный блокер

Проверка `task #0` гейта сверяет argv обёртки с `cron.d` — это КОМПОЗИЦИЯ писателя и читателя,
и как оракул точки входа она засчитывается.

Но оракул точки входа отвечает на вопрос «согласованы ли producer и consumer», а не «доедет ли
это до прода». Второй вопрос я проверил замером, и первый замер дал тревожный ответ: **ни один
файл из диапазона круга 2 не проходит фильтр путей `deploy.yml`** — `deploy/**` в
`on.push.paths` отсутствует, поэтому по дельте КРУГА 2 деплой не пошёл бы:

```
$ python3 … dc.path_matches(f, dc.push_paths(yaml.safe_load(open('.github/workflows/deploy.yml'))))
   SKIP  deploy/README.md
   SKIP  deploy/bin/journal-offsite-cron.sh
   SKIP  deploy/cron.d/builder-prune
   SKIP  deploy/cron.d/journal-offsite
   SKIP  milestones/M-73-offsite-schedule.md
   SKIP  scripts/verify_M-73.sh
ИТОГ: кодовых файлов в дельте — НЕТ -> SKIP
```

**Однако мержится не диапазон круга 2, а ВЕТКА.** Полная дельта против `main` несёт коммит
круга 1 с `crates/gateway/src/lib.rs`, и он фильтр проходит:

```
$ git diff --name-only $(git merge-base origin/main origin/feat/R1-offsite-schedule)..0dacd03
crates/gateway/src/lib.rs      ← КОД, триггерит деплой
deploy/…  milestones/…  research/reviews/R-151-…  scripts/verify_M-73.sh
ИТОГ: DEPLOY пойдёт
```

Дальше цепочка замкнута и проверена в первоисточнике: `deploy.yml:276-292` ставит КАЖДЫЙ файл
из `deploy/cron.d/*` в `/etc/cron.d/hft-<name>` с предварительной валидацией `crontab -n` и
fail-closed при ошибке. Валидацию я прогнал локально:

```
$ for f in deploy/cron.d/*; do crontab -n "$f"; done
The syntax of the crontab file was successfully checked.   (builder-prune)       exit=0
The syntax of the crontab file was successfully checked.   (journal-offsite)     exit=0
The syntax of the crontab file was successfully checked.   (journal-retention)   exit=0
```

⇒ после merge'а деплой установит `hft-journal-offsite` и `hft-builder-prune`, и расписание
оживёт. **`Н-5` не переоткрываю** — он остаётся долгом ровно в том виде, в каком записан в
`R-151`: подключение едет ЗАЙЦЕМ на комментарии в `crates/`, и это хрупко (убери задачу #3 —
merge инертен). Карточку заведу при close-out.

---

## БЛОКИРУЮЩЕЕ

### Б-1-остаток. Переписанный комментарий утверждает механизм, которого нет — снова

`deploy/bin/journal-offsite-cron.sh:117-125`:

> `# … Это НЕ cross-task serialisation: flock сериализует только процессы на ОДНОМ файле, и`
> `# retention/compaction берут СВОИ lock-файлы (см. journal-retention-cron.sh и`
> `# journal-compaction-cron.sh). …`

Главное исправление ЗАСЧИТАНО: прежняя ложь («отдельный lock сериализует retention/compaction/
offsite») снята явно и правильно, и развязка названа верно — разное расписание. Но
придаточное предложение — новое ложное утверждение того же класса, и оно ССЫЛАЕТСЯ НА ДВА
ФАЙЛА КАК НА ПРУФ:

```
$ for f in deploy/bin/journal-retention-cron.sh deploy/bin/journal-compaction-cron.sh; do
    printf '%-45s flock/lock вхождений: ' "$f"; grep -ciE 'flock|lock' "$f"; done
deploy/bin/journal-retention-cron.sh          flock/lock вхождений: 0
deploy/bin/journal-compaction-cron.sh         flock/lock вхождений: 0
```

Ноль. Ни один из них не берёт НИКАКОГО lock-файла — ни своего, ни общего. Читатель, пришедший
по ссылке, не найдёт там ничего.

**Почему это не придирка.** `R-151` `Б-1` — находка ровно о том, что комментарий описывает
несуществующий механизм (класс `TD-138`), и требование было «привести три комментария к тому,
что делает код». Два из трёх приведены; третий переписан и в переписанном виде снова
неправдив. Практическое следствие адресное: сегодня cross-task защита держится ИСКЛЮЧИТЕЛЬНО
на разнице минут, и ни на чём больше. Читатель, поверивший в «у каждого свой lock», сделает
неверный вывод именно в тот момент, когда это дороже всего — при включении ретеншена в режим
`apply` (следующий шаг `П-023`), где 04:07 удаляет, а :22 копирует, и общей блокировки нет ни
у одной стороны.

Здесь же — сопутствующее ослабление формулировки в `deploy/cron.d/journal-offsite:53-54`: «Это
ГАРАНТИЯ непересечения по времени». Разнос минут гарантирует несовпадение МОМЕНТОВ ЗАПУСКА, но
не непересечение ИСПОЛНЕНИЯ: компакция в 03:50, длящаяся дольше 32 минут, догонит копию в
04:22. Соседний файл (`cron.d/builder-prune:37-40`) в этом месте честнее — он называет запас в
минутах и обосновывает его замером (~27 с). Просьба привести формулировку к той же честности.

**Требуется:** снять придаточное про «СВОИ lock-файлы» (или заменить его на правду — «ни
retention, ни compaction lock не берут ВОВСЕ; cross-task развязка держится только на
расписании») и смягчить «ГАРАНТИЯ» до утверждения о моментах запуска. Правка текстовая, кода
не касается.

### Б-4. Поведенческого доказательства нет НИ ОДНОГО, а изменилось именно поведение

Это находка тестера, и я её подтверждаю замером, а не пересказом.

Гейт по построению статичен — его собственная шапка это объявляет: «судит ARGV и тексты… но не
поведение на реальном канале. Поведенческая часть… предъявляется ручным прогоном на проде в
Done Block'е». Спека `M-73` называет это остаточным риском. **Но объявить риск — не значит его
закрыть, а Done Block'а с прогоном нет ни в одном из шести коммитов:**

```
$ for c in dfcd9dc 5dd41fd dc23359 c33e0cf 6173534 0dacd03; do git log -1 --format='%B' $c; done \
    | grep -icE 'RSYNC_EXIT|ручной прогон на проде|идемпотент'
0
```

Состояние прода на 2026-08-30T11:21Z — снято мной по ssh:

```
$ ssh … 'ls -la /etc/cron.d/; ls -la /var/lib/hft/; date -u; cd /root/hft-platform && git rev-parse --short HEAD'
/etc/cron.d/:  .placeholder  e2scrub_all  hft-journal-retention     ← offsite и builder-prune ОТСУТСТВУЮТ
/var/lib/hft/: journal-offsite.last-success   Aug 29 16:02          ← след РУЧНОГО прогона круга 1
               compaction.last-success        Aug 30 03:51
               retention.last-success         Aug 30 04:07
               gateway-checkpoint.last-success Aug 30 11:15
date:  Sun Aug 30 11:21:47 UTC 2026                                 ← маркеру 43 часа
HEAD:  eb1c20a
hft-recorder Up 39 hours (healthy) · hft-gateway-serve Up 39 hours (healthy) · / 62% (89G/150G)
```

То есть офсайт-копия сегодня — **одноразовый снимок 12:54/16:02 от 29.08**, и именно этот
милестоун обязан превратить её в расписание.

**Что именно не проверено, и почему это не формальность.** Круг 2 изменил ровно те места,
которые определяют поведение на живом канале, и ни одно из них ни разу не исполнялось против
Storage Box:

1. **`--partial-dir=.rsync-partial`** — новая семантика на ПРИЁМНИКЕ: rsync создаёт служебный
   каталог в целевом дереве. На Storage Box'е (SFTP-субаккаунт, restricted shell) это первое
   исполнение. Проверять его впервые молчаливым cron'ом — ровно то, чего мы избегаем.
2. **`parse_dst_target()`** — 30 строк нового разбора, fail-loud по `return 1/2`. Я протрассировал
   его на прод-значении вручную (`u659392-sub1@…:journal/` → user/host выделяются верно), но
   трассировка не заменяет исполнения: ошибка здесь означает `exit 1` КАЖДЫЙ ЧАС.
3. **`set -u` без `pipefail`** — изменение поведения ВСЕГО скрипта, а не одного конвейера.

**Цена молчаливого отказа — не гипотетическая.** Сторож `ops-watchdog` не знает про
`journal-offsite` (`CRON_JOBS` = `compaction|gateway-checkpoint|retention`, `R-151` `Н-4`) и на
проде не установлен вовсе. Значит если после merge'а часовой job начнёт падать, об этом не
узнает НИКТО и НИКОГДА — а это дословно состояние, запрещённое собственным «Инвариантом
достаточности» M-73: «(а) копия устарела, а никто не знает». Мы получили бы худший исход:
милестоун закрыт, все считают журнал защищённым, копия стоит на 29.08.

Основание требования — наш собственный урок `R1`, и founder называет его тем же: **бэкап,
который не восстанавливали, бэкапом не считается.** `TD-020` (acceptance-ворота Ф0) до сих пор
OPEN именно по этой причине.

**Требуется** (прогон на проде НОВОЙ версии скрипта, как дев уже делал в круге 1 — процедура
известна и воспроизводима):

1. Скопировать `deploy/bin/journal-offsite-cron.sh` ревизии `0dacd03` на прод и исполнить.
   В Done Block — СЫРОЙ вывод: предполётный ssh, `RSYNC:`-блок, статистика rsync, `exit=$?`.
2. **Идемпотентность:** второй прогон подряд — копирует ≈ноль байт (всё уже на приёмнике),
   `exit=0`.
3. **Ничего не удалено на ИСТОЧНИКЕ:** число файлов и `du -sh` в
   `/var/lib/docker/volumes/hft-platform_journal-data/_data` до и после совпадают.
4. **`--partial-dir` не оставил мусора** в видимом дереве приёмника: `ls` целевого каталога
   после успешного прогона не содержит `.rsync-partial` (либо содержит пустым — назвать факт).

Прогонять обрыв канала я НЕ требую — это дорого и небезопасно на единственной копии.

---

## Примечания (не блокируют)

**Н-8. Runbook и код расходятся в форме пути — на один символ.** `README.md:101` предписывает
`…your-storagebox.de:/journal/` (ведущий слэш), код по умолчанию берёт
`…your-storagebox.de:journal/` (`journal-offsite-cron.sh:90`). Для субаккаунта, чей домашний
каталог и есть корень (README §1.2 это утверждает, и вывод `pwd` = `~` в §1 согласуется), обе
формы разрешаются одинаково, поэтому это не блокер. Но `Б-3` была ровно про расхождение
runbook'а с кодом, и оставлять там второе, пусть безобидное, расхождение не стоит: приведите
README к форме, которую печатает `HFT_CRON_PRINT_ARGV=1`.

**Н-9. Гейт `task #1` не покрывает `builder-prune`.** Цикл проверки коллизий обходит только
`deploy/cron.d/journal-retention` и `deploy/cron.d/journal-compaction` (второго файла не
существует, он пропускается по `[ -f ]`). Сегодня коллизии нет (:22 против 04:30), но перевод
копии на `30 * * * *` гейт бы не поймал. Зона architect'а (`scripts/verify_*.sh` sacred),
поэтому — примечание, а не требование к деву.

**Н-4/Н-5/Н-6/Н-7 не переоткрываю** — они и не входили в круг 2. Карточки долга завожу при
close-out (зона reviewer'а).

---

## Done Block — прогон РЕВЬЮЕРА

Всё снято мной в собственном дереве `/tmp/hft-rev-m73` (`git worktree add --detach
origin/feat/R1-offsite-schedule`), HEAD `0dacd039`.

```
$ git log -1 --format='%H %s'
0dacd039bfdccc43d9bcffc7fcdb0f352fbf74d1 fix(M-73): task #5 Н-2/Н-3 — предполётный ssh из DST_URL … [engine-dev]

$ bash scripts/check_review_fa.sh 99e83c33 0dacd039
SKIP (диапазон не трогает crates/**)                                        FA_EXIT=0

$ HFT_CRON_PRINT_ARGV=1 deploy/bin/journal-offsite-cron.sh
VARS: SRC_DIR=/var/lib/docker/volumes/hft-platform_journal-data/_data
      DST_URL=u659392-sub1@u659392-sub1.your-storagebox.de:journal/
      SSH_KEY=/root/.ssh/storagebox  SSH_PORT=23  MIN_AGE_MIN=15
      BWLIMIT_MBPS=40  NICE_LEVEL=10  IONICE_CLASS=2  IONICE_LEVEL=7
FIND: find "${SRC_DIR}" -type f -mmin +${MIN_AGE_MIN} ! -name 'recorder.heartbeat' -printf '%P\0'
RSYNC: … rsync --archive --partial-dir=.rsync-partial --human-readable --stats \
       --bwlimit=${BWLIMIT_MBPS}M -e "ssh -i ${SSH_KEY} … -p ${SSH_PORT} …" \
       --from0 --files-from=- "${SRC_DIR}/" "${DST_URL}"          ARGV_EXIT=0

$ for f in deploy/cron.d/*; do crontab -n "$f"; done               все три exit=0

$ bash scripts/verify_M-73.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
PASS: test -x deploy/bin/journal-offsite-cron.sh
PASS: test -x deploy/bin/builder-prune-cron.sh
PASS: HFT_CRON_PRINT_ARGV=1 deploy/bin/journal-offsite-cron.sh >/dev/null 2>&1
PASS: HFT_CRON_PRINT_ARGV=1 deploy/bin/builder-prune-cron.sh >/dev/null 2>&1
PASS: grep -rq 'journal-offsite-cron.sh' deploy/cron.d/
PASS: grep -rq 'builder-prune-cron.sh' deploy/cron.d/
PASS: <многострочная проверка>          # task #0 КОМПОЗИЦИЯ argv ↔ cron.d
PASS: <многострочная проверка>          # task #1 расписание
PASS: <многострочная проверка>          # task #2 --partial-dir
PASS: ! grep -nE "ssh://[A-Za-z0-9._-]+@" deploy/README.md
PASS: ! rsync_block \                   # task #4 половина 1 (чисто)
PASS: <многострочная проверка>          # task #4 половина 2 (мутант) + setup-guard
PASS: <многострочная проверка>          # task #5 хост из DST_URL
VERDICT: PASS
VERIFY_EXIT=0
```

Мутационный контроль (пять мутаций, каждая — отдельным прогоном на изолированной копии):
результаты в таблице выше; базовая линия чистого дерева **13 PASS / 0 FAIL**, каждая мутация
даёт ровно **1 FAIL** и `VERDICT: FAIL`. Мой прогон полного гейта воспроизводит результат
тестера (16/16, `exit=0`) — я его не принял на слово, а снял сам на своём дереве.

### ГЕЙТ ЗЕЛЁНЫЙ, А ВЕРДИКТ REJECT — это не противоречие, и я обязан назвать почему

`VERIFY_EXIT=0` означает ровно то, что гейт умеет проверять, и ни капли больше. Его
собственная шапка это объявляет: он «судит ARGV и тексты… но не поведение на реальном канале».
Оба моих блокера лежат ИМЕННО в слепой зоне гейта, и ни один из них не является придиркой к
тому, что гейт уже покрыл:

- **`Б-1`-остаток** — ложность УТВЕРЖДЕНИЯ в комментарии. Гейт сверяет расписание (числа) и не
  читает прозу о том, кто какой lock берёт. Машина здесь помочь не может по построению.
- **`Б-4`** — поведение на живом канале. Гейт исполняет обёртку в режиме `HFT_CRON_PRINT_ARGV=1`,
  то есть ДО side-эффектов: он видит намерение команды, но не её результат.

Зелёный статичный гейт — необходимое условие merge'а, а не достаточное. Принимать его за
доказательство работоспособности бэкапа значило бы совершить ровно ту ошибку, которую этот
проект уже назвал: «объявлено в репо ≠ работает в проде» (`TD-048`).

### Состояние мира (ярус S)

```
$ ssh … 'docker ps --format "{{.Names}} {{.Status}}"; df -h /; ls /etc/cron.d; ls /var/lib/hft'
hft-gateway-serve Up 39 hours (healthy) · hft-recorder Up 39 hours (healthy)
/dev/sda1 150G 89G 55G 62% /
/etc/cron.d: .placeholder  e2scrub_all  hft-journal-retention
/var/lib/hft: compaction.last-success gateway-checkpoint.last-success
              journal-offsite.last-success(29.08 16:02) retention.last-success
heartbeat: {"events":9679646,"next_seq":407562127,"segment_index":475,"writable":true,
            "free_bytes":58937937920}    — свежий, журнал растёт
```

---

## Что требуется для APPROVED (круг 3)

1. **`Б-1`-остаток** — снять/исправить придаточное «retention/compaction берут СВОИ
   lock-файлы» в `deploy/bin/journal-offsite-cron.sh:117-125`; смягчить «ГАРАНТИЯ
   непересечения» в `deploy/cron.d/journal-offsite:53-54` до утверждения о моментах запуска.
2. **`Б-4`** — ручной прогон НОВОГО скрипта на проде, сырой вывод в Done Block: (а) успешная
   копия с `exit=0`, (б) идемпотентность повторного прогона, (в) источник не потерял файлов,
   (г) `.rsync-partial` не оставил мусора на приёмнике.
3. `Н-8` — README к форме `…:journal/` (косметика, делается тем же коммитом).

**Чего я НЕ прошу:** ничего из уже сделанного не переделывать. Расписание, `--partial-dir`,
разбор `DST_URL`, снятие `pipefail`, переписанная канарейка и сам гейт — приняты и проверены
мутацией. Круг 3 — это две строки комментария плюс прогон, который дев уже умеет делать.

=== END R-155 ===
