<!-- GATE-META
milestone: M-84
audited_repo: a3ka/hft-platform
audited_base: 7fb154a2b372709e24bdf98b15f1fe682e39395b
audited_head: 8453a2e11b3c0d8be6cc4e3c1f10cdfbd98402dc
verdict: REJECT
-->

# C-221 — M-84 fixed depth bands, round 2

## VERDICT: REJECT

The four C-220 gaps now have committed, executable successors: a parser-entrypoint suite,
an exact f32-lookalike case, a fourteen-row snapshot oracle, and a checkpoint-coexistence
oracle. `VB-I-12` is also correctly marked as specified but not yet implemented.

The artifact set is nevertheless not implementable safely. Two RED suites give mutually
incompatible requirements to the same `Selector` validation surfaces, and the resulting
legacy-checkpoint escape hatch is not pinned. Dev must not be dispatched.

`VB-I-12` is the live FA invariant named for this audit: server-owned fixed bands, no client
`bands`, an explicit `invalid_selector` rejection, and fourteen `(band, side)` rows. The
committed documentation correctly says its code is not present at this head.

## What was audited

- Subject branch: `feat/M-84-fixed-bands`.
- Range: `7fb154a2b372709e24bdf98b15f1fe682e39395b..8453a2e11b3c0d8be6cc4e3c1f10cdfbd98402dc`.
- Commits: `2573ebd` (C-220 follow-up RED/gate artifacts) and `8453a2e` (task 4 documentation).
- Artifact set: milestone; six RED suites; `scripts/verify_M-84.sh`; CT-RFC-09 and the
  `viz-backend` FA. No `crates/contracts/**` (T1) path is in the range.

## Blockers

### B-1 — the transport RED suites require opposite answers for the same normalized selector

**Where.** `red_fixed_bands_entrypoint.rs:50-60` always executes
`parse_message → parse_selector → session::validate_selector`. Its absent-field positive
control requires that path to return a selector with `CANONICAL_DEPTH_BANDS`
(`:93-102`). Conversely, `red_fixed_bands_wire.rs:69-77` passes the same canonical
`Selector` value directly to that same `session::validate_selector` and requires rejection.

`wire_v1::parse_selector` returns only `Result<Selector, SelectorError>`
(`crates/gateway-serve/src/wire_v1.rs:120-143`); `Selector` has no field carrying whether
the JSON key was client-supplied. Thus, once parsing produces the canonical selector for an
absent field, the session validator has no observable fact with which to distinguish it from
the canonical selector constructed by the wire test. A deterministic validator cannot return
both `Ok(())` and `Err(_)` for that value.

This is not theoretical: it blocks the correct design stated by CT-RFC-09 §2.2bis/§2.7 —
presence of the JSON `bands` key must be rejected by the parser, while the normalized
server-owned selector must be valid downstream.

**Condition to remove.** Put the client-field-presence decision at the parser boundary and
test it there (foreign, empty, and canonical-present all return `invalid_selector`; absent
constructs the canonical selector). Then make the session-validator test use the normalized
server selector and require acceptance. The RED set must express one contract, not source
provenance that the `Selector` type has already discarded.

### B-2 — the legacy-prewarm oracle forces an unguarded `advance_to` bypass

**Where.** `red_fixed_bands_canonical.rs:101-112` requires
`gateway::validate_selector(Selector { bands: vec![0.001], .. })` to reject. But
`red_fixed_bands_prewarm.rs:100-107` passes that exact legacy selector to
`gateway::checkpoint::advance_to` and requires success. The production function invokes
that same guard at `crates/gateway/src/lib.rs:3533-3541`.

**Reproduction.** In a detached mutation worktree at the audited head, I added an exact
canonical guard to `gateway::validate_selector`, then removed only the
`validate_selector(sel)?` call from `checkpoint::advance_to`. This is a distinct adversarial
implementation from C-220's parser-side canonicalization: it accepts arbitrary noncanonical
selectors through checkpoint prewarm while the direct library guard rejects them.

The committed RED suites did not detect that bypass:

```text
canonical rc=0
test result: ok. 9 passed; 0 failed
prewarm rc=0
test result: ok. 2 passed; 0 failed
fingerprint rc=0
test result: ok. 4 passed; 0 failed
frame rc=0
test result: ok. 3 passed; 0 failed
```

The bypass violates the milestone's stated reason for a crate-level guard: direct builders,
including the checkpointer, must not leave a bypass surface. It also makes the planned
prewarm path accept more than the one legacy checkpoint it claims to preserve.

**Condition to remove.** Specify and test a narrow, explicit legacy-prewarm transition rather
than requiring the generic public `advance_to` path to accept a foreign selector. The normal
checkpoint API must remain guarded; the transition must prove both that the known legacy
selector can be prepared and that a different foreign selector cannot enter through that
transition. Re-run the mutation above: removing the normal guard or widening the transition
must make an M-84 oracle fail.

## Checks that passed

- Scope is within the milestone's architect-owned artifact paths; `git diff --check` returned
  `0`.
- The intentional COMPILE-RED state is real: all six suites fail on the absent
  `CANONICAL_DEPTH_BANDS` symbol. `verify_M-84.sh` returns `1` with `VERDICT: FAIL (12)`, as
  declared for this pre-implementation revision.
- Task 4 is factually consistent with code: the current session validator still accepts
  non-empty valid bands (`crates/gateway-serve/src/session.rs:70-105`), matching the
  `VB-I-12` status “в коде ЕЩЁ НЕТ”.
- `M-68` (`7ed83f1`) is an ancestor of the audited head; `TD-159` remains OPEN/MAJOR and the
  FA still states that the recovery-time emission barrier does not exist. Task 6 remains
  blocked and the gate verifies the current `GATEWAY_BANDS:-0.001` default.

## gates.md §9 recheck — covered

- **(a) Code claims:** `bash scripts/verify_design_claims.sh --merge-preview origin/main`
  passed. The task-4 claim about the pre-M-84 validator was separately opened and confirmed
  at `crates/gateway-serve/src/session.rs:70-105`.
- **(b) Authority / boundary C:** the range neither changes `RETENTION_MODE` nor enables the
  seven production bands; the two named enablement prerequisites remain live. No founder-only
  action is inferred from this plan artifact.
- **(c) Linkage:** the merge-preview recheck passed its DESIGN/RFC references, SHA checks, and
  existing-path checks. `CT-RFC-09` §2.2bis, §2.7, §5 O-7 and FA `VB-I-12` are mutually linked.

## Required resubmission

Resolve B-1 and B-2 in the committed milestone, RED, and verify artifacts; keep the declared
COMPILE-RED cause singular. Submit the revised committed range for the next critic round. Do
not implement or dispatch dev on this revision.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-221
rc=0

$ git diff --check 7fb154a..HEAD
diff_check_rc=0

$ bash scripts/verify_M-84.sh
verify_rc=1
VERDICT: FAIL (12)

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ git merge-base --is-ancestor 7ed83f1 HEAD
m68_ancestor_rc=0

$ detached mutation: exact gateway guard + delete checkpoint::advance_to validation
canonical rc=0; 9 passed; 0 failed
prewarm rc=0; 2 passed; 0 failed
fingerprint rc=0; 4 passed; 0 failed
frame rc=0; 3 passed; 0 failed
```
