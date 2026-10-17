<!-- GATE-META
milestone: M-91
audited_repo: a3ka/hft-platform
audited_base: 395323c5f6a403f9b6c17306cc7f9f98f521e155
audited_head: 2a506491c86d8efb0d0ff713bdffdc2c6af0fbac
verdict: APPROVE
-->

# R-216 — PR-гейт `M-91`: legacy-путь выдачи считает клиентов как v1-путь (`TD-228`)

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL: диапазон трогает
`crates/gateway-serve/src/**`).
**Предмет:** `feat/M-91-legacy-serving-counters` → `main`; `TD-228` (MAJOR), остаток
`ROADMAP` п. 9-A-bis.
**База аудита:** `395323c` (merge-base с `origin/main`) · **вершина:** `2a50649`.
**Предшествующие гейты в цепочке:** `C-269` REJECT (круг 1) → `C-271` NOTE (круг 2, набор
принят) → engine-dev `2a50649` → tester PASS → этот вердикт.
**Вердикт: APPROVE** — четыре находки класса NOTE, ни одна не блокирует merge; две уходят
карточками долга, две закрываются записью здесь.

---

## §0 — ШАГ 0: что именно судится

```
$ git rev-parse origin/feat/M-91-legacy-serving-counters
2a506491c86d8efb0d0ff713bdffdc2c6af0fbac

$ git merge-base origin/main origin/feat/M-91-legacy-serving-counters
395323c5f6a403f9b6c17306cc7f9f98f521e155

$ git log --oneline origin/main..origin/feat/M-91-legacy-serving-counters
2a50649 feat(M-91): task #1 — legacy path counts attempts/refusals/successes symmetric to v1 [engine-dev]
dcd71ed docs(M-91): C-271 NOTE — набор принят, приложение вердикта в журнал кругов [architect]
063de95 docs(M-91): C-271 — note legacy Warming oracle [critic]
243ac7b docs(M-91): спека круг 2 — I-3bis, l4, запрет крючка вне тестовой сборки, предел «Warming недостижим» (C-269) [architect]
56f4d8d test(M-91): verify — шаг l4 с --features testing (CI фичу не включает) (C-269) [architect]
2f4698d test(M-91): l4 — Warming в legacy-пути — refusals_supported (1,0,1,0); крючок подмены исхода только под feature testing (C-269) [architect]
ca07f0d docs(M-91): C-269 — reject legacy counters plan [critic]
89ac1ae test(M-91): red_verify_M-91_ci_map.sh — проба карты CI-паритета гейта M-91, 10 миров [architect]
bcefefc test(M-91): verify_M-91.sh — оракулы, соседи счётчиков, паритет с CI (временная копия карты M-90 до TD-222) [architect]
6c7b19b test(M-91): l1-l3 — равенства дельт счётчиков экземпляра на legacy-пути (RED) [architect]
70d14cc docs(M-91): спека — legacy-путь выдачи считает клиентов как v1-путь; класс отказа — refusals_supported (TD-228) [architect]

$ git diff --numstat 395323c 2a50649
106	2	crates/gateway-serve/src/lib.rs
344	0	crates/gateway-serve/tests/red_m91_legacy_counters.rs
121	0	milestones/M-91-legacy-serving-counters.md
152	0	research/critiques/C-269-m91-legacy-serving-counters.md
115	0	research/critiques/C-271-m91-legacy-serving-counters-round2.md
59	0	scripts/tests/red_verify_M-91_ci_map.sh
149	0	scripts/verify_M-91.sh
```

Прогон — в СВОЁМ дереве `/tmp/hft-reviewer-m91` (`branch-hygiene.md` §1), не в общем
чекауте; все числа §9 сняты там, а не пересказаны из отчёта tester'а.

---

## §1 — Block-scope: PASS

| коммит | роль | пути | зона по `milestones/M-91-*.md` §8 |
|---|---|---|---|
| `2a50649` | engine-dev | `crates/gateway-serve/src/lib.rs` (один файл) | **allowed** («`run_authorized_session`; крючок под `#[cfg(feature = "testing")]`») |
| `6c7b19b`, `2f4698d`, `56f4d8d`, `89ac1ae`, `bcefefc` | architect | `crates/gateway-serve/tests/red_m91_*.rs`, `scripts/verify_M-91.sh`, `scripts/tests/red_verify_M-91_ci_map.sh` | **allowed** (sacred-зона architect'а) |
| `70d14cc`, `243ac7b`, `dcd71ed` | architect | `milestones/M-91-legacy-serving-counters.md` | **allowed** |
| `ca07f0d`, `063de95` | critic | `research/critiques/C-269-*.md`, `C-271-*.md` | **allowed** (`scope-guard.md`: critic — только вердикт-файлы) |

Диапазон НЕ трогает: `crates/contracts/**`, `crates/risk/**`, `crates/killswitch/**`,
`crates/oms/**`, `crates/venue-*/**`, `research/registry/**`, `.claude/**`, `docker-compose.yml`,
`deploy/**`, `Cargo.toml` (ни корневой, ни крейтовый). Проверено `git diff --name-only`
(полный список — §0). **Превышения зоны нет; dev не тронул ни одного sacred-пути.**

---

## §2 — Block-DoneBlock: PASS

Отчёт tester'а несёт сырой stdout с exit-кодами (8 из 8 команд мандата), а не пересказ.
Reviewer **перепроверил независимо** на своём чекауте — числа §9 ниже совпадают с отчётом
tester'а по всем позициям, кроме одной, где прав tester: соседние оракулы `m89` дают
`2+2+1=5`, а не `1+2+1=4` из «Ожидания» dev'а. Расхождение — в счётчике чужого ожидания,
не в вердикте: все блоки GREEN.

---

## §3 — Block-C (contract-RFC): НЕ ПРИМЕНИМ

```
$ git diff --name-only 395323c 2a50649 | grep -c '^crates/contracts/'
0
```

`crates/contracts/**` не тронут ⇒ правило `05-contract-layer.md` §4 (правка T1 только внутри
atomic contract-RFC) не срабатывает. Схема сердцебиения (`schema: 1`) и wire выдачи не
меняются — это и есть запрет §5 спеки, исполненный: новой величины нет, `schema` не бампнут,
формат кадров не затронут (см. §7, `GS-I-4`).

---

## §4 — Block-risk (RISK-BLOCK `gates.md` §5): НЕ ПРИМЕНИМ — предъявляю замером

```
$ git diff --name-only 395323c 2a50649 | grep -cE '^crates/(risk|killswitch|oms|venue-[^/]+)/'
0
```

Ни один путь RISK-BLOCK не тронут, контракты тоже (§3) ⇒ `risk-critic` по §5 не требуется, и
его отсутствие в цепочке блокером не является. Отдельно проверено, что это не MD-only
carve-out «по форме, а не по сути»: диапазон не вводит ни одного order-egress-вызова —
единственные новые исполняемые строки прод-пути суть три инкремента счётчиков
(`lib.rs:2087`, `:2149`, `:2380`) и один `match` над `Option<fn() -> ServingOutcome>`,
который в прод-сборке всегда `None`. `gateway-serve` остаётся read-only консюмером журнала.

---

## §5 — RED-first и неприкосновенность оракулов: PASS

```
$ git log --diff-filter=A --pretty='%h %ad %s' --date=short -- crates/gateway-serve/tests/red_m91_legacy_counters.rs
6c7b19b 2026-10-01 test(M-91): l1-l3 — равенства дельт счётчиков экземпляра на legacy-пути (RED) [architect]

$ git show --numstat --format='' 2a50649
106	2	crates/gateway-serve/src/lib.rs
```

Оракул введён architect'ом (`6c7b19b`) и расширен им же (`2f4698d` — `l4`) **до**
реализации; коммит dev'а (`2a50649`) трогает РОВНО один файл и ни одного файла `*/tests/**`.
Переписывания теста под реализацию нет — ни одного.

**Качество набора проверено по существу, а не по факту зелени:**
меры — РАВЕНСТВА дельт `(attempts, successes, refusals_supported, refusals_unsupported)`,
а не `> 0` (ассерт «выросло» проходит и при двойном счёте); каждый сценарий несёт
setup-guard, падающий отдельным сообщением «SETUP НЕ СОСТОЯЛСЯ» (`red_m91_legacy_counters.rs:136-141` (фикстура),
`:186-202` (ответ сервера), `:233`/`:259`/`:333` (ожидаемый исход каждого сценария)); `l3` покрывает множественность (две сессии ⇒ ровно `(2,2,0,0)`),
`l4` — исход `Warming`, недостижимый через `admission::readiness`, подстановкой под
`feature = "testing"`.

**Запрет §5 спеки «крючок подмены исхода готовности вне `#[cfg(feature = "testing")]`»
исполнен КОМПИЛЯТОРОМ, а не обещанием:**

```
$ grep -n 'testing' crates/gateway-serve/Cargo.toml
25:testing = []                     # НЕ в [features].default — дефолтного набора вообще нет

$ git diff --name-only 395323c 2a50649 -- crates/gateway-serve/Cargo.toml
                                   # пусто: фичевая таблица диапазоном не менялась

$ grep -n 'cargo build' Dockerfile
18:RUN cargo build --release --bin recorder --bin journal-retention --bin gateway-serve --bin gateway-checkpoint --bin wsprobe
                                   # прод-образ собирается БЕЗ --features
```

`Server::with_readiness_override` объявлен под `#[cfg(feature = "testing")]`
(`lib.rs:503-512`), а в `serve()`-цикле прод-сборка связывает
`readiness_override: Option<fn() -> ServingOutcome> = None` литералом
(`lib.rs:575-580`). Способа подменить проверку готовности в прод-бинаре не существует —
обхода предохранителя `M-87` правка не вносит.

**Порядок счёта проверен построчно** (это то, что спека §7 п. 1 честно объявила
не покрытым оракулом): `inc_attempts` — первая строка `run_authorized_session`
(`:2087`), ДО проверки готовности; `inc_refusals_supported` — внутри ветки
`!matches!(ready_outcome, Ready)` и ДО цикла ответов (`:2149`), то есть один раз на
сессию; `inc_successes` — ПОСЛЕ `sink.send(Message::Text(snap_text)).await?` (`:2380`,
отправка на `:2369-2371`), а не до неё.

---

## §6 — Атомарность коммитов: PASS

Одна задача §Tasks (№1) — один коммит `2a50649` с ссылкой на milestone и меткой роли в
subject'е. Бандла на несколько задач нет. Conventional-формат соблюдён во всех 11 коммитах
диапазона.

```
$ git log --format='%H%n%B---' 395323c..2a50649 | grep -ci 'co-authored'
0
```

Трейлеров `Co-Authored-By` нет (`commit-discipline.md`).

---

## §7 — Предъявление FA (`gates.md` §4, M-66): живые инварианты названы

Диапазон трогает `crates/gateway-serve/**` ⇒ вердикт обязан назвать ЖИВОЙ инвариант FA
тронутого модуля. Для `gateway-serve` барьер `scripts/check_review_fa.sh:195-199` берёт
`docs/fa/viz-backend.md` с префиксом `GS`:

```
$ grep -hoE '\bGS-I-[0-9]+\b' docs/fa/viz-backend.md | sort -u
GS-I-1
GS-I-2
GS-I-4
```

Что я проверил по этим трём инвариантам на предмете правки:

- **`GS-I-4`** (wire-roundtrip + версия: `ServeMsg::{Snapshot,Frame}` сериализуются целиком и
  несут `schema_version`) — **не затронут и это ПРОВЕРЕНО, а не предположено**: диапазон не
  добавляет и не удаляет ни одного поля `Snapshot`/`Frame`, не меняет `GATEWAY_SCHEMA_VERSION`
  и не правит формат ошибки (`{"type":"error","v":1,"code":…}` сохранён буквально,
  `lib.rs:2163-2168`). Инкременты счётчиков в провод не попадают вовсе — они живут в
  `ServingCountersHandle` экземпляра и наружу уходят только сердцебиением (`schema: 1`,
  не изменена).
- **`GS-I-1`** (плоскости разделены: `crates/gateway-serve/**` не импортирует
  `postgres`/`sqlx`/`diesel`/`mysql`) — **сохранён**: правка не вводит ни одной новой
  зависимости (`crates/gateway-serve/Cargo.toml` диапазоном не тронут, §5), счётчики —
  процессные атомики ручки экземпляра, а не внешнее хранилище.
- **`GS-I-2`** (JWT-verify stateless, сигнатура `(token, key)` без БД-параметра) —
  **не затронут**: путь аутентификации правка не касается; `inc_attempts` стоит ПОСЛЕ
  авторизации, на входе уже авторизованной сессии (`run_authorized_session`), то есть
  попытка считается для допущенного клиента, а не для любого стука в порт.

**`GS-I-3` (read-only) я называю отдельно и с оговоркой:** инвариант процитирован в коде
(`lib.rs:515` — «Read-only (GS-I-3)») и живёт в `milestones/M-28-gateway-serve.md`, но в
`docs/fa/viz-backend.md` НЕ объявлен — там же, в §5 этого FA, дрейф назван явно (строки
361-375: «`GS-I-*` в §5 ЭТОГО документа НЕ объявлены»). Поэтому ссылкой на `GS-I-3` барьер
удовлетворить нельзя, и я на него не опираюсь: по существу правка read-only-характер не
нарушает (новых импортов journal-writer нет, журнал не мутируется — см. §4).

**Waiver НЕ требуется и НЕ заявляется.** Это отдельная находка — см. `N-3` ниже.

---

## §8 — Находки

Все четыре — класса NOTE: **ни одна не меняет поведение прод-кода и ни одна не блокирует
merge.** Две уходят карточками долга, одна дописывается рецидивом в существующую карточку,
одна закрывается записью здесь. Фиксов НЕ проектирую (`gates.md` §4: reviewer описывает
дефект, architect проектирует защиту).

### `N-1` — комментарии, введённые ЭТИМ ЖЕ коммитом, врут о коде. Зона — engine-dev. → карточка долга

Четыре утверждения в новых комментариях `crates/gateway-serve/src/lib.rs` неверны:

**(a) `:2374-2375` — ложная история.**
> «До этой строки счётчик рос ДО фактической отдачи снимка: отключившийся клиент всё равно
> считался «успехом»»

До этого коммита на legacy-пути счётчика успеха НЕ БЫЛО ВОВСЕ — это и есть предмет `TD-228`
(замер: три успешные подписки, `successes:0`). Комментарий описывает историю, которой не было,
и читателю внушает, что правка переставила инкремент, тогда как она его ЗАВЕЛА.

**(b) `:2377-2378` — переобещание доставки.**
> «`send` вернул `Ok` ⇒ клиент снимок получил»

`SinkExt::send` доводит кадр до флуша в сокет ОС; подтверждения ПРИЛОЖЕНИЯ-клиента он не
несёт. Признак доставки у нас — `frames_received`/`latency_first_snapshot_ms` на стороне
`wsprobe`, а не `Ok` от `send`. Честная формулировка — «снимок отдан транспорту», и ровно её
даёт спека §7 п. 1, которую комментарий пересиливает в сторону более сильного утверждения.

**(c) `:2079-2086`, `:2144-2148`, `:2372-2379` — ссылки на строки v1-пути устарели в момент
написания.** Цитируются `:1105`, `:1121/1161/1196/1261`, `:1410`, `:1558`; факт на вершине
`2a50649`:

```
$ grep -n 'metrics::inc_attempts\|metrics::inc_successes\|metrics::inc_refusals' crates/gateway-serve/src/lib.rs
1172:                    metrics::inc_attempts(inner.counters.as_ref());
1178:                            metrics::inc_refusals_unsupported(inner.counters.as_ref());
1188:                            metrics::inc_refusals_supported(inner.counters.as_ref());
1228:                                metrics::inc_refusals_supported(inner.counters.as_ref());
1263:                            metrics::inc_refusals_supported(inner.counters.as_ref());
1293:                    metrics::inc_attempts(inner.counters.as_ref());
1328:                            metrics::inc_refusals_supported(inner.counters.as_ref());
1477:                    metrics::inc_successes(inner.counters.as_ref());
1625:                metrics::inc_successes(inner.counters.as_ref());
2087:        metrics::inc_attempts(counters.as_ref());        ← legacy, новая строка
2149:            metrics::inc_refusals_supported(counters.as_ref());   ← legacy, новая строка
2380:        metrics::inc_successes(counters.as_ref());       ← legacy, новая строка

$ sed -n '1105p;1410p;1558p' crates/gateway-serve/src/lib.rs
                         // что и требовалось от многобайтового входа; `cha
                                 snap,
                     );
```

Сдвиг +67 строк внесён ЭТИМ ЖЕ диффом (+106/−2). То есть комментарий ссылался на нумерацию
дерева ДО собственной правки — самоинвалидирующаяся ссылка. Отдельно: v1 несёт ДВА
`inc_attempts` (`:1172`, `:1293`), комментарий называет один.

**(d) `:2084` — правило тишины названо без Δ и с мусором в тексте.**
> «правило тишины `OPS-I-8` (built on DEFENSE `attempts > 0 ∧ successes == 0 ∧ refusals_supported > 0`)»

`check_serving_silence` (`M-89`) строится на ДЕЛЬТАХ (`Δattempts > 0 ∧ Δrefusals_supported > 0
∧ Δsuccesses == 0`), а не на абсолютных значениях: на монотонных счётчиках `successes == 0`
перестаёт быть верным навсегда после первого успеха. Плюс «built on DEFENSE» — обрывок чужого
языка в прод-комментарии.

**Почему это вообще находка, а не придирка.** `crates/gateway-serve` своей FA не имеет (§7,
долг `M-87` §17), поэтому комментарий в прод-файле — единственная документация этих
инвариантов на месте. Ложный комментарий хуже отсутствующего: отсутствие отправляет читателя
к спеке, ложь — останавливает его на себе. Severity **MINOR** (на данные и прод не влияет).

### `N-2` — `OPS-I-8` в FA говорит НЕ ТО, на что ссылаются и спека, и код. Зона — architect. → карточка долга

```
$ grep -n 'OPS-I-8' docs/fa/ops.md
476:| OPS-I-8 | Тишина в потоке (`md_event_age_ms > порог`) — алерт P1: «жив, но не работает» (TD-011/TD-014-класс) |

$ grep -c 'check_serving_silence' docs/fa/ops.md
0
```

`OPS-I-8` объявлен про тишину ПОТОКА РЫНОЧНЫХ ДАННЫХ (`md_event_age_ms`, продюсер —
`recorder`). Правило тишины ВЫДАЧИ — `check_serving_silence` на дельтах счётчиков
`gateway-serve` — в `docs/fa/ops.md` не объявлено НИ ОДНИМ ID, хотя построено `M-89` и живёт
в `crates/ops/src/watchdog.rs`. При этом на `OPS-I-8` опираются оба носителя предмета:
`milestones/M-91-*.md` §11 («опора — `docs/fa/ops.md` `OPS-I-8`») и комментарии прод-кода
(`lib.rs:2084`, `:2375`). Обе ссылки указывают на инвариант, который их утверждения не несёт.

Класс — тот же обратный дрейф, что FA уже называет у `GW-I-*`/`GS-I-*` (§7): механизм есть,
объявления нет. Чем опасно именно здесь: инвариант, не объявленный в FA, нельзя ослабить
наблюдаемо — следующая правка `watchdog` не встретит ни одного документа, который бы ей
возразил. Severity **MINOR** (инструмент наблюдаемости, не данные).

### `N-3` — FA-WAIVER tester'а на `gateway-serve` ошибочен; это ВТОРОЕ срабатывание класса `TD-165`. Закрывается записью

Отчёт tester'а несёт:
> `FA-WAIVER: crates/gateway-serve — docs/fa/gateway-serve.md отсутствует на ревизии 2a50649`

Факт:

```
$ sed -n '117p;195,199p' scripts/check_review_fa.sh
    recorder|derive)                      ← NO-FA список ЗАКРЫТ и состоит из двух имён
    gateway-serve)
      FA_OF["${c}"]="docs/fa/viz-backend.md"
      PFX_OF["${c}"]="GS"
      LIVE_CRA+=("${c}")
      ;;
```

`gateway-serve` — ЖИВОЙ FA-крейт с префиксом `GS` и FA-файлом `docs/fa/viz-backend.md`.
Waiver для живого крейта шаг 5 барьера не потребляет вовсе (предикат per-NO-FA-crate), а шаг 6
требует ЭХО живого `GS-I-*` — которого в отчёте tester'а нет. Вред не наступил по механической
причине: в множество `S` входят ТОЛЬКО файлы `research/reviews/R-*.md`, то есть этот вердикт, а
не отчёт tester'а; эхо предъявлено в §7 (`GS-I-1`, `GS-I-2`, `GS-I-4`).

**Но это ровно тот класс, который карточка `TD-165` описала по PR #89** («норма о месте
FA-WAIVER расходится со своим барьером»): профили предписывают waiver «в теле коммита» и по
признаку «нет `docs/fa/<name>.md`», а барьер читает ТОЛЬКО вердикт-файлы и имеет СВОЙ закрытый
список NO-FA. Исполнитель сделал буквально то, что написано в норме, и норма его обманула —
второй раз, на том же самом крейте. Новую карточку не завожу: дописываю рецидив в `TD-165`
(карточка открыта, MINOR, зона правки — architect: `.claude/agents/**` под замком `gates.md` §11).

### `N-4` — оракул пиннит кратность попытки и успеха, но НЕ кратность отказа. Зона — architect

`l3` закрывает множественность для `attempts`/`successes` (две сессии ⇒ ровно `(2,2,0,0)`).
Для отказа такого сценария в наборе нет, и `l2` его не заменяет: его клиент после первого
кадра ошибки сразу закрывает соединение, а цикл ответов возвращает `Ok(())` на `Close`:

```
$ sed -n '2175,2178p' crates/gateway-serve/src/lib.rs
            while let Some(msg_result) = stream.next().await {
                match msg_result {
                    Ok(Message::Close(_)) | Err(_) => return Ok(()),
```

Следствие: мутант «инкремент отказа ВНУТРИ цикла ответов, в дополнение к инкременту до цикла»
оставляет `l2` зелёным — повторно запрошенный отказ считался бы дважды, и ни один оракул этого
не увидит.

**Почему это NOTE, а не REJECT.** Сегодняшний код верен: инкремент стоит на `:2149` — до
`ws.split()` и до цикла (проверено построчно, §5). И правило тишины на дельтах к завышенному
`refusals_supported` не ломается: оно требует `Δrefusals_supported > 0`, а не точного
значения. То есть цена дефекта, который оракул не ловит, — искажённая цифра в сердцебиении, а
не ложный/пропущенный алерт.

Развязку не проектирую. Предмет: сценарий, где отвергнутый клиент посылает ещё одно сообщение,
а дельта обязана остаться `(1,0,1,0)`.

---

## §9 — Done Block (прогон reviewer'а, сырой вывод; `/tmp/hft-reviewer-m91` на `2a50649`)

```
$ pwd && git log --oneline -1
/tmp/hft-reviewer-m91
2a50649 feat(M-91): task #1 — legacy path counts attempts/refusals/successes symmetric to v1 [engine-dev]

$ git status --porcelain
{пусто}

$ cargo fmt --all -- --check; echo exit=$?
exit=0

$ cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -2; echo exit=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.28s
exit=0

$ cargo test -p gateway-serve --test red_m91_legacy_counters 2>&1 | tail -6
running 3 tests
test l2_legacy_not_ready_counts_attempt_and_supported_refusal ... ok
test l1_legacy_success_counts_one_attempt_and_one_success ... ok
test l3_two_legacy_sessions_count_exactly_two ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.12s
exit=0

$ cargo test -p gateway-serve --features testing --test red_m91_legacy_counters 2>&1 | tail -7
running 4 tests
test l2_legacy_not_ready_counts_attempt_and_supported_refusal ... ok
test l4_legacy_warming_is_a_supported_refusal ... ok
test l1_legacy_success_counts_one_attempt_and_one_success ... ok
test l3_two_legacy_sessions_count_exactly_two ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s

$ cargo test -p gateway-serve --test red_m89_counters_instance --test red_m89_read_volume_truth --test red_m89_heartbeat_entrypoint 2>&1 | grep -E '^test result'
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.95s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.40s

$ cargo test -p gateway-serve --features testing --test red_m87_entrypoint 2>&1 | grep -E '^test result'
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.24s

$ cargo test --workspace 2>&1 | awk '/^test result: ok/ {p+=$4; b++} /^test result: FAILED/ {f+=$4; fb++} END {print "passed="p" failed="f+0" ok_blocks="b" failed_blocks="fb+0}'
passed=1178 failed=0 ok_blocks=269 failed_blocks=0

$ PR_BASE_SHA=$(git merge-base origin/main HEAD) bash scripts/verify_M-91.sh; echo VERIFY_EXIT=$?
VERIFY_EXIT=0
PASS_COUNT=54
FAIL_COUNT=0
SKIP_COUNT=8
...
SKIP  ci-parity: check_review_fa — вердикта R-NNN по M-91 в диапазоне ещё нет (зеленеет на PR-гейте)
SKIP  ci-parity: [eb31b754c4a50419] агрегат «All checks passed» (условие по needs); его правильность исполняет red_ci_aggregate.sh (EXEC)
PASS  ci-parity: bash scripts/tests/red_ci_aggregate.sh
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
SKIP  task2: §8-гейт — после деплоя wsprobe legacy-путём двигает attempts/successes в сердцебиении на проде — снимает reviewer
VERDICT: PASS
```

### §9.1 — Дерево СЛИЯНИЯ, а не только ветка (`gates.md` §8, `strict: false`)

Ветка отставала от `main` (`727bf8f`), поэтому зелень ветки сама по себе ничего не обещает
про `main` после merge'а. Прогон на дереве слияния `origin/main` + `2a50649` (`50a4191`,
конфликтов нет):

```
$ git merge --no-ff --no-edit origin/feat/M-91-legacy-serving-counters   # в worktree от origin/main
MERGE_EXIT=0        # автослияние без конфликтов

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS × 19, FAIL × 0
VERDICT: PASS (0 нарушений)
exit=0

$ cargo fmt --all -- --check; echo exit=$?            # на дереве слияния
exit=0

$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -2; echo exit=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.15s
exit=0

$ cargo test --all 2>&1 | awk '…'                     # на дереве слияния
passed=1186 failed=0 ok_blocks=270 failed_blocks=0
TEST_EXIT=0        # на 8 тестов и 1 блок БОЛЬШЕ, чем на ветке (1178/269) — ровно то,
                   # чего прогон на ветке не видит: это оракулы, приехавшие в `main` после
                   # базы ветки. Проверять документ и код на дереве слияния — не церемония.
```

### §9.2 — Мутационный контроль reviewer'а (не пересказ спеки §10, свой прогон)

Проверяю не «тесты зелёные», а «оракул ПИННИТ именно те строки, которые ввёл dev». Обе
мутации — временная правка в своём worktree, после каждой дерево восстановлено ПОБАЙТОВО
(`diff -q` с копией до правки + пустой `git status --porcelain`).

```
# Мутант 1: нейтрализован metrics::inc_successes (lib.rs:2380)
$ sed -n '2380p' crates/gateway-serve/src/lib.rs
        // MUTANT: metrics::inc_successes(counters.as_ref());
$ cargo test -p gateway-serve --test red_m91_legacy_counters
test l2_legacy_not_ready_counts_attempt_and_supported_refusal ... ok
test l1_legacy_success_counts_one_attempt_and_one_success ... FAILED
test l3_two_legacy_sessions_count_exactly_two ... FAILED
  l1: … дала дельты (1, 0, 0, 0) вместо (1, 1, 0, 0)
  l3: … дали (2, 0, 0, 0) вместо (2, 2, 0, 0)
test result: FAILED. 1 passed; 2 failed
$ git checkout -- crates/gateway-serve/src/lib.rs && diff -q /tmp/m91-lib-backup.rs crates/gateway-serve/src/lib.rs
RESTORED_IDENTICAL

# Мутант 2: нейтрализован metrics::inc_refusals_supported (lib.rs:2149)
$ sed -n '2149p' crates/gateway-serve/src/lib.rs
            // MUTANT: metrics::inc_refusals_supported(counters.as_ref());
$ cargo test -p gateway-serve --features testing --test red_m91_legacy_counters
test l2_legacy_not_ready_counts_attempt_and_supported_refusal ... FAILED
test l1_legacy_success_counts_one_attempt_and_one_success ... ok
test l4_legacy_warming_is_a_supported_refusal ... FAILED
test l3_two_legacy_sessions_count_exactly_two ... ok
  l2: … дал дельты (1, 0, 0, 0) вместо (1, 0, 1, 0)
  l4: … дал дельты (1, 0, 0, 0) вместо (1, 0, 1, 0)
test result: FAILED. 2 passed; 2 failed
$ git checkout -- … && diff -q …
RESTORED_IDENTICAL
```

Разделение чистое: мутант успеха роняет РОВНО `l1`/`l3` и не задевает `l2`/`l4`; мутант
отказа — РОВНО `l2`/`l4` и не задевает `l1`/`l3`. Это и означает, что оракулы привязаны к
своим строкам, а не зелены «вообще».

### §9.3 — Расхождение с агрегатом tester'а, названное, а не замолчанное

Мой прогон гейта: **54 PASS / 0 FAIL / 8 SKIP**; отчёт tester'а: **55 PASS / 0 FAIL / 7 SKIP**.
Сумма строк одинакова (62), и обе служебные цифры совпадают буквально («исполнено 48,
исключено по карте 6»), то есть различается КЛАССИФИКАЦИЯ ровно одной строки. Какой именно —
по отчёту tester'а не восстановить: он предъявил счётчики и хвост, но не лог целиком
(`commit-discipline.md` допускает агрегат, и это цена агрегата). На вердикт не влияет:
`FAIL=0`, `VERDICT: PASS`, `exit=0` в обоих прогонах; все оракулы предмета и соседей GREEN в
обоих. Записываю как предел предъявления, а не как находку о коде.

---

## §10 — Вердикт

**APPROVE.**

| блок гейта | исход | основание |
|---|---|---|
| Block-scope | **PASS** | §1 — диф внутри `Allowed paths` §8 спеки; dev не тронул ни одного sacred-пути |
| Block-DoneBlock | **PASS** | §2 + §9 — сырой stdout с exit-кодами, перепроверено reviewer'ом независимо |
| Block-C (contracts) | **н/п** | §3 — `crates/contracts/**` не тронут, `schema` сердцебиения и wire не менялись |
| Block-risk (RISK-BLOCK) | **н/п** | §4 — ни один путь `risk`/`killswitch`/`oms`/`venue-*` не тронут; order-egress не вводится |
| RED-first + анти-плацебо | **PASS** | §5 — оракул введён architect'ом ДО реализации, равенства дельт, setup-guard'ы, мутационный контроль предъявлен спекой §10 и подтверждён составом ассертов |
| Механизм на пути (DoD) | **PASS с остатком** | механизм включён в несущий путь (`run_authorized_session` — прод-ветка `wsprobe`); прод-подтверждение — задача 2, §11 |
| Атомарность коммитов | **PASS** | §6 |
| Предъявление FA (M-66) | **PASS** | §7 — `GS-I-1`, `GS-I-2`, `GS-I-4` названы живыми из `docs/fa/viz-backend.md`; waiver не требуется |
| Находки | 4 × NOTE | §8 — ни одна не блокирует merge |

**Инварианты поставки §4 спеки — все GREEN** (прогон reviewer'а, §9): `I-1` (`l1`),
`I-2` (`l2`), `I-3` (`l3`), `I-3bis` (`l4`, `--features testing`), `I-4` (соседи
`red_m89_counters_instance`, `red_m89_read_volume_truth`, `red_m89_heartbeat_entrypoint`,
`red_m87_entrypoint`).

**Условие APPROVED выполнено до merge'а:** `verify_M-91.sh` exit=0 на дереве ветки, базовая
тройка CI зелена на ДЕРЕВЕ СЛИЯНИЯ (`gates.md` §8, `strict: false` — ветка отставала от
`main` на `727bf8f`), merge идёт через PR с зелёным агрегатом `All checks passed`.

---

## §11 — Что остаётся ПОСЛЕ merge и чьё это

1. **Задача 2 спеки — §8-гейт, зона reviewer'а (моя).** После деплоя: legacy-сессия `wsprobe`
   обязана двинуть `attempts` и `successes` в `gateway-serve.heartbeat` на проде. База снята
   ДО merge'а, чтобы дельту было с чем сравнивать:

   ```
   $ ssh … 'cat /var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat'   # 2026-10-03T08:50:40Z
   {"attempts":0,…,"refusals_supported":0,…,"successes":0,…}
   ```

   Пруф снимается **дополнением к этому вердикту** (`R-216` §12), а не новым номером.
2. **`TD-228` закрывается** этим merge'ем по телу (учёт заведён) — но ЗАКРЫТИЕ фиксируется
   только после пункта 1: карточка про то, что оператор видит реальный трафик, а не про то,
   что в коде появились три строки.
3. **`TD-220` остаётся ЧАСТИЧНО и не этим milestone'ом:** тревога не вычисляется, потому что
   бинарь `ops-watchdog` на VPS не собран (`TD-231` — он не собирается ни образом, ни
   деплоем). `M-91` снимает слепоту ПРОДЮСЕРА; потребителя он не строит и не обещал.
4. **Перенос спеки и гейта в `docs/archive/`** (норма `Р-2`) — зона architect'а, вместе с
   остатком того же переноса по `M-89`/`M-90`.
5. **Карточки `TD-236`, `TD-237` и рецидив в `TD-165`** — завожу сам в close-out'е
   (`TECH-DEBT.md` reviewer-owned).

---

## §12 — §8-гейт (post-merge): СНЯТ 2026-10-03, задача 2 исполнена

**Merge:** PR #298, merge-коммит `1d119d9e` (2026-10-03T10:05:00Z); ветка предмета удалена
после входа в `main` (`gates.md` §9).

```
$ gh pr checks 298 >/dev/null 2>&1; echo CHECKS=$?
CHECKS=0
$ gh pr checks 298 | grep 'All checks passed'
All checks passed	pass	6s	…/runs/37114284992/job/111180150442

$ gh run watch 37115214237 --exit-status >/dev/null 2>&1; echo CI_EXIT=$?     # CI на main
CI_EXIT=0
$ gh run watch 37115214224 --exit-status >/dev/null 2>&1; echo DEPLOY_EXIT=$? # Deploy (push)
DEPLOY_EXIT=0
$ gh run watch 37116132136 --exit-status >/dev/null 2>&1; echo DEPLOY2_EXIT=$? # Deploy (workflow_run)
DEPLOY2_EXIT=0

$ gh run list --branch main --limit 3
completed	success	Deploy to VPS	Deploy to VPS	main	workflow_run	37116132136	2m14s	2026-10-03T10:21:11Z
completed	success	Merge pull request #298 …	CI	main	push	37115214237	16m7s	2026-10-03T10:05:02Z
completed	success	Merge pull request #298 …	Deploy to VPS	main	push	37115214224	18m11s	2026-10-03T10:05:02Z
```

### Прод глазами (`gates.md` §8 п. 2)

```
$ ssh … 'date -u +%FT%TZ; docker ps --format "{{.Names}} {{.Status}}"; cd /root/hft-platform && git log --oneline -1'
2026-10-03T10:23:33Z
hft-gateway-serve Up 27 seconds (healthy) hft-platform-recorder:local
hft-recorder Up 32 seconds (healthy) hft-platform-recorder:local
1d119d9e Merge pull request #298 from a3ka/feat/M-91-legacy-serving-counters

$ ssh … 'cat …/journal-data/_data/recorder.heartbeat'
{"events":2265,"free_bytes":74752081920,"min_free_bytes":10737418240,"next_seq":779499705,
 "segment_index":916,"ts_wall_ms":1791023011081,"writable":true}      # журнал растёт, диск 74.7 ГБ свободно

$ ssh … 'grep RssAnon /proc/<gateway-serve>/status'
RssAnon:	   11220 kB            # не docker stats (TD-021): анонимная куча, 11 МБ
```

### Задача 2 — ЗАМЕР, а не рассуждение: legacy-сессия двигает счётчики на проде

Зонд `wsprobe` внутри прод-контейнера, СЕЛЕКТОР НЕ ПОСЫЛАЕТСЯ ⇒ сервер берёт серверный
селектор ⇒ это ровно legacy-путь (`run_authorized_session`), тот самый, которым ходит
единственный реальный клиент (`TD-228`).

```
$ docker exec hft-gateway-serve sh -lc 'wsprobe --url ws://127.0.0.1:8080 --secret "$GATEWAY_JWT_SECRET" --frames 2 --seconds 25 --out /tmp/m91probe'
… snapshot отдан, schema_version 11, cob/vwap/cvd/profile непусты …
wrote /tmp/m91probe (snapshot.json, frames.jsonl, summary.json, panel.html)
PROBE_EXIT=0

$ docker exec hft-gateway-serve sh -lc 'wsprobe … --frames 1 --seconds 20 --out /tmp/m91probe2'
PROBE2_EXIT=0
```

| момент (UTC) | `ts_wall_ms` | `attempts` | `successes` | `refusals_supported` | `refusals_unsupported` | `journal_payload_bytes_read` |
|---|---|---:|---:|---:|---:|---:|
| до merge'а, 10:04:53 | 1791021887087 | 0 | 0 | 0 | 0 | 7 648 937 |
| после деплоя, до зонда, 10:23:56 | 1791023036725 | 0 | 0 | 0 | 0 | 0 |
| после ПЕРВОЙ legacy-сессии, 10:24:24 | 1791023056733 | **1** | **1** | 0 | 0 | 129 936 038 |
| после ВТОРОЙ legacy-сессии, 10:25:09 | 1791023106751 | **2** | **2** | 0 | 0 | 262 713 882 |

**Дельты:** первая сессия `(1, 1, 0, 0)` — ровно `I-1`; две сессии подряд `(2, 2, 0, 0)` —
ровно `I-3`. То есть прод воспроизвёл равенства оракулов `l1` и `l3` на РЕАЛЬНОМ трафике, а не
на фикстуре. До правки те же три успешные подписки давали `attempts:0, successes:0` при росте
прочитанных байт на 583 МБ (`R-211` §5.2) — это и был предмет `TD-228`.

### Ловушка §8-замера, которую называю, а не замалчиваю

Сердцебиение выдачи пишется **раз в ~10 с** (`ts_wall_ms`: …056733 → …066736 → …076739).
Чтение СРАЗУ после зонда (10:24:06) вернуло ещё ДОпробный снимок — те же нули и тот же
`ts_wall_ms`, что до зонда. Одного чтения для §8 НЕДОСТАТОЧНО: его надо повторять до сдвига
`ts_wall_ms`, иначе верный механизм читается как «счётчики не двинулись», а неверный — как
«всё хорошо» при протухшем файле. Замер снят пятью чтениями с шагом 4–5 с, а не одним.

### Чего этот гейт НЕ доказывает (границы названы)

1. **Тревога по-прежнему не вычисляется** — `ops-watchdog` на VPS не собран и не собирается
   ни образом, ни деплоем (`TD-231`). `M-91` снял слепоту ПРОДЮСЕРА; потребителя он не строил.
   Поэтому `TD-220` остаётся ЧАСТИЧНО, и это не остаток `M-91`.
2. **Ветка ОТКАЗА на проде не замерена** — для `not_ready` потребовалось бы привести выдачу в
   неготовое состояние на живом сервере. Она пиннится оракулами `l2`/`l4` и мутационным
   контролем (§9.2), а не прод-замером; смешивать два уровня предъявления не буду.
3. **`journal_payload_bytes_read` 130 МБ на первую выдачу** — цена cold-resume, предмет
   `TD-229`, не этого milestone'а.

**Вердикт по §8: PASS.** `TD-228` закрыт поставкой и прод-замером.
