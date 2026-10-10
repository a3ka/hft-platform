<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 3753efb5b30305466ca71280d28832cea060a712
audited_head: 2dcea69dab785c0a4f4e70bc8293114ab63c7d7b
verdict: APPROVE
-->

# R-241 — адверсарий шага D10 `verify_delivery_M-08.sh` (compose в прод-форме деплоя), M-92

- Роль: адверсарий харнесс-трека (`docs/workflow/harness-track.md` §3), свежий контекст;
  засчитывается как перепроверка `gates.md` §9 (а)/(б)/(в).
- Предмет: ветка `fix/m92-compose-workdir`. На момент старта вершина `a572c2e` (D10,
  architect); во время аудита приехал `2dcea69` (правка `docker-compose.yml`, engine-dev) —
  судится вершина `2dcea69`, D10 введён в `a572c2e`.
- Дата: 2026-10-04T23:44Z. Диапазон `crates/**` не трогает → `check_review_fa.sh` даст `SKIP`,
  FA-требование здесь когнитивное; живой инвариант предмета — `I-8` (`milestones/M-92-manifest-verified-prune.md:46`,
  топология `journal-retention`).

## Вердикт: APPROVE

D10 делает то, что обещает: воспроизводит отказ Deploy `37242446986` в форме, совпадающей с
прод-формой деплоя (проверено НА САМОМ VPS: compose 5.3.1, настоящий `.env`), падает против
`main`, зеленеет против правки `2dcea69`, краснеет при неверном умолчании и устойчив к обоим
способам «подсунуть переменную» (окружение оператора, `.env` каталога). Пути без docker ведут
себя как объявлено (SKIP без `DEEP`, FAIL при `HFT_DELIVERY_DEEP=1`); джоб `delivery` стоит в
агрегате `All checks passed` (`.github/workflows/ci.yml:630`). Блокеров нет. Находки ниже —
`Н` (не блокируют), две из них — пробелы, которые автору надо либо закрыть, либо принять записью.

## 1. Прод-форма — чем деплой зовёт compose и чем это отличается от D10

Источник: `.github/workflows/deploy.yml:244-247` (`appleboy/ssh-action`, `cd /root/hft-platform`),
`:311` — `docker compose up -d --build recorder gateway-serve`: **без `-f`, без `--profile`,
без `--env-file`**. Замер VPS (только имена, значения не печатались):

```
$ ssh … 'cut -d= -f1 /root/hft-platform/.env'
GATEWAY_JWT_SECRET
GATEWAY_BANDS
$ ssh … 'docker compose version'            → Docker Compose version v5.3.1
$ ssh … 'env | cut -d= -f1 | sort | tr "\n" " "'   (сессия ssh без tty, как у деплоя)
_ DBUS_SESSION_BUS_ADDRESS HOME LANG LOGNAME PATH PWD SHELL SHLVL SSH_CLIENT SSH_CONNECTION USER XDG_RUNTIME_DIR XDG_SESSION_CLASS XDG_SESSION_ID XDG_SESSION_TYPE
$ ssh … 'env | cut -d= -f1 | grep -E "^(COMPOSE_|DOCKER_|RETENTION)" || echo none'   → none
$ ssh … 'ls -a /root/hft-platform | grep -iE "^\.env|compose|override"'
.env  .env.bak-1789933437  docker-compose.override.yml  docker-compose.yml
$ ssh … 'git status --porcelain --ignored | grep -iE "override|\.env"'
?? docker-compose.override.yml
!! .env
!! .env.bak-1789933437
```

| ось | деплой (VPS) | D10 (`scripts/verify_delivery_M-08.sh:424-426`) | совпадает? |
|---|---|---|---|
| переменные окружения, влияющие на compose | нет `COMPOSE_*`/`DOCKER_*`; из `.env` — ровно два имени | `env -i PATH HOME` + те же два имени (`--env-file /dev/null`) | да |
| файл(ы) compose | без `-f` ⇒ `docker-compose.yml` **+ `docker-compose.override.yml`** (на VPS лежит untracked, 468 B, 2026-07-25; только `recorder.environment` с ключами Binance) | `-f docker-compose.yml` — override **не мержится** | **нет** → Н-1 |
| профили | без `--profile` | `--profile "*"` | нет, но безвредно: интерполяция идёт ДО фильтра профилей (C2, C6a/C6d — обе формы падают одинаково); а для ВТОРОЙ половины шага флаг обязателен (без него `journal-retention` отсутствует в выводе — см. §2 C2′) |
| версия compose | 5.3.1 | локально 2.39.1; раннер CI `ubuntu-24.04` — 2.38.2 (`actions/runner-images`, `Ubuntu2404-Readme.md`: «Docker Compose 2.38.2», «Python 3.12.3») | класс ошибки одинаков на обеих мажорных линиях — лог деплоя (5.3.1) и локальный C1 (2.39.1) дают одну строку; реплей на VPS ниже |

Лог упавшего деплоя — та самая ошибка:

```
$ gh run view 37242446986 --log | grep -E 'empty section|=== deploy|DEPLOY FAILED'
… === deploy 086069b9 (prev 087ba995) ===
… invalid spec: :/work: empty section between colons
… === DEPLOY FAILED — logs + rollback to 087ba995 ===
```

**Реплей прод-формы на самом VPS** (только `config`, без `up`; файлы ветки скопированы в
`/tmp` и удалены после; `--project-directory /root/hft-platform` ⇒ читается ЕГО `.env`,
override подключён явно, `--profile` нет):

```
--- compose 5.3.1; форма деплоя = project-dir /root/hft-platform (его .env) + override, без --profile
[branch.yml]   (docker-compose.yml с a572c2e = main)
  rc=1
  err: empty section between colons
[fixed.yml]    (byte-идентичен docker-compose.yml с 2dcea69: `diff` пуст)
  rc=0
  journal-retention в выводе без --profile: False
--- fixed.yml с --profile "*": source /work
   ['/var/lib/hft/retention-work']
--- cleaned: 0
```

Модель D10 подтверждена прод-формой в обе стороны: до правки — тот же отказ, после — разбор
проходит, источник `/work` разрешается в ожидаемое умолчание. Ожидаемое умолчание совпадает
со спекой и cron-обёрткой: `milestones/M-92-manifest-verified-prune.md:100`,
`deploy/bin/journal-retention-cron.sh:80`, `deploy/cron.d/journal-retention:64`.

## 2. Мутанты — что ловится, что нет

Прямые вызовы compose с флагами D10 над копиями файла (скрипт в scratchpad, не в репозитории):

```
## C1 baseline: ветка a572c2e, D10-флаги
rc=1  err: empty section between colons
## C2 мутант без --profile '*':
rc=1  err: empty section between colons
## C3 .env каталога проекта задаёт RETENTION_WORK_DIR
   шаг (--env-file /dev/null): rc=1  err: empty section between colons
   мутант (без --env-file):    rc=0
## C4 RETENTION_WORK_DIR в окружении оператора
   шаг (env -i):    rc=1
   мутант (без env -i): rc=0
## C5 позитивный контроль и проверка источника
   (a) верное умолчание:   rc=0      source=/var/lib/hft/retention-work
   (b) неверное умолчание: rc=0      source=/tmp/wrong        ← мутант без проверки источника дал бы PASS
   (c) путь захардкожен, переменной нет: rc=0  source=/var/lib/hft/retention-work  ← D10 PASS (вне объёма D10; ловит `red_m92_manifest_prune.rs:1105` — `contains("RETENTION_WORK_DIR")`)
## C6 иная переменная без умолчания в другом сервисе (поверх исправленного файла)
   (a) volume-форма, ПРОФИЛЬНЫЙ gateway-checkpoint — D10-флаги: rc=1 ; форма деплоя (без --profile/-f): rc=1
   (b) environment-форма в recorder — D10-флаги: rc=0, warnings 'not set' в stderr: 1 ; форма деплоя: rc=0
   (d) :? в ПРОФИЛЬНОМ gateway-checkpoint — D10-флаги: rc=1 ; форма деплоя (без --profile): rc=1
## C7 docker-compose.override.yml рядом с `${OVR_NO_DEFAULT}:/ovr` у recorder
   D10-флаги (-f пиннит один файл):           rc=0
   форма деплоя (без -f → override мержится): rc=1  err: empty section between colons
```

C2′ — тот же мутант «без `--profile`» ПОСЛЕ правки, но через логику самого шага:

```
$ … docker compose --env-file /dev/null -f docker-compose.yml config --format json | python3 -c '…journal-retention…/work…'
source=''
```

то есть шаг без `--profile "*"` не пропускает молча, а падает ЛОЖНО («источник = ''»): флаг
несущий, но его потеря громкая.

Шаг через сам скрипт — с переменной в окружении оператора (должен не замечать её):

```
$ RETENTION_WORK_DIR=/var/lib/hft/retention-work bash scripts/verify_delivery_M-08.sh | grep -E 'D10|DELIVERY'   (вершина a572c2e)
FAIL  D10 compose НЕ разбирается в окружении деплоя (только GATEWAY_JWT_SECRET/GATEWAY_BANDS) — …
DELIVERY: FAIL (1)
exit=1
```

Итог по мутантам: все четыре названных в мандате ловятся **шагом как он написан** (C3, C4 —
флаги срабатывают; C5b — проверка источника срабатывает; C2 — профиль не несущий для разбора
и громко несущий для источника). Но ни один из них не ловится **ничем, кроме самого шага**:
пробы для D10 нет (`ls scripts/tests | grep -iE 'deliver|d10|compose'` — пусто), и мутант
шага без `env -i` или без `--env-file /dev/null` на машине, где переменная задана, проходит
зелёным, никем не замеченный. Это Н-2.

## 3. Без docker и в CI

```
=== S1: docker compose недоступен (шим docker→exit 1), DEEP unset
SKIP  D10 (разбор compose в окружении деплоя) — нет docker compose; в CI (HFT_DELIVERY_DEEP=1) обязателен
DELIVERY: PASS
exit=0
=== S2: docker недоступен, HFT_DELIVERY_DEEP=1
FAIL  D1-deep прод-образ не собирается
FAIL  D10 docker compose недоступен — разбор compose в прод-форме проверить нечем
DELIVERY: FAIL (2)
exit=1
```

CI: джоб `delivery` (`ci.yml:38-45`, `HFT_DELIVERY_DEEP: "1"`) входит в `needs:` агрегата
(`ci.yml:630`); раннер несёт docker compose 2.38.2 и python3 3.12.3 — D10 там исполняется, а не
скипается. На `main` (без правки compose) D10 даст FAIL и удержит merge — и это правильно: `main`
сейчас не деплоится.

## 4. Мусор: `.d10.err`

```
=== S3: compose исправлен, python3 падает (шим python3→exit 1)
PASS  D9 cron-задания пишут позитивный *.last-success — …
python3 shim: boom
exit=1                      ← строки DELIVERY: нет, скрипт оборван set -e
--- leftovers:
-rw-rw-r-- 1 nous nous 0 Oct  4 23:37 .d10.err
?? .d10.err
=== S4: тот же compose, честный прогон
PASS  D10 compose разбирается в окружении деплоя; /work ← /var/lib/hft/retention-work по умолчанию
DELIVERY: PASS
exit=0
ls: cannot access '.d10.err': No such file or directory
```

На честном пути файл убирается (`:443`). На пути «python3 отказал/отсутствует» — остаётся
untracked в корне репозитория (`.gitignore` его не знает: `grep -nE 'd10|\.err' .gitignore` —
пусто), а вердикт `DELIVERY:` не печатается вовсе. Exit=1 — fail-closed, так что ворота не
дырявые; но это Н-3.

## Находки

**Н-1 — `-f docker-compose.yml` расходится с прод-формой: деплой мержит `docker-compose.override.yml`.**
`scripts/verify_delivery_M-08.sh:426`. Деплой (`deploy.yml:311`) зовёт compose без `-f`, и
compose подхватывает `docker-compose.override.yml`, если он лежит рядом; на VPS он лежит
(untracked, с 2026-07-25). C7: интерполяция без умолчания в override → D10 rc=0, деплой rc=1.
Сегодня override на VPS содержит только `environment` recorder'а без `${…}`, и файлы вне
репозитория D10 не увидит ни при каком флаге — поэтому не блокер. Но `-f` здесь ничего не
покупает: в CI override нет, а локально он у оператора может быть и именно он у деплоя
читается. Убрать `-f` — шаг становится равен форме деплоя бесплатно. Автору решать; предел
«VPS-only файлы невидимы» в любом случае стоит назвать в комментарии шага.

**Н-2 — несущие флаги шага не пиннит ничто, кроме самого шага (`harness-track.md` §5 п.2).**
Мутанты «без `env -i`» (C4) и «без `--env-file /dev/null`» (C3) зелёные везде, где переменная
задана; «без проверки источника» (C5b) зелёный при неверном умолчании. Пробы `scripts/tests/red_*`
для D10 нет; коммит `a572c2e` предъявляет мутацию ПРЕДМЕТА (compose), не ШАГА. Для соседнего
D6b проба есть (`red_runbook_markers.sh`, A-047). По `binding-requires-mechanism`: либо проба
(шаг вынести в функцию/`scripts/lib`, стабы — C3/C4/C5b из этого вердикта, они готовы), либо
остаточный риск принять записью. Конструкцию я не расширяю — помечаю.

**Н-3 — `.d10.err` в корне репозитория + новая зависимость от `python3`.**
`:426` пишет ошибку в `${ROOT}/.d10.err`; `rm -f` на `:443` не достигается, если `python3`
на `:433` отказал — `set -e` обрывает скрипт без строки `DELIVERY:`, файл остаётся
untracked в общем чекауте (`branch-hygiene.md` п.7 — класс «незнакомый untracked-файл»).
`python3` в этом скрипте используется только здесь (`grep -n python3` → одна строка, `:433`):
D10 вводит зависимость, которой у гейта доставки не было; на хосте без python3 скрипт не
скажет «FAIL D10», а молча оборвётся. Дёшево лечится `mktemp` + `trap`/ветка `|| d10_src=…`;
на раннере CI python3 есть (3.12.3), так что CI это не трогает.

**Н-4 (вне объёма D10) — `environment`-форма `${X}` без умолчания проходит в обеих формах.**
C6b: compose лишь предупреждает «variable is not set» и подставляет пустую строку — деплой
не упал бы, D10 не падает; расхождения нет. Если захочется ловить и ЭТОТ класс (пустое
значение в `environment` — тихая деградация, не откат), шаг мог бы считать предупреждения
«is not set» в `.d10.err`. Кандидат, не находка против D10.

**Н-5 (вне объёма, спека) — формулировка `I-8` протухнет после `2dcea69`.**
`milestones/M-92-manifest-verified-prune.md:46` и `:103` пишут `${RETENTION_WORK_DIR}:/work`
буквально; после правки форма — `${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}:/work`.
Rust-оракул `crates/journal/tests/red_m92_manifest_prune.rs:1105` проверяет только
`contains("RETENTION_WORK_DIR")` — зелёный; строка `:100` спеки умолчание уже называет.
Изложение, не форма: автор правит сам.

## Что проверено и держится

- Разбор: `env -i` до двух имён + `--env-file /dev/null` — ровно то окружение, что у деплоя
  (замер VPS); C3/C4 показывают, что оба флага работают.
- Позитивный контроль на НАСТОЯЩЕЙ правке `2dcea69` (`git show 2dcea69 -- docker-compose.yml`:
  одна строка `${RETENTION_WORK_DIR}` → `${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}`):
  `bash scripts/verify_delivery_M-08.sh` → `PASS D10 … /work ← /var/lib/hft/retention-work`,
  `DELIVERY: PASS`, exit=0; тот же файл на VPS с compose 5.3.1 — rc=0, источник разрешён.
- Неверное умолчание — FAIL по источнику (C5b); `--profile "*"` — не меняет исход разбора
  (C2, C6a, C6d) и обязателен для источника (C2′ падает громко).
- Без docker — SKIP/FAIL как объявлено (S1/S2); джоб в агрегате; на раннере docker compose есть.
- Мусора после честного прогона нет (S4; `git status --porcelain` пуст после прогонов).

## Done Block

```
$ git -C /tmp/hft-adv-d10 rev-parse HEAD
2dcea69dab785c0a4f4e70bc8293114ab63c7d7b
$ git merge-base HEAD origin/main
3753efb5b30305466ca71280d28832cea060a712
$ git diff --stat origin/main...a572c2e
 scripts/verify_delivery_M-08.sh | 35 +++++++++++++++++++++++++++++++++++
$ git show 2dcea69 --stat --format='%h %s' | tail -1
 docker-compose.yml | 2 +-
$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D10|DELIVERY'; echo exit=$?      # a572c2e
FAIL  D10 compose НЕ разбирается в окружении деплоя (только GATEWAY_JWT_SECRET/GATEWAY_BANDS) — деплой упадёт и откатится, как Deploy 37242446986:
      time="2026-10-04T23:35:19Z" level=warning msg="The \"RETENTION_WORK_DIR\" variable is not set. Defaulting to a blank string."
      invalid spec: :/work: empty section between colons
DELIVERY: FAIL (1)
exit=1
$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D10|DELIVERY'; echo exit=$?      # 2dcea69
PASS  D10 compose разбирается в окружении деплоя; /work ← /var/lib/hft/retention-work по умолчанию
DELIVERY: PASS
exit=0
$ git status --porcelain      # после прогонов, до добавления этого файла
(пусто)
```

Полные сырые выводы мутантов, шимов и реплея на VPS — в §1–§4 выше; все они сняты в этом
же аудите, в отсоединённом worktree `/tmp/hft-adv-d10`, временные файлы на VPS удалены
(`cleaned: 0`).
