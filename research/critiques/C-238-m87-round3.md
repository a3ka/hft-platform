<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: 7e817365e9e1bb4e0ef615fa98da353b5797cf6c
verdict: REJECT
-->

# C-238 — M-87 serving circuit breaker, round 3: REJECT

## Scope

This is the post-arbitration round required by `A-037` §6.  It judges only
execution of A-037 D-1…D-4 and У-1…У-9 on
`feat/M-87-serving-circuit-breaker` at `7e817365e9e1bb4e0ef615fa98da353b5797cf6c`.
The transport-versus-library boundary is not reopened.

The committed artifact set is present: M-87, `red_m87_admission.rs`,
`red_m87_entrypoint.rs`, the amended `red_ws_protocol.rs`, and
`scripts/verify_M-87.sh`.  `contracts/**` is untouched.  `gateway-serve` and
`gateway` have no dedicated FA; this audit uses `VB-I-10`, `VB-I-11`,
`OPS-I-8`, `OPS-I-10`, and `DESIGN` `PL-I-4/5/8`.

**Verdict: REJECT.**  Dev is not dispatched.  The two blocking RED-oracles
specified by the arbitration are still placebo-capable: the entrypoint trap is
never exercised, and C4 permits the very success outcome that it must forbid.
The D-4 replacement also fails to require the named, live-connection outcome
ordered by A-037.

## Findings

### R3-1 — У-1 is not implemented: the entrypoint trap has no call site or guard

`red_m87_entrypoint.rs` defines `poison_segment` at line 151, but has no call
site.  The required `u1_guard_trap_actually_traps` does not exist.  Hence none
of the four checkpoint states is exercised at the WS boundary over the corrupt
segment, `checkpoint::advance` is never made to prove that the fixture itself
fails, the warm-path-over-head-trap case is absent, and there is no real-serving
positive control that a non-empty tail increments
`ServingCounters::journal_payload_bytes_read`.

The four `readiness()` unit states were moved out of the deleted library test
(У-2), and the fresh-checkpoint anti-placebo is present.  That is not a
substitute for У-1: A-037 requires the same four states again at the actual WS
entrypoint, with a fail-fast reader trap.  The existing C1 only uses an intact
journal and a self-reported counter, so a pre-admission read plus a suppressed
counter remains falsely green.

**Condition for return:** commit the required four WS-over-trap cases and all
three companions from A-037 У-1: the named trap guard, warm serving over a
head trap, and the real-serving nonzero counter control on a tail beyond a
snapshot.  Each cold case must assert its named serving outcome rather than a
transport failure.

### R3-2 — У-6 C4 remains nondeterministic and expressly accepts `snapshot`

`c4_slot_is_held_until_work_ends_not_until_response_timeout` uses a large
journal as an implicit delay; it has no test-controlled latch.  It only checks
`slots_in_flight >= 1`, then accepts either `overloaded` **or** `snapshot`:

```rust
matches!(code, "overloaded") || mb.get("type").and_then(|t| t.as_str()) == Some("snapshot")
```

This is the exact alternative A-037 У-6 prohibits.  A fast first request, a
timeout-released slot, or two parallel computations can yield `snapshot` and
pass.  The test also does not assert `slots_in_flight == 1` while work is held,
nor that it is released after the test-controlled completion.

**Condition for return:** C4 must hold the first work observably until the
test releases it; during that hold the second request must be `overloaded` and
the observed count must be exactly one.  It must then demonstrate release.

### R3-3 — D-4 option (i) changed prose/name but does not test its required outcome

The FA clarification and renamed `o5_broken_checkpoint_is_named_outcome_not_silent_rebuild`
exist, so the selected D-4 option is visible.  Its only assertion is `assert!(!ok)`
where `ok` means “a snapshot was received.”  This passes if the server closes
the connection, times out, emits an unrelated error, or fails JSON decoding.
It does not assert `not_ready`/another named readiness outcome and does not
show that the connection remains live, both required by A-037 D-4.

**Condition for return:** make the WS oracle distinguish the named readiness
outcome from closure/timeout/unrelated failure and prove that the connection
remains usable after the broken-checkpoint response.  The worker-path cache
oracle remains the place for legal rebuild semantics.

## Checks that did satisfy the arbitration

| Requirement | Result | Evidence |
|---|---|---|
| D-1(a) | PASS | Base range changes `crates/gateway/tests/**` only by `A red_m87_*`; a committed mutation of existing `red_frames_seek_bound.rs` is caught. |
| D-1(b) | PASS | `cargo test -p gateway` aggregated `passed=223 failed=0`, including `resume_without_checkpoint_reports_full_replay`. |
| У-2 | PARTIAL | The deleted library oracle is absent and four `readiness()` states plus `Ready` anti-placebo exist; the WS half is blocked by R3-1. |
| У-3–У-5 | Present in RED form | `bind_with_policy`, six-field counters, and event/payload/cancel/slot oracles are in the committed RED set. |
| У-7 | PASS | C9 appends after checkpoint creation and asserts strict `snapshot_ms < source_ms`. |
| У-8 | PASS | Both required adversarial mutations make their corresponding verifier steps fail. |
| У-9 | PASS | Milestone removes the old library file, records A-037, and documents the trap and D-4 disposition. |

## Done Block

```text
$ git rev-parse HEAD; git merge-base origin/main HEAD
7e817365e9e1bb4e0ef615fa98da353b5797cf6c
d5163b5b35abbca204a8981e974bd6e5a97eb9de
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m87-round3-target bash scripts/verify_M-87.sh
FAIL  task1: нет модуля admission.rs с пятью исходами ServingOutcome
PASS  task2: библиотечный оракул изъят; готовность судится в транспорте
FAIL  task1+3+4+7: red_m87_admission КРАСЕН — компиляция
FAIL  task2+3+5+6+8: red_m87_entrypoint КРАСЕН — компиляция
FAIL  task4: в ReadStats нет payload_bytes_read — бюджет по байтам мерить нечем
FAIL  task5: ограничителя параллелизма нет ни в одном файле crates/gateway-serve/src (найдено: 0)
FAIL  task6: нет metrics.rs с раздельными счётчиками отказов
FAIL  task7: key_material отсутствует либо зонд её не зовёт — трактовок по-прежнему две (TD-207)
FAIL  task8: свежести нет ни в одном файле выдачи (совпадений: 0)
FAIL  task9: нет пары лимитов уровня сервиса у: gateway-serve(0) recorder(0) gateway-checkpoint(0)
SKIP  task9: ФАКТИЧЕСКИ применённые лимиты, запас для recorder'а и поведение НА лимите снимаются на проде (docker inspect)
PASS  task10: решение с токеном записано по каждому из 17 файлов и по двум тестам D-4
PASS  D-1(а): библиотечный корпус тронут только добавлениями red_m87_*
PASS  D-1(б): библиотечный корпус зелен — passed=223 failed=0
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 11)
exit=1

$ [isolated committed mutation] existing crates/gateway/tests/red_frames_seek_bound.rs
$ D-1(a) predicate
D-1(a) guard result: FAIL as expected
M	crates/gateway/tests/red_frames_seek_bound.rs
exit=0  # mutation harness: guard caught the forbidden modification

$ [isolated M-9a] add cpus/memory only beneath labels: for all three services
gateway-serve=0
recorder=0
gateway-checkpoint=0
task9 mutation result: FAIL as expected; missing: gateway-serve(0) recorder(0) gateway-checkpoint(0)
exit=0  # mutation harness: guard caught the false-green attempt

$ [isolated M-10b] retain smoke_ws.rs row and empty its decision cell
task10 mutation result: FAIL as expected; missing: smoke_ws.rs
exit=0  # mutation harness: guard caught the false-green attempt

$ rg -n 'poison_segment\(' crates/gateway-serve/tests/red_m87_entrypoint.rs
151:fn poison_segment(dir: &std::path::Path, index: usize) {
$ rg -n 'u1_guard_trap_actually_traps|trap_actually_traps' crates/gateway-serve/tests
exit=1  # no required guard exists

$ bash scripts/next_artifact_id.sh C
C-238
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-21T00:00Z
- Milestone: M-87-serving-circuit-breaker
- Статус: BLOCKED
- HEAD: 7e81736 — docs(roadmap): SCALE — спор по M-87 разрешён A-037, решение исполнено [architect]

## §B — Что я сделал
- Проверил только исполнение A-037 D-1…D-4 и У-1…У-9 на фактической вершине ветки.
- Воспроизвёл baseline, D-1(a), M-9a и M-10b; проверил D-4 и RED-оракулы по коду.

## §C — Артефакты / результаты
- `research/critiques/C-238-m87-round3.md`
- Done Block: baseline verify exit=1 (ожидаемый RED, 11 fail); все три mutation-harness exit=0, потому что соответствующий guard поймал мутацию.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь только R3-1, R3-2 и R3-3 из research/critiques/C-238-m87-round3.md на feat/M-87-serving-circuit-breaker. Не переоткрывай границу A-037. Нужны: WS-ловушка для всех четырёх состояний с u1_guard_trap_actually_traps, тёплым путём и позитивным serving-counter контролем; детерминированный C4 с защёлкой, exact overloaded/slots=1/release; D-4 o5 с названным исходом и живым соединением. После commit+push запроси следующий critic-круг.
  ```
- Push-статус: pending — this verdict must be committed and pushed to `origin/feat/M-87-serving-circuit-breaker`.
- Кэш: pending removal after commit/push.

## §E — Риски / открытые вопросы
- BLOCKED by R3-1/R3-2/R3-3; no new arbitration trigger: these are execution defects of A-037, not a reopened boundary dispute.

=== END HANDOFF ===
