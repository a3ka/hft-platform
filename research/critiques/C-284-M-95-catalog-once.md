# C-284 — M-95 catalog-once: REJECT

<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: 6ee3f1af60fd1a482842d267d84fc625505c2db4
audited_head: d341c8d38dda75212b89af6f6d437db97e7a81ff
verdict: REJECT
-->

## Verdict

**REJECT — dev dispatch blocked.**  The branch has the required plan-time artifact
set (milestone, RED suite, verify script and its CI-map probe); it has no T1 contract
change, so a contract-RFC is not required.  The proposed T2/T3 refactor nevertheless
contradicts a live sacred oracle and removes a fail-closed observation without a
replacement oracle.

## Blocking findings

### B1 — §3 makes the existing M-62 resource oracle impossible to keep truthful and green

`milestones/M-95-catalog-once.md:67-71` requires that the catalog be built before the
first `pump` and passed into that pump.  With `Some(catalog)`,
`journal::stream_from_at_with_catalog` takes `is_fresh`/`refresh`
(`crates/journal/src/segments.rs:2579-2606`), rather than the `None` branch that calls
`SegmentCatalog::open`.

That is the opposite temporal contract of the existing M-62 oracle:

- `crates/gateway/tests/red_segment_meta_bound.rs:298-307` (`SM-1`) and `:414-435`
  (`SM-3`) require the **first pump** to report `segment_meta_ops >= N`;
- `crates/journal/src/segments.rs:1454-1464,2565-2577` defines that field as the actual
  work of the stream: the full `open` belongs to the first `None` call, whereas a fresh
  supplied catalog costs only the freshness probe;
- `crates/gateway/src/lib.rs:3825-3833,3857-3864` merely forwards the stream's measured
  counter into `ReadStats`; adding the earlier `resume` scan to that later value would
  make the counter claim work that its stream did not do.

M-95 §4 calls `red_segment_meta_bound` an I-4 oracle and §9 requires it to remain green,
but §10 does not allow architect to revise that sacred test and §11 contains no task for
doing so.  Thus a literal correct implementation turns `SM-1`/`SM-3` red; a fake carry of
the count into `pump` preserves green only by falsifying the oracle's metric.

**Condition to clear B1:** architect must explicitly move the M-62 observation boundary
from “first pump” to the first catalog observation of a subscription, add the necessary
architect-owned test path to M-95, and keep the positive `countfake` protection by
measuring the real `SegmentCatalog::open` work on `resume + first pump` rather than
copying a number into a later stream result.  The updated RED must be red on the
pre-M-95 implementation and green only once the single-catalog path exists.

### B2 — catalog reuse drops the current-history observation required by VB-I-11

Current serving deliberately makes two observations.  `validate_lineage` lists the
catalog in `crates/gateway/src/lib.rs:4830-4839`; later
`history_provenance_for_serve` calls `current_history_provenance`
(`:4166-4174`), whose `first_visible_seq` performs a new `journal::list_segments`
(`:4113-4121`).  The existing RED documents why this matters: a compaction/retention
change between those reads is a real but nondeterministic transport race
(`crates/gateway/tests/red_m87_history_provenance_failclosed.rs:28-39`).

M-95 §3 instead reuses the initial catalog for lineage, provenance and the first pump.
If retention removes the earliest segment after that catalog is built but before the
snapshot is serialized, provenance can retain the old `history_start_seq` and
`history_truncated=false`.  The first pump may later notice the layout change through
`is_fresh`, but that is after the dishonest snapshot has been sent.  This violates
`VB-I-11` and M-95's own I-4.

`red_m87_history_provenance_failclosed` remains green on the branch, but it tests the
old helper in isolation; it cannot exercise the new catalog-aware transport path.
`red_m95_catalog_once` changes no catalog layout between construction and snapshot.
Neither oracle catches this regression.

**Condition to clear B2:** specify the mutation boundary and add a deterministic RED
for it.  Before snapshot provenance is emitted, the catalog-aware path must either
prove the catalog fresh and compute provenance from it, refresh/recompute it, or return
the existing conservative `(frozen_start_seq, true)` result.  The RED must force an
earliest-segment removal after the initial catalog observation and before provenance is
finalized, and assert that no snapshot claims complete history.  It must cover v1 and
legacy through the production boundary or pair a deterministic helper oracle with an
entry-point canary.

## Note

`scripts/verify_M-95.sh:3` points to `milestones/M-95-calc-profile.md`, not the audited
`M-95-catalog-once.md`.  Correct the traceability comment with the resubmission.

## Audit evidence

- Subject branch fetched twice: `origin/feat/M-95-catalog-once` stayed at
  `d341c8d38dda75212b89af6f6d437db97e7a81ff`; the mandate SHA is its ancestor.
- Net artifact delta from `6ee3f1a..d341c8d`: `milestones/M-95-catalog-once.md`,
  `crates/gateway-serve/tests/red_m95_catalog_once.rs`, `scripts/verify_M-95.sh`, and
  `scripts/tests/red_verify_M-95_ci_map.sh`.  No `contracts/**` or implementation path
  is in the artifact commit set.
- The M-95 process-boundary oracle is sound on its stated baseline: it observes each
  `.zst` and fails `k1..k4` on the measured 3/6/3/5 catalog openings, respectively; its
  watcher cleanup also left no child process.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-95-catalog-once
d341c8d38dda75212b89af6f6d437db97e7a81ff
exit=0

$ git merge-base origin/main origin/feat/M-95-catalog-once
6ee3f1af60fd1a482842d267d84fc625505c2db4
exit=0

$ git diff --name-status 6ee3f1a..d341c8d
A       crates/gateway-serve/tests/red_m95_catalog_once.rs
A       milestones/M-95-catalog-once.md
A       scripts/tests/red_verify_M-95_ci_map.sh
A       scripts/verify_M-95.sh
exit=0

$ cargo test -p gateway-serve --test red_m95_catalog_once -- --nocapture
running 4 tests
test k1_v1_subscription_walks_catalog_once ... FAILED
  expected 1, got 3 opens of every .zst
test k2_second_subscription_walks_catalog_once_more ... FAILED
  expected 2, got 6 opens of every .zst
test k3_legacy_path_walks_catalog_once ... FAILED
  expected 1, got 3 opens of every .zst
test k4_resubscribe_same_id_walks_catalog_once_more ... FAILED
  expected 2, got 5 opens of every .zst
test result: FAILED. 0 passed; 4 failed
exit=101

$ cargo test -p gateway --test red_segment_meta_bound sm3_first_tick_legitimately_pays_full_price -- --nocapture
running 1 test
test sm3_first_tick_legitimately_pays_full_price ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out
exit=0

$ cargo test -p gateway --test red_m87_history_provenance_failclosed -- --nocapture
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ bash scripts/tests/red_verify_M-95_ci_map.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
PASS  строка дописана в блок базы ⇒ FAIL (C-266 F2) (exit=1)
PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
PASS  (i) check_review_fa + дописка ⇒ исполняется, не SKIP (exit=0)
PASS  (ii) run: |- — литеральный блок, исполняется (exit=0)
PASS  (iii) run: > — складывающий скаляр > ⇒ FAIL (exit=1)
VERDICT: PASS — 10 сценариев
exit=0

$ VERIFY_M95_CI_DRY=1 bash scripts/verify_M-95.sh
PASS  ci-parity: учтено шагов 59 из 59 (исполнено 52, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/verify_M-95.sh
FAIL  task1-3: red_m95_catalog_once (k1…k4) (exit=101)
PASS  I-4: red_m87_history_provenance_failclosed
PASS  I-4: red_segment_meta_bound (SM-*)
PASS  I-4: red_checkpoint_bin_prod_argv
PASS  I-4: red_checkpoint_prefix_pruned
PASS  I-4: journal red_stitch_monotonic (JR-I-11)
PASS  ci-map: проба карты CI-паритета (red_verify_M-95_ci_map.sh; число миров печатает проба)
The subsequent full CI-parity execution was terminated by this critic after the decisive
RED baseline and structural REJECT; it is not reported as a PASS.
terminate_verify_group_exit=0

$ git diff --check 6ee3f1a..d341c8d
exit=0
```
