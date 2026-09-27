<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: dd581a7ada9d120c58d408c08369d66481f59632
verdict: ESCALATE
-->

# C-236 — M-87 serving circuit breaker, round 2: ESCALATE

## Decision

**ESCALATE.** M-87 must not proceed to implementation.

The committed set is present (T-contracts in the milestone, trait/API intent,
RED tests, verifier, and milestone file), but it does not close C-234 R1–R4.
Most importantly, R2 repeats the same boundary failure as C-234: a direct
`LiveReducer::resume` test requires a semantic change in a shared library while
the milestone forbids that change.  A-033 D-2 made that class of product policy
guard invalid.  Under `gates.md` §0 and the critic role, a second REJECT for the
same cause is routed to an arbiter rather than a third critic loop.

The arbiter must decide the serving-admission boundary and the status of the
shared-library regression before an architect supplies a new artifact chain.

## Audit subject

| field | value |
|---|---|
| subject branch | `feat/M-87-serving-circuit-breaker` |
| audited base | `d5163b5b35abbca204a8981e974bd6e5a97eb9de` (`origin/main`) |
| audited head | `dd581a7ada9d120c58d408c08369d66481f59632` |
| compared range | `d5163b5..dd581a7` |

FA-WAIVER: `crates/gateway-serve` has no dedicated FA (the known reading-map
gap).  This audit therefore uses `docs/fa/viz-backend.md` §5 (VB-I-9/10/11),
OPS-I-8 and OPS-I-10, and DESIGN PL-I-4/5/8 as the live-invariant basis.

## Findings

### E1 — R1 remains open: the declared contract cannot yet be executed end to end

The new unit test has an unbounded-profile oracle, but the real-server oracle
does not exercise the declared post-`session::validate_selector` admission
point: it sends `timeframe_ms: 1`, which the existing selector validator rejects
as `invalid_selector` before that point.  The asserted `unsupported` result
therefore requires moving admission before validation or altering validation,
contrary to the documented boundary and CT-RFC-09 §2.7.

`CallBudget`, `BudgetStop`, `Cancel`, and `ServingSlots` occur in the milestone,
but are absent from the M-87 RED tests and verifier.  `ServeConfig` also has no
policy/budget/slot injection path, while the entrypoint test constructs a
`policy()` it never passes to the real server.  A hard-coded product default
would satisfy the present test.

The proposed `ServingCounters` contract includes
`journal_payload_bytes_read` and `slots_in_flight`, but the new admission test
constructs literals with only the four older fields.  Implementing the stated
public struct will make those test literals fail to compile.  Thus the artifact
set neither pins the complete API nor makes budget exhaustion, cancellation,
and slot release observable.

**Condition for return:** after the arbiter fixes the boundary, submit a
committed RED set with a selector that is valid generally but disallowed by the
limited serving profile, plus independently observable outcomes for every
declared budget/cancel/slot state.  The configuration exercised by the real
server must carry that policy rather than leave it implicit.

### E2 — R2 repeats C-234 and conflicts with A-033

`crates/gateway/tests/red_m87_cold_path_reads_nothing.rs` still invokes
`LiveReducer::resume` directly and requires missing, corrupt, and incompatible
checkpoint cases to decode zero events.  Section 10 simultaneously forbids
changing `LiveReducer::resume`/its pump.  The test therefore either stays red,
or is made green by the very common-library semantic guard that A-033 rejected.
It is not a public-serving entrypoint oracle.

The new WS test is a useful real carrier, but its cold assertion only compares
the public counter before and after the response.  It has no fail-fast reader
trap or admission-placement mutation: a developer can read the journal before
admission and omit or delay the counter increment.  It consequently does not
prove that admission lies between `session::validate_selector` and
`spawn_blocking`, nor that no journal reader was reached.

This is the repeated cause requiring arbitration.  The six Section 10
prohibitions name several forbidden repairs, but do not resolve the contradiction
created by the direct shared-library test; a prohibition list is not a
mutation-resistant WS boundary oracle.

### E3 — R3 remains open: C4, C6, and C9 do not prove their claimed behavior

* C4 starts two requests but does not control work duration or require the
  second request to be refused; it accepts either `overloaded` *or* a snapshot.
  It cannot establish a one-slot concurrency bound or release on all exits.
* C6 observes the in-process `serving_counters()` getter, not a runtime producer
  emission of a sample series.  OPS-I-10 requires running the named producer and
  asserting emitted `name{labels} value`; registry/getter parity is insufficient.
* C9 sends a fresh checkpoint and only checks four positive fields.  It neither
  constructs a stale checkpoint nor distinguishes the four dimensions, so one
  shared constant could satisfy it and stale serving behavior is unproved.

### E4 — R4 verifier output matches the expected red count, but its two passes are not earned

`bash scripts/verify_M-87.sh` produced twelve failures and exit 1, with only
task10 and formatting passing.  The presented task10 mutation is reproducible:
removing `smoke_ws.rs` from the Section 16.1 decision list makes task10 fail.
The analogous task9 mutation (remove the `gateway-checkpoint` memory-limit line
from an otherwise complete fixture) also fails.  These show field/name presence,
not acceptance semantics.

Task10 is currently a false green.  It only checks that all 17 filenames occur
in Section 16.1.  Two classifications are substantively wrong:

* `red_frames_seek_bound.rs` already calls `LiveReducer::resume` directly to
  verify full replay without a checkpoint.  It is a worker/shared-library test,
  not a public-serving path to “move to worker”; M-87’s stated entrypoint-only
  scope leaves it untouched.
* `red_ws_protocol.rs` tests the real WS behavior for a broken checkpoint that
  currently falls back rather than failing.  Labelling it merely a fixture to
  warm changes a behavior decision, not test setup.  It needs an explicit,
  separately adjudicated disposition.

Task9 only parses resource-limit text and explicitly skips runtime inspection
and headroom/limit behavior.  It cannot establish its task’s operational claim.

## Evidence retained in the gate run

The following is the raw material result, with exit status, from the audited
head:

```text
$ git fetch origin --quiet && git rev-parse origin/feat/M-87-serving-circuit-breaker
dd581a7ada9d120c58d408c08369d66481f59632
exit=0

$ bash scripts/next_artifact_id.sh C
C-236
exit=0

$ bash scripts/verify_M-87.sh; echo exit=$?
FAIL task1
FAIL task2 red M87: 3 passed 4 failed
FAIL admission compilation unresolved modules/ReadStats
FAIL entrypoint compilation unresolved modules
FAIL task4
FAIL task5
FAIL task6
FAIL task7
FAIL task8
FAIL task9 svc 0s
SKIP task9 applied etc
PASS task10 17
PASS fmt
FAIL clippy
FAIL test all
VERDICT FAIL (12)
exit=1

$ [in-memory task10 mutation: remove smoke_ws.rs decision]
corpus=17 missing=smoke_ws.rs
FAIL task10: missing §16.1 decision for smoke_ws.rs
exit=1

$ [in-memory task9 mutation: remove gateway-checkpoint mem_limit]
control: PASS task9 resource limits
mutation: FAIL task9: gateway-checkpoint(1)
control_exit=0 mutation_exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ cargo test -p gateway --test red_frames_seek_bound resume_without_checkpoint_reports_full_replay -- --exact
test resume_without_checkpoint_reports_full_replay ... ok
exit=0
```

## Done Block

- [x] Fetched the subject branch and audited its tip, not a prompt SHA.
- [x] Audited milestone, T-contracts/API intent, RED tests, verifier, and the
      17-file Section 16.1 corpus disposition.
- [x] Reproduced the required verifier baseline and both requested mutation
      classes without writing to the subject artifact set.
- [x] Allocated this artifact with `scripts/next_artifact_id.sh C`.
- [x] Routed the repeated R2 cause to an arbiter as required by the gate.
