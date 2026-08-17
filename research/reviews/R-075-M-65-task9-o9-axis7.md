<!-- GATE-META
milestone: M-65
audited_repo: a3ka/hft-platform
audited_base: 3c667772f32fd9d0a71ac1b7681c1c89fc82759b
audited_head: 2e342d3c441e5ff845559f9f954d30e4855249c9
verdict: APPROVED
-->

<!-- Шапка дописана 2026-08-17 architect'ом ПОСТФАКТУМ: вердикт написан до введения
     нормы GATE-META (M-60b); барьер судит все вердикты диапазона без исключений
     (grandfathering отвергнут осознанно, GM-30). Значения извлечены из самого
     вердикта: Предмет строкой :4 — «ОДИН коммит `2e342d3` на `origin/feat/M-65-ws-session`». Содержание не изменено. -->

# R-075 — M-65 task #9 / оракул `O-9` (ось 7): **APPROVED с четырьмя NOTE**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата:** 2026-08-14
**Предмет:** ОДИН коммит `2e342d3` на `origin/feat/M-65-ws-session`
(`test(M-65): task #9 - o9 pins session frames by reference [architect-codex]`)
**Спека:** `milestones/M-65-ws-session.md` §3.1 / §4.2 ось 7 / §4.6 · **Норматив:** `CT-RFC-09` §2.1-2.9
**Цепочка до меня:** `C-077` REJECT → rev2 → `C-078` NOTE → RED `540d86b` → engine-dev →
`R-057` REJECT (`b691242`) → engine-dev-фиксы Б-1/Б-2/Б-3 → architect `e25f936`/`b35e57a`/`4e05e22`/`af0f634` → **этот коммит**
**Вердикт:** **APPROVED** — предмет делает ровно то, что заявляет; доказано МОИМ мутантом, не пересказом
авторского. Четыре находки — все категории (iii)/(i) по §4.4, merge не блокируют.

> **APPROVED здесь НЕ ЕСТЬ close-out милестоуна и НЕ ЕСТЬ разрешение на merge в `main`.**
> Гейт `verify_M-65.sh` на этом же tip'е — **`VERDICT: FAIL (2)`** (§6 Done Block). Разрешено
> ровно одно: коммит остаётся на ветке как законная часть цепочки. Условия merge — §5.

---

## 0. Почему вердикт положительный, хотя гейт красный

Красное в гейте — **по замыслу спеки** (`F2` fail-closed до задачи 9) плюс один
инфраструктурный флак, к M-65 отношения не имеющий (Н-4). Предмет же этого круга — не
милестоун, а оракул: `SESSION-HANDOFF` §0bis очередь п.1 назвала дыру прямо — «ось 7 не
пиннится НИЧЕМ; `gateway::Frame` не несёт ни `venue`, ни `symbol`, ассерт `o9` слеп ПО
ПОСТРОЕНИЮ; зонд показал соединение, 13 тиков отдающее ЧУЖОЙ инструмент при зелёном оракуле».

Коммит эту дыру закрывает. Проверено не чтением: я собрал СВОЙ мутант оси 7 —
канонический `connshare` из таблицы §4.5 (реестр селекторов process-global по `sub id`) — и
он краснеет (§6.3). Авторский мутант (принудительный `symbol = "ETHUSDT"` в разборе
`subscribe`) я НЕ перепрогонял намеренно: он бьёт скорее в ось 1, и подтверждать оракул
чужим выбором мутанта — значит проверять отчёт, а не предмет.

---

## 1. Block-scope — ✅ ПРОЙДЕН

| проверка | результат |
|---|---|
| диф коммита | ✅ РОВНО один файл: `crates/gateway-serve/tests/red_ws_session.rs` (`148 20`) |
| зона architect'а (спека §2: `crates/gateway-serve/tests/**` — RED-набор) | ✅ соответствует; dev тестов не трогал |
| `crates/gateway/src/**` (запретный список §5) | ✅ 0 файлов — ни в коммите, ни во всём диапазоне `b691242..2e342d3` |
| `crates/contracts/**`, `risk`/`killswitch`/`oms`/`venue-*` | ✅ 0 файлов |
| `ports:` в `docker-compose.yml` | ✅ не тронут этим коммитом |
| процессный слой (`.claude/**`, `CLAUDE.md`, `docs/04-workflow.md`) — замок §11 | ✅ 0 файлов; токен `FOUNDER-APPROVED` не требуется |
| `scripts/verify_M-65.sh` (sacred, architect-only) | ✅ в диапазоне не менялся ВОВСЕ — гейт судит набор, а не подстроен под него |
| sacred `*/tests/**` — кто трогал в диапазоне | ✅ только architect-роли: `e25f936`, `b35e57a`, `0a26df6`, `2e342d3` |
| carve-out статус-колонки (`88aafe2` [engine-dev] правит `milestones/`) | ✅ диф — РОВНО колонка Status семи строк §Tasks, ничего иного |
| revert-пара `b35e57a` → `4e05e22` (снос чужой работы) | ✅ восстановление подтверждено ЗАМЕРОМ: `git diff --stat 88aafe2..af0f634` = 1 файл (тесты, 61+/19−); `src/**` engine-dev'а цел |

## 2. Block-C (контракты T1) — ✅ N/A

`crates/contracts/**` не тронут; contract-RFC не требуется. Проводная форма `CT-RFC-09`
§2.3 набором НЕ расширяется — наоборот: этот коммит продолжает линию решения §4.2bis
(эталон вместо синтетического кадра), то есть работает в сторону контракта, а не от него.

## 3. Block-risk — ✅ N/A, проверено дифом

`risk`/`killswitch`/`oms`/`venue-*` не тронуты. `gateway-serve` — read-only консюмер журнала
(`GS-I-3`), order-egress отсутствует; MD-only carve-out `gates.md` §5 применим. risk-critic
не требуется. Ярлыку милестоуна не доверял — смотрел `git diff --name-only`.

## 4. Block-DoneBlock — ✅ сырой, и ✅ ВОСПРОИЗВЁЛСЯ

Handoff §C несёт команды и exit-коды, а не пересказ. Все шесть утверждений перепрогнаны мной
независимо (§6) и сошлись. Расхождений отчёта с git и с прогонами — нет.
Единственное расхождение — не в числах, а в статусе: см. Н-1.

---

## 5. Что именно закрыто — и чем это доказано

**Было** (до коммита): ассерты `!a_tail.is_empty()` + `!a_body.contains("ETHUSDT")` —
проверка ПРИСУТСТВИЯ строки в JSON-дампе. Против дефекта «A получает содержимое B в кадрах
без поля `symbol`» слепа по построению: `gateway::Frame` инструмент не несёт.

**Стало** (`red_ws_session.rs:963-1029`): якорь — селектор, который КЛИЕНТ послал в
`subscribe` (`wire_snapshot:409` перетирает `Snapshot.selector` до `apply`, чтобы серверная
самоаттестация не участвовала в сверке), затем
`snapshot(A) ⊕ frames(A) == gateway::snapshot(dir, filter, selector_A, Cursor::at(to))` —
эталон НЕЗАВИСИМЫМ путём по §4.6 (полная свёртка против инкрементального `LiveReducer`
сервера), плюс негатив на ТОЙ ЖЕ фикстуре и цепочечный ассерт `frame.from == acc.cursor`.

**Покрытие оси 7 после коммита — ЗАМЕРЕНО, а не заявлено:**

| значение оси 7 | чем стережётся | мой замер |
|---|---|---|
| `подписка другого соединения меняет выдачу текущего` (V) | позитив: все кадры A рождены ПОСЛЕ подписки B, и обязаны сойтись с эталоном A | мутант `connshare` роняет o9, exit=101 (§6.3) |
| `одинаковый sub id в двух соединениях делит состояние` (V) | негатив: кадры B, применённые к снапшоту A, обязаны РАЗОЙТИСЬ с эталоном A | ровно этот ассерт и покраснел на мутанте |
| `два соединения ... дают независимые потоки` (L) | позитив A + расхождение B | покрыто ЧАСТИЧНО — см. Н-3 |

**Условия merge M-65 в `main` (ни одно не выполнено на `2e342d3`):**
1. задача 9 — **батарея** `scripts/tests/red_ws_session_battery.sh` — не существует; шаг `F2` красен fail-closed (это работает как задумано, `gates.md` §2);
2. ветка отстаёт от `origin/main` на **195** коммитов; предмет обязан проверяться на дереве слияния (`gates.md` §8) — я это сделал (§6.5), но перед merge прогон повторяется на актуальном `main`;
3. блокеры `R-057` Б-1/Б-2/Б-3 закрывались коммитами `f3c9668`/`eb0b450`/`d7c1691`, и **PR-гейт по ним не проходил** — этот вердикт их НЕ засчитывает: предмет круга иной. Нужен отдельный круг reviewer'а по диапазону `b691242..HEAD`.

---

## 6. Находки

### Н-1 (NOTE, дисциплина) — коммит называет себя «task #9», которым он не является

**Где:** subject `2e342d3` + Handoff §A «Статус: DONE».
**Факт:** §Tasks строка 9 — это `Батарея мутантов scripts/tests/red_ws_session_battery.sh (§4.5)`.
Батареи нет (`ls` → `No such file or directory`), шаг `F2` красен, `verify_M-65.sh` → `VERDICT: FAIL (2)`.
Сделанное — ремонт оракула `O-9`, то есть **задача 7** (RED-набор).
**Почему это не придирка:** §Tasks и статус-строки читаются следующей ролью как правда, и проект
уже платил за расхождение бухгалтерии с фактом (M-62 §Tasks, `TD-140`, `R-070`). «Статус: DONE»
в §A при красном гейте — ровно то, что `commit-discipline.md` запрещает («работа в процессе НЕ
называется done»); §E при этом честно называет и отсутствие батареи, и непрогнанный verify —
поэтому NOTE, а не блокер: сокрытия нет, есть неверный ярлык.
**Что требуется:** строку 9 §Tasks НЕ переводить в DONE; следующий handoff по этому предмету
несёт `IN_PROGRESS`; при написании батареи в её коммите назвать, что задача 9 — это она.
Правка §Tasks — зона architect'а, не моя.

### Н-2 (NOTE, потеря покрытия «в довесок») — самоаттестацию `Snapshot.selector` теперь не стережёт НИКТО

**Где:** `crates/gateway-serve/tests/red_ws_session.rs:405-410` (`snap.selector = client_selector.clone()`).
**Решение верное:** серверу, чьё смешение подписок проверяется, нельзя верить на слово — якорем
обязан быть клиентский селектор. Но вместе со старым `!a_body.contains("ETHUSDT")` исчез
ЕДИНСТВЕННЫЙ во всём наборе ассерт, связывающий проводное поле `data.selector` с запрошенным:
`grep 'selector' red_ws_session.rs` не даёт ни одной сверки ни в одном оракуле.
**Чем это платится:** сервер, отдающий ПРАВИЛЬНОЕ содержимое под ЧУЖОЙ подписью селектора,
проходит весь набор. Фронт рисует заголовок «ETHUSDT» над графиком BTC — расхождение с
`CT-RFC-09`, невидимое гейту.
**Фикс стоит одной строки и БЕСПЛАТЕН — это ЗАМЕРЕНО, а не предположено** (§6.4): вставка
`assert_eq!(&snap.selector, client_selector)` ПЕРЕД перетиранием проходит сегодня (o9 ok, exit=0)
— то есть сервер и клиент уже согласны, а якорь Р-Б от этого не страдает: сверка содержимого
остаётся на клиентском селекторе.
**Категория (iii)** по §4.4 — новой оси не вводит.

### Н-3 (NOTE, полнота) — сторона B не пришпилена к СВОЕМУ эталону

**Где:** `red_ws_session.rs:963-1029`; `snap_b` используется РОВНО один раз — в setup-ассерте
равенства курсоров (`:975`), содержимое B с эталоном не сверяется никогда.
**Что из этого следует:** манифест заявляет L-значение `два соединения с одинаковым sub id и
разными селекторами дают независимые потоки` (`MANIFEST` строки o9/7/L), но исполняется
половина: A сходится с эталоном бит-в-бит, а B доказано лишь «не равен эталону A». Реализация,
отдающая B нечто третье (устаревшее, обрезанное), набор проходит.
**Цена фикса — пять строк на УЖЕ существующей фикстуре** (ETHUSDT дозаписывается тем же
`append_more`): `apply_frames(snap_b, &b_frames, ...) == reference_at(dir, &sel_b, ...)`.
**Категория (i)** по §4.4 — значение известной оси, круг короток. Не блокер: доминирующий
режим отказа (`connshare`) замером пойман (§6.3).

### Н-4 (NOTE → **TD-151**, целостность гейта) — `verify_M-65.sh` шаг `T` краснеет от ОКРУЖЕНИЯ

**Где:** `crates/journal/tests/red_retention.rs:51-56`
(`disk_guard_halts_writes_explicitly_when_free_space_is_low`).
**Замер:** в моём прогоне `verify_M-65.sh` шаг `T` (`cargo test --all`) упал именно на нём —
`2 passed; 1 failed`. **Это НЕ M-65:** файл в диапазоне `b691242..2e342d3` не тронут (0 коммитов),
и в одиночном прогоне тест GREEN **3/3** (§6.6).
**Механизм:** порог берётся как `free_bytes(dir) + 1`, то есть тест ждёт, что guard сработает.
Между замером `free` и `append` свободное место на хосте может ВЫРАСТИ (у меня рядом шли три
параллельные сборки `cargo` на том же `/dev/md2`, кэши освобождались) — тогда `append` проходит,
и тест падает по причине, к своему инварианту отношения не имеющей.
Это `testing.md` §«Целостность гейта» свойство 2: оракул обязан мерить СВОЙ инвариант, а не
окружение. Родственный `TD-026` закрыт правкой ДРУГОГО теста и в своём close-out прямо
отсылает «disk-guard проверяется отдельно в `red_retention`» — то есть указывает ровно на
этот, оставшийся env-чувствительным.
**Заведено:** `TD-151` в `TECH-DEBT.md` (зона фикса — architect: sacred-тест).
**Не блокер этого круга:** предмет — тест gateway-serve; шаг `T` при повторе зелен.

---

## 7. Done Block (сырой вывод, агрегированный по `commit-discipline.md`)

### 7.1 Предмет и чистота дерева

```
$ git log -1 --oneline
2e342d3 test(M-65): task #9 - o9 pins session frames by reference [architect-codex]

$ git show --numstat --format='' HEAD
148	20	crates/gateway-serve/tests/red_ws_session.rs

$ git status --porcelain
{пусто}

$ git diff --name-only b691242..2e342d3 | grep -E "crates/(gateway/src|contracts|risk|killswitch|oms|venue-)"
{пусто — запретные зоны не тронуты}

$ git rev-list --count 2e342d3..origin/main
195
```

### 7.2 Набор — зелёный и НЕ флакающий

```
$ cargo test -p gateway-serve --test red_ws_session
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.89s
TEST_EXIT=0

$ for i in $(seq 1 10); do cargo test ... o9_connections_are_isolated | grep "^test result"; done
      1 test result: ok. 1 passed; ... finished in 8.07s
      1 test result: ok. 1 passed; ... finished in 8.08s
      4 test result: ok. 1 passed; ... finished in 8.09s
      2 test result: ok. 1 passed; ... finished in 8.10s
      1 test result: ok. 1 passed; ... finished in 9.24s
      1 test result: ok. 1 passed; ... finished in 9.31s
(10/10 ok — флака нет; для сравнения `R-057` фиксировал 4 падения из 10 на O-10 до фикса Б-3)
```

### 7.3 Мутационный контроль — МОЙ мутант, канонический `connshare` (§4.5, ось 7)

Мутация в КОПИИ дерева (`/tmp/hft-rev-m65-mut1`, отдельный `CARGO_TARGET_DIR`), прод-исходники
не тронуты. Внесено в `crates/gateway-serve/src/lib.rs:674` (сразу после `validate_selector`):
реестр селекторов — **process-global по `sub id`** (`static SHARED: Mutex<HashMap<String, Selector>>`,
`entry(id).or_insert(sel)`), то есть ровно запрет §5 «состояние подписок, общее на ПРОЦЕСС или
на пространство `sub id`».

```
$ CARGO_TARGET_DIR=/tmp/hft-rev-m65-mut1-target cargo test -p gateway-serve --test red_ws_session o9_connections_are_isolated -- --nocapture
thread 'o9_connections_are_isolated' panicked at crates/gateway-serve/tests/red_ws_session.rs:1023:5:
assertion `left != right` failed: O-9 NEGATIVE: чужие кадры B, применённые к снапшоту A, сошлись с эталоном A. Оракул снова слеп к cross-talk по содержимому.
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 11 filtered out; finished in 8.09s
MUT1_EXIT=101
```

Мутант убит НЕГАТИВНЫМ контролем — тем самым ассертом, которого в оракуле не было до этого
коммита. Обратная проверка (тот же прогон без мутации) — §7.2, GREEN.

### 7.4 Зонд к Н-2 — усиление бесплатно

```
(в дереве merge-preview, до перетирания вставлен assert_eq!(&snap.selector, client_selector))
$ cargo test -p gateway-serve --test red_ws_session o9_connections_are_isolated
test result: ok. 1 passed; 0 failed; ... finished in 8.08s
PROBE_EXIT=0
```

### 7.5 Дерево СЛИЯНИЯ (`gates.md` §8) — набор зелен и там

`origin/main` ушёл на 195 коммитов; конфликт слияния РОВНО один и он документный
(`docs/SESSION-HANDOFF.md`). Важно: `crates/gateway/src/lib.rs` в `main` изменён (+70/−10) —
а это источник НЕЗАВИСИМОГО эталона `gateway::snapshot`, на котором держится новый `O-9`.

```
$ git merge --no-edit origin/main
CONFLICT (content): Merge conflict in docs/SESSION-HANDOFF.md   ← единственный
$ CARGO_TARGET_DIR=/tmp/hft-rev-m65-mp-target cargo test -p gateway-serve --test red_ws_session
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.52s
MERGEPREVIEW_EXIT=0
```

### 7.6 Acceptance-гейт milestone'а — красный, и это разобрано

```
$ bash scripts/verify_M-65.sh
PASS  A crates/gateway-serve/tests/red_ws_session.rs на месте
PASS  A форматирование (паритет с CI-шагом fmt)
PASS  N манифест ⇄ §4.2 совпал в обе стороны; у каждой из восьми осей есть легитимный сценарий
PASS  F набор GREEN: исполнено 12 оракулов (ожидалось ≥ 11: o0 + O-1..O-10)
FAIL  F2 батареи scripts/tests/red_ws_session_battery.sh НЕТ — анти-плацебо не предъявлено (задача 9, пишется architect'ом ПОСЛЕ задач 1-6)
PASS  L невалидный лимит «0» ⇒ отказ старта (exit=2)
PASS  L невалидный лимит «-1» ⇒ отказ старта (exit=2)
PASS  L невалидный лимит «abc» ⇒ отказ старта (exit=2)
PASS  M соседние оракулы gateway-serve GREEN (10 тестов)
PASS  T clippy
FAIL  T cargo test --all
      ↳ test disk_guard_halts_writes_explicitly_when_free_space_is_low ... FAILED
      ↳ test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
VERDICT: FAIL (2 нарушений)
VERIFY_EXIT=1

$ for i in 1 2 3; do cargo test -p journal --test red_retention disk_guard_... ; done
test result: ok. 1 passed; 0 failed; ... finished in 0.03s
test result: ok. 1 passed; 0 failed; ... finished in 0.01s
test result: ok. 1 passed; 0 failed; ... finished in 0.02s
$ git log --oneline b691242..2e342d3 -- crates/journal/tests/red_retention.rs | wc -l
0
```

`F2` — красное ПО ЗАМЫСЛУ (§6bis базовой линии: «до задачи 9 шаг красен fail-closed»).
`T` — Н-4/`TD-151`, к предмету отношения не имеет.

---

## 8. Что я НЕ проверял (границы этого вердикта — названы явно)

- **Закрытие `R-057` Б-1/Б-2/Б-3 не засчитано.** Мультиплекс (`f3c9668`), надгробие
  `draining_ids` (`eb0b450`), снятие синтетического heartbeat'а (`d7c1691`) — я видел их
  оракулы зелёными (`o2`, `o10`, `o11`), но мутационно не проверял: предмет круга иной.
  Нужен отдельный PR-гейт по диапазону `b691242..HEAD`.
- **Прод (`gates.md` §8 п.2) не смотрел** — ничего не мержилось, деплой не запускался.
- **Авторский мутант не перепрогонял** — вместо него свой, более строгий по оси (§0).

---

## 9. Условие APPROVED

Коммит принимается как есть; правок в нём не требуется. Требуется — при следующем шаге
предмета (написание батареи, задача 9):
1. Н-1: §Tasks строка 9 не помечается DONE до появления `red_ws_session_battery.sh`;
2. Н-2 и Н-3 закрываются в том же коммите набора (обе — правки `tests/**`, зона architect'а,
   суммарно ~6 строк), либо явно отклоняются с основанием в спеке;
3. `TD-151` живёт в `TECH-DEBT.md` до фикса; шаг `T` `verify_M-65.sh` перед close-out
   прогоняется на незанятом хосте, чтобы красное не читалось как «наверное, флак».

**Push-статус:** вердикт закоммичен и запушен на `feat/M-65-ws-session` ДО завершения работы
(`gates.md` §4: вердикт в переписке аудит-трейлом не является).
