<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: c368ed6579c8621216c95913977894c1b9bc4af2
audited_head: 5a22a824e926b46c518c2690eeb2ebc3b20fbb3e
verdict: NOTE
-->

# C-210 — M-70: narrow FA audit of `VB-I-5` — NOTE

**Role:** critic · **Date (UTC):** 2026-09-04

## Scope and artifact set

This is the explicitly requested narrow `gates.md` §9 circle for the FA-invariant
change in `5a22a82`, audited on `origin/docs/M-70-rev2`, not on the current
`origin/main`.  The committed set is present:

- T-designate shape and signature: `DepthRow` retains
  `series: Vec<(i64, i64)>` and adds
  `series_provenance: Vec<Option<String>>` (`crates/gateway/src/lib.rs:384-393`).
  `docs/05-contract-layer.md` §2 classifies this gateway form as T-designate:
  additive evolution with a version bump, without a T1 contract-RFC.
- RED oracles: `DB-I-4`/`4b`/`4c`/`4d` in
  `red_depth_point_provenance.rs`; the v1-consumer oracle in
  `red_gateway_export_v2.rs`; and MD-I-8 obligation 4's `d7`/`d7b` cases in
  `red_depth_from_book.rs`.
- Acceptance gate: `scripts/verify_M-70.sh` has an explicit `FAIL` accumulator,
  CI parity commands, one named check per task, and a terminal
  `VERDICT: PASS|FAIL` with a matching exit code.
- Milestone: `milestones/M-70-depth-bands-enablement.md` §2bis.-1 declares the
  additive shape before implementation; §2bis.-3 defines the separate
  binding oracle for merge plus eviction.

## Findings

### N-1 — NOTE: `VB-I-5` is true of the audited code

The new FA wording is supported by the implementation, rather than merely by
the milestone prose.  `DepthAcc.values` stores `(depth_e8, reach_at_observation)`
at write time, and `finish_ref` creates `series[i]` and
`series_provenance[i]` in the same iteration from that stored pair
(`crates/gateway/src/lib.rs:1556-1575`).  This is the live `MD-I-8` obligation 4:
the number and provenance originate in one observation.  The RED cases cover both
a shrinking and a growing delta book; `DB-I-4d` additionally protects the
point-to-label binding through merge and eviction.

### N-2 — NOTE: the form is additive for the actual v1 consumer

The pre-v10 `series` tuple form remains intact, and serde ignores the new
row field.  The committed v1 consumer (`DepthRowV1`) successfully deserializes
the audited snapshot's depth rows in
`red_gateway_export_v2::snapshot_carries_schema_version_and_is_v1_additive`.
This directly supports `VB-I-4` and the additive clause of `VB-I-5`.

The handoff calls this target `red_gateway_devexport_v2`; that target does not
exist in the `gateway` package. Cargo reports `red_gateway_export_v2` as the
similarly named committed target, and that is the test executed below. This is
a handoff-name discrepancy, not a code or oracle failure.

## Known boundary — not cleared by this narrow verdict

`bash scripts/verify_M-70.sh` is still red only because the existing external
`TD-199` oracle violates `VB-I-2`: replay has four depth points whereas
`snapshot(C)+frames` has three on a delta-only tail. This audit neither weakens
that oracle nor treats M-70 as globally green. The defect is shown independently
by `gw_i_4_holds_when_the_tail_frame_is_delta_only` and is outside the FA-shape
change in `5a22a82`.

`C-209` was a passing NOTE and later M-70 work changed `scripts/verify_M-70.sh`.
The subject-lock is therefore opened deliberately by this independently audited
successor verdict's commit message, using its prescribed audit token; no earlier
passing verdict is being silently reused.

## Verdict: NOTE

The three requested FA claims pass: `VB-I-5` reflects the committed code,
`MD-I-8` obligation 4 remains explicit and executable, and the v1 consumer
reads the additive depth-series form. The external `TD-199` red gate remains
a merge blocker for the full milestone.

## Done Block

```text
$ cargo test -p gateway --test red_gateway_devexport_v2 --quiet
error: no test target named `red_gateway_devexport_v2` in `gateway` package
help: a target with a similar name exists: `red_gateway_export_v2`
red_gateway_devexport_v2_exit=101

$ cargo test -p gateway --test red_gateway_export_v2 snapshot_carries_schema_version_and_is_v1_additive -- --exact
test snapshot_carries_schema_version_and_is_v1_additive ... ok
test result: ok. 1 passed; 0 failed
red_gateway_export_v2_v1_additivity_exit=0

$ cargo test -p gateway --test red_depth_point_provenance db_i_4_two_points_of_one_row_carry_their_own_provenance -- --exact
test db_i_4_two_points_of_one_row_carry_their_own_provenance ... ok
test result: ok. 1 passed; 0 failed
db_i_4_same_observation_exit=0

$ cargo test -p gateway --test red_depth_point_provenance db_i_4d_point_to_label_binding_survives_merge_and_eviction -- --exact
test db_i_4d_point_to_label_binding_survives_merge_and_eviction ... ok
test result: ok. 1 passed; 0 failed
db_i_4d_binding_exit=0

$ cargo test -p gateway --test red_depth_from_book md_i8_d7_reach_is_sampled_where_the_numbers_are_delta_shrinks_the_book -- --exact
test md_i8_d7_reach_is_sampled_where_the_numbers_are_delta_shrinks_the_book ... ok
test result: ok. 1 passed; 0 failed
md_i8_d7_shrink_exit=0

$ cargo test -p gateway --test red_depth_from_book md_i8_d7b_reach_is_sampled_where_the_numbers_are_delta_grows_the_book -- --exact
test md_i8_d7b_reach_is_sampled_where_the_numbers_are_delta_grows_the_book ... ok
test result: ok. 1 passed; 0 failed
md_i8_d7b_grow_exit=0

$ bash scripts/verify_M-70.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
PASS: task #6 версия схемы поднята 9 → 10
PASS: task #6b sacred-пин версии согласован с константой (10)
PASS: task #7 канонический набор из семи полос объявлен записью в блоке gateway-serve
VERDICT: FAIL (1)
verify_M-70_exit=1

$ cargo test -p gateway --test red_depth_provenance_by_reach gw_i_4_holds_when_the_tail_frame_is_delta_only -- --exact
test gw_i_4_holds_when_the_tail_frame_is_delta_only ... FAILED
... Реплей: [confirmed, not-observed reach=0.000015, not-observed reach=0.005000, confirmed]
... собранное клиентом: [confirmed, not-observed reach=0.005000, confirmed]
td_199_oracle_exit=101

$ git diff --check c368ed6579c8621216c95913977894c1b9bc4af2..5a22a824e926b46c518c2690eeb2ebc3b20fbb3e
diff_check_exit=0
```

## Handoff

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC, ISO-8601): 2026-09-04T23:00Z
- Milestone: M-70-depth-bands-enablement
- Status: DONE — narrow FA circle NOTE; full milestone remains red on TD-199
- HEAD audited: 5a22a824e926b46c518c2690eeb2ebc3b20fbb3e

## §B — What I did
- Audited the committed FA wording against `DepthRow` and the point-level provenance path.
- Reproduced additive v1 decoding and the MD-I-8 same-observation oracles.

## §C — Artifacts / results
- `research/critiques/C-210-M-70-fa-vb-i-5.md`
- Done Block above; acceptance is `exit=1` only because of the named external TD-199 oracle.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```text
  C-210 gives NOTE for the narrow M-70 FA change: VB-I-5, MD-I-8 obligation 4,
  and v1 additive decoding are verified at 5a22a82. Do not claim M-70 is green:
  verify_M-70 remains exit=1 on external TD-199 (delta-only tail yields 3 points
  through snapshot+frames versus 4 through replay). Preserve that oracle; route
  the separate TD-199 decision/fix before any M-70 merge.
  ```
- Push status: committed and pushed with this verdict.
- Cache: will be removed after the committed verdict is pushed.

## §E — Risks / open questions
- The specific handoff target name `red_gateway_devexport_v2` is stale; the committed
  `red_gateway_export_v2` is the actual v1-additivity oracle.
- TD-199 remains a blocking full-milestone dependency.

=== END HANDOFF ===
