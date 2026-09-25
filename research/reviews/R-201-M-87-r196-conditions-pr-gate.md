<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: 3a2e4607882756f660d664be5d2ba45780af0a4d
audited_head: 8a5582dc1e3e2ec6b43c917ec38a09e61448e2bb
verdict: REJECT
-->

# R-201 — M-87 «предохранитель выдачи»: PR-гейт круга задач 22-25

**Вердикт: REJECT.** Три блокера. Все три — в задачах, ЗАВЕДЁННЫХ прошлым кругом
(`R-200` §B7) ради устранения ровно этих дефектов; предъявленные фиксы закрывают
ОРАКУЛ, а не УСЛОВИЕ. Приёмка при этом зелёная — и это самостоятельная находка о
форме оракулов, вынесенная в Б-1/Б-2.

Предмет: `origin/feat/M-87-serving-circuit-breaker` @ `8a5582d`, 113 коммитов над
`origin/main` @ `3a2e460`. Ветка СОДЕРЖИТ весь `origin/main` (`git rev-list --count
head..origin/main` → 0), поэтому оговорка `strict:false` (`gates.md` §8) здесь не
срабатывает — судится фактическое дерево слияния.

---

## Блок-scope — PASS

`git diff --name-only origin/main...8a5582d` → 55 файлов, все внутри §12 Allowed paths:

- `crates/gateway-serve/src/**` (5 файлов) · `crates/gateway/src/lib.rs` · `bin/wsprobe.rs`
  — зона engine-dev, объявлена §12;
- `crates/gateway-serve/tests/**` · `crates/gateway/tests/**` · `scripts/verify_M-87.sh` ·
  `milestones/M-87-*.md` · `docs/fa/viz-backend.md` — зона architect, объявлена §12;
- `docker-compose.yml` — объявлен §12 явно; диф проверен построчно: три блока
  `deploy.resources.limits` + пять переменных политики. **Состав записи (граница C,
  `П-008` п.1) НЕ тронут** — блок `RECORDER_*` идентичен `main`. Подписи founder'а не
  требуется;
- `research/{critiques,reviews,arbitration}/**` — артефакты гейтов;
- `docs/ROADMAP.md` (2 строки, `[architect]`) — вне §12 milestone'а, но внутри
  глобальной зоны architect'а (`scope-guard.md`: `docs/`) и требуется барьером
  `roadmap-sync`. **NOTE, не нарушение.**

`crates/contracts/**`, `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`,
`crates/venue-*/**` — НЕ тронуты ни одним коммитом.

## Блок-C (контракты) — N/A

`crates/contracts/**` в дифе отсутствует. `GATEWAY_SCHEMA_VERSION` не бампается.
Contract-RFC не требуется.

## Блок-risk — N/A (risk-critic НЕ требуется)

`gates.md` §5 привязан к путям `risk|killswitch|oms|venue-*|contracts` — ни один не
тронут. `gateway-serve`/`gateway` — read-only консюмер журнала (`VB-I-3`), order-egress
отсутствует. `gates.md` §9 (safety-документы) тоже не срабатывает: тронут
`docs/fa/viz-backend.md`, а не `fa/risk|killswitch|oms`, и правка (+7/−2) — уточнение
`GW-I-9(б)` по решению `A-037` D-4, инварианты не ослабляет.

## Блок-RED-first — PASS, и это сильное свидетельство

`git log --format='%h %s' -- 'crates/*/tests/*'` по диапазону: **34 коммита, ВСЕ с
меткой `[architect]`**, плюс один merge-коммит. Ни один dev-коммит тестов не касался.

Отдельно проверено по ключевому оракулу круга: `red_m87_r196_conditions.rs` имеет
РОВНО ОДИН коммит (`621b216`, `[architect]`), и `scripts/verify_M-87.sh` после него не
менялся ВООБЩЕ. Пять фиксов dev'а (`1606b61`, `e958661`, `7be2ada`, `4d7642a`,
`f1ac6a4`) трогают исключительно `crates/*/src/**`. Оракул заморожен, код доведён под
него — дисциплина соблюдена безупречно.

**Именно поэтому блокеры ниже адресованы не дисциплине, а СОДЕРЖАНИЮ оракулов:** когда
оракул заморожен и при этом проверяет текст, а не работу, безупречная дисциплина даёт
зелёный гейт поверх неисполненного условия.

## Блок-DoneBlock — PASS

Сырой stdout приёмки на СВОЁМ чистом чекауте (`/tmp/hft-reviewer-m87`, detached
`8a5582d`) — в конце файла. Отчёт tester'а воспроизведён независимо, расхождений с ним
нет: приёмка действительно `VERDICT: PASS`. Вердикт REJECT вынесен НЕ вопреки прогону,
а поверх него.

---

# БЛОКЕРЫ

## Б-1 — задача 24: «бюджет подключён к пути выдачи» подключён к ГРЕПУ, а не к выдаче; и он ДОРОЖЕ, чем его отсутствие

**Где:** `crates/gateway-serve/src/lib.rs:2001-2021` (коммит `4d7642a`), внутри
`run_authorized_session` → `spawn_blocking` → drain-цикл.

**Что предъявлено.** `feed_tail_within` теперь вызывается из `src/**`, и оракул
`c3_call_budget_has_a_caller_in_production_code` зеленеет.

**Что на самом деле.** Вызов сделан так, что не ограничивает ничего:

```rust
let _budget_guard = {
    ...
    let budget = CallBudget {
        max_events: 0,
        max_payload_bytes: 0,
        max_wall_ms: u64::MAX,
        max_output_bytes: u64::MAX,
        max_state_bytes: u64::MAX,
    };
    feed_tail_within(cfg1.journal_dir.as_path(), &cfg1.selector, budget, &cancel)
};
if frames.is_empty() { break; }
```

1. **Исход ОТБРАСЫВАЕТСЯ.** `Option<BudgetStop>` связан с `_budget_guard` и не
   читается ни одной строкой. Выход из `loop` по-прежнему только по `frames.is_empty()`.
   Бюджет не может остановить дренаж — он и не пытается.
2. **Бюджет выставлен в ноль НАМЕРЕННО, чтобы вызов сразу вернулся.** Комментарий
   dev'а (`:1978-1990`) говорит это прямо: «бюджет ставим МИНИМАЛЬНЫМ: `max_events: 0`,
   `max_payload_bytes: 0` — проверка срабатывает СРАЗУ после первого `fetch_add`».
3. **Dev сам называет это заглушкой** (`:1998-2000`): «ПРИМЕЧАНИЕ: реальный бюджет (с
   конкретными `max_events`/`max_payload_bytes`) подключается отдельной задачей:
   нынешняя форма — страж от зацикливания, и только».

Это дословно «built-not-wired» (`gates.md` §4 «Механизм на пути»), и заводился он
задачей 24 ради устранения этого же класса — внутри милестоуна О ПРЕДОХРАНИТЕЛЕ.

**Почему это хуже, чем ничего — замер прода.** Вызов не бесплатен: `feed_tail_within`
идёт в `pump_one`, а тот обходит ВСЕ сегменты и читает до `MAX_SEGMENT_BYTES` из
каждого. Замер на проде 2026-09-25:

```
$ ssh … 'ls -1 $D/*.jrnl | wc -l; du -sh $D; ls -lS $D/*.jrnl | head -1'
11
92G
1073741780  segment-00000822.jrnl
```

**11 сегментов по ~1 ГБ каждый.** ⇒ каждый проход `pump_one` = 11 `open` + 11 × 8 МБ =
**88 МБ чтения**, полностью выброшенных (`buf` используется только под `buf.len()`).
Цикл `loop` крутится до исчерпания хвоста, вызов стоит ВНУТРИ цикла, и путь этот —
`run_authorized_session`, то есть **инициализация КАЖДОЙ WS-сессии на проде**.

Милестоун существует, чтобы один запрос не клал сервис. Поставка добавила на путь
запроса ≥88 МБ бесполезного чтения на итерацию. Это регресс класса `TD-011` и прямое
столкновение с `VB-I-10` (`docs/fa/viz-backend.md:208`: «Память `snapshot`/`frames_since`
ограничена ОКНОМ … иначе host-OOM (прод: RSS 7.3GB)»).

**Почему оракул это пропустил.** `c3` (`red_m87_r196_conditions.rs:236-249`) считает
строки `src/**`, содержащие текст `feed_tail_within(`:

```rust
let calls = src_code_lines().iter()
    .filter(|l| l.contains("feed_tail_within("))
    .filter(|l| !l.contains("pub fn feed_tail_within"))
    .count();
assert!(calls > 0, …);
```

Это проверка ПО ИМЕНИ. Коммит, вводящий оракул, назван «пять условий R-196 проверяются
**ПО РАБОТЕ, не по имени**» (`621b216`) — и в этом пункте своему заголовку не отвечает.
`testing.md` §«Механизм несущего пути обязан иметь оракул точки входа»: «Проверка должна
быть по ВЫЗОВУ (исполнением/поведением), а не по тексту».

## Б-2 — задача 23: счётчик прочитанных байт стал мерить РАЗМЕР КАТАЛОГА — величину, запрещённую дословно

**Где:** `crates/gateway/src/lib.rs:5057-5060` (коммит `e958661`), WARM-ветка
`LiveReducer::resume`.

**Требование задачи 23 (§13, дословно):** «`journal_payload_bytes_read` считает
ПРОЧИТАННОЕ, а не размер каталога; `read_dir`+`metadata` уходят с горячего пути».

**Что сделано:**

```rust
let stats = ReadStats {
    payload_bytes_read: payload_bytes_for_dir(dir).unwrap_or(0),
    ..ReadStats::default()
};
```

А `payload_bytes_for_dir` (`crates/gateway/src/lib.rs:3847-3862`) — это буквально:

```rust
for entry in fs::read_dir(dir)? {
    if p.extension().is_some_and(|x| x == "jrnl") {
        let md = fs::metadata(&p)?;
        total = total.saturating_add(md.len());
    }
}
```

То есть `read_dir` + `metadata` + сумма размеров `.jrnl` = **размер каталога**. Ровно
две запрещённые вещи в одной строке. Прежний дефект (`snap_text.len()`) снят, и заменён
величиной, которую задача называет по имени как неверную.

**Цена по замеру.** WARM — основной прод-путь (слепок есть). На нём журнал НЕ читается
(состояние берётся из слепка), а счётчик отчитается о **92 ГБ** — размере всего
каталога. Счётчик кормит `ServingCounters::journal_payload_bytes_read` через
`metrics::add_journal_payload_bytes_pub` (`lib.rs:1162` и `:1308`). Это, по формулировке
самой задачи 23, «главная цифра милестоуна: по ней судят, читает ли живой путь журнал».
Прежняя ложь (размер ответа, килобайты) заменена ложью на **пять порядков крупнее** и в
ту же сторону — «читали много», когда не читали ничего.

**Честная величина лежала рядом и не взята.** На том же пути `stats = stats + pump_stats`
(`lib.rs:1971`) уже копит РЕАЛЬНО прочитанное хвостовым `pump`. Для WARM честный ответ —
байты загрузки слепка + байты хвоста, а не опись каталога.

**Почему оракулы это пропустили — оба:**

- `c2_payload_counter_is_not_fed_by_response_size` пиннит ОДНУ прежнюю форму:
  `.filter(|l| l.contains("snap_text.len()") || l.contains("text.len()"))`. Любая другая
  неверная величина проходит. Это не оракул свойства, это оракул одного литерала;
- `u1_served_request_with_tail_increments_payload_counter`
  (`red_m87_entrypoint.rs:1043-1065`) требует лишь `counter > before`. Размер каталога
  удовлетворяет этому тривиально. Комментарий dev'а в `e958661` называет этот оракул
  причиной выбора — то есть величина подобрана ПОД оракул.

`testing.md` §«Оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ» п.1-2: «Назови, что именно считает
цифра… Оракул границы ресурса меряет ресурс, а не прокси».

**Сопутствующее противоречие в поставке.** Комментарий `4d7642a` (`lib.rs:1145-1152`)
утверждает «warm: 0; cold: сумма всех `.jrnl`». Коммит `e958661`, идущий РАНЬШЕ в той же
ветке, сделал warm = сумма всех `.jrnl`. Два коммита одной поставки описывают одно
поведение взаимоисключающе; один из них заведомо лжёт читателю.

## Б-3 — задача 24б: усечение `pump_one` на 8 МБ обосновано ДОГАДКОЙ, и догадка опровергнута замером

**Где:** `crates/gateway-serve/src/admission.rs:381-382` (коммит `7be2ada`).

```rust
const MAX_SEGMENT_BYTES: usize = 8 * 1024 * 1024; // 8 МБ — потолок на сегмент
```

Комментарий dev'а прямо признаёт, что опоры нет: «`max_segment_bytes: 8 * 1024` в
`WriterConfig` — это МАКСИМАЛЬНОЕ число фреймов, не байт; реальный потолок байт у
`journal::Writer` значительно выше, **но продовый сегмент укладывается в
`MAX_SEGMENT_BYTES`**».

**Замер опровергает:**

```
$ ssh … 'find $D -name "*.jrnl" -size +8M | wc -l'
11
```

**Все 11 из 11 сегментов прода — ~1 ГБ, то есть в 128 раз больше потолка.** Ни один не
укладывается. Следствия:

1. `step.payload_bytes` занижен ~в 128 раз — бюджет по байтам неверен в ОПАСНУЮ сторону
   (считает, что прочитано мало, когда прочитано много);
2. эвристика `step.events = buf.len() / 64` занижена тем же множителем;
3. `buf` накапливает до 8 МБ и используется ТОЛЬКО под `buf.len()` — аллокация ради
   счётчика длины.

`testing.md` §«Форма прода снимается ЗАМЕРОМ, а не воображается» требует снять форму
sacred I/O-пути ДО коммита и назвать её в спеке. Здесь форма воображена, названа в
комментарии как факт и оказалась ложной.

---

# ПРИМЕЧАНИЯ (не блокируют, но входят в условие APPROVED)

**Н-1 — статусы §Tasks не приведены к факту.** На вершине `8a5582d` задачи **10, 15,
16, 22, 23, 24, 25** стоят `⏳ OPEN` (`milestones/M-87-serving-circuit-breaker.md:498,
509, 510, 516-519`), хотя по 22-25 коммиты есть, а шаг `verify` «task10» зелёный и
§16.1 непуста (19 файлов). Спека сама объявляет расхождение статуса и факта дефектом
(строка задачи 15: «Ложь в спеке в эту сторону хуже прежней»). Направление здесь более
мягкое (`OPEN` при готовом коде), но по задачам 23/24 статус `OPEN` **фактически верен**
— см. Б-1/Б-2. Привести к факту одним коммитом ПОСЛЕ устранения блокеров.

**Н-2 — задача 25 исполнена верно; один остаточный край.** `f1ac6a4` заменяет
`load`+`store` на `SLOTS_IN_FLIGHT_GLOBAL.fetch_sub(1, SeqCst)` — дефект снят по
существу, асимметрия с `fetch_add` устранена. Край: `fetch_sub` на беззнаковом при
нуле ЗАВОРАЧИВАЕТСЯ в `u64::MAX`, тогда как снятый `saturating_sub` — нет. Сегодня
пары `acquire`/`drop` держит RAII-guard, так что недостижимо; но `slots_in_flight` —
единственная величина, по которой видна идущая работа предохранителя, и её заворот
был бы неотличим от «сервис перегружен навсегда». Либо `debug_assert!(prev > 0)`, либо
строка в спеке о том, почему заворот недостижим.

**Н-3 — задача 22 исполнена верно.** `1606b61`: ветка `checkpoint_dir: None` больше не
объявляет `Ready` по умолчанию, а возвращает `ServingOutcome::NotReady` с отправкой
названного исхода клиенту. Fail-open снят. Возражений нет.

**Н-4 — задача 21 исполнена верно, и compose согласован.** Пять переменных объявлены
формой `${VAR:-default}`; проверено построчно, что дефолты согласованы с прод-значениями
того же блока: `ALLOWED_SYMBOLS=BTCUSDT` ↔ `GATEWAY_SYMBOL:-BTCUSDT` (`:16`),
`ALLOWED_PROFILES=1000/60000/1000` ↔ `TIMEFRAME_MS:-1000` (`:17`) / `WINDOW_MS:-60000`
(`:35`) / `DEPTH_CADENCE_MS:-1000` (`:47`). Прод-бинарь на этом окружении поднимается
(`red_m87_prod_entrypoint_argv` 3/3).

**Н-5 — шаг приёмки «ПРОД» остаётся неснятым, и это НЕ придирка.** `verify` даёт
`SKIP task9`, §15 требует `docker inspect` по фактически применённым лимитам, а замер
2026-09-21 уже ловил `NanoCpus=0 Memory=0` при живом описании сервисов. Форма
`deploy.resources.limits` в `docker compose up` (не Swarm) ИГНОРИРУЕТСЯ — ровно тот
механизм, что дал прошлый нулевой замер. Снять `docker inspect` обязательно на
post-merge гейте §8; при нулях — завести TD «built-not-wired: лимиты объявлены, не
применены».

**Н-6 — §9 перепроверка.** Диф трогает `docs/fa/viz-backend.md` и `scripts/verify_M-87.sh`
— уставную зону `gates.md` §9. В цепочке есть вердикты сильной модели со свежим
контекстом (`C-245`…`C-254`, `A-037`…`A-040`), покрывающие оракулы и verify. Отдельного
артефакта §9 по FA-правке нет; правка мелкая и производна от `A-037` D-4, поэтому
блокером не делаю — но назвать это в close-out обязательно.

**Н-7 — состояние мира (ярус S).** `main` зелёный (`CI success`, `Deploy success`
2026-09-24T14:38Z). Прод на `46e5cc0` при `origin/main` `3a2e460` — отставание штатное:
`deploy.yml` триггерится по путям `crates/**`, а последние коммиты `main` — docs-only.
Контейнеры `hft-gateway-serve` / `hft-recorder` — `Up 46 hours (healthy)`.

---

# УСЛОВИЕ APPROVED

1. **Б-1:** либо бюджет подключается ПО-НАСТОЯЩЕМУ (реальные пределы, исход `BudgetStop`
   читается и ОСТАНАВЛИВАЕТ дренаж), либо вызов-заглушка снимается целиком и задача 24
   остаётся `OPEN` с TD-записью «built-not-wired» severity MAJOR. Третьего не дано:
   держать в дереве вызов, который ничего не ограничивает и стоит 88 МБ на итерацию, —
   худший из трёх исходов. **Оракул `c3` переписывается architect'ом на проверку ПО
   РАБОТЕ** (дренаж на бюджете, исчерпаемом фикстурой, ОБЯЗАН остановиться названным
   `BudgetStop`), потому что в нынешней форме он не отличает механизм от его имени.
2. **Б-2:** `payload_bytes_read` на WARM-пути кормится РЕАЛЬНО прочитанным (загрузка
   слепка + хвост `pump_stats`), `payload_bytes_for_dir` с этого пути уходит. **Оракул
   `c2` переписывается** на проверку свойства, а не литерала: счётчик НЕ растёт, когда
   журнал не читался, и растёт РОВНО на прочитанное — с верхней границей, отличающей
   «хвост» от «весь каталог».
3. **Б-3:** `MAX_SEGMENT_BYTES` либо обосновывается ЗАМЕРОМ прода (сегодня: 11 × ~1 ГБ),
   либо усечение снимается в пользу потоковой обработки без накопления в `buf`.
   Комментарий, утверждающий «продовый сегмент укладывается», удаляется как ложный.
4. **Н-1:** статусы §Tasks приведены к факту одним коммитом architect'а.
5. Приёмка перепрогоняется целиком; `verify` обязан покраснеть ДО фиксов по Б-1/Б-2 на
   переписанных оракулах — иначе переписывание не состоялось.

**Кому:** `engine-dev` — по Б-1/Б-2/Б-3 (зона `crates/gateway-serve/src/**`,
`crates/gateway/src/lib.rs`). **Оракулы `c2`/`c3` — architect, sacred** (`scope-guard.md`:
`*/tests/**` dev не правит; при несогласии — `!!! SCOPE VIOLATION REQUEST !!!`).

---

# Предъявление FA (M-66)

Диф трогает `crates/gateway-serve/**` и `crates/gateway/**`. FA зоны —
`docs/fa/viz-backend.md` (собственного `fa/gateway-serve.md` не существует, и этот факт
сам по себе долг — `reading-map.md` §2, ярус B).

Живые инварианты на проверяемой ревизии, использованные по существу:

- **`VB-I-10`** (`docs/fa/viz-backend.md:208`) — «Bounded-window snapshot (M-37,
  `TD-039`). Память `snapshot`/`frames_since` ограничена ОКНОМ `[at−W, at]` …, НЕ числом
  time-бакетов истории — иначе host-OOM (прод: RSS 7.3GB)». **Опора Б-1 и Б-3:** вызов
  бюджета-заглушки добавил на путь выдачи неограниченное окном чтение (88 МБ/итерацию),
  а `MAX_SEGMENT_BYTES` объявил границу, не снятую с прода.
- **`VB-I-11`** (`:209`) — «Провенанс ИСТОРИИ (M-48, `TD-048`)… Система не отказывается
  отдать то, что есть, но обязана НЕ ВЫДАВАТЬ ЭТО ЗА ДРУГОЕ». **Опора Б-2** по классу:
  счётчик, отчитывающийся о 92 ГБ прочитанного там, где не прочитано ничего, выдаёт одно
  за другое — тот же класс нечестности, но на ресурсной величине.
- `VB-I-3` (`:201`) — read-only Read Gateway; проверено, что диф его не нарушает
  (order-egress и journal-writer отсутствуют), — основание для «Блок-risk: N/A».

**Ярус C — что искал грепом** (`reading-map.md` §2: «прочитал TECH-DEBT» — заведомо
ложное утверждение): `TECH-DEBT.md` по `TD-207`, `TD-011`, `TD-202`, `TD-217`,
`gateway-serve`, `built-not-wired`; `PROJECT-STATE.md` по `M-87`, `gateway-serve`,
`предохранител`. Найденное и относящееся к предмету: `TD-207` (зонд `wsprobe` против
прод-формы секрета) — закрывается задачами 7/11 этой поставки, зелёный шаг `task11`;
закрытие карточки — на close-out'е ПОСЛЕ устранения блокеров, не сейчас.

---

# МУТАЦИОННЫЙ КОНТРОЛЬ — оракулы `c2`/`c3` предъявлены ПЛАЦЕБО прогоном

`testing.md` §«Мутационный контроль»: «Нейтрализуй строки, которые оракул обязан
защищать. Не упал → оракул ничего не пиннит». Обе мутации применены на чистом
чекауте `/tmp/hft-reviewer-m87` @ `8a5582d` и откачены.

## Мутация 1 — величина счётчика заменена КОНСТАНТОЙ (Б-2)

```
$ sed -i 's|payload_bytes_read: payload_bytes_for_dir(dir).unwrap_or(0),|payload_bytes_read: 1, // MUTATION|' crates/gateway/src/lib.rs
$ git diff --numstat crates/gateway/src/lib.rs
1	1	crates/gateway/src/lib.rs

$ cargo test -p gateway-serve --test red_m87_r196_conditions
test c2_payload_counter_is_not_fed_by_response_size ... ok
test c3_call_budget_has_a_caller_in_production_code ... ok
test c4_manual_meta_read_agrees_with_journal_api ... ok
test c1_missing_checkpoint_dir_refuses_instead_of_declaring_ready ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s

$ cargo test -p gateway-serve --features testing --test red_m87_entrypoint u1_served
test u1_served_request_with_tail_increments_payload_counter ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.08s
```

**«Главная цифра милестоуна» заменена на литерал `1` — НИ ОДИН оракул не покраснел.**
`c2` зелен, потому что ищет подстроку `snap_text.len()`; `u1` зелен, потому что
требует лишь `counter > before`, а `1 > 0`. Оракулы не пиннят ни смысл величины, ни её
порядок. Размер каталога (Б-2) проходит их ровно по той же причине, по какой проходит
бессмысленная единица.

## Мутация 2 — текст вызова СОХРАНЁН, исполнение выключено (Б-1)

```
$ python3 …  # `feed_tail_within(…)` обёрнут в `if true { None } else { … }`
мутация применена
$ git diff --numstat crates/gateway-serve/src/lib.rs
3	2	crates/gateway-serve/src/lib.rs

$ cargo test -p gateway-serve --test red_m87_r196_conditions
test c3_call_budget_has_a_caller_in_production_code ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
```

**Вызов бюджета НЕ ИСПОЛНЯЕТСЯ НИ РАЗУ — `c3` зелен.** Он считает строки, содержащие
текст `feed_tail_within(`, и мутация этот текст сохранила. Оракул проверяет наличие
ИМЕНИ в исходнике, а не подключение МЕХАНИЗМА к пути — то есть ровно то, что
`gates.md` §4 («Механизм на пути») и `testing.md` §«Механизм несущего пути обязан иметь
оракул точки входа» запрещают принимать за доказательство.

## Откат и регрессия

```
$ git checkout -- crates/gateway/src/lib.rs crates/gateway-serve/src/lib.rs
$ git status --porcelain
?? research/reviews/R-201-M-87-r196-conditions-pr-gate.md
$ cargo test -p gateway-serve --test red_m87_r196_conditions
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
```

Дерево чисто (единственный untracked — ЭТОТ вердикт), оракулы восстановлены.

**Вывод, обязательный к исполнению:** зелёная приёмка на этой ревизии НЕ является
свидетельством исполнения задач 23 и 24. Переписывание `c2`/`c3` — не «желательное
улучшение», а условие того, чтобы следующий круг вообще мог что-то доказать.

---

# Done Block — сырой stdout

## Шаг 0 — что именно судится

```
$ pwd
/tmp/hft-reviewer-m87

$ git rev-parse HEAD
8a5582dc1e3e2ec6b43c917ec38a09e61448e2bb

$ git rev-parse origin/feat/M-87-serving-circuit-breaker
8a5582dc1e3e2ec6b43c917ec38a09e61448e2bb

$ git rev-parse origin/main
3a2e4607882756f660d664be5d2ba45780af0a4d

$ git rev-list --count origin/feat/M-87-serving-circuit-breaker..origin/main
0                      # ветка СОДЕРЖИТ весь main — судится дерево слияния

$ git rev-list --count origin/main..origin/feat/M-87-serving-circuit-breaker
113
```

## Приёмка — ПОЛНЫЙ вывод (собственный прогон, не отчёт tester'а)

```
$ bash scripts/verify_M-87.sh; echo "VERIFY_EXIT=$?"
PASS  task1: пять исходов объявлены в admission.rs
PASS  task2: библиотечный оракул изъят; готовность судится в транспорте
PASS  task1+3+4+7: red_m87_admission — test result: ok. 21 passed; 0 failed; …
PASS  task2+3+5+6+8: red_m87_entrypoint — test result: ok. 14 passed; 0 failed; …
PASS  task4: ReadStats несёт payload_bytes_read
PASS  task5: ограничитель параллелизма присутствует (файлов: 2)
PASS  task6: счётчики выдачи разделяют поддержанные и неподдержанные отказы
PASS  task7: секрет трактуется ОДНОЙ функцией, её зовут обе стороны
PASS  A-039: реестр общего порога — test result: ok. 10 passed; 0 failed; …
PASS  A-039: дописок-литералов в предмете нет — число живёт в реестре и только там
PASS  A-039: политика предмета берёт оба числа из источника
PASS  A-040: биекция харнесса — перечни совпали (15 сценариев)
PASS  task11: направление трактовки секрета — test result: ok. 3 passed; 0 failed; …
PASS  task12: порог свежести переживает измеренную каденцию — ok. 7 passed; …
PASS  task13: различитель подменного пути ЗЕЛЁН — ok. 1 passed; 0 failed; …
PASS  task13: набор точки входа ПОД ФЛАГОМ testing — ok. 15 passed; 0 failed; …
PASS  task8: свежесть представлена в коде выдачи (совпадений: 9)
PASS  task9: процессор И память ограничены у ВСЕХ трёх классов (выдача, запись, прогреватель)
SKIP  task9: ФАКТИЧЕСКИ применённые лимиты … снимаются на проде (docker inspect) — шаг деплой-гейта §8
PASS  task10: решение с токеном записано по каждому из 19 файлов и по двум тестам D-4
PASS  D-1(а): библиотечный корпус тронут только добавлениями red_m87_*
PASS  D-1(б): дифф оракулов D-1 — ровно ожидаемая форма
PASS  D-4: два теста пути слепка переведены на прогретую фикстуру
PASS  task19: ловушка порчи ЗАМЫКАЕТСЯ — ok. 1 passed; 0 failed; …
PASS  task20: сбой провенанса не выдаётся за полную историю — ok. 5 passed; 0 failed; …
PASS  task20: оба сайта зовут fail-closed обёртку (вызовов: 2, записей признака: 2)
PASS  task20: честность истории на пути ПОДПИСКИ — ok. 2 passed; 0 failed; …
PASS  task14: прод-бинарь поднимается на окружении ИЗ compose — ok. 3 passed; 0 failed; …
PASS  task16: четыре условия R-196 (fail-closed без слепка · счётчик · бюджет · метаданные) — ok. 4 passed; …
PASS  task16: глобальный счётчик слотов не теряет декременты под гонкой — ok. 1 passed; …
PASS  CI-паритет: cargo fmt --all -- --check
PASS  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
PASS  CI-паритет: cargo test --all

VERDICT: PASS
VERIFY_EXIT=0

$ grep -cE "^PASS" ; grep -cE "^FAIL" ; grep -cE "^SKIP"
30
0
1
```

**Приёмка ЗЕЛЁНАЯ, и вердикт всё равно REJECT.** Расхождение не случайно и не является
спором с прогоном: мутационный контроль выше показывает, что зелёный цвет здесь не
несёт информации о задачах 23 и 24.

## Scope и RED-first

```
$ git diff --name-only origin/main...8a5582d | wc -l
55

$ git log --format='%s' origin/main..8a5582d -- 'crates/*/tests/*' | sed -E 's/.*\[([a-z-]+)\]$/\1/' | sort | uniq -c
      1 0272260 Merge remote-tracking branch 'origin/main' into sync-m87
     34 architect

$ git log --oneline origin/main..8a5582d -- crates/gateway-serve/tests/red_m87_r196_conditions.rs
621b216 test(M-87): R-200 §B7 — пять условий R-196 проверяются ПО РАБОТЕ, не по имени [architect]

$ git log --oneline 621b216..8a5582d -- scripts/verify_M-87.sh
(пусто — verify после оракула не правился)
```

## Барьеры харнесса

```
$ EVENT_NAME=pull_request PR_BASE_SHA=3a2e460… bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 22, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 1
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=3a2e460… bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 3a2e460..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```

## Замер прода (основание Б-1 и Б-3)

```
$ ssh -i /home/nous/.ssh/hft_deploy root@167.233.192.131 \
    'D=/var/lib/docker/volumes/hft-platform_journal-data/_data;
     ls -1 $D/*.jrnl | wc -l; du -sh $D; ls -lS $D/*.jrnl | head -1;
     find $D -name "*.jrnl" -size +8M | wc -l'
11
92G	/var/lib/docker/volumes/hft-platform_journal-data/_data
1073741780 /var/lib/docker/volumes/hft-platform_journal-data/_data/segment-00000822.jrnl
11
```

11 сегментов, каждый ~1 ГБ, ВСЕ 11 больше `MAX_SEGMENT_BYTES` = 8 МБ.

## Состояние мира (ярус S)

```
$ gh run list --branch main --limit 2
completed  success  Deploy to VPS  main  workflow_run  36014223093  16s  2026-09-24T14:38:22Z
completed  success  Merge pull request #223 …  CI  main  push  36013247738  8m19s  2026-09-24T14:30:00Z

$ ssh … 'cd /root/hft-platform && git rev-parse --short HEAD; docker ps --format "{{.Names}} {{.Status}}"'
46e5cc0
hft-gateway-serve Up 46 hours (healthy)
hft-recorder Up 46 hours (healthy)
```

Отставание прода от `origin/main` штатное: `deploy.yml` триггерится по путям
`crates/**`, последние коммиты `main` — docs-only.

## Чистота дерева на момент вердикта

```
$ git status --porcelain
?? research/reviews/R-201-M-87-r196-conditions-pr-gate.md
```

---

**Reviewer, 2026-09-25. Вердикт: REJECT.** `PROJECT-STATE.md` и `TECH-DEBT.md` НЕ
обновляются — они обновляются только на APPROVED (`gates.md` §4).
