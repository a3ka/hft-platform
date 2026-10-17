# C-010 — M-09 Task 2 recon-redesign critic verdict

**Date:** 2026-07-17T16:52:18Z  
**Agent:** critic  
**Audited HEAD:** `c5c59e3` (`origin/feat/M-09-task2`)  
**Audited commits:** `41cf6a5` + `c5c59e3`  
**Scope:** M-09 Task 2 recon-redesign after §8 live flood (`ReconDivergence` on healthy market)

## Verdict

**APPROVE**

## Summary

The redesign is internally consistent and executable. `red_recon_live.rs` now models live-mode
two-source asymmetry instead of comparing a book to itself: shallow reference vs deeper local,
timing skew, near-book corruption, and empty reference are all represented. The current implementation
fails exactly the expected live-mode RED cases; a narrow prototype of the intended `ops::recon`
behavior turns the full recon suite green; negative mutations confirm the oracles are not placebo.

No blocking findings.

## §A — RED integrity

### Current implementation RED run

Command:

```bash
cargo test -p ops --test red_recon_live
```

Result: **expected FAIL** with exactly 3 failing tests:

- `recon_bands_are_shallow_within_rest_reach`
- `best_price_timing_skew_is_tolerated`
- `deep_local_vs_truncated_reference_does_not_flood`

The other 4 tests pass on current implementation:

- `real_desync_best_moved_ten_bps_still_diverges`
- `near_book_eviction_within_reach_diverges`
- `near_touch_phantom_within_reach_diverges`
- `empty_reference_is_not_silently_ok`

This is the desired RED shape: current `RECON_BANDS=[1.5%,3%,8%]`, exact best-price compare, and
no depth-aware skip fail; real near-book damage and empty reference are already guarded.

### Live-mode fixture quality

Evidence:

- `crates/ops/tests/red_recon_live.rs:24` states the fixture models different representations,
  depths, and moments, not book-to-self.
- `crates/ops/tests/red_recon_live.rs:97` models sub-bp best-price timing skew.
- `crates/ops/tests/red_recon_live.rs:147` models deep local vs truncated reference.
- `crates/ops/tests/red_recon_live.rs:176` and `:192` guard near-book damage.
- `crates/ops/tests/red_recon_live.rs:213` guards empty reference not becoming silence.

The new testing rule is aligned:

- `.claude/rules/testing.md:204` introduces the live-mode two-source RED requirement.
- `.claude/rules/testing.md:217-226` requires different representations, different depth,
  different moments, and anti-placebo in both directions.

## §B — Reachability

I temporarily prototyped only `crates/ops/src/recon.rs` with:

- `RECON_BANDS=[0.001,0.003,0.005]`
- best-price tolerance (`BEST_SKEW_BPS`)
- skip when `reference.max_reach_pct(side) < band`
- empty reference remains best divergence

Command:

```bash
cargo test -p ops --test red_ops_recon --test red_recon_live --test red_recon_sink
```

Result: **16/16 PASS**

- `red_ops_recon`: 6 passed
- `red_recon_live`: 7 passed
- `red_recon_sink`: 3 passed

The prototype was reverted before this verdict was written.

## §C — Mutation checks

All mutations were temporary and reverted.

1. **No-skip mutation**: disabled `reference.max_reach_pct` skip.
   Command:
   ```bash
   cargo test -p ops --test red_recon_live deep_local_vs_truncated_reference_does_not_flood
   ```
   Result: expected FAIL; `deep_local_vs_truncated_reference_does_not_flood` caught flood
   (`divergence_bps=26315`).

2. **Over-skip mutation**: disabled best divergence and skipped all depth bands.
   Command:
   ```bash
   cargo test -p ops --test red_recon_live
   ```
   Result: expected FAIL in 4 tests: empty reference, near-book eviction, near-touch phantom,
   and 10 bps best desync.

3. **Empty-reference silence mutation**: treated `Some`/`None` best mismatch as non-divergence.
   Command:
   ```bash
   cargo test -p ops --test red_recon_live empty_reference_is_not_silently_ok
   ```
   Result: expected FAIL.

4. **Migrated fixture anti-placebo**: `reconcile` returns always-silent outcome.
   Commands:
   ```bash
   cargo test -p ops --test red_ops_recon --test red_recon_sink
   cargo test -p ops --test red_recon_sink
   ```
   Result: expected FAIL. `red_ops_recon` failed 4/6 (missing best, multiple levels, empty local,
   ε_test); `red_recon_sink` failed 2/3 (emit + metrics).

## §D — FA / milestone / verify audit

PASS.

- `docs/fa/ops.md:80-87` scopes OPS-I-1 to reference-covered near-book depth and explicitly
  skips bands outside reference reach.
- `docs/fa/ops.md:102-108` aligns `ε_test` with `BEST_SKEW_BPS` and near-book bands.
- `docs/fa/ops.md:123-131` documents the measured Binance REST reach and rejects the bucket/raw
  diagnosis by measurement.
- `docs/fa/ops.md:133-149` gives the founder-selected design and honestly separates 6-60% deep
  bands into a later track.
- `docs/fa/ops.md:151-153` points the RED oracle to `red_recon_live.rs`.
- `milestones/M-09-data-safety-net.md:68-87` records the §8 failure, founder-selected near-book
  redesign, ops-only implementation scope, and separate deep-book track.
- `scripts/verify_M-09.sh:31-32` adds `red_recon_live` to the real M-09 gate.

One minor documentation mismatch remains pre-existing in the task row wording:
`milestones/M-09-data-safety-net.md:62` still says Task 2 impl is `venue-dev + engine-dev`.
The explicit redesign block at `:77-80` supersedes it and says the fix is only
`crates/ops::recon` / engine-dev, with venue wiring untouched. I do not treat this as blocking
because the later, specific section is unambiguous.

## §E — Scope audit

PASS.

Changed files:

- `.claude/rules/testing.md`
- `crates/ops/tests/red_ops_recon.rs`
- `crates/ops/tests/red_recon_live.rs`
- `crates/ops/tests/red_recon_sink.rs`
- `docs/fa/ops.md`
- `milestones/M-09-data-safety-net.md`
- `scripts/verify_M-09.sh`

No touched paths under:

- `crates/risk/**`
- `crates/killswitch/**`
- `crates/oms/**`
- `crates/venue-*/**`
- `crates/contracts/**`
- `deploy/**`
- `research/registry/**`
- `research/decisions/**`

Forbidden-surface grep found no order-egress/risk/killswitch/oms code path. `OrderBook` mentions
are test fixture type names only.

## §F — Gate behavior

Command:

```bash
bash scripts/verify_M-09.sh
```

Result: expected **VERDICT: FAIL (1)** in RED phase:

- PASS: CT-RFC-03 tests
- PASS: schema parity
- PASS: original OPS-I-1 degraded inputs
- FAIL: new `red_recon_live`
- PASS: OPS-I-9 budget
- PASS: metrics/silence
- PASS: OPS-I-6 no runtime journal dependency
- PASS: OPS-I-5 parity checks

This is correct before engine-dev implements the recon redesign.

## Findings

### Blocking

None.

### Notes

- **N1:** Task-row wording still mentions `venue-dev`, but the new Task 2 redesign block narrows
  the fix to `crates/ops::recon` / engine-dev and explicitly says venue-dev/wiring are untouched.

## Recommended next action

Proceed to **engine-dev** for the narrow `ops::recon` implementation only.

## Handoff

=== HANDOFF: critic -> engine-dev ===

### §A — Metadata

- UTC datetime: 2026-07-17T16:52:18Z
- From agent: critic
- Milestone: M-09 Task 2 recon-redesign
- Verdict: APPROVE
- Audited branch/head: `origin/feat/M-09-task2 @ c5c59e3`
- Critic verdict file: `research/critiques/C-010-M-09-recon-redesign.md`

### §B — What I checked

- RED integrity of `red_recon_live.rs`
- Temporary reachability prototype for `ops::recon`
- Negative mutations: no-skip, over-skip, empty-reference silence, always-silent
- FA §4.2 / OPS-I-1 / milestone / verify gate alignment
- Scope: no `risk`, `killswitch`, `oms`, `venue-*`, contracts, order-egress

### §C — Outcomes

- APPROVE
- Current RED: `cargo test -p ops --test red_recon_live` fails exactly 3 intended tests.
- Prototype reachability: 16/16 recon tests pass, then prototype reverted.
- `bash scripts/verify_M-09.sh` fails only the new live-mode recon test, as expected in RED phase.

### §D2 — Paste-ready prompt for engine-dev

```text
Ты — engine-dev проекта hft-platform. Рабочий каталог /home/nous/hft-platform.

Branch: feat/M-09-task2. Bootstrap in your own worktree from latest origin/feat/M-09-task2.
Read first:
- CLAUDE.md
- docs/DESIGN.md
- docs/04-workflow.md
- .claude/rules/scope-guard.md
- .claude/rules/testing.md
- docs/fa/ops.md §4 / §4.2 / §6 OPS-I-1
- milestones/M-09-data-safety-net.md Task 2 redesign block
- research/critiques/C-010-M-09-recon-redesign.md

Task: implement ONLY the near-book recon redesign in crates/ops/src/recon.rs.

Allowed paths:
- crates/ops/src/recon.rs only, unless compilation forces a strictly ops-local helper under crates/ops/src.

Forbidden:
- Do not touch crates/venue-*/src, crates/risk, crates/killswitch, crates/oms, crates/contracts.
- Do not edit tests, docs, milestones, scripts/verify_M-09.sh, or research/critiques.
- No order-egress or venue REST changes; architect/critic determined the fix is ops::recon only.

Implementation contract:
- RECON_BANDS = [0.001, 0.003, 0.005].
- Add best-price skew tolerance BEST_SKEW_BPS: sub-bp skew must not set best_price_diverged; 10 bps desync must still diverge.
- For each side/band, skip comparison when reference.max_reach_pct(side) < band.
- Empty/one-sided reference against live local must not become silent OK; best mismatch remains divergence.
- Preserve ReconThresholds / EPS_TEST_BPS / EPS_PROD_DEFAULT_BPS / EPS_MAX_BPS fail-closed semantics.

Required verification:
- cargo test -p ops --test red_recon_live
- cargo test -p ops --test red_ops_recon
- cargo test -p ops --test red_recon_sink
- bash scripts/verify_M-09.sh

Expected after implementation:
- red_recon_live: 7/7 PASS
- red_ops_recon: 6/6 PASS
- red_recon_sink: 3/3 PASS
- verify_M-09.sh may still fail on later unimplemented M-09 tasks; if so, report exact remaining FAIL lines.

Commit:
- One atomic commit, e.g. `fix(M-09): task 2 — implement near-book recon`
- Push to origin/feat/M-09-task2 only after checking `git log origin/feat/M-09-task2..HEAD --oneline` contains only your commit.

Return Done Block with raw command outputs and Handoff to tester/reviewer per workflow.
```

### §E — Outstanding risks

- Deep 6-60% OBI/recon remains deliberately out of M-09 scope; separate founder track required.
- Engine-dev should keep implementation in `ops::recon`; touching venue wiring would reopen scope.

=== END HANDOFF ===
