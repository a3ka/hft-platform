<!-- GATE-META
milestone: M-72
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: 2a701eb1b5322bbf33e2f6c1cb257d2deb8655f3
verdict: REJECT
-->

# C-190 — M-72 subscription terminality, round 1 — REJECT

## Scope and decision

**REJECT. Do not dispatch engine-dev.** This is an audit of committed artifacts at
`2a701eb1b5322bbf33e2f6c1cb257d2deb8655f3`, not of the handoff prose. Remote
`feat/M-72-subscription-terminality` resolved to that SHA immediately before the
verdict. The requested graph comparison is `origin/main..HEAD`; its merge-base is
`d1221b1ca932d0b8e95403c2849308ed6e7b9ce2`. `origin/main` itself is
`d77398d7b22396c452d2651e90498033186055dd` and is 167 commits ahead of the subject.

The committed set is present: M-72, entrypoint RED, TD-179 RED, TD-180 RED, and
`scripts/verify_M-72.sh`. T1/contracts and new production traits are deliberately
out of scope. That is not sufficient under A-028 §1: every named behavioural
oracle must be an executable committed artifact (a genuine COMPILE-RED is allowed),
and the task-5 wire contract has no such oracle.

### Critic decision for task 5

Choose **(b): one terminal-subscription code with a mandatory, distinct reason
field**, rather than reusing `invalid_selector` without a reason. The code must be
the truthful generic `subscription_terminated`; `reason` must be an enum-like wire
value including at least `response_limit_exceeded` and `pump_failed`. Thus neither
terminal condition masquerades as invalid client input, while the client can make
the required distinction.

`docs/rfc/CT-RFC-09-ws-session.md` must add this v1 error envelope/taxonomy and its
meaning. It must also be pinned by a committed entrypoint RED which injects the
non-cap midstream failure and asserts `type`, `sub`, `code`, `reason`, termination,
and no subsequent frame. The existing E-2 only asserts `type == "error"` and `sub`;
it cannot decide the form.

**No `GATEWAY_SCHEMA_VERSION` bump follows from this choice.** Measurement shows that
this constant versions `gateway::Snapshot` and `gateway::Frame`, whereas the v1
error envelope is built independently by `wire_v1::error_msg(sub, code, message)`.
M-68 already changed the semantic state/Frame version 8→9; `origin/main` is 9.
Changing an error code/adding an additive v1 error field does not alter state or
Frame semantics. A stale branch changing its 8 straight to 10 would both collide
with M-68's 8→9 line and claim an unrelated schema invalidation. The RFC is required;
the state-schema bump is not.

## Blocking findings

### B-1 — TD-177 names a new seam although the committed deterministic seam already exists

`task 2` declares a five-function `gateway_serve::test_seam` and calls it under
`feature = "testing"`. It does not exist, so its COMPILE-RED is real:
`error[E0432]: unresolved import gateway_serve::test_seam`.

But this is not an unimplemented deterministic contract that dev must invent.
`crates/gateway-serve/src/test_sync.rs:48-172` already exports the testing-only,
production-wired rendezvous contract:

```text
arm(id: &str)
pump_signal_and_wait(id: &str)
test_wait_for_pump(id: &str, timeout: Duration) -> bool
test_release(id: &str)
test_remove(id: &str)
```

It is called at both v1 and legacy pump sites (`lib.rs:1228`, `:1693`) and is already
used by the committed O-12 test. The milestone's proof that there are zero
`cfg(feature = "testing")` strings is an ineffective textual probe: the actual
guard is `cfg(any(test, feature = "testing"))`. Cargo declares that feature exactly
to expose this seam.

Architect must replace the invented signature with the existing committed contract
and make task 2 executable RED under it. Two hooks are sufficient *only* with the
existing ordering: arm before the old subscription, wait for the old pump's entered
signal, switch, then release; this is deterministic and scheduler-independent. A
second, unconnected seam would leave the named test as a compile failure without
proving the production pump stopped at its hold point.

### B-2 — TD-177's claimed proof that the new subscription is live has no fresh event

After switching to ETH, the test accepts an ETH snapshot, releases the stale BTC
pump, and waits for an ETH `frame`. It appended 20 ETH events **before** the switch
(`red_ws_terminality_entrypoint.rs:480`), but appends none after it (`:499-550`). A
frame is caused by journal growth after the subscription; a snapshot is not proof
of future pumping. Therefore a correct implementation can time out exactly where
the test calls the new subscription dead. Conversely, a queued unrelated frame does
not identify a post-switch delivery.

Architect must add a fresh, identifiable ETH append after switch/release and assert
the corresponding frame while retaining the no-stale-error half and deterministic
cleanup. Existing O-12 demonstrates this necessary shape by writing fresh events
after its switch. This is R-1/R-2 oracle completeness, not an engine-dev choice.

### B-3 — task 5 has no committed oracle for the notification it delegates to this gate

Both current cap-terminal sites emit `invalid_selector` (`gateway-serve/src/lib.rs:1401-1415`
and `:1806-1815`). CT-RFC-09 defines that code for invalid selector input. TD-179's
RED calls `LiveReducer` directly; it cannot observe a wire error. E-2 observes only
an error and subscription id. No committed oracle names or asserts the reason for a
non-cap midstream failure, so task 5 cannot be dispatched under A-028 §1.

The decision above is the contract impact for the next architect round. It requires
the RFC change and its RED before dev is assigned task 5. A green TD-179 reducer test
alone is not client notification.

### B-4 — task 8 is impossible for its assigned developer

Task 8 assigns only `crates/gateway/src/lib.rs` to engine-dev but accepts only when
`git grep -c 'scripts/verify_M-71' crates/ deploy/` is zero. The committed range has
two carriers:

```text
crates/gateway-serve/tests/red_ws_terminality_entrypoint.rs:411
crates/gateway/src/lib.rs:1964
```

The first is the architect's sacred test file, outside the developer's allowed
write zone. The handoff promise to fix it later is not a committed artifact. Before
dispatch, architect must remove/reword that own carrier or make it an explicit
completed prerequisite; only then can the task-8 acceptance be reachable. This is
the TD-138 class: a live cross-zone reference makes a declared mechanism untrue.

## Checked non-blockers and declared debt

* `cargo test --all` fails exactly at the declared TD-179 M-2 oracle; its positive
  M-1 control passes. No second `cargo test --all` failure was observed. TD-180 is
  independently, intentionally RED (`None` vs `Some(24999)`), and E-1/E-2 pass
  without the testing feature.
* Task 6 does not purchase its state-position snapshot rule by weakening TD-179:
  TD-180 reports `snapshot cursor None` versus state `24999`, while TD-179 separately
  reports delivery bookmark `Some(767)` after a failed pump. The surfaces are distinct
  (state position vs delivered cursor), as the independent RED executions show.
* `crates/gateway` and `crates/gateway-serve` have no dedicated FA documents. This is
  declared debt, not a waiver by silence: `docs/fa/viz-backend.md` supplies VB-I-2
  (live equals replay) and the gateway-serve boundary, while DESIGN §22 reports the
  inverse drift `GW-I: declared 0 / in oracles 13`.

FA-WAIVER: crates/gateway — dedicated FA absent; audit uses viz-backend VB-I-2 and
DESIGN §22 GW-I inverse drift (declared 0, oracle 13).

FA-WAIVER: crates/gateway-serve — dedicated FA absent; audit uses viz-backend
gateway-serve boundary and DESIGN §22 GW-I inverse drift (declared 0, oracle 13).

`check_review_fa.sh` did not establish an FA: it returned `SKIP` because the range
touches only test paths. Its precise output is retained below.

## Done Block

```text
$ git ls-remote --heads origin feat/M-72-subscription-terminality
2a701eb1b5322bbf33e2f6c1cb257d2deb8655f3	refs/heads/feat/M-72-subscription-terminality
exit=0

$ git rev-parse origin/main
d77398d7b22396c452d2651e90498033186055dd
$ git rev-parse HEAD
2a701eb1b5322bbf33e2f6c1cb257d2deb8655f3
$ git merge-base origin/main HEAD
d1221b1ca932d0b8e95403c2849308ed6e7b9ce2
exit=0

$ cargo test -p gateway-serve --test red_ws_terminality_entrypoint --quiet
running 2 tests
..
test result: ok. 2 passed; 0 failed; finished in 23.54s
exit=0

$ cargo test -p gateway --test red_pump_midstream_failure --quiet
running 2 tests
td_179_m2_failed_pump_must_not_leave_cursor_ahead_of_delivered --- FAILED
.
test result: FAILED. 1 passed; 1 failed
left: Some(767)
right: None
exit=101

$ cargo test -p gateway --test red_snapshot_cursor_honesty --quiet
running 2 tests
. 1/2
td_180_s2_snapshot_declares_state_position_not_delivery_bookmark --- FAILED
test result: FAILED. 1 passed; 1 failed
left: None
right: Some(24999)
exit=101

$ cargo test -p gateway-serve --features testing --test red_ws_terminality_entrypoint td177_stale_pump_does_not_kill_new_sub
error[E0432]: unresolved import `gateway_serve::test_seam`
help: a similar name exists in the module: `test_sync`
exit=101

$ bash scripts/verify_M-72.sh
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet                 # only TD-179 M-2 failed
PASS: task #1 entrypoint oracle
FAIL: task #2 COMPILE-RED unresolved `gateway_serve::test_seam`
FAIL: task #3 two unsubscribe carriers lack generation check
FAIL: task #4 TD-179 RED
FAIL: task #5 task-4 suite not green
FAIL: task #6 TD-180 RED
PASS: task #7 egress-cap paths
FAIL: task #8 scripts/verify_M-71 lives in 2 files
PASS: P/S/M/C/G/H
VERDICT: FAIL (8)
exit=1

$ git grep -n 'scripts/verify_M-71' -- crates/ deploy/
crates/gateway-serve/tests/red_ws_terminality_entrypoint.rs:411:...scripts/verify_M-71.sh:37-40...
crates/gateway/src/lib.rs:1964:...scripts/verify_M-71.sh...
match-file-count=2
exit=0

$ bash scripts/check_review_fa.sh "$(git merge-base origin/main HEAD)" HEAD
SKIP (range touches only non-production crate paths — tests/examples/benches)
  crates/gateway-serve/tests/red_ws_terminality_entrypoint.rs
  crates/gateway/tests/red_egress_cap_paths.rs
  crates/gateway/tests/red_pump_midstream_failure.rs
  crates/gateway/tests/red_snapshot_cursor_honesty.rs
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS [2-COVERAGE] §22: GW-I — declared=0, in oracles=13
VERDICT: PASS (0 violations)
exit=0

$ bash scripts/next_artifact_id.sh C
C-190
exit=0
```

## Handoff

### 1. Status

REJECT, round 1. Engine-dev receives no M-72 dispatch yet.

### 2. Audited artifact and push target

Audited remote head: `2a701eb1b5322bbf33e2f6c1cb257d2deb8655f3` on
`feat/M-72-subscription-terminality`. This verdict must be committed and pushed to
that same subject branch.

### 3. Required architect work before the next critic round

Repair task 2 around the existing `test_sync::rendezvous` API; make its live-frame
half append and identify fresh post-switch ETH data; add the task-5 wire/RFC RED for
`subscription_terminated` plus reason; and remove the sacred task-8 grep carrier or
complete it as an architect prerequisite. Update milestone and verify to name those
committed artifacts.

### 4. Recheck commands

Run the three named RED commands, `bash scripts/verify_M-72.sh`, `cargo test --all`,
`bash scripts/check_review_fa.sh "$(git merge-base origin/main HEAD)" HEAD`, and
`bash scripts/verify_design_claims.sh --merge-preview origin/main`. The next critic
must audit the new committed SHA, not this plan.

### 5. Risks and limits

The branch is stale relative to main; a merge-preview remains mandatory before merge.
No dedicated FA exists for either touched crate, so the declared `GW-I` inverse-drift
debt remains visible rather than being mistaken for review coverage.
