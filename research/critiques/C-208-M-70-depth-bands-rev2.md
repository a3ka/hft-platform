<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: aacab22205d436e9e5d9399bc7098cd499a73e7c
audited_head: 153d329a00168c99f656c728921c1643e2107014
verdict: REJECT
-->

# C-208 — M-70 depth-bands rev2: RED set is complete, but two required oracles/gates are not valid

## Verdict: REJECT

The architect artifact set is now present: declared T-designate shapes, five named
`DB-I-*` tests, `verify_M-70.sh`, and the milestone are committed on the audited
head. `crates/contracts/**` and the write-side paths are outside the range. The
set is nevertheless not dispatchable: task 5 states mutually incompatible
provenance semantics, and the acceptance script makes an already satisfied
precondition permanently red.

`VB-I-10` was read from `docs/fa/viz-backend.md`; its live invariant is that the
bounded window is attached to the event cursor rather than wall-clock. Task 8
does execute the existing bounded-window tests, but that does not cure the
findings below.

## Blocking findings

### B-1 — `DB-I-5d` requires provenance from a different observation than the milestone requires

`M-70` §2bis.1 says a heatmap cell's reach must come from **the same observation
as that bucket's contents**; otherwise the label describes the wrong moment
(`milestones/M-70-depth-bands-enablement.md:333-336`). The proposed signature
instead supplies one `reach_bid` and one `reach_ask` when the final heatmap is
built (`:315-325`). On the audited implementation, `HeatmapBucketState` stores
only bids, asks, and mid (`crates/gateway/src/lib.rs:771-775`), while the reducer
keeps one latest reach per side (`:738-741`) and `finish_ref` reads those two
latest values (`:1460-1461`). There is no reach associated with a historical
bucket.

The RED fixture then constructs a 3%-reach bucket followed by a resync to 1%
(`red_depth_label_dictionary.rs:265-274`) and demands that the cell from the
first bucket be labelled `not-observed` because of the **current** 1% reach
(`:303-312`). That is the opposite of the declared same-observation rule: the
first bucket was observed at 3%. A green implementation can satisfy the test by
applying the final reach to every historic cell, but then it violates the
milestone's stated protection against a label for the wrong moment. Conversely,
per-bucket provenance follows the stated rule but fails `DB-I-5d`.

Architect must choose and state one temporal meaning of heatmap provenance, then
make §2bis.1, its declared data shape, and all `DB-I-5*` scenarios express that
same meaning. The corrected RED suite must include the 3% → 1% transition and
fail the rejected interpretation; dev cannot be asked to choose the policy.

### B-2 — the M-75 prerequisite guard is false-red under its own `pipefail` mode

The required function is present on `origin/main` as
`pub fn effective_heatmap_window_frac()` (`crates/gateway/src/lib.rs:108`). Yet
the prerequisite in `verify_M-70.sh:118` is:

```bash
git show origin/main:crates/gateway/src/lib.rs | grep -qE '^pub fn effective_heatmap_window_frac\('
```

With the script's `set -o pipefail`, `grep -q` exits after the match and closes
the pipe; `git show` exits `141`, so the pipeline is false even while its
requirement is true. The independent reproduction below returned pipeline stages
`141 0`. Therefore the gate cannot become PASS after the planned implementation;
it retains a red precondition that has already been fulfilled. Repair the guard
and add a check that proves both the true-P and absent-P cases have the intended
exit code.

### B-3 — green `DB-I-7` is only an env-parser guard, not the claimed delivery oracle

`DB-I-7` is allowed to be green before task 7: it is explicitly a regression
guard, just as green `DB-I-0` is a measurement/regression guard with a positive
control. But `red_depth_bands_delivery.rs:71-137` only invokes
`serve_config_from_env` and compares `ServeConfig.selector.bands`. It never
starts the production entrypoint, binds a server, accepts a subscription, or
observes the selector used to produce a response.

Thus an implementation in which the parser remains correct but the executable
discards or replaces that selector after parsing still passes all three tests.
That is exactly the built-not-wired world the test claims to exclude. The
milestone's acceptance criterion says the canonical set reaches the service that
builds the response (`M-70:459`, `verify_M-70.sh:256-261`); `testing.md` requires
an oracle for a carrying path to execute the process boundary using the production
invocation. Add the missing entrypoint/response-path evidence, or narrow the
claim so it no longer claims delivery.

## Non-blocking confirmations

- `DB-I-0` is legitimately green: it measures serialized response bytes and its
  companion scenario proves the wider selector actually adds depth-series rows
  while leaving the heatmap size unchanged.
- `DB-I-3` has an explicit `MAX_BANDS = 32` declaration, clean-function RED
  path, inclusive boundary, and canonical-set anti-placebo. The delegated choice
  is expressed in the milestone rather than left to dev.
- `DB-I-4` declares `DepthPoint` and the architecture-owned test-adapter task
  before dev dispatch. Its current RED result is a real form mismatch, not a
  vacuum.
- The presence-guard table required by `A-031` §1 is present in
  `verify_M-70.sh:19-55`; the scope diff contains no `contracts/` or write-side
  paths.

## Required resubmission evidence

1. A committed resolution of B-1 with an oracle that rejects the opposite
   temporal provenance policy.
2. A repaired M-75 prerequisite whose true and false cases are both executed,
   and a `verify_M-70.sh` result whose red items are only deliberately open
   implementation tasks.
3. A task-7 oracle that reaches the actual response-producing route, not solely
   `serve_config_from_env`.

## Done Block

```text
$ git rev-parse HEAD && git merge-base origin/main HEAD
153d329a00168c99f656c728921c1643e2107014
aacab22205d436e9e5d9399bc7098cd499a73e7c
exit=0

$ git diff --name-status aacab22205d436e9e5d9399bc7098cd499a73e7c..153d329a00168c99f656c728921c1643e2107014
A  crates/gateway-serve/tests/red_depth_bands_delivery.rs
A  crates/gateway/tests/red_depth_bands_cap.rs
A  crates/gateway/tests/red_depth_egress_canonical.rs
A  crates/gateway/tests/red_depth_label_dictionary.rs
A  crates/gateway/tests/red_depth_point_provenance.rs
M  milestones/M-70-depth-bands-enablement.md
A  research/critiques/C-193-M-70-depth-bands-rev2.md
A  scripts/verify_M-70.sh
exit=0

$ cargo test -p gateway --test red_depth_egress_canonical -- --nocapture
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ cargo test -p gateway --test red_depth_bands_cap -- --nocapture
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ cargo test -p gateway --test red_depth_point_provenance -- --nocapture
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ cargo test -p gateway --test red_depth_label_dictionary -- --nocapture
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ cargo test -p gateway-serve --test red_depth_bands_delivery -- --nocapture
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ bash scripts/verify_M-70.sh
FAIL: git show origin/main:crates/gateway/src/lib.rs | grep -qE '^pub fn effective_heatmap_window_frac\\('
VERDICT: FAIL (11)
exit=1

$ set -o pipefail; git show origin/main:crates/gateway/src/lib.rs | grep -qE '^pub fn effective_heatmap_window_frac\\('; printf 'm75_guard_pipeline_exit=%s stages=%s\\n' "$?" "${PIPESTATUS[*]}"
m75_guard_pipeline_exit=141 stages=141 0
exit=0
```
