<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: f4bf24da0eee46ba25b1d50f56d358d0ead94f87
verdict: DECISION
-->

# A-049 — M-94 calc-profile — DECISION: набор НЕ диспетчеризуется; шесть правок architect'у, затем critic круг 3

**Созыв:** `gates.md` §0 п.2 — третий круг по одному предмету после `C-280` (REJECT) и `C-281`
(REJECT). Арбитр — клон architect'а на Fable со СВЕЖИМ контекстом; ведущий architect в предмете
СТОРОНА. Предмет — ветка `origin/feat/M-94-calc-profile`; вершина взята командой
(`f4bf24d`, совпала со справкой мандата; повторный `fetch` перед записью — та же).
Работа — в отсоединённом дереве `/tmp/hft-arbiter-m94`; обманные реализации и эталоны —
во временном каталоге, в предмет не коммитятся.

**Живые инварианты тронутого предмета** (диапазон не трогает `crates/*/src/**`, барьер
`review-fa` в своём `SKIP`; называю когнитивно): `VB-I-2` (`docs/fa/viz-backend.md:285` —
одно определение у писателя и читателя слепка), `VB-I-11` (`:294` — провенанс истории),
`OPS-I-11` (`docs/fa/ops.md:479` — «жив, но не обслуживает» выдачи; `C-281` ссылался на
`OPS-I-8`, но тот — про тишину ПОТОКА РЫНОЧНЫХ ДАННЫХ, `ops.md:476` говорит это прямо).

## Решение коротко

| пункт мандата | решение |
|---|---|
| 1. `C-280` R1/R2/R3 и `C-281` B1 закрыты по существу? | **ДА, все четыре** — проверено исполнением (ниже) |
| 2. Что пробы ПРОПУСКАЮТ | найдено **пять дыр**, из них **четыре блокирующих** (М1, М2, М3 у пробы отката; Y2 у `p8`/`p8b`) и одна обязательная по чек-листу `testing.md` п.6 (Y1); плюс четыре NOTE |
| 3. Выполнимость набора | взаимоисключающих требований **нет**; один ПРОБЕЛ спеки (дом единственного разборщика окна heatmap / шага VP), который без правки даст `SCOPE VIOLATION REQUEST` на первом же дне dev'а |
| 4. Шестой провал `verify_M-94.sh` (S17b) | **зависит от хоста** — воспроизведено в обе стороны одной переменной `TMPDIR`; дефект пробы `red_runbook_markers.sh` (харнесс), не M-94; у критика `TMPDIR` лежал ВНУТРИ git-дерева |
| 5. Диспетчеризовать engine-dev? | **НЕТ.** Шесть правок architect'у (Р-1…Р-6), каждая с мутантом и ожидаемым исходом; после — critic круг 3 (Fable), скоуп — только этот перечень |

## 1. Закрытие находок прошлых кругов — исполнением

**`C-280` R1 (барьер, проба, джоб, sacred, сторож M-70) — закрыто.** Все артефакты на
вершине (`git diff --stat origin/main..HEAD`: `scripts/check_calc_profile.sh`,
`scripts/tests/red_calc_profile.sh`, `ci.yml` +28 строк — джоб `calc-profile` в `needs` и в
условии агрегата, `.claude/rules/scope-guard.md:32` — строка sacred, коммит `231914a` несёт
`FOUNDER-APPROVED: П-032 п.3`; `scripts/verify_M-70.sh` −100 строк). Проба барьера:
`сценариев: 30; провалов: 0`, 7/7 мутантов пойманы; `verify_M-94.sh` task9/task10 — PASS.

**`C-280` R2 (`p8` сравнивал текст путей) — закрыто `p8b`.** На кандидате (`/tmp/hft-m94-corpus`,
cron-обёртка с прогоном `next`) `p8`/`p8b`/`p9` зелены; контроль Y3 (`next` пишет в
`/ckpt/covered_through_seq`) — `p8` и `p8b` FAILED; контроль Y4 (псевдоним
`/ckpt/./covered_through_seq`) — `p8b` FAILED по `canonicalize`. Эффект на файле ретеншена
судится настоящим бинарём; путь ретеншена снимается с его скрипта (`HFT_CRON_PRINT_ARGV=1` →
`--checkpoint-coverage=/ckpt/covered_through_seq`, проверено прогоном).

**`C-280` R3 (`g7` судил порядок строк) — закрыто `red_m94_deploy_apply.sh`.** На МОЁМ эталоне
`deploy-apply.sh` (написан по §3.7 независимо, не по эталону architect'а) и фикстуре
`deploy.yml` в форме после задачи 6 — 9/9 PASS; `a1` пиннит `reset → build@PREV`, отсутствие
`up`, cron и профиль PREV. Мутант «`deploy.yml` без `15m`» — `a7` FAILED.

**`C-281` B1 (здоровье одним значением) — закрыто.** Заглушка отвечает по службе
(`red_m94_deploy_apply.sh:74-80`), журнал `inspect.log`; мутант критика (inspect только
`gateway-serve`, без логов, без prune) — `a2`, `a4`, `a4r`, `a4s` FAILED (4 из 9); мутант «откат
без cron из PREV» — `a4`/`a4r`/`a4s` FAILED; `a5` и `a7`(`15m`) есть.

**Базовая линия RED на вершине — красна ПО ПРЕДМЕТУ:** warmer 1 passed / 11 failed
(`c1`/`p1`/`p2` — нет `GATEWAY_CALC_PROFILE` в compose; `p3`/`p4`/`p5` — ключи/флаги/порча
приняты; `p6` — нет `.profile`; `p7` — флаг неизвестен; `p8`/`p8b` — один вызов runner'а; `p9` —
записан `ckpt-b0f1ed89ec2ec142.bin`, профиль проигнорирован); serve 0/3 (`s1` — композиция,
`s2`/`s3` — процесс ЖИВ: ключи и порча приняты); `o1` зелёный сторож; `g0`/`a0` — SETUP FAIL
«скрипта нет». Ни одного падения «по сборке» или по чужой причине.

## 2. Анти-плацебо — что пробы ПРОПУСКАЮТ (построено и прогнано мной)

Швы: `M94_APPLY_UNDER_TEST`/`M94_DEPLOY_YML_UNDER_TEST`, `M94_GATE_UNDER_TEST`, подмена
`BARRIER` в копии пробы барьера, правка cron-обёртки кандидата для `p8`/`p8b`. Каждый мутант
предъявлен диффом к эталону (SETUP-страж: мутант, не отличающийся от эталона, не засчитан —
один такой у меня был и переделан).

### Проба отката `red_m94_deploy_apply.sh` — три дыры БЛОКИРУЮЩИЕ

| ID | обманная реализация | проба | почему это дыра |
|---|---|---|---|
| **М1** | откат по здоровью: `reset PREV` → `up -d` **без `build`** | **PASS 9/9** | `compose up -d` без пересборки поднимает ТОТ ЖЕ образ по тегу `hft-platform-recorder:local` — то есть сломанный образ TARGET; откат — no-op при зелёном отчёте «up на PREV». Сегодняшний `deploy.yml:312` делает `up -d --build`; спека §3.7 строка 4 говорит «up на PREV» и обосновывает возврат ТЕГА только для отказа гейта (`:192-195`). `a4*` смотрят последний `up@`, но не `build@` (`:173-177`). Тот же класс, что `C-280` R3, перенесённый на ветку здоровья |
| **М2** | `wait_healthy`: любой статус **кроме `unhealthy`** считается здоровым | **PASS 9/9** | миры дают только `healthy`/`unhealthy` (`:186-188`); прод после `up -d` отвечает `starting` (`start_period: 10s`, `docker-compose.yml` healthcheck) — мутант объявляет здоровье до того, как оно известно, ставит сторожа и прунит образы; при последующем падении отката нет. Проба не меряет СВОЙ инвариант «ожидание healthy КАЖДОЙ службы» (`testing.md` «целостность гейта» св-2/3) |
| **М3** | cron ставится **без `crontab -n`** | **PASS 9/9** | спека §3.7 шаг 3 требует валидацию «(с `crontab -n`)», контракт M-48 B3 сегодня в `deploy.yml:276-296` fail-closed; проба подставляет `DEPLOY_CRON_VALIDATE=true` (`:105`) и ни одним миром не даёт невалидного cron-файла — перенос тела в скрипт теряет контракт молча |
| М4 | `prune` ДО `up` | PASS | NOTE: `a2` проверяет наличие `prune@`, не порядок (`:142`) |
| М5 | сторож ставится сразу после `up`, ДО ожидания здоровья | PASS | NOTE: `seq` регэксп `:138` не различает `up→watchdog→wait` и `up→wait→watchdog`; на откате `$WD \|\| true` переустанавливает — ущерб мал |
| М6 (контроль) | откат без cron из PREV | FAILED ×3 | проба пиннит — хорошо |
| М7 (мутант `C-281`) | inspect только `gateway-serve`, без логов, без prune | FAILED ×4 | B1 закрыт |

### Проба гейта `red_m94_deploy_gate.sh` — дыр блокирующих нет

Эталон 31/31. Ga (ключ подстрокой) — `g3c` FAILED; Gd (`.env` не читается) — 25 FAILED.
Пропущены два низких: **Gb** форма имени `ckpt-*.bin` без 16 hex — PASS (бинарь печатает hex
сам, ущерб гипотетический); **Gc** `-e` вместо `-f` (каталог сойдёт за слепок) — PASS. NOTE.

### Проба барьера `red_calc_profile.sh` — дыр блокирующих нет; две NOTE для харнесс-трека

Барьер сам ПРАВ в обоих случаях (проверено на мирах, построенных руками): **X1** —
merge-коммит, чей `--cc`-дифф меняет `active.env` (разрешение конфликта): барьер FAIL, мутант
с `--no-merges` PASS, а все 18 миров пробы его не роняют; **X2** — заголовок `## П-040`
добавлен коммитом ПОЗЖЕ коммита с токеном: барьер FAIL, мутант «`$PS` на `HEAD`, а не на `$c`»
PASS и пробой не ловится (батарея `no_p_check` ловит отсутствие проверки, не её ревизию).
Оба — цена низкая, дом — харнесс-трек, не блокер M-94.

### `p8`/`p8b` на кандидате — одна дыра БЛОКИРУЮЩАЯ (композиция) и одна обязательная по чек-листу

| ID | мутация cron-обёртки | `p8` | `p8b` | почему |
|---|---|---|---|---|
| **Y2** | прогон `next` с `--ckpt-dir /ckpt/next` | PASS | PASS | **композиция с гейтом не пиннится:** гейт ищет `$CALC_GATE_CKPT_HOST_DIR/<имя от --print-ckpt-name>` в КОРНЕ тома (`red_m94_deploy_gate.sh:34,105-112`), а слепок `next` лежит в подкаталоге — PR «переключение» будет отвергаться ВЕЧНО; fail-closed, но тупик, видимый только на втором PR. `p9` не помогает — зовёт бинарь напрямую с одним `--ckpt-dir`. `testing.md` «канарейка» п.2: путь producer'а обязан совпасть с путём consumer'а |
| **Y1** | покрытие `next` в `/journal/covered_through_seq.next` | PASS | PASS | у `gateway-checkpoint` том `journal-data:/journal:ro` (`docker-compose.yml`); `compose_volumes` (`red_m94_calc_profile_warmer.rs:1166-1176`) отбрасывает режим, `map_container_path` даёт записываемый каталог. На проде прогон `next` падал бы EROFS каждый цикл. `testing.md` чек-лист п.6 «права доступа к носителю» — пункт BINDING: «не покрыт хотя бы один пункт — набор не готов» |
| Y3 (контроль) | `next` → путь ретеншена | FAILED | FAILED | R2 закрыт |
| Y4 (контроль) | `/ckpt/./covered_through_seq` | — | FAILED | нормализация работает |

Названный обеими сторонами предел (симлинк-псевдоним; побочные действия самой обёртки сверх
вызовов runner'а) — остаётся названным пределом, не правкой: `C-281` его принял примечанием,
спека §5 запрещает обёртке побочные действия.

## 3. Выполнимость — взаимоисключающих требований нет; один пробел спеки

Проверено исполнением на черновом кандидате: `p8`/`p8b`/`p9` — 3/3 PASS;
адаптированные `red_m90_warmer_cron_composition` и `red_checkpoint_bin_prod_argv` — 8/8 и 8/8
на кандидате (утверждение §9.3 «зелены после» подтверждено для двух из пяти файлов — для тех,
что собираются в крейте `gateway`; остальные три — утверждение architect'а, мной не
перепроверялись). Разобранные сомнения:

- **`g1` зовёт `gateway-checkpoint --print-ckpt-name` ГОЛЫМ** (ARGS непусты ⇒ `command:`
  заменён целиком, `--dir` нет), а `p7` даёт `--dir`/`--ckpt-dir`. Противоречия нет:
  `parse_args` даёт дефолты (`gateway-checkpoint.rs:251-255`: `./journal-data`,
  `./gateway-ckpt`), значит голый флаг разбирается; `p7` не моделирует голый argv, но и не
  требует невозможного. NOTE: `p7` может добавить голую форму одним вызовом — дёшево.
- **`p3` требует отказа прогревателя на `GATEWAY_CANONICAL_BANDS`/`ALLOWED_PROFILES`/
  `VP`/`HEATMAP` в env** — величины, которые он не читал. Требование — о НАЛИЧИИ (`П-032` п.1),
  реализуемо тем же списком ключей; кандидат так и делает.
- **`p5` «полосы-мусор» требует имени `GATEWAY_BANDS` в stderr** — есть путь с этим именем
  (`gateway-checkpoint.rs:228`), флаговый путь `:222` пишет `--bands parse` — dev обязан
  подавать значение профиля через именованный путь. Выполнимо.
- **ПРОБЕЛ (Р-6).** `p5` «окно heatmap 1.5» и «шаг VP 0» требуют от ПРОГРЕВАТЕЛЯ отказа с
  именем ключа, а единственные разборщики этих величин живут в `gateway-serve`
  (`crates/gateway-serve/src/lib.rs:2980-2994`, `:3247-3276`); крейт `gateway` зависеть от
  `gateway-serve` не может. §3.2 говорит «проверку ЗНАЧЕНИЙ загрузчик не дублирует — её делают
  существующие разборщики», §5 запрещает «дублировать разбор значений в загрузчике», а
  Allowed paths §10 дают dev'у в `crates/gateway/src/lib.rs` только `pub mod`. Кандидат
  architect'а обошёл это `parse().unwrap()` без валидации — ровно поэтому `p5` на нём красен.
  Честный dev остановится со `SCOPE VIOLATION REQUEST`. Это не противоречие, а неназванный
  дом: спека обязана сказать, что ЕДИНСТВЕННЫЙ разборщик окна heatmap / шага VP / связки
  «тройка ∈ `ALLOWED_PROFILES`» переезжает в крейт `gateway` (`calc_profile.rs` — путь уже
  разрешён), а `serve_config_from_env`/`admission_policy_from_env` его ЗОВУТ (правка
  `gateway-serve/src/lib.rs` — тоже разрешена). Один разборщик, два вызывателя.

## 4. Шестой провал `verify_M-94.sh` — факт установлен

```
$ echo "TMPDIR=${TMPDIR}"                                         → /home/nous/.cache/paxio-tmp
$ bash scripts/tests/red_runbook_markers.sh | grep -E 'S17b|VERDICT'   (вершина, cwd /tmp/hft-arbiter-m94)
ok    S17b корень — не рабочее дерево git (rc=1)
VERDICT: PASS                                                      exit=0
$ TMPDIR=/tmp/hft-arbiter-m94/.probe-tmp bash scripts/tests/red_runbook_markers.sh | grep -E 'S17b|VERDICT'
FAIL  S17b SETUP НЕ СОСТОЯЛСЯ: каталог внутри git
FAIL  S17b корень — не рабочее дерево git — отказ есть, но не по причине «не рабочее дерево git»
VERDICT: FAIL                                                      exit=1
$ (cd /tmp/hft-m94-clean && bash scripts/tests/red_runbook_markers.sh | grep -E 'S17b|VERDICT')   (origin/main 045fef9)
ok    S17b корень — не рабочее дерево git (rc=1)
VERDICT: PASS                                                      exit=0
$ ls -d /tmp/hft-codex-critic-1791223575/tmp                       → существует (045fef9)
```

Проба `red_runbook_markers.sh:19` берёт `TMP=$(mktemp -d)`; S17b (`:231-234`) требует, чтобы
этот каталог НЕ лежал в git-дереве. Обёртка критика (codex) держит `TMPDIR=<дерево>/tmp` —
каталог внутри клона ⇒ setup-страж S17b краснеет на ЛЮБОЙ ревизии, включая `origin/main`.
**Исход зависит от хоста; дефект — пробы `red_runbook_markers.sh`** (фикстура опирается на
окружение, `testing.md` «целостность гейта» св-2), **не M-94**. Утверждение `C-281` «already
red at `045fef9` (`origin/main`) — pre-existing CI baseline defect» неточно: CI на `main`
зелен (`gh run list --branch main`: `Merge pull request #327 … CI success`); красен хост критика.
Запись `M-94` §16 описывает это верно. Лечение (харнесс-трек, не этот милестоун):
`GIT_CEILING_DIRECTORIES="$TMP"` для S17b либо явный страж «`TMPDIR` внутри git ⇒ SKIP с
причиной». Кандидат в долг — reviewer'у.

На моём хосте (`TMPDIR` вне git) `verify_M-94.sh` на вершине даёт **ровно пять провалов** —
warmer, serve, `g0`, `a0`, `cargo test --all` (те же три RED-набора внутри), — `PASS=62`,
`SKIP=8`, `red_runbook_markers.sh` зелен (строка 61 из 61 шагов паритета учтена). Число
architect'а (5) и число критика (6) оба верны для своих хостов; расхождение объяснено.

## 5. Решение и перечень правок architect'у (обязательны к исполнению обеими сторонами)

**Диспетчеризация engine-dev — НЕТ** до приземления Р-1…Р-6 на ветке. Каждая правка —
оракул + мутант + ожидаемый исход; мутанты из §2 воспроизводимы по диффам, приведённым там.

| № | правка | где | ожидаемый исход |
|---|---|---|---|
| **Р-1** | откат по здоровью возвращает ТЕГ образа: `a4`/`a4r`/`a4s` требуют последний `build@PREV` раньше последнего `up@PREV`; спека §3.7 строка 4 — «reset к PREV, cron из PREV, **пересборка на PREV**, up на PREV» | `red_m94_deploy_apply.sh:168-185`; `milestones/M-94-calc-profile.md:187` | мутант М1 → `a4`,`a4r`,`a4s` FAILED; эталон 9/9 |
| **Р-2** | здоровье — состояние, а не константа: заглушка `inspect` отвечает по ПОСЛЕДОВАТЕЛЬНОСТИ (`starting`×k → `healthy`; `starting` навсегда; контейнера нет — `inspect` exit≠0). Миры: (а) `starting→healthy` обеих ⇒ exit 0, `inspect` обеих ≥ k+1 раз; (б) `starting` навсегда у одной ⇒ откат по истечении `DEPLOY_HEALTH_TIMEOUT`; (в) `inspect` падает ⇒ откат | `red_m94_deploy_apply.sh:63-86` (заглушка), новые миры | мутант М2 → (а) или (б) FAILED; эталон зелен |
| **Р-3** | валидация cron до установки: мир `a6` — `DEPLOY_CRON_VALIDATE` = заглушка, отказывающая на файле-маркере в TARGET ⇒ exit≠0, `up` не было, cron хоста не тронут (mtime) | `red_m94_deploy_apply.sh` (новый мир), §8 таблица `a*` | мутант М3 → `a6` FAILED |
| **Р-4** | композиция прогрева с гейтом: `p8b` (или `p8`) утверждает, что `--ckpt-dir` прогона `next` после нормализации РАВЕН `--ckpt-dir` прогона `active`, и что после прогона `next` в этом каталоге лежит `ckpt-<fp>.bin` селектора `next` (имя — `ckpt_path_for_pub` по селектору, разобранному из `next.env` независимо) | `red_m94_calc_profile_warmer.rs:1322-1416`; спека §3.5/§8 строка `p8b` | мутант Y2 → `p8b` FAILED |
| **Р-5** | режим монтирования: `compose_volumes` возвращает режим; тома `:ro` в фикстуре делаются недоступными для записи (`chmod a-w`) ИЛИ `p8b` утверждает, что `--coverage-out`/`--ckpt-dir` прогона `next` лежат на RW-томе `gateway-checkpoint` | `red_m94_calc_profile_warmer.rs:1166-1208` | мутант Y1 → `p8b` FAILED (на `chmod`-варианте — прогон `next` упадёт SETUP-стражем «прогон next упал», что и требуется) |
| **Р-6** | текст спеки: §3.2 и §10 называют ДОМ единственного разборщика окна heatmap / шага VP / связки «тройка ∈ `ALLOWED_PROFILES`» — крейт `gateway` (`calc_profile.rs`), `gateway-serve` его зовёт; §5 — строка «дублировать разбор» уточняется «второй разбор = второе правило; единственный живёт в `gateway`» | `milestones/M-94-calc-profile.md:78-87, 240, 450-457` | без оракула — это снятие неопределённости, не поведение; `p5`/`s3` остаются оракулами |

NOTE (по желанию, не условие): М4, М5 (порядок `prune`/сторожа), Gb/Gc (форма имени, `-f`),
`p7` голой формой, X1/X2 — в харнесс-трек по `red_calc_profile.sh`; S17b — долг reviewer'у.

**Маршрут после правок:** правятся нормативные секции спеки (§3.7 таблица, §8) ⇒ critic
обязателен (`gates.md` §9; профиль architect'а: «правка нормативных секций… всегда критик»).
Круг 3 — critic на Fable со свежим контекстом, **скоуп — только перечень Р-1…Р-6 и
мутационные свидетельства к ним**; закрытие `C-280`/`C-281` критик вправе принять по этому
файлу, не повторяя. Вердикт NOTE/APPROVE ⇒ engine-dev по задачам 1–6.

**Founder'у — ничего нового.** Состав значений не меняется (`П-014` п.4, `П-029`), носитель
подписан `П-032`, отсрочка п.4 (в) — названная. Вопросов границы C в этом споре нет.

## Done Block

```text
$ git -C /home/nous/hft-platform fetch origin && git rev-parse origin/feat/M-94-calc-profile
f4bf24da0eee46ba25b1d50f56d358d0ead94f87
$ git merge-base origin/main origin/feat/M-94-calc-profile
045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
$ git worktree add --detach /tmp/hft-arbiter-m94 origin/feat/M-94-calc-profile
HEAD is now at f4bf24d docs(M-94): по C-281 — контракт отката по двум службам, мутанты, предел p8b, журнал круга 2 [architect]
$ bash scripts/check_branch_health.sh | tail -1
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)

$ cargo test -p gateway --test red_m94_calc_profile_warmer
test result: FAILED. 1 passed; 11 failed; 0 ignored          exit=101
$ cargo test -p gateway-serve --test red_m94_calc_profile_serve
test result: FAILED. 0 passed; 3 failed; 0 ignored           exit=101
$ cargo test -p ops --test red_m94_heartbeat_profile_compat
test result: ok. 1 passed; 0 failed                           exit=0
$ bash scripts/tests/red_calc_profile.sh | tail -2
сценариев: 30; провалов: 0; каталогов песочницы до уборки: 145 (убираются trap EXIT)
VERDICT: PASS                                                 exit=0
$ bash scripts/tests/red_m94_deploy_apply.sh | tail -2
FAIL  a0 SETUP: /tmp/hft-arbiter-m94/deploy/bin/deploy-apply.sh нет — …
VERDICT: FAIL (pass=0 fail=1; миры не исполнялись — судить нечего)   exit=1
$ bash scripts/tests/red_m94_deploy_gate.sh | tail -2
FAIL  g0 SETUP: гейта /tmp/hft-arbiter-m94/deploy/bin/calc-profile-gate.sh нет — …
VERDICT: FAIL (pass=0 fail=1; остальные миры не исполнялись — судить нечего)   exit=1

# позитивный контроль — ЭТАЛОНЫ АРБИТРА (не architect'а), фикстура deploy.yml в форме после задачи 6
$ M94_APPLY_UNDER_TEST=$A/ref-apply.sh M94_DEPLOY_YML_UNDER_TEST=$A/deploy.fixture.yml bash scripts/tests/red_m94_deploy_apply.sh | tail -2
сценариев: 9 (pass=9 fail=0)
VERDICT: PASS                                                 exit=0
$ M94_GATE_UNDER_TEST=$A/ref-gate.sh bash scripts/tests/red_m94_deploy_gate.sh | tail -2
сценариев: 31 (pass=31 fail=0)
VERDICT: PASS                                                 exit=0

# мутанты пробы отката (дифф к эталону — одна строка каждый)
M1  (откат по здоровью без build)        → сценариев: 9 (pass=9 fail=0)  VERDICT: PASS   ← ДЫРА
M2  ([ "$st" != unhealthy ] && return 0) → сценариев: 9 (pass=9 fail=0)  VERDICT: PASS   ← ДЫРА
M3  (cron без $VALIDATE)                 → сценариев: 9 (pass=9 fail=0)  VERDICT: PASS   ← ДЫРА
M4  (prune до up)                        → сценариев: 9 (pass=9 fail=0)  VERDICT: PASS   ← NOTE
M5  (сторож до ожидания здоровья)        → сценариев: 9 (pass=9 fail=0)  VERDICT: PASS   ← NOTE
M6  (откат без cron из PREV — контроль)  → a4,a4r,a4s FAILED             VERDICT: FAIL   ← пиннится
M7  (мутант C-281: один inspect, без логов/prune) → a2,a4,a4r,a4s FAILED  VERDICT: FAIL   ← B1 закрыт
M8  (deploy.yml command_timeout: 10m)    → a7 FAILED                     VERDICT: FAIL   ← пиннится

# мутанты гейта
Ga  (ключ подстрокой)  → g3c FAILED (pass=30 fail=1)      Gb (ckpt-*.bin без hex) → PASS 31/31 ← NOTE
Gc  (-e вместо -f)     → PASS 31/31 ← NOTE                Gd (.env не читается)   → 25 FAILED ← пиннится

# барьер профиля — миры, которых нет в пробе (барьер прав, проба слепа к мутанту)
W1 merge-коммит, --cc трогает active.env:  check_calc_profile.sh → VERDICT: FAIL (нарушений: 1) exit=1
                                           мутант --no-merges   → VERDICT: PASS exit=0
W2 заголовок П-040 добавлен ПОЗЖЕ токена:  check_calc_profile.sh → VERDICT: FAIL (нарушений: 1) exit=1
                                           мутант «$PS на HEAD»  → VERDICT: PASS exit=0

# p8/p8b на кандидате /tmp/hft-m94-corpus (оракул синхронизирован с вершиной, cron-обёртка кандидата)
базовая линия кандидата: p8 ok, p8b ok, p9 ok  (3 passed)
Y1  (--coverage-out=/journal/covered_through_seq.next, том :ro)   → p8 ok, p8b ok      ← ДЫРА (testing.md п.6)
Y2  (--ckpt-dir /ckpt/next у прогона next)                       → p8 ok, p8b ok      ← ДЫРА (композиция с гейтом)
Y3  (next → /ckpt/covered_through_seq — контроль)                 → p8 FAILED, p8b FAILED
Y4  (/ckpt/./covered_through_seq — контроль)                      → p8b FAILED («ОДИН файл после разрешения путей»)
cmp cron-обёртки с оригиналом после каждого мутанта → restored

$ (cd /tmp/hft-m94-corpus && cargo test -q -p gateway --test red_m90_warmer_cron_composition --test red_checkpoint_bin_prod_argv)
test result: ok. 8 passed; 0 failed
test result: ok. 8 passed; 0 failed                           exit=0

$ bash scripts/verify_M-94.sh   (вершина, мой хост, TMPDIR=/home/nous/.cache/paxio-tmp)
FAIL  task1-2,4-5: red_m94_calc_profile_warmer (c1 c2 p1…p9, p8b) (exit=101)
FAIL  task1,3-4: red_m94_calc_profile_serve (s1 s2 s3) (exit=101)
FAIL  task6: red_m94_deploy_gate.sh (g0…g6) (exit=1)
FAIL  task6: red_m94_deploy_apply.sh (a0…a4, a7 — откат исполнением, C-280 R3) (exit=1)
FAIL  ci-parity: cargo test --all (exit=101)
PASS  ci-parity: учтено шагов 61 из 61 (исполнено 54, исключено по карте 6)
VERDICT: FAIL (провалов: 5)                                   exit=1      (PASS=62, SKIP=8; red_runbook_markers.sh — PASS)

$ gh run list --branch main --limit 3
completed  success  Deploy to VPS                              main  workflow_run  2026-10-05T12:26:32Z
completed  success  Merge pull request #327 … CI               main  push          2026-10-05T12:10:13Z
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile   (перед записью)
f4bf24da0eee46ba25b1d50f56d358d0ead94f87   (совпадает с audited_head)
```
