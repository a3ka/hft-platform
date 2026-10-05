<!-- GATE-META
milestone: M-93
audited_repo: a3ka/hft-platform
audited_base: 81132e10c0d51a2a9d8ccfe2d74dac991fe38d19
audited_head: 7010774d66b322b32eb6c9bcf4b5bf84af191fee
verdict: DECISION
-->

# A-046 — M-93: закрытие B-1/B-2/B-3, корень установки `HFT_WATCHDOG_ROOT`, хостовый сторож из образа, маршрут к engine-dev

**Созван:** `gates.md` §0 п.2 — третий круг по одному предмету (`C-277` REJECT → `C-278` REJECT →
правки architect'а `d9191d4`/`7010774`). Арбитр — свежий контекст, сильная модель; ведущий
architect — сторона: его правки после `C-278` судятся как предмет, а не принимаются на слово.

**Предмет — ветка** `origin/feat/M-93-watchdog-delivery`; вершина взята командой после `fetch`
(`7010774d66b322b32eb6c9bcf4b5bf84af191fee`), справка мандата `7010774` с ней совпала;
`ede3d98` (предмет `C-278`) и `744e815` (предмет `C-277`) — её предки. База —
`git merge-base origin/main HEAD` = `81132e1`. Собственное detached-дерево `/tmp/hft-arbiter-m93`;
эталон и мутанты строились в нём временно и возвращены (`git status --porcelain` пуст, §7).

**Живой инвариант предмета:** `OPS-I-10` (`docs/fa/ops.md:478` — «ОБЪЯВЛЕНА ⟹ ЭМИТИТСЯ»:
сторож, которого нет в доставке, объявлен, но не исполняется), опора `DESIGN` §23.1 п.1
(`docs/DESIGN.md:961`+). Диапазон ветки `crates/**` не трогает; инвариант назван по существу.

---

## 1. Решение — коротко

| # | вопрос мандата | решение |
|---|---|---|
| 1 | `C-277` B-1, `C-277` B-2, `C-278` B-3 закрыты? | **Все три — ЗАКРЫТЫ**, прогоном (§3): честный эталон §4 ⇒ **9/9 под uid 1002** без `/usr/local/lib/hft` и без привилегий; мутанты M1–M8 спеки §9 — каждый роняет свой сценарий; сверх спеки проверены M1b/M9/M10 — убиты |
| 2 | `HFT_WATCHDOG_ROOT` — недопустимый тестовый шов в прод-коде? ослабляет `I-1` на проде? | **Допустим; `I-1` на проде не ослаблен.** Класс — `DESTDIR` (корень установки), слабее уже принятых швов `RETENTION_RUNNER`/`CHECKPOINT_RUNNER` (подменяют КОМАНДУ) и `WATCHDOG_BIN` (подменяет ПОЛНЫЙ путь). На проде переменной нет ни в cron, ни в `/etc/environment`; расхождение сред не тихое — ловится `I-6` (§4) |
| 3 | сторож — хостовый процесс; бинарь — из образа РАБОТАЮЩЕГО контейнера; `deploy.yml` — architect? | **Обосновано, все три.** Единственное, что требует формы: `deploy.yml` исполняется на проде ⇒ полный цикл + перепроверка §9 — спека это уже говорит (§5) |
| 4 | диспетчеризовать engine-dev на задачи 1–2 без нового круга критика? | **Да** — после одного коммита architect'а с правками §6 (все — изложение/журнал/пределы, ни одна не меняет инвариант, оракул или acceptance). Задача 3 и перепроверка §9 `deploy.yml` — по маршруту спеки §12, без изменений |

Решение обязательно обеим сторонам (`gates.md` §0).

## 2. Что судилось — первоисточники, не пересказ

Прочитаны целиком: `milestones/M-93-watchdog-delivery.md` (`7010774`), `C-277`, `C-278`,
`scripts/tests/red_m93_watchdog_delivery.sh`, `scripts/verify_M-93.sh`, диф `scripts/verify_delivery_M-08.sh`
(D9-deep), `Dockerfile`, `scripts/watchdog_cron.sh`, `deploy/cron.d/watchdog`, `.github/workflows/deploy.yml`
(шаг «Deploy via SSH» `:239-310`), `docs/workflow/harness-track.md` §1–§5, `docs/04-workflow.md` §2 (`:51-75`,
`:235-243`), `docs/fa/ops.md` `OPS-I-10`, `DESIGN` §23.1, `.claude/rules/scope-guard.md` (таблица владения),
шов-прецеденты в `deploy/bin/*.sh`. Прод — только чтение, ssh (§7).

## 3. Вопрос 1 — три находки закрыты ПРОГОНОМ

### 3.1 `C-278` B-3 — `w1b` самодостаточен (решающий пункт круга)

Честный эталон §4 (установщик с `mkdir -p` под `set -e` — отказ каталога = отказ установки,
ничего не глотается; `Dockerfile` `--bin ops-watchdog` + `COPY`; `watchdog_cron.sh` дефолт
`${HFT_WATCHDOG_ROOT:-}/usr/local/lib/hft/ops-watchdog`; `deploy.yml` — вызов на обеих ветках)
собран во временном дереве и прогнан пробой под непривилегированным пользователем:

```text
$ id -u; test -d /usr/local/lib/hft; echo dir=$?
1002
dir=1
$ bash scripts/tests/red_m93_watchdog_delivery.sh; echo exit=$?
pass  w1 композиция: установка печатает и cron зовёт один путь (/usr/local/lib/hft/ops-watchdog), вне target/
pass  w1b путь по умолчанию в настоящем режиме: установка записала, cron зовёт тот же файл (/usr/local/lib/hft/ops-watchdog)
pass  w2 установка: бинарь образа работающего hft-recorder (sha256:run1773515239), исполняемый, без хвостов (inspect create cp rm )
pass  w3 отказ «cp»: exit=1, прежний бинарь цел, хвостов нет
pass  w4 отказ «inspect»: exit=1, прежний бинарь цел, хвостов нет
pass  w5 отказ «empty»: exit=1, прежний бинарь цел, хвостов нет
pass  w6 Dockerfile: ops-watchdog собирается и копируется в /usr/local/bin
pass  w7 deploy.yml: установка сторожа на ветке healthy и на ветке отката
pass  w8 cron без бинаря: exit=1, ALERT в логе и файле тревоги
сценариев: 9   pass=9   FAIL=0
VERDICT: PASS
exit=0
```

Ровно то, чего требовал `C-278`: «honest M-93 §4 delivery form reaches the docker stub and
passes all nine scenarios without a host-specific privilege shim». Механизм — `w1b`
(`red_m93_watchdog_delivery.sh:88-101`): установка пишет под `HFT_WATCHDOG_ROOT=$d/root`,
cron под тем же корнем печатает путь, файл по пути cron'а сверяется `cmp` с бинарём образа,
и отдельно проверяется, что корень — ЧИСТЫЙ префикс (`:96`: путь без корня равен пути `w1`).
Позитивный контроль есть; «вечно-красный `w1b`» (`C-278` §E) снят.

### 3.2 Мутационный контроль — M1–M8 спеки §9 и три сверх неё

Каждый мутант — правка эталона в моём дереве, прогон, восстановление; печатаются только
`FAIL`-строки и итог (сырой вывод усечён до 160 знаков на строку):

```text
### M1 запись прямо в DST (TMP="$DST", форма «mv X X»)
FAIL  w1b … FAIL  w2 … FAIL  w3 … FAIL  w4 … FAIL  w5          сценариев: 9 pass=4 FAIL=5
### M1b docker cp ПРЯМО в DST, без временного файла и без mv (тонкая форма)
FAIL  w5 отказ «empty»: exit=1, dst=«», файлов 1                сценариев: 9 pass=8 FAIL=1
### M2 нет rm контейнера при отказе cp
FAIL  w3 отказ «cp»: exit=1, dst=«OLD», файлов 1 контейнер не удалён после отказа cp
### M3 нет проверки непустоты
FAIL  w5 отказ «empty»: exit=0, dst=«», файлов 1
### M4 cron зовёт target/
FAIL  w1 композиция: установка печатает «/usr/local/lib/hft/ops-watchdog», cron зовёт «/root/hft-platform/target/release/ops-watchdog»
FAIL  w1b … cron под корнем зовёт «/root/hft-platform/target/release/ops-watchdog»
### M5 установка только на ветке healthy
FAIL  w7 deploy.yml: установка сторожа — ветка healthy: 1, ветка отката: 0 (нужно ≥1 на обеих)
### M6 образ захардкожен (inspect вызван, результат проигнорирован)
FAIL  w1b … FAIL  w2 установка: exit=0, create «sha256:hardcoded», … в dst: #!/bin/sh echo FOREIGN-watchdog
FAIL  w4 отказ «inspect»: exit=0 …                                сценариев: 9 pass=6 FAIL=3
### M7 печать — новый путь, запись — старый
FAIL  w1b путь по умолчанию в настоящем режиме: exit=1 …          сценариев: 9 pass=8 FAIL=1
### M8 установка игнорирует корень (DST без ${HFT_WATCHDOG_ROOT:-})
FAIL  w1b путь по умолчанию в настоящем режиме: exit=1 …          сценариев: 9 pass=8 FAIL=1
### M9 inspect чужого контейнера (hft-gateway-serve вместо hft-recorder)
FAIL  w1b … FAIL  w2 установка: exit=0, create «sha256:foreign», … в dst: #!/bin/sh echo FOREIGN-watchdog
### M10 без chmod 0755
FAIL  w2 установка: exit=0, … в dst: #!/bin/sh echo running-image-watchdog   (не исполняем)
=== после восстановления эталона ===
сценариев: 9   pass=9   FAIL=0
VERDICT: PASS
```

Каждый мутант убит. Две поправки к тексту спеки §9 (не дефекты оракула, а точность журнала):
(а) утверждение «„запись прямо в `DST`“ ⇒ `w2`/`w3`/`w5`» зависит от ФОРМЫ мутанта — грубая
форма роняет пять сценариев, тонкая (`docker cp` сразу в `DST`, без `mv`) — только `w5`; одного
достаточно, но спека обязана называть то, что меряла; (б) `M6` роняет `w2`, `w4` **и** `w1b` —
третий сценарий спека не называет.

### 3.3 `C-277` B-1 — поток «inspect(работающий) → create → cp» доказан

`red_m93_watchdog_delivery.sh:29-49`: образ работающего `hft-recorder` — `STUB_IMG`, случайный
на КАЖДЫЙ прогон (`:67` `sha256:run$RANDOM$RANDOM`); `inspect` любого другого объекта отдаёт
`sha256:foreign` (`:40`); `cp` из контейнера чужого образа отдаёт `FOREIGN_BIN` (`:46`). `w2`
(`:107-109`) требует: содержимое `DST` == бинарь образа (`cmp`), `-x`, ровно один файл в
каталоге, `inspect …hft-recorder` позван, образ в `create` начинается на `sha256:run`, контейнер
удалён. Мутанты M6 (`C-277`-класс «inspect позван, результат проигнорирован») и M9 (чужой
контейнер) роняют `w2` чужим бинарём — §3.2. Стражи подготовки: `mk_stub:58` (PATH выбирает
именно заглушку, она исполняема), `run_install` → 99/98 с отдельной веткой `SETUP не состоялся`
в `w3`–`w5` (`:119`), `w1b` — своя (`:101`). Путь по умолчанию в настоящем режиме — `w1b`,
мутант M7 — убит. **Закрыто.**

Предел, названный, но не блокирующий: заглушка не различает `-f '{{.Image}}'` и
`-f '{{.Config.Image}}'` (оба верны на проде: тег `hft-platform-recorder:local` переезжает на
новый образ при `compose up --build`, замер §7) и не отвергает `inspect` без `-f` (на проде
даст JSON в `docker create` ⇒ отказ ⇒ `I-3` fail-closed, виден по красному деплою и §8).

### 3.4 `C-277` B-2 — владение `deploy.yml` и `docs/ROADMAP.md`

Спека `7010774` §7: задача 3 — **architect**, `docs/ROADMAP.md` — в путях architect'а; §8 строка 3
— architect; диф круга 2–3 по `git diff --name-status origin/main...HEAD` — только
`docs/ROADMAP.md`, `milestones/M-93-*`, `scripts/tests/red_m93_*`, `scripts/tests/red_verify_M-93_ci_map.sh`,
`scripts/verify_M-93.sh`, `scripts/verify_delivery_M-08.sh`, два вердикта — всё в §7. **Закрыто.**
Обоснованность самого назначения architect'у — §5.

## 4. Вопрос 2 — `HFT_WATCHDOG_ROOT`: допустимый корень установки, не дыра в `I-1`

**Класс.** Это `DESTDIR` — соглашение установщиков старше репозитория: корень, под который
кладётся неизменный абсолютный путь; на проде пуст. Прецеденты швов в прод-скриптах репозитория
СИЛЬНЕЕ по воздействию: `RETENTION_RUNNER` (`deploy/bin/journal-retention-cron.sh:27`) и
`CHECKPOINT_RUNNER` (`deploy/bin/gateway-checkpoint-cron.sh:33`) подменяют ИСПОЛНЯЕМУЮ КОМАНДУ
целиком, `HFT_CRON_PRINT_ARGV` (`gateway-checkpoint-cron.sh:127`) — ветка печати до побочных
эффектов, и в самом `scripts/watchdog_cron.sh:18` уже живёт `WATCHDOG_BIN` — подмена ПОЛНОГО пути,
причём только с одной стороны композиции. Корень установки меняет префикс пути, читается ОБЕИМИ
сторонами одинаковым выражением и пуст по умолчанию — это слабейший шов из перечисленных.

**`I-1` на проде.** Замер (§7): `/etc/cron.d/hft-watchdog` несёт одну переменную
`WATCHDOG_SERVING_HEARTBEAT_PATH`; `grep -rn HFT_WATCHDOG_ROOT /etc/cron.d/ /etc/environment` —
пусто; шаг «Deploy via SSH» (`deploy.yml:246-310`) её не задаёт. Обе стороны вычисляют
`${HFT_WATCHDOG_ROOT:-}` + один литерал ⇒ при незаданной переменной пути тождественны. Новая ось
расхождения («задана в одной среде, не в другой») — не новый КЛАСС: ось `HFT_WATCHDOG_DST` ↔
`WATCHDOG_BIN` (две независимые подмены полного пути) уже существует и сильнее. И расхождение
НЕ ТИХОЕ: cron с другим путём попадает в `[ ! -x ]` ⇒ `ALERT … бинарь не найден`, exit 1 —
`I-6`/`w8`, проверено пробой. Это тот же аргумент, которым спека §6 п.3 закрывает риск libc.

**Почему не отвергать переменную в cron, как `M-90` `I-3`.** `gateway-checkpoint-cron.sh:85-89`
отвергает ТОЛЬКО оси селектора (собственный источник данных); пути (`CHECKPOINT_JOURNAL_DIR`,
`_CKPT_DIR`) и шов `CHECKPOINT_RUNNER` там прямо названы НЕ отвергаемыми. Корень установки — путь.
Отказ был бы непоследователен с прецедентом и сломал бы `w1b`.

**Альтернативы без шва взвешены:** `unshare -Ur` + bind-mount поверх `/usr/local/lib` — зависит
от userns хоста (apparmor Ubuntu ≥ 23.10 их ограничивает, в CI флак); `fakeroot`/`fakechroot` —
внешняя зависимость; заглушки `mkdir`/`mv`/`cp` в PATH — стабят проверяемую вещь. Корень — наименее
инвазивный и единственный, у которого есть внешнее соглашение.

**Условие (правка §6 E2):** шапка установщика и §4 называют три переменные
(`HFT_WATCHDOG_ROOT`/`_DST`/`_CONTAINER`) швом гейта класса `DESTDIR`/`RETENTION_RUNNER`, на проде
НЕ задаваемым; §8 задача 4 проверяет их отсутствие в окружении cron (E4).

## 5. Вопрос 3 — хостовый сторож, бинарь из образа работающего контейнера, `deploy.yml` у architect'а

**Хостовый процесс — верно.** `ops-watchdog` зовёт `docker ps`/`docker inspect` (`C-277`
проверил по коду); сторож внутри docker не сообщит о смерти демона — ровно класс «жив, но не
работает», против которого стоит `DESIGN` §23.1 п.1–2. Предел назван честно: с ХОСТОМ он судьбу
делит (ребут, мёртвый crond) — это закрывает свежесть `*.last-success` для внешнего монитора
(`deploy/README.md:303`) и канал `П-003`, не предмет M-93.

**Из образа РАБОТАЮЩЕГО контейнера, а не из тега, — верно и точнее.** `{{.Image}}` даёт ID образа
того, что реально крутится: на ветке healthy — новая сборка, на ветке отката — `PREV` (compose
пересобрал по старому чекауту), так что на хосте всегда бинарь того кода, что в контейнерах
(`I-5`). Работающий образ не висячий ⇒ `docker image prune -f` (`deploy.yml:302`) его не трогает,
порядок «установка до/после prune» безразличен. Запрещённые альтернативы (§5 спеки) — верно
запрещены: сборка на хосте — второй источник бинаря (`TD-227`-класс) и toolchain'а нет; bind-mount
из контейнера — бинарь исчез бы вместе с контейнером.

**Совместимость libc — подтверждена замером, не только названа:** ручной бинарь требует
`GLIBC_2.39`, хост даёт `2.43` (§7). Расхождение в будущую сторону ловится `I-6` громко.

**Пользователь деплоя — root, `sudo` установщику не нужен:** `/root` 700 `root:root`, чекаут
`/root/hft-platform` — root, все 258 входов по ключу за 14 дней — `root` (§7). `sudo install`
в `deploy.yml:284` — защитный no-op. Если `VPS_USER` когда-то сменится, `mkdir /usr/local/lib/hft`
откажет ⇒ `=== WATCHDOG INSTALL FAILED ===`, деплой красный — fail-closed, не тихо.

**`deploy.yml` — architect: обосновано.** (а) Таблица `scope-guard.md` НЕ отдаёт
`.github/workflows/**` ни одной роли — ни engine-dev (его `deploy/**` — другой каталог), ни
architect'у поимённо; (б) `docs/04-workflow.md:235-238` и `harness-track.md:48` относят
`.github/workflows/**` к харнессу, автор которого — architect (`harness-track.md:66`); (в) история:
34 из 35 коммитов в `.github/workflows/` и все три меченых коммита `deploy.yml` — `[architect]`;
прецедент `ee2db4e` (`ci.yml`, `roadmap-sync`) — `[architect]`, подтверждён; (г) единственный
обратный прецедент — `M-48` задача 10 (`26b6228`, 2026-07-29, engine-dev) — старше решения о
харнесс-треке (2026-08-15) и таблицы владения. Спека верно НЕ ведёт файл харнесс-треком: три
вопроса `harness-track.md` §4 — «исполняется прод-процессом?» — да (шаг на VPS по ssh) ⇒ полный
цикл + независимая перепроверка §9. Наблюдение вне предмета: профиль architect'а в Writes
`.github/workflows/**` не перечисляет — пробел ПРОФИЛЯ (замок §11, founder), не милестоуна.

**Предел `deploy.yml`, который спека не называет (правка E3):** фильтр `paths:` (`deploy.yml:33-47`)
не содержит ни `deploy/bin/**`, ни `scripts/watchdog_cron.sh`. Сам M-93 деплой запустит
(`Dockerfile` и `deploy.yml` — в фильтре), но ПОСЛЕДУЮЩАЯ правка одного установщика или обёртки
приедет на VPS лишь со следующим кодовым push'ем или `workflow_dispatch`. Тот же класс, что сегодня
у `deploy/cron.d/**` (`TD-150` п.2) — не блокер, но обязан стоять в §6; расширять ли фильтр —
отдельное решение architect'а (оно меняет частоту редеплоев, `TD-086`).

## 6. Обязательные правки architect'а — один коммит на ветке ДО диспетчеризации

Все пять — изложение, пределы и журнал: ни одна не меняет инвариант `I-1`…`I-6`, оракул
`w1`…`w8`, acceptance или Allowed paths. Поэтому они НЕ открывают нового круга критика (§8).

| # | файл | что | предъявление |
|---|---|---|---|
| E1 | `milestones/M-93-watchdog-delivery.md` §13 | строка «3 \| `A-046` **DECISION** (арбитр, `7010774`) \| B-1/B-2/B-3 закрыты прогоном; корень установки допустим (класс `DESTDIR`); маршрут → engine-dev задачи 1–2 \| правки E2–E5» | `grep -c 'A-046' milestones/M-93-watchdog-delivery.md` ≥ 1 |
| E2 | там же §4, пункт `install-watchdog.sh` | дописать: «`HFT_WATCHDOG_ROOT`/`HFT_WATCHDOG_DST`/`HFT_WATCHDOG_CONTAINER` — шов гейта класса `DESTDIR` (прецедент `RETENTION_RUNNER`, `CHECKPOINT_RUNNER`); на проде НЕ задаются — ни `deploy.yml`, ни `deploy/cron.d/watchdog`; шапка скрипта это называет; `sudo` не нужен — деплой идёт root'ом (`A-046` §5)» | `grep -c 'DESTDIR' …M-93….md` ≥ 2 |
| E3 | там же §6 | пункт 5: «фильтр `paths:` `deploy.yml` не содержит `deploy/bin/**` и `scripts/watchdog_cron.sh`: правка одного установщика/обёртки едет на VPS со следующим кодовым push'ем или `workflow_dispatch` (класс `TD-150` п.2); расширение фильтра — отдельное решение» | `grep -c 'workflow_dispatch' …M-93….md` ≥ 1 |
| E4 | там же §8, задача 4 (reviewer) | добавить проверку: `grep -c 'HFT_WATCHDOG_ROOT\|HFT_WATCHDOG_DST\|WATCHDOG_BIN' /etc/cron.d/hft-watchdog /etc/environment` → 0 на обоих | строка в таблице §8 |
| E5 | там же §9, абзац «Мутационный контроль» | круг 3 (арбитр): «M1 грубая форма ⇒ `w1b`/`w2`/`w3`/`w4`/`w5`; M1b (`docker cp` прямо в `DST`, без `mv`) ⇒ только `w5`; M6 ⇒ `w2`/`w4`/`w1b`; M9 (чужой контейнер) ⇒ `w2`/`w1b`; M10 (без `chmod`) ⇒ `w2`» | `grep -c 'M1b' …M-93….md` ≥ 1 |
| E6 | `docs/ROADMAP.md` строка 4bis, колонка «Состояние» | «круг критика не начат» протухло: заменить на «`C-277`/`C-278` REJECT → `A-046` DECISION; engine-dev задачи 1–2» | `grep -c 'A-046' docs/ROADMAP.md` ≥ 1 |

Коммит: `docs(M-93): круг 3 — A-046: журнал, пределы §6, швы §4, проверка §8 [architect]`; push
ветки до handoff (`gates.md` §8). Статус-колонки и журнал — «изложение» по `gates.md` §9.

## 7. Done Block

```text
$ cd /home/nous/hft-platform && git fetch origin && git rev-parse origin/feat/M-93-watchdog-delivery
7010774d66b322b32eb6c9bcf4b5bf84af191fee
$ git worktree add --detach /tmp/hft-arbiter-m93 origin/feat/M-93-watchdog-delivery
HEAD is now at 7010774 docs(M-93): спека круг 3 — корень установки в §4/I-1, w1b и M8 в §9, журнал C-278 [architect]
$ git merge-base origin/main HEAD
81132e10c0d51a2a9d8ccfe2d74dac991fe38d19
$ git log --oneline origin/main..HEAD | wc -l
11

$ git log --format='%s' -- .github/workflows/deploy.yml | grep -o '\[[a-z-]*\]' | sort | uniq -c
      3 [architect]
$ git log --format='%s' -- .github/workflows/ | grep -o '\[[a-z-]*\]' | sort | uniq -c
     34 [architect]
      1 [engine-dev]
$ git show --stat --format='%h %s' ee2db4e | head -3
ee2db4e ci: барьер ps-on-archive и его проба в джобе roadmap-sync (R-213 Ф-2) [architect]
 .github/workflows/ci.yml | 13 +++++++++++++
$ git log -1 --format='%cd %s' --date=short 26b6228
2026-07-29 build(M-48): task #10 — deploy.yml УСТАНАВЛИВАЕТ deploy/cron.d/* на VPS (B3)

$ grep -rn 'CHECKPOINT_RUNNER=\|RETENTION_RUNNER=\|HFT_CRON_PRINT_ARGV:-0' deploy/bin/*.sh | head -4
deploy/bin/journal-compaction-cron.sh:33:RETENTION_RUNNER="${RETENTION_RUNNER:-docker compose run --rm journal-compaction}"
deploy/bin/journal-retention-cron.sh:27:RETENTION_RUNNER="${RETENTION_RUNNER:-docker compose run --rm journal-retention}"
deploy/bin/gateway-checkpoint-cron.sh:33:CHECKPOINT_RUNNER="${CHECKPOINT_RUNNER:-docker compose run --rm gateway-checkpoint}"
deploy/bin/gateway-checkpoint-cron.sh:127:if [ "${HFT_CRON_PRINT_ARGV:-0}" = "1" ] || [ "${RETENTION_PRINT_ARGV:-0}" = "1" ]; then

$ # эталон §4 (временно: deploy/bin/install-watchdog.sh, Dockerfile, scripts/watchdog_cron.sh, deploy.yml)
$ git diff --stat
 .github/workflows/deploy.yml | 2 ++
 Dockerfile                   | 3 ++-
 scripts/watchdog_cron.sh     | 2 +-
$ id -u; test -d /usr/local/lib/hft; echo dir=$?
1002
dir=1
$ bash scripts/tests/red_m93_watchdog_delivery.sh | tail -2; echo exit=$?
сценариев: 9   pass=9   FAIL=0
VERDICT: PASS
exit=0
$ # мутанты M1–M8 + M1b/M9/M10 — §3.2 (каждый FAIL, эталон после восстановления 9/9)
$ git checkout -- Dockerfile scripts/watchdog_cron.sh .github/workflows/deploy.yml && rm -f deploy/bin/install-watchdog.sh && rmdir deploy/bin; git status --porcelain; echo status-exit=$?
status-exit=0

$ ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 '<только чтение>'
== uid ==                0
== git ==                26c9a019
== paths ==              ls: cannot access '/usr/local/lib/hft/ops-watchdog': No such file or directory
                         -rwxr-xr-x 1 root root 5030640 Oct  2 10:41 /root/hft-platform/target/release/ops-watchdog
== running recorder ==   sha256:c79f7c06caea03650b9b5293a6af1609d4ab6ec45b0738c392146389b85ce47e  /  hft-platform-recorder:local
== docker ==             Docker version 29.6.1, build 8900f1d
== cron ==               WATCHDOG_SERVING_HEARTBEAT_PATH=/var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat
                         */5 * * * * root flock -n /var/lock/hft-watchdog.lock /root/hft-platform/scripts/watchdog_cron.sh
== HFT_WATCHDOG_ROOT в /etc/cron.d, /etc/environment ==   (пусто)
== log tail ==           [ops-watchdog] 1791053701887 — норма, ни одно условие не сработало
== image has ops-watchdog? ==  no
== /root ==              root:root 700 /root
== accepted publickey (14 сут) ==   258 root
== glibc ==              host 2.43; бинарь требует GLIBC_2.39

$ bash scripts/check_branch_health.sh | tail -3
веток кроме main: 14; замечаний: 0
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
exit=0

$ bash scripts/next_artifact_id.sh A
A-046
$ bash scripts/reserve_artifact_id.sh A
reserve: попытка 1/8 — A-046 ← 8d8f0a18aa2cdc77b3568e2c93f001e7e7985621
A-046
reserve: резерв A-046 взят; снять после приземления носителя

$ git fetch origin && git rev-parse origin/feat/M-93-watchdog-delivery   # перед записью, 2026-10-03T19:07Z
7010774d66b322b32eb6c9bcf4b5bf84af191fee
дрейфа нет
```

## 8. Вопрос 4 — маршрут

Решение арбитра обязательно обеим сторонам и закрывает спор по трём находкам (`gates.md` §0).
Нового блокера набор не содержит: честный эталон зелёный без привилегий, мутанты убиты, стражи
подготовки на месте, владение путей согласовано. Правки §6 — изложение/пределы/журнал, формы не
меняют ⇒ критик по `gates.md` §9 не триггерится; требовать четвёртый круг значило бы повторить
то, что уже проверено исполнением здесь. **После коммита E1–E6 и push'а ветки architect
диспетчеризует engine-dev на задачи 1–2** (мандат — ветка `origin/feat/M-93-watchdog-delivery`,
зона §7, acceptance `bash scripts/tests/red_m93_watchdog_delivery.sh` 9/9 и
`bash scripts/verify_M-93.sh`). Далее — §12 спеки без изменений: architect — задача 3
(`deploy.yml`, полный цикл, перепроверка §9 независимым Fable-агентом или вердикт reviewer'а,
покрывший (а)–(в)), tester, reviewer (§8 задача 4 с E4; `TD-231`/`TD-220`).

Несогласие любой стороны фиксируется в этом файле отдельной секцией, решения не отменяет.
