# C-012-M-09-seed-gate — critic verdict

UTC: 2026-07-18T16:08:24Z  
Branch: `origin/feat/M-09-task2 @ 676cb3a`  
Worktree: `/tmp/hft-critic-m09seed`  
Scope: M-09 task 2, defect A only (seed-race). Volume defect B / oracles 1-7 are under founder fork and were not semantically audited.

## Verdict

**REJECT**

The seed-gate direction is correct and reachable, but the RED oracle set does not pin the full architect contract. It catches "no alert before seed" and "do alert on post-seed empty local", but it does not catch a bad implementation that suppresses the startup best-price alert while still feeding the empty local into the volume windows before seed.

That violates the documented invariant in `docs/fa/ops.md §4.3.1` and `milestones/M-09`: before seed, `observe` must return no-alert **and must not feed the window**.

## Findings

### B1 — Missing oracle for pre-seed window poisoning

Severity: BLOCKER

A temporary bad implementation that:

- adds `seeded: bool`,
- returns `no-alert` for unseeded empty local,
- but still runs the existing window-feed path for that empty local before suppressing the verdict,

passes the entire `red_recon_window` suite:

```text
$ cargo test -p ops --test red_recon_window; echo exit=$?
running 10 tests
test empty_local_after_seed_is_corruption_and_emits ... ok
test empty_local_before_first_seed_does_not_emit ... ok
test unreachable_band_is_skipped_not_flooded ... ok
test churn_with_same_sign_run_stays_silent ... ok
test persistent_volume_deficit_alerts ... ok
test near_book_eviction_persists_then_alerts ... ok
test persistent_volume_surplus_alerts ... ok
test volume_timing_skew_does_not_alert ... ok
test windowed_eps_test_not_calibratable ... ok
test detector_is_deterministic_across_replay ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

exit=0
```

Why this matters: pre-seed empty cycles against a full REST reference produce large signed volume deficits. If a bad implementation stores those samples while suppressing only the immediate alert, the detector is "seeded" later with a poisoned window and can emit a false window alert on the first real local snapshot. The current tests never run the sequence:

```text
empty local before seed for K-1/K cycles -> first non-empty local identical to reference
```

so they do not prove the "НЕ кормит окно" part of the contract.

Required architect fix: add a sacred RED oracle such as `empty_local_before_seed_does_not_poison_window`:

```text
1. Create fresh detector.
2. Observe empty local vs full reference for RECON_WINDOW - 1 or RECON_WINDOW cycles; assert no alert.
3. Observe first non-empty local identical to reference; assert no alert and best_price_diverged=false.
4. This must fail under the bad "feed window then suppress verdict" mutation.
```

After that, rerun critic on defect A. The existing 9a/9b tests should remain.

## Done Block

Status: REJECT  
Audited commits: `2ea5937` + `676cb3a` over `9db808c`  
Temporary mutations: reverted; only this verdict file remains modified.  
Push status: reported by critic final response after push-scope.

## Checks

Scope check passed:

```text
$ git diff --name-only 9db808c..676cb3a
crates/ops/tests/red_recon_window.rs
docs/fa/ops.md
milestones/M-09-data-safety-net.md
```

No audited production code / recorder / contracts / risk / killswitch / oms paths were touched in `9db808c..676cb3a`.

Current RED behavior passed the intended shape:

```text
$ cargo test -p ops --test red_recon_window empty_local; echo exit=$?
running 2 tests
test empty_local_after_seed_is_corruption_and_emits ... ok
test empty_local_before_first_seed_does_not_emit ... FAILED

failures:
    empty_local_before_first_seed_does_not_emit

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s

exit=101
```

Full current `red_recon_window` is isolated to 9a:

```text
$ cargo test -p ops --test red_recon_window; echo exit=$?
running 10 tests
test empty_local_after_seed_is_corruption_and_emits ... ok
test unreachable_band_is_skipped_not_flooded ... ok
test churn_with_same_sign_run_stays_silent ... ok
test empty_local_before_first_seed_does_not_emit ... FAILED
test near_book_eviction_persists_then_alerts ... ok
test persistent_volume_deficit_alerts ... ok
test volume_timing_skew_does_not_alert ... ok
test persistent_volume_surplus_alerts ... ok
test windowed_eps_test_not_calibratable ... ok
test detector_is_deterministic_across_replay ... ok

failures:
    empty_local_before_first_seed_does_not_emit

test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

exit=101
```

Reachability check passed with a temporary correct self-seeding implementation (`seeded: bool`, set on first local with `best_bid` or `best_ask`, return before `reconcile`/window feed while unseeded and empty):

```text
$ cargo test -p ops --test red_recon_window; echo exit=$?
running 10 tests
test empty_local_after_seed_is_corruption_and_emits ... ok
test empty_local_before_first_seed_does_not_emit ... ok
test churn_with_same_sign_run_stays_silent ... ok
test near_book_eviction_persists_then_alerts ... ok
test unreachable_band_is_skipped_not_flooded ... ok
test persistent_volume_surplus_alerts ... ok
test persistent_volume_deficit_alerts ... ok
test windowed_eps_test_not_calibratable ... ok
test volume_timing_skew_does_not_alert ... ok
test detector_is_deterministic_across_replay ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

exit=0
```

```text
$ cargo test -p ops; echo exit=$?
red_ops_budget: 5 passed
red_ops_metrics: 6 passed
red_ops_recon: 5 passed
red_recon_live: 5 passed
red_recon_sink: 4 passed
red_recon_window: 10 passed
Doc-tests ops: 0 passed

exit=0
```

Required anti-placebo passed: an over-suppress mutation that always returns no-alert on empty local fails 9b and leaves the rest of the suite green.

```text
$ cargo test -p ops --test red_recon_window empty_local; echo exit=$?
running 2 tests
test empty_local_before_first_seed_does_not_emit ... ok
test empty_local_after_seed_is_corruption_and_emits ... FAILED

failures:
    empty_local_after_seed_is_corruption_and_emits

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s

exit=101
```

```text
$ cargo test -p ops --test red_recon_window; echo exit=$?
running 10 tests
test empty_local_before_first_seed_does_not_emit ... ok
test empty_local_after_seed_is_corruption_and_emits ... FAILED
test churn_with_same_sign_run_stays_silent ... ok
test unreachable_band_is_skipped_not_flooded ... ok
test near_book_eviction_persists_then_alerts ... ok
test persistent_volume_deficit_alerts ... ok
test volume_timing_skew_does_not_alert ... ok
test windowed_eps_test_not_calibratable ... ok
test persistent_volume_surplus_alerts ... ok
test detector_is_deterministic_across_replay ... ok

failures:
    empty_local_after_seed_is_corruption_and_emits

test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

exit=101
```

Determinism check passed at design/code-contract level: seed is sequence state, and the inspected design/RED text keeps the invariant as "same observation sequence -> same verdict sequence"; no wall-clock/rand source is introduced or required.

All temporary mutations were reverted:

```text
$ git status --short --branch
## HEAD (no branch)
```

## Handoff

**Next agent:** architect  

Patch the RED suite before engine-dev implementation. Add an oracle that fails if pre-seed empty local samples are written into the window, while preserving the existing 9a/9b behavior:

- 9a: startup empty local before first seed is silent.
- 9b: empty local after seed emits as real corruption.
- New required pin: startup empty local does not poison the window; first real local snapshot identical to reference remains silent after a run of pre-seed empty observations.

After the new RED fails under the poisoned-window mutation and the current scope remains limited to architect-owned tests/docs, return to critic re-audit.
