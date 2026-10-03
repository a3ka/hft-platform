<!-- GATE-META
milestone: M-69
audited_repo: a3ka/hft-platform
audited_base: 10bc072c7e008bce3feee80013fb187e3436fd17
audited_head: 00886e3b745ec7813c2114143137f18a1e420954
verdict: APPROVE
-->

# R-128 — M-69 window guard (`GW-I-14`): PR-time reviewer, **APPROVED**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL).
**Предмет:** `10bc072..00886e3` на `origin/feat/M-69-window-guard`, PR #32.
**Дерево слияния:** `git merge-tree --write-tree origin/main origin/feat/M-69-window-guard`
→ `2447e82` без конфликтов; проверочный merge-коммит `85866fc`
(`origin/main 0536505` + ветка `00886e3`).

**Живой инвариант тронутого модуля (M-66, `gates.md` §4).** Диф трогает
`crates/gateway/src/**` и `crates/gateway-serve/src/**`. Собственной FA у обоих крейтов
НЕТ — это уже названный долг (`docs/workflow/reading-map.md` §2, строка
«derive / recorder / gateway-serve»), и барьер `check_review_fa.sh` мапит оба на
`docs/fa/viz-backend.md` (префиксы `VB` и `GS`). Живой ID, задевающий предмет:
**`VB-I-10`** (`docs/fa/viz-backend.md:158`) — «память `snapshot`/`frames_since`
ограничена ОКНОМ `[at−W, at]` (`Selector.window_ms=Some(W)`) … `window_ms=None` —
offline-режим (полная свёртка)». Именно эта строка делает `None` РЕЖИМОМ, а не пустотой,
и потому превращает молчаливый `parse::<i64>().ok()` в вход в unbounded. `FA-WAIVER`
не требуется: тронутые крейты не входят в NO-FA-список барьера (`recorder`, `derive`).

**Ярус C — что искал грепом** (`reading-map.md` §2; «прочитал TECH-DEBT» писать нельзя):
`TECH-DEBT.md` по `GATEWAY_WINDOW_MS`, `R7`, `TD-020`, `TD-039`, `TD-091`, `TD-162`;
`PROJECT-STATE.md` по `M-69` и `GW-I-14` (оба — ноль попаданий, милестоун ещё не
приземлён).

---

## 1. Block-scope — PASS

Диапазон `10bc072..00886e3`, 13 файлов / +1835 −14. Сверка с §Allowed paths спеки:

| файл | автор по диапазону | Allowed paths говорит | вердикт |
|---|---|---|---|
| `crates/gateway-serve/src/lib.rs` | engine-dev (`dc82dad`, `5e25042`, `37143ec`) | engine-dev, задачи #1/#2/#4 | ✅ |
| `crates/gateway/src/lib.rs` | engine-dev (`2e891e1`) | engine-dev, задача #3 | ✅ |
| `crates/gateway-serve/tests/red_window_guard_startup.rs` | architect (`a073f8a`) | architect-only, sacred | ✅ |
| `crates/gateway/tests/red_window_selector_guard.rs` | architect (`a073f8a`) | architect-only, sacred | ✅ |
| `scripts/verify_M-69.sh` | architect (`e555cb4`) | architect-only, sacred | ✅ |
| `docs/plans/gateway-ws-contract.md` | architect (`10753df`, `f3102f0`, `00886e3`) | architect, задача #5 | ✅ |
| `docs/DESIGN.md` | architect (`f3102f0`) | architect-only, §22 счётчик (`A-014` B-6) | ✅ |
| `milestones/M-69-window-guard.md` | architect + dev-статус (`7ad7acb`) | architect-only; dev — ТОЛЬКО колонка Status | ✅ |
| `research/critiques/C-099,C-100,C-104,C-132` | critic | зона critic | ✅ |
| `research/arbitration/A-014-m69-window-guard.md` | арбитр | зона арбитра (`gates.md` §0) | ✅ |

**Forbidden paths — предъявлено отсутствием, а не молчанием:**

```
$ git diff --name-only 10bc072..00886e3 -- crates/contracts docker-compose.yml \
    crates/gateway/src/bin/gateway-checkpoint.rs '*/Cargo.toml' Cargo.lock
(пусто)
```

Carve-out dev'а по milestone-файлу соблюдён буквально: `7ad7acb` = 4 добавления / 4
удаления, все — `⏳ OPEN → ✅ DONE` в §Tasks; ни одной правки Objective / Allowed paths /
Acceptance.

## 2. Sacred RED не переписан под реализацию — PASS

Требование профиля (`.claude/agents/reviewer.md` §5) проверено ПОКОММИТНО, а не по
итоговому дифу — реверт-пара внутри диапазона так тоже ловится:

```
$ git log --format='%h %s' 10bc072..00886e3 -- '*/tests/*'
a073f8a test(M-69): RED-набор GW-I-14 — две точки гварда, честный RED [architect]
```

Единственный коммит, тронувший `*/tests/**`, — architect'а, и он ПРЕДШЕСТВУЕТ всем
четырём коммитам реализации (`2e891e1`, `dc82dad`, `5e25042`, `37143ec`). RED-first
соблюдён по порядку, а не по декларации.

**Оракул не вакуумен — парный vantage на месте.** `red_window_guard_startup.rs` держит
девять отказных кейсов (`abc`, `60000ms`, `60_000`, `6e4`, `60000.0`, два переполнения,
`-60000`, `-1`) И пять принимающих (`unset` / `""` / пробелы / `"0"` → `None`;
`60000` → `Some(60_000)`; padded `" 60000 "`; плюс контроль, что гвард `GW-I-10` не
задет). Заглушка «всегда `Err`» валится на второй половине — то есть оракул не
удовлетворяется переширокой реализацией, а не только узкой.
`red_window_selector_guard.rs` бьёт по ТРЁМ публичным входам (`snapshot`,
`frames_since`, `replay`) плюс прямой `validate_selector` — то есть проверяет
ОТСУТСТВИЕ байпас-поверхности, а не наличие проверки в одной точке.

## 3. Block-C (контракты) — неприменим, PASS

```
$ git diff --name-only 10bc072..00886e3 -- crates/contracts contracts
(пусто)
```

`Selector` — T2 крейта `gateway`; его ФОРМА не менялась (правится допустимое МНОЖЕСТВО
значений существующего поля `window_ms`), wire-схема `GATEWAY_SCHEMA_VERSION` не
бампалась. contract-RFC не требуется (`05-contract-layer.md` §4).

## 4. Block-risk — RISK-BLOCK неприменим, PASS

`gates.md` §5 привязан к `crates/{risk,killswitch,oms,venue-*}/**` и `crates/contracts/**`
— диапазон не трогает ни одного. Зона предмета — read-path (`gateway`, `gateway-serve`):
консюмер журнала без order-egress (`scope-guard.md`: `VB-I-3`, `GS-I-1`). Подтверждено
отсутствием: в дифе нет submit/cancel/подписи торговых действий. risk-critic не требуется
и его отсутствие блокером НЕ является.

## 5. Атомарность коммитов — PASS

Одна задача = один коммит, каждый несёт `M-69` и номер задачи:

```
dc82dad  fix(M-69): task #1 — GATEWAY_WINDOW_MS parse-error/overflow fail-closed на старте
5e25042  fix(M-69): task #2 — GATEWAY_WINDOW_MS negative → Err на старте
2e891e1  fix(M-69): task #3 — validate_selector rejects negative window_ms
37143ec  fix(M-69): task #4 — док-комментарий serve_config_from_env приведён к факту
```

Бандл-коммита на несколько задач нет; `Co-Authored-By` в телах нет.

## 6. Гейт-цепочка предъявлена ФАЙЛАМИ, а не словами (`gates.md` §4)

| гейт | артефакт | вердикт |
|---|---|---|
| plan-time critic, круги 1-3 | `C-099`, `C-100`, `C-104` | REJECT ×3 |
| арбитраж (`gates.md` §0 п.2) | `A-014-m69-window-guard.md` | DECISION |
| plan-time critic, круг 4 (ограниченный, сильная модель) | `C-132-M-69-window-guard-r4.md` | **NOTE — dev may proceed** |
| tester | Done Block в handoff (13 PASS / 0 FAIL, 845/0) | PASS |
| PR-time reviewer | **этот файл** | APPROVED |

**Перепроверка `gates.md` §9 закрыта, отдельного круга не требуется.** Диф трогает
`docs/DESIGN.md` — зону §9. `A-014` §5 п.2 предписал круг 4 на СИЛЬНОЙ модели со свежим
контекстом именно ради совмещения, и `gates.md` §9 это прямо разрешает («вердикт критика
на сильной модели со свежим контекстом, покрывший (а)–(в), засчитывается как
перепроверка»). `C-132` покрыл (а) утверждения о коде — прогонами на ветке и merge-preview,
(б) полномочия и зону, (в) связность. Обратное направление (перепроверка вместо критика)
правилом запрещено и здесь не использовано.

**Маршрут `A-014` §5 исполнен целиком:** п.1 — `f3102f0` (три файла, нового харнесса ноль);
п.2 — `C-132` NOTE; п.3 — engine-dev `#1–#4` → architect флип фактуры `00886e3` → tester →
reviewer. Ни одного шага не пропущено, порядок совпадает с предписанным.

## 7. Механизм на пути (DoD, `gates.md` §4) — PASS

Милестоун вводит механизм НЕСУЩЕГО пути (`serve_config_from_env` исполняется прод-бинарём
`gateway-serve`), поэтому «built-not-wired» проверяется отдельно, а не подразумевается:

- **точка входа исполняется оракулом,** а не грепается: `red_window_guard_startup.rs`
  зовёт `serve_config_from_env` с инжектируемым getter'ом — тем же вызовом, каким её
  зовёт `main.rs`;
- **композиция producer→consumer цела:** регресс-шаг `red_serve_window_wiring` (M-37)
  подтверждает, что `GATEWAY_WINDOW_MS` доезжает до `Selector.window_ms`, — то есть гвард
  стоит НА пути, а не рядом с ним;
- **второй вход закрыт тем же коммитом:** `validate_selector` — библиотечная точка, через
  которую `Selector` собирают чекпоинтер (M-38b), будущий shared-tailer (M-39) и
  research-cli. Гвард только в транспорте оставил бы ровно ту байпас-поверхность, которой
  посвящён `TD-019`/`TD-020`.

TD-запись «built-not-wired» не заводится: отложенного подключения нет.

## 8. Прод не сломан — канарейка предъявлена исполнением

```
$ grep -nE 'GATEWAY_WINDOW_MS' docker-compose.yml
139:      GATEWAY_WINDOW_MS: ${GATEWAY_WINDOW_MS:-60000}
```

Дефолт `60000` — валидное положительное значение, проходит новый гвард (`prod_window_value_still_starts`
в оракуле пиннит именно его). Правка НЕ может уронить старт прода на текущей конфигурации;
она меняет исход ровно для тех входов, которых сегодня на проде нет и которые никто не
может хотеть.

## 9. Находки

### NOTE-1 — `GW-I-13` остаётся дырой в семействе (не блокер)

Спека (`milestones/M-69-window-guard.md:10`) обосновывает выбор `GW-I-14` тем, что
«семейство `GW-I` занято по `GW-I-13`». Замер этого не подтверждает:

```
$ grep -rhoE '\bGW-I-[0-9]+\b' crates/ | sort -u | tr '\n' ' '
GW-I-1 GW-I-10 GW-I-11 GW-I-12 GW-I-14 GW-I-2 GW-I-3 GW-I-4 GW-I-5 GW-I-6 GW-I-7 GW-I-8 GW-I-9
$ grep -rn 'GW-I-13' docs/ milestones/
docs/plans/scale-architecture-decision.md:293  … контракт внутренних инвариантов GW-I-13+ …
docs/plans/gateway-ws-contract.md:556          … Следующий свободный — **`GW-I-13`**.
```

`GW-I-13` — не занятый инвариант, а ПРОЗАИЧЕСКОЕ УПОМИНАНИЕ «следующего свободного»;
греп спеки шёл по `crates/ docs/` вместе и посчитал упоминание за определение. Практического
вреда нет: коллизии не возникло, `GW-I-13` остаётся свободным и `gates.md` §12 непрерывности
не требует. Почему всё же записано, а не умолчано: строка `gateway-ws-contract.md:556`
живёт в документе, который САМ этот милестоун объявил «фактурой для RED-оракулов» и
синхронизировал задачей #5 — а следующий автор, читающий её, возьмёт `13` и получит
семейство с обгоняющим `14`. Это дешевле исправить строкой, чем объяснять через месяц.
**Не REJECT:** находка класса, обнаружимого с круга 1, а `A-014` §5 п.2 такие
квалифицирует как NOTE; и она не задевает ни sacred, ни safety.

### NOTE-2 — шапка спеки осталась в `📝 PROPOSED`

`milestones/M-69-window-guard.md:3` всё ещё «Статус: 📝 **PROPOSED** … ждёт plan-time
критика», хотя критик прошёл (`C-132`), dev закрыл #1–#4, а §Tasks помечены DONE.
Зона architect'а (`scope-guard.md`: reviewer milestone-файлы не пишет), поэтому правка
не моя. Фиксирую как расхождение шапки с телом того же файла — тот же класс, что
`A-014` B-7 выправлял в `gateway-ws-contract.md`, только внутри милестоуна.

### NOTE-3 — `docs/SESSION-HANDOFF.md:442` отстал на два круга

Живой индекс пайплайна говорит «M-69 — круг 2 у критика», тогда как факт — круг 4
пройден, реализация GREEN. Зона architect'а; называю, чтобы следующая сессия не
бутстрапилась на устаревшем состоянии (`reading-map.md` §Ярус A: `SESSION-HANDOFF` §0
читается ВСЕГДА, поэтому его дрейф дороже дрейфа обычного документа).

**Ни одна из трёх находок не является условием APPROVED.** Все три — документальные,
вне кода, вне sacred, вне safety-пути.

## 10. Условие APPROVED

APPROVED **безусловный**: блокеров нет, находки не блокирующие, все механические барьеры
диапазона зелены на ДЕРЕВЕ СЛИЯНИЯ (не только на ветке — `gates.md` §8, `strict: false`
делает это обязательным, а ветка форкнута от `10bc072` при `origin/main = 0536505`).

---

## 11. Мутационный контроль — оракулы пиннят реализацию, и в ОБЕ стороны

`testing.md` §«Мутационный контроль» требует двух вопросов, а не одного. Проведены оба,
исполнением, в моём worktree `/tmp/hft-rev-M69`; дерево после каждой мутации восстановлено.

**Вопрос 1 — привязан ли оракул к дефекту.**

| # | что нейтрализовано | результат |
|---|---|---|
| M1 | гвард старта снят: `match`-разбор заменён обратно на `s.trim().parse::<i64>().ok()` | `red_window_guard_startup` **FAILED**, exit=101 (`garbage_…`, `float_…`, `minus_one_…`, `i64_max_plus_one_…`, `offline_forms_still_start`) |
| M2 | гвард библиотеки снят: блок `if let Some(w) = sel.window_ms { if w < 0 {…} }` удалён из `validate_selector` | `red_window_selector_guard` **FAILED**, exit=101, `test result: FAILED. 3 passed; 5 failed` — падают все три публичных входа (`snapshot`, `frames_since`, `replay`) плюс прямой вызов |

**Вопрос 2 — что пришлось ослабить рядом.** Это тот вопрос, который обычно не задают, и
здесь он не риторический: развилка «`"0"` — offline, а не ошибка» объявлена спекой
ОСОЗНАННОЙ (паритет с argv-путём `crates/gateway/src/bin/gateway-checkpoint.rs:162` — «`0`
⇒ `None` (offline unbounded)»). Обратная мутация проверяет, что послабление охраняется:

| # | что ужесточено | результат |
|---|---|---|
| M3 | переширокая строгость: `Ok(0) => None` заменено на `return Err(…)` | `red_window_guard_startup` **FAILED**, exit=101, `test result: FAILED. 13 passed; 1 failed` — падает ровно `offline_forms_still_start` |

M3 — существенная, а не декоративная. Без неё `Ok(0) => None` можно было бы «ужесточить»
следующим милестоуном, сломав research-cli, replay-tutor и чекпоинтер M-38b, и ни один
тест бы не покраснел. Оракул краснеет и против слишком мягкой реализации, и против слишком
строгой ⇒ он пиннит ГРАНИЦУ, а не одну её сторону.

**Зависимого эталона нет:** оракул сравнивает не «X с Y, вычисленным через X», а
наблюдаемое поведение `serve_config_from_env` / `validate_selector` с литеральными
ожиданиями (`Err` против `Some(60_000)`/`None`), заданными architect'ом ДО реализации.

## 12. Done Block — сырой вывод

Агрегация по `commit-discipline.md` («сырой ≠ ВЕСЬ»): зелёное сжато до `test result`,
красное печаталось бы целиком — красного нет. Каждое утверждение подкреплено командой и
её exit-кодом.

### 12.1 Предмет и чистота дерева

```
$ cd /tmp/hft-rev-M69 && git log -1 --format='%H %s'
00886e3b745ec7813c2114143137f18a1e420954 docs(M-69): A-014 §5 п.3 — флип фактуры к свершившемуся факту после GREEN [architect]

$ git status --porcelain
?? research/reviews/R-128-M-69-window-guard.md      ← этот вердикт, ещё не закоммичен

$ git diff --stat
(пусто — после мутационного контроля дерево восстановлено побайтно)
```

### 12.2 Acceptance-гейт milestone'а — НА ВЕТКЕ

```
$ bash scripts/verify_M-69.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'; echo exit=$?
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test -p gateway-serve --test red_window_guard_startup --quiet
PASS: cargo test -p gateway --test red_window_selector_guard --quiet
PASS: bash -c grep -qE '`GATEWAY_WINDOW_MS`.*fail-closed.*GW-I-14' docs/plans/gateway-ws-contract.md
PASS: bash -c ! grep -qE '`GATEWAY_WINDOW_MS`.*graceful, НЕ ошибка' docs/plans/gateway-ws-contract.md
PASS: cargo test -p gateway --test red_timeframe_session_alignment --quiet
PASS: cargo test -p gateway-serve --test red_timeframe_guard_startup --quiet
PASS: cargo test -p gateway-serve --test red_serve_window_wiring --quiet
PASS: cargo test -p gateway --test red_gateway_window --quiet
PASS: bash -c ! grep -q 'пусто/не парсится' crates/gateway-serve/src/lib.rs
PASS: bash -c grep -qE 'GATEWAY_WINDOW_MS:[[:space:]]*.*60000' docker-compose.yml
PASS: cargo test --all --quiet
VERDICT: PASS
verify_M69_branch_exit=0
```

**13 PASS / 0 FAIL.** Все ЧЕТЫРЕ ожидаемо-красных на plan-time шага (`A-014` B-8,
§Acceptance спеки) стали зелёными: оба RED-target, greп задачи #4 и дублирующий их
`cargo test --all`. Ни одного красного сверх списка не было и на plan-time (`C-132`), ни
одного не осталось сейчас — то есть маска покраснения совпала с объявленной ТОЧНО.

```
$ grep -E '^test result' /tmp/rev-m69-verify.out | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=890 failed=0 (блоков: 209)

$ grep -cE '^test result: FAILED' /tmp/rev-m69-verify.out
0
```

Число 890/209 — сумма по ВСЕМ `cargo`-вызовам внутри `verify_M-69.sh` (адресные прогоны +
`--all`), а не по одному `cargo test --workspace`; изолированный workspace-прогон tester'а
на той же ревизии дал 845/203. Величины разные, потому что считают разное — назвал явно,
чтобы цифру нельзя было сверить с чужой и объявить расхождением.

### 12.3 ДЕРЕВО СЛИЯНИЯ — базовая тройка CI (`gates.md` §8, `strict: false`)

Ветка форкнута от `10bc072`, а `origin/main` = `0536505`. Защита `main` НЕ включает
`strict`, поэтому зелёный чек снимается на СТАРОЙ базе и merge-коммит не тестируется
никем до попадания в `main`. Прогон на дереве слияния поэтому не примечание, а условие.

```
$ git merge-tree --write-tree origin/main origin/feat/M-69-window-guard
2447e82f38b1a42fb019b311a2c83e275eaa96fe        ← конфликтов нет, exit=0
$ git commit-tree 2447e82 -p origin/main -p origin/feat/M-69-window-guard -m 'merge-preview M-69 (reviewer R-128)'
85866fc6ebb04251cb52337f8d2ad40080862f1a

$ cd /tmp/hft-rev-M69-merge
$ cargo fmt --all -- --check;                                   echo fmt_exit=$?
fmt_exit=0
$ cargo clippy --all-targets --all-features -- -D warnings;     echo clippy_exit=$?
clippy_exit=0
$ cargo test --all 2>&1 | grep -E '^test result' | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" blocks="NR}'
passed=877 failed=0 blocks=208
```

877 против 845 у tester'а на ветке — прирост даёт `main` (за время кругов M-69 туда
приехали новые оракулы). Это ровно то, что прогон на дереве слияния и обязан показывать:
предмет судится вместе с тем, во что он вливается, а не отдельно.

### 12.4 Механические барьеры — НА ДЕРЕВЕ СЛИЯНИЯ

Все зовутся прод-проводкой (`EVENT_NAME=pull_request`, `PR_BASE_SHA=origin/main`) — той
же, какой их зовёт `ci.yml`.

```
$ EVENT_NAME=pull_request PR_BASE_SHA=0536505 bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 5, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 1
gate_meta_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=0536505 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 0536505..HEAD не ввёл второй носитель под занятым идентификатором
artifact_ids_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=0536505 bash scripts/check_docs_freeze.sh
docs_freeze_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=0536505 bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (0536505..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
protected_artifacts_exit=0

$ bash scripts/verify_design_claims.sh
PASS  [7-RFC-PATH] путей-кандидатов … всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
design_claims_merge_exit=0
```

**Замечание о `verify_design_claims.sh`, которое architect назвал первым и которое я
подтверждаю.** Прогон `--merge-preview` ВЕРСИЕЙ СКРИПТА С ВЕТКИ даёт ложный FAIL
`[5-ФАЗЫ]`: ветка форкнута до PR #49, её скрипт не знает про архивный переезд M-18.
Решает версия, которая победит в merge'е, — она выше и она PASS. Проверено мной
независимо, а не принято на веру: счётчик §22 `GW-I` подтверждён прямым замером на дереве
слияния.

```
$ grep -n 'GW-I' docs/DESIGN.md | sed -n 2p
924:| GW-I | gateway-serve | 0 | 13 | обратный дрейф: оракулы есть, докс-семейство не заведено |
$ grep -rhoE '\bGW-I-[0-9]+\b' crates/ | sort -u | wc -l
13
```

### 12.5 Контракты и гигиена диффа

```
$ git diff --name-only 10bc072..00886e3 -- crates/contracts contracts docker-compose.yml \
    crates/gateway/src/bin/gateway-checkpoint.rs '*/Cargo.toml' Cargo.lock
(пусто)
t1_and_forbidden_diff_exit=0

$ git diff --check 10bc072..00886e3
(пусто)
diff_check_exit=0

$ git log --format='%h %s' 10bc072..00886e3 -- '*/tests/*'
a073f8a test(M-69): RED-набор GW-I-14 — две точки гварда, честный RED [architect]
```

### 12.6 Мутационный контроль (§11) — exit-коды

```
$ # M1 — гвард старта снят
$ cargo test -p gateway-serve --test red_window_guard_startup --quiet; echo mutant_serve_exit=$?
garbage_window_blocks_startup --- FAILED
float_window_blocks_startup --- FAILED
minus_one_window_blocks_startup --- FAILED
i64_max_plus_one_blocks_startup --- FAILED
offline_forms_still_start --- FAILED
mutant_serve_exit=101

$ # M2 — гвард validate_selector снят
$ cargo test -p gateway --test red_window_selector_guard --quiet; echo mutant_gw_exit=$?
validate_selector_itself_rejects_negative_window --- FAILED
negative_window_rejected_by_replay --- FAILED
minus_one_window_rejected_not_panic --- FAILED
negative_window_rejected_by_frames_since --- FAILED
negative_window_rejected_by_snapshot --- FAILED
test result: FAILED. 3 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
mutant_gw_exit=101

$ # M3 — переширокая строгость: "0" отвергается
$ cargo test -p gateway-serve --test red_window_guard_startup --quiet; echo mutant_overstrict_exit=$?
offline_forms_still_start --- FAILED
test result: FAILED. 13 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
mutant_overstrict_exit=101

$ git diff --stat        # после восстановления
(пусто)
```

---

## Вердикт: **APPROVED**

Милестоун закрывает буквальный класс R7 — **единственное оставшееся отступление read-path
от fail-closed** (`docs/08-arch-improvement-roadmap.md:35`, CRIT; `DESIGN.md:940` `PL-I-5`).
Гвард стоит в ДВУХ точках, и вторая — не перестраховка: `Selector` собирают напрямую
чекпоинтер M-38b, будущий shared-tailer M-39 и research-cli, поэтому гвард только в
транспорте оставил бы ровно ту байпас-поверхность, которой посвящены `TD-019`/`TD-020`.
Прод на текущей конфигурации (`GATEWAY_WINDOW_MS=60000`) не задет и запиннен отдельным
кейсом оракула.

Закрывает `TD-162` (MAJOR, Ф2) — карточка сама предписывает закрытие merge'ем M-69 + §8,
без отдельной работы. Обновление `TECH-DEBT.md` и `PROJECT-STATE.md` — за мной, следом
за merge'ем.

Три находки (§9) — NOTE, все документальные, ни одна не условие APPROVED и ни одна не
задевает sacred/safety: дыра `GW-I-13` в семействе, шапка спеки в `PROPOSED`, отставший
`SESSION-HANDOFF.md:442`. Все три — зона architect'а.
