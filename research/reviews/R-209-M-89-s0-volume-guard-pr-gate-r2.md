<!-- GATE-META
milestone: M-89
audited_repo: a3ka/hft-platform
audited_base: 3809085c4053c6e031ecebfe55bcdfc2745aeaac
audited_head: 25c9ff4accc5897d1d3a7b4dd1da3dda19dff829
verdict: REJECT
-->

# R-209 — M-89 «остаток S0: объём одной выдачи структурно + видно снаружи», PR-гейт, круг 2

**Вердикт: REJECTED — узко.** Все ЧЕТЫРЕ функциональных дефекта `R-208` действительно
исправлены, и я проверил каждый независимо, а не по отчёту. Не поставлено другое:
**ЗАКРЕПЛЕНИЕ**, которого условия `R-208` требовали дословно. Три условия из шести имеют
невыполненную половину, и все три лежат в architect-sacred зонах (`*/tests/**`,
`scripts/verify_*.sh`) — поэтому engine-dev их и не закрыл, и это правильно.

Приёмка при этом ЗЕЛЕНА, и я воспроизвёл её на СВОЁМ чистом чекауте
(`/tmp/hft-rev89-gate`, detached `25c9ff4`): Done Block в §6.

**Это НЕ спор и НЕ триггер арбитра (`gates.md` §0).** Второй REJECT подряд по смежной
причине обычно означает, что стороны не понимают друг друга. Здесь иначе: оба коммита-фикса
engine-dev'а ПРЯМО НАЗЫВАЮТ пробел (тело `d89c43f` цитирует `testing.md` §«канарейка» и
говорит, что шаг task11 судит ТЕКСТ, а не поведение), но зоны закрытия ему запрещены
(`scope-guard.md`). Цепочка после фиксов ушла `tester → reviewer`, минуя architect'а.
Это незавершённая МАРШРУТИЗАЦИЯ, а не разногласие: предмет возвращается architect'у на два
оракула, не на переработку.

## 0. Что проверено и чем

| блок | исход |
|---|---|
| Block-scope | диф не трогает `crates/{risk,killswitch,oms,contracts,venue-*,book,recorder}/**` — `git diff --name-only origin/main...HEAD`, пусто. RISK-BLOCK (`gates.md` §5) неприменим; ордерного пути нет |
| Block-C | `crates/contracts/**` в дифе отсутствует ⇒ contract-RFC не требуется |
| plan-time цепочка | `C-260` REJECT → `C-262` REJECT → `C-263` ESCALATE → `A-042` DECISION → `C-264` NOTE — четыре файла в `research/`; RAW-гейт на сильной модели предъявлен |
| Done Block | прогнан МНОЙ заново на чистом чекауте (§6), не принят пересказом tester'а |
| мутационный контроль | проведён МНОЙ (§2.1, §3.1) — обе находки предъявлены сырым выводом, а не рассуждением |
| FA (M-66) | `JR-I-2` (`docs/fa/journal.md:112`), `JR-I-11` (`:133`), `OPS-I-6` (`docs/fa/ops.md:474`), `OPS-I-8` (`:476`) — живые на `25c9ff4` |
| ярус C | `TECH-DEBT.md` грепом по `TD-219`/`TD-220`/`TD-221`/`TD-224`/`TD-225` и `TD-124`; `PROJECT-STATE.md` грепом по `M-87`/`M-89`. Оба читаны на `origin/main` — локальный `main` чекаута отстал |
| ярус S | `gh run list` (main зелен), `gh run list --workflow=deploy.yml`, `ssh rev-parse HEAD`, состояние прода — §5 |

## 1. Условия `R-208` — построчно

| # | условие `R-208` | исход |
|---|---|---|
| 1 | `Б-1`: per-pump байты идут в ручку ЭКЗЕМПЛЯРА **+ оракул судит ручку ПОСЛЕ pump'ов** | код ✅ / **оракул ❌ — §2** |
| 2 | `Б-2`: warm-`resume` отдаёт только ПРОЧИТАННОЕ; `segments()`/`read_dir` ушли | ✅ полностью — §4.1 |
| 3 | `Б-3`: §13 приведён в соответствие с фактом | ✅ — §4.2 |
| 4 | `Н-1`: присваивание ВЫШЕ расписания **+ шаг task11 это различает** | файл ✅ / **гейт ❌ — §3** |
| 5 | `Н-2`: Done Block engine-dev'а с мутантами §14 предъявлен сырым выводом | **❌ — §4.3** |
| 6 | `Н-7`: ветка синхронизирована с `main`, merge-preview прогнан | ✅ — `git rev-list --count HEAD..origin/main` = **0**, `merge-base` = `3809085` = `origin/main` |

## 2. Б-1 — БЛОКЕР (остаток). Фикс верен и НИЧЕМ НЕ ЗАКРЕПЛЁН

**Код исправлен — проверено.** Три точки per-pump push кормят ручку экземпляра:
`crates/gateway-serve/src/lib.rs:1831` (`inner.counters.as_ref()`), `:2180`
(`counters_for_setup.as_ref()`), `:2461` (`counters.as_ref()`); `:2272` — `resume`-путь.
`metrics::add_journal_payload_bytes` (`metrics.rs:163-169`) кормит handle И process-global.

**Оракула, судящего ручку ПОСЛЕ pump'ов, НЕТ.** Это не вывод по чтению — это замер.

### 2.1. Мутационный контроль (сырой вывод, прогон reviewer'а)

Три точки возвращены к `add_journal_payload_bytes_pub(...)` — воспроизведён РОВНО дефект
`R-208` Б-1 (байты pump'ов в process-global; `resume` по-прежнему кормит handle):

```
### i1/i2 counters_instance
test i2_instance_handle_is_fed_by_the_real_serving_path ... ok
test i1_requests_to_one_server_do_not_move_counters_of_another ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s
### v read_volume_truth
test v_counter_includes_first_pump_and_equals_the_reader_delta ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.97s
```

**Ноль красных.** Тест, НАЗВАННЫЙ по проверяемому свойству —
`i2_instance_handle_is_fed_by_the_real_serving_path` — зелен при сломанном свойстве.
Файл восстановлен побайтно (`git diff --stat HEAD` пуст, `git status --porcelain` пуст).

### 2.2. Почему набор слеп — по построению

| оракул | ассерт | чем удовлетворяется |
|---|---|---|
| `i1` | `a1.journal_payload_bytes_read > 0` (`red_m89_counters_instance.rs:246`) | одним `resume` |
| `i2` | `after.journal_payload_bytes_read > before...` (`:277`) | одним `resume` |
| `h1` | `b1 > 0` (`red_m89_heartbeat_entrypoint.rs:636`) | одним `resume` |
| `v` | равенство ПРОЦЕССНОМУ `serving_counters()` | обе прибавки складываются — разницы не видно |

Ни один не требует, чтобы байты pump'ов дошли до ручки. **`git diff --stat 97307e3..HEAD`**
(после вердикта `R-208`) показывает: из тестов изменены только `red_m87_entrypoint.rs`
(задача 7) и два чужих харнесс-пробника (`red_ci_aggregate.sh`, `red_deploy_catchup.sh`,
приехали синхронизацией с `main`). **Оракул на находку `R-208` не добавлялся вовсе.**

**Почему это существенно, а не педантизм.** Величина в сердцебиении — ЕДИНСТВЕННЫЙ предмет
`TD-220` («счётчики наружу процесса»), и `M-89` эту карточку ЗАКРЫВАЕТ. Набор зелен и с
дефектом, и без него — значит правильность закрываемого долга не держит ничто. Закрыть
`TD-220` в таком виде — воспроизвести класс «built-not-wired» в новой одежде, тот самый,
которым `TD-219`/`TD-220` и являются. Норма прямая (`testing.md`): «Исправление по вердикту
тоже требует оракула… без RED на саму находку следующий круг ловит её же в новой одежде».

**Дизайн оракула — зона architect'а** (`gates.md` §4, граница reviewer↔architect). Здесь
описан дефект, не фикс. Форма требования названа `R-208` и не изменилась: ручка выросла
РОВНО на сумму `ReadStats.payload_bytes_read` всех pump'ов сессии, и мутация §2.1 обязана
его ронять.

## 3. Н-1 — MAJOR (остаток). Файл исправлен, гейт по-прежнему слеп

**Файл исправлен — проверено.** `deploy/cron.d/watchdog`: присваивание — строка **33**,
расписание — строка **34**. Порядок верный.

**Шаг task11 порядок НЕ РАЗЛИЧАЕТ.** `scripts/verify_M-89.sh:345`:

```sh
cron_path=$(grep -E '^WATCHDOG_SERVING_HEARTBEAT_PATH=' "$cron" 2>/dev/null | head -1 | cut -d= -f2- | tr -d '"')
```

`grep` по ВСЕМУ файлу + `head -1` — значение берётся где угодно, в том числе НИЖЕ
расписания, где на задание оно не действует. Замер: копия файла с переставленными строками
(присваивание ПОД расписанием — ровно дефект `R-208`) проходит шаг с **`exit=0`**, вывод
байт-в-байт совпадает с прогоном на исправленном файле. Другого места, проверяющего порядок,
в репозитории нет (`h0`, `scripts/tests/red_verify_M-89_*.sh`, `crates/ops/tests/**`
проверены).

Инвариант держится КОММЕНТАРИЕМ `deploy/cron.d/watchdog:28-32` — то есть ничем.
Тело коммита `d89c43f` само это называет: «Шаг verify task11 ранее судил … где угодно в
файле — это проверка ТЕКСТА, не поведения». Названо — и оставлено, потому что
`scripts/verify_*.sh` для engine-dev'а sacred. Зона закрытия — **architect**.

## 4. Что ПРИНЯТО

### 4.1. Б-2 — закрыт полностью
`crates/gateway/src/lib.rs:5087-5090`: warm-`resume` отдаёт `payload_bytes_read: ckpt_bytes_read`
— только размер файла слепка, ни оценки, ни дубликата. Вызовов `tail_bytes_after_cursor` /
`payload_bytes_after_cursor` / `tail_bytes_for_dir` на горячем пути НЕТ
(`grep -rn 'tail_bytes_for_dir' crates/ --include=*.rs` вне `journal/src/segments.rs` даёт
только комментарии и ре-экспорт). `read_dir` + заголовки с warm-пути ушли ПО СВОЙСТВУ, а не по имени.

### 4.2. Б-3 — закрыт
§13: задачи 1–11 `✅ DONE`, задача 12 `⏳ OPEN` (прод-замер, зона reviewer'а). Текст «ОСТАТОК»
приведён в соответствие. Задача 7 честно называет, что прежняя пометка DONE (`1378946`) стояла
ДО исполнения.

### 4.3. Н-2 — НЕ закрыт (MAJOR, остаток)
§13 задача 1 требует «Done Block с мутантами §14, включая четыре мутанта стыка (`A-042` §5)»;
задача 10 — «Done Block с мутантами §14». Замер по телам коммитов диапазона: `56b858e`
(задача 1) и `344dcc7` (задача 10) — **0** упоминаний мутации. Мутационные прогоны несут
только коммиты architect'а (`8277881`, `a1a892e`, `cc36b91`, `207144f`, `e93c6c3`) и арбитра.
Непацебность гардов (в)/(г) и наблюдаемого отката не ПРЕДЪЯВЛЕНА — она заявлена зелёным
набором, а зелёный набор и есть то, что мутация проверяет.

### 4.4. Принято без оговорок
- **`TD-225` механизирован по-настоящему.** Ручная сверка reviewer'а стала гейтом:
  `verify_M-89.sh:469` гоняет `red_m89_serial_guard` (сторож + мутанты `m1`…`m6`).
  Ручная проверка сошлась: `grep -cE '^#\[tokio::test'` = **14**, `grep -c 'SERIAL.lock().await'` = **14**.
- **Задача 7 исполнена.** В `red_m87_entrypoint.rs` нет ни одного КОДОВОГО вызова процессного
  `serving_counters()` (упоминания на `:39`, `:398`, `:426` — док-комментарии); `c1`/`c5`/`c6`/`u1`
  читают `counters_handle()` (`:440`).
- **Н-7 закрыт:** ветка на `origin/main` (`behind = 0`).
- `docker-compose.yml`: том `gateway-state:/state` rw, `GATEWAY_HEARTBEAT_PATH` — ФАЙЛ, не каталог.

## 5. Состояние мира (ярус S) — проверено, находок нет

- `main` зелен: последний CI `success` (`36330811316`), Deploy `success` (`36331449850`).
- **Отставание прода — БЕЗВРЕДНО и проверено, а не предположено.** Прод `843894f`, это на 34
  коммита позади `origin/main` (`3809085`). Из этих 34 коммитов рантайм-исходников не тронут
  НИ ОДИН (`git diff --name-only 843894f..origin/main` под `crates/**/*.rs` даёт единственный
  файл — ТЕСТ `red_m87_entrypoint.rs`; остальное `docs/`, `research/`, `scripts/`, `.github/`),
  а `deploy.yml` фильтрован по `paths:`. Деплой законно не срабатывал.
- Контейнеры `hft-gateway-serve`/`hft-recorder` — `Up 3 days (healthy)`; `recorder.heartbeat`
  свежий (дельта ≈ 5 с на момент замера).
- **Задача 11 на проде НЕ установлена** (ожидаемо, §11 п. 5 — ручной шаг founder ★):
  `/etc/cron.d/hft-watchdog` — нет, `/var/lib/hft/watchdog.last-success` — нет, тома
  `hft-platform_gateway-state` — нет, монтирования `gateway-serve` — `/journal rw=false`,
  `/ckpt rw=false`.
- **Задача 12 (прод-замер ДО/ПОСЛЕ) в этом круге НЕ СНИМАЕТСЯ и это не отказ от неё:** кода
  `M-89` на проде нет, «ПОСЛЕ» физически не существует. Замер — на §8-гейте ПОСЛЕ merge'а и
  деплоя; основание и форма — `docs/plans/m89-warm-resume-measure-2026-09-27.md` (курсор
  слепка ≈ 412 МБ вглубь активного сегмента).

## 6. Done Block — прогон reviewer'а на ЧИСТОМ чекауте

Дерево `/tmp/hft-rev89-gate`, `git worktree add --detach 25c9ff4`, `git status --porcelain` пуст.

```
$ cargo build --workspace --tests 2>&1 | tail -2
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.82s
BUILD_EXIT=0

$ cargo fmt --all -- --check; echo fmt_exit=$?
fmt_exit=0

$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -2
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.86s
clippy_exit=0

$ cargo test --all --all-features 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" блоков="NR}'
passed=1177 failed=0 блоков=268

$ grep -E 'FAILED|panicked at' <полный лог>
{пусто}
```

```
$ bash scripts/verify_M-89.sh 2>&1 | grep -E "^(PASS|FAIL|SKIP|VERDICT)"; echo exit=$?
PASS=83 FAIL=0 SKIP=2
VERDICT: PASS
VERIFY_EXIT=0

SKIP  task11: установка deploy/cron.d/watchdog в /etc/cron.d и сборка ops-watchdog на VPS
      — ручной шаг founder ★ (deploy/README.md); предъявляется reviewer'ом на §8-гейте
SKIP  task12: rchar/время первой выдачи новой подписки на проде ДО/ПОСЛЕ при курсоре
      слепка глубоко в активном сегменте — снимается reviewer'ом на §8-гейте ПОСЛЕ деплоя
```

Оба `SKIP` — названные пределы поставки (§11 п. 5, п. 7), не замаскированные провалы.
Против `R-208` (81 PASS / 0 FAIL / 3 SKIP) третий `SKIP` стал `PASS`: шаг `review-fa`
зеленеет, когда в диапазоне есть вердикт reviewer'а.

**Предел этого Done Block назван честно:** он предъявляет, что набор ЗЕЛЁН. Он НЕ
предъявляет, что набор что-либо ЛОВИТ, — это предъявляет мутация, и она в §2.1 КРАСНОЙ не
стала. Ровно поэтому вердикт REJECT при зелёной приёмке: расхождение между «гейт зелен» и
«гейт держит» и есть предмет этого круга.

## 7. Условие APPROVED (круг 3) — зона architect'а

1. **`Б-1`**: оракул судит ручку ЭКЗЕМПЛЯРА ПОСЛЕ pump'ов — «ручка выросла РОВНО на сумму
   `ReadStats.payload_bytes_read` всех pump'ов сессии». Предъявляется мутацией §2.1: возврат
   трёх точек к `add_journal_payload_bytes_pub` обязан РОНЯТЬ оракул. Сегодня — 3 зелёных из 3.
2. **`Н-1`**: шаг `task11` различает ПОРЯДОК строк в `cron.d` (присваивание выше расписания).
   Предъявляется прогоном на переставленной копии: сегодня `exit=0`, обязан быть `exit≠0`.
3. **`Н-2`**: Done Block engine-dev'а по задачам 1 и 10 с мутантами §14 (включая четыре
   мутанта стыка `A-042` §5) — сырым выводом.
4. **`Н-3`** (MINOR, по усмотрению, того же круга): два док-комментария описывают СНЯТОЕ
   состояние — `crates/gateway-serve/tests/red_m89_read_volume_truth.rs:7` («warm-`resume`
   кормит счётчик `ckpt_bytes + payload_bytes_after_cursor`») и
   `crates/gateway-serve/src/lib.rs:2268-2271` («warm = ckpt_bytes + tail_bytes_after_cursor»)
   — оба называют механизм, удалённый фиксом `Б-2`.
5. **Новое, MINOR:** `journal::tail_bytes_for_dir` остался `pub` и ре-экспортирован
   (`crates/journal/src/lib.rs:26`) при НУЛЕ вызывающих — введён `9faa54d`, осиротел фиксом
   `Б-2`. Либо снять, либо назвать назначение.
6. **`Н-4`/`Н-6`** — прежние MINOR/NOTE, не изменились: восемь коммитов диапазона без ролевой
   метки; `docs/DESIGN.md` вне §12 Allowed paths (правка — счётчик покрытия `JR-I` 7 → 8,
   фактически верный; зона названа, блокером не считаю).

**Маршрут:** architect (пункты 1, 2, 4) + engine-dev (пункт 3) → tester → reviewer.
Merge держится до закрытия пунктов 1–3.

