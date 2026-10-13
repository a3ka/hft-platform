<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: b9d05d1905eeeb916ab8c0cf9bc53a8ecfe4a975
audited_head: 255c359af9d9613f2b3bce03ae04b0c364a68277
verdict: REJECT
-->

# R-141 — M-68 круг 3 impl (задачи 12, 21, 22): PR-time reviewer, **REJECTED**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-28
**Предмет:** `b9d05d1..255c359` на `origin/feat/M-68-rev4` — шесть коммитов: три RED
architect'а (`52377a1` d12-перестройка, `1e3a0ca` d17, `b089c7d` d18 + carve-out зоны) и три
impl engine-dev'а (`a9f89d9` задача 12, `10d6ebe` задача 21, `255c359` задача 22).
**Дерево ревью:** `/tmp/hft-reviewer-m68-rev4`, detached на `255c359`, чистый чекаут из origin.
**Мандат:** проверить исполнение трёх блокеров `R-138` (Б-1 close, Б-2 НОК, Б-3 проводка).

**Прочитано на этой ревизии:** `milestones/M-68-depth-from-book.md` (§0sexies.2bis/2ter/2quater/
2quinquies, §3/§3.1, §4), `R-138` целиком, `docs/fa/viz-backend.md` §4/§5, `docs/04-workflow.md`
§2/§3, `docs/05-contract-layer.md` §4/§6, `crates/gateway/src/lib.rs` в тронутых местах +
`finish`/`finish_ref`/`LiveReducer`, `crates/gateway-serve/src/lib.rs` (`serve_config_from_env`,
legacy- и v1-сессии), `crates/gateway/src/bin/gateway-checkpoint.rs`, `docker-compose.yml`,
`red_depth_cadence.rs` (`d12`), `red_depth_cadence_from_env.rs` (`d18`),
`red_gateway_live_eq_replay.rs`. **Ярус C — грепом по предмету, не целиком:** `TECH-DEBT.md`
по `TD-158|TD-159|TD-161|TD-167|M-68` (:92, :93, :94, :100, :778, :798, :826, :829, :1149-1187),
`PROJECT-STATE.md` по `M-68|depth_cadence|GATEWAY_DEPTH_CADENCE` — **совпадений ноль**.

**Предъявление FA (M-66).** Диф трогает `crates/gateway/src/**` и `crates/gateway-serve/src/**`
⇒ живые инварианты названы прямым чтением `docs/fa/viz-backend.md` на этой ревизии:
**`VB-I-1`** (`:188` — «каждый индикатор — чистый редьюсер над `journal::stream`;
детерминизм-тест обязателен») и **`VB-I-2`** (`:189` — «live == replay: серия, посчитанная на
live-хвосте, бит-идентична серии из replay того же окна журнала»). `VB-I-1` **held**: каденция
ведётся временем события, wall-clock/rand в новом коде нет. **`VB-I-2` НАРУШЕН — блокер Б-2
ниже, установлен замером, а не чтением.** `FA-WAIVER` не требуется (оба крейта имеют FA).

---

## Block-scope — ЧИСТО

| коммит | пути | вердикт |
|---|---|---|
| `a9f89d9`, `10d6ebe` | `crates/gateway/src/lib.rs` | ✅ штатная зона engine-dev (§3) |
| `255c359` | `crates/gateway-serve/src/lib.rs`, `docker-compose.yml` | ✅ carve-out §3 (решение founder'а 2026-08-27), названный спекой поимённо |
| `52377a1`, `1e3a0ca`, `b089c7d` | `crates/gateway/tests/**`, `crates/gateway-serve/tests/**`, `milestones/M-68-*.md`, `scripts/verify_M-68.sh` | ✅ зона architect'а |

**Тесты dev не трогал ни в одном из трёх коммитов** (`git show --name-only` по каждому) —
RED-first цел, sacred-зона не задета. `crates/contracts/**` не тронут (**Block-C: ЧИСТО**,
шаг `H` гейта). `crates/book|venue-*|journal` не тронуты (шаг `K`). `GATEWAY_BANDS` не тронут
(шаг `I`). `selector_fingerprint` не переписан (шаг `J`).

**RISK-BLOCK не применяется** — проверено, а не предположено: диапазон не трогает
`crates/risk|killswitch|oms|venue-*|contracts`; предмет — Слой 8, read-only консюмер журнала
(`VB-I-3`), order-egress отсутствует. risk-critic по `gates.md` §5 не требуется.

## Block-commits — ЧИСТО

Три задачи — три коммита, каждый называет номер задачи, блокер `R-138` и оракул. Бандла нет.
`git log --format=%B b9d05d1..255c359 | grep -c 'Co-Authored-By'` → **0**.

## Block-DoneBlock — ВОСПРОИЗВЕДЁН СВОИМ ПРОГОНОМ

Done Block тестера не пересказан, а перепрогнан в собственном дереве: `VERDICT: PASS`,
`verify_exit=0`, **27 PASS-строк, 0 FAIL** (сырой вывод — в Done Block ниже).

**Расхождение в числе названо, потому что число в отчёте гейта СЧИТАЕТСЯ, а не заявляется:**
тестер дважды написал «28 PASS-строк». В моём прогоне их 27, и в ЕГО СОБСТВЕННОМ логе
(`/tmp/verify_m68_full.log`, `grep -c '^PASS'`) — тоже **27**. Состав шагов совпадает
поимённо; расходится только заявленное число. На вердикт не влияет, но это `N-3`.

---

## Б-1 (БЛОКЕР) — прод-чекпоинт становится непригоден: писатель и читатель разошлись отпечатком

**Замер, а не чтение.** Задача 18 (круг 2) внесла `depth_cadence_ms` в `selector_fingerprint`
(`lib.rs:2825`), а имя файла чекпоинта — это и есть отпечаток (`ckpt_path_for`, `:2666-2671`).
Задача 22 (этот круг) сделала прод-дефолт `gateway-serve` равным `Some(1000)`. **Писатель
чекпоинта остался на `None`:** `crates/gateway/src/bin/gateway-checkpoint.rs:247` —
`depth_cadence_ms: None`, ручки у бинаря нет (`grep GATEWAY_ crates/gateway/src/bin/
gateway-checkpoint.rs` → только сообщение о `TIMEFRAME_MS`), и `docker-compose.yml`
сервису `gateway-checkpoint` (`:197-215`) каденцию не передаёт — такого флага не существует.

Проба reviewer'а (временный файл, прогнан и УДАЛЁН; дерево чистое):

```
R139-CKPT written_cursor=Cursor { upto_seq: Some(119) } files=["ckpt-9e7e403ce976f21e.bin", "zz.lock"]
R139-CKPT fp(writer cadence=None)=9e7e403ce976f21e  fp(reader cadence=Some(1000))=b3a743216e64d3e7
R139-RESUME [A: читатель = писатель (cadence None)]   events_decoded=0   events_scanned=0   segments_opened=0
R139-RESUME [B: прод после задачи 22 (cadence 1000)]  events_decoded=120 events_scanned=120 segments_opened=1
```

Читатель ищет `ckpt-b3a743216e64d3e7.bin`, которого нет и не будет: **`read_checkpoint`
возвращает `None` (silent rebuild) ⇒ ПОЛНЫЙ РЕПЛЕЙ журнала на КАЖДОМ подключении.** Легаси-
сессия прода резюмируется именно `cfg.selector` (`gateway-serve/src/lib.rs:1347-1351`), то есть
селектором с новым дефолтом.

**Это ровно та цена, ради снятия которой построены M-38b/M-48/M-54:** `TD-044` — 409 s реплея
при подключении, `R-029` §C/`TD-097` — +404 ms константы. Мы её возвращаем целиком, а
чекпоинт, который каждые 15 минут пишет ops-cron (`deploy/cron.d/journal-retention:79`),
становится мёртвым грузом.

**Утверждение спеки «ПОВЕДЕНИЕ ПРОДА НЕ МЕНЯЕТСЯ — замер, не обещание» (§0sexies.2quinquies)
ложно в этом измерении.** Замер там снимался по КЛЮЧАМ депт-серии (100 событий → 10 ключей в
обоих случаях) и потому не мог увидеть композицию писателя и читателя. Класс — `testing.md`,
канарейка точки входа, пункт 2: «путь, куда пишет producer, совпадает с путём, откуда читает
consumer — рассогласование двух строк даёт тихий no-op». Здесь оно даёт не no-op, а
восстановленную латентность.

**Ни один оракул этого не пиннит:** `d18` судит `serve_config_from_env` в отрыве от писателя;
`d15`/`d15-C` судят, что смена каденции ИНВАЛИДИРУЕТ чекпоинт (и это работает) — но никто не
судит, что прод-писатель и прод-читатель договорились об ОДНОМ значении.

## Б-2 (БЛОКЕР) — `VB-I-2` (live == replay) нарушен при включённой каденции

**Замер той же пробы, одна фикстура, два пути (`gateway::snapshot` против
`LiveReducer::resume+pump+snapshot`):**

```
R139 cadence=None        replay_pts=240 live_pts=240 identical=true
R139 cadence=Some(1000)  replay_pts=240 live_pts=238 identical=false
      replay_tail=[(1752000119, 619000000), (1752000118, 618000000)]
      live_tail  =[(1752000118, 618000000), (1752000117, 617000000)]
R139 cadence=Some(10000) replay_pts=24  live_pts=22  identical=false
R139 cadence=Some(60000) replay_pts=4   live_pts=2   identical=false
      replay_tail=[(1752000060, 619000000), (1752000000, 559000000)]
      live_tail  =[(1752000000, 559000000)]
```

**Причина — структурная, названа строкой кода.** Сброс незакрытого интервала висит на
`finish(self)` (`lib.rs:1326-1334` → `flush_pending_depth_interval`), а live-путь идёт через
`LiveReducer::snapshot(&self)` → `finish_ref_with_at(&self)` → `finish_ref(&self)`
(`:3796-3797`, `:1493-1494`) — по сигнатуре `&self` он сбросить ничего не может. Реплей
сбрасывает, live — нет. При каденции 60 с live теряет ПОЛОВИНУ точек.

**Оракул слеп по построению:** `red_gateway_live_eq_replay.rs:120` задаёт
`depth_cadence_ms: None` — то есть единственный оракул `VB-I-2` гоняет ровно тот режим, в
котором расхождения нет. Шаг `F` гейта зелен и к предмету безразличен.

**Прод-цена — не гипотетическая, и она создана ЭТИМ кругом:** до задачи 22 прод жил на `None`
(колонка `identical=true`), после — на `Some(1000)`, то есть на расходящейся ветке. Кокпит
получает снапшот без САМОЙ СВЕЖЕЙ точки глубины, а реплей того же окна её содержит.

**Обе трактовки допустимы, и я не выбираю за архитектора:** либо сброс незакрытого интервала
обязан жить на пути, которым ходит live (тогда правится `finish_ref`-ветка), либо частичная
точка незакрытого интервала не должна эмититься вовсе (тогда правится `finish`, и §0sexies.2ter
«незакрытый последний интервал сбрасывается на `finish`» пересматривается). Запрещено
третье — то, что сейчас: два пути с разной семантикой при инварианте `VB-I-2`.
§3.1 милестоуна запрещает это прямо: «менять семантику `close` … — `VB-I-2` live == replay и
merge-путь `Snapshot::apply` стоят на ней».

## Б-3 (MAJOR) — гвард отношения не поднят на СТАРТ: контейнер здоров, ошибку получает каждый клиент

Задача 21 поставила гвард `cadence_ms % timeframe_ms` в `validate_selector`
(`lib.rs:2260-2280`) — правильно. Но `serve_config_from_env` **дублирует** проверки каденции
своими (`gateway-serve/src/lib.rs:2046-2092`: `>= 1000`, делимость суток) и отношения среди
них НЕТ. Замер пробы:

```
R139-START tf=1000 cadence=10000 | СТАРТ OK … | validate_selector: OK
R139-START tf=3000 cadence=10000 | СТАРТ OK, selector.timeframe_ms=3000 depth_cadence_ms=Some(10000)
                                 | validate_selector ОТКАЗ (на клиентском пути): MD-I-8 d17 …
R139-START tf=3000 cadence=1000  | СТАРТ OK … | validate_selector: OK
```

То есть пара `GATEWAY_TIMEFRAME_MS=3000` + `GATEWAY_DEPTH_CADENCE_MS=10000` поднимает
ЗДОРОВЫЙ по healthcheck контейнер, который отвергает каждое подключение. Это дословно тот
исход, который осуждает комментарий в ЭТОМ ЖЕ файле двумя сотнями строк выше (`:1859-1864`,
урок `TD-019`/`TD-020`: «§8 eyes-on увидит `(healthy)`, а кокпит будет пуст»), и он же
противоречит формулировке задачи 22 «невалидное → отказ СТАРТА с именем переменной».

## N-1 (NOTE) — подписанный дефолт 1000 мс не доходит до v1-пути

`serve_config_from_env` задаёт каденцию только `cfg.selector`, которым пользуется ЛЕГАСИ
env-сессия (`:1308-1312`, `:1347`). Подписки протокола v1 (`CT-RFC-09`, M-65) строят селектор
из клиентского JSON (`wire_v1::parse_selector`, `wire_v1.rs:120-143`), а поле объявлено
`#[serde(default)]` (`gateway/src/lib.rs:143-146`) ⇒ клиент, не приславший `depth_cadence_ms`,
получает `None`. Итого в одном сервисе ДВА разных дефолта одной ручки: `Some(1000)` на легаси
и `None` на v1. Существующие клиенты не ломаются (это хорошо), но «значение настраивается,
дефолт 1000» на пути, которым пойдёт фронт, не выполняется. Класс `A-015` §3 п.1 / `A-026` §1
(«поведение не имеет права зависеть от того, КАК ручку забыли задать») — здесь оно зависит от
того, КАКИМ путём пришёл клиент. Оракул `d18` этого не видит: он судит только `serve_config`.

## N-2 (NOTE) — статус-колонка §Tasks по-прежнему лжёт; условие (4) `R-138` не исполнено

`R-138` назвал условием APPROVED «колонка приведена к правде». На `255c359`:
задачи **1-10 и 13** стоят `⏳ OPEN` при зелёном гейте (реализованы `44d6aac`), задачи
**21 и 22** — тоже `⏳ OPEN`, хотя закрыты ЭТИМ кругом (`10d6ebe`, `255c359`). Правку статуса
`scope-guard.md` разрешает dev'у прямо (carve-out колонки Status). Класс `TD-167`, седьмое
срабатывание на M-68.

## N-3 (NOTE) — заявленное число PASS не совпало с логом

См. Block-DoneBlock: «28 PASS-строк» против фактических 27 в обоих логах. Отчёт гейта обязан
СЧИТАТЬ, а не заявлять (`gates.md` §9, формулировка про пробу).

## N-4 (NOTE) — шаг `I` гейта грепает контекст диффа

`scripts/verify_M-68.sh:208` — `git diff … | grep -q 'GATEWAY_BANDS'` без фильтра `^[+-]`:
сейчас зелен только потому, что новая переменная легла далеко от `GATEWAY_BANDS`. Любая
правка compose рядом с ней даст ЛОЖНОЕ КРАСНОЕ. Наблюдение совпадает с замечанием тестера;
файл sacred (architect-only), сам не правлю.

---

## Что проверено и дефекта НЕ найдено — названо явно

- **`R-138` Б-1 (close) ЗАКРЫТ на replay-пути — замером.** Каденция 60 с даёт точки
  `t0 → 559000000` (событие `i=59`) и `t0+60 → 619000000` (`i=119`) — ПОСЛЕДНИЕ наблюдения
  своих интервалов. Прежняя реализация давала `500000000` (первое). Конструкция ролловера
  (`maybe_commit_depth_interval` ДО `book.apply_*`, `:1246-1262`) семантике соответствует, и
  стоимость осталась «одно чтение книги на интервал» — `recompute_depth_from_book` для
  каденс-режима NO-OP (`:1197`), legacy-путь при `None` сохранён.
- **`R-138` Б-2 (гвард отношения) закрыт в `validate_selector`** — замер: `(1000, 10000)`
  accept, `(3000, 10000)` reject с сообщением, называющим обе величины и остаток. Развязка А
  §0sexies.2quater, конструкция не выходит за разрешённую.
- **`R-138` Б-3 частично закрыт:** значение из env доходит до селектора, отсутствие ≡ пустое ≡
  пробельное → 1000, невалидное → отказ старта с именем переменной, переменная объявлена в
  `docker-compose.yml`. Остатки — Б-1/Б-3/N-1 выше.
- **Соседние инварианты не куплены:** `red_gateway_bounded`, `red_snapshot_noclone`
  (`VB-I-10`), `red_depth_provenance_by_reach`, `red_gateway_schema_version`,
  `red_depth_from_book` (9/9), мутация `C-M68-1` (шаг `B`) — зелены.
- **Форма чекпоинта:** новое поле `Reducer::depth_cadence_current_bucket` попадает в
  postcard-состояние (`Reducer` несёт `Serialize/Deserialize`, `:612`), но
  `GATEWAY_SCHEMA_VERSION` уже поднят 8→9 задачей 9 — старые чекпоинты отвергаются штатно,
  скрытой несовместимости нет.

---

## ВЕРДИКТ: **REJECTED**

Два блокера, оба установлены ЗАМЕРОМ и оба созданы этим кругом:

1. **Б-1** — прод-чекпоинт непригоден: писатель (`gateway-checkpoint`, `None`) и читатель
   (`gateway-serve`, `Some(1000)`) разошлись отпечатком ⇒ полный реплей на каждом
   подключении (`TD-044`-класс).
2. **Б-2** — `VB-I-2` нарушен: live-снапшот теряет незакрытый интервал, реплей его отдаёт;
   при каденции 60 с расхождение вдвое. Единственный оракул `VB-I-2` гоняет `None` и слеп.

**Условие APPROVED:**
1. Б-1 — прод-писатель и прод-читатель обязаны договориться об ОДНОМ значении каденции
   (ручка у `gateway-checkpoint` + её проводка в compose, ЛИБО возврат дефолта serve к `None`
   до появления такой ручки), и композиция обязана быть предъявлена оракулом точки входа —
   не грепом. Решение о ФОРМЕ развязки — architect/founder, не reviewer (`gates.md` §4,
   граница reviewer↔architect).
2. Б-2 — семантика незакрытого интервала одинакова на обоих путях, и это пиннит оракул
   `VB-I-2` С ЗАДАННОЙ каденцией (сегодня `red_gateway_live_eq_replay` её не задаёт). Оракул
   sacred ⇒ architect.
3. Б-3 — гвард отношения зеркалится в `serve_config_from_env` (отказ на СТАРТЕ), либо
   старт-гейт зовёт `gateway::validate_selector` целиком вместо ручного дубля.
4. N-2 — колонка §Tasks приведена к правде (carve-out dev'а).

**Маршрут.** Б-2 и Б-3 исполнимы в зоне engine-dev'а, но оракул на Б-2 — sacred; Б-1 требует
решения о форме (новый CLI-флаг прод-бинаря = интерфейс, не механическая правка). Поэтому
следующий — **architect** (оракул `VB-I-2` под каденцией + строка спеки о композиции
писатель↔читатель), затем dev по SVR-response. Карточки долга (`A-025` §6 + «свойство
стоимости не пиннится» + Б-1/Б-2) заводятся reviewer'ом в close-out ПОСЛЕ merge'а — merge'а
не было, `TECH-DEBT.md` этим кругом не трогаю.

---

## Done Block (сырой stdout; `/tmp/hft-reviewer-m68-rev4`, detached `255c359`)

```
$ pwd; git rev-parse HEAD; git status --porcelain
/tmp/hft-reviewer-m68-rev4
255c359af9d9613f2b3bce03ae04b0c364a68277
{пусто}

$ bash scripts/verify_M-68.sh; echo "verify_exit=$?"
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
PASS: cargo test -p gateway --test red_depth_from_book --quiet
PASS: A состав набора — 9 оракулов (ожидалось ровно 9: d1 d2 d3 d4 d5 d7 d7b d8 d8b)
PASS: B набор КРАСЕН против мутанта C-M68-1 (мутация внесена и прогнана в копии)
PASS: cargo test -p gateway --test red_depth_recompute_cost --quiet
PASS: cargo test -p gateway --test red_depth_semantics --quiet
PASS: C2 состав набора — 3 оракулов (ожидалось ровно 3: d9 d9-C d10)
PASS: cargo test -p gateway-serve --test red_depth_cadence_from_env --quiet
PASS: C3bis состав набора — 4 оракулов (ожидалось ровно 4: доходит, дефолт≡пусто≡отсутствие, отказ на мусоре, объявлена в compose)
PASS: cargo test -p gateway --test red_depth_cadence --quiet
PASS: C3 состав набора — 6 оракулов (ожидалось ровно 6: d12 d13 d14 d15 d16 d17)
PASS: C4 самоописание согласовано (обещаний=0, собственных материализаций=2)
PASS: C4 ложное самоописание снято — снятая snapshot-only семантика поля depth_reach_bid (lib.rs:636-658)
PASS: C4 ложное самоописание снято — то же, вторая половина того же комментария
PASS: C4 ложное самоописание снято — ложное «как прежний depth_within с None mid» (lib.rs:1134-1136)
PASS: D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)
PASS: cargo test -p gateway --test red_gateway_schema_version --quiet
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
PASS: cargo test -p gateway --test red_depth_provenance_by_reach --quiet
PASS: H crates/contracts не тронут
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут
PASS: J selector_fingerprint не переписан
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: PASS
verify_exit=0

$ grep -c '^PASS' /tmp/rev139_verify.log; grep -c '^PASS' /tmp/verify_m68_full.log   # мой лог; лог тестера
27
27

$ # ПРОБА Б-1/Б-2 (временные файлы, прогнаны и УДАЛЕНЫ; дерево чистое — см. git status выше)
$ cargo test -p gateway --test zz_reviewer_probe_r139 -- --nocapture
R139 cadence=None replay_pts=240 live_pts=240 identical=true replay_tail=[(1752000119, 619000000), (1752000118, 618000000)] live_tail=[(1752000119, 619000000), (1752000118, 618000000)]
R139 cadence=Some(1000) replay_pts=240 live_pts=238 identical=false replay_tail=[(1752000119, 619000000), (1752000118, 618000000)] live_tail=[(1752000118, 618000000), (1752000117, 617000000)]
R139 cadence=Some(10000) replay_pts=24 live_pts=22 identical=false replay_tail=[(1752000110, 619000000), (1752000100, 609000000)] live_tail=[(1752000100, 609000000), (1752000090, 599000000)]
R139 cadence=Some(60000) replay_pts=4 live_pts=2 identical=false replay_tail=[(1752000060, 619000000), (1752000000, 559000000)] live_tail=[(1752000000, 559000000)]
R139-CKPT written_cursor=Cursor { upto_seq: Some(119) } files=["ckpt-9e7e403ce976f21e.bin", "zz.lock"]
R139-CKPT fp(writer cadence=None)=9e7e403ce976f21e  fp(reader cadence=Some(1000))=b3a743216e64d3e7
R139-RESUME [A: читатель = писатель (cadence None)] events_decoded=0 events_scanned=0 segments_opened=0
R139-RESUME [B: прод после задачи 22 (cadence 1000)] events_decoded=120 events_scanned=120 segments_opened=1
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p gateway-serve --test zz_reviewer_probe_r139b -- --nocapture
R139-START tf=1000 cadence=10000 | СТАРТ OK, selector.timeframe_ms=1000 depth_cadence_ms=Some(10000) | validate_selector: OK
R139-START tf=3000 cadence=10000 | СТАРТ OK, selector.timeframe_ms=3000 depth_cadence_ms=Some(10000) | validate_selector ОТКАЗ (на клиентском пути): MD-I-8 d17 (R-138 Б-2): selector.depth_cadence_ms=10000 не выравнен на timeframe_ms=3000 …
R139-START tf=3000 cadence=1000 | СТАРТ OK, selector.timeframe_ms=3000 depth_cadence_ms=Some(1000) | validate_selector: OK
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ git log --format='%h %s' b9d05d1..255c359
255c359 feat(M-68): task #22 — GATEWAY_DEPTH_CADENCE_MS env-переменная в serve_config (R-138 Б-3, d18) [engine-dev]
10d6ebe feat(M-68): task #21 — гвард отношения cadence_ms % timeframe_ms в validate_selector (R-138 Б-2, d17) [engine-dev]
a9f89d9 feat(M-68): task #12 — depth_cadence CLOSE через РОЛЛОВЕР, не съём в начале (R-138 Б-1, d12) [engine-dev]
b089c7d test(M-68): d18 — каденция есть КОНФИГ, а не константа (R-138 Б-3); carve-out зоны, задача 22 [architect]
1e3a0ca test(M-68): d17 — выдача не смеет объявлять каденцию, которой не даёт (R-138 Б-2); задача 21 [architect]
52377a1 test(M-68): d12 перестроен по A-025 §5.4 П-1 — close, а не съём в начале интервала (R-138 Б-1) [architect]

$ git log --format='%B' b9d05d1..255c359 | grep -c 'Co-Authored-By'
0

$ for c in a9f89d9 10d6ebe 255c359; do git show --name-only --format="== $c" $c; done | grep -E '^(==|crates|docker)'
== a9f89d9
crates/gateway/src/lib.rs
== 10d6ebe
crates/gateway/src/lib.rs
== 255c359
crates/gateway-serve/src/lib.rs
docker-compose.yml
```

## Cross-references

- `R-138` (Б-1/Б-2/Б-3, N-1 — условия APPROVED этого круга)
- `milestones/M-68-depth-from-book.md` §0sexies.2bis/2ter/2quater/2quinquies, §3 (carve-out), §3.1, §4
- `gates.md` §4 (PR-time, DoD «Механизм на пути», граница reviewer↔architect), §5 (RISK-BLOCK — н/п)
- `testing.md` («Механизм несущего пути обязан иметь оракул точки входа», п.2 КОМПОЗИЦИЯ)
- `docs/fa/viz-backend.md:188-189` (`VB-I-1`, `VB-I-2`), `TD-044`/`TD-097`/`R-029` (цена реплея при подключении)
