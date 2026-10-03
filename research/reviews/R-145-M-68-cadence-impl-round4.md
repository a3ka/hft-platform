<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 2e5585dfc3213409600762f3233a745ce6785f16
audited_head: 32c88aaf738e68c639d39286d5a81100a0ca367c
verdict: REJECT
-->

# R-145 — M-68 круг 4 (задачи 13/23/24 + `d20`): PR-time reviewer, **REJECTED**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-28
**Предмет:** `2e5585d..32c88aa` на `origin/feat/M-68-rev4` — пять коммитов: RED architect'а
(`06f6a02` — `d20` + шаг `C3ter` + переписанный §0sexies.2ter + §2sexies/§2septies + задачи
23/24) и четыре коммита engine-dev'а (`6c9ed41` Б-2, `8e43cfb` задача 23 / Б-1, `01da186`
задача 24 / Б-3, `32c88aa` статус-колонка / N-2).
**Дерево ревью:** `/tmp/hft-reviewer-m68`, detached на `32c88aa`, чекаут из `origin`.
**Мандат:** проверить исполнение условий `R-141` (Б-1 композиция писатель↔читатель, Б-2
`VB-I-2` под каденцией, Б-3 старт-гейт, N-2 статус-колонка).

**Прочитано на этой ревизии:** `milestones/M-68-depth-from-book.md` (§0sexies.2ter/2sexies/
2septies, §3 Allowed paths, §3.1, §4 §Tasks), `R-141` целиком, `docs/fa/viz-backend.md`
(таблица инвариантов), `docs/04-workflow.md` §1/§2, `docs/05-contract-layer.md` §2/§4,
`docs/workflow/reading-map.md` §1/§2, `crates/gateway/src/lib.rs` в тронутых местах
(`finish`/`finish_ref`/`finish_with_at`/`flush_pending_depth_interval`),
`crates/gateway/src/bin/gateway-checkpoint.rs` целиком в тронутой части,
`crates/gateway-serve/src/lib.rs` (`serve_config_from_env`), `docker-compose.yml`
(службы `gateway-serve` и `gateway-checkpoint`), `crates/gateway/tests/red_depth_cadence.rs`
(`d20`), `crates/gateway-serve/tests/red_depth_cadence_from_env.rs` (все 4 оракула),
`scripts/verify_M-68.sh` (все 17 шагов), `deploy/bin/gateway-checkpoint-cron.sh`,
`deploy/cron.d/journal-retention`.
**Ярус C — грепом по предмету, не целиком** (`reading-map.md` §2): `TECH-DEBT.md` по
`M-68|depth_cadence|GATEWAY_DEPTH_CADENCE|TD-044|TD-097` (:74, :75, :100, :101, :106, :132,
:1159-1206, :1209-1254, :1327-1338 — живы `TD-167`, `TD-168`, `TD-173`); `PROJECT-STATE.md`
по `M-68|depth_cadence|GATEWAY_DEPTH_CADENCE` — **совпадений ноль** (милестоун не закрыт).

**Предъявление FA (M-66).** Диф трогает `crates/gateway/src/**` и `crates/gateway-serve/src/**`
⇒ живые инварианты названы прямым чтением `docs/fa/viz-backend.md` на этой ревизии:
**`VB-I-2`** («live == replay: серия, посчитанная на live-хвосте, бит-идентична серии из replay
того же окна журнала») и **`VB-I-1`** («чистый редьюсер над `journal::stream`; нет
wall-clock/rand/I-O в расчёте»). `VB-I-2` — **предмет Б-2 `R-141`; на этой ревизии ВОССТАНОВЛЕН
и запиннен мутацией** (см. ниже). `VB-I-1` held: каденция ведётся временем события.
`FA-WAIVER` не требуется.

---

## Block-scope — ЧИСТО по коду, с оговоркой по тексту спеки (`N-2` ниже)

| коммит | пути | вердикт |
|---|---|---|
| `06f6a02` | `crates/gateway/tests/**`, `milestones/M-68-*.md`, `scripts/verify_M-68.sh` | ✅ зона architect'а |
| `6c9ed41` | `crates/gateway/src/lib.rs` | ✅ штатная зона engine-dev |
| `8e43cfb` | `crates/gateway/src/bin/gateway-checkpoint.rs`, `docker-compose.yml` | ✅ bin — внутри `crates/gateway/src/**`; compose — по существу спеки §0sexies.2sexies, но §3 его для задачи 23 не называет (`N-2`) |
| `01da186` | `crates/gateway-serve/src/lib.rs` | ✅ по существу §0sexies.2septies; §3 carve-out назван только для задачи 22 (`N-2`) |
| `32c88aa` | `milestones/M-68-depth-from-book.md` | ✅ carve-out статус-колонки (`scope-guard.md`) — проверено дифом: изменена ТОЛЬКО колонка Статус |

**Тесты dev не трогал ни в одном из четырёх коммитов** (`git show --name-only` по каждому) —
RED-first по sacred-зоне цел. `crates/contracts/**` не тронут — **Block-C: ЧИСТО** (шаг `H`
гейта; T2-поля `Selector::depth_cadence_ms` / `SeriesBundle::cadence_ms` — собственность крейта
`gateway`, T-designate по `05-contract-layer.md` §2, contract-RFC не требуется).
`crates/book|venue-*|journal` не тронуты (шаг `K`), `GATEWAY_BANDS` не тронут (шаг `I`),
`selector_fingerprint` не переписан (шаг `J`).

**RISK-BLOCK не применяется** — проверено, а не предположено: диапазон не трогает
`crates/risk|killswitch|oms|venue-*|contracts`; предмет — Слой 8, read-only консюмер журнала
(`VB-I-3`), order-egress отсутствует. risk-critic по `gates.md` §5 не требуется.

## Block-commits — ЧИСТО по атомарности, `N-3` по ссылке на задачу

Четыре задачи — четыре коммита, бандла нет.
`git log --format=%B 2e5585d..HEAD | grep -c 'Co-Authored-By'` → **0**.
Ссылка на задачу у `6c9ed41` неверна — `N-3`.

## Block-DoneBlock — ВОСПРОИЗВЕДЁН СВОИМ ПРОГОНОМ, расхождение с отчётом тестера названо

Done Block тестера не пересказан, а перепрогнан в собственном дереве (сырой вывод — в Done
Block ниже). Два расхождения с отчётом тестера — `N-4`.

---

## Б-1 (БЛОКЕР) — задача 24 реализована БЕЗ ОРАКУЛА; гейт не содержит на неё ни одного шага

**Замер, а не чтение.** Мутация: гвард отношения, внесённый `01da186` в
`serve_config_from_env` (`crates/gateway-serve/src/lib.rs:2101-2128`), УДАЛЁН целиком —
то есть предмет задачи 24 нейтрализован:

```
MUT3 gateway-serve: passed=76 failed=0 (блоков: 20)
MUT3 gateway:       passed=157 failed=0 (блоков: 38)
```

**Ни один оракул не покраснел.** Проверено и прямым поиском: `grep -rn` по
`crates/gateway-serve/tests/` не находит ни одного теста, подающего пару
`GATEWAY_TIMEFRAME_MS=3000` + `GATEWAY_DEPTH_CADENCE_MS=10000`; набор `d18`
(`red_depth_cadence_from_env.rs`) состоит ровно из четырёх оракулов —
`env_cadence_reaches_the_selector`, `absent_cadence_yields_signed_default`,
`invalid_cadence_is_rejected_naming_the_variable` (судит `>= 1000` и делимость суток, не
отношение), `knob_is_declared_in_compose`. Шаг `C3bis` гейта требует РОВНО 4 — то есть
состав заморожен на догейтовом наборе.

**Гейт задачу 24 не покрывает вовсе:** `grep -nE '^step ' scripts/verify_M-68.sh` — 17 шагов,
`grep -c 'задача 24' scripts/verify_M-68.sh` → **0**. Это прямое нарушение `gates.md` §3
(«минимум 1 проверка на задачу из §Tasks») и §2 («implementation-код без предшествующего
RED-теста не пишется НИКЕМ»).

**Спека называет оракул, который предмета не касается.** Строка задачи 24 в §Tasks:
«`MD-I-8` `d20` соседний, оракул старт-гейта — по образцу `red_depth_cadence_from_env`».
`d20` живёт в ДРУГОМ крейте (`crates/gateway/tests/red_depth_cadence.rs`) и судит равенство
живой и перепроигранной серий; к `serve_config_from_env` он не обращается ни разу. То есть
таблица задач объявляет покрытие, которого нет — седьмое (восьмое на милестоуне)
срабатывание класса `TD-167`.

**Цена — та же, что назвал `R-141` Б-3, и она вернулась целиком:** пара
`GATEWAY_TIMEFRAME_MS=3000` + `GATEWAY_DEPTH_CADENCE_MS=10000` поднимает ЗДОРОВЫЙ по
healthcheck контейнер, отвергающий каждое подключение (`TD-019`/`TD-020`). Сегодня гвард в
коде есть и работает; ЗАВТРА его снимет любой рефакторинг, и ни гейт, ни набор этого не
заметят. Механизм несущего пути без оракула — ровно то, что `gates.md` §4 (DoD «Механизм на
пути») запрещает мержить.

## Б-2 (БЛОКЕР) — оракул композиции снят по ОПРОВЕРГНУТОЙ причине; условие 1 `R-141` не исполнено

`R-141` условием APPROVED назвал: «композиция обязана быть предъявлена **оракулом точки
входа — не грепом**». Носитель заменён на шаг `C3ter` — лексический инвентарь
(`grep -qE 'depth_cadence_ms:\s*None' crates/gateway/src/bin/gateway-checkpoint.rs`).
Обоснование замены записано в §0sexies.2sexies и в шапке шага дословно:

> композиция «бинарь ↔ служба» наблюдаема только запуском ПРОЦЕССА с argv из docker-compose,
> а **из интеграционного теста Rust она недостижима: `selector_fingerprint` и `ckpt_path_for`
> — `pub(super)`**.

**Это утверждение ОПРОВЕРГНУТО замером.** Композиция наблюдаема ПОВЕДЕНЧЕСКИ, и отпечаток
для этого вычислять не нужно: интеграционный тест крейта `gateway` запускает НАСТОЯЩИЙ
прод-бинарь через `env!("CARGO_BIN_EXE_gateway-checkpoint")` с argv из `docker-compose.yml`,
после чего читателем идёт публичный `gateway::LiveReducer::resume` и предъявляет
`ReadStats::events_decoded`. Проба reviewer'а (временный файл, прогнан и УДАЛЁН; дерево
чистое — см. Done Block), ~110 строк, 3.1 с прогона:

```
R144-A writer exit=0 files=["covered_through_seq", "ckpt-b0f1ed89ec2ec142.bin", "zz.lock"]
R144-A         читатель cadence=Some(1000):  events_decoded=0   events_scanned=0     ← ТЁПЛЫЙ старт
R144-A-контроль читатель cadence=None:       events_decoded=120 events_scanned=120   ← полный реплей
R144-B writer(env=10000, без флага) exit=0
R144-B         читатель cadence=Some(10000): events_decoded=0   events_scanned=0
```

То есть настоящая канарейка точки входа достижима, дешева и была бы ЗЕЛЁНОЙ на этой ревизии.
Долг заведён на ложной посылке: «отдельная работа» — это один файл теста, а не отдельный
милестоун. `testing.md` («Механизм несущего пути обязан иметь оракул точки входа», п.1-2)
требует именно этого — запустить вызывателя и проверить КОМПОЗИЦИЮ producer↔consumer.

**Инвентарь при этом не ловит реалистичную регрессию — тоже замером.** `C3ter` судит ровно
одну лексему у писателя, а объявление ручки в compose проверяет `knob_is_declared_in_compose`
поиском подстроки по ВСЕМУ файлу: снятие обеих строк каденции у службы `gateway-checkpoint`
(`--depth-cadence-ms=…` и блок `environment:`) оставляет оба шага ЗЕЛЁНЫМИ, потому что
переменная по-прежнему объявлена у службы `gateway-serve`. Контрольная мутация дефолта
писателя `1000 → 2000` тоже прошла набор целиком:

```
MUT2 набор milestone: passed=233 failed=0 (блоков: 58)   ← C3ter при этом PASS
```

(эта конкретная мутация прод-путь не ломает — compose передаёт флаг явно; она показывает
ЧУВСТВИТЕЛЬНОСТЬ инвентаря, а не дефект.)

**Что при этом НЕ является дефектом и названо явно: сам фикс Б-1 верен.** Композиция
писатель↔читатель на прод-дефолтах работает — предъявлено выше замером (`events_decoded=0`
против 120 у контроля). Блокер здесь — отсутствие ОРАКУЛА при опровергнутой причине его
отсутствия, а не поведение кода.

---

## Что проверено и дефекта НЕ найдено — названо явно

- **`R-141` Б-2 (`VB-I-2` под каденцией) ЗАКРЫТ и ЗАПИННЕН — мутационный контроль исполнен.**
  Возврат состояния круга 3 (сброс незакрытого интервала возвращён в `finish(self)`,
  `flush_pending_depth_interval` восстановлен) роняет `d20` с точными числами:

  ```
  test md_i8_d20_live_equals_replay_under_cadence ... FAILED
  assertion `left == right` failed: … при каденции 1000 мс живая депт-серия НЕ РАВНА
  перепроигранной — точек 238 против 240 …
  test result: FAILED. 6 passed; 1 failed
  ```

  Оракул несёт setup-guard (`n_replay == 0` ⇒ `setup_failed`), гоняет три каденции
  (1000/10000/60000) и ассертит РАВЕНСТВО, а не «не хуже». Выбранная семантика («не эмитить
  незакрытый интервал ни на одном пути») разрешена переписанным §0sexies.2ter явно и не
  ломает `M-56`/`TD-097` (`&self` на живом пути сохранён).
- **`R-141` Б-1 закрыт ПО ПОВЕДЕНИЮ** — замер выше: прод-писатель (настоящий бинарь, argv из
  compose) и прод-читатель находят один слепок, `events_decoded=0`.
- **`R-141` Б-3 закрыт ПО КОДУ** — гвард отношения зеркалится на старте, сообщение называет
  обе переменные и остаток. Дефект — отсутствие оракула (Б-1 выше), не поведение.
- **`R-141` N-2 исполнен** — статус-колонка приведена к правде: задачи 1-10, 12-24 несут
  фактический статус, задача 20 помечена `⏸ ОТЛОЖЕНА (долг №2)`. Диф `32c88aa` содержит
  ТОЛЬКО колонку Статус.
- **`R-141` N-4 исполнен** — шаг `I` теперь судит только изменённые строки
  (`--unified=0` + фильтр `^[+-][^+-]`).
- **Соседние инварианты не куплены:** `red_gateway_bounded`, `red_snapshot_noclone`
  (`VB-I-10`), `red_depth_provenance_by_reach`, `red_gateway_schema_version`,
  `red_depth_from_book` (9/9), `red_gateway_live_eq_replay`, мутант `C-M68-1` (шаг `B`) —
  зелены в моём прогоне.
- **Фикс Б-2 не трогает legacy-путь `depth_cadence_ms: None`** — `flush` и раньше был no-op
  при отсутствии каденции; шаг `F` (`VB-I-2` без каденции) зелен.

---

## N-1 (MAJOR NOTE) — новый код лжёт о себе: «fail-closed» против замера exit=0

`crates/gateway/src/bin/gateway-checkpoint.rs:288` — комментарий
`// Невалидное env обрабатывается ниже как fail-closed`, а конструкция
`trimmed.parse::<i64>().ok()` глотает ошибку разбора и отдаёт `None`, после чего
`args.depth_cadence_ms.or(cadence_from_env)` подставляет ДЕФОЛТ. Замер:

```
R144-C writer(env="не-число") exit=0  ← принято молча, взято 1000
R144-D writer(env=999)        exit=2  err="…GATEWAY_DEPTH_CADENCE_MS=999 невалидно…"
```

То есть fail-closed работает для «невалидного ЧИСЛА» и не работает для «не-числа».
Соседняя служба на ту же переменную ведёт себя ИНАЧЕ: `d18`
(`invalid_cadence_is_rejected_naming_the_variable`) требует от `serve_config_from_env` отказа
на `"abc"`, `"1000.0"`, `"1_000"` — и получает его. Итог: одна переменная, два разных контракта
на мусор. Прод от этого не деградирует тихо (читатель откажется стартовать, и деплой
покраснеет), поэтому не блокер — но класс ровно тот, ради которого на этом милестоуне
существуют задачи 12/19 и шаг `C4`: **самоописание кода расходится с кодом**.

## N-2 (NOTE) — §3 Allowed paths не расширены на задачи 23/24

§3 читается дословно: «`crates/gateway/src/**` **+ (задача 22, carve-out ниже)**
`crates/gateway-serve/src/lib.rs` и `docker-compose.yml`». Задачи 23 и 24 добавлены коммитом
`06f6a02` в §Tasks и подробно обоснованы в §0sexies.2sexies/2septies (правка compose и
`gateway-serve` там предписана ПРЯМО), но строка зоны осталась привязанной к задаче 22.
Scope-нарушения по существу нет — спека требует именно этих правок; но dev, читающий §3
буквально, обязан был бы остановиться `SCOPE VIOLATION REQUEST`. Тот же класс `TD-167`.

## N-3 (NOTE) — коммит `6c9ed41` ссылается на задачу, которой не соответствует

Subject: `feat(M-68): task #13 — VB-I-2 live==replay при каденции`. Задача 13 в §Tasks — это
«Односторонняя книга: точка в серию НЕ пишется», оракул `d9`, и она была закрыта раньше
(статус `✅ DONE` проставлен тем же кругом). У предмета Б-2 строки задачи в таблице НЕТ вовсе:
architect завёл задачи 23 и 24, но не завёл задачу на Б-2. `commit-discipline.md` требует
ссылки на задачу milestone'а — ссылка есть, но неверная.

## N-4 (NOTE) — отчёт тестера неверно называет предмет прогона

«артефакт N-2 … подтверждён как **единственная дельта поверх `06f6a02`**» — неверно:
`git log --oneline 06f6a02..32c88aa` даёт **три** impl-коммита (`6c9ed41`, `8e43cfb`,
`01da186`) плюс `32c88aa`, то есть ровно ту работу, которую тестер и прогонял. Прогон сделан
на правильном SHA — на вердикт не влияет, но предмет назван неверно.

**Число PASS у тестера СОШЛОСЬ** и названо здесь явно, потому что `R-141` N-3 фиксировал
обратное: заявленные «28 строк PASS» подтверждены моим прогоном (`grep -c '^PASS'` → **28**;
27 было до появления шага `C3ter`). Повтора замечания нет.

## N-5 (NOTE) — мёртвая функция оставлена «на всякий случай»

`flush_pending_depth_interval` сведена к `let _ = self;` под `#[allow(dead_code)]` с
комментарием «если внешний код позже решит диагностировать открытый интервал». Ни одного
вызывателя нет; это не диагностика, а имя, переживающее свой предмет. Удаление дешевле
объяснения — но зона `crates/gateway/src/**`, решение за architect'ом/dev'ом, не за
reviewer'ом.

## N-6 (NOTE) — статус задачи 11 устарел

`🚧 rev4: расширяется на задачи 12-14` — фактически шаг гейта расширен и на задачу 23
(`C3ter`), а на задачу 24 не расширен вовсе (Б-1 выше).

---

## ВЕРДИКТ: **REJECTED**

Два блокера. Оба — про ОРАКУЛЫ, а не про поведение: сам код круга 4 по всем трём блокерам
`R-141` работает, и это предъявлено замером выше.

1. **Б-1** — задача 24 реализована без единого оракула и без единого шага гейта; мутация
   (удаление гварда) оставляет `gateway-serve` 76/0 и `gateway` 157/0 зелёными. Нарушены
   `gates.md` §2 (RED-first) и §3 (≥1 проверка на задачу). Спека при этом объявляет
   покрытие (`d20`), которого нет.
2. **Б-2** — условие 1 `R-141` («оракул точки входа, не греп») не исполнено, а причина
   неисполнения — «из интеграционного теста Rust недостижимо» — опровергнута замером:
   запуск прод-бинаря через `CARGO_BIN_EXE_gateway-checkpoint` + `LiveReducer::resume`
   наблюдает композицию поведенчески и `pub(super)` не требует.

**Условие APPROVED:**

1. Задача 24 получает RED-оракул (пара `timeframe/cadence`, отвергаемая на СТАРТЕ) и шаг в
   `scripts/verify_M-68.sh`; строка задачи 24 в §Tasks называет ЭТОТ оракул, а не `d20`.
   Оракул sacred ⇒ architect; шаг гейта ⇒ architect.
2. Композиция прод-писатель↔прод-читатель предъявляется оракулом ТОЧКИ ВХОДА (запуск
   бинаря + наблюдение `events_decoded`), а §0sexies.2sexies и шапка шага `C3ter`
   приводятся к правде: утверждение о недостижимости снимается. **Форму решает architect** —
   reviewer предъявляет только достижимость (`gates.md` §4, граница reviewer↔architect).
   Если architect настаивает, что остаточная часть композиции (compose → процесс) всё же
   не механизируема, — это записывается явным `COGNITIVE-ONLY` с названным пределом, а не
   утверждением о невозможности целого.
3. `N-1` — комментарий приводится к правде ЛИБО поведение приводится к комментарию
   (симметрия с `serve_config_from_env` предпочтительна, но это решение architect'а).
4. `N-2`/`N-3` — §3 Allowed paths называет задачи 23/24; строка задачи для предмета Б-2
   заводится (иначе работа круга 4 не отражена в §Tasks вовсе).

**Маршрут.** Оракулы и гейт — зона architect'а (sacred), правка комментария — engine-dev по
SVR-response. Поэтому следующий — **architect**. Карточки долга по `A-025` §6, `TD-168`,
`TD-173` и по классу «инвентарь вместо канарейки» заводятся reviewer'ом в close-out ПОСЛЕ
merge'а — merge'а не было, `TECH-DEBT.md`/`PROJECT-STATE.md` этим кругом не трогаю.

---

## Done Block (сырой stdout; `/tmp/hft-reviewer-m68`, detached `32c88aa`)

```
$ pwd; git rev-parse HEAD; git status --porcelain
/tmp/hft-reviewer-m68
32c88aaf738e68c639d39286d5a81100a0ca367c
{пусто}

$ git log --format='%h %s' 2e5585d..HEAD
32c88aa docs(M-68): N-2 — статус-колонка §Tasks приведена к правде [engine-dev]
01da186 feat(M-68): task #24 — старт-гейт зеркалит гвард отношения, Б-3 R-141 [engine-dev]
8e43cfb feat(M-68): task #23 — gateway-checkpoint читает GATEWAY_DEPTH_CADENCE_MS, Б-1 R-141 [engine-dev]
6c9ed41 feat(M-68): task #13 — VB-I-2 live==replay при каденции, Б-2 R-141 [engine-dev]
06f6a02 test(M-68): d20 + инвентарь C3ter по R-141 — оба блокера были следствием МОЕЙ спеки [architect]

$ git log --format='%B' 2e5585d..HEAD | grep -c 'Co-Authored-By'
0

$ for c in 06f6a02 6c9ed41 8e43cfb 01da186 32c88aa; do git show --name-only --format="== $c" $c; done
== 06f6a02
crates/gateway/tests/red_depth_cadence.rs
milestones/M-68-depth-from-book.md
scripts/verify_M-68.sh
== 6c9ed41
crates/gateway/src/lib.rs
== 8e43cfb
crates/gateway/src/bin/gateway-checkpoint.rs
docker-compose.yml
== 01da186
crates/gateway-serve/src/lib.rs
== 32c88aa
milestones/M-68-depth-from-book.md

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
PASS: C3ter прод-писатель чекпоинта не вшивает каденцию (инвентарь, не оракул — см. комментарий)
PASS: cargo test -p gateway-serve --test red_depth_cadence_from_env --quiet
PASS: C3bis состав набора — 4 оракулов (ожидалось ровно 4: доходит, дефолт≡пусто≡отсутствие, отказ на мусоре, объявлена в compose)
PASS: cargo test -p gateway --test red_depth_cadence --quiet
PASS: C3 состав набора — 7 оракулов (ожидалось ровно 7: d12 d13 d14 d15 d16 d17 d20)
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
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут (судятся только изменённые строки)
PASS: J selector_fingerprint не переписан
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: PASS
verify_exit=0

$ grep -E "^test result" <лог того же прогона> | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=950 failed=0 (блоков: 223)

$ grep -c '^PASS' <лог того же прогона>
28

$ # ПРОБА КОМПОЗИЦИИ (Б-2): запуск ПРОД-БИНАРЯ + чтение прод-селектором.
$ #   временный файл crates/gateway/tests/zz_reviewer_probe_r144.rs — прогнан и УДАЛЁН
$ cargo test -p gateway --test zz_reviewer_probe_r144 -- --nocapture
R144-A writer exit=0 err="gateway-checkpoint: ok … achieved_cursor=Cursor { upto_seq: Some(119) } covered=119 …" files=["covered_through_seq", "ckpt-b0f1ed89ec2ec142.bin", "zz.lock"]
R144-A читатель cadence=Some(1000): events_decoded=0 events_scanned=0  (ТЁПЛЫЙ старт ⇔ 0)
R144-A-контроль читатель cadence=None: events_decoded=120 events_scanned=120 (ожидается полный реплей)
R144-B writer(env=10000, без флага) exit=0
R144-B читатель cadence=Some(10000): events_decoded=0 events_scanned=0
R144-C writer(env=МУСОР) exit=0
R144-C читатель cadence=Some(1000) после мусорного env у писателя: events_decoded=0 (0 ⇒ писатель молча взял 1000)
R144-D writer(env=999) exit=2 err="gateway-checkpoint: GATEWAY_DEPTH_CADENCE_MS=999 невалидно: требуется >= 1000 и выравнено на границу UTC-суток …"
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.11s

$ # МУТАЦИЯ 1 (анти-плацебо d20): состояние круга 3 возвращено в /tmp/hft-mut-m68
$ cargo test -p gateway --test red_depth_cadence
test md_i8_d20_live_equals_replay_under_cadence ... FAILED
assertion `left == right` failed: MD-I-8 d20 / VB-I-2 (R-141 Б-2): при каденции 1000 мс живая депт-серия НЕ РАВНА перепроигранной — точек 238 против 240. …
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

$ # МУТАЦИЯ 2 (чувствительность C3ter): дефолт каденции ПИСАТЕЛЯ 1000 -> 2000
$ if grep -qE 'depth_cadence_ms:\s*None' crates/gateway/src/bin/gateway-checkpoint.rs; then echo "C3ter: FAIL"; else echo "C3ter: PASS (мутацию НЕ ловит)"; fi
C3ter: PASS (мутацию НЕ ловит)
$ cargo test -p gateway -p gateway-serve | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=233 failed=0 (блоков: 58)

$ # МУТАЦИЯ 3 (Б-1 БЛОКЕР): гвард задачи 24 удалён из serve_config_from_env
MUT3 gateway-serve: passed=76 failed=0 (блоков: 20)
MUT3 gateway: passed=157 failed=0 (блоков: 38)

$ grep -nE '^step ' scripts/verify_M-68.sh | wc -l; grep -c 'задача 24\|задачи 24' scripts/verify_M-68.sh
17
0

$ grep -rn "GATEWAY_TIMEFRAME_MS=3000\|не выравнен на GATEWAY_TIMEFRAME_MS" crates/gateway-serve/tests/ | wc -l
0
```

## Cross-references

- `R-141` (условия APPROVED этого круга: Б-1 композиция, Б-2 `VB-I-2`, Б-3 старт-гейт, N-2)
- `milestones/M-68-depth-from-book.md` §0sexies.2ter (переписан), §2sexies, §2septies, §3, §4
- `gates.md` §2 (RED-first), §3 (≥1 проверка на задачу, паритет с CI), §4 (PR-time, DoD
  «Механизм на пути», граница reviewer↔architect), §5 (RISK-BLOCK — н/п)
- `testing.md` («Механизм несущего пути обязан иметь оракул точки входа», п.1-2 КОМПОЗИЦИЯ;
  «Мутационный контроль — обязателен»)
- `docs/fa/viz-backend.md` (`VB-I-1`, `VB-I-2`, `VB-I-3`, `VB-I-10`)
- `TD-167` (самосогласованность артефактов), `TD-168`, `TD-173`, `TD-044`/`TD-097` (цена реплея)
