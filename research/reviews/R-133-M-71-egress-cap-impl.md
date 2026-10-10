<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: 0adc4e185807484fc5dff4b18ab67c11f091b4e1
audited_head: ca482e406cab012a355d26b9bfc70719f9ea6b53
verdict: REJECT
-->

# R-133 — M-71 «предел объёма ответа» (impl engine-dev): PR-time reviewer, **REJECTED**

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-08-26
**Предмет:** `0adc4e1..ca482e4` на `origin/feat/M-71-egress-cap-dev` (4 коммита engine-dev)
**Дерево слияния:** `git merge-tree --write-tree origin/main ca482e4` — без конфликтов
(`origin/main` = `38671e7`; merge-base с веткой = `3b49620`)
**Вердикт:** **REJECTED** — четыре блокера, каждый предъявлен ИСПОЛНЕНИЕМ, не рассуждением.

---

## 0. Живой инвариант тронутого модуля (M-66, `gates.md` §4)

Диф трогает `crates/gateway/src/**` и `crates/gateway-serve/src/**`; собственной FA у крейтов
нет, барьер `check_review_fa.sh` мапит оба на `docs/fa/viz-backend.md`. Живые ID, задетые
предметом:

- **`VB-I-2`** (`docs/fa/viz-backend.md:189`) — «**live == replay**: серия, посчитанная на
  live-хвосте, бит-идентична серии из replay того же окна журнала». Нарушен — **B-2**.
- **`VB-I-11`** (`:198`) — «Провенанс ИСТОРИИ… Система не отказывается отдать то, что есть,
  но обязана **НЕ ВЫДАВАТЬ ЭТО ЗА ДРУГОЕ**: `Snapshot` несёт `history_start_seq` … и
  `history_truncated`». Нарушен — **B-3**.
- **`VB-I-10`** (`:197`) — предел ПАМЯТИ, соседний инвариант; не ослаблен (шаг `E` гейта зелен).

`FA-WAIVER` не требуется.

## 1. Что ПРОШЛО (чтобы не потерялось на фоне блокеров)

| блок | результат |
|---|---|
| **Block-scope** | ✅ Диф — ровно 4 файла: `crates/gateway/src/lib.rs`, `crates/gateway/Cargo.toml`, `crates/gateway-serve/src/lib.rs`, `docker-compose.yml`. Все в `Allowed paths` спеки §4. `serde_json` в `[dependencies]` gateway разрешён спекой §0ter и `scope-guard.md` §«Билд-конфиги». Утечек в `crates/{contracts,risk,killswitch,journal,book,venue-*}` — ноль. |
| **RED-first** | ✅ `git diff --name-only 0adc4e1..ca482e4 -- '*/tests/*' scripts/ milestones/ docs/` — ПУСТО. Sacred-оракулы не тронуты dev'ом. |
| **Block-C** | ✅ `crates/contracts` не тронут (шаг `F` гейта). Contract-RFC не требуется. |
| **Риск-блок** | ✅ НЕ триггерится: `gateway`/`gateway-serve` — read-only консюмеры журнала (`VB-I-3`, `GS-I-3`), order-egress отсутствует, путей `crates/{risk,killswitch,oms,venue-*}` диапазон не касается. `risk-critic` не требуется (`gates.md` §5 + MD-only carve-out). |
| **Done Block tester'а** | ✅ Воспроизведён МОИМ прогоном на СВОЁМ чекауте: `verify_M-71.sh` → `VERDICT: PASS`, `exit=0` (сырой вывод — §5). |
| **Атомарность** | ⚠ NOTE N-4 ниже. |

**Гейт зелёный, и это не оправдание.** Все четыре блокера ниже живут В ЩЕЛЯХ набора: три из
них — в поведении, которое оракулы не судят вовсе, четвёртый — в коде, добавленном ПОСЛЕ
последнего круга plan-time. Спека §0quater сама это предупредила: «plan-time не последний
рубеж».

---

## 2. B-1 — BLOCKER: операторская ручка **не подключена** (built-not-wired)

**Что.** `GATEWAY_MAX_RESPONSE_BYTES` разбирается, валидируется, кладётся в atomic — и
**никогда не читается**. Все десять точек enforce'а передают КОМПИЛЯЦИОННУЮ константу.

```
crates/gateway-serve/src/lib.rs:243   pub fn effective_max_response_bytes() -> usize
crates/gateway-serve/src/lib.rs:2166  server::set_effective_max_response_bytes(max_response_bytes);
```

`grep -rn 'effective_max_response_bytes' crates/` даёт **три** совпадения: определение
геттера, определение сеттера, один вызов сеттера. **Ни одного чтения.** Для сравнения — те
самые прецеденты, по образцу которых написан код (`lib.rs:234` называет их прямо):

```
effective_max_subs()   → читается на КАЖДОМ соединении   crates/gateway-serve/src/lib.rs:821
effective_grace_ms()   → читается на КАЖДОЙ сессии       crates/gateway-serve/src/lib.rs:603
effective_max_response_bytes()  → 0 читателей
```

**Воспроизведение (мой прогон, сырой вывод):**

```
[R-133 B-1] serve_config_from_env(GATEWAY_MAX_RESPONSE_BYTES=1000) -> Ok
[R-133 B-1] effective_max_response_bytes() = 1000
[R-133 B-1] gateway::snapshot отдал Ok(224854 Б) при заявленном пределе 1000 Б
```

Оператор объявил предел 1000 байт; сервис отдал 224 854 байта. Ручка декоративна.

**Отдельно — ложное утверждение О КОДЕ в самом коде.** `crates/gateway-serve/src/lib.rs:241`:

> «Получить runtime-предел объёма ответа (**читается на каждом `snapshot`/`frames_since`/
> `pump`-вызове**, см. `enforce_response_limit`).»

`enforce_response_limit` (`crates/gateway/src/lib.rs:1953`) принимает `limit: usize`
параметром, и все вызывающие подставляют `DEFAULT_MAX_RESPONSE_BYTES`. Комментарий описывает
механизм, которого нет, — класс `TD-138` («документ обосновывает инвариант механизмом,
которого нет»), только внутри исходника.

**Почему блокер, а не долг.** `gates.md` §4, DoD «Механизм на пути»: milestone, вводящий
механизм несущего пути, мержится ТОЛЬКО с подключением механизма к этому пути, доказанным
оракулом точки входа. Задача §5 #4 звучит «конфигурация предела», а доставлена половина:
отказ старта на мусоре есть, УПРАВЛЕНИЕ пределом отсутствует. `docker-compose.yml:150`
объявляет `${GATEWAY_MAX_RESPONSE_BYTES:-2000000}` оператору как рабочую ручку — это
обещание, которого код не держит.

**Почему гейт этого не поймал.** `red_egress_cap_startup.rs` судит ТОЛЬКО код возврата
`serve_config_from_env` (Ok/Err) — ни один из десяти его оракулов не проверяет, что
разобранное значение чем-то УПРАВЛЯЕТ. Шаг `D` гейта проверяет наличие строки в
`docker-compose.yml`, то есть ДОСТАВКУ объявления, а не действие. Обе проверки зелены при
полностью отключённой ручке.

## 3. B-2 — BLOCKER: предел действует **по-разному** на live и replay (`VB-I-2`; §4.1 запрет)

**Что.** `LiveReducer::snapshot_checked` (`crates/gateway/src/lib.rs:3517-3532`) при
превышении **не отказывает, а усекает**: вычищает `volume_bubbles` и отдаёт `Ok`. Тот же
ресурс через библиотечный `gateway::snapshot` даёт `Err`.

**Воспроизведение (одна и та же фикстура, 25 000 сделок):**

```
[R-133 B-2] gateway::snapshot            -> Err(PL-I-5: response exceeds limit:
                                              limit=2000000 bytes, observed=2804856 bytes
                                              (heatmap=0 cob=0 vp_bins=25000 bubbles=25000 ohlcv=25 depth_rows=0))
[R-133 B-3] LiveReducer::snapshot()      -> 2804765 Б, bubbles=25000, history_truncated=false
[R-133 B-2] LiveReducer::snapshot_checked -> Ok(654765 Б), bubbles=0, vp_bins=25000, ohlcv=25,
                                              history_truncated=true, history_start_seq=0
```

Клиент, подписавшийся по WS (`crates/gateway-serve/src/lib.rs:847` switch-путь, `:939`
ADD-путь, `:1469` legacy-путь), получает снимок, из которого **молча вынуты 25 000 пузырей**,
и никакого признака этого, кроме захваченного чужого флага (см. B-3). Реплей того же окна
вернёт `Err`. Это ровно та строка, которую спека §4.1 внесла в ЗАПРЕТНЫЙ список:

> «ослаблять `VB-I-2` (live == replay) | предел обязан действовать **ОДИНАКОВО на обоих
> путях**, иначе реплей перестанет воспроизводить live»

**Почему гейт этого не поймал — две причины, обе структурные.**

1. Оракул `B` (`crates/gateway/tests/red_egress_cap.rs:524`) запрещает усечение, но судит
   `gateway::snapshot` — путь, на котором dev усечения и не делал.
2. Уровень 2 (`W3`, `crates/gateway-serve/tests/red_egress_cap_wire.rs:390`) судит
   ИСКЛЮЧИТЕЛЬНО размер: `assert!(n <= PROPOSED_CAP)`. Усечённый ответ **проходит его по
   построению** — он же стал меньше. Полнота на уровне 2 не проверяется ни одним оракулом.
3. Сторож `red_gateway_live_eq_replay` (шаг `E` гейта) гоняет фикстуру ПОД пределом, поэтому
   к расхождению слеп.

**Граница reviewer↔architect соблюдена** (`gates.md` §4): я описываю дефект, я НЕ проектирую
фикс. Спека допускает «явно помеченное усечение» как альтернативу отказу — но она же требует
одинакового поведения на обоих путях. Какое из двух требований уступает и что становится
маркером — решение architect'а, и оно требует своего RED-оракула (`testing.md`: «исправление
по вердикту тоже требует оракула»).

## 4. B-3 — BLOCKER: `history_truncated` захвачен под чужой смысл (`VB-I-11`)

**Что.** `crates/gateway/src/lib.rs:3528` помечает усечение ПОЛЕЗНОЙ НАГРУЗКИ флагом,
который в этом протоколе означает потерю ПРЕФИКСА ЖУРНАЛА:

```rust
truncated.series.volume_bubbles.clear();
truncated.history_truncated = true;          // ← lib.rs:3528
```

`history_start_seq` при этом не трогается. Документированная в том же файле эквивалентность
ломается:

```
crates/gateway/src/lib.rs:2490   /// M-48 (VB-I-11): `true` ⇔ `history_start_seq > 0`.
```

**Воспроизведение:** `history_truncated=true, history_start_seq=0` — ответ противоречит сам
себе. Потребители читают ОБА поля: `crates/gateway-serve/src/bin/wsprobe.rs:494,506` печатает
их рядом; фронт получает их в `Snapshot` целиком (`GS-I-4`).

**Почему это хуже, чем «неудачное имя флага».** `VB-I-11` введён ровно против этого класса —
дословно: «Система не отказывается отдать то, что есть, но обязана **НЕ ВЫДАВАТЬ ЭТО ЗА
ДРУГОЕ**». Здесь деградация выдана за ДРУГУЮ деградацию: оператор, увидев
`history_truncated=true`, пойдёт искать retention-prune, которого не было. `PL-I-7`
(«деградация не выдаётся за норму») формально соблюдён, `VB-I-11` — нарушен.

Дополнительно: смена СЕМАНТИКИ поля провода прошла **без bump `GATEWAY_SCHEMA_VERSION`**
(остался `8`, `crates/gateway/src/lib.rs:65`). `VB-I-11` требует bump на смену формы; смена
смысла при неизменной форме для консюмера хуже — она невидима.

## 5. B-4 — BLOCKER: клиентски-достижимая **ПАНИКА** в коде, добавленном против DoS

**Что.** Усечение эха `venue` режет строку **по БАЙТУ**, а не по символу:

```
crates/gateway-serve/src/lib.rs:772   &name[..MAX_VENUE_ECHO],   // UnknownVenue
crates/gateway-serve/src/lib.rs:786   &s[..MAX_VENUE_ECHO],      // Invalid
```

`name` — произвольная строка из клиентского JSON (`crates/gateway-serve/src/wire_v1.rs:130`,
`other.to_string()`). Индексация `String` по байтовому диапазону **паникует**, если граница
не совпадает с границей символа. Комментарий рядом говорит «256 **символов**» — код считает
байты.

**Воспроизведение через СОКЕТ, прод-границей (`connect_async`), не вызовом приватной функции:**

```
[R-133 B-4a] ASCII venue 300 Б -> Some((375, Some(String("unknown_venue")), Some(String("error"))))   ← контроль: честная ошибка приходит

thread ... panicked at crates/gateway-serve/src/lib.rs:772:46:
end byte index 256 is not a char boundary; it is inside '日' (bytes 255..258 of string)
[R-133 B-4b] multibyte venue 300 Б -> None                                                             ← клиенту НЕ пришло НИЧЕГО
```

Вход: `venue = "日".repeat(100)` — 300 байт, байт 256 приходится на середину символа.
Сессия падает, соединение рвётся, клиент не получает ни `code`, ни причины.

**Три следствия сразу:**
1. `W-C3` («честная ошибка по-прежнему доставляется с кодом и причиной») нарушен для
   многобайтового входа — оракул гоняет только ASCII и потому зелен;
2. лечение egress-DoS само даёт клиенту дешёвый способ ронять обработчик — panic на каждое
   сообщение вместо ответа;
3. падение локализовано задачей соединения (`tokio::spawn`, `crates/gateway-serve/src/lib.rs:368`,
   профиль `panic=unwind`) — процесс не умирает, поэтому **все liveness-проверки `gates.md`
   §8 останутся зелёными**: контейнер `healthy`, heartbeat свежий, журнал растёт. Ровно тот
   класс тихой деградации, против которого §8 и написан.

Код добавлен ПОСЛЕ последнего круга plan-time (коммит `ca482e4`), поэтому ни один критик его
не видел — это не упрёк набору, а причина, по которой PR-time гейт не пропускается.

---

## 6. NOTE — не блокеры, но обязаны быть записаны

**N-1 — `SNAPSHOT_ENVELOPE_BYTES = 300` есть ОЦЕНКА, а не замер, и свой же комментарий это
признаёт.** `crates/gateway/src/lib.rs:1975-1976`: «замер… показывает 223–228 Б оверхеда;
типичный прод-селектор укладывается в 300 (расширенный symbol/**мультиполосный селектор
поднимают до ~400**)». То есть при мультиполосном селекторе ответ, прошедший
`enforce_response_limit`, на проводе превысит объявленный предел примерно на 100 Б. На 2 МБ
это несущественно РЕСУРСНО, но утверждение «ответ ≤ limit» перестаёт быть истинным, а
единственная его проверка (`red_egress_cap_boundary.rs:97`) гоняет ту самую фикстуру, под
которую константа подобрана. Кандидат в `TECH-DEBT` severity MINOR.

**N-2 — `LEGACY_DRAIN_BATCH = 256` меняет поведение догона и не покрыт оракулом.**
`crates/gateway-serve/src/lib.rs:1397`: дренаж legacy-пути переведён с `usize::MAX` на
батчи по 256 событий. Число эмпирическое («256 × ~150 Б ≈ 38 КБ»), взято из плотности сделок
на ОДНОЙ фикстуре; на плотном L2-потоке одно событие весит иначе, и оценка не проверяется
ничем. Ни один оракул набора не судит поведение догона.

**N-3 — щель, в которую встала реализация, — свойство НАБОРА, и её закрытие за architect'ом.**
Уровень 2 судит только байты (`W3`), уровень 1 судит только библиотечные строители (`B`).
Между ними — прод-путь `resume → pump → snapshot_checked`, где полнота не проверяется вовсе.
Пока эта щель открыта, любой фикс B-2/B-3 останется незапиненным. Это НЕ моя зона
(`gates.md` §4: reviewer описывает, architect проектирует RED).

**N-4 — атомарность коммитов на грани.** `de85577` несёт задачи #1,#2,#3, `ca482e4` — #2,#6;
`commit-discipline.md` требует «одна задача = ≥1 коммит… разные задачи, даже мелкие и
связанные, — разные коммиты». Авто-reject правилом наступает на бандле из пяти; здесь три и
две, ссылки на задачи проставлены явно и тела коммитов содержательны. Засчитываю как NOTE,
не как отдельный блокер.

---

## 7. Done Block — сырой вывод (мой прогон, не пересказ)

```
$ pwd
/tmp/hft-reviewer-m71

$ git rev-parse HEAD
ca482e406cab012a355d26b9bfc70719f9ea6b53

$ git status --porcelain
(пусто)

$ git diff --name-status 0adc4e1..HEAD
M	crates/gateway-serve/src/lib.rs
M	crates/gateway/Cargo.toml
M	crates/gateway/src/lib.rs
M	docker-compose.yml

$ git diff --name-only 0adc4e1..HEAD -- '*/tests/*' scripts/ milestones/ docs/ crates/contracts/
(пусто — sacred-зоны dev'ом не тронуты)

$ grep -rn 'effective_max_response_bytes' crates/
crates/gateway-serve/src/lib.rs:243:    pub fn effective_max_response_bytes() -> usize {
crates/gateway-serve/src/lib.rs:248:    pub fn set_effective_max_response_bytes(n: usize) {
crates/gateway-serve/src/lib.rs:2166:    server::set_effective_max_response_bytes(max_response_bytes);
                                        ← ни одного ЧТЕНИЯ

$ bash scripts/verify_M-71.sh; echo "exit=$?"
=== task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all ===
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
PASS: cargo test --all --quiet
… (13 шагов, все PASS — полный лог /tmp/hft-rev-m71-verify.log)
VERDICT: PASS
exit=0
```

**Гейт зелёный — и это ЧАСТЬ находки, а не смягчающее обстоятельство.** Четыре блокера выше
живут там, куда ни один шаг гейта не смотрит: `D` проверяет объявление ручки, а не её
действие; `W3` проверяет размер, а не полноту; `W-C3` гоняет ASCII, а не многобайтовый вход.

**Воспроизводящий набор** — `/tmp/hft-rev-m71-repro2.rs` (три теста, сокетный оракул для B-4).
В репозиторий он НЕ идёт: `*/tests/**` — sacred-зона architect'а (`scope-guard.md`), и
оракулы на находки пишет он, а не reviewer. Прогон:

```
test r133_b1_operator_handle_is_decorative ... ok        ← B-1 воспроизведён
test r133_b2_b3_live_truncates_where_replay_refuses ... ok ← B-2/B-3 воспроизведены
test r133_b4_multibyte_venue_panics_instead_of_erroring ... FAILED  ← B-4: паника в lib.rs:772
```

## 8. Условие APPROVED

1. **B-1** — предел, которым управляет оператор, обязан УПРАВЛЯТЬ; либо ручка снимается и из
   `docker-compose.yml`, и из `serve_config_from_env`, и константа объявляется единственным
   источником. Полумеры «объявили, но не подключили» — это built-not-wired, `gates.md` §4.
2. **B-2/B-3** — одно поведение предела на live- и replay-путях; если усечение остаётся, у
   него обязан быть СВОЙ маркер, а не захваченный `history_truncated`.
3. **B-4** — усечение строки по границе символа (или иная форма, не паникующая на
   произвольном UTF-8 от клиента).
4. Каждая правка — со СВОИМ RED-оракулом (`testing.md`: «исправление по вердикту тоже требует
   оракула»); оракулы пишет **architect**, не dev.
5. N-1/N-2 — либо закрыть, либо завести карточками в `TECH-DEBT.md` (пишу их я на close-out).

**Маршрут:** REJECT → `architect` (проектирование фиксов B-2/B-3 затрагивает `VB-I-2`/`VB-I-11`
и требует RED-оракулов в sacred-зоне) → `engine-dev` (impl B-1/B-4 по оракулам) → `tester` →
reviewer (круг 2).
