<!-- GATE-META
milestone: M-100
audited_repo: a3ka/hft-platform
audited_base: 0a907ad60b8aa6d8444abcbcaa5755b8d3d142ef
audited_head: 28cae3136102a55e6841ea372f7a84cff95f607b
verdict: REJECT
-->

# R-264 — M-100 «три контракта выдачи» (S1a-K), PR-гейт: **REJECT**

PR #351, ветка `origin/feat/M-100-three-serving-contracts`. Вершину взял командой (`git fetch
origin && git rev-parse …` → `28cae313…`) — совпала со справкой мандата. База аудита —
merge-base с `main` `0a907ad6` (PR #346); `main` с тех пор ушёл на `f43c1191` (+6, харнесс и
документы TD-257 — с предметом не пересекаются).

**Итог одной строкой.** Работа по существу хорошая: все пять RED-файлов зелёные, гейт
`VERDICT: PASS`, шесть мутантов из таблицы спеки §8 пойманы, фикстуры `m88` и расщепление
версий подтверждены независимо. Блокирует одно: **подписка без `window_ms` проходит допуск и
получает `not_ready` вместо `unsupported`** — ровно тот класс (`TD-242`, `П-030`), который
задача 5 закрывает, а `CT-RFC-09` §2.10.1, вливаемый этим же PR, объявляет закрытым.

## Предъявление FA

`crates/gateway/**` и `crates/gateway-serve/**` ⇒ `docs/fa/viz-backend.md`:
- **`VB-I-2`** (live == replay) — `h1`/`h2` зелёные, мутант «объединение наблюдений» роняет оба (М5);
- **`VB-I-12`** (контракт обновления карты, `Replace` колонки) — тот же оракул; строка FA ещё
  говорит «гейтить на `schema_version == 11`» — правится задачей 7 (`VB-I-13`), не здесь;
- **`VB-I-4`** (аддитивность формы) — три поля `Snapshot` с `#[serde(default)]`, фикстуры `m88`
  изменены ТОЛЬКО добавлением (ниже).
Плюс `PL-I-5` (`DESIGN` §22) — предел объёма, М4.
`gateway-serve` собственного FA не имеет (`reading-map.md` §2) — долг назван, не новый.

Грепом искал: `TECH-DEBT.md` — `TD-242`, `TD-197`, `M-100`; архив `TD-242`
(`docs/archive/TECH-DEBT-closed-2026-08-16.md:2862`); `PROJECT-STATE.md` — `M-100`.

## Блоки гейта

| блок | итог |
|---|---|
| Scope | **PASS с одной оговоркой.** Все пути ⊂ §6. Вне зоны dev — `5b501fe4` (24 фикстуры `crates/gateway/tests/fixtures/m88/**`, sacred); принято architect'ом (`3053ca99`, §14 круг 5). Проверил сам — см. «Фикстуры». `crates/contracts/**`, `journal`, `config/calc-profile`, `docker-compose.yml`, `deploy/**` не тронуты |
| Contract Block-C | PASS — T1 не тронут; провод — T-designate `gateway` (`05` §2), правка через `CT-RFC-09` §2.10 + критик на сильной модели (`C-303`…`C-309`, `A-054`) |
| RISK-BLOCK | не применяется — нет order-пути (`gates.md` §5) |
| RED-first | PASS — RED `ca708c27` раньше первой реализации `27ee9014`; dev `*/tests/*.rs` не правил (только фикстуры, выше) |
| Done Block / acceptance | PASS — сырой вывод ниже |
| Атомарность | **Н-2** (ниже) |
| Механизм на пути | PASS — прод зовёт `bind_with_policy` (`crates/gateway-serve/src/main.rs:49`); оракулы `w*` идут через WS-транспорт с политикой |

## Б-1 (блокер) — тройка `(timeframe, window, cadence)` с ОТСУТСТВУЮЩИМ `window_ms` не судится; клиент получает `not_ready`

**Где.** `crates/gateway-serve/src/admission.rs:154-178`, `profile_allowed`:
```
if let Some(w) = sel.window_ms {          // :177
    if p.window_ms != w { continue; }
}
```
`window_ms == None` проверку окна ПРОПУСКАЕТ (комментарий `:154`: «legacy/offline-режим …
admit НЕ отвергает»).

**Что нарушено.** `CT-RFC-09` §2.10.1 (этот PR): «`timeframe_ms`/`window_ms`/`depth_cadence_ms`
клиента обязаны тройкой совпасть с одним из `allowed_profiles`; иначе ⇒ `unsupported`». `None`
ни с одним `LiveProfile.window_ms: i64` не совпадает. Спека §1 п.1 «всё, что вне профиля, —
названный отказ `unsupported`», инвариант поставки `I-2`.

**Воспроизведение** (проба reviewer'а — временная копия `red_m100_three_contracts_wire.rs` +
один тест, удалена после прогона, дерево чистое):
```
selector = sel_json(None, 60_000) минус ключ "window_ms"
ZZ_PROBE window_absent => {"code":"not_ready","message":"readiness: NotReady","sub":"zz","type":"error","v":1}
assertion `left == right` failed  left: String("not_ready")  right: String("unsupported")
exit=101
```

**Почему это блокер, а не примечание.** `not_ready` по контракту значит «повтори позже», а
прогрев он не создаёт (`M-87` §4) — клиент, забывший `window_ms`, будет ретраить ВЕЧНО. Это
дословно вред, описанный `П-030` и спекой §2 п.3, и он остаётся достижим ровно на одной форме
входа. Безопасность не страдает (неограниченное окно не обслуживается — fail-closed держит
`readiness`), страдает контракт, который этот же PR вливает в `main` как действующий:
документ, утверждающий механизм, которого на пути нет, — класс `TD-138`.

**Происхождение — не самодеятельность dev'а, а конфликт двух sacred-текстов.** Обход сделан,
чтобы не покраснел `red_m87_admission::form_canonical_request_is_not_refused`
(`crates/gateway-serve/tests/red_m87_admission.rs:211`, селектор `sel_with` — `window_ms: None`,
`:115`). Оракул M-87 утверждает «канонический запрос не отвергается» именно на форме без окна;
RFC rev3 эту форму объявил вне профиля. Круг 5 обновил стражи семи прежних оракулов (§6), этот
в перечень не попал. Таблица мутантов §8 строку «тройка не сверяется» привязывает к `w5`, а
`w5` судит только `Some(30_000)` — пропуск оси оракулом не покрыт.

**Условие APPROVE (решение — architect'а; reviewer дефект описывает, не проектирует —
`gates.md` §4).** Одно из двух, с RED на саму находку (`testing.md` «исправление по вердикту
тоже требует оракула»):
- (а) RFC в силе: оракул M-87 приводится к профилю (окно профиля), отсутствие `window_ms` →
  `unsupported`, RED-кейс «тройка без окна» на проводе; либо
- (б) RFC правится: отсутствие окна названо явно (что именно получает клиент и почему не
  вечный `not_ready`), и это решение проходит тот же маршрут, что §2.10.

Попутно на том же месте (не блокеры, но правятся той же правкой):
- **Н-1** — текст отказа `crates/gateway-serve/src/lib.rs:1241` «(bands/symbol/timeframe_ms)»
  снова расходится с осями `admit()`: теперь судятся ещё `window_ms`/`depth_cadence_ms`.
  Предсказано `R-235` Н-2 (оракула на текст нет). FA `docs/fa/ops.md:479` (`OPS-I-11`) — «три
  проверки `admit()` … `timeframe_ms`» — тоже устарело (зона architect'а).

## Н-2 — бандлы задач в коммитах dev'а

`27ee9014` «task #4 — version decoupling, calc_key, view module» несёт модули задач 3 и 2;
`b41e0ca1` «tasks #2 + #5» — две задачи одним коммитом. `commit-discipline.md`: «бандл на
несколько задач одним коммитом = авто-reject». Прецедент — `R-196` B9 (REJECT), затем `R-200`
принял исторические бандлы как непереписываемый долг аудит-трейла. Здесь поступаю так же:
историю не переписывать (ветка опубликована и прошла тестера), **но правка по Б-1 — отдельным
атомарным коммитом на задачу**. Самостоятельной причиной REJECT не делаю, чтобы не выдавать
процессную находку за предметную.

## Н-3 — устаревший комментарий в коде

`crates/gateway/src/lib.rs:4858-4866` (писатель слепка): «sacred-тест
`red_m87_admission::u2_readiness_fresh_checkpoint_is_ready` … держится красным как КОНТРАКТНЫЙ
долг». Ложно с `872f59f9`: оракул приведён к `CALC_STATE_VERSION` и зелёный (М2 его роняет —
значит он сторожит). Зона engine-dev.

## Проверено и принято (по пунктам мандата)

**Фикстуры `m88` (`5b501fe4`).** Плоский диф всех 24 JSON (`git show 5b501fe4^:` против
`5b501fe4:`), объединение изменений по всем файлам:
```
('/calc_key_id', '<absent>', 'd9fb7211e2e906e6e57aec2110eff33577c12f7c8afcb659c4525aed7444b80e')
('/calc_profile', '<absent>', 'None')
('/heatmap_window_e8', '<absent>', '100000')
('/schema_version', '11', '12')
fixtures unchanged since 5b501fe4
indep sha256 d9fb7211e2e906e6e57aec2110eff33577c12f7c8afcb659c4525aed7444b80e
```
Независимый `hashlib.sha256` канонической формы §2.10.4 (`calc-key/v1`, Binance, BTCUSDT, 1000,
`none`, `none`, `100000`) совпал. Других изменений нет. Нарушение зоны dev'ом названо, содержимое
верно.

**Дерево слияния.** `git rev-parse 28cae313^{tree} e8e6c7f8^{tree}` →
`fa1824f8…` дважды — равны; пересоздание merge-коммита architect'ом содержимого не меняет.

**`62adca04`** (`parse_selector` без политики принимает `bands` как есть): путь только `bind`
без политики; прод — `bind_with_policy` (`main.rs:49`). При политике с пустым
`canonical_bands` клиентские полосы дальше режет `bands_subset` в `admit` ⇒ `unsupported` —
fail-closed. Принимаю.

**Слияние M-98 + M-100 в `gateway-serve/src/lib.rs`** — `cargo test --all` и
`cargo test -p gateway-serve --features testing` в гейте зелёные, CI ветки зелёный.

**`TD-197`** закрывается этой поставкой (окно карты объявлено в `Snapshot.heatmap_window_e8`) —
карточку закрою на close-out после §8, не раньше.

## Мутационный контроль (reviewer, на `28cae313`; каждая правка — откат `git checkout`)

```
M1 read_and_validate сверяет GATEWAY_SCHEMA_VERSION
   gateway/red_m100_state_version exit=101 :: sv2_checkpoint_with_state_version_is_accepted_under_new_wire_version FAILED; sv3_foreign_state_version_is_rejected FAILED
M2 admission::readiness сверяет GATEWAY_SCHEMA_VERSION
   gateway-serve/red_m100_three_contracts_wire w8 exit=101 :: w8_pre_m100_checkpoint_state_11_admitted_under_wire_12 FAILED
   gateway-serve/red_m87_admission exit=101 :: u2_readiness_fresh_checkpoint_is_ready FAILED; u2_readiness_incompatible_checkpoint FAILED
M3 readiness принимает любую версию состояния
   gateway-serve/red_m100_three_contracts_wire w9 exit=101 :: w9_foreign_state_version_is_not_ready FAILED
M4 PL-I-5: снимок судится по SeriesBundle+300, а не целиком
   gateway/red_egress_cap_boundary exit=101 :: pl_i_5_boundary_last_accepted_fits_and_next_is_refused FAILED
M5 объединение наблюдений в book_series_in
   gateway/red_m100_heatmap_frame_close exit=101 :: h1_level_removed_within_one_live_frame_disappears_for_client FAILED; h2_same_events_different_live_frame_boundaries_same_state FAILED
M6 view не применён к кадрам (v1 pump)
   gateway-serve/red_m100_three_contracts_wire w exit=101 :: w1_server_resolves_calc_key_and_view_selects_rows FAILED; w7_resubscribe_changes_view_not_calc FAILED
$ git status --porcelain crates/    → (пусто)
```
Расщепление версий судится с обеих сторон: библиотека — `sv2`/`sv3`, допуск — `w8`/`w9`/`u2`.
`PL-I-5` судит весь ответ — М4 роняет оракул границы.

## Done Block

```
$ git fetch origin && git rev-parse origin/feat/M-100-three-serving-contracts
28cae3136102a55e6841ea372f7a84cff95f607b
$ git rev-parse 28cae313^{tree} e8e6c7f8^{tree}
fa1824f8a2c96c7b481ce3fe1dccb7a10e7a5a08
fa1824f8a2c96c7b481ce3fe1dccb7a10e7a5a08

$ bash scripts/verify_M-100.sh 2>&1 | grep -E '^(PASS|FAIL|SKIP|VERDICT)'; echo exit=$?
PASS  task1: red_m100_heatmap_frame_close (h1, h2)
PASS  task2: red_m100_view_projection (p1…p4)
PASS  task2/4/5: red_m100_three_contracts_wire (w1…w9)
PASS  task3: red_m100_calc_key_id (k1…k5)
PASS  task4: red_m100_state_version (sv1…sv3)
PASS  I-reg: gateway red_m88_update_contract
PASS  I-reg: gateway red_m88_golden_vectors
PASS  I-reg: gateway-serve red_m87_admission
PASS  I-reg: gateway-serve red_m95_provenance_fresh (f0…f6)
PASS  ci-map: проба карты CI-паритета (red_verify_M-100_ci_map.sh; число миров печатает проба)
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
PASS  ci-parity: cargo test --all
PASS  ci-parity: cargo test -p gateway-serve --features testing
…
PASS  ci-parity: учтено шагов 64 из 64 (исполнено 57, исключено по карте 6)
SKIP  task6: §8 — снимок wsprobe v12 …; прод-слепок принят тёплым путём без пересборки — снимает reviewer
VERDICT: PASS
exit=0
(FAIL-строк в логе: 0)

$ gh pr checks 351   (до коммита этого вердикта)
fmt + clippy + test (ветка)   pass   16m22s
Review FA …                   fail   — ожидаемо: вердикта с живым инвариантом на ветке ещё не было
прочие 19 джобов              pass
```

Задача 6 (§8 деплой) **не исполнялась**: при REJECT merge'а нет.

## Условие APPROVE

1. Б-1 закрыт по варианту (а) или (б), с RED на находку, падающим на `28cae313`.
2. Н-1/Н-3 — той же правкой (текст отказа и комментарий); FA `OPS-I-11` — architect.
3. Правка — атомарными коммитами (Н-2). Следующий круг судит ТОЛЬКО диф после `28cae313`
   плюс повтор гейта; принятое здесь не переоткрывается.

## Handoff §D — кому

Разбор находки — **`architect`** (founder 2026-09-04; `.claude/agents/reviewer.md` §Handoff):
он выбирает (а)/(б), правит оракул/RFC сам и при необходимости диспетчеризует engine-dev.
