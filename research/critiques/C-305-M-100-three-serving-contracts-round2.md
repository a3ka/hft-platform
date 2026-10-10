<!-- GATE-META
milestone: M-100
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: 082daf9fd76f467fe635c3298dccddd7de94fa42
verdict: REJECT
-->

# C-305 — M-100 three serving contracts, round 2: REJECT

## Subject and artifact set

This is a RAW-gate, plan-time audit of the committed subject
`origin/feat/M-100-three-serving-contracts` at
`082daf9fd76f467fe635c3298dccddd7de94fa42`, not a review of plan prose alone.  The handoff
SHA `082daf9f` remains the fetched branch tip and is its ancestor.  The audit base is
`a660b3a7b78b116a0a98bc7c9d9eb1e03153725c`.

The complete architect set is present before dev dispatch:

- `milestones/M-100-three-serving-contracts.md` declares the T-designate API surface,
  allowed/forbidden paths, task ownership, RED suite and acceptance command;
- `docs/rfc/CT-RFC-09-ws-session.md` rev3 declares the server resolver, `gateway::view`,
  `gateway::calc_key`, v12 wire shape and separate calculation-state version;
- the five committed RED files cover `h1/h2`, `k1…k5`, `sv1…sv3`, `p1…p4`, and `w1…w9`;
- `scripts/verify_M-100.sh` is a `set -uo pipefail` explicit FAIL-count aggregator, with
  every task represented and a CI-parity map guarded by the ten-world
  `scripts/tests/red_verify_M-100_ci_map.sh` probe.

There is no change under `crates/contracts/**`: these are T-designate gateway/WS forms, not a
T1 mutation.  The RFC is the correct design artefact for the public wire contract; Block-C is
not triggered.  `VB-I-2` (live equals replay) and `VB-I-12` (Replace semantics for observed
heatmap columns) in `docs/fa/viz-backend.md` are the live invariants material to this round.

## Verdict: REJECT

Dev dispatch remains blocked by one RED-oracle setup defect.

### R1 — h2 does not prove that every declared partition reaches a live-frame boundary

`h2_same_events_different_live_frame_boundaries_same_state` correctly supplies four event
groupings and sends every emitted `Frame` through the serialize/deserialize client path.  It also
uses independent full replay as its oracle.  On the current code it is RED in the all-in-one
case: the client retains `64990 × 5` while replay does not.

Its setup guard is nevertheless aggregate-only:

```rust
n_frames += frames.len();
// after every group:
assert!(n >= groups.len(), ...);
```

That count does not establish the property named by the test: one earlier `pump` may emit several
frames while a later requested group emits none.  The total can still satisfy `n >= groups.len()`.
Such a run has not exercised that later live-frame boundary, so a repair that is green only
because the producer silently coalesced or omitted a group can pass the claimed boundary-
invariance oracle.  This violates the per-scenario setup-guard requirement in
`.claude/rules/testing.md`.

Condition to clear: in `fold_partition`, assert immediately for every group that its own
`pump` returned at least one frame (and retain the existing wire/application and independent-
replay comparisons).  If the milestone continues to state the exact current classification of
the four partitions, provide executable/raw evidence for all four; the present RED invocation
legitimately stops at `[1 2 3]` and only directly demonstrates that first mismatch.

## Round-one disposition

### R2 — cleared: w8/w9 reach the real admission path and name branch-local mutations

`fixture_with_header_version` rewrites bytes `12..16` of the checkpoint actually discovered by
`checkpoint::ckpt_path_for_pub`, with a length setup guard.  `w8` then starts the real server via
`bind_with_policy`, subscribes over WS, and requires a v12 snapshot from an explicitly state-11
checkpoint.  It is RED today because the emitted snapshot is still v11; it is not a library-only
test.  `w9` drives both foreign state values (10 and 12) through the same admission path and
requires the named `not_ready` result; it is GREEN today.

The mutation table is now honest.  A mutation confined to `admission::readiness` is held by
`w8`; a mutation confined to `read_and_validate` is held by `sv2`; acceptance of arbitrary
state versions is held by `w9`/`sv3`.  It explicitly says that the library-only branch is held
only by `sv2`, rather than claiming the wire suite covers it.

### R3 — cleared: the claim gate passes on the merge preview

`bash scripts/verify_design_claims.sh --merge-preview origin/main` exits 0.  Its §22 check sees
the amended `VB-I` count (10 oracles), and it also reports no broken checked RFC paths or SHA
claims.  `git diff --check` and `git fsck --no-dangling --no-reflogs` both exit 0.

### N2 — cleared; no arbiter escalation

CT-RFC-09 §2.10.1 explicitly identifies its change as a replacement of the old M-84 entry
rule, names `A-033` §1.4 and D-2, keeps the guard at the product resolver, and states the
replacement invariant: a client can only submit the server profile unchanged or receive
`unsupported`; it cannot choose a grid.  This preserves D-2's placement and the
server-owned-grid rationale while providing the named v1 compatibility case.  M-84 remains an
unmerged parked branch.  The primary sources therefore resolve the relation; this is not an
unresolved dispute requiring `ESCALATE` to an arbiter.

## Gates.md §9 recheck

- **(a) Code claims measured on the merge tree:** h2, w8 and w9 were executed against the
  committed tip; the three missing T-designate interfaces fail compile-RED as expected; the
  merge-preview claims gate is PASS.
- **(b) Authority:** the range changes no T1 contract and no boundary-C decision.  The only
  supersession question is explicitly bounded above; the calculation profile remains
  server-owned and no new data composition, phase, promotion, limit, or money decision appears.
- **(c) Links and coherence:** the merge-preview gate passes all checked design/RFC paths and
  SHA references.  The milestone's allowed paths match the architect-only documents/tests/gate
  plus the named future engine-dev implementation surfaces.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-100-three-serving-contracts
082daf9fd76f467fe635c3298dccddd7de94fa42
exit=0

$ git merge-base --is-ancestor 082daf9f origin/feat/M-100-three-serving-contracts
exit=0

$ bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-304 …
reserve: C-304 занят; следующий кандидат — C-305
C-305
reserve: резерв C-305 взят
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=a660b3a7… bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона a660b3a..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ cargo test -p gateway --test red_m100_heatmap_frame_close h2_same_events_different_live_frame_boundaries_same_state -- --nocapture
assertion `left == right` failed: … разбиение [1 2 3] — клиент разошёлся с полным пересчётом
left:  … (1752000010, "bid", 6499000000000): 500000000
right: … no (1752000010, "bid", 6499000000000)
test result: FAILED. 0 passed; 1 failed
exit=101

$ cargo test -p gateway-serve --test red_m100_three_contracts_wire w8_pre_m100_checkpoint_state_11_admitted_under_wire_12 -- --nocapture
assertion `left == right` failed: … слепок с версией состояния 11 обязан дать снимок провода v12
left: Number(11)
right: Number(12)
test result: FAILED. 0 passed; 1 failed
exit=101

$ cargo test -p gateway-serve --test red_m100_three_contracts_wire w9_foreign_state_version_is_not_ready -- --nocapture
test w9_foreign_state_version_is_not_ready ... ok
test result: ok. 1 passed; 0 failed
exit=0

$ cargo test -p gateway --test red_m100_{calc_key_id,state_version,view_projection} --no-run
calc_key: unresolved import `gateway::calc_key`
state_version: cannot find `gateway::checkpoint::CALC_STATE_VERSION`
view_projection: unresolved import `gateway::view`
each expected compile-RED exit=101

$ VERIFY_M100_CI_DRY=1 bash scripts/verify_M-100.sh
PASS  ci-parity: учтено шагов 64 из 64 (исполнено 57, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_verify_M-100_ci_map.sh
VERDICT: PASS — 10 сценариев
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: VB-I — заявлено=12, в оракулах=10
PASS  [7-RFC-PATH] … все 183 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --check origin/main...082daf9f
exit=0

$ git fsck --no-dangling --no-reflogs
exit=0
```
