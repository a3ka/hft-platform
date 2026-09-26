<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: 6d6e1f8f82e8e1f487c78c292b268fa587e47938
verdict: REJECT
-->

# C-234 — M-87 serving circuit breaker: REJECT

## Предмет и граница аудита

Сужу вершину `origin/feat/M-87-serving-circuit-breaker` `6d6e1f8f82e8e1f487c78c292b268fa587e47938`, не SHA из мандата и не локальное состояние общего checkout. База — `origin/main` `d5163b5b35abbca204a8981e974bd6e5a97eb9de`.

Закоммиченный набор полный по носителям: milestone, два RED-набора и `scripts/verify_M-87.sh`; T1 `contracts/**` не тронут. Но контракт и RED-набор неполны по семантике: они не фиксируют бюджет, отмену, слот, свежесть и — главное — входную точку, которая обязана остановить живой запрос до `spawn_blocking`.

**Вердикт: REJECT.** Dev не диспетчеризуется, пока architect не закроет R1–R4 новым закоммиченным RED-набором и повторным critic-кругом.

## Ответы на вопросы мандата

| Вопрос | Ответ |
|---|---|
| 1. Достаточен ли контракт допуска? | Нет. `AdmissionPolicy` содержит символы/полосы/число слотов, но не ограниченный набор профилей. Вход `Selector { venue: Binance, symbol: "BTCUSDT", timeframe_ms: 1, bands: CANONICAL, window_ms: None, depth_cadence_ms: None }` проходит нынешний общий валидатор: `1` делит сутки, а `None` — прямо unbounded offline. Если admission признает этот символ и полосы, контракт не способен его отклонить как профиль. В нём также нет исполнимого типа/предела для пяти бюджетов, ни точки кооперативной отмены. |
| 2. Анти-плацебо | Воспроизведено: `red_m87_cold_path_reads_nothing` — ровно 3 passed / 4 failed, exit 101. Это честный RED против текущего `LiveReducer`, но не доказательство, что публичный WS-путь вызовет новый предохранитель. |
| 3. «Всегда Unsupported» / «счётчик всегда ноль» | Мутация в одноразовом detached worktree: `admit` всегда возвращает `Unsupported` ⇒ краснеет `form_canonical_request_is_not_refused` (`red_m87_admission.rs:158-165`), exit 101. Нулевой счётчик ловит `form_payload_bytes_are_counted_at_all` (`:212-224`), exit 101. Это два полезных unit-стража; третьего независимого стража на *проводке реального WS-входа* нет. `c1_ready_state_is_actually_served` (`red_m87_cold_path_reads_nothing.rs:287-305`) обходит `admit` полностью. |
| 4. Нет C4/C9 | Пробел назван честно, но поставка от этого не становится годной. §14.3 запрещает только dispatch 5/6/8, хотя C6 сейчас тоже лишь формирует структуру и не проверяет эмиссию из serving path. Milestone не может стать GREEN, пока execution-RED для C4, C6 и C9 отсутствуют; иначе `verify` допускает реализацию по grep-следам. |
| 5. API §4.1 чрезмерен? | Требования «`admit` без I/O», отдельная проверка готовности и единая трактовка секрета — корректные поведенческие границы; они предотвращают cold work и повтор `TD-207`. Но предписание конкретных публичных модулей, `Vec<String>`, `Vec<f64>` и `Vec<u8>` забирает у dev выбор представления/инкапсуляции, не фиксируя нужный `allowed_profiles` и бюджет. То есть API одновременно переопределяет механику и недоопределяет обязательную политику. |
| 6. Граница M-84 | Отказ сам по себе не есть вычисление, поэтому policy на реальной WS-границе может быть отдельной от M-84. Но набор не доказывает эту границу: `red_m87_cold_path_reads_nothing` зовёт общий `LiveReducer::resume`, а настоящий `handle_v1_message` сегодня напрямую запускает `LiveReducer::resume` в `spawn_blocking`. GREEN можно получить глобальной правкой gateway, ровно тем местом гварда, которое `A-033` признал негодным (209/329 красных и поломка прод-argv). |
| 7. Полон ли запретный список? | Нет. Он не запрещает менять общую `gateway::validate_selector`/`LiveReducer::resume`/fallback checkpoint, WS error/соседние подписки, auth кроме ровно общей функции, `GATEWAY_CHECKPOINT_DIR`/checkpoint cron и `CHECKPOINT_BANDS`. Любая из этих правок может сломать `VB-I-10`, `VB-I-11`, `CT-RFC-09` §2.7 или вернуть холодный путь, оставаясь в Allowed paths. |
| 8. Реален ли verify-гейт? | Базовая линия честная: 12 FAIL, exit 1. Но шаг 9 (`scripts/verify_M-87.sh:95-101`) станет PASS от любых четырёх строк `cpus|mem_limit|memory`; он не связывает их с выдачей/recorder/прогревателем, не проверяет применённые лимиты, запас recorder или named degradation. Шаг 10 (`:104-109`) станет PASS от любой непустой записи, а не от решения по каждому изменённому ожиданию. Это зелёные при невыполненных задачах. |
| 9. Утверждения о коде | `bash scripts/verify_design_claims.sh --merge-preview origin/main` завершился PASS, exit 0. Поштучно сверены координаты M-87: `main.rs:17`, `gateway/lib.rs:3179-3186`, `:3762-3784`, `:4251-4252`, `session.rs:70-105`, `gateway-serve/lib.rs:2413`, `wsprobe.rs:155-160`, `ops.md:476` и `reading-map.md:82`. Они живы. Три протухшие номера §3.1 описаны как протухшие ссылки *других* документов и не выданы за текущие координаты. |

## Находки

### R1 — admission contract не может выразить bounded live profile и не фиксирует бюджет

`milestones/M-87-serving-circuit-breaker.md:102-104` задаёт `AdmissionPolicy { allowed_symbols, canonical_bands, max_concurrent_serves }`, хотя план требует также «ограниченный набор профилей» (`docs/plans/scale-program-2026-09-21.md:95-96`). Никакой RED не требует ограничения `timeframe_ms`, `window_ms` и `depth_cadence_ms`.

Это не академический вход: `crates/gateway/src/lib.rs:287-305` определяет `window_ms=None` как unbounded offline; `validate_selector` пропускает `None` и любой положительный делитель суток (`:3022-3054`). Значит указанный выше selector способен попасть в обещанное поле допустимых symbol/bands, оставаясь неограниченным профилем. Отдельно `red_m87_admission.rs:212-224` доказывает только, что byte-counter не нулевой на одном replay; он не задаёт ни пяти пределов, ни их исчерпания, ни точки остановки/отмены.

Условие снятия: RED-контракт на ограниченный live-profile и на каждый из пяти пределов с кооперативной отменой; он обязан судить реальный serving path, а не только наличие поля/модуля.

### R2 — C1/C2 обходят единственную точку, которая должна защищать публичный запрос

`crates/gateway/tests/red_m87_cold_path_reads_nothing.rs:128-134` называет прямой `gateway::LiveReducer::resume` «live path». Но настоящий публичный путь после `session::validate_selector` (`crates/gateway-serve/src/lib.rs:806-814`) непосредственно делает `spawn_blocking` и `LiveReducer::resume` для switch (`:842-849`) и ADD (`:938-944`). В RED-наборе нет ни одного WS/entrypoint-оракула, требующего `admit → readiness → named outcome` *до* этих вызовов.

Следствие двустороннее: dev может не подключить admission вообще, либо изменить общий `LiveReducer`/`validate_selector`. Второй вариант подменяет M-87 реализацией M-84: `A-033-M-84-fixed-bands.md:15-32` измерил у такого гварда 209 FAILED из 329 и поломку прод-argv. Это также не удерживает `VB-I-10` (окно остаётся bounded только для live) и `VB-I-11` (не менять честность checkpoint/replay).

Условие снятия: RED исполняет WS entrypoint с отсутствующим/битым/несовместимым/отставшим checkpoint и доказывает отсутствие `spawn_blocking`/payload-read; отдельная мутация обхода admission обязана его красить. Запретный список обязан сохранить общие offline/replay/checkpoint API.

### R3 — C4, C6 и C9 не готовы к dispatch, а их отсутствие нельзя закрыть декларацией

`milestones/M-87-serving-circuit-breaker.md:354-365` правильно признаёт отсутствие C4/C9, но позволяет немедленно выдать задачи 1–4, 7 и 9. Этого недостаточно для full artifact set: C3 не имеет исполнимого oracle на расход пяти бюджетов/отмену; C6 (`red_m87_admission.rs:231-285`) создаёт входные `ServingCounters`, но не доказывает producer/emission на serving path, что требуется `OPS-I-10` (`docs/fa/ops.md:478`); C9 отсутствует полностью.

`OPS-I-8` (`docs/fa/ops.md:476`) и `PL-I-8` требуют наблюдать «жив, но не работает». Счётчик без production producer и активной пробы не является таким наблюдением. Названное ограничение делает неготовность видимой, но не разрешает начать milestone с неполной RED-спецификацией.

Условие снятия: architect коммитит второй RED-набор до любого dispatch M-87; C4 проверяет удержание слота после response-timeout, C6 — producer+наружную sample/active probe, C9 — четыре различимых позиции и несрабатывание stop при checkpoint-lag.

### R4 — verify имеет ложнозелёные шаги 9 и 10

Шаг 9 `scripts/verify_M-87.sh:95-101` считает строки YAML и сам сообщает, что фактические `docker inspect` отложены. Он не меряет требование задачи 9 из `milestones/M-87-serving-circuit-breaker.md:291`: applied limits трёх классов сервиса, запас recorder и поведение на лимите. Четыре несвязанные строки сделают этот шаг зелёным.

Шаг 10 `scripts/verify_M-87.sh:104-109` требует лишь, чтобы пропала одна placeholder-фраза. Он не сравнивает §16.1 с каждым из десяти/восьми найденных оракулов (`milestone:390-407`) и не ловит пропуск решения. Оба шага нарушают `testing.md` свойство «меряет свой инвариант, а не прокси».

Условие снятия: verify должен связывать проверку с реальными service names и фактически применённой конфигурацией в deployment acceptance, а ревизию корпуса — с исчерпывающим выявленным списком/решением по каждому элементу. PASS не может появиться, когда задача 9 или 10 фактически не выполнена.

## FA / полномочия

- `VB-I-9`: изменение auth сохраняет stateless JWT boundary; единая `key_material` нужна именно для устранения `TD-207`, но не оправдывает ослабление verify.
- `VB-I-10` и `VB-I-11`: live bounded window и provenance checkpoint нельзя менять общим gateway-guard'ом.
- `OPS-I-8` и `PL-I-8`: выдаче нужен настоящий alarm «жив, но не работает», включая отсутствие спроса через активную пробу.
- `PL-I-4` и `PL-I-5`: user request не должен читать event-store и не может получить unbounded path.
- `FA-WAIVER: crates/gateway-serve — собственной FA нет; применены viz-backend §5, ops OPS-I-8 и DESIGN PL-I-4/5/8.`
- `FA-WAIVER: crates/gateway — собственной FA нет; применены viz-backend VB-I-10/11 и DESIGN PL-I-4/5/8.`

`П-014`, `П-020`, `П-027`, `П-029` и решение о парковке M-84 прочитаны. M-87 не вправе менять набор полос, response cap либо состав продукта; политика допуска на public entrypoint допустима только как технический предохранитель без подмены запроса и без превращения общей gateway-library в M-84.

## Done Block

```text
$ git fetch origin --quiet && git rev-parse origin/feat/M-87-serving-circuit-breaker
6d6e1f8f82e8e1f487c78c292b268fa587e47938
exit=0

$ git log --oneline origin/main..HEAD
6d6e1f8 docs(roadmap): SCALE — S0 получил дом M-87, набор закоммичен [architect]
2dddf9f chore(M-87): acceptance-гейт; базовая линия снята КРАСНОЙ [architect]
bb7516b test(M-87): RED формы допуска — политика, бюджет, сторож молчания, секрет [architect]
8ddaa13 docs(M-87): форма API дословно; пробел оракулов назван, а не умолчан [architect]
dd86a9c test(M-87): RED — живой путь не пересчитывает; четыре состояния слепка [architect]
d81f8a8 docs(M-87): спека — предохранитель выдачи, живой запрос не пересчитывает [architect]
exit=0

$ cargo test -p gateway --test red_m87_cold_path_reads_nothing
running 7 tests
test result: FAILED. 3 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ cargo test -p gateway-serve --test red_m87_admission
error[E0432]: unresolved import `gateway_serve::admission`
error[E0432]: unresolved import `gateway_serve::metrics`
error[E0432]: unresolved import `gateway_serve::auth::key_material`
error[E0609]: no field `payload_bytes_read` on type `ReadStats`
exit=101

$ bash scripts/verify_M-87.sh
FAIL  task1: нет модуля admission.rs с пятью исходами ServingOutcome
FAIL  task2: red_m87_cold_path_reads_nothing КРАСЕН — test result: FAILED. 3 passed; 4 failed
FAIL  task1+3+4+7: red_m87_admission КРАСЕН — компиляция
FAIL  task4: в ReadStats нет payload_bytes_read — бюджет по байтам мерить нечем
FAIL  task5: ограничителя параллелизма нет ни в одном файле crates/gateway-serve/src (найдено: 0)
FAIL  task6: нет metrics.rs с раздельными счётчиками отказов
FAIL  task7: key_material отсутствует либо зонд её не зовёт — трактовок по-прежнему две (TD-207)
FAIL  task8: свежести нет ни в одном файле выдачи (совпадений: 0)
FAIL  task9: ресурсных лимитов в docker-compose.yml недостаточно (строк: 0, нужно ≥4)
FAIL  task10: таблица §16.1 пуста — решение по каждому изменённому ожиданию не записано
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 12)
exit=1

$ [detached mutation] admit() => ServingOutcome::Unsupported
$ cargo test -p gateway-serve --test red_m87_admission form_canonical_request_is_not_refused -- --exact
test form_canonical_request_is_not_refused ... FAILED
assertion `left != right` failed
left: Unsupported
right: Unsupported
exit=101

$ [detached mutation] payload_bytes_read => 0
$ cargo test -p gateway-serve --test red_m87_admission form_payload_bytes_are_counted_at_all -- --exact
test form_payload_bytes_are_counted_at_all ... FAILED
счётчик прочитанных байт не растёт даже на полном реплее — мерить бюджет нечем
exit=101

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [3-ССЫЛКИ] все 8 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 636 ссылок вида docs/*.md указывают на существующие файлы
PASS  [6-RFC-SHA] SHA-подобных токенов: всего=38 проверено=38 пропущено=0
PASS  [7-RFC-PATH] путей-кандидатов: всего=274 проверено=182 пропущено=92
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/next_artifact_id.sh C
C-234
exit=0
```
