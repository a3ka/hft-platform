<!-- GATE-META
milestone: M-85
audited_repo: a3ka/hft-platform
audited_base: 95d2422cb175010fb6ba8f9302c8aa4a84bda71c
audited_head: f0205be59798477c03ccd4b08081a0f40e233d65
verdict: REJECT
-->

# C-223 — M-85 frames/book continuity — REJECT

Дата: 2026-09-16 UTC
Роль: critic, plan-time gate after architect artifacts
Предмет: `feat/M-85-frames-book-continuity` @ `f0205be` поверх `origin/main` @ `95d2422`.

## Verdict

**REJECT — dev не диспетчеризуется.** Набор действительно содержит milestone, T2-шов
`seed_book`, RED-набор и verify; T1/`contracts/**` не затронуты. `m85_1` честно красен на
предмете, `m85_2` и `m85_3` зелены, а предшествующий
`red_depth_provenance_by_reach` зелён на точной базе. Но центральный выбор К1/К2 не имеет
закоммиченного оракула и сформулирован против уже иной фактической топологии кода.

Живой инвариант предмета: **VB-I-2** из `docs/fa/viz-backend.md:199` — live и replay
одного окна должны давать бит-идентичную серию. Проверено открытием FA на audited head.

## REJECT findings

### B-1 — задача 3 названа, но её RED-оракул и шаг verify отсутствуют

`milestones/M-85-frames-book-continuity.md:153-159` делает измерение числа посещённых
событий единственным правилом выбора К1/К2. Затем `:167` прямо говорит, что оракул задачи 3
будет написан **после** этого вердикта. `scripts/verify_M-85.sh:12-20,85-112` содержит
шаги задач 1/2/4/5 и setup, но не задачу 3; `:29-31` это подтверждает.

Это неполный pre-dispatch artifact set, не допустимая RED-фаза. Решение арбитра A-028 §1
гласит: каждый оракул, названный спекой, существует закоммиченным до dispatch; допускается
только COMPILE-RED против дословно объявленной сигнатуры, но не отсутствующий текст.
`gates.md` §3 также требует хотя бы одну проверку на каждую задачу.

**Условие снятия:** architect коммитит до нового круга оракул границы ресурса и его
трёхисходный шаг в `verify_M-85.sh`. Оракул должен предъявлять объявленное правило выбора
на детерминированном счётчике посещённых событий, включая путь без чекпоинта; не достаточно
добавить обещание или измерение после dev.

### B-2 — ось К1/К2 не соответствует текущим путям, поэтому не может выбирать реализацию

Спека объявляет К1 и К2 невыбранными (`milestones/M-85-frames-book-continuity.md:145-155`),
но RED-набор вызывает **`gateway::frames_since`** (`crates/gateway/tests/red_m85_frames_book_continuity.rs:192-214`).
На audited head этот API уже открывает полный `journal::stream` (`crates/gateway/src/lib.rs:2966-2984`),
то есть уже является К1. Это намеренно: соседний комментарий `:2956-2965` запрещает
делегировать его seek-варианту, потому что тот ломает абсолютную семантику VWAP.

`frames_since_with_stats` — отдельный seek-bound API (`:2987-3045`), прямо отмеченный как
неиспользуемый `frames_since`/`gateway-serve`; он уже известен как несовместимый с VWAP
(`:2991-3012`). M-85 не объявляет, какой из этих двух APIs меняет К2, каким контрактом
находится/проверяется чекпоинт, либо как сохраняется существующий VWAP-инвариант. Поэтому
измерение, написанное после dispatch, не сможет превратить текущую ложную развилку в
однозначную спецификацию для dev.

**Условие снятия:** architect сначала исправляет ось на фактические функции и их
семантические ограничения, объявляет необходимый T2-шов/сигнатуру для выбранной формы и
коммитит соответствующий RED-оракул из B-1. Только после этого выбор может быть отдан
измерению, а не вкусу dev.

## NOTE

### N-1 — M-85-specific anti-placebo не ловит полный bootstrap, хотя общий RED-корпус ловит

Мутант в отдельном dirty worktree заменил в
`crates/gateway/src/lib.rs:2253-2255` `reducer.seed_vwap(&event)` на
`reducer.apply(&event)`: это запрещённый полный bootstrap, создающий double-counting
не-книжных серий. На нём все три `m85_*` стали зелёными. Значит `m85_2` (`red_m85…:280-309`)
не доказывает заявленную защиту от этой развязки сам по себе.

Это **NOTE**, не дополнительный блокер: уже существующий независимый
`red_gateway_live_eq_replay::mid_stream_snapshot_completeness_merges_same_bucket` краснеет
на том же мутанте (5 passed, 1 failed), а verify запускает `cargo test --all`. При исправлении
B-1/B-2 architect должен сохранить эту независимую защиту; дублировать решение в этом
вердикте не требуется.

### N-2 — ссылка на M-70 неразрешима в audited artifact set

`milestones/M-85-frames-book-continuity.md:231` ссылается на
`M-70-depth-bands-enablement.md §3sexies (в-0)`, однако `rg` по этому milestone на audited
head не находит ни `3sexies`, ни `в-0` (exit 1). Исправить или заменить ссылку на живой
носитель зависимости при ревизии B-1/B-2. Это не самостоятельный блокер: `TD-204` прямо
фиксирует, что M-70 ждёт M-85.

## Artifact and scope audit

- Полный предмет в диапазоне `95d2422..f0205be`: ровно
  `milestones/M-85-frames-book-continuity.md`,
  `crates/gateway/tests/red_m85_frames_book_continuity.rs`,
  `scripts/verify_M-85.sh`.
- T1/`contracts/**`, `docs/rfc/**`, `crates/venue-*`, `crates/journal/**` и
  `crates/gateway-serve/src/**` в диапазоне не менялись. Contract-RFC и risk-critic не
  триггерятся.
- Allowed paths соответствуют architect/engine-dev split из scope-guard. Объявленный
  внутренний T2-шов `seed_book` достаточен для предмета B-1, но ещё не определяет K2 из B-2.
- Verify использует явный FAIL-счётчик, self-check helper'ов, корректный terminal exit и
  повторяет базовый CI trio (`fmt`, `clippy`, `test --all`).

## Done Block

```text
$ git diff --name-status 95d2422..f0205be
A	crates/gateway/tests/red_m85_frames_book_continuity.rs
A	milestones/M-85-frames-book-continuity.md
A	scripts/verify_M-85.sh
exit=0

$ cargo test -p gateway --test red_depth_provenance_by_reach  # at 95d2422
running 9 tests
.........
test result: ok. 9 passed; 0 failed
exit=0

$ cargo test -p gateway --test red_m85_frames_book_continuity  # at f0205be
running 3 tests
test m85_2_anchored_window_already_agrees_and_must_keep_agreeing ... ok
test m85_1_client_assembled_series_equals_full_replay_on_delta_tail ... FAILED
test m85_3_setup_guard_tail_frame_is_delta_only ... ok
test result: FAILED. 2 passed; 1 failed
exit=101

$ bash -n scripts/verify_M-85.sh
exit=0

$ cargo test -p gateway --test red_m77_frame_book_continuity --quiet
running 6 tests
......
test result: ok. 6 passed; 0 failed

$ cargo test -p gateway --test red_m77_pump_cost --quiet
running 1 test
.
test result: ok. 1 passed; 0 failed
exit=0

$ bash scripts/verify_M-85.sh
PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
# cargo test --all --quiet did not reach a terminal result after four minutes; interrupted by critic
exit=130 (operator interruption; not interpreted as a gate result)

$ MUTANT: reducer.seed_vwap(&event) -> reducer.apply(&event)
$ cargo test -p gateway --test red_m85_frames_book_continuity
test result: ok. 3 passed; 0 failed
exit=0

$ cargo test -p gateway --test red_gateway_live_eq_replay
running 6 tests
test mid_stream_snapshot_completeness_merges_same_bucket ... FAILED
test result: FAILED. 5 passed; 1 failed
exit=101

$ rg -n '3sexies|в-0' milestones/M-70-depth-bands-enablement.md
exit=1

$ git ls-tree -r --name-only f0205be | rg 'A-028-m74-pre-dispatch-completeness'
exit=1
$ git merge-base --is-ancestor dcff9aa f0205be
exit=1

$ bash scripts/next_artifact_id.sh C
C-223
exit=0
```

## Required next action

Architect revises and recommits the M-85 artifact set. The next critic round audits the
new committed head; it does not accept a transcript-only measurement or a post-dispatch
oracle.
