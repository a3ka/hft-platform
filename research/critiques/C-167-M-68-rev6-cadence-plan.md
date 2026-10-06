<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: dd2d16718334bf2fb9a2e38b5719d7f770ddfcfe
audited_head: bfa2b1c898a47cd31cc5396427fde1eea9516873
verdict: REJECT
-->

# C-167 — M-68 rev6: cadence plan cannot reach GREEN as specified

## Verdict: REJECT

`C-165`'s procedural blockers are closed: the complete committed artifact set is
present and `П-020` is an ancestor of the audited head.  This is a substantive
review of that set, not a repeat of C-165.  It cannot be handed to dev yet: one
new RED oracle is infeasible against the declared wire representation, task 12
has no acceptance check, and cadence-sensitive checkpoint invalidation is only
prose.

`VB-I-2` is the live invariant for this review.  The proposed cadence must remain
event-time deterministic both for a full replay and for checkpoint/resume; the
current tests do not establish that latter path.

## Artifact-set disposition

| Required artifact | Status |
|---|---|
| Milestone | Present: `milestones/M-68-depth-from-book.md` |
| T1 contracts / contract-RFC | No `crates/contracts/**` delta; no T1 change declared or found |
| T2/T3 and trait signatures | The intended `Selector` and `SeriesBundle` fields are explicitly RED; no new public trait is declared |
| RED suite | Present, including `d9`, `d9-C`, `d10`, `d12`, and `d13` |
| Acceptance script | Present, fail-closed aggregator, but incomplete for task 12 and the checkpoint claim |
| Milestone authority | `П-014` and `П-020` are present at the audited head |

## Blocking findings

### B1 — `d12` demands more points than the committed depth-series form can represent

`red_depth_cadence.rs:75` fixes `timeframe_ms: 1_000`, then generates 100-ms
events (`:48-51`, `:85-93`) and requires strictly `fast(100 ms) > slow(1000 ms)
> slower(10000 ms)` depth point counts (`:121-139`).  The existing form cannot
make the first strict comparison true:

- `DepthRow.series` is explicitly `(time_s, depth)` close semantics
  (`crates/gateway/src/lib.rs:255-263`);
- `DepthAcc.values` is a `BTreeMap<i64, i64>` (`:434-439`), so each key admits
  one point;
- `bucket_time_s` divides the bucket by 1,000 (`:824-831`), and the depth path
  writes with `row.values.insert(time_s, sum)` (`:1137-1162`).

Thus the fixture's events at offsets 0, 100, and 900 ms all key to the same
`time_s`; a 100-ms depth cadence and a one-second depth cadence each leave one
stored point per second.  Making `d12` pass requires an undeclared change of the
time coordinate / `DepthRow` semantics, not merely the two proposed fields.  It
would also require the associated export/checkpoint compatibility decision and
oracle.

Required resubmission: choose and specify one form before dev.  Either constrain
`depth_cadence_ms >= 1000` and make `d12` exercise representable intervals, or
introduce an explicitly versioned sub-second depth-series form with RED coverage
for serialization, merge, window eviction, and checkpoint compatibility.  In
both cases, the test must directly observe that heatmap stays per-event; `d12`
currently only counts `depth_series`, while `d13` only asserts a label.

### B2 — task 12 is declared but has no acceptance check

Task 12 requires a truthful `recompute_depth_from_book` self-description
(`milestones/M-68-depth-from-book.md:481`): either actually reuse the heatmap
vectors or remove the claim.  Its named `d6a`/`d6b` only measure depth scaling
and the per-band multiplier (`:592-593`).  The acceptance table maps their step
`C` only to task 8 (`:644-657`), and the committed script's only invocation is
`scripts/verify_M-68.sh:102-103`; it contains no task-12 check.

The two legal implementations of task 12 are distinguishable only by the false
claim / actual reuse, yet both can leave `d6a` and `d6b` green.  This violates
the required at-least-one check per task and lets the R-134 B-2(ii) repair be
silently omitted.

Required resubmission: add a fail-closed task-12 step and oracle.  It must
accept either declared implementation but reject the present false assertion;
if it asserts reuse, it must measure the claimed allocation/materialization
property rather than the existing visited-level proxy.

### B3 — cadence is required to invalidate a checkpoint but no oracle protects it

The new RED test says cadence must enter `selector_fingerprint`
(`crates/gateway/tests/red_depth_cadence.rs:17-26`), because cadence changes a
reducer's meaning.  Neither `d12` nor `d13` opens, writes, or resumes a
checkpoint, and neither compares fingerprints.  The acceptance script's step
`J` only rejects a changed *function declaration* via
`grep -E '^[+-].*fn selector_fingerprint'` (`scripts/verify_M-68.sh:156-160`);
it neither requires the new field in the hash nor detects a missing field.

A dev can implement both requested output fields and pass `d12`/`d13` while a
checkpoint made under cadence A is reused under cadence B.  That is precisely a
warm-start divergence from `VB-I-2`.

Required resubmission: add a RED checkpoint/resume oracle with two otherwise
identical selectors differing only in cadence.  It must demonstrate the chosen
fail-closed behavior (different fingerprint / stale checkpoint rejection or a
proven equivalent) and wire it into `verify_M-68.sh` as task-15 coverage.

## Checks that passed

- The branch contains `П-020`; `origin/main` is an ancestor of the audited head.
- No `crates/contracts/**` change exists in the audited subject delta, so
  Block-C and contract-RFC are not triggered.
- The declared baseline reproduces: `verify_M-68.sh` is `FAIL (4)`, exit 1;
  `red_depth_cadence` has exactly the two intended compile errors and `d10`
  fails on `[16, 16]`.
- `d9` is not a placebo: replacing the one-sided early return in an isolated
  copy with zero-point insertion produced one `d9` failure while `d9-C` passed.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-167
exit=0

$ git rev-parse HEAD; git merge-base --is-ancestor origin/main HEAD; echo exit=$?
bfa2b1c898a47cd31cc5396427fde1eea9516873
exit=0

$ git diff --name-status dd2d167..bfa2b1c -- crates/contracts
{пусто}
exit=0

$ bash scripts/verify_M-68.sh | grep -E '^(===|PASS:|FAIL:|VERDICT:)'; echo exit=$?
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: cargo test -p gateway --test red_depth_semantics --quiet
FAIL: cargo test -p gateway --test red_depth_cadence --quiet
VERDICT: FAIL (4)
exit=1

$ cargo test -p gateway --test red_depth_cadence --quiet
error[E0560]: struct `Selector` has no field named `depth_cadence_ms`
error[E0609]: no field `cadence_ms` on type `SeriesBundle`
error: could not compile `gateway` (test "red_depth_cadence") due to 2 previous errors
exit=101

$ # isolated d9 mutation: one-sided early return -> zero-point insertion
$ cargo test -p gateway --test red_depth_semantics md_i8_d9 --quiet
running 2 tests
. 1/2
md_i8_d9_one_sided_book_writes_no_point_not_a_zero --- FAILED
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out
exit=101

$ for offset_ms in 0 100 900 1000; do echo "$offset_ms -> $(((1752000000000 + offset_ms) / 1000))"; done
0 -> 1752000000
100 -> 1752000000
900 -> 1752000000
1000 -> 1752000001
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-26T19:38Z
- Milestone: M-68-depth-from-book (rev6)
- Статус: BLOCKED
- HEAD: bfa2b1c — test(MD-I-8): d12/d13 — cadence on series and its output declaration [architect]

## §B — Что я сделал
- Audited the committed artifact set at `bfa2b1c`, including the milestone, T-contract/trait surface, RED tests, and acceptance script.
- Reproduced the four expected RED failures and independently executed the d9 mutation.

## §C — Артефакты / результаты
- `research/critiques/C-167-M-68-rev6-cadence-plan.md`
- Done Block: baseline gate exit=1; cadence COMPILE-RED exit=101 with exactly two intended errors; d9 mutation exit=101 with one semantic test failure and one passing control.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  C-167 REJECTED M-68 rev6 on feat/M-68-rev4. Amend only the architect artifact set, then commit and push before another critic round. Resolve B1 by choosing an explicit representable cadence contract: either enforce depth_cadence_ms >= 1000 and use representable test intervals, or define/version a sub-second DepthRow time form with RED coverage for serialization, merge, window eviction, and checkpoint compatibility. d12 must also observe heatmap remains per-event. Resolve B2 by adding a fail-closed task-12 acceptance check for truthful self-description / actual reuse, not d6a/d6b's visited-level proxy. Resolve B3 with a RED checkpoint/resume test proving cadence changes selector_fingerprint and prevents reuse across different cadences. Do not change implementation code.
  ```
- Push-статус: pending this verdict commit to `origin/feat/M-68-rev4`
- ⏸ кэш оставлен — the isolated critic worktree contains a compiled target cache; it will be reclaimed after the verdict is committed and pushed.

## §E — Риски / открытые вопросы
- This REJECT is for reasons different from C-165 (which only found an incomplete set), so the two-REJECT-same-reason arbitration trigger does not apply.
- Whether the product permits cadence below one second is a design choice for architect to state; the current artifact set states neither a lower bound nor a compatible sub-second wire form.

=== END HANDOFF ===
