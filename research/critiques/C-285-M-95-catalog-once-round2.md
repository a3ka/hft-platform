# C-285 — M-95 catalog-once, round 2: REJECT

<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: 6ee3f1af60fd1a482842d267d84fc625505c2db4
audited_head: f37677e41464fcacfdec81a1048cf7bf5dc082a5
verdict: REJECT
-->

## Verdict

**REJECT — dev dispatch remains blocked.** C-284 B1 is closed: `SM-1` and
`SM-3` now measure the honest subscription boundary (`resume + first pump`),
retain the lower bound that rejects `countfake`, and add an independent
one-walk upper bound. C-284 B2, however, is not closed by the new RED suite:
the suite does not prove that its deletion occurs after the subscription's
catalog was built. A pre-build rendezvous and an implementation that never
calls `is_fresh` pass all of `f0…f2` while violating `VB-I-11` in production.

The committed artifact set is otherwise complete for this pre-implementation
round: milestone, two RED suites, adapted M-62 oracle, acceptance script and
CI-map probe are present. `contracts/**` is untouched, hence no T1
contract-RFC is applicable. No public T2 type or trait signature is introduced:
the declared shape may preserve the existing `LiveReducer::resume` signature
by retaining the catalog in the reducer.

## Blocking finding

### B2-R2 — `f1`/`f2` do not prove the claimed mutation ordering

`milestones/M-95-catalog-once.md:76-81` requires the test rendezvous after a
subscription has built its catalog and before provenance is calculated.
`crates/gateway-serve/tests/red_m95_provenance_fresh.rs:222-232`, though,
observes only that a caller has signalled a string channel; it has no witness
that `LiveReducer::resume` (or another catalog-building operation) has already
occurred.

Consequently the following broken implementation passes the proposed suite:

1. signal `m95-catalog:<id>` / `m95-catalog:legacy` immediately **before**
   building the subscription catalog;
2. let `f1`/`f2` remove the oldest segment while the subscription is paused;
3. build the catalog after release and calculate provenance from that catalog
   without `is_fresh` or `refresh`.

The post-deletion catalog correctly reports `history_truncated=true`, so `f1`
and `f2` pass; the no-deletion paired world `f0` also passes. Yet a real
retention deletion after catalog construction leaves the cached catalog stale
and can send `history_truncated=false`, which is exactly the `VB-I-11` failure
that C-284 B2 identified. The test therefore accepts the mutation it says it
rejects.

**Condition to clear:** the architect-owned RED must make the ordering
observable, so it fails both for (a) a rendezvous before catalog construction
and (b) provenance calculated from a stale catalog without a freshness check.
The test may use any suitable production-boundary witness; the required
property is that deletion is demonstrated to occur after the initial catalog
observation, not merely asserted in comments or channel naming.

## Audit notes

- `JR-I-11` remains explicitly protected by the existing `SM-*` and
  `red_stitch_monotonic` oracles; `VB-I-11` is the live invariant for this
  finding.
- The local full CI-parity run exposed an unrelated host-fixture failure in
  `red_runbook_markers.sh`: its intentional non-Git fixture is below `/tmp`,
  but this host has `/tmp/.git`. The M-95 range does not change that probe; it
  is recorded below and is not attributed to M-95.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-95-catalog-once
f37677e41464fcacfdec81a1048cf7bf5dc082a5
exit=0

$ git merge-base origin/main origin/feat/M-95-catalog-once
6ee3f1af60fd1a482842d267d84fc625505c2db4
exit=0

$ git merge-base --is-ancestor f37677e41464fcacfdec81a1048cf7bf5dc082a5 origin/feat/M-95-catalog-once
exit=0

$ git diff --name-status 6ee3f1a..f37677e
A       crates/gateway-serve/tests/red_m95_catalog_once.rs
A       crates/gateway-serve/tests/red_m95_provenance_fresh.rs
M       crates/gateway/tests/red_segment_meta_bound.rs
A       milestones/M-95-catalog-once.md
A       research/critiques/C-284-M-95-catalog-once.md
A       scripts/tests/red_verify_M-95_ci_map.sh
A       scripts/verify_M-95.sh
exit=0

$ cargo test -p gateway --test red_segment_meta_bound -- sm1 sm3
running 3 tests
test sm10_foreign_file_does_not_break_session ... ok
test sm1_counter_measures_operations_not_calls ... ok
test sm3_first_tick_legitimately_pays_full_price ... ok
test result: ok. 3 passed; 0 failed; 7 filtered out
exit=0

$ cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh
running 3 tests
test f0_v1_without_retention_history_is_complete ... FAILED
test f1_v1_retention_between_catalog_and_provenance_is_honest ... FAILED
test f2_legacy_retention_between_catalog_and_provenance_is_honest ... FAILED
test result: FAILED. 0 passed; 3 failed
failure point: red_m95_provenance_fresh.rs:224 test_wait_for_pump(m95-catalog:*)
exit=101  # expected RED: M-95 implementation/rendezvous is absent

$ bash scripts/verify_M-95.sh
FAIL  task1-3: red_m95_catalog_once (k1…k4) (exit=101)
FAIL  task1,3 / I-5: red_m95_provenance_fresh (f0…f2, --features testing — CI их не гоняет) (exit=101)
PASS  I-4: red_m87_history_provenance_failclosed
PASS  I-4: red_segment_meta_bound (SM-*)
PASS  I-4: red_checkpoint_bin_prod_argv
PASS  I-4: red_checkpoint_prefix_pruned
PASS  I-4: journal red_stitch_monotonic (JR-I-11)
PASS  ci-map: проба карты CI-паритета (red_verify_M-95_ci_map.sh; число миров печатает проба)
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
FAIL  ci-parity: cargo test --all (exit=101; expected M-95 k1…k4 RED)
FAIL  ci-parity: bash scripts/tests/red_runbook_markers.sh (exit=1)
FAIL  S17b SETUP НЕ СОСТОЯЛСЯ: каталог внутри git
PASS  ci-parity: учтено шагов 59 из 59 (исполнено 52, исключено по карте 6)
SKIP  task4: §8-замер на проде — reviewer
VERDICT: FAIL (провалов: 4)
exit=1  # expected pre-implementation RED; runbook S17b is host-specific extra failure

$ git -C /tmp rev-parse --show-toplevel --git-dir
/tmp
.git
exit=0

$ bash scripts/next_artifact_id.sh C; bash scripts/reserve_artifact_id.sh C
C-285 reserved through refs/reserved/C-285
exit=0

$ git ls-remote --refs origin refs/reserved/C-285
69c6bfccf16cef18c8e302bcd70622dabbf04736    refs/reserved/C-285
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-06T14:22Z
- Milestone: M-95-catalog-once (TD-229)
- Статус: BLOCKED — second consecutive REJECT in C-284 B2 provenance-honesty area
- HEAD: f37677e — test(M-95): provenance RED gate [architect]

## §B — Что я сделал
- Audited the branch tip fetched from `origin/feat/M-95-catalog-once`, not the handoff SHA alone.
- Re-ran the adapted M-62 oracle, the new feature-gated RED baseline, and the M-95 acceptance script.

## §C — Артефакты / результаты
- `research/critiques/C-285-M-95-catalog-once-round2.md`
- Done Block: `red_segment_meta_bound` exit=0; `red_m95_provenance_fresh` exit=101 expected RED; `verify_M-95.sh` exit=1 expected pending RED plus recorded host-fixture failure.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter` (fresh strong-model context)
- **Paste-ready промпт:**
  ```
  Resolve the M-95 round-2 dispute on origin/feat/M-95-catalog-once. Read C-284 and C-285,
  milestones/M-95-catalog-once.md, crates/gateway-serve/tests/red_m95_provenance_fresh.rs,
  docs/fa/viz-backend.md VB-I-11, and the current gateway-serve entry points. Decide whether
  f0/f1/f2 actually force deletion after construction of the subscription catalog, or whether a
  pre-build rendezvous plus no is_fresh check passes them. State the required next action and
  commit A-NNN verdict to this subject branch. This is arbitration because C-285 is the second
  REJECT in the C-284 B2 provenance-honesty area.
  ```
- Push-статус: pending — commit and push this critic verdict before dispatch.
- Кэш: pending — remove `/tmp/hft-critic2-m95/target` after verdict push.

## §E — Риски / открытые вопросы
- The M-95 provenance oracle is feature-gated and CI does not execute it; acceptance is its only automatic execution path.
- The host has `/tmp/.git`, making one unrelated runbook fixture invalid locally.

=== END HANDOFF ===
