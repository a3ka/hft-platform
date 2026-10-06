<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: 153d329a00168c99f656c728921c1643e2107014
audited_head: c82057cd1aac6348ef0b4e2f768eb66b97903d5e
verdict: NOTE
-->

# C-209 — M-70: C-208 closures hold under independent prototype and carrying-path mutation

## Verdict: NOTE

This is the deliberately narrow third critic pass over
`153d329..c82057c`, limited to B-1, B-2, and B-3 of `C-208`. All three
blockers are closed. The only finding is a non-blocking report-integrity debt
(`P-022` below); it does not hold this subject branch.

## C-208 closure evidence

### B-1 — one temporal policy, and the opposite world is killed

`M-70` §2bis.1, the task-5 test header, and `DB-I-5d` now state one policy:
a heatmap cell is labelled from the reach of its own bucket observation.
The proposed task shape derives reach from that bucket's `bids`, `asks`, and
`mid`; it does not pass the reducer's latest reaches into the heatmap builder.
The declared consequence is explicit: for a level actually present in a
bucket, `not-observed` is structurally unreachable for the heatmap.

I reproduced both implementations in isolated, discarded worktrees. Prototype
A computes per-bucket bid/ask reach and makes all four `DB-I-5*` scenarios pass.
Prototype B instead passes `Reducer::depth_reach_*` from the latest book to
every historical bucket. It leaves `DB-I-5`, `5b`, and `5c` green, but fails
only `DB-I-5d` after the 3% → 1% resync with
`not-observed band=0.014000 reach=0.010000`. The oracle therefore distinguishes
the rejected temporal world rather than merely describing it.

### B-2 — M-75 prerequisite is no longer false-red

`verify_M-70.sh` no longer uses `git show | grep -q` under `pipefail`.
It reads the source once and evaluates a helper against a here-string. Its
self-check executes both outcomes: an actual function declaration returns 0,
while a comment-only mention returns 1. The real `origin/main` prerequisite is
green. The remaining gate failure is the intended RED/open-task set, not the
already-satisfied M-75 prerequisite.

### B-3 — delivery proof reaches the wire and kills a carrying-path mutation

`DB-I-7d` now uses the production server boundary: `bind` → `serve` → JWT WS
connection → first `ServeMsg::Snapshot` on the wire. It checks both the
delivered selector's seven bands and the corresponding fourteen non-empty depth
rows. I mutated only the legacy response-producing `LiveReducer::resume` call
to discard configured bands while retaining parser behaviour. The three parser
scenarios stayed green; exactly `DB-I-7d` failed (wire snapshot: 1 band, expected
7). This closes the built-not-wired world identified in C-208.

## NOTE — P-022: stale gate-result prose in the milestone

The current gate execution reports `VERDICT: FAIL (10), exit=1`, whereas the
unchanged M-70 task/artifact tables still say `FAIL (6)`. Those same old rows
also say `DB-I-0` and `DB-I-3` are intentionally unwritten, although both files
are present. This is stale report prose, not a defect in the three C-208
closures; record it as `P-022` and repair it in the appropriate documentation
pass. It does not block merge of this subject branch.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-209
exit=0

$ cargo test -p gateway --test red_depth_label_dictionary -- --nocapture
# isolated prototype A: per-bucket reach
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ cargo test -p gateway --test red_depth_label_dictionary -- --nocapture
# isolated prototype B: latest Reducer reach for every historical bucket
test db_i_5_one_dictionary_for_cell_and_row_in_the_same_response ... ok
test db_i_5b_map_labels_discriminate_side_like_the_series_does ... ok
test db_i_5c_series_does_not_lose_liveness_to_unification ... ok
test db_i_5d_cell_keeps_the_provenance_of_its_own_observation ... FAILED
DB-I-5d НАРУШЕН: ... "not-observed band=0.014000 reach=0.010000"
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ cargo test -p gateway-serve --test red_depth_bands_delivery -- --nocapture
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ cargo test -p gateway-serve --test red_depth_bands_delivery -- --nocapture
# isolated legacy LiveReducer::resume mutation: configured selector replaced by [0.001]
test db_i_7_canonical_bands_from_env_reach_the_selector ... ok
test db_i_7b_absent_bands_fall_back_to_prod_default_not_to_refusal ... ok
test db_i_7c_canonical_and_default_are_actually_distinguishable ... ok
test db_i_7d_canonical_bands_reach_the_frame_on_the_wire ... FAILED
DB-I-7d НАРУШЕН: кадр НА ПРОВОДЕ несёт 1 полос вместо 7
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ bash scripts/verify_M-70.sh
PASS: самопроверка предиката M-75 — истинный вход даёт 0, ложный (комментарий) даёт 1
PASS: ПРЕДУСЛОВИЕ — M-75 влит в main (^pub fn effective_heatmap_window_frac( присутствует)
VERDICT: FAIL (10)
exit=1

$ git diff --check 153d329..c82057c
exit=0
```
