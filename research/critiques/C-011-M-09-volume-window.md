# C-011 — M-09 Task 2 volume-window recon critic verdict

**Date:** 2026-07-17T21:58:31Z  
**Agent:** critic  
**Audited branch/head:** `origin/feat/M-09-task2 @ 4b56cd3`  
**Local audit worktree:** `/tmp/hft-critic-m09vol` (`critic/M-09-task2-volume-window`, tracking `origin/feat/M-09-task2`)  
**Audited commits:** `fef36c3` + `4b56cd3`  
**Scope:** M-09 Task 2, second §8 failure: volume timing-skew -> windowed persistence

## Verdict

**REJECT**

## Summary

The new volume-window oracle itself is sound: it models live volume-skew as a sequence of moments, not book-to-self; churn and corruption have the same per-cycle magnitude and differ only by sign persistence; a narrow temporary implementation made the full recon suite pass 21/21; and negative mutations caught both too-strict and too-soft implementations.

The milestone is not dispatchable yet because two plan-time gates are incomplete:

1. `scripts/verify_M-09.sh` does not run `red_recon_sink`, so the final acceptance gate can go green while the new stateful sink contract remains `todo!()`.
2. The plan requires engine-dev to rewire `crates/recorder/src/main.rs`, but current scope text does not allow engine-dev to touch `crates/recorder/**`.

## §A — RED integrity

### Current skeleton RED shape

Command:

```bash
cargo test -p ops --test red_recon_window
```

Result: expected RED, **0/7 PASS**. All 7 tests panic at `crates/ops/src/recon.rs:244` (`todo!()` in `ReconDetector::observe`).

Command:

```bash
cargo test -p ops --test red_recon_sink
```

Result: expected RED, **0/4 PASS**. All 4 tests panic at `crates/ops/src/sink.rs:69` (`todo!()` in `handle_recon_snapshot`).

Command:

```bash
cargo test -p ops --test red_recon_live --test red_ops_recon
```

Result: expected GREEN, **10/10 PASS** (`red_ops_recon` 5/5, `red_recon_live` 5/5). The prior near-book best/depth behavior remains intact.

### Fixture quality

PASS.

Evidence:

- `crates/ops/tests/red_recon_window.rs:87-109` models healthy timing-skew churn as alternating volume sign, not book-to-self.
- `crates/ops/tests/red_recon_window.rs:116-160` covers a real measured short same-sign run (`---`) while keeping the full window balanced.
- `crates/ops/tests/red_recon_window.rs:167-201` covers persistent deficit and persistent surplus.
- `crates/ops/tests/red_recon_window.rs:208-247` covers within-reach non-best eviction.
- `crates/ops/tests/red_recon_window.rs:37-39`, `:90-95`, `:170-172`, and `:193-194` use the same per-cycle magnitude (15% => about 1500 bps) for churn and corruption; only persistence differs.

## §B — Reachability

I temporarily implemented only:

- `crates/ops/src/recon.rs`: stateful per `(band, side)` window with signed mean, `reference.max_reach_pct(side)` skip, threshold `EPS_TEST_BPS.min(thr.prod_bps())`, best-price immediate via `reconcile`.
- `crates/ops/src/sink.rs`: call `detector.observe`, update `book_divergence_bps`, emit `Sys(ReconDivergence)` and increment `book_resync_total` only on `verdict.alert`.

Command:

```bash
cargo test -p ops --test red_recon_window --test red_recon_sink --test red_recon_live --test red_ops_recon
```

Result: **21/21 PASS**:

- `red_ops_recon`: 5/5
- `red_recon_live`: 5/5
- `red_recon_sink`: 4/4
- `red_recon_window`: 7/7

The temporary implementation was reverted before writing this verdict.

## §C — Anti-placebo mutations

All mutations were temporary and reverted.

### Mutation A — per-cycle volume trigger

Change: full-window guard mutated from `window.len() == RECON_WINDOW` to effectively `len >= 1`.

Command:

```bash
cargo test -p ops --test red_recon_window
```

Result: expected profile, **5/7 PASS**:

- FAILED: `volume_timing_skew_does_not_alert`
- FAILED: `churn_with_same_sign_run_stays_silent`
- PASSED: persistent deficit, persistent surplus, near-book eviction, windowed ε_test, deterministic replay

This confirms the oracle catches the too-strict implementation that would reproduce §8 live flood.

### Mutation B — always-silent volume path

Change: volume persistence alert disabled.

Command:

```bash
cargo test -p ops --test red_recon_window
```

Result: expected profile, **2/7 PASS**:

- PASSED: both churn silence tests
- FAILED: `persistent_volume_deficit_alerts`
- FAILED: `persistent_volume_surplus_alerts`
- FAILED: `near_book_eviction_persists_then_alerts`
- FAILED: `windowed_eps_test_not_calibratable`
- FAILED: `detector_is_deterministic_across_replay` setup guard

This confirms the oracle catches the too-soft implementation that suppresses real persistent corruption.

## §D — Scope / T1 / Class A doc audit

### Scope and T1

PASS for forbidden surfaces.

`git diff --name-only fef36c3^..4b56cd3` touches:

- `.claude/rules/testing.md`
- `crates/ops/src/recon.rs`
- `crates/ops/src/sink.rs`
- `crates/ops/tests/red_ops_recon.rs`
- `crates/ops/tests/red_recon_live.rs`
- `crates/ops/tests/red_recon_sink.rs`
- `crates/ops/tests/red_recon_window.rs`
- `docs/fa/ops.md`
- `milestones/M-09-data-safety-net.md`
- `scripts/verify_M-09.sh`

No diff under `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**`, `crates/contracts/**`, `contracts/**`, `research/registry/**`, or `research/decisions/**`.

T1 is unchanged. The existing `ReconAudit` shape remains sufficient for windowed alerts: `best_price_diverged=false` plus `divergence_bps=|signed_mean|`. CT-RFC is not required for this redesign.

### Class A docs

Mostly PASS.

- `docs/fa/ops.md:155-229` documents the measured second §8 failure and the stateful signed-window design.
- `docs/fa/ops.md:260-270` updates `OPS-I-1` to best immediate + volume window.
- `milestones/M-09-data-safety-net.md:89-124` records the second §8 failure, founder direction, implementation scope, T1 non-impact, and §8 acceptance boundary.
- `.claude/rules/testing.md` now requires two-source RED fixtures to model live mode, including sequences for mean-reverting volume.

The two blockers below make the current Class A artifact set non-dispatchable.

## Findings

### BLOCKING

**B1 — Acceptance script omits the new stateful sink oracle.**

Evidence:

- `scripts/verify_M-09.sh:28-38` runs `red_ops_recon`, `red_recon_live`, `red_recon_window`, budget, and metrics, but does not run `cargo test -p ops --test red_recon_sink`.
- `crates/ops/src/sink.rs:43-69` is a `todo!()` skeleton.
- Direct run proves `red_recon_sink` is RED: `cargo test -p ops --test red_recon_sink` => 0/4 PASS.

Worst realistic case: engine-dev implements `ReconDetector::observe` only. `verify_M-09.sh` can then go green while the actual `handle_recon_snapshot` emission path still panics or no-ops. That is exactly the class this milestone is supposed to prevent: a green gate over a non-running path.

Suggested fix: add `cargo test -p ops --test red_recon_sink` to the Task 2 section of `scripts/verify_M-09.sh`, with wording that names the stateful sink/emission path. Re-run critic after that small change.

**B2 — Recorder rewire is required but not in engine-dev scope.**

Evidence:

- `milestones/M-09-data-safety-net.md:112-114` says engine-dev rewires `crates/recorder/src/main.rs` by hoisting `ReconDetector::new(thr)` before the loop and passing `&mut detector` to `handle_recon_snapshot`.
- `.claude/rules/scope-guard.md:12` defines engine-dev scope as `crates/{journal,book,oms,sim,runner,alpha,portfolio,strategy,ops}/src/**` plus `deploy/**`; it does not include `crates/recorder/**`.
- `milestones/M-09-data-safety-net.md:35-54` Allowed paths do not add a Task-2 carve-out for `crates/recorder/src/main.rs`.
- `cargo check -p recorder` fails exactly at `crates/recorder/src/main.rs:173` because the call still uses the old sink signature.

This compile break is acceptable as feat-branch RED state only if the follow-up dev has an explicit, reviewable path allowance to fix it. As written, engine-dev would have to violate scope or stop with a SCOPE VIOLATION REQUEST.

Suggested fix: either add an explicit M-09 Task 2 Allowed-path carve-out for `engine-dev` to edit only `crates/recorder/src/main.rs` for the three-line recon-loop rewire, or split the API/signature change so recorder remains compiling until a separate authorized step.

## Notes

- The volume-window oracle answers the key question correctly: it models live skew as a sequence of asynchronous moments with sign churn, not a book compared with itself.
- Anti-placebo coverage is present in both directions: per-cycle strictness floods churn; always-silent misses persistent corruption.
- The `recorder` compile failure is localized. `rg "handle_recon_snapshot\\(" -n` finds only the sink definition, sink tests, and `crates/recorder/src/main.rs`.
- The Class A doc direction is otherwise coherent with `docs/fa/ops.md` §4.3 and `.claude/rules/testing.md`.

## Recommended next action

Architect should make a narrow revision commit:

1. Add `red_recon_sink` to `scripts/verify_M-09.sh`.
2. Add an explicit Task-2 scope allowance for `crates/recorder/src/main.rs` (or change the split so recorder does not require engine-dev scope).

Then re-run critic. No implementation dispatch before these two are fixed.

## Handoff

=== HANDOFF: critic -> architect ===

### §A — Metadata

- UTC datetime: 2026-07-17T21:58:31Z
- From agent: critic
- Milestone: M-09 Task 2 volume-window recon
- Verdict: REJECT
- Audited branch/head: `origin/feat/M-09-task2 @ 4b56cd3`
- Critic verdict file: `research/critiques/C-011-M-09-volume-window.md`

### §B — What I checked

- RED integrity of `red_recon_window` and `red_recon_sink` against current `todo!()` skeletons
- Reachability via temporary correct implementation (21/21 PASS), then reverted
- Anti-placebo mutations in both directions
- Scope/T1 forbidden surfaces
- Class A doc consistency across `docs/fa/ops.md`, `milestones/M-09-data-safety-net.md`, `.claude/rules/testing.md`
- Recorder compile break locality

### §C — Outcomes

- REJECT due to two plan-time blockers:
  - `verify_M-09.sh` does not run `red_recon_sink`
  - recorder rewire is required but not explicitly allowed for engine-dev
- Oracle semantics themselves are approved as reachable and non-placebo.

### §D — Paste-ready prompt for architect

```text
Ты — architect проекта hft-platform. Рабочий каталог /home/nous/hft-platform.

Branch: feat/M-09-task2. Bootstrap in your own worktree from latest origin/feat/M-09-task2.
Read:
- CLAUDE.md
- .claude/rules/gates.md §3/§9
- .claude/rules/scope-guard.md
- milestones/M-09-data-safety-net.md Task 2 second §8 block
- research/critiques/C-011-M-09-volume-window.md

Task: address critic REJECT C-011 with a narrow plan-time revision only.

Required changes:
1. Add `cargo test -p ops --test red_recon_sink` to `scripts/verify_M-09.sh` under Task 2, naming the stateful sink/emission path.
2. Add an explicit Task-2 Allowed-path carve-out for engine-dev to edit ONLY `crates/recorder/src/main.rs` for the planned recon-loop rewire (`ReconDetector::new(thr)` before loop, pass `&mut detector`, remove old `thr` argument), OR redesign the split so recorder stays compiling without an engine-dev recorder touch.

Do not implement `ReconDetector::observe` or sink behavior; this is a plan/spec gate fix.

After commit, hand back to critic for re-audit. Include commit SHA and updated milestone path.
```

### §E — Outstanding risks

- None beyond the two REJECT blockers. The volume-window oracle design itself is reachable and anti-placebo.

=== END HANDOFF ===

---

## Re-audit 3b19a0c

**Date:** 2026-07-17T22:09:47Z  
**Agent:** critic  
**Audited branch/head:** `origin/feat/M-09-task2 @ 3b19a0c`  
**Audited revision commit:** `3b19a0c` (on top of `cbf011f`)  
**Scope:** narrow re-audit of prior C-011 blockers B1/B2 only

### Verdict

**APPROVE**

### B1 — verify gates the stateful sink oracle

PASS.

Evidence:

```bash
grep -n "red_recon_sink" scripts/verify_M-09.sh
# 36:  cargo test -p ops --test red_recon_sink
```

`scripts/verify_M-09.sh:33-36` now runs both the window detector oracle and the stateful
sink/emission oracle under Task 2. The gate no longer goes green while `handle_recon_snapshot`
is still `todo!()`.

Command:

```bash
bash scripts/verify_M-09.sh; echo exit=$?
```

Result: expected RED-phase failure:

- FAIL: `red_recon_window`
- FAIL: `red_recon_sink`
- final line: `VERDICT: FAIL (2)`
- exit: `1`

This closes C-011 B1.

### B2 — recorder carve-out

PASS.

Evidence:

- `milestones/M-09-data-safety-net.md:48-57` explicitly grants engine-dev a Task-2 carve-out for
  `crates/recorder/src/{main.rs,recon_loop.rs}`.
- The carve-out names the planned `main.rs` rewire: hoist `ReconDetector::new(thr)` before
  `while let Some(reference)`, pass `&mut detector` into `handle_recon_snapshot`, and remove the
  old `thr` argument.
- The same block limits the carve-out to recon-loop wiring / MD-only feeder work and explicitly
  keeps the journal-write path and other recorder logic out of scope.
- `milestones/M-09-data-safety-net.md:122-125` repeats the same engine-dev rewire note and points
  back to Allowed paths.

Assessment:

- The carve-out is sufficient for the required `main.rs` state hoist and sink signature rewire.
- Including `recon_loop.rs` is acceptable because the milestone text limits it to `apply_md_to_books`
  / MD-book feeder work, not journal writing.
- It does not authorize order-egress, `risk`, `killswitch`, `oms`, T1, or broader recorder changes.
  Reviewer Block-scope has a concrete check: any recorder diff must be a subset of those two files
  and must not touch journal-write/order path.

This closes C-011 B2.

### Revision scope

PASS.

`3b19a0c` touches only architect-owned plan/gate files:

```text
M milestones/M-09-data-safety-net.md
M scripts/verify_M-09.sh
```

No code, tests, T1/contracts, `risk`, `killswitch`, `oms`, or venue/order path changed in the
revision commit. `git diff --check cbf011f..3b19a0c` is clean.

### Recommended next action

Proceed to engine-dev for Task 2 implementation.

=== HANDOFF: critic -> engine-dev ===

#### §A — Metadata

- UTC datetime: 2026-07-17T22:09:47Z
- From agent: critic
- Milestone: M-09 Task 2 volume-window recon
- Verdict: APPROVE
- Audited branch/head: `origin/feat/M-09-task2 @ 3b19a0c`
- Critic verdict file: `research/critiques/C-011-M-09-volume-window.md`

#### §B — What I checked

- C-011 B1: `scripts/verify_M-09.sh` now runs `red_recon_sink`
- C-011 B2: milestone Allowed paths now grants a narrow recorder recon-loop carve-out
- Revision scope: only `milestones/M-09-data-safety-net.md` and `scripts/verify_M-09.sh`
- Prior oracle semantics remain approved by C-011: reachable 21/21 and anti-placebo in both directions

#### §C — Outcomes

- APPROVE: plan-time blockers are closed.
- Current RED-phase verify is correctly `VERDICT: FAIL (2)` on `red_recon_window` + `red_recon_sink`.
- No CT-RFC needed for the windowed alert shape; T1 is unchanged.

#### §D — Paste-ready prompt for engine-dev

```text
Ты — engine-dev проекта hft-platform. Рабочий каталог /home/nous/hft-platform.

Branch: feat/M-09-task2. Bootstrap in your own worktree from latest origin/feat/M-09-task2.
Read first:
- CLAUDE.md
- .claude/rules/scope-guard.md
- .claude/rules/testing.md
- milestones/M-09-data-safety-net.md §Allowed paths + Task 2 second §8 block
- docs/fa/ops.md §4.3 / §6 OPS-I-1
- research/critiques/C-011-M-09-volume-window.md, especially "Re-audit 3b19a0c"

Task: implement M-09 Task 2 volume-window recon.

Allowed paths:
- crates/ops/src/recon.rs
- crates/ops/src/sink.rs
- crates/recorder/src/main.rs ONLY for recon-loop wiring per milestone carve-out
- crates/recorder/src/recon_loop.rs ONLY for MD-book feeder / recon-loop wiring per milestone carve-out, if needed

Forbidden:
- Do not edit tests, docs, milestones, scripts/verify_M-09.sh, or research/critiques.
- Do not touch crates/risk, crates/killswitch, crates/oms, crates/contracts/T1, venue order-egress, journal-write path, or unrelated recorder logic.

Implementation contract:
- Implement ReconDetector::observe as a stateful window over signed near-touch volume deltas per (band, side).
- Use best-price per-cycle via reconcile (immediate alert), not windowed.
- Keep per-cycle volume divergence as metric gauge only.
- Skip volume bands beyond reference.max_reach_pct(side).
- Threshold for window alert: EPS_TEST_BPS.min(thr.prod_bps()).
- Implement handle_recon_snapshot through ReconDetector: update book_divergence_bps every cycle; emit Sys(ReconDivergence) + increment book_resync_total only on alert.
- Rewire recorder recon-loop to hoist ReconDetector::new(thr) before while-loop and pass &mut detector to handle_recon_snapshot; remove old thr argument.

Required verification:
- cargo test -p ops --test red_recon_window
- cargo test -p ops --test red_recon_sink
- cargo test -p ops --test red_recon_live --test red_ops_recon
- bash scripts/verify_M-09.sh

Expected after implementation:
- red_recon_window: 7/7 PASS
- red_recon_sink: 4/4 PASS
- red_recon_live + red_ops_recon: PASS
- verify_M-09.sh should no longer fail on Task 2 window/sink; report any remaining RED from later M-09 tasks exactly.

Commit and push:
- One or more atomic commits scoped to Task 2.
- Before push: `git log origin/feat/M-09-task2..HEAD --oneline` must contain only your commits.
- Push fast-forward to `origin/feat/M-09-task2`.

Return Done Block with raw command outputs and handoff to tester, then reviewer §8.
```

#### §E — Outstanding risks

- Production `K` and windowed `ε_prod` remain `[verify-at-impl]` / §8 calibration; the oracle pins semantics, not final live tuning.
- Final acceptance still requires feeder integration and reviewer §8 live Binance re-run after engine-dev implementation.

=== END HANDOFF ===
