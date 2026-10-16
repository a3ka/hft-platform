<!-- GATE-META
milestone: M-65
audited_repo: a3ka/hft-platform
audited_base: a9f0bf511a0a1de5433b0de412cb55234027b56a
audited_head: bc19ee772448f2af0afa6dd48b7b00e21526918f
verdict: REJECT
-->

# R-093 — M-65 ws-session, PR-гейт круг 2 (после `R-086`): **REJECT**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-17
**Предмет:** PR #29, ветка `fix/M-65-battery-recalibration`, вершина `bc19ee7`;
диапазон `a9f0bf5..bc19ee7` (47 коммитов, из них после `R-086` — семь: `0fd7380`, `b623470`,
`d74aedd`, `162effe`, `5405603`, `cdce6e3`, `1daf2a8`, `bc19ee7` + merge `c4d3561`).
**Дерево:** `/tmp/rev-m65`, detached на `bc19ee7`, чистое. Общий чекаут не тронут.
**Спека:** `milestones/M-65-ws-session.md` · **Норматив:** `CT-RFC-09` §2.1-2.9
**Цепочка:** `C-077` REJECT → `C-078` NOTE → `R-057` REJECT → `R-075` APPROVED →
`R-080` APPROVED → **`R-086` REJECT** (блокер: гонка switch × in-flight pump) → круг 3
(`b623470` развязки А+Б) → **этот вердикт**.

**Вердикт: REJECT.** Реализация блокера `R-086` §2 сделана и, по моему чтению, сделана
ВЕРНО — но **три из четырёх условий снятия REJECT'а, названных `R-086` §5, не выполнены**,
и главное из них не выполнено по существу, а не по форме: **защиты не пиннит НИЧТО**.
Мутационный контроль — мой, не пересказ — роняет обе развязки поодиночке, и все двенадцать
оракулов остаются ЗЕЛЁНЫМИ (§3.1). `R-086` §5 говорит прямо: «Merge в `main` до выполнения
п.1-3 запрещён».

---

## 1. Block-scope

| проверка | результат |
|---|---|
| крейты, тронутые диффом | ТОЛЬКО `crates/gateway-serve/**` (`git diff --name-only \| cut -d/ -f2 \| sort -u`) |
| `crates/{risk,killswitch,oms,venue-*}/**` | не тронуты |
| `crates/contracts/**` | **не тронут** → Block-C не применяется |
| `crates/gateway/src/**`, `crates/journal/**` (forbidden §2) | не тронуты |
| `ports:` в compose (forbidden §2) | не появились |
| `CT-RFC-09` нормативная часть §2.1-2.9 | **не изменена**; диф трогает ТОЛЬКО §6 (внесение подписей founder'а) — §2 спеки это architect'у прямо разрешает |
| RED-first | ни один коммит `[engine-dev]` не трогает `crates/gateway-serve/tests/**`; sacred-зона цела |
| ролевые метки в subject'е | 45 из 47; без метки — `c4d3561` (merge, законно) и `b691242` (уже названо `R-086` N-12) |
| **правка чужих артефактов гейта** | **находка Н-3 ниже**: `1daf2a8`/`bc19ee7` `[architect]` правят `research/critiques/**` и `research/reviews/**` — вне §2 Allowed paths и вне строки architect'а в `scope-guard.md` |

**Block-scope: ЧИСТО, кроме Н-3** (не блокер — разбор в §4).

## 1bis. Block-risk — триггер НЕ СРАБАТЫВАЕТ

`gates.md` §5 привязан к путям `crates/risk|killswitch|oms|venue-*|contracts` — ни один
диффом не тронут, поэтому risk-critic не требуется по несрабатыванию самого триггера, а не
по carve-out'у. Дополнительная проверка духа правила пройдена МОИМ прогоном:

```
$ git diff a9f0bf5 bc19ee7 -- crates/ | grep -E "^\+" \
    | grep -nE "submit|place_order|new_order|cancel_order|hmac|api_key|secret_key|\.post\("
{пусто — order-egress отсутствует}
```

**Block-risk: PASS.** Путь к деньгам не затронут.

## 1ter. Предъявление FA (`gates.md` §4)

Диф трогает `crates/gateway-serve/**`. **Собственной FA у крейта НЕТ** — и это, per
`docs/workflow/reading-map.md` §2 (ярус B, строка `derive`/`recorder`/`gateway-serve`), сам по
себе долг, который я обязан назвать, а не обойти молчанием. Опора — `docs/fa/viz-backend.md`.

Живой инвариант, применимый к диффу и проверенный мной на этой ревизии:

> **`VB-I-3`** (`docs/fa/viz-backend.md:117`): «Read Gateway read-only: grep-канарейка —
> gateway не импортирует journal-writer/recorder-write; recorder не зависит от gateway».

Проверка на `bc19ee7`:

```
$ grep -rnE "journal::(Writer|WriterConfig)|journal_writer|recorder::" crates/gateway-serve/src/
{пусто — writer не импортируется}
```

`VB-I-3` цел. (Замер по FA: `grep -oE "GS-I-[0-9]+" docs/fa/viz-backend.md | sort -u` даёт
РОВНО `GS-I-4` — семейство `GS-I`, на которое ссылаются `scope-guard.md` и `R-086` §1bis
(`GS-I-1`, `GS-I-3`, `GS-I-5`), в FA не заведено. Это отдельный долг документации, назван
здесь, заведён мной TD-записью при ближайшем close-out.)

## 1quater. Ярус C — что я искал грепом

`TECH-DEBT.md` и `PROJECT-STATE.md` — ярус C, читаются запросом, а не целиком
(`reading-map.md` §2, профиль reviewer'а). Искал: `gateway-serve`, `M-65`, `ws-session`,
`TD-039`, `TD-097`, `TD-151`, `TD-155`, `TD-124`.

- `TECH-DEBT.md`: записей о `gateway-serve`/`M-65`/`ws-session` **нет ни одной**;
- `PROJECT-STATE.md`: `M-65` не упоминается — состояния «реализовано» ещё не заявлено, и это
  верно;
- `TD-039` (OOM снапшота при `window_ms = None`) жив и блокирует M-28/M-36 (`TECH-DEBT.md:1927`,
  `:1935`) — прямо относится к находке Н-5 ниже.

---

## 2. Что я ПОДТВЕРЖДАЮ — блокер `R-086` §2 устранён ПО КОНСТРУКЦИИ

Это не уступка: реализация круга 3 сделана по спеке §10.2 и, насколько показывает чтение,
сделана правильно. Проверено мной построчно на `bc19ee7`:

1. **Развязка А исполняется.** Тик берёт `live` опционально, `Sub` остаётся в карте:
   `lib.rs:1095-1096` — `let Some(mut live) = inner.subs.get_mut(&id).and_then(|s| s.live.take())`.
   Ветка `subs.remove(&id)` из `R-086` §2.3 в дереве отсутствует.
2. **Ветка SWITCH стала достижимой.** `lib.rs:724` `if inner.subs.contains_key(id.as_str())` —
   при `live: None` ключ на месте, `contains_key == true`, значит `subscribe` во время pump'а
   идёт по SWITCH, а не ADD. Это ровно корень, названный `R-086` §2.3 шагом 3.
3. **Развязка Б исполняется НАСТОЯЩИМ инкрементом.** `gens` живёт ВНЕ `Sub`
   (`lib.rs:521` `gens: BTreeMap<String, u64>`); switch инкрементирует атомарно с заменой
   записи (`lib.rs:792-796` `.entry(...).and_modify(|g| *g += 1).or_insert(1)`), unsubscribe
   удаляет (`lib.rs:942`), pump фиксирует на старте (`:1103`) и сверяет на возврате
   (`:1192-1197`). Замер `R-086` §2.1 («`generation` не инкрементируется нигде») на этой
   ревизии больше не воспроизводится.
4. **Утечка ёмкости закрыта по построению.** Отдельного `subs_count` нет как класса; лимит
   считается `subs.len()` (`grep -n "subs_count" crates/gateway-serve/src/lib.rs` → пусто).
5. **Обе копии цикла симметричны.** Тот же контур повторён в legacy-пути
   (`lib.rs:1484-1500`, `:1551-1560`, `:1588-1597`) — рассогласования двух реализаций, которое
   было бы естественным дефектом такой правки, НЕТ.
6. **`R-086` N-1 закрыт.** `session.rs:37-61` описывает исполняемый код: `live: Option<...>`,
   `generation` вынесен в `SessionInner::gens`, запрет «чинить инкрементом внутри `Sub`» назван
   явно. Документация модуля больше не обещает механизма, которого на пути нет.
7. **Перекалибровка батареи (`§4.5bis`) — разбор, а не подгонка.** Различение «мутант стал
   невыразимым» против «сместился якорь» сделано замером, отвергнутый кандидат `capleak`
   назван вместе с ПРИЧИНОЙ отказа (ловится по неверному ассерту оси 8, а не по ёмкости).
   Это добросовестная работа, и я её не оспариваю.
8. **GATE-META backfill добросовестен** — разбор в §4, Н-3.

Иными словами: **дефект вылечен, но лечение ничем не удержано.** Дальше — почему этого
недостаточно.

---

## 3. БЛОКЕРЫ

### Б-1 — `R-086` §5 п.1 НЕ ВЫПОЛНЕН: оракула на находку нет, защита не пиннится НИЧЕМ

`R-086` §5 п.1 и `testing.md` («Исправление по вердикту тоже требует оракула») требуют, чтобы
устранение было предъявлено **оракулом на саму находку**: «фикстура обязана переключать
селектор, пока pump В ПОЛЁТЕ, а не после `settle`-паузы».

**Оракула нет, и это признано самим architect'ом** — `milestones/M-65-ws-session.md` §Tasks
задача 12: статус **⏳ OPEN**. Честность формулировки я отмечаю отдельно и засчитываю: спека
прямо называет причину («точка синхронизации под `#[cfg(test)]`, невидима из
`crates/gateway-serve/tests/**`, замер: `E0433 could not find test_sync`»). Но признанный
пробел остаётся пробелом: условие снятия REJECT'а именно этим пунктом и было.

Замер, подтверждающий отсутствие:

```
$ git log --oneline 03540b7..bc19ee7 -- crates/gateway-serve/tests/
{пусто — набор оракулов не менялся с круга R-086}

$ sed -n '557p' crates/gateway-serve/tests/red_ws_session.rs
    let _settle_before_switch = drain(&mut ws, 2 * GRACE_MS + 600).await;
```

Строка `:557` — ровно та, которую `R-086` §2.6 назвал гасителем оси 4. Она на месте
нетронутой, а `claims("o1", 4, "кадр в полёте приходит после смены")` (`:543`) продолжает
заявлять покрытие, которого фикстура не исполняет. **Номинальное покрытие оси 4 не
исправлено; оно перешло в новый круг в том же виде.**

#### 3.1 Мутационный контроль — МОЙ прогон, обе развязки не удержаны

Процедура `testing.md` §«Мутационный контроль»: нейтрализовать строки, которые оракул обязан
защищать, и прогнать набор.

**Сначала — ДОСТИЖИМОСТЬ мутируемого пути** (без неё зелёный результат мутации ничего не
значит: он был бы совместим с «путь просто не исполняется»). Проба-паника на `lib.rs:1197`:

```
$ sed -i '1197s/.*/... let live_keeps = ...; if true { panic!("REACHABILITY PROBE: ... "); }/' \
    crates/gateway-serve/src/lib.rs
$ cargo test -p gateway-serve --test red_ws_session 2>&1 | grep -E "^test |REACHABILITY"
test o1_subscribe_switches_instrument_and_old_frames_stop ... FAILED
test o7_selector_validation_keeps_connection_and_neighbours_alive ... FAILED
test o4_subscription_cap_is_fail_closed ... FAILED
test o11_frames_come_from_journal_not_synthesis ... FAILED
test o10_unsubscribe_stops_sub_and_frees_capacity ... FAILED
test o2_multiplex_subscriptions_are_independent ... FAILED
test o9_connections_are_isolated ... FAILED
REACHABILITY PROBE: pump-completion Ok-path reached, drained=false current_gen=Some(0) gen_at_pump=0
```

**Путь достижим — семь оракулов из двенадцати его исполняют.** И тут же виден второй факт,
решающий для оценки покрытия: во ВСЕХ наблюдениях `current_gen == Some(0)` и `gen_at_pump == 0`.
То есть набор ни разу не приводит систему в состояние, где генерации РАСХОДЯТСЯ, — а именно
расхождение и есть предмет защиты.

**Мутация 1 — нейтрализация развязки Б** (сверка генераций выброшена):

```
$ sed -n '1197p' crates/gateway-serve/src/lib.rs
    let live_keeps = !drained; let _ = (current_gen, gen_at_pump); // MUTANT: развязка Б нейтрализована
$ cargo test -p gateway-serve --test red_ws_session 2>&1 | grep "^test result"
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.59s
```

**Мутация 2 — нейтрализация развязки А** (in-flight подписка снова считается отсутствующей,
то есть `subscribe` во время pump'а снова уходит по ADD — БУКВАЛЬНО дефект `R-086` §2.3):

```
$ sed -n '724p' crates/gateway-serve/src/lib.rs
    if inner.subs.get(id.as_str()).is_some_and(|s| s.live.is_some()) { // MUTANT: развязка А нейтрализована
$ cargo test -p gateway-serve --test red_ws_session 2>&1 | grep "^test result"
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.59s
```

Дерево восстановлено после каждой мутации (`git status --porcelain` пуст, проверено).

**Вывод.** Ни одна из двух развязок не удерживается набором: каждую можно снять поодиночке, и
гейт останется зелёным. Батарея этого не меняет — в её таблице §4.5 тринадцать мутантов, и
**ни один не адресует ось «switch пришёл, пока pump в полёте»** (проверено чтением таблицы;
`stalefeed` правит ветку SWITCH, а не развилку ADD/SWITCH). Батарея наследует слепоту набора
по построению — ровно как и предупреждал `R-086` §6.1.

Практическое следствие, а не формальное: **следующая правка в этой области вернёт дефект
молча.** Именно от этого класса и защищает требование «оракул на находку».

### Б-2 — `test_sync.rs`: 174 строки, не исполняемые НИ В ОДНОЙ сборке (built-not-wired)

Это находка круга, а не повторение `R-086`.

`crates/gateway-serve/src/test_sync.rs` (174 строки) объявлен под `#[cfg(test)]`
(`lib.rs:77-78`), и его единственные вызовы — тоже внутри `#[cfg(test)]`-блоков
(`lib.rs:1116-1120`, `:1503-1507`). В Rust `cfg(test)` истинен ТОЛЬКО при сборке unit-тестов
самого крейта; интеграционные тесты в `tests/**` линкуются с ОБЫЧНОЙ сборкой библиотеки, где
этих блоков нет. Замер:

```
$ cargo test -p gateway-serve 2>&1 | grep -A1 "unittests src/lib.rs"
     Running unittests src/lib.rs (target/debug/deps/gateway_serve-be368b180c46acd3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ grep -rn "rendezvous\|pump_signal_and_wait\|test_wait_for_pump\|test_release" \
    crates/gateway-serve/tests/
{пусто}
```

Unit-тестов в крейте **ноль**. Значит `rendezvous` не исполняется ни в интеграционной сборке
(там его нет), ни в unit-сборке (там его некому позвать). Механизм, написанный ради оракула,
не работает **ни при какой форме вызова**.

Это тот же класс, который в этом модуле предъявлялся уже ДВАЖДЫ: `R-057` М-1/М-2 (мёртвая
`session::Session` обещала инварианты) и `R-086` N-1 (`generation`, документированный и не
инкрементируемый). Здесь он воспроизведён в третий раз, и заметно ХУЖЕ: шапка `test_sync.rs`
формулирует «Контракт» из четырёх пунктов и «Cleanup policy (BINDING)» для теста, которого не
существует и который в этой форме существовать не может. `gates.md` §4 «Механизм на пути
(DoD)» на такой случай и написан: механизм несущего пути мержится с подключением, доказанным
оракулом точки входа, либо с TD-записью «built-not-wired» severity MAJOR. Здесь нет ни того,
ни другого.

Спека это ЗНАЕТ (задача 12 называет `E0433` дословно) — и всё равно код закоммичен в
предмет PR'а как есть. Знание о непригодности механизма, зафиксированное в спеке, не делает
сам механизм пригодным.

### Б-3 — `R-086` §5 п.3 НЕ ВЫПОЛНЕН: N-2/N-3/N-4 не исправлены и не переведены в норматив

`R-086` §5 п.3: «либо реализация приведена к §2.1-2.9, либо норматив изменён contract-RFC'ом;
умолчание не годится». Решения по всем трём приняты и подробно записаны
(`milestones/M-65-ws-session.md` §11), но **ни одно не исполнено**. Цитаты норматива
проверены мной ОТКРЫТИЕМ `docs/rfc/CT-RFC-09-ws-session.md`, не пересказом:

| # | норматив (проверен чтением) | код на `bc19ee7` | решение §11 | сделано? |
|---|---|---|---|---|
| N-2 | §2.6: «`max_subscriptions_per_connection` — конфиг, **отсутствие**/невалидное значение ⇒ **отказ старта**» | `lib.rs:1855` `None => 16_usize` | узкий contract-RFC + `red_max_subs_config.rs` + правки `:168`/`:1697`/`:1699` | **НЕТ.** Нового RFC на ветке нет (`git diff --name-only -- docs/rfc/ docs/contract-rfc/` → только `CT-RFC-09`, и там правлен ТОЛЬКО §6); файла `red_max_subs_config.rs` нет; `lib.rs` не тронут |
| N-3 | §2.8: «`initial_subscribe_grace_ms` (**конфиг**, дефолт 250 ms)» | `lib.rs:540` `const GRACE_MS: u64 = 250;` | «править реализацию — читать из окружения» | **НЕТ** |
| N-4 | §2.3: `{"type":"error",…,"sub":"<id>\|null",…}` | `wire_v1.rs:193` `"sub": sub.unwrap_or("null")` — строка `"null"`, не литерал | «править реализацию (`Value::Null`); **оракул обязателен**» | **НЕТ** |

Отдельно: §Tasks не содержит НИ ОДНОЙ задачи на §11 и §12. Решения приняты, в план не
превращены — поэтому их невыполнение и оказалось ненаблюдаемым (см. Н-2).

### Б-4 — `R-086` §5 п.4 НЕ ВЫПОЛНЕН: мёртвый код на месте, TD не заведён

`milestones/M-65-ws-session.md` §12 решает прямо: «удалить, не заводить TD… удаление стоит
минут». Замер на `bc19ee7`:

```
$ grep -n "NotTextPayload\|MalformedSelector\|fn version\|fn id(" crates/gateway-serve/src/wire_v1.rs
51:    pub fn version(&self) -> u64 {
59:    pub fn id(&self) -> &str {
73:    NotTextPayload,
88:    MalformedSelector(String),

$ grep -n "_grace_ms" crates/gateway-serve/src/lib.rs
620:        _grace_ms: u64,
```

N-5, N-6, N-8 — на месте. Ни удаления, ни TD-записи (`TECH-DEBT.md` по `gateway-serve` пуст).
Решение §12 не исполнено, и его собственный аргумент («мёртвый вариант ошибки — источник
ложной уверенности… тот же класс, что `generation` из §10») теперь работает против него:
класс воспроизведён Б-2.

---

## 4. Находки ниже блокера

| # | находка | где | категория |
|---|---|---|---|
| **Н-1** | `verify_M-65.sh` не наблюдает ОТСУТСТВИЕ оракула задачи 12: у гейта семь шагов (`A`, `N`, `F`, `F2`, `L`, `M`, `T`), ни один не упоминает `test_sync`/rendezvous/in-flight (`grep` → пусто). Гейт зелёный при невыполненном условии снятия REJECT'а — `testing.md` §«Целостность гейта», свойство 4: «наблюдает ОТСУТСТВИЕ, не только сбой» | `scripts/verify_M-65.sh` | дыра гейта |
| **Н-2** | §Tasks не содержит задач на §11 (N-2/N-3/N-4) и §12 (N-5..N-8). Задачи 10-13 покрывают только фикс гонки и батарею. Решение, не ставшее задачей, не имеет ни исполнителя, ни статуса, ни оракула — отсюда Б-3 и Б-4 | `milestones/M-65-ws-session.md` §3 | планирование |
| **Н-3** | `1daf2a8`/`bc19ee7` `[architect]` правят `research/critiques/**` и `research/reviews/**` — вне §2 Allowed paths милестоуна и вне строки architect'а в `scope-guard.md`; это артефакты critic'а и reviewer'а. **Правка проверена мной построчно и добросовестна** (разбор ниже) — но правильный маршрут был `!!! SCOPE VIOLATION REQUEST !!!` либо диспетч reviewer'а, а не самостоятельная правка чужого артефакта гейта | `git show 1daf2a8`, `bc19ee7` | зона |
| **Н-4** | Handoff §C заявляет «`cargo test -p gateway-serve` → 27 passed». Мой замер по одиннадцати тест-бинарям: **47** (4+3+2+3+6+3+3+4+5+12+2). Число в Done Block обязано быть воспроизводимым — `branch-hygiene.md` п.9 про симметрию проверок ровно об этом | Handoff §C | дисциплина Done Block |
| **Н-5** | `lib.rs:1828-1832`: `GATEWAY_WINDOW_MS` с parse-ошибкой ⇒ `None` ⇒ offline ⇒ **unbounded** свёртка; комментарий рядом зовёт это «graceful fallback», а `docker-compose.yml` про ту же переменную предупреждает об OOM (`TD-039`, замер RSS 7.3 GB). Находка НЕ моя — её называет сам `milestones/M-65-ws-session.md` §11 как «СЕРЬЁЗНЕЕ предмета спора», и она не заведена ни задачей, ни TD. **Вне диффа M-65** (код M-37), поэтому НЕ блокер этого PR; завожу TD-записью при ближайшем close-out | `crates/gateway-serve/src/lib.rs:1828` | долг, унаследованный |
| **Н-6** | `R-086` N-9 (`set_effective_max_subs` — process-global `AtomicUsize` с публичным сеттером, тест-бэкдор в прод-коде, `lib.rs:180-189`, `:1877`) принят прошлым кругом «как TD, merge не блокирует» — но TD-записи так и нет. Принятие долга без карточки есть его исчезновение | `crates/gateway-serve/src/lib.rs:180` | учёт долга |
| **Н-7** | Семейство `GS-I-*`, на которое ссылаются `scope-guard.md` (`GS-I-1`) и `R-086` §1bis (`GS-I-3`, `GS-I-5`), в `docs/fa/viz-backend.md` заведено только как `GS-I-4`. У `gateway-serve` нет собственной FA (`reading-map.md` §2 называет это долгом). Ссылки на несуществующие ID — тот же класс «норма без механизма» | `docs/fa/viz-backend.md` | долг документации |

### 4.1 Разбор Н-3 — правка чужих артефактов гейта (проверка по прямому поручению §D)

Проверено построчно, каждое утверждение — замером:

- **Диф аддитивен.** `git show 1daf2a8 --stat`: «5 files changed, **71 insertions(+)**» — ни
  одного удаления. `bc19ee7` — две строки `verdict: APPROVED` → `APPROVE`.
- **Значения не выдуманы.** Каждый `audited_head` сверен с текстом САМОГО вердикта на
  дореволюционной ревизии (`git show 03540b7:<файл>`): `C-077` → `50dae79` ✓,
  `C-078` → `5a90bae` ✓, `R-057` → `b691242` ✓ (и база `3c66777` названа там же),
  `R-075` → `2e342d3` ✓, `R-080` → `5a2f5d9` ✓.
- **Ревизии существуют и предковы.** Все шесть SHA (пять голов + база):
  `git merge-base --is-ancestor <sha> HEAD` → **yes** для каждого.
- **Вердикты соответствуют содержанию:** REJECT / NOTE / REJECT / APPROVE / APPROVE.
  Правка `bc19ee7` (`APPROVED` → `APPROVE`) корректна: поле машинное, перечисление
  фиксированное, и поймано прогоном барьера, а не вычиткой — это правильный порядок.
- **Правка самообъявлена.** В каждый файл вложен комментарий «Шапка дописана 2026-08-17
  architect'ом ПОСТФАКТУМ… Содержание вердикта не изменено ни на символ». Следующий читатель
  увидит происхождение блока, а не примет его за работу автора вердикта.

**Оценка.** По СОДЕРЖАНИЮ претензий нет; grandfathering отвергнут верно (дата внутри файла
подделываема). По ФОРМЕ — нарушение зоны: артефакт гейта принадлежит вынесшей его роли
(`branch-hygiene.md` §4), и правит его эта роль либо тот, кому передали через SVR. Как
reviewer я эту правку **принимаю постфактум** — переделывать её ради формы значило бы
потерять час на переписывание проверенно-верного, — но фиксирую прецедент: следующая
надобность такого рода идёт через SVR, а не через самостоятельную правку.

---

## 5. Условие снятия REJECT

Условия `R-086` §5 п.1, п.3, п.4 остаются в силе НЕВЫПОЛНЕННЫМИ, к ним добавляется Б-2:

1. **Б-1.** Оракул на находку существует и ИСПОЛНЯЕТ переключение селектора, пока pump в
   полёте. Критерий приёмки — не зелёный прогон, а мутационный: снятие развязки А ЛИБО
   развязки Б обязано ронять этот оракул. Проектирование — architect (`gates.md` §4),
   реализация механики — engine-dev.
2. **Б-2.** Механизм синхронизации либо подключён так, что реально исполняется (feature-флаг
   `--features testing`, либо оракул unit-тестом внутри `src/` — оба пути названы задачей 12),
   либо **удалён**. Третьего («лежит в `main` неисполняемым») быть не должно: это ровно тот
   класс, за который этот модуль уже получил `R-057` М-1/М-2 и `R-086` N-1.
3. **Б-3.** N-2/N-3/N-4: реализация приведена к `CT-RFC-09` §2.1-2.9 ЛИБО норматив изменён
   contract-RFC'ом. Решения §11 приняты — их надо исполнить, а не переподтвердить.
4. **Б-4.** N-5…N-8 удалены (решение §12) либо заведены TD-записью с обоснованием.
5. **Н-1.** Шаг гейта, наблюдающий ОТСУТСТВИЕ оракула задачи 12, — иначе следующий круг снова
   получит `VERDICT: PASS` поверх невыполненного условия.
6. Н-2…Н-7 — принимаются как NOTE/TD, merge не блокируют. Н-5, Н-6, Н-7 завожу в
   `TECH-DEBT.md` сам (файл reviewer-owned).

**Merge PR #29 в `main` запрещён до выполнения п.1-4.** PR я не мержу и по прямому указанию
цепочки: вердикт возвращается founder'у на диспетч.

---

## 6. Done Block — сырой вывод (агрегирован по `commit-discipline.md`)

Дерево `/tmp/rev-m65`, detached на `bc19ee7`. Прогон МОЙ, не пересказ Handoff'а.

```
$ git rev-parse HEAD
bc19ee772448f2af0afa6dd48b7b00e21526918f

$ git rev-parse origin/fix/M-65-battery-recalibration
bc19ee772448f2af0afa6dd48b7b00e21526918f

$ git status --porcelain          # до создания этого вердикта
{пусто}

$ df -h / | tail -1
/dev/md2        437G  327G   88G  79% /

$ cargo fmt --all -- --check; echo "exit=$?"
exit=0

$ cargo clippy -p gateway-serve --all-targets --all-features -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.15s
exit=0

$ cargo test -p gateway-serve 2>&1 | grep -E "^test result"
test result: ok. 0 passed;  0 failed;  # unittests src/lib.rs   <- ноль unit-тестов (Б-2)
test result: ok. 0 passed;  0 failed;  # unittests src/main.rs
test result: ok. 0 passed;  0 failed;  # unittests src/bin/wsprobe.rs
test result: ok. 4 passed;  0 failed;  # tests/red_jwt_verify.rs
test result: ok. 3 passed;  0 failed;  # tests/red_serve_consumes_checkpoint.rs
test result: ok. 2 passed;  0 failed;  # tests/red_serve_passthrough.rs
test result: ok. 3 passed;  0 failed;  # tests/red_serve_window_wiring.rs
test result: ok. 6 passed;  0 failed;  # tests/red_timeframe_guard_startup.rs
test result: ok. 3 passed;  0 failed;  # tests/red_ws_honesty_sessions.rs
test result: ok. 3 passed;  0 failed;  # tests/red_ws_liveness_under_load.rs
test result: ok. 4 passed;  0 failed;  # tests/red_ws_protocol.rs
test result: ok. 5 passed;  0 failed;  # tests/red_ws_series_vs_replay.rs
test result: ok. 12 passed; 0 failed;  # tests/red_ws_session.rs
test result: ok. 2 passed;  0 failed;  # tests/smoke_ws.rs
ИТОГО passed=47 failed=0   (Handoff §C заявлял 27 — расхождение, находка Н-4)
```

Гейты прод-формой (`EVENT_NAME=pull_request`, `PR_BASE_SHA=$(git merge-base origin/main HEAD)`
= `a9f0bf5`):

```
$ bash scripts/check_gate_meta.sh; echo "exit=$?"
VERDICT: PASS — вердиктов проверено: 6, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ bash scripts/check_protected_artifacts.sh; echo "exit=$?"
OK: защищённые артефакты целы на HEAD (a9f0bf5..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ bash scripts/check_artifact_ids.sh; echo "exit=$?"
OK: ни один коммит диапазона a9f0bf5..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```

Мутационный контроль (полностью — §3.1):

```
$ # проба достижимости на lib.rs:1197
REACHABILITY PROBE: pump-completion Ok-path reached, drained=false current_gen=Some(0) gen_at_pump=0
   → 7 оракулов из 12 исполняют путь; генерации НИ РАЗУ не расходятся

$ # мутация 1: развязка Б нейтрализована
test result: ok. 12 passed; 0 failed;    <- защита НЕ ПИННИТСЯ

$ # мутация 2: развязка А нейтрализована (дефект R-086 §2.3 восстановлен буквально)
test result: ok. 12 passed; 0 failed;    <- защита НЕ ПИННИТСЯ

$ git status --porcelain    # дерево восстановлено после мутаций
{пусто}
```

Acceptance-гейт `scripts/verify_M-65.sh` — см. §6.1.

### 6.1 `verify_M-65.sh` — результат прогона

```
$ bash scripts/verify_M-65.sh; echo "exit=$?"
--- A: RED-набор и батарея на месте, парсятся, форматированы ---
PASS  A crates/gateway-serve/tests/red_ws_session.rs на месте
PASS  A форматирование (паритет с CI-шагом fmt)
--- N: манифест набора ⇄ таблица осей §4.2, в ОБЕ стороны ---
PASS  N манифест ⇄ §4.2 совпал в обе стороны; у каждой из восьми осей есть легитимный сценарий
--- F: RED-набор O-1..O-10 GREEN ---
PASS  F набор GREEN: исполнено 12 оракулов (ожидалось ≥ 11: o0 + O-1..O-10)
--- F2: батарея мутантов §4.5 — FAIL-CLOSED до задачи 9 ---
PASS  F2 BATTERY: PASS (13/13)
--- L: лимит подписок fail-closed В ОБЕ СТОРОНЫ ---
PASS  L невалидный лимит «0» ⇒ отказ старта (exit=2)
PASS  L невалидный лимит «-1» ⇒ отказ старта (exit=2)
PASS  L невалидный лимит «abc» ⇒ отказ старта (exit=2)
--- M: регресс — цена M-65 не уплачена соседним инвариантом ---
PASS  M соседние оракулы gateway-serve GREEN (10 тестов)
--- T: паритет с CI + НЕНУЛЕВОЕ число исполненных тестов ---
PASS  T clippy
PASS  T cargo test --all: passed=835

VERDICT: PASS
exit=0
```

Батарея 13/13 и `VERDICT: PASS` воспроизведены МОИМ прогоном и не оспариваются; `passed=835`
против `815` у `R-086` — рост от коммитов `main`, влитых `c4d3561`, не от M-65.

**Зелёный гейт здесь — САМ ПО СЕБЕ находка, и это второй круг подряд, где это так**
(`R-086` §6.1 сказал то же). Гейт не краснеет ни против снятой развязки А, ни против снятой
развязки Б, ни против отсутствующего оракула задачи 12, ни против трёх неисправленных
отклонений §2.1-2.9. Он меряет то, что умеет мерить, и это ровно симптом из `testing.md`:
«гейт зелёный, но ты не можешь предъявить, ЧТО он покраснеет против конкретного слома».

---

## 7. Состояние ветки и мира (для следующего круга)

- `main` зелёный: `gh run list --branch main --limit 5` — CI и Deploy `success`
  (`32071272016`, `32071271607`, `32071943643`);
- PR #29 `MERGEABLE`, база `a9f0bf5`, ветка СВЕДЕНА с `main` (`c4d3561`, конфликт один —
  `docs/SESSION-HANDOFF.md`, взята версия `main`; заявление Handoff'а §B подтверждаю);
- чеки PR #29: одиннадцать `SUCCESS`, `fmt + clippy + test` был `IN_PROGRESS` на момент
  начала гейта. **Зелёные чеки не отменяют этот REJECT:** ни один из них не проверяет
  условия `R-086` §5;
- `strict: false` (`gates.md` §8) — перед возможным будущим merge'ем обязателен
  `verify_design_claims.sh --merge-preview origin/main` на ДЕРЕВЕ СЛИЯНИЯ.
