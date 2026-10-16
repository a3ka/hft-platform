<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: 7c1bc09df40c3d53fe230380946decc4c6635945
audited_head: 6a8e0bb97ac0670ea6b00ff2c84510e5f18b98f1
verdict: REJECT
-->

# R-200 — M-87 «предохранитель выдачи», PR-time гейт круга задачи 20: **REJECT**

**Предмет.** Ветка `feat/M-87-serving-circuit-breaker`; вершина взята КОМАНДОЙ, а не из
мандата (норма делегирования, `ROADMAP` строка `GM-GAP`): `git rev-parse
origin/feat/M-87-serving-circuit-breaker` → `6a8e0bb97ac0670ea6b00ff2c84510e5f18b98f1` —
совпало с SHA в отчёте tester'а. База аудита — `7c1bc09`, вершина, которую судил `R-196`:
судятся **57** коммитов, легших после предыдущего PR-гейта.

**Главное в одной строке.** Задача 20 сделана В ТОЙ ФОРМЕ, которую предписала спека, и
всё-таки НЕ ЗАКРЫТА: из двух мест транспорта, ради которых она заводилась, оракулом пиннится
ОДНО. Мутация второго места проходит ВЕСЬ гейт зелёной (§B4). Это тот же класс, который
задача 20 и лечила, — механизм, который нельзя предъявить прогоном.

## Предъявление FA (`gates.md` §4, M-66)

Диф трогает `crates/gateway/**` и `crates/gateway-serve/**`; своей FA у обоих крейтов нет —
`check_review_fa.sh:154-162` отображает их на `docs/fa/viz-backend.md` (префиксы `VB` и `GS`).
Сам этот пробел — числящийся долг (`reading-map.md` §2, спека §17).

**Живой инвариант предмета — `VB-I-11`** (`docs/fa/viz-backend.md:214`): «система не
отказывается отдать то, что есть, но обязана НЕ ВЫДАВАТЬ ЭТО ЗА ДРУГОЕ: `Snapshot` несёт
`history_start_seq` … и `history_truncated`». Задача 20 закрывает путь, которым `VB-I-11`
обходился молча: при `Err` пересчёта наружу уходило замороженное `history_truncated=false`.
Второй живой ID зоны транспорта — `GS-I-1` (§5 того же документа); предметом круга не тронут,
назван для полноты множества U.

**`FA-WAIVER` не требуется** — барьер требует его только для NO-FA крейтов (`recorder`,
`derive`, `check_review_fa.sh:60`), ни один не тронут. Строка `FA-WAIVER: crates/gateway-serve`
в Handoff'е tester'а лишняя: крейт в барьере отображён, а не числится NO-FA. Не находка —
поправка факта.

## Block-scope — **PASS**

`git diff --name-only origin/main...HEAD` — 50 файлов, каждый в зоне, разрешённой спекой §12:

| зона | файлов | владелец |
|---|---|---|
| `crates/gateway-serve/src/**` (вкл. `bin/wsprobe.rs`) | 6 | engine-dev ✓ |
| `crates/gateway/src/lib.rs` | 1 | engine-dev ✓ |
| `docker-compose.yml` (ресурсные лимиты СВОИХ сервисов) | 1 | engine-dev ✓ |
| `crates/{gateway,gateway-serve}/tests/**` | 16 | architect ✓ |
| `scripts/verify_M-87.sh` · `milestones/M-87-*.md` · `docs/fa/viz-backend.md` | 3 | architect ✓ |
| `docs/ROADMAP.md` (строка `SCALE` — перечень/порядок) | 1 | architect ✓ |
| `research/{critiques,arbitration,reviews}/**` | 21 | critic / арбитр / reviewer ✓ |

Вне зоны — ничего: `crates/{journal,book,venue-*,contracts,risk,killswitch}` не тронуты.

**Тесты sacred dev'ом не тронуты** — проверено, а не принято на слово:
`git log --format='%h %s' origin/main..HEAD -- 'crates/*/tests/**' | grep -E '\[(engine|venue|signal|research)-dev\]'`
→ пусто; все 28 коммитов, тронувших `*/tests/**`, несут `[architect]`.

**RED-first по задаче 20 соблюдён:** `44bc152 test(M-87): задача 20 — RED-оракул и шаг
приёмки … [architect]` лёг ДО `edad3b9 fix(M-87): task #20 … [engine-dev]`.

## Block-C (T1) — **не применим**

`crates/contracts/**` не тронут, `GATEWAY_SCHEMA_VERSION` не бампался. Выбранная §14.1decies
форма («консервативная трактовка») именно затем и выбрана, чтобы не менять форму провода;
поле «провенанс неизвестен» названо кандидатом на следующий bump, а не сделано тайком.
Contract-RFC не требуется.

## Block-risk — **risk-critic НЕ требуется**, основание предъявлено путями

`gates.md` §5 привязан к `crates/risk|killswitch|oms|venue-*|contracts` — ни один не в дифе.
`gates.md` §9 тоже не срабатывает: тронут `docs/fa/viz-backend.md`, а не `fa/risk|killswitch|oms`;
правок `RK-I-*`/`INTG-I-*` в дифе нет. Ордерного пути в поставке нет вовсе.

## Block-DoneBlock — отчёт tester'а **ВОСПРОИЗВЕДЁН**

Прогон на независимом дереве (`/tmp/hft-reviewer-m87-r2`, detached `6a8e0bb`) дал те же
числа: 26 PASS / 1 SKIP / 0 FAIL, `VERDICT: PASS`, exit=0; `cargo test --all` без падений;
fmt и clippy зелены. Отчёт tester'а фактам соответствует — и именно поэтому находки ниже
важны: **гейт зелен, а два места кода не защищены**. Зелёный гейт не есть доказательство, если
не предъявлено, ЧТО он покраснит (`testing.md` §«Целостность гейта»).

## §A — задача 20 по существу: форма верна, покрытие — половина

Принято:

1. **Форма ровно предписанная** (`crates/gateway/src/lib.rs:3781+`):
   `history_provenance_for_serve(dir, filter, frozen_start_seq) -> (u64, bool)`,
   `Ok(calc) => calc`, `Err(_) => (frozen_start_seq, true)`. Коды ошибок поимённо не
   разбираются — так и требовалось.
2. **Прямых вызовов в транспорте не осталось**: `grep -c current_history_provenance
   crates/gateway-serve/src/lib.rs` → **0**.
3. **`current_history_provenance` семантику не сменила**; аддитивность библиотеки (`A-037`
   D-1) держится — шаг `D-1(а)` зелёный.
4. **Оракул не плацебо — предъявлено мутацией, а не рассуждением:** `h0` — setup-страж (оба
   пути сбоя ДЕЙСТВИТЕЛЬНО дают `Err`); `h1` — парный vantage с заведомо ложным `u64::MAX`,
   ловит заглушку «всегда truncated»; `h3`/`h4` — два РАЗНЫХ кода сбоя, один требуемый исход.
   Мутация `Err(_) => (frozen_start_seq, false)` роняет `h3` и `h4`:
   `test result: FAILED. 3 passed; 2 failed`, rc=101 (§Done Block, мутация 1).
5. **Метка FA в теле коммита есть** (`edad3b9` называет `VB-I-11`).

Не принято — §B4: **второй вызыватель не пиннит никто**, и объявленная связка «оракул +
канарейка закрывают и поведение, и подключение» (§14.1decies, последний абзац) на этом месте
НЕ ВЫПОЛНЯЕТСЯ.

## §B1 — БЛОКЕР (механический). `check_gate_meta` КРАСЕН на дереве слияния

Джоб `gate-meta` входит в агрегат `All checks passed` (`.github/workflows/ci.yml:615`) ⇒
merge физически невозможен, пока барьер красен.

```
EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) bash scripts/check_gate_meta.sh
FAIL  research/arbitration/A-038-m87-latch-form.md: subject-lock — после проходного вердикта (DECISION) тронут класс «гейт»: scripts/verify_M-87.sh
FAIL  research/arbitration/A-039-m87-common-policy-proof.md: … то же
FAIL  research/arbitration/A-040-m87-registry-parser-class.md: … то же
FAIL  research/critiques/C-244-m87-round7.md: … (NOTE)
FAIL  research/critiques/C-254-m87-c253-execution.md: … (NOTE)
VERDICT: FAIL (5)   exit=1
```

Причина названа точно: токен `ALLOW-SUBJECT-CHANGE` в диапазоне ровно ОДИН — `dd47ef7`
(исполнение `A-037`), и он открыл лок только для `A-037` (барьер печатает по нему `NOTE`).
Пять последующих проходных вердиктов остались запертыми, а `scripts/verify_M-87.sh` правился
после каждого — последний раз `44bc152` (шаг приёмки задачи 20), токена не несущий.

Правки гейта здесь ЗАКОННЫ (исполнение решений арбитра и критика), но барьер по построению
требует явного аудит-следа на каждую; `gates.md` §11 называет токен аудит-следом, а не
разрешением. **Закрывает architect** — `scripts/verify_*.sh` его sacred-зона.

## §B2 — БЛОКЕР (механический). Ветка отстаёт на 73 коммита и НЕ СЛИВАЕТСЯ

```
git rev-list --count origin/feat/M-87-serving-circuit-breaker..origin/main   → 73
git merge --no-commit --no-ff origin/main
CONFLICT (content): Merge conflict in docs/ROADMAP.md
```

Конфликт мелкий: `main` переписал строку `P0-CORR`, ветка — соседнюю строку `SCALE`.
Разрешение — `P0-CORR` из `main`, `SCALE` из ветки.

**Важнее конфликта то, что за ним.** `main` принял `M-88`, и диф
`merge-base..origin/main -- crates/gateway/src/lib.rs` = **+375/−32** плюс новые фикстуры
`crates/gateway/tests/fixtures/m88/**`: два предмета живут в ОДНОМ файле. При `strict: false`
(`gates.md` §8) зелёный чек снимается на СТАРОЙ базе, и дерево слияния не проверяет никто.
Я прогнал его сам — результат в §Done Block. **Синхронизацию делает architect** (строка
`SCALE` — перечень/порядок, его зона; колонка «Состояние» — моя, правки не требует).

## §B3 — БЛОКЕР (содержательный). §Tasks лжёт: 19 задач из 20 стоят ⏳ OPEN

В `milestones/M-87-serving-circuit-breaker.md` §13 на `6a8e0bb` `✅ DONE` стоит ТОЛЬКО у
задачи 20. Задачи 1–9 и 11–19 помечены `⏳ OPEN` при легшей реализации: `d1fd221` task #12,
`a86b8b5` #13, `b2421f0` #11, `5521036` #14, `eaf2d35` #15, `c8401fd` #16, `99f225e`/`5d475bd`
#17, `f78d0a1` #18, `e995522` #19.

Задача 19 требует отдельной строки, потому что `revert` в subject'е читается как отмена
задачи: `git show e995522` показывает обратное — отменено ПОГЛОЩЕНИЕ `InvalidData`,
внесённое работой по задаче 17; `advance` теперь возвращает `Err`. Задача исполнена, статус —
`OPEN`.

Документ, входящий в `main`, обязан быть правдив: по нему исполняют следующие круги, и
«OPEN» у сделанной задачи вводит в заблуждение ровно так же, как «DONE» у несделанной.
Колонка Status — carve-out dev'а (`scope-guard.md`), закрывается одним коммитом engine-dev'а.

## §B4 — БЛОКЕР (содержательный). Второй вызыватель задачи 20 не пиннит НИЧТО

**Мест, ради которых заводилась задача 20, два:**

- `crates/gateway-serve/src/lib.rs:1268` — ADD/SWITCH-путь, внутри `handle_v1_message`;
- `crates/gateway-serve/src/lib.rs:1949` — путь первичного снимка, внутри
  `run_authorized_session`.

**Мутация второго рода** (вызов обёртки ОСТАВЛЕН, убрана только перезапись признака на
ADD-пути: `let (live_start, _live_truncated) = …;` без строки `snap.history_truncated =
live_truncated;`) даёт клиенту замороженное из слепка `history_truncated` — ровно ту ложь,
ради устранения которой задача заводилась. Прогон в изолированном дереве
(`/tmp/hft-rev-m87-mut`, detached `6a8e0bb`), сырой вывод — в §Done Block: канарейка гейта
**PASS** (`CHP=0 HPS=3`), `red_m87_history_provenance_failclosed` **5 passed**,
`red_ws_honesty_sessions` **3 passed**, `red_m87_entrypoint --features testing`
**15 passed**, `clippy -D warnings` **rc=0**. **Не покраснело НИГДЕ.**

Почему так — две сцепленные причины, каждая проверяема:

1. **Оракул судит ФУНКЦИЮ, а не сквозной путь** — это спека признаёт сама (§14.1decies,
   «предел собственной правки») и перекладывает доказательство подключения на канарейку.
2. **Канарейка считает ВХОЖДЕНИЯ ИМЕНИ, а не вызовы.** `HPS=$(grep -c
   history_provenance_for_serve crates/gateway-serve/src/lib.rs)` → **3**, и одно из трёх —
   реэкспорт `pub use gateway::{checkpoint::history_provenance_for_serve, …}` (`:2396`).
   Порог `HPS -ge 2` держится при ЕДИНСТВЕННОМ живом вызове, тогда как требование задачи —
   ДВА места транспорта.

**Покрыт оракулом только сайт `:1949`** — через `red_ws_honesty_sessions::o6_pruned_journal_is_honestly_marked`
(проверено: оба файла, ассертящие `history_truncated`, строят снимок хелперами
`ws_snapshot*`, то есть идут путём первичного снимка; ни один тест не проверяет провенанс
ПОСЛЕ `subscribe`/ADD).

**Что требуется:** оракул, который после ADD-подписки требует честного `history_truncated`
(зеркало `o6` на ADD-пути), и канарейка, считающая ВЫЗОВЫ, а не упоминания (исключить
реэкспорт либо считать `history_provenance_for_serve(` со скобкой). Форма оракула — зона
architect'а (`gates.md` §4, граница reviewer↔architect: я описываю дефект, фикс проектирует
architect).

## §B5 — БЛОКЕР (харнесс). Шаг `D-1(б)` решает ПО ТЕКСТУ и fail-open'ит на некомпилирующемся корпусе

`scripts/verify_M-87.sh:311-317`:

```bash
LIBOUT=$(cargo test -p gateway 2>&1)
if printf '%s\n' "$LIBOUT" | grep -qE '^test result: FAILED'; then fail …; else pass …; fi
```

`gates.md` §3 запрещает это прямо: «решение принимается по КОДУ ВОЗВРАТА, а не по тексту
вывода». Последствие не теоретическое: при ошибке КОМПИЛЯЦИИ строки `test result: FAILED` в
выводе нет вовсе, и гейт печатает `PASS D-1(б): библиотечный корпус зелен — passed= failed=`.
Воспроизведено дословно (§Done Block). Пустые `passed=`/`failed=` в строке PASS — видимый
признак того, что шаг не мерил ничего. Починка — `LIBRC=$?` сразу после вызова. Зона —
architect.

## §B6 — БЛОКЕР (DoD). Задача 14 «механизм НА прод-пути» оракулом точки входа не предъявлена

`gates.md` §4 DoD: milestone, вводящий механизм несущего пути, мержится ТОЛЬКО с
подключением, доказанным оракулом ТОЧКИ ВХОДА; отложено осознанно — TD-запись
«built-not-wired» severity MAJOR.

Замер: `grep -rn 'CARGO_BIN_EXE_gateway-serve\|Command::new' --include=*.rs
crates/gateway-serve/tests/ crates/gateway/tests/` → совпадения ТОЛЬКО в
`crates/gateway/tests/red_checkpoint_bin_prod_argv.rs` (это прецедент для ДРУГОГО бинаря,
`gateway-checkpoint`). Прод-бинарь `gateway-serve` не запускает ни один тест.

При этом именно прод-вход несёт два обязательства задач 3 и 14:
`crates/gateway-serve/src/main.rs:37` — `admission_policy_from_env` (fail-closed разбор env,
отказ СТАРТА на невалидном) и `:49` — `bind_with_policy`. Оба подтверждены сегодня ТОЛЬКО
компиляцией. `red_m87_entrypoint` называется «оракулом реальной точки входа», но входит через
библиотеку, а не через границу процесса: argv из `docker-compose.yml`
(`entrypoint: ["/usr/local/bin/gateway-serve"]`) и разбор env не исполняются нигде.

`R-196` B1 («предохранитель НЕ ПОДКЛЮЧЁН к прод-пути») закрыт, таким образом, в коде и
НЕ ЗАКРЫТ предъявлением. Выход — любой из двух, но явный: оракул точки входа по образцу
`red_checkpoint_bin_prod_argv.rs` ЛИБО карточка `built-not-wired` MAJOR, заводимая мной на
close-out, с явным согласием architect'а, что предъявление отложено.

## §B7 — БЛОКЕР. Пять условий APPROVED из `R-196` не исполнены, а гейт этого не видит

Проверял каждое условие командой на `6a8e0bb`, а не по отчётам. Закрыты полностью: №1 (B1 — в
коде, см. оговорку §B6), №2 (B2 — порог в политике, дефолт 255 600 ≥ опоры 127 800), №3 (B3 —
`key_material` = сырые байты, `parse_secret` удалён), №9 (атомарность — на будущее), №10
(гейт зелен без `--test-threads=1`), B10 (девять фикстур группы A получили слепок). **Не
исполнены или исполнены наполовину — пять:**

| условие `R-196` | что требовалось | что в коде на `6a8e0bb` | вердикт |
|---|---|---|---|
| №6 (B6) | бюджет вызова ПОДКЛЮЧЁН к пути выдачи; `pump_one` не читает сегмент целиком | `grep -rn feed_tail_within crates/` → определение в `crates/gateway-serve/src/admission.rs` и ТРИ вызова только из `tests/red_m87_admission.rs`; из `src/**` — ни одного. `admission.rs:394` → `f.read_to_end(&mut buf)?` — чтение сегмента ЦЕЛИКОМ, регресс класса `TD-011` | **НЕ ЗАКРЫТО** |
| №5 (B5) | счётчик меряет ПРОЧИТАННЫЕ байты журнала | `crates/gateway-serve/src/lib.rs:1202` и `:1334` → `metrics::add_journal_payload_bytes_pub(snap_text.len() as u64)` — размер ОТПРАВЛЕННОГО снимка, не прочитанного журнала. `read_dir`+`metadata` с горячего пути ушли (это закрыто), но мерится по-прежнему не то, что обещано | **ПОЛОВИНА** |
| №7 (B7) | `SlotGuard::drop` — `fetch_sub` | `fetch_sub` применён только к ЛОКАЛЬНОМУ счётчику; глобальный, который и кормит `serving_counters()`, по-прежнему `load` + `store`: `SLOTS_IN_FLIGHT_GLOBAL.load(…)` → `…store(next_global,…)`. Декременты под гонкой теряются, как и было в находке | **ПОЛОВИНА** |
| №4 (B4) | ветка «нет политики / нет чекпоинта» — fail-closed НАЗВАННЫМ исходом | `crates/gateway-serve/src/lib.rs:1056` → `if inner.cfg.checkpoint_dir.is_some() {` (иначе `readiness` не зовётся вовсе) и `:1805` → `} else { … ServingOutcome::Ready };` — fail-open на месте. Прод-путь сегодня fail-closed лишь потому, что `main.rs` всегда идёт через `bind_with_policy`; сама ветка жива и её никто не краснит | **ПОЛОВИНА** |
| №8 (B8) | разбор `journal.meta` — через API `crates/journal` | `admission.rs:256-266` → `journal_open_next_seq` читает `u64::from_le_bytes(bytes[..8])` вручную, без проверки версии. Первая половина условия (удаление `journal_list_segments_count`) исполнена | **ПОЛОВИНА** |

Существенно не только это, но и то, что **гейт зелен при всех пяти**: ни один шаг
`verify_M-87.sh` не смотрит, ЗОВЁТСЯ ли `feed_tail_within`, чем кормится счётчик байт и
теряет ли глобальный счётчик декременты. Шаги задач 4, 5, 6, 8 — грепы присутствия ИМЕНИ
(`:70`, `:77`, `:85`, `:231`), а присутствие имени и работа механизма — разные утверждения.
Это тот же класс, что §B4 и §B5, и он в поставке системный, а не единичный.

## §N1 — NOTE. В гейте есть вычисленная и выброшенная проверка

`scripts/verify_M-87.sh:337` считает `SWALLOW` (греп на `if let Ok(…)` вокруг провенанса) и
НИ РАЗУ не использует: `grep -n SWALLOW scripts/verify_M-87.sh` → одна строка, только
присваивание. Комментарий над ней объявляет проверку, решение принимают `CHP`/`HPS`. По
узкому предмету защита не страдает (`CHP == 0` сильнее для ЭТОГО дефекта), но объявленный
более широкий сторож — «молчаливое поглощение не вернётся ВООБЩЕ» — не работает.

## §N2 — NOTE. Приёмка задачи 9 до деплоя невозможна по построению

`verify` печатает по задаче 9 `SKIP` и отсылает к §8 — единственно возможный порядок:
`docker inspect` меряет ФАКТИЧЕСКИ применённые лимиты, а применить их можно только раскаткой.
Требование `R-196` №10 («`docker inspect` с прода — в Done Block») до merge неисполнимо;
исполняю его я, после merge, и без пруфа milestone не закрывается.

Базовая линия снята сейчас (прод, до раскатки):

```
ssh … docker inspect -f '{{.Name}} NanoCpus={{.HostConfig.NanoCpus}} Memory={{.HostConfig.Memory}}' hft-gateway-serve hft-recorder
/hft-gateway-serve NanoCpus=0 Memory=0
/hft-recorder       NanoCpus=0 Memory=0
```

Риск назван заранее, чтобы деплой-гейт проверил адресно: лимиты объявлены ключом
`deploy.resources.limits`, который частью инструментов вне swarm игнорируется. На проде живёт
`Docker Compose version v5.3.1` — применять обязан, но доказательство есть `NanoCpus`/`Memory`
≠ 0 ПОСЛЕ раскатки, а не версия клиента и не текст compose. Отдельно: текстовый шаг гейта
`task9` (разбор `docker-compose.yml`) зелен — то есть прокси заведомо ложно-зелёный, и
единственная настоящая проверка живёт в §8.

## §N3 — NOTE. Ссылка на вердикт `C-255`, которого нет файлом

Спека §14.1nonies названа «…`C-255`, красный страж ловушки», коммит `6e58026` ссылается на
`C-255`, а файла нет: `ls research/critiques/ | grep C-255` → пусто. `gates.md` §4: «этап
пройден только когда предъявлен ФАЙЛ». Правка задачи 19 от этого неверной не становится — её
держит оракул `u1_guard_trap_actually_traps`, — но ссылка обязана быть либо снята, либо
превращена в файл.

## §N4 — NOTE. Семь задач без шага приёмки ПО ИМЕНИ

`gates.md` §3 требует минимум одну проверку на задачу. Шаги по имени есть у задач
1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 20. Нет у: **2bis, 14, 15, 16, 17, 18, 19**.
Фактически 15/16/19 покрыты внутри чужих шагов, 17 и 18 — ТОЛЬКО агрегатом `cargo test --all`
(их падение отчиталось бы меткой «CI-паритет», а не задачей), 14 не покрыта ничем (§B6).
Отдельно `task8` (`:231-236`) считает ТЕКСТ по `*.rs` вместе с комментариями: из 9 совпадений
7 — строки `//`/`///`, то есть шаг зелен от одного комментария.

## §N5 — NOTE. Кандидат во флак

`crates/gateway-serve/tests/red_ws_session.rs` под параллельной нагрузкой пакета дал одно
падение из четырёх прогонов и не воспроизвёлся. Не доказан, но значим: через шаг
«CI-паритет» он красит весь гейт. Наблюдать; при повторе — карточка.

## Условие APPROVED

| # | что | кто |
|---|---|---|
| 1 | Оракул на ADD/SWITCH-сайт провенанса (зеркало `o6` после `subscribe`); канарейка считает ВЫЗОВЫ, а не упоминания (реэкспорт исключён) | architect (форма) → engine-dev |
| 2 | `D-1(б)` решает по коду возврата (`LIBRC=$?`), не по тексту | architect |
| 3 | Задача 14: оракул точки входа прод-бинаря (argv из `docker-compose.yml`, fail-closed разбор env) ЛИБО явное согласие на `built-not-wired` MAJOR с моей карточкой на close-out | architect |
| 4 | `ALLOW-SUBJECT-CHANGE` на правки `verify_M-87.sh` после `A-038`/`A-039`/`A-040`/`C-244`/`C-254`; `check_gate_meta.sh` на дереве слияния → exit=0 | architect |
| 5 | Ветка синхронизирована с `origin/main` (конфликт `ROADMAP`: `P0-CORR` из `main`, `SCALE` из ветки); базовая тройка CI и `verify_M-87.sh` прогнаны на ДЕРЕВЕ СЛИЯНИЯ | architect |
| 6 | §Tasks: статусы задач 1–9 и 11–19 приведены к факту; у задачи 19 названо, что `revert` отменяет ПОГЛОЩЕНИЕ, а не задачу | engine-dev |
| 7 | `SWALLOW` включён в решение либо объявление снято; `C-255` предъявлен файлом либо ссылка снята | architect / critic |
| 8 | Бюджет вызова ПОДКЛЮЧЁН (`feed_tail_within` зовётся из `src/**`); чтение сегмента порциями вместо `read_to_end` (`admission.rs:394`) — с шагом приёмки, проверяющим ВЫЗОВ, а не имя | architect (оракул) → engine-dev |
| 9 | Счётчик прочитанных байт меряет журнал, а не `snap_text.len()`; глобальный `SLOTS_IN_FLIGHT_GLOBAL` — через `fetch_sub`; ветка «нет чекпоинта» — названный исход, а не `Ready`; `journal.meta` — через API `crates/journal` | architect (форма) → engine-dev |

## Что проверено и находкой НЕ является

- **Атомарность коммитов после `R-196` исправлена**: задачи 11–20 — по одному коммиту на
  задачу, метка роли у каждого. Исторические бандлы (`7733eaa` на восемь задач и др.)
  остаются: переписывать историю нечем, `R-196` B9 это зафиксировал. Принимаю как названный
  непереписываемый долг аудит-трейла, а не как открытую находку.
- **Отчёт tester'а воспроизведён** независимым деревом, числа сошлись.
- **fmt / clippy зелены** и на ветке, и на дереве слияния: `M-88` и `M-87` в одном файле
  компилируются вместе под `-D warnings`.
- **Барьеры, кроме `gate-meta`, на дереве слияния зелены**: `protected-artifacts`,
  `artifact-ids`, `roadmap-sync`, `archived-refs`, `secret-material`, `review-fa`,
  `docs-freeze` — exit=0 у каждого.
- **Процессный слой не тронут** (`.claude/**`, `CLAUDE.md`, `04-workflow.md`) ⇒
  `FOUNDER-APPROVED` не требуется.
- **Запрещённых форм** `cmd && echo PASS || echo FAIL` и цепочек `grep … && push` в гейте нет;
  `set -uo pipefail`, FAIL-счётчик и `exit 1` при FAIL>0 на месте. Нарушение одно — §B5.

## Долги, заводимые reviewer'ом НА CLOSE-OUT (после merge, не сейчас)

1. **«built-not-wired» MAJOR** — спека §11 п.5: сторож молчания построен, поверхность экспорта
   счётчиков наружу не строится (`crates/ops/src/**` дифом не тронут). Если §B6 закроется
   согласием, а не оракулом, — вторая карточка того же класса на задачу 14.
2. **`TD-207`** — закрывается задачей 11; закрытие подтверждается прогоном зонда против
   ПРОДА на деплой-гейте, а не зелёным юнитом.
3. **Протухшие координаты `файл:строка`** в карточке `TD-207`, названные спекой §3.1.

## Done Block

```
$ pwd
/tmp/hft-reviewer-m87-r2                        ← своё дерево, detached 6a8e0bb

$ git rev-parse HEAD ; git rev-parse origin/feat/M-87-serving-circuit-breaker
6a8e0bb97ac0670ea6b00ff2c84510e5f18b98f1
6a8e0bb97ac0670ea6b00ff2c84510e5f18b98f1        ← вершина взята командой, совпала с мандатом

$ git rev-list --count origin/feat/M-87-serving-circuit-breaker..origin/main
73

# ── прогон гейта НА ВЕТКЕ ──
$ grep -E "^(fmt_exit|clippy_exit|verify_exit)" /tmp/rev_m87_gates.txt
clippy_exit=0
verify_exit=0
$ grep -cE "^PASS" /tmp/rev_m87_verify.txt ; grep -cE "^FAIL" ; grep -cE "^SKIP"
PASS=26 FAIL=0 SKIP=1
$ grep -E "^VERDICT" /tmp/rev_m87_verify.txt
VERDICT: PASS
$ grep -E "^test result" /tmp/rev_m87_tests.txt | awk "{p+=\$4; f+=\$6} END {print}"
passed=1078 failed=0 (блоков: 247)

# ── ДЕРЕВО СЛИЯНИЯ (origin/main влит в 6a8e0bb, конфликт ROADMAP разрешён; /tmp/hft-rev-m87-merge) ──
$ git merge --no-commit --no-ff origin/main
CONFLICT (content): Merge conflict in docs/ROADMAP.md
$ grep -E "^(fmt_exit|clippy_exit|tests_exit|passed=)" /tmp/mt_gates.txt
fmt_exit=0
clippy_exit=0
tests_exit=0
passed=1108 failed=0 (блоков: 252)
$ bash scripts/verify_M-87.sh ; echo exit=$?
VERDICT: PASS   exit=0   (PASS=26 FAIL=0 SKIP=1 — те же числа, что на ветке)

# ── барьеры CI на ДЕРЕВЕ СЛИЯНИЯ (EVENT_NAME=pull_request, PR_BASE_SHA=origin/main) ──
check_protected_artifacts      exit=0
check_artifact_ids             exit=0
check_gate_meta                exit=1  ← FAIL (5) subject-lock
check_roadmap_sync             exit=0
check_archived_refs            exit=0
check_secret_material          exit=0
check_review_fa                exit=0
check_docs_freeze              exit=0

# ── МУТАЦИЯ 1 (положительный контроль оракула задачи 20) ──
### МУТАЦИЯ 1 — библиотека: Err(_) => (frozen_start_seq, false)
вхождений: 1
thread 'h4_missing_journal_dir_does_not_claim_complete_history' (3175225) panicked at crates/gateway/tests/red_m87_history_provenance_failclosed.rs:260:5:
thread 'h3_unreadable_journal_does_not_claim_complete_history' (3175224) panicked at crates/gateway/tests/red_m87_history_provenance_failclosed.rs:234:5:
test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s
mut1_rc=101
откат 1: изменений 0

# ── МУТАЦИЯ 2e (ADD/SWITCH-сайт: вызов оставлен, перезапись признака снята) ──
### МУТАЦИЯ 2e — ADD/SWITCH-сайт: вызов обёртки остаётся, перезапись признака снята
сайты присваивания (1-based): [1274, 1955]
мутация на строке 1267
канарейка гейта: CHP=0 HPS=3
КАНАРЕЙКА: PASS (ложно-зелёная на мутанте)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.95s
provenance_rc=0
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
o6_rc=0
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.53s
entrypoint_rc=0
clippy_rc=0
откат 2e: изменений 0

# ── условия R-196 №4-8: предъявление КОДОМ ──
$ grep -rn feed_tail_within crates/ | cut -d: -f1 | sort | uniq -c
      2 crates/gateway-serve/src/admission.rs
      3 crates/gateway-serve/tests/red_m87_admission.rs
$ grep -n read_to_end crates/gateway-serve/src/admission.rs
394:        f.read_to_end(&mut buf)?;
$ sed -n "/impl Drop for SlotGuard/,/^    }/p" crates/gateway-serve/src/admission.rs | grep -E "load|store|fetch_sub"
        let prev_local = self.local.fetch_sub(1, Ordering::SeqCst);
        let prev_global = crate::metrics::SLOTS_IN_FLIGHT_GLOBAL.load(Ordering::SeqCst);
        crate::metrics::SLOTS_IN_FLIGHT_GLOBAL.store(next_global, Ordering::SeqCst);
$ grep -n "add_journal_payload_bytes_pub" crates/gateway-serve/src/lib.rs
1202:                    metrics::add_journal_payload_bytes_pub(snap_text.len() as u64);
1334:                metrics::add_journal_payload_bytes_pub(snap_text.len() as u64);

# ── прод (Ярус S) ──
$ gh run list --branch main --limit 1 ; ssh … rev-parse HEAD ; docker inspect
completed success  CI  main  push  36013247738
prod HEAD=46e5cc0   (origin/main=3a2e460 — прод отстаёт, деплой M-87 не начинался)
/hft-gateway-serve NanoCpus=0 Memory=0
/hft-recorder       NanoCpus=0 Memory=0
```

---

# ВЕРДИКТ: **REJECT**

Задача 20 в своей ФОРМЕ принята, но поставка не может быть влита ни по одному из трёх
независимых оснований:

1. **Механически невозможно** — `check_gate_meta` красен на дереве слияния (§B1), ветка
   отстаёт на 73 коммита и не сливается (§B2).
2. **Механизм не предъявлен прогоном там, где обещан** — второй вызыватель задачи 20 не
   пиннит никто (§B4, мутация зелена насквозь); задача 14 не имеет оракула точки входа (§B6).
3. **Пять условий APPROVED предыдущего круга не исполнены** (§B7), и гейт зелен при всех
   пяти — шаги задач 4/5/6/8 проверяют присутствие ИМЕНИ, а не работу механизма.

**Маршрут:** architect (§B1, §B2, §B4-форма, §B5, §B6, §B7-форма, §N1, §N3) → engine-dev
(§B3 статусы, исполнение §B7) → tester прогоняет ДЕРЕВО СЛИЯНИЯ → reviewer.

**Отдельно founder'у, как факт, а не просьба:** M-87 прошёл семь кругов критика, четыре
арбитража и два PR-гейта; каждый круг находит дефекты НЕ в предмете, а в том, что гейт
объявляет проверенным. Это тот же класс, что `TD-138`. Решать, что с этим делать — менять
форму шагов приёмки или число кругов, — вне зоны reviewer'а.
