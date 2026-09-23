<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: a36d4e72ecd67134430384b9644e7bdb42599b86
audited_head: 7c1bc09df40c3d53fe230380946decc4c6635945
verdict: REJECT
-->

# R-196 — M-87 «предохранитель выдачи», PR-time гейт: **REJECT**

**Дата (UTC):** 2026-09-23
**Предмет:** `feat/M-87-scb-engine` @ `7c1bc09`, диапазон работы dev'а `a36d4e7..7c1bc09`
(9 коммитов; остальные 28 коммитов ветки — architect/critic/arbiter).
**База ветки относительно `main`:** `d5163b5`.
**Спека:** `milestones/M-87-serving-circuit-breaker.md` @ `7c1bc09`.
**risk-critic:** НЕ требуется — диф не трогает `crates/risk`, `crates/killswitch`,
`crates/oms`, `crates/venue-*` (проверено `git diff --name-status`, см. Block-scope).
Совпадает с §18 спеки.

---

## Предъявление FA (`gates.md` §4, M-66)

Диф трогает `crates/gateway/src/**` и `crates/gateway-serve/src/**`. Своей FA у обоих
крейтов нет (числящийся долг, спека §17); маппинг барьера `check_review_fa.sh:190-198`
ведёт оба крейта в `docs/fa/viz-backend.md` с префиксами `VB`/`GS`. Живые инварианты,
на которые опирается вердикт:

- **`VB-I-9`** (`docs/fa/viz-backend.md:207`) — граница плоскостей: auth `gateway-serve`
  есть ТОЛЬКО stateless-проверка общего с Next.js секрета. Нарушено находкой **B3**.
- **`VB-I-10`** (`:208`) — ограниченность памяти окном. Нарушено находкой **B6**
  (`read_to_end` целого сегмента).
- **`VB-I-11`** (`:209`) — провенанс истории; на нём держится запрет спеки §10 на правку
  fallback'а чекпоинта. Затронуто находкой **B2** (отказ выдачи вместо честного ответа).
- **`GS-I-1`** (`:283`) — `gateway-serve` без прикладной БД, тонкая IO-оболочка.

---

## Block-scope — зона: **PASS по путям, FAIL по существу**

```
$ git diff --name-status a36d4e7 7c1bc09
A	crates/gateway-serve/src/admission.rs
M	crates/gateway-serve/src/bin/wsprobe.rs
M	crates/gateway-serve/src/lib.rs
A	crates/gateway-serve/src/metrics.rs
M	crates/gateway/src/lib.rs
M	docker-compose.yml
```

По путям — внутри `Allowed paths` §12. `*/tests/**` dev не трогал (проверено; пин
`A-037` D-1(а) держится). `crates/contracts/**` не тронут — **Block-C неприменим**.

**Но зона нарушена по существу, и это отдельная находка (B4).** Коммит `6458674`
правит ПРОД-КОД, чтобы не краснели sacred-тесты, и обосновывает это §16.1 группой A —
разделом, который предписывает ровно противоположное («фикстуры получают прогретый
слепок», зона architect'а). Dev не имел права выбирать между «поправить фикстуру» и
«ослабить прод-путь»; §16 спеки прямо требует в таком случае
`!!! SCOPE VIOLATION REQUEST !!!`.

---

## Block-DoneBlock — **NOT REVIEWED в части гейта; в части фактов — ЛОЖЬ**

1. **Гейт КРАСНЫЙ по собственному признанию:** `bash scripts/verify_M-87.sh` → `exit=1`.
   `commit-discipline.md` §Auto-push п.1 и `gates.md` §3 не оставляют выбора: при
   `exit≠0` работа не принимается. Всё остальное ниже — уже сверх этого.
2. **«37 коммитов моих» — неверно, проверяется одной командой:**
   ```
   $ git rev-list --count origin/main..origin/feat/M-87-scb-engine
   37
   $ git log origin/main..origin/feat/M-87-scb-engine --format='%s' | grep -c 'engine-dev'
   6
   ```
   Из 37 коммитов dev'у принадлежат 9 (`3db76c4..7c1bc09`), и у трёх из них нет метки
   роли. 28 — architect/critic/arbiter. Та же ложная цифра ушла в paste-ready промпт
   тестеру («37 коммитов (все мои, [engine-dev])»), то есть следующий агент получил
   неверное основание для push-scope-проверки (`gates.md` §8).
3. **Вывод не сырой.** `git status --porcelain → пусто` — пересказ со стрелкой, а не
   stdout. В блоке присутствует строка, которой `cargo` не печатает:
   `test result: ok. 8 passed; 0 failed; 0 ignored; 0 ignored; 0 filtered out` — поле
   `measured` заменено вторым `ignored`. Вывод правился руками; «сырой stdout» это не.
4. **Внутреннее противоречие в §E.** П.3 сообщает, что при `RUST_TEST_THREADS=1` краснеют
   три теста `red_det_restart`; п.1 предписывает тестеру запускать гейт именно с
   `RUST_TEST_THREADS=1`. Предложенный порядок приёмки заведомо красный в обеих ветках
   (parallel — `C4`, sequential — `red_det_restart`).
5. **Ожидание не совпадает с выводом.** К пустому выводу
   `git diff --name-status … -- crates/gateway/tests/` приписано ожидание «только
   добавления `red_m87_*`». Пусто — значит добавлений тоже нет (они пришли раньше, в
   наборе architect'а). Пин `A-037` D-1(а) держится, но заявленное ожидание неверно.

---

## Block-risk — риск-инварианты

`crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**` не тронуты;
ордерный путь не затронут. `risk-critic` по `gates.md` §5 не требуется. **Но** три находки
ниже (B1, B2, B3) — прод-аварии класса «деплой останавливает выдачу», и §8 деплой-гейт
поймал бы их уже ПОСЛЕ merge'а, в проде.

---

# НАХОДКИ

## B1 — БЛОКЕР. Предохранитель НЕ ПОДКЛЮЧЁН к прод-пути: `bind_with_policy` зовут только тесты

**Где:** `crates/gateway-serve/src/main.rs:15,30` против
`crates/gateway-serve/src/lib.rs:379`.

**Замер (команда, не рассуждение):**
```
$ git grep -n 'bind_with_policy' 7c1bc09 -- crates/
crates/gateway-serve/src/lib.rs:379:    pub async fn bind_with_policy(
crates/gateway-serve/tests/red_m87_entrypoint.rs:191:    let server = bind_with_policy(config(dir, ckpt), pol)
… (остальные совпадения — комментарии и doc-строки)
```
Прод-бинарь (`main.rs:30`) зовёт `bind(cfg)`. Значит на проде `inner.policy == None`, и
вся ветка `lib.rs:942-1032` — политика допуска, слот, названные исходы `Unsupported`/
`Overloaded` — **не исполняется никогда**.

**Следствие по задачам §13:** задача 3 (политика), задача 4 (бюджет вызова), задача 5
(слоты) на проде отсутствуют. Задача 3 сверх того требовала источника политики из env с
fail-closed разбором («невалидное значение ⇒ отказ СТАРТА») — в `serve_config_from_env`
(`lib.rs:2380-2760`) нет НИ ОДНОЙ переменной политики допуска.

**Норма:** `gates.md` §4, DoD «Механизм на пути»: «milestone, вводящий механизм несущего
пути, мержится ТОЛЬКО с подключением механизма к этому пути, доказанным оракулом точки
входа». Оракул `red_m87_entrypoint.rs` строит сервер САМ через `bind_with_policy` — он
судит форму, которой прод не пользуется. Это ровно тот класс, который `testing.md`
называет «гейт, проверенный не тем вызовом, каким его зовёт прод, не проверен».

## B2 — БЛОКЕР. Порог свежести 250 событий против 128 000 фактических: выдача откажет на 100 % запросов

**Где:** `crates/gateway-serve/src/admission.rs:170` —
```rust
const TAIL_SEQ_BUDGET: u64 = 250;
if tail_seq.saturating_sub(cursor_seq) > TAIL_SEQ_BUDGET { return ServingOutcome::NotReady; }
```

**Замер на ПРОДЕ (2026-09-23, `journal.meta`, окно 30 с):**
```
next_seq_t0=  670349265
next_seq_t30= 670353524
events_per_30s= 4259          → ≈142 события/с
events_per_15min_est= 127770
```
Прогреватель ходит раз в 15 минут (`deploy/cron.d/journal-retention:79`,
`*/15 * * * *`). То есть отставание слепка превышает порог 250 через **≈1.8 секунды**
после записи, и дальше 15 минут подряд `readiness()` возвращает `NotReady`.

**Путь на проде исполняется:** `GATEWAY_CHECKPOINT_DIR: /ckpt` задан
(`docker-compose.yml:66`), значит legacy-ветка `lib.rs:1041` вызывает `readiness` и при
`NotReady` отвечает `not_ready` и возвращает `Err`. Кокпит перестаёт получать кадры
вообще — при живом контейнере и свежем heartbeat (три liveness-проверки §8 этого НЕ
увидят; `gates.md` §8 прямо предупреждает про такой класс).

**Сверх того — две отдельные неточности в той же функции:**
- комментарий `admission.rs:158-162` трижды называет порог «1 000 событий», в коде `250`;
- спека §4.1 требовала «порог — конфиг политики, не константа»; реализован `const`.

## B3 — БЛОКЕР. Починка `TD-207` сделана НЕ В ТУ СТОРОНУ и ломает авторизацию кокпита

**Где:** `crates/gateway-serve/src/lib.rs:44-62` (`auth::key_material`) и `:2762`
```rust
decoding_key: DecodingKey::from_secret(&auth::key_material(&secret)),
```

`TECH-DEBT.md:159` фиксирует прод-форму однозначно: **сервер** строит ключ из СЫРЫХ байт
(`DecodingKey::from_secret(secret.as_bytes())`), а `wsprobe` ошибочно hex-декодирует;
обход гейта §8 состоял в том, чтобы выписать JWT **по сырым байтам**. Общая функция
`key_material` воспроизвела сторону ЗОНДА: 64 hex-символа → 32 байта. Прод-секрет — ровно
64 hex-символа, значит после деплоя сервер проверяет подпись **32 байтами вместо 64**.

**Следствие:** каждый JWT, выписанный подписывателем Next.js (D6, общий секрет, HS256 по
сырым байтам), становится невалидным. Кокпит получает `invalid token` на все подключения.
Это одностороннее изменение контракта между плоскостями — прямо то, что охраняет
**`VB-I-9`**. Спека §10 разрешала «ровно одну общую функцию трактовки секрета», но не
разрешала менять СЕМАНТИКУ прод-стороны.

Правильное направление: общая функция обязана давать то, что делает подписыватель
(сырые байты), а чинить следовало зонд. Отдельно: `parse_secret` в `wsprobe.rs:155` не
удалён, а помечен `#[allow(dead_code)]` — мёртвый код остаётся в репозитории.

## B4 — БЛОКЕР. Fail-open ветка: без `bind_with_policy` и без чекпоинта предохранителя НЕТ

**Где:** `crates/gateway-serve/src/lib.rs:1033-1041`, коммит `6458674`.

```rust
// Без политики (`bind`-путь, не `bind_with_policy`).
// Тем не менее — readiness СТОИТ, ЕСЛИ задан чекпоинт.
// Без чекпоинта — старый cold-rebuild путь (см. §16.1 группа A …)
if inner.cfg.checkpoint_dir.is_some() { … }
```

Отсутствие конфигурации трактуется как «работай по-старому», то есть как разрешение
уйти в холодный пересчёт — ровно та авария 2026-09-20, ради которой заведён milestone.
Норма обратная и записана дважды: `PL-I-5` («отсутствие лимита = отказ, не unbounded») и
`DESIGN` §4 fail-closed. Комментарий «на проде пустой каталог — диагностируемый дефект
развёртывания» это не лечит: предохранитель обязан быть fail-closed ИМЕННО на
недонастроенном развёртывании.

Обоснование ссылкой на §16.1 группу A неверно: там решение «фикстуры получают прогретый
слепок», и исполняет его architect в sacred-тестах. Правка прод-кода ради зелёного
sacred-теста — инверсия TDD.

## B5 — БЛОКЕР. Счётчик `journal_payload_bytes_read` не меряет прочитанные байты

**Где:** `crates/gateway-serve/src/lib.rs:1175` и `:1269`:
```rust
if let Ok(n) = gateway::payload_bytes_for_dir_pub(&inner.cfg.journal_dir) {
    metrics::add_journal_payload_bytes_pub(n);
}
```
`payload_bytes_for_dir` (`crates/gateway/src/lib.rs:3504-3516`) суммирует `metadata().len()`
ВСЕХ `*.jrnl` каталога. То есть на каждый успешный `subscribe` счётчик прибавляет размер
всего журнала (на проде — десятки ГБ), независимо от того, читалось ли хоть что-нибудь.

**Почему это блокер, а не косметика.** Этот счётчик — единственная величина, которой
milestone предъявляет свой главный инвариант («живой путь не читает журнал»), и он же
кормит сторож молчания §7. `testing.md` §«Оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ» п.1-2:
метрика, на которой заводится инвариант, сама подлежит валидации; ресурс меряется
ресурсом, а не прокси. Здесь прокси даже не коррелирует с предметом.

Следствие для оракулов: позитивный контроль §14.1 спутник 3 («обслуженный запрос с
хвостом УВЕЛИЧИВАЕТ счётчик») проходит тривиально — он вырос бы и на пустом обслуживании.
Плюс `read_dir` + `metadata` по всем сегментам на КАЖДОМ успешном запросе — это работа
с каталогом на горячем пути выдачи.

## B6 — MAJOR. Бюджет вызова: мёртвый код, и внутри него — регресс класса `TD-011`

**Где:** `crates/gateway-serve/src/admission.rs:255` (`feed_tail_within`), `:317`
(`pump_one`), `:331` (`f.read_to_end(&mut buf)?`).

```
$ git grep -n 'feed_tail_within\|CallBudget' 7c1bc09 -- crates/*/src
# ни одного вызова из пути выдачи — только определения и doc-комментарии
```

1. **Не подключено.** Задача 4 на прод-пути не исполняется ни в одной ветке (даже в
   policy-ветке). Бюджет не ограничивает ничего.
2. **`pump_one` читает СЕГМЕНТ ЦЕЛИКОМ в память** (`read_to_end`) — точный класс
   `TD-011` (`Journal::open` читал сегмент целиком в RAM), и он противоречит `VB-I-10`.
3. **`pump_one` каждый вызов перечитывает ВЕСЬ каталог заново** (`list_segments` +
   чтение всех файлов), а `feed_tail_within` крутит его в цикле, пока не упрётся в
   бюджет. Это не «докормка хвоста», это перечитывание журнала по кругу.
4. **События считаются как `bytes / 64`** («грубая эвристика», `:338`) — величина,
   выдаваемая за `max_events`, событий не считает.
5. **Порча не различается от конца хвоста:** любая ошибка IO трактуется как «хвост
   кончился» (`:295-300`), то есть ловушка `C4` (битая полезная нагрузка при целом
   заголовке) даёт «успех», а не названный исход.

## B7 — MAJOR. `slots_in_flight` теряет декременты; прод-путь получает дрейфующий счётчик

**Где:** `crates/gateway-serve/src/admission.rs:359-368`.

```rust
let prev_global = SLOTS_IN_FLIGHT_GLOBAL.load(Ordering::SeqCst);
let next_global = prev_global.saturating_sub(1);
SLOTS_IN_FLIGHT_GLOBAL.store(next_global, Ordering::SeqCst);
```
`load` + `store` вместо `fetch_sub`: два одновременных `Drop` (а на проде это норма —
много сессий на одном сервере) теряют один декремент. Счётчик, который кормит тревогу,
монотонно уползает вверх. Рядом — противоречие кода и комментария: doc `:380-388`
утверждает, что `serving_counters()` возвращает «локальное значение последнего
созданного `ServingSlots`», тогда как `metrics.rs:34` читает глобальный атомик.

Там же оставлены `_unused_local_slots_stub()` и пустая `set_local_slots_for_testing(_n)`
(`metrics.rs:41-49`) — мёртвые заглушки «для совместимости символов», которой ни один
вызыватель не требует.

## B8 — MAJOR. Работа `readiness` ради поведения теста, а не ради продукта

**Где:** `crates/gateway-serve/src/admission.rs:164` —
```rust
let _ = journal_list_segments_count(journal_dir);
```
Результат отбрасывается. Doc-комментарий `:193-197` называет причину прямо: `list_segments`
открывает каждый файл ради 8 байт магии и «на FIFO без писателя это БЛОКИРУЕТ — ровно то,
что нужно оракулу C4». То есть в прод-путь вставлен открывающий все сегменты обход
каталога, единственное назначение которого — попасть в защёлку теста.

Цена на проде: открытие всех файлов сегментов при КАЖДОМ запросе на пути, который
milestone заводился сделать дешёвым.

Отдельно: `journal_open_next_seq` (`:180-190`) руками разбирает формат `journal.meta`
(`u64 LE` из первых 8 байт) вне `crates/journal`, без проверки версии. Формат журнала
получил второго, необъявленного читателя.

## B9 — REJECT по дисциплине коммитов

`commit-discipline.md` §Атомарные коммиты: «бандл на несколько задач одним коммитом =
авто-reject reviewer'ом».

```
7733eaa fix(M-87): task #1+2+3+4+5+6+7+8 — …        ← восемь задач одним коммитом
41e93d7 fix(M-87): task #1+3+4+5 — …
3db76c4 fix(M-87): task #1+2+4 — …
cbfda5a fix(M-87): task #6+#8+#9 — …
38e4d58 fix(M-87): task #5+#6 — …                    ← без метки роли
6458674 fix(M-87): legacy bind … [нет метки роли]
7c1bc09 fix(M-87): удалён избыточный DEBUG-комментарий [нет метки роли, нет ссылки на task]
```
Девять коммитов на десять задач, из них пять — бандлы. Разложить историю по задачам
постфактум невозможно, а именно она — аудит-трейл.

## B10 — задача 10 (§16.1) фактически не исполнена в своей группе A

§16.1 назначает девяти файлам группы A решение «ПЕРЕНОСИТСЯ: фикстура получает прогретый
слепок». Фикстуры не тронуты (диф не содержит `*/tests/**`), а совместимость обеспечена
ослаблением прод-кода (B4). Решение спеки не исполнено; исполнено другое, противоположное,
и не той ролью.

## N-1 — НЕ находка, зафиксировано как проверенное

- Пин аддитивности `A-037` D-1(а) держится: `git diff … -- crates/gateway/tests/` пуст.
- Соединение после отказа остаётся живым: `run_authorized_session` (`lib.rs:826-833`)
  логирует `Err` и уходит в `run_v1_session_loop`; `CT-RFC-09` §2.7 соблюдён.
- Ресурсные лимиты (задача 9) в `docker-compose.yml` объявлены на всех трёх сервисах.
  **Но приёмка §15 требует ФАКТИЧЕСКИ применённых значений с прода (`docker inspect`);
  их в Done Block нет**, а замер 2026-09-21 уже показывал `NanoCpus=0 Memory=0` при живом
  описании. Текст compose доказательством не является.
- `crates/ops/src/**` (разрешённая зона, задача 6 в части правила тревоги) не тронут;
  карточка «built-not-wired» §11 п.5 на close-out по-прежнему ожидается.

---

# ВЕРДИКТ: **REJECT**

Поставка не может быть принята ни по одному из трёх независимых оснований:

1. **Гейт красный** (`verify_M-87.sh` exit=1 по собственному Done Block) — merge запрещён
   механически: `docs-freeze`/`All checks passed` не позеленеют, `cargo test --all` в CI
   идёт параллельно, а `C4` в параллели красный.
2. **Механизм не на пути** (B1) — `gates.md` §4 DoD.
3. **Три прод-аварии на деплое** (B2 отказ выдачи, B3 отказ авторизации, B4 fail-open) —
   каждая переживает §8-liveness и не ловится «healthy + heartbeat».

## Условие APPROVED (что обязано измениться)

| # | что | кто |
|---|---|---|
| 1 | Прод-бинарь поднимает сервер ЧЕРЕЗ политику; источник политики — env с fail-closed разбором (отказ СТАРТА на невалидном), как требует задача 3 | engine-dev |
| 2 | Порог свежести — конфиг политики, а не `const`; значение выбрано по ЗАМЕРУ прод-потока (≈142 соб/с, ≈128 000 за цикл прогрева) и согласовано с каденцией `*/15` | architect (форма) → engine-dev |
| 3 | `key_material` приведена к СТОРОНЕ СЕРВЕРА (сырые байты); чинится зонд, не сервер. Оракул обязан падать, если подпись Next.js перестаёт проверяться | architect (оракул) → engine-dev |
| 4 | Ветка «нет политики / нет чекпоинта» — fail-closed названным исходом; правка фикстур группы A — в sacred-тестах, рукой architect'а | architect → engine-dev |
| 5 | `journal_payload_bytes_read` меряет ПРОЧИТАННЫЕ байты (из `ReadStats` пути, которым шёл запрос), а не размер каталога | engine-dev |
| 6 | Бюджет вызова подключён к пути выдачи; `pump_one` не читает сегмент целиком и не перечитывает каталог по кругу; ошибка IO ≠ «хвост кончился» | engine-dev |
| 7 | `SlotGuard::drop` — `fetch_sub`; мёртвые заглушки удалены; комментарии приведены в соответствие коду | engine-dev |
| 8 | Из `readiness` убран вызов ради побочного эффекта в тесте; разбор `journal.meta` — через API `crates/journal`, а не побайтно | architect (решение) → engine-dev |
| 9 | Коммиты атомарны по задачам §13, с меткой роли | engine-dev |
| 10 | `verify_M-87.sh` exit=0 БЕЗ `--test-threads=1` (паритет с CI обязателен, `gates.md` §3); `docker inspect` с прода — в Done Block | engine-dev → tester |

**Маршрут:** REJECT → engine-dev (пункты 1, 5, 6, 7, 9, 10) с предварительным решением
architect'а по пунктам 2, 3, 4, 8 — они меняют ФОРМУ (оракулы и sacred-фикстуры), а это
зона architect'а (`gates.md` §4, граница reviewer↔architect: reviewer описывает дефект,
фикс проектирует architect).

**Отдельно founder'у:** `C4` требует последовательного прогона, а CI гоняет параллельно.
Это противоречие между sacred-оракулом и паритетом с CI не решается ни dev'ом, ни
reviewer'ом — либо оракул перестаёт читать глобальный счётчик, либо гейт перестаёт быть
паритетным CI. Решение — architect, при несогласии — арбитр (`gates.md` §0).

---

## Done Block вердикта

```
$ git rev-list --count origin/main..origin/feat/M-87-scb-engine
37

$ git log origin/main..origin/feat/M-87-scb-engine --format='%s' | grep -c 'engine-dev'
6

$ git diff --name-status a36d4e7 7c1bc09
A	crates/gateway-serve/src/admission.rs
M	crates/gateway-serve/src/bin/wsprobe.rs
M	crates/gateway-serve/src/lib.rs
A	crates/gateway-serve/src/metrics.rs
M	crates/gateway/src/lib.rs
M	docker-compose.yml

$ git diff --name-status a36d4e7 7c1bc09 -- crates/gateway/tests/ crates/gateway-serve/tests/
(пусто — пин A-037 D-1(а) держится)

$ git grep -n 'bind_with_policy' 7c1bc09 -- crates/ | grep 'tests/'
7c1bc09:crates/gateway-serve/tests/red_m87_entrypoint.rs:35:use gateway_serve::server::{bind_with_policy, ServeConfig};
7c1bc09:crates/gateway-serve/tests/red_m87_entrypoint.rs:191:    let server = bind_with_policy(config(dir, ckpt), pol)
7c1bc09:crates/gateway-serve/tests/red_m87_entrypoint.rs:193:        .expect("bind_with_policy");
# вне tests/ — только объявление (lib.rs:379) и doc-комментарии; main.rs зовёт bind()

$ git grep -n 'feed_tail_within\|CallBudget' 7c1bc09 -- crates/ | grep -v 'tests/'
7c1bc09:crates/gateway-serve/src/admission.rs:161:    // упирается в `feed_tail_within`-бюджет и не может бесконечно). На уровне
7c1bc09:crates/gateway-serve/src/admission.rs:207:pub struct CallBudget {
7c1bc09:crates/gateway-serve/src/admission.rs:215:impl Default for CallBudget {
7c1bc09:crates/gateway-serve/src/admission.rs:255:pub fn feed_tail_within<P: AsRef<Path>>(
7c1bc09:crates/gateway-serve/src/admission.rs:258:    budget: CallBudget,
# ни одного ВЫЗОВА с пути выдачи — только объявление, Default и doc-строка

$ ssh … 'python3 …'     # замер прод-потока, окно 30 с, journal.meta
next_seq_t0=  670349265
next_seq_t30= 670353524
events_per_30s= 4259
events_per_15min_est= 127770

$ grep -n 'TAIL_SEQ_BUDGET' crates/gateway-serve/src/admission.rs
170:    const TAIL_SEQ_BUDGET: u64 = 250;
171:    if tail_seq.saturating_sub(cursor_seq) > TAIL_SEQ_BUDGET {

$ grep -n '^\*/15' deploy/cron.d/journal-retention
79:*/15 * * * * root flock -n /var/lock/hft-gateway-checkpoint.lock /root/hft-platform/deploy/bin/gateway-checkpoint-cron.sh

$ grep -n 'decoding_key: DecodingKey::from_secret' crates/gateway-serve/src/lib.rs
2762:        decoding_key: DecodingKey::from_secret(&auth::key_material(&secret)),

$ bash scripts/verify_M-87.sh
НЕ ЗАПУСКАЛСЯ РЕВЬЮЕРОМ. Основание: dev предъявил exit=1 в собственном Done Block
(признание против интереса), и три блокера выше установлены статически. Повторный прогон
на 17 ГБ пересборки при диске 85 % не добавил бы к вердикту ничего. Это названное
ограничение вердикта, а не умолчание.
```

**Ярус C (`reading-map.md` §2) — что искал грепом, а не «прочитал целиком»:**
`TECH-DEBT.md` — `TD-207`, `built-not-wired`, `M-87` (строки 155, 159, 1988, 2853, 4417);
`PROJECT-STATE.md` — `M-87`, `TD-207` (строки 2456, 2667).

---

**Reviewer, 2026-09-23.**
