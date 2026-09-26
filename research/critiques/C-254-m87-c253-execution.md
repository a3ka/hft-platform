<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: cd36ff235f17338baf93a6593b3c0a306a7d0f31
audited_head: b82e49d6a22ea5ed816a22ced47db2df3a408d62
verdict: NOTE
-->

# C-254 — M-87 round 9: C-253 execution and the COMPILE-RED boundary

## Verdict: NOTE

This round audits only removal of `C-253` R8-1 on `b82e49d`. It does not
reopen the parser class decided by `A-040`: a form outside the explicit
direct-double-quoted-literal contract is an honest guard refusal, not a new
parser round.

`C-253` R8-1 itself is closed. In the real `c1` body, replacing the driver
argument with a foreign registry name held in a variable and placing the
scenario's own literal below the call makes `r7` red, with the required
"first argument is NOT a direct literal" diagnosis. A direct literal control
remains green. Comments after `(`, `concat!`, runtime concatenation, and a
raw string are all refused fail-closed; they do not create a false green.

## R9-1 — r7 cannot prove a call reached through a function alias (NOTE)

`r7` only searches the body for the spellings `serve_for(`,
`ckpt_from_registry(`, and `registry_tail(`. The required setup check is a
global `calls >= 3`, not a per-scenario proof. I changed `c1` to:

```rust
let driver = serve_for;
let (addr, _ckpt_guard) = driver(
    "u1_warm_path_is_served_over_trapped_head",
    dir.path(),
).await;
```

The c1 fixture therefore comes from a registered *neighbour*, yet the exact
`r7` and the entire nine-test registry suite both stayed green. The source
changed (`3 2`); it was restored and the baseline suite returned green. This
is a real limitation of the present static claim, not an A-040 parser-class
reopening: the direct-call guard never observes this call at all.

The appropriate ninth-round decision is to stop strengthening `r7`. Adding
one more spelling blacklist (for example `let driver = serve_for`) would
repeat the non-convergent source-parser cycle that `A-040` expressly ended.
The COMPILE-RED interval is unsuitable for proving this semantic property.
The limit must remain explicit: before task 12, `r7` is only a guard for
spelled direct calls; it is not proof that every scenario uses its own
registry row. After task 12, the mandated harness execution/reclassification
must prove the scenario-to-fixture connection dynamically. This NOTE ships;
it does not request an r7 round 10.

The relevant live invariant remains **GW-I-9(б)** in
`docs/fa/viz-backend.md`: an unsuitable checkpoint on the serving path has a
named readiness outcome instead of a silent full rebuild. The registry is
supporting evidence for the fixtures around that invariant; until the harness
can execute the COMPILE-RED subject, source inspection cannot establish the
fixture provenance completely.

## A-040 Q4 sufficient set

| Required proof | Result on audited head |
|---|---|
| P1 `#[quickcheck]` | `r0b` red, exit 101; file changed `1 0` |
| P2 attribute and `fn` on one line | `r0b` red, exit 101; file changed `1 2` |
| P3 `macro_rules!` | `r0b` red, exit 101; file changed `5 0` |
| P4/P5 | `r6b` green over positive, multiline-negative, nested-negative, and one-line-negative fixtures; exit 0 |
| A-039 mutations | delete c9 (`0 7`), c9 `200 -> 2_000` (`1 1`), threshold `1_000 -> 150` (`1 1`), anchor `100 -> 300` (`1 1`): each red, exit 101; restored suite green |
| A-040 harness bijection | step is present in `verify_M-87.sh`; it is red on the declared COMPILE-RED state, exit 101 (`AdmissionPolicy` fields and `slots_handle` absent) |

The committed R8 artifact set is otherwise scoped: `cd36ff2..b82e49d`
changes only `crates/gateway-serve/tests/red_m87_registry.rs` and
`milestones/M-87-serving-circuit-breaker.md`; it adds no T-contract or trait
signature change. The RED test, verify step, and milestone task-12 handoff
remain present.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-87-serving-circuit-breaker
b82e49d6a22ea5ed816a22ced47db2df3a408d62
exit=0

$ bash scripts/next_artifact_id.sh C
C-254
exit=0

$ cargo test -p gateway-serve --test red_m87_registry r7_driver_argument_is_a_literal_equal_to_scenario_name -- --exact
R8-1 variable foreign argument + later same-name decoy: FAILED
"первый аргумент serve_for( НЕ прямой литерал"
exit=101 (file changed 3 5; restored)

$ cargo test -p gateway-serve --test red_m87_registry r7_driver_argument_is_a_literal_equal_to_scenario_name -- --exact
direct literal control: 1 passed; 0 failed
exit=0 (file changed 1 1; restored)

$ cargo test -p gateway-serve --test red_m87_registry r7_driver_argument_is_a_literal_equal_to_scenario_name -- --exact
comment after (: FAILED; concat!: FAILED; runtime concatenation: FAILED; raw string: FAILED
each exit=101 (each source mutation observed and restored)

$ cargo test -p gateway-serve --test red_m87_registry r7_driver_argument_is_a_literal_equal_to_scenario_name -- --exact
function alias + foreign literal: 1 passed; 0 failed
exit=0 (file changed 3 2; unexpected; restored)

$ cargo test -p gateway-serve --test red_m87_registry
P1/P2/P3: each r0b FAILED, exit=101 (file changes 1 0 / 1 2 / 5 0)
r6b: 1 passed; 0 failed; exit=0
A-039 mutations: delete c9 / c9 200->2_000 / threshold 1_000->150 / anchor 100->300
each registry suite FAILED, exit=101 (file changes 0 7 / 1 1 / 1 1 / 1 1)
after every restoration: 9 passed; 0 failed
exit=0

$ cargo test -p gateway-serve --features testing --test red_m87_entrypoint -- --list
error[E0599]: no method named `slots_handle`
error[E0560]: AdmissionPolicy has no field `max_tail_events`
error[E0560]: AdmissionPolicy has no field `expected_warmup_events`
exit=101

$ bash scripts/verify_M-87.sh
PASS  A-039: реестр общего порога — test result: ok. 9 passed; 0 failed
FAIL  A-040: перечень харнесса недоступен — предмет не собирается (ожидаемо до задач 12-13)
VERDICT: FAIL (провалов: 10)
exit=1

$ git diff --exit-code -- crates/gateway-serve/tests/red_m87_entrypoint.rs crates/gateway-serve/tests/m87_registry/mod.rs
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-23T19:23Z
- Milestone: M-87-serving-circuit-breaker, round 9 (C-253 R8-1 only)
- Статус: DONE — NOTE
- HEAD: b82e49d — test(M-87): C-253 — страж связи читает АРГУМЕНТ, а не текст рядом [architect]

## §B — Что я сделал
- Reproduced the requested variable-plus-decoy bypass and confirmed r7 now rejects it.
- Probed direct-literal false positives, A-040 Q4 controls, and the function-alias bypass.
- Decided the static mechanism has reached its declared COMPILE-RED limit; no further r7 hardening is requested.

## §C — Артефакты / результаты
- `research/critiques/C-254-m87-c253-execution.md`
- Done Block above: R8-1 red; Q4 controls hold; alias-with-foreign-row stays green and is recorded as the explicit boundary.

## §D — Следующий агент + инвокация
- **Следующий агент:** `engine-dev`
- **Paste-ready промпт:**
  ```
  Продолжи M-87 по назначенной задаче, не меняя r7 ради C-254: C-254 NOTE фиксирует
  предел статического стража на COMPILE-RED. Для задачи 12 реализуй policy/slots и затем
  исполни обязательство A-040: биекция harness --list зелёная, а переклассификация c1
  роняет сценарий по фикстуре. Докажи это RED/verify, закоммить и запушь GREEN-коммит в
  feat/M-87-serving-circuit-breaker.
  ```
- Push-статус: ✅ C-254 verdict committed and pushed to `origin/feat/M-87-serving-circuit-breaker` at `308560d`
- ✅ кэш убран — `/tmp/hft-critic-m87-r9/target` очищен после push (`cargo clean`; прямая `rm -rf` была отклонена средой безопасности)

## §E — Риски / открытые вопросы
- До задачи 12 сценарий, вызывающий driver через alias, не проверяется r7; это известный,
  явно ограниченный COMPILE-RED предел, не доказательство полноты.

=== END HANDOFF ===
