<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 0f897d3b14730378156ca22e1b3015a2c2d425da
audited_head: c96948e7d61cf19cb79d1378926ab247089d0fe8
verdict: APPROVE
-->

# R-243 — адверсарий D10 «compose в прод-форме деплоя» (библиотека + проба + CI), M-92

- Роль: адверсарий харнесс-трека (`docs/workflow/harness-track.md` §3), свежий контекст;
  засчитывается как перепроверка `gates.md` §9 (а)/(б)/(в). Второй круг после `R-241`
  (Н-1…Н-3 того вердикта — предмет этого).
- Предмет: ветка `harness/d10-deploy-form`, вершина `c96948e` (один коммит над базой
  `0f897d3`): `scripts/lib/compose_deploy_form.sh` (новая), `scripts/tests/red_compose_deploy_form.sh`
  (новая, 9 сценариев), шаг D10 в `scripts/verify_delivery_M-08.sh:416-427`, шаг в джобе
  `delivery` `.github/workflows/ci.yml:48-49`. Worktree отсоединённый `/tmp/hft-adv-d10b`.
- Дата: 2026-10-05T11:20Z. Диапазон `crates/**` не трогает:
  `bash scripts/check_review_fa.sh 0f897d3… HEAD` → `SKIP (диапазон не трогает crates/**)`;
  живой инвариант предмета — `I-8` (`milestones/M-92-manifest-verified-prune.md:46`,
  топология `journal-retention`, том `/work`).

## Вердикт: APPROVE

Все три находки `R-241` закрыты исполнением, не словом: compose зовётся без `-f` и override
каталога читается (C6 пиннит: мутант с `-f` → `FAIL C6`); ошибка разбора идёт в `mktemp`, корень
репозитория чист на всех путях (V1–V5: `git status --porcelain` пуст, `.d10.err` нет); отказ
`python3` даёт строку `FAIL D10 … '<разбор JSON не удался>'` и `DELIVERY: FAIL (1)`, а не обрыв
(V4). Каждый НЕСУЩИЙ флаг библиотеки роняет свой сценарий (таблица §1: 8 мутантов из 14 ловятся,
и это ровно те, что меняют исход проверки). D10 краснеет на сломанном `docker-compose.yml` той же
строкой, что Deploy `37242446986`, и не поддаётся маскировке ни окружением оператора, ни `.env`
репозитория (V2/V3). Проба убирает фикстуры (число — 0; `tmp.*` в `TMPDIR` не прибавляется).
Джоб `delivery` стоит в агрегате `All checks passed` (`ci.yml:632`).

Блокеров нет. Находки ниже — `Н`: шесть мутантов проходят пробу, и один из них — обманный стаб,
не зовущий compose вовсе. Ни один не меняет исход честной проверки на настоящем файле, поэтому
не блокирую; но Н-1 — ровно тот класс («`df` мимо PATH», M-60), ради которого в треке стоит
адверсарий, и автору его надо либо закрыть (сценарий готов, см. ниже), либо принять записью.

## 1. Перечень проверок библиотеки → мутант → какой сценарий падает

Мутанты — `sed` над копией `scripts/lib/compose_deploy_form.sh`, проба запускалась с
`COMPOSE_DEPLOY_FORM_LIB=<мутант>` (скрипты — в scratchpad, не в репозитории). Строки —
`compose_deploy_form.sh:27-47`.

| # | строка / свойство | мутант | упавшие сценарии | пиннится? |
|---|---|---|---|---|
| L1 | `:27` `env -i` — окружение вызывающего очищено | `env -i PATH` → `env PATH` | `C5` | да |
| L2 | `:29` `--env-file /dev/null` — `.env` каталога не читается | флаг убран | `C4` | да |
| L3 | `:29` без `-f` — override каталога мержится (R-241 Н-1) | добавлен `-f docker-compose.yml` | `C6` | да |
| L4 | `:29` `--profile "*"` — служба под профилем видна | флаг убран | `C1 C7 C9` | да (громко: источник `''`) |
| L5 | `:28` `GATEWAY_JWT_SECRET=…` — первое имя прод-`.env` | убрано | `C1 C3 C7 C9` | да |
| L6 | `:28` `GATEWAY_BANDS=…` — второе имя прод-`.env` | убрано | **нет** | **нет** → Н-2 |
| L7 | `:31` `rc≠0` → «разбор НЕ прошёл», `return 1` | `if false` | `C2 C4 C5 C6 C8` (по причине: отказ есть, но «ожидался» вместо «разбор НЕ прошёл») | да |
| L8 | `:43` `src = want` — сверка источника | `if true` | `C3` | да |
| L9 | `:43` `want` — ПАРАМЕТР, не константа | `"${want}"` → `"/var/lib/hft/retention-work"` | **нет** | **нет** → Н-2 |
| L10 | `:40` источник берётся у `journal-retention`, не у любого сервиса | `svc` = все тома всех сервисов | **нет** | **нет** → Н-2 |
| L11 | `:27` `cd "${dir}"` — проверяется переданный каталог | `cd "${ROOT:-.}"` | `C2 C3 C4 C5 C6 C8` | да |
| L12 | `:42` `\|\| src="<разбор JSON не удался>"` — отказ `python3` назван | ветка убрана | **нет** | **нет** → Н-5 (поведение fail-closed и без неё, V5) |
| L13 | `:34` `rm -f "${err}"` на пути отказа — без мусора | строка убрана | **нет** (+5 файлов `tmp.*` в `TMPDIR` за прогон) | **нет** → Н-4 |
| L14 | `:29` `2>"${err}"` + `:33` печать причины compose | `2>/dev/null` | **нет** | **нет** → Н-3 (диагностика не пиннится) |

Сырой вывод (имена `M<n>_…`; `tmp:` — число `tmp.*` в `TMPDIR` до→после):

```
MUTANT                   VERDICT  упавшие сценарии
M1_no_env-i              exit=1   C5  tmp:2482→2482
M2_no_envfile            exit=1   C4  tmp:2482→2482
M3_with_-f               exit=1   C6  tmp:2482→2482
M4_no_profile            exit=1   C1 C7 C9  tmp:2482→2482
M5_no_jwt                exit=1   C1 C3 C7 C9  tmp:2482→2482
M6_no_bands              exit=0   <ничего> tmp:2482→2482
M7_rc_ignored            exit=1   C2 C4 C5 C6 C8  tmp:2482→2482
M8_src_ignored           exit=1   C3  tmp:2482→2482
M9_want_hardcoded        exit=0   <ничего> tmp:2482→2482
M10_no_py_fallback       exit=0   <ничего> tmp:2482→2482
M11_no_rm_err_on_fail    exit=0   <ничего> tmp:2482→2482
M12_dir_ignored          exit=1   C2 C3 C4 C5 C6 C8  tmp:2482→2482
M13_any_service_work     exit=0   <ничего> tmp:2482→2482
M14_err_to_devnull       exit=0   <ничего> tmp:2482→2482
```

(`tmp:` в этой таблице считался по `/tmp`, а `mktemp` в сессии пишет в `TMPDIR=/home/nous/.cache/paxio-tmp`;
замер M11 по правильному каталогу — §4.) Заявление автора «мутанты 7/7 роняют свой сценарий»
(тело `c96948e`) подтверждается: его семь — это L1–L5, L7, L8 плюс L11; расхождение с моей
таблицей — в том, что я считал СТРОКИ библиотеки, а не только задуманные проверки.

## 2. Обманная реализация, проходящая пробу

**Стаб без docker.** Пятнадцать строк bash: регулярками по `docker-compose.yml` +
`docker-compose.override.yml` ищет `${ИМЯ}`/`${ИМЯ:?…}` вне двух прод-имён → печатает «разбор НЕ
прошёл … empty section between colons»; источник `/work` вырезает из `${RETENTION_WORK_DIR:-…}:/work`.
Compose не вызывается ни разу:

```
$ COMPOSE_DEPLOY_FORM_LIB=…/STUB_nodocker.sh bash scripts/tests/red_compose_deploy_form.sh
ok    C1 … ok    C9 …
сценариев: 9, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
exit=0
# шим docker: `compose version` → rc 0, всё остальное → "DOCKER CALLED" + exit 1
$ PATH=…/shim:$PATH COMPOSE_DEPLOY_FORM_LIB=…/STUB_nodocker.sh bash scripts/tests/red_compose_deploy_form.sh | tail -2
сценариев: 9, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
# контроль — честная библиотека под тем же шимом краснеет:
$ PATH=…/shim:$PATH bash scripts/tests/red_compose_deploy_form.sh | grep -E 'FAIL|VERDICT' | head -3
FAIL  C1 верное умолчание, служба под профилем — ожидалось pass, rc=1
FAIL  C3 неверное умолчание — отказ есть, но не по причине «ожидался»
FAIL  C7 обязательная переменная прод-.env задана — ожидалось pass, rc=1
```

Все девять фикстур одной формы (`"${X…}:/work"` в `volumes`), и регулярка эту форму покрывает.
Проба не пиннит, что разбор ИДЁТ ЧЕРЕЗ compose, — то есть не отличает «прод-форму деплоя» от её
имитации. Это Н-1. Второй обман в пределах конструкции — зашитый `want` (L9, M9): проба всегда
передаёт один и тот же `WANT`, равный константе из `verify_delivery_M-08.sh:420`.

## 3. Агрегация: отказ ≠ эта причина (урок A-048)

`expect` для `fail`-сценариев проверяет подстроку `why`, но `why` — ПРЕФИКС библиотеки
(«разбор НЕ прошёл»), а не причина compose. Копия пробы, где в C2 вместо подстановки без умолчания
— синтаксический мусор YAML, в C6 override — мусор, в C8 — незакрытая скобка:

```
ok    C2 подстановка без умолчания (rc=1)
ok    C6 override каталога читается, как у деплоя (rc=1)
FAIL  C8 SETUP НЕ СОСТОЯЛСЯ            ← страж setup'а (grep OTHER_NO_DEFAULT) сработал
ok    C8 дефект в другом сервисе роняет весь разбор (rc=1)
FAIL  C9 … rc=1                        ← артефакт копии вне репо (ROOT от $0), не предмет
# что библиотека печатает на мусорном override:
compose-deploy-form: разбор НЕ прошёл в окружении деплоя (rc=1):
  yaml: line 1: did not find expected key
```

C2 и C6 «ok» при фикстуре, которая проверяет не то. У C6 страж setup'а — только `[ -f override ]`,
у C2 стража нет. Библиотека при этом ПЕЧАТАЕТ причину compose (`variable is not set`,
`empty section between colons` — одна и та же строка на 2.39.1 здесь и 5.3.1 на VPS по `R-241` §1),
так что пиннить её дёшево. Это Н-3; вместе с L14 (`2>/dev/null` не ловится).

## 4. D10 в `verify_delivery_M-08.sh`: краснеет, не маскируется, не мусорит

`TMPDIR=/home/nous/.cache/paxio-tmp`; `tmp.*` считался там.

```
=== V1 честный прогон (без DEEP)
PASS  D10 compose в окружении деплоя: compose-deploy-form: разбор прошёл; /work ← /var/lib/hft/retention-work
DELIVERY: PASS
exit=0
tmp.* 6464→6464; porcelain: []
=== V2 сломанный docker-compose.yml (умолчание убрано, как 086069b)
FAIL  D10 compose НЕ проходит прод-форму деплоя — деплой упадёт и откатится, как Deploy 37242446986:
      compose-deploy-form: разбор НЕ прошёл в окружении деплоя (rc=1):
        time="2026-10-05T11:04:59Z" level=warning msg="The \"RETENTION_WORK_DIR\" variable is not set. Defaulting to a blank string."
        invalid spec: :/work: empty section between colons
DELIVERY: FAIL (1)
exit=1
tmp.* 6464→6464; porcelain: [ M docker-compose.yml ]        ← только моя правка compose
=== V3 сломанный compose + RETENTION_WORK_DIR в окружении оператора + .env репозитория с ним же
FAIL  D10 compose НЕ проходит прод-форму деплоя — …
DELIVERY: FAIL (1)
exit=1
tmp.* 6464→6464; porcelain: []                               ← после отката compose и rm .env
=== V4 compose исправлен, python3 отказывает (шим exit 1) — R-241 Н-3
FAIL  D10 compose НЕ проходит прод-форму деплоя — …
      compose-deploy-form: источник /work у journal-retention = '<разбор JSON не удался>', ожидался /var/lib/hft/retention-work
DELIVERY: FAIL (1)
exit=1
tmp.* 6464→6464; porcelain: []; .d10.err: No such file or directory
=== V5 то же с мутантом M10 (без `|| src=…`)
      compose-deploy-form: источник /work у journal-retention = '', ожидался /var/lib/hft/retention-work
DELIVERY: FAIL (1)
exit=1                                   ← fail-closed и без ветки: `if d10_out=$(…)` снимает errexit
=== V6 docker недоступен, HFT_DELIVERY_DEEP=1
FAIL  D10 docker compose недоступен — разбор compose в прод-форме проверить нечем
DELIVERY: FAIL (2)
exit=1
```

Мусор библиотеки на пути отказа — мутант M11 (без `rm -f` на `:34`), замер в `TMPDIR`:

```
$ COMPOSE_DEPLOY_FORM_LIB=…/M11_no_rm_err_on_fail.sh bash scripts/tests/red_compose_deploy_form.sh | tail -1
VERDICT: PASS
tmp.* files: before=6459 after=6464 (+5)       ← пять fail-сценариев, пять файлов
$ bash scripts/tests/red_compose_deploy_form.sh | tail -1   # честная
VERDICT: PASS
before=6464 after=6464
```

Честная библиотека не мусорит; проба этого свойства не меряет — её «каталогов фикстур после
прогона: 0» считает только собственный `$TMP`. Это Н-4.

## 5. Прод-форма и CI-проводка

```
$ ssh … root@167.233.192.131 "cut -d= -f1 /root/hft-platform/.env"
GATEWAY_JWT_SECRET
GATEWAY_BANDS
$ grep -nE 'docker compose (up|config)' .github/workflows/deploy.yml
298:            if docker compose up -d --build recorder gateway-serve \
312:              docker compose up -d --build recorder gateway-serve
$ grep -nE 'RETENTION_WORK_DIR|GATEWAY_BANDS:|GATEWAY_JWT_SECRET:' docker-compose.yml
98:      - "${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}:/work"
173:      GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001}
211:      GATEWAY_JWT_SECRET: ${GATEWAY_JWT_SECRET:?GATEWAY_JWT_SECRET must be set}
356:      GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001}
$ git show origin/main:docker-compose.yml | grep -n 'RETENTION_WORK_DIR'
98:      - "${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}:/work"      ← правка 2dcea69 уже в main
$ sed -n '38,49p' .github/workflows/ci.yml      # джоб delivery: verify (DEEP=1) → проба D6b → проба D10
$ grep -n 'needs:' .github/workflows/ci.yml | tail -1
632:    needs: [build-test, security, delivery, …]
$ gh run list --branch harness/d10-deploy-form --limit 5
completed  success  fix(M-92): D10 — …  Branch build  harness/d10-deploy-form  push  37298524154
```

Форма вызова деплоя (`cd /root/hft-platform; docker compose up` без `-f`/`--env-file`/`--profile`)
и библиотека совпадают по осям, которые влияют на разбор; расхождение `--profile "*"` —
несущее для СВЕРКИ источника и безвредное для разбора (`R-241` §1/§2, C2′; здесь L4). Отметка: на
ветке исполнялся только `branch-build.yml`; `ci.yml` (и джоб `delivery` с пробой) запускается на
`pull_request`/`push main` — проба в CI ЕЩЁ НЕ бежала, первый прогон будет в PR. На раннере без
docker проба падает громко (`exit 1`, `:18-19`), а не скипается — верно для блокирующего джоба.

## 6. Полномочия и связность (§9 б/в)

- Пути диффа: `scripts/lib/**`, `scripts/tests/**`, `scripts/verify_*.sh`, `.github/workflows/ci.yml`
  — зона architect/харнесса (`scope-guard.md`, `gates.md` §9); замок §11 не тронут.
- Ссылки: шапка библиотеки → `R-241` Н-1/Н-4 (есть, §1/Н-4 того файла); проба → `R-241` Н-2;
  комментарий D10 `verify_delivery_M-08.sh:417` называет оба файла — существуют.
  `# shellcheck source=` относительные — корректны; `shellcheck` локально не установлен, в CI
  джоба нет — не проверялось ничем, не утверждаю.
- Н-5 `R-241` (спека `I-8` пишет `${RETENTION_WORK_DIR}:/work` буквально, `:46`) автор не правит
  («спека в архиве») — файл лежит в `milestones/`, не в `docs/archive/`; изложение, не блокер.

## Находки

**Н-1 — проба зелёная против стаба, не зовущего compose (§2).** `red_compose_deploy_form.sh`
не пиннит, что проверка идёт через `docker compose`: регулярочный стаб даёт 9/9 даже при docker,
подменённом на `exit 1`. Класс M-60 «`df` мимо PATH». Готовый сценарий: C10 — `PATH`-шим
`docker`, у которого `compose version` проходит, а `compose config` падает; честная проверка на
каталоге C1 ОБЯЗАНА вернуть ≠0 с «разбор НЕ прошёл» (под этим шимом она так и делает — показано
контролем в §2). Четыре строки.

**Н-2 — три параметра не пиннятся (L6/L9/L10).** `GATEWAY_BANDS` — фикстура даёт ему умолчание
(`:-0.1`, `:38`); в `proj()` сделать `:?` или добавить C7b. `want` — проба всегда передаёт один
`WANT`; один сценарий с другим ожиданием (умолчание `/tmp/other`, `WANT=/tmp/other` → pass)
закрывает зашитую константу. Адресация `journal-retention` — в `proj()` добавить сервис-приманку
ПЕРЕД `journal-retention` с томом `"/tmp/decoy:/work"`.

**Н-3 — причина отказа не пиннится (§3, L14).** `why` для C2/C4/C5/C6/C8 — префикс библиотеки;
мусорный YAML проходит за «подстановку без умолчания». Пиннить `empty section between colons`
(C2/C4/C5/C6/C8) и `variable is not set` — строки, которые библиотека уже печатает (`:33`);
стражи setup'а C2/C6 — грепом по `${…}` в фикстуре, как у C8.

**Н-4 — мусор библиотеки на пути отказа не меряется (§4, L13).** Проба считает только свой
`$TMP`; `rm -f "${err}"` на `:34` ничем не пиннится (+5 файлов за прогон у мутанта). Дёшево:
считать `tmp.*` в `${TMPDIR:-/tmp}` до/после в самой пробе — число, а не `[ -e "$TMP" ]`.

**Н-5 — `|| src="<разбор JSON не удался>"` (L12) — строка только сообщения.** Поведение без
неё то же (V5: источник `''`, FAIL, exit 1), сценария отказа `python3` в пробе нет. Либо
сценарий (шим `python3` → ожидать «разбор JSON не удался»), либо принять: fail-closed доказан на
уровне `verify_delivery` (V4/V5), в пробе — нет.

**Н-6 (состояние, не дефект) — проба в CI ещё не исполнялась** (§5): `ci.yml` триггерится
на PR/`main`; доказательство «джоб зелёный с пробой» появится в PR. `R-241` §3 замерил раннер
(`docker compose 2.38.2`, `python3 3.12.3`) — здесь не перепроверялось.

## Что проверено и держится

- Восемь несущих свойств библиотеки пиннятся своим сценарием (таблица §1, M1–M5, M7, M8, M12);
  агрегация в `expect` различает «отказ по разбору» и «отказ по сверке» (M5 → C3, M7 → C2… по причине).
- `R-241` Н-1: C6 + V-прогоны — override читается; Н-3: `.d10.err` нет ни на одном пути,
  `python3` отказ → строка FAIL + `DELIVERY: FAIL`; Н-2: проба есть и стоит в джобе агрегата.
- D10 на сломанном compose — та же строка, что в логе Deploy `37242446986`; маскировка окружением
  и `.env` репозитория не работает (V3).
- Честный прогон: 0.7 с, фикстур после — 0, `tmp.*` не прибавляется, `porcelain` пуст.

## Done Block

```
$ git -C /tmp/hft-adv-d10b rev-parse HEAD; git merge-base HEAD origin/main
c96948e7d61cf19cb79d1378926ab247089d0fe8
0f897d3b14730378156ca22e1b3015a2c2d425da
$ git diff --stat origin/main...HEAD
 .github/workflows/ci.yml                 |   2 +
 scripts/lib/compose_deploy_form.sh       |  49 ++++++++++++
 scripts/tests/red_compose_deploy_form.sh | 123 +++++++++++++++++++++++++++++++
 scripts/verify_delivery_M-08.sh          |  34 ++-------
$ docker compose version; python3 --version
Docker Compose version v2.39.1
Python 3.12.3
$ time bash scripts/tests/red_compose_deploy_form.sh; echo exit=$?
ok    C1 … ok    C9 (9 строк ok)
сценариев: 9, провалов: 0, каталогов фикстур после прогона: 0
VERDICT: PASS
real 0m0.717s
exit=0
$ bash scripts/verify_delivery_M-08.sh 2>&1 | grep -E 'D10|DELIVERY'; echo exit=${PIPESTATUS[0]}
PASS  D10 compose в окружении деплоя: compose-deploy-form: разбор прошёл; /work ← /var/lib/hft/retention-work
DELIVERY: PASS
exit=0
$ bash scripts/check_review_fa.sh 0f897d3b14730378156ca22e1b3015a2c2d425da HEAD | tail -1
SKIP (диапазон не трогает crates/**)
$ bash scripts/next_artifact_id.sh R
R-243
$ git status --porcelain      # после всех прогонов, до добавления этого файла
(пусто)
```

Мутанты, стабы, шимы и копия пробы — в scratchpad сессии, в репозиторий не попадают;
`docker-compose.yml` и `.env` в worktree возвращены (`porcelain` пуст).
