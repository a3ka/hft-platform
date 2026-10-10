# C-286 — M-95 catalog-once, round 3: NOTE

<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: 6ee3f1af60fd1a482842d267d84fc625505c2db4
audited_head: ab776631e6514cec9c88caedf2966cca60439cc5
verdict: NOTE
-->

## Verdict

**NOTE — A-050 A1–A4 are executed; plan-time critic gate is clear for dev dispatch.**
The audit used the fetched tip of `origin/feat/M-95-catalog-once`, not the handoff SHA as
authority. The committed artifact set contains the milestone, both M-95 RED suites, the
adapted `SM-1`/`SM-3` oracle, acceptance script, CI-map probe, and the prior gate/arbitration
artifacts. No `contracts/**` or implementation path is changed by the audited head.

The live invariant exercised by this subject is `VB-I-11` (history provenance honesty) from
`docs/fa/viz-backend.md`. The journal-side boundary remains protected by `JR-I-11` through
the existing `red_stitch_monotonic` and `SM-*` oracles.

## A-050 execution findings

### A1 — accepted

The milestone §3 pins `m95-catalog:<id>` immediately before v1 provenance and
`m95-catalog:legacy` after the legacy catch-up loop, with no catalog operation between the
rendezvous and provenance. §6 explicitly forbids moving the point before the legacy drain or
separating it from provenance with `pump`, `is_fresh`, `refresh`, or `SegmentCatalog::open`.

### A2 — accepted

`f1` and `f2` append `AT_PAUSE` events beginning at `TAIL_END` during the pause and require
`cursor.upto_seq < TAIL_END`. `TAIL_END` is derived by the fixture (`TAIL_FROM + 40`) and is
used by the fixture append, so the upper boundary is not an independent magic literal. The
legacy-only lower bound (`upto_seq >= TAIL_END - 1`) is explicitly a setup guard: legacy must
have completed its pre-point catch-up, while v1 has no such drain and therefore correctly uses
only the upper bound. The test comments name this asymmetry and explain why omitting the v1
lower bound is not vacuuming the assertion.

### A3 — accepted

§8 names mutant `A-050 E5` for the legacy point-before-drain plus provenance-without-freshness
case, and the test's witness failure identifies `A-050 E5/W2`. The table also preserves the
`VB-I-11` mutant and the `C-285` pre-build ordering mutant.

### A4 — accepted

§14 records A-050, the A1–A4 changes, and the architect's seam matrix. I independently ran
the two required temporary seams in `crates/gateway-serve/src/lib.rs`, then reverted them:

* rendezvous immediately before provenance: `3 passed`;
* legacy rendezvous before the catch-up loop: `f2 FAILED` with `A-050 E5/W2`, observing
  `upto_seq=3046 >= 3040`.

The baseline without the temporary rendezvous is still RED as expected: all `f0…f2` fail
because the committed pre-implementation has no M-95 rendezvous point.

## NOTE 1 from A-050

The SWITCH/resubscription branch still emits snapshot provenance frozen in the checkpoint and
does not call `history_provenance_for_serve`. Accepting the milestone's §5 decision to keep
this pre-existing `VB-I-11` gap out of M-95 is appropriate: expanding task 3 without an `f3`
RED oracle would widen scope and violate RED-first. The required TD through the reviewer at
the M-95 PR gate is an explicit, auditable follow-up condition; it must not be silently omitted
from the reviewer close-out.

## Scope and artifact checks

The subject branch changes only architect-owned M-95 artifacts plus the prior committed
critique/arbitration records. The critic writes only this new file. No milestone, docs,
contracts, crates, or project-state file was edited by this audit.

## Done Block

```text
$ git fetch origin feat/M-95-catalog-once && git rev-parse origin/feat/M-95-catalog-once
ab776631e6514cec9c88caedf2966cca60439cc5
exit=0

$ git merge-base origin/main origin/feat/M-95-catalog-once
6ee3f1af60fd1a482842d267d84fc625505c2db4
exit=0

$ git diff --name-status origin/main..origin/feat/M-95-catalog-once
A       crates/gateway-serve/tests/red_m95_catalog_once.rs
A       crates/gateway-serve/tests/red_m95_provenance_fresh.rs
M       crates/gateway/tests/red_segment_meta_bound.rs
A       milestones/M-95-catalog-once.md
A       research/arbitration/A-050-m95-provenance-ordering.md
A       research/critiques/C-284-M-95-catalog-once.md
A       research/critiques/C-285-M-95-catalog-once-round2.md
A       scripts/tests/red_verify_M-95_ci_map.sh
A       scripts/verify_M-95.sh
exit=0

$ bash scripts/next_artifact_id.sh C
C-286
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m95r3/target cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh -- --nocapture   # honest seam
test result: ok. 3 passed; 0 failed; ... finished in 3.44s
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m95r3/target cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh f2_legacy_retention_between_catalog_and_provenance_is_honest -- --nocapture   # E5 seam
M-95 / A-050 E5/W2: legacy — snapshot cursor.upto_seq=3046 >= 3040
test result: FAILED. 0 passed; 1 failed; ...
exit=101

$ CARGO_TARGET_DIR=/tmp/hft-critic-m95r3/target cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh -- --nocapture   # baseline
test result: FAILED. 0 passed; 3 failed; ...
baseline_exit=101

$ git diff --check
exit=0
```

## Gate condition

`NOTE` clears this narrowed plan-time round. Dev dispatch remains subject to the milestone
route: M-94 must be merged first, and reviewer must create the promised TD for the SWITCH
provenance gap at PR time.
