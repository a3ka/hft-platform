<!-- GATE-META
milestone: M-85-frames-book-continuity
audited_repo: a3ka/hft-platform
audited_base: d35d25bf216faf86dd217492f3e24aa50166ade0
audited_head: 08c7fb48ae93b5fd5466d905b3d5c2c6c3f9440a
verdict: REJECT
-->

# C-224 — M-85 frames/book continuity, round 2 — REJECT

Дата: 2026-09-16 UTC
Роль: critic, plan-time gate after architect artifacts
Предмет: `feat/M-85-frames-book-continuity` @ `08c7fb4`, продолжение после `C-223` @ `d35d25b`.

## Verdict

**REJECT — dev не диспетчеризуется.** `C-223` B-1 и N-1 закрыты исполнимыми оракулами и их мутациями; T1 не затронут. Но `C-223` B-2 не закрыт для полного artifact set: RED-спека, на которой должен исполнять dev, всё ещё описывает `frames_since` через ранее отвергнутую seek-топологию `frames_since_with_stats`.

Это **второй REJECT подряд по той же причине B-2** — ложная модель фактических путей `frames_since`/seek-варианта. По `gates.md` §0 следующий шаг — независимый арбитр со свежим контекстом, не третий architect↔critic круг.

Живые инварианты предмета: **VB-I-2** (`docs/fa/viz-backend.md:199`) требует бит-идентичности live/replay; **VB-I-10** (`:207`) запрещает рост состояния вне границы окна. Оба открыты на audited head.

## REJECT finding

### B-2r — rev2 поправила milestone, но оставила ложную топологию в RED-спецификации

`crates/gateway/tests/red_m85_frames_book_continuity.rs:11-22` называет предметом
`frames_since`, но схема `:14-20` описывает seek-функцию
`frames_since_with_stats → journal::stream_from` и утверждает, что «этот путь» не
видит событий до `after`. `m85_1` фактически вызывает **`frames_since`** (`:212`),
а его failure diagnostic снова называет причиной `frames_since_with_stats` и seek
(`:275-281`). Это не исторический блок и не помеченная снятая редакция.

На той же ревизии код говорит обратное: `frames_since` открывает
`journal::stream` с головы (`crates/gateway/src/lib.rs:2966-2976`), тогда как
`frames_since_with_stats` — отдельный API, который открывает `journal::stream_from`
(`:3020-3039`). Общая реальная причина — свежий `Reducer::new` и ветка `seed_vwap`
в `reduce_event_stream` (`:2226-2260`), как правильно сказано в rev2 milestone
(`milestones/M-85-frames-book-continuity.md:40-55,163-190`).

RED-тест — спецификация до реализации, а не только исполняемый assert. Поэтому
оставшаяся ложная модель направляет dev к seek API, который M-85 объявляет вне
предмета, и противоречит закрытию B-2 в milestone. Требуется согласованный
code-facing RED artifact: описание и diagnostic должны называть фактический full-stream
путь и его общую reducer-ветку; seek API остаётся только явно обозначенным отдельным
ограничением.

### B-2r supplementary — guard отсутствия не покрывает собственное требование

`milestones/M-85-frames-book-continuity.md:203-206` утверждает, что у
`frames_since_with_stats` ровно один потребитель, `red_push_seek_bounded.rs`; фактически
его также вызывают `red_read_stats_passthrough.rs:146` и `red_egress_cap.rs:686`.
Это не доказывает production caller (скан `crates/*/src` нашёл только declaration), но
делает основание «один потребитель» ложным.

Также milestone обещает наблюдать отсутствие вызова в `crates/*/src/**` (`:207-208`),
а verify проверяет только `gateway-serve`, `recorder` и `ops`
(`scripts/verify_M-85.sh:125-127`). Такой guard не покрывает весь заявленный
production-source universe и не удовлетворяет собственной шапке гейта: предмет
наблюдения обязан совпасть с предметом требования (`:7-22`). Это та же незакрытая
B-2 граница, а не новый дизайн-предмет.

## Confirmed closures

- **C-223 B-1:** `red_m85_bootstrap_cost.rs` содержит `m85_4` и `m85_5`; verify
  запускает оба (`scripts/verify_M-85.sh:117-123`). На чистом head — 2/2 зелёные.
  Изолированный буферизующий мутант в общей reducer-ветке дал 184872 B (200 seed events)
  против 488456 B (800), ratio 2.642 > ceiling 2.0: `m85_4` красный, `m85_5` зелёный.
- **C-223 N-1:** замена `seed_vwap(&event)` на `apply(&event)` дала 3 passed / 1 failed
  в M-85 suite — падает `m85_6`; независимый `red_gateway_live_eq_replay` дал
  5 passed / 1 failed. Анти-плацебо не ослаблен.
- T2-шов `seed_book` объявлен в milestone; trait/T1 изменения не требуются. Диапазон не
  меняет `contracts/**`, журнал, venue или production implementation; Block-C и
  risk-critic не триггерятся. Allowed paths соответствуют architect/engine-dev split.
- Verify — реальный агрегирующий gate (`set -uo pipefail`, FAIL counter, terminal exit);
  его ожидаемый текущий результат — FAIL(3): CI `cargo test --all`, RED `m85_1`, и task #5.

## Done Block

```text
$ git diff --name-status 95d2422..08c7fb4
A	crates/gateway/tests/red_m85_bootstrap_cost.rs
A	crates/gateway/tests/red_m85_frames_book_continuity.rs
A	milestones/M-85-frames-book-continuity.md
A	research/critiques/C-223-M-85-frames-book-continuity.md
A	scripts/verify_M-85.sh
exit=0

$ cargo test -p gateway --test red_m85_frames_book_continuity --quiet
running 4 tests
m85_1_client_assembled_series_equals_full_replay_on_delta_tail --- FAILED
test result: FAILED. 3 passed; 1 failed
exit=101

$ cargo test -p gateway --test red_m85_bootstrap_cost --quiet
running 2 tests
..
test result: ok. 2 passed; 0 failed
exit=0

$ MUTANT: reducer.seed_vwap(&event) -> reducer.apply(&event)
$ cargo test -p gateway --test red_m85_frames_book_continuity --quiet
running 4 tests
m85_6_non_book_series_do_not_double_count_under_bootstrap --- FAILED
test result: FAILED. 3 passed; 1 failed
exit=101

$ cargo test -p gateway --test red_gateway_live_eq_replay --quiet  # same mutant
running 6 tests
mid_stream_snapshot_completeness_merges_same_bucket --- FAILED
test result: FAILED. 5 passed; 1 failed
exit=101

$ MUTANT: buffer every pre-after Event in reduce_event_stream
$ cargo test -p gateway --test red_m85_bootstrap_cost --quiet
running 2 tests
m85_4_bootstrap_cost_does_not_grow_with_history_length --- FAILED
  seed 200 -> 184872 B
  seed 800 -> 488456 B
  ratio 2.642, ceiling 2.0
test result: FAILED. 1 passed; 1 failed
exit=101

$ bash scripts/verify_M-85.sh; echo verify_exit=$?
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: oracle m85_1 (ran 1, exit=101)
PASS: m85_2 / m85_3 / m85_6 / m85_4+m85_5 / M-77 guards
PASS: seek-variant absence check (its currently narrow universe)
FAIL: task #5 stale frames_msgs live-push comment
VERDICT: FAIL (3)
verify_exit=1

$ rg -n "frames_since_with_stats" crates/gateway/tests/red_{push_seek_bounded,read_stats_passthrough,egress_cap}.rs
red_push_seek_bounded.rs:127,165
red_read_stats_passthrough.rs:146
red_egress_cap.rs:686
exit=0

$ bash scripts/next_artifact_id.sh C
C-224
exit=0
```

## Required next action

**Arbiter (fresh strong context).** Decide whether the remaining false RED-test
topology and narrower-than-declared absence guard leave B-2 open; if so, state the
minimal artifact correction and rerun condition. The arbiter must read `C-223`, this
verdict, rev2 milestone, RED test, verify script, and the cited gateway functions on
`08c7fb4`; it must verify claims in code rather than inheriting either side's framing.
