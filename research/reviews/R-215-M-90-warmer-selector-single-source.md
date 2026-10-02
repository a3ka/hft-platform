<!-- GATE-META
milestone: M-90
audited_repo: a3ka/hft-platform
audited_base: 1107c40b2abed7d3d716ab7cbb31cca55a9fadc5
audited_head: 85090b02a02209de74bd1986368203eb09d4646e
verdict: APPROVE
-->

# R-215 — PR-гейт M-90 «у селектора прогревателя ОДИН источник с сервером выдачи»

**Дата (UTC):** 2026-10-02
**Предмет:** ветка `origin/feat/M-90-warmer-selector-single-source`
**Вершина взята командой** (`04-workflow.md` §2, п. 2): `git fetch origin && git rev-parse …`
→ `85090b02a02209de74bd1986368203eb09d4646e`. SHA справки мандата — `85090b0` — СОВПАЛ с вершиной.
**База аудита:** `origin/main` = `1107c40b2abed7d3d716ab7cbb31cca55a9fadc5` на момент взятия вершины.
**Дрейф базы, напечатанный, а не умолчанный** (`04-workflow.md` §2 п.4): пока шёл гейт, `main`
ушла на `12db94ec6cc11df74af422cc52b7563948d8cb7b` (PR #295 `harness/ps-on-archive`). Вершина
ПРЕДМЕТА (`85090b0`) не двигалась — повторный `git fetch origin && git rev-parse` дал её же.
Движущуюся цель не пересуживаю; второй прогон гейта сделан на дереве слияния с НОВОЙ `main`.
**Расхождение с мандатом, названное сразу:** мандат говорит «ветка отстаёт от main на 34 коммита»;
замер — **24** (`git rev-list --left-right --count origin/main...origin/feat/M-90-…` → `36 24`).
Числа мандата устарели между его написанием и моим прогоном; предмет судится по замеру.

**Живой инвариант FA тронутого модуля (M-66):** `VB-I-11` (`docs/fa/viz-backend.md:294`) —
«значения провенанса ОДИНАКОВЫ на обоих путях (`snapshot` и `snapshot_from_checkpoint`);
бутстрап чекпоинта легален; отказ — только при разрыве чекпоинт↔журнал». Именно этот инвариант
`TD-227` обращал в ложь по РЕЗУЛЬТАТУ: путь `snapshot_from_checkpoint` на проде не достигался
вовсе, потому что сервер искал файл с другим `selector_fingerprint`, чем писал прогреватель.
Второй опорный — `OPS-I-8` (`docs/fa/ops.md:476`, «жив, но не работает» — класс этого отказа).
**Поправка к отчёту tester'а:** он назвал `GW-I-9`. Барьер `scripts/check_review_fa.sh:190-193`
для `crates/gateway` принимает ТОЛЬКО префикс `VB` (`PFX_OF[gateway]="VB"`), поэтому `GW-I-9`
барьер бы не удовлетворил; живой ID назван выше. FA-WAIVER не требуется.

---

## §0 — Что проверено и чем

| блок | исход |
|---|---|
| Block-scope | **PASS с одной названной оговоркой** (§1) |
| Block-DoneBlock | **PASS** — сырой stdout, exit-коды, прогон на ДЕРЕВЕ СЛИЯНИЯ (§4) |
| Block-C (contracts T1) | **N/A** — `crates/contracts/**` не тронут ни одним коммитом диапазона |
| Block-risk | **N/A по норме** — `crates/{risk,killswitch,oms,venue-*}` не тронуты; `gateway` — read-only консюмер журнала без order-egress (`gates.md` §5 MD-only класс). risk-critic по маршруту не требуется, и его отсутствие — не пробел |
| RED-first | **PASS** (§2) |
| Атомарность коммитов | **PASS** (§2) |
| Plan-time цепочка | **PASS** — `C-265` REJECT → `C-266` REJECT → `A-043` DECISION → `C-267` NOTE, все четыре артефакта в диапазоне |
| Паритет с CI на дереве слияния | §4 |

**Ярус C — чем грепал** (`reading-map.md` §2; `TECH-DEBT.md` + `PROJECT-STATE.md` = 1 017 КБ,
целиком не читаются): `TD-227`, `TD-228`, `TD-229`, `TD-230`, `TD-220`, `M-90`, `MS-STATE`,
`CHECKPOINT_BANDS`.

---

## §1 — Block-scope

Диапазон `1107c40..85090b0` (36 коммитов ветки), суммарный диф — 13 файлов, +2210/−34.

Зоны спеки §6 соблюдены: engine-dev тронул РОВНО `crates/gateway/src/bin/gateway-checkpoint.rs`,
`deploy/bin/gateway-checkpoint-cron.sh`, `docker-compose.yml`, `deploy/README.md`;
architect — спеку, `crates/gateway/tests/red_m90_*.rs`, `scripts/verify_M-90.sh`,
`scripts/tests/red_verify_M-90_ci_map.sh`. `*/tests/**` dev'ом НЕ тронут (проверено
по-коммитно `git show --numstat`), `crates/gateway-serve/**` и `crates/gateway/src/lib.rs`
не тронуты — как §6 и запрещает. Процессный слой (`CLAUDE.md`, `.claude/**`,
`docs/04-workflow.md`) не тронут ⇒ `docs-freeze` не применим.

**Оговорка (NOTE, не блокер) — С-1: последний коммит ветки `85090b0` вне зоны СПЕКИ, но
РАЗРЕШЁН ROADMAP'ом.** `docs(R-212): close-out M-89` правит `deploy/cron.d/watchdog` (+3/−2) и
комментарий сервиса **`gateway-serve`** в `docker-compose.yml` (+2/−1). Спека `M-90` §6 даёт
dev'у `docker-compose.yml` только «сервис `gateway-checkpoint`: `environment:`, `command:`», а
`deploy/cron.d/*` не даёт вовсе — то есть по спеке это выход за зону.
**Но разрешение существует и найдено не в коммите, а в источнике полномочий:**
`docs/ROADMAP.md:111` в `origin/main` (строка `M-89`, автор [architect], `65f2123`) говорит
прямо: «Остаток — перенос спеки и гейта в `docs/archive/` … после снятия двух ссылок чужой зоны
(`docker-compose.yml`, `deploy/cron.d/watchdog`), **их снимает engine-dev в поставке `M-90`**».
Это ровно то, что коммит сделал. Тело коммита при этом авторизует себя само («ПОПУТНО к M-90,
разрешено как часть этой поставки») и на ROADMAP не ссылается — проверять приходилось мне, хотя
одна ссылка в теле закрыла бы вопрос.
**Block-scope — PASS**, находка в другом: **две нормы о зоне M-90 расходятся**. Спека §6
(«Вне зоны: всё прочее») не знает о поручении, которое ROADMAP выдал позже. Расхождение
безвредно здесь (правка комментарная целиком — ни одной исполняемой строки, проверено дифом;
вносимое утверждение ПРАВДИВО: `red_m89_heartbeat_entrypoint::h0` существует на судимой
ревизии — `crates/gateway-serve/tests/red_m89_heartbeat_entrypoint.rs:439`), но класс
«поручение живёт в ROADMAP, запрет — в спеке» воспроизводим: следующий dev, прочитавший только
спеку, остановится со SCOPE VIOLATION REQUEST, а прочитавший только ROADMAP выйдет за зону без
следа. Фиксирую как NOTE, зона правки — architect (спека/ROADMAP).

---

## §2 — RED-first и атомарность

Порядок истории ветки (`git log --oneline`, снизу вверх) предъявляет RED-first буквально:
`d8916bb` спека → `1d5aec4` `w1`–`w3` (RED) → `74bda7b` гейт → … → `3706465` `w4a`–`w4e` →
`f9643f8` `w3`+каденция → и ТОЛЬКО потом три коммита engine-dev'а `12130c0` → `57e3410` →
`9883f78`. Реализация ни в одном коммите не идёт раньше или вместе со своим оракулом.

Атомарность: задача 1 → `12130c0`, задача 2 → `57e3410`, задача 3 → `9883f78`; одна задача —
один коммит, у каждого ссылка на `M-90` и на пункт спеки. Бандла нет. Метка роли в subject'е
стоит у всех.

**Мутационный контроль** — предъявлен architect'ом в спеке §8 и §12 (мутант «только полосы» ⇒
`w4a`–`w4d` FAILED при зелёных `w1`–`w3`; «каденция литералом» ⇒ `w4e` FAILED; «копия с верным
значением» ⇒ `w2`/`w3` FAILED). Я его не повторял: мутанты названы с конкретными исходами, а
набор на судимой ревизии зелен ровно теми же восемью оракулами.

---

## §3 — Проверка предмета ЗАМЕРОМ (а не чтением диффа)

Три проверки, каждая — исполнением, не грепом.

**(а) Единый источник — побайтовый паритет шести осей между службами.** Разбор
`docker-compose.yml` на дереве слияния: `gateway-serve` и `gateway-checkpoint` объявляют
`GATEWAY_{VENUE,SYMBOL,TIMEFRAME_MS,BANDS,WINDOW_MS,DEPTH_CADENCE_MS}` с ИДЕНТИЧНЫМИ
правыми частями (`${GATEWAY_BANDS:-0.001}` и т.д., все шесть совпали строка в строку). Это и
есть «один источник» — предмет милестоуна, и он выполнен.

**(б) `I-3` — отказ на `CHECKPOINT_*` прод-путём, с печатью argv.** Прогон самого скрипта:

```
$ HFT_CRON_PRINT_ARGV=1 CHECKPOINT_BANDS=0.001 bash deploy/bin/gateway-checkpoint-cron.sh
exit=1
ALERT CHECKPOINT_BANDS в окружении cron'а ЗАПРЕЩЕНА — собственный источник селектора запрещён
      (TD-227, M-90 I-3). Источник — GATEWAY_* из host .env через compose environment: …

$ HFT_CRON_PRINT_ARGV=1 bash deploy/bin/gateway-checkpoint-cron.sh
exit=0
--dir /journal --ckpt-dir /ckpt --coverage-out=/ckpt/covered_through_seq --cursor LATEST
```

Отказ срабатывает ВЫШЕ печати argv; `I-4` (контракт `HFT_CRON_PRINT_ARGV` M-48) сохранён, и
флагов селектора в argv нет ни одного. Напечатанный argv совпадает с тем, что `deploy/README.md`
§8b обещает оператору — документ не расходится с исполнением.

**(в) Приоритет источников у бинаря.** Прогон собранного прод-бинаря с env:

```
GATEWAY_BANDS=abc        → exit=1  gateway-checkpoint: GATEWAY_BANDS="abc" не парсится …
GATEWAY_TIMEFRAME_MS=abc → exit=1  gateway-checkpoint: GATEWAY_TIMEFRAME_MS="abc" не парсится …
GATEWAY_WINDOW_MS=xyz    → exit=1  gateway-checkpoint: GATEWAY_WINDOW_MS="xyz" не парсится …
GATEWAY_VENUE=Nope       → exit=1  gateway-checkpoint: unsupported venue `Nope` (…)
GATEWAY_TIMEFRAME_MS=777 → exit=2  … не выравнен на границу UTC-суток (GW-I-10)
```

Fail-closed держится: мусор в env НЕ уезжает в отпечаток, прогреватель не стартует.

### Находки §3 — обе NOTE, обе про ТОЧНОСТЬ УТВЕРЖДЕНИЙ, не про поведение

**Н-1 — ложная строка в module-doc: «exit 2» и «называя ПЕРЕМЕННУЮ».**
`crates/gateway/src/bin/gateway-checkpoint.rs:31-32` (новый текст M-90) утверждает:
«Невалидное значение env ⇒ **exit 2** с сообщением, называющим **ПЕРЕМЕННУЮ**». Замер (в)
показывает: четыре из пяти осей дают **exit 1** (ошибка разбора в `parse_args`), `exit 2`
даёт только выравнивание таймфрейма — отдельный гвард `GW-I-10`, существовавший до M-90.
И для `GATEWAY_VENUE` сообщение переменную НЕ называет («unsupported venue \`Nope\`»):
оператор не узнает из него, ГДЕ править. Тело коммита `12130c0` при этом говорит «exit 1» —
то есть документ противоречит и коду, и собственному коммиту.
**Почему NOTE:** поведение fail-closed и правильное, ни один оракул на числе 2 не стоит,
цена — операторское недоумение, а не отказ. Фикс — зона engine-dev (его файл), оформлю
карточкой долга, а не блоком: `gates.md` §4 запрещает мне ПРОЕКТИРОВАТЬ правку, и заворачивать
поставку, лечащую восьмисуточный прод-отказ, ради одной цифры в комментарии — хуже по существу.

**Н-2 — `I-3` не ловит переменную, заданную ПУСТОЙ.** Замер:

```
$ HFT_CRON_PRINT_ARGV=1 CHECKPOINT_BANDS= bash deploy/bin/gateway-checkpoint-cron.sh
exit=0   (argv напечатан, отказа нет)
```

Скрипт проверяет `[ -n "$val" ]`, поэтому `CHECKPOINT_BANDS=` в cron'е проходит молча, хотя
`I-3` спеки говорит «переменная селектора В ОКРУЖЕНИИ cron'а ⇒ exit ≠ 0». Оракул `w3` этого
мира не содержит — он варьирует непустые значения.
**Почему NOTE, а не находка класса `TD-227`:** вреда нет и быть не может — скрипт этих
переменных больше НЕ ЧИТАЕТ, второго источника пустая строка не создаёт; разойтись селекторам
от неё нечем. Это дыра в ПРЕДЪЯВЛЕНИИ («оператор не получил отказа»), а не в инварианте
поставки. Мир для `w3` — зона architect'а.

---

## §4 — Done Block: прогон на ДЕРЕВЕ СЛИЯНИЯ (ветка отстаёт на 24 коммита)

Ветка не свежее `main` (`strict: false`, `gates.md` §8), поэтому решающий прогон — не на ветке.
Дерево слияния построено явно: `git worktree add --detach /tmp/hft-reviewer-m90-merge 85090b0`
→ `git merge --no-edit origin/main` → merge чистый, без конфликтов, merge-коммит `ffec047`.

```
$ git log -1 --oneline
ffec047 Merge remote-tracking branch 'origin/main' into HEAD

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [7-RFC-PATH] путей-кандидатов …: всего=274 проверено=182 пропущено=92 — все существуют
VERDICT: PASS (0 нарушений)
claims_exit=0

$ PR_BASE_SHA=$(git merge-base origin/main HEAD) bash scripts/verify_M-90.sh
PASS  task1-3: red_m90_warmer_cron_composition (w1 прод-.env, w2 контроль, w3 отказ на CHECKPOINT_*)
PASS  task1: red_checkpoint_bin_prod_argv (прод-argv compose, c3ter) не сломан
PASS  task2: verify_M-48.sh (PRINT_ARGV, coverage-out == checkpoint-coverage ретеншена)
PASS  ci-map: проба карты CI-паритета (red_verify_M-90_ci_map.sh; число миров печатает проба)
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
PASS  ci-parity: cargo test --all
PASS  ci-parity: cargo audit
PASS  ci-parity: bash scripts/verify_delivery_M-08.sh
…  (48 исполнено, 6 исключено по карте, 1 SKIP review-fa — вердикта ещё нет)
FAIL  ci-parity: bash scripts/check_protected_artifacts.sh (exit=1)
FAIL  milestones/M-87-serving-circuit-breaker.md: артефакт ИСЧЕЗ с HEAD, и ни один коммит его не удалял
PASS  ci-parity: bash scripts/tests/red_deploy_catchup.sh
PASS  ci-parity: bash scripts/tests/red_branch_health.sh --battery
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
SKIP  task4: §8-гейт — снимает reviewer после деплоя
VERDICT: FAIL (провалов: 1)
verify_exit=1
```

### Единственный FAIL — артефакт МОЕЙ геометрии preview, а не дефект ветки. Установлено замером

Отчитываться «один FAIL, но он неважный» нельзя, поэтому причина доведена до конца.

`scripts/check_protected_artifacts.sh` сравнивает СОСТОЯНИЕ: путь, живший в **магистрали**
диапазона (база + цепочка **first-parent**) и отсутствующий на HEAD, — нарушение, если
удаливший коммит в диапазон не попал (`check_protected_artifacts.sh:152` — множество
`mainline`). Геометрия решает всё:

| кто строит merge | первый родитель | `milestones/M-87-…md` в магистрали? | исход барьера |
|---|---|---|---|
| я: `git merge origin/main` **из ветки** | ветка `85090b0` — она форкнута ДО архивного переезда и путь НАСЛЕДУЕТ | **да** | `exit=1` |
| GitHub (`refs/pull/N/merge`) и post-merge push | `main` — там путь уже переехал в `docs/archive/` коммитом `8d81b44` | нет | `exit=0` |

Проверено исполнением, а не рассуждением — второе дерево слияния построено в геометрии CI
(`git worktree add --detach … origin/main && git merge origin/feat/M-90-…`, родители
`41af97b: 12db94e 85090b0`):

```
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main)     bash scripts/check_protected_artifacts.sh
NOTE  milestones/M-87-serving-circuit-breaker.md: ALLOW-ARTIFACT-DELETE в 8d81b44
OK: защищённые артефакты целы на HEAD (12db94e..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0
```

Артефакт при этом НЕ потерян: `docs/archive/M-87-serving-circuit-breaker.md` на дереве
слияния есть (переезд `M-87` в архив — `8d81b44`, PR #288, норма `Р-2`), в `milestones/`
его нет НИ в `main`, НИ в дереве слияния. То есть барьер сработал на РЕНЕЙМ, объяснение
которому лежит в коммите базы, а не диапазона.

**Что это значит для решения:** первый прогон предъявлен целиком, включая красное, и
переинтерпретации не подлежит — но решающим для merge'а он не является: единственный его
FAIL первопарентно-зависим, и в геометрии, которой пользуется CI, тот же шаг зелен. Решение
принимается по коду возврата агрегата `All checks passed` на PR (`gates.md` §8), который
гоняет ТУ ЖЕ проверку в ТОЙ ЖЕ геометрии. Прочие 54 шага first-parent не зависят.

**Долг, который я из этого завожу (не фикс — описание):** норма `gates.md` §8 предписывает
«прогнать verify на дереве слияния», НЕ называя порядок родителей, — и у агента,
исполнившего её буквально, гейт краснеет на здоровой ветке. `TD-234`.

### Параллельное наблюдение, подтверждающее §5

В ЭТОМ прогоне `cargo test --all`, `red_deploy_catchup.sh` и `red_branch_health.sh --battery`
— **все три PASS** при той же конкуренции (чужие `cargo test` в `/tmp/hft-engine-m91` и
`/tmp/hft-engine-m92`). Ровно те три шага, что падали у tester'а в прогоне 1. Нестабильность
подтверждена независимым прогоном, а не принята на слово.

### Второй прогон — в геометрии CI, с текущей `main`

`main` ушла, пока шёл гейт (`1107c40` → `12db94e`, PR #295 `harness/ps-on-archive`):
`04-workflow.md` §2 п.4 — печатаю дрейф, а не пересуживаю. На дереве слияния с НОВОЙ `main`:
`verify_design_claims.sh --merge-preview origin/main` → `exit=0`; полный `verify_M-90.sh`
запущен и его исход дописывается в §8-дополнение этого файла вместе с пруфом деплоя.

---

## §5 — Поправка (а) мандата: три FAIL первого прогона tester'а — ПРОВЕРЕНО САМ

Tester дал PASS по третьему прогону, назвав первые два нестабильными. Проверено мной, а не
принято на слово:

1. **Предмет падений вне дифа M-90.** Упали `crates/journal/tests/red_tail_integrity_prodscale.rs`
   (`ti_7`), `crates/gateway/tests/red_egress_cap_boundary.rs`, `scripts/tests/red_deploy_catchup.sh`,
   `scripts/tests/red_branch_health.sh --battery`. Суммарный диф ветки (13 файлов, §1) не
   содержит ни `crates/journal/**`, ни этих скриптов, ни `red_egress_cap_boundary.rs`. Регрессия
   M-90 исключена по построению, а не по вероятности.
2. **Оба падения, названные мандатом, — на строке `:87` `.expect("append")`**, то есть на
   ЗАПИСИ фикстуры прод-масштаба, а не на проверяемом ассерте. Это отказ setup'а под ресурсным
   давлением, и оракулы честно падают, а не зеленеют молча.
3. **Конкуренция замерена, а не предположена.** В момент моего прогона на этой машине (16 ядер,
   `load average 5.35`) ЧУЖИЕ агенты гоняли тесты в своих деревьях: `cargo test --all` в
   `/tmp/hft-engine-m91` (с 14:58) и `cargo test -p journal` в `/tmp/hft-engine-m92` (с 15:12).
   Прод-масштабные оракулы журнала и пробы, поднимающие временные git-репозитории, к такой
   конкуренции чувствительны по построению.
4. **Диск: заявленное мандатом не воспроизвелось и выдаваться за проверенное не будет.** Мои
   замеры — 82–84 % занятого (`df -h /`: 340→347 ГБ из 437, 69–76 ГБ свободно). 95–99 % на
   момент прогона tester'а я подтвердить не могу — замера того момента нет. Что я вижу как
   причину накопления: **100 worktree** в `git worktree list` (`branch-hygiene.md` §Worktree
   lifecycle — кэши `target/` не шарятся). Уборку запускаю в close-out.

Вывод: PASS tester'а принимается, но НЕ по его аргументу «прогон 3 чистый», а потому что
предмет падений лежит вне дифа и причина давления замерена.

---

## §6 — Условие APPROVED

APPROVED **при** `VERDICT: PASS` гейта на дереве слияния (§4) и зелёном агрегате
`All checks passed` на PR. Решение принимается по КОДУ ВОЗВРАТА, не по тексту.
Задача 4 спеки (§8-гейт) снимается мной ПОСЛЕ merge и дописывается в этот файл отдельным
коммитом — до него `M-90` закрытым не считается.

## §7 — Что НЕ входит в эту поставку (чтобы не читалось как закрытое)

- `TD-228` — счётчики legacy-пути молчат; следующий предмет (`M-91`), не этот.
- `TD-229` — пер-сессионный каталог сегментов, ×3 построения на подписку; зона architect.
- `TD-230` — форма пруфа §8; в этом вердикте исполнена новая форма (по ДЖОБУ), карточка
  закрывается не здесь, а правкой нормы `gates.md` §8 (зона architect).
- Класс «ручная правка прод-конфига, стираемая деплоем» (спека §5 п. 3): `M-90` убирает
  ЕДИНСТВЕННУЮ известную величину, жившую так; сторожа на класс нет и он не изображается.
