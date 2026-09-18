# C-002 — Critic Verdict — M-04 Research core (re-pass — closure audit of C-001)

**Date:** 2026-07-10
**Milestone:** `milestones/M-04-research-core.md`
**Prior verdict:** `research/critiques/C-001-M-04-plan.md` (REJECT, 2026-07-10)
**Audited commit:** `f02c418` (fix(M-04): critic-response C-001 — book::top_n_depth примитив +
T1-designate выравнивание [architect])
**Scope of this pass:** closure-only — does `f02c418` close C1/M1/m1/m2 from C-001? Not a
full re-audit of the milestone.

## Verdict

**NOTE**

## Verdict Justification

All four C-001 findings are closed. C1 (CRITICAL, blocking) is closed the way the prior verdict
suggested: a `book::OrderBook::top_n_depth` signature + doc-comment + sacred RED test were added
by architect; the method body stays `todo!()`; implementation is delegated to `engine-dev` via a
narrow, explicitly-named carve-out (one method, one file) added to the milestone's scope table and
to Task 2 — `signal-engineer` remains forbidden from `crates/book/**`, so Task 3 no longer requires
an unauthorized touch of a sacred crate. Empirically re-ran `cargo test -p book`: the 3 pre-existing
tests still pass, the 2 new `top_n_depth` tests genuinely FAIL on the `todo!()` stub (not placebo).
M1 (MAJOR) is closed via option (b) from C-001's suggested resolution: a named, dated FA amendment
in `docs/fa/research-cli.md` §N (with an Amendment-history row) states `ValidationReport`/
`TrialsLedger` stay T1-designate with promotion deferred, and the milestone's "Contract impact"
section was rewritten to match both `docs/05-contract-layer.md` §2 (unchanged, still lists both as
T1) and the new FA §N wording — no more three-way fork; `TD-008-t1-report-forms-promotion` is
explicitly named for reviewer to open at merge. m1 and m2 (MINOR) are closed as documented awareness
notes, matching what C-001 asked for (m1: an inline verify-script comment for Task 8's intentional
gate-absence, plus a new comment noting T6 is expected-red until Task 2 lands; m2: an inline test
comment naming the effect-vs-mechanism oracle limitation — C-001 explicitly said "not blocking;
noting for awareness," not "fix the test").

Dev dispatch is unblocked on the C1 front. `verify_M-04.sh` correctly reports one more failure
than C-001's baseline (7 vs. 6) because the new sacred book RED test is included in T6 — this is
the expected, intentional pre-Task-2 state, not a regression.

## Findings (closure-only; no new findings raised)

### C1 — CLOSED
- Evidence: `crates/book/src/lib.rs:119-126` — `pub fn top_n_depth(&self, side: Side, n: usize) ->
  i64` added, doc-comment cites C-001 C1 + delegates impl to engine-dev/Task-2, body is
  `todo!("engine-dev: M-04 task 2 (carve-out per C-001 C1)")`.
- Sacred RED test added: `crates/book/tests/test_top_n_depth.rs` (2 tests: best-levels-sum
  including an n=1 top-of-book case, and an n-exceeds-levels/empty-side/n=0 edge-case test).
  Empirically re-ran `cargo test -p book`:
  ```
  running 3 tests   (crates/book/src/lib.rs unit tests)
  test tests::best_mid_spread ... ok
  test tests::depth_bands_filter_by_pct ... ok
  test tests::microprice_between_bid_ask ... ok
  test result: ok. 3 passed; 0 failed

  running 2 tests   (tests/test_top_n_depth.rs)
  test test_top_n_depth_n_exceeds_levels_and_edges ... FAILED
  test test_top_n_depth_sums_best_levels ... FAILED
  test result: FAILED. 0 passed; 2 failed
  ```
  Exactly the 3-old-pass / 2-new-fail split the re-pass instructions asked to verify. Both
  failures are genuine `not yet implemented` panics on the `todo!()` stub — not a placebo GREEN.
- Scope-table carve-out (`milestones/M-04-research-core.md` §"Allowed / Forbidden paths"):
  `engine-dev` row now reads `crates/sim/src/**`, `crates/sim/Cargo.toml` (deps), **carve-out per
  C-001 C1: `crates/book/src/lib.rs` ТОЛЬКО реализация `top_n_depth` (сигнатура и RED-тест —
  architect)**. `signal-engineer` row is unchanged (`crates/signals/src/**`,
  `crates/signals/Cargo.toml`, `research/specs/` only — no `book/` access). The catch-all "все dev"
  row still forbids `crates/book/**` in general; the engine-dev-specific row is the narrower
  override, consistent with how the pre-existing per-role rows already worked. Task 2's row was
  updated to include `book::top_n_depth (carve-out C1, RED-тест ...)` in both the task description
  and its Verify column (`SM-I-1,2,4,5,6,7,8,10 GREEN + book-тест GREEN`).
- Sufficiency for Task 3: yes. Task 3 (signal-engineer, OBI TopN mode) now has a real public API
  path to top-N depth that will exist once Task 2 lands, with no forbidden-zone touch required by
  signal-engineer. The C-001-identified blocking dependency is resolved architecturally (signature +
  test authored by architect; only the trivial-arithmetic body is deferred to engine-dev, in-scope
  for that role already via the sim/book proximity in Task 2).

### M1 — CLOSED
- `docs/fa/research-cli.md` §N carries the named amendment block (quoted, dated 2026-07-10,
  explicitly cross-referencing "critic C-001 M1 — named, not silent") plus a matching row in the
  file's Amendment-history table at the bottom.
- `milestones/M-04-research-core.md` "Contract impact (T1)" section rewritten: no longer claims
  "T1 НЕ трогается" as a blanket statement; now explicitly states `TrialRecord`/`ValidationReport`
  **remain T1 forms** per `05 §2`, names the T1-designate status + FA §N amendment, and instructs
  reviewer to open `TD-008-t1-report-forms-promotion` at merge (reviewer-owned per scope-guard,
  correctly not something architect can write itself).
- Cross-checked `docs/05-contract-layer.md` §2 — unchanged, still lists `ValidationReport` and
  `TrialsLedger entry` as T1 with research-cli as producer. All three documents (05 §2, FA
  research-cli §N, milestone) now agree; no more fork between prose and canon.

### m1 — CLOSED
- `scripts/verify_M-04.sh`: new inline comment directly above the T6 check explaining T6 is
  expected red until engine-dev closes Task 2 (the new book RED test lives inside that check); new
  inline comment block after the T7 checks stating Task 8 is intentionally check-free (gated by
  data-accumulation + risk-critic/founder sign-off, verified by humans against
  `research/reports/R-001*`, not by this script). Matches the pattern used elsewhere in the script.

### m2 — CLOSED (documentation, as requested — not a behavior change)
- `crates/research-cli/tests/red_research.rs::test_ledger_append_only` now carries a 4-line comment
  immediately above the test body naming the effect-vs-mechanism limitation verbatim (checks
  prefix-preservation effect, not `O_APPEND` mechanism; a rewrite-based implementation would be
  caught at code review instead). This is exactly what C-001 asked for ("not blocking; noting for
  awareness") — no test-logic change expected or made.

## Gate re-verification (empirical, this pass)

- `cargo test -p book` → 3 old unit tests PASS, 2 new `top_n_depth` tests genuinely FAIL on
  `todo!()` (see C1 above). Matches instructions exactly.
- `cargo fmt --all -- --check` → PASS (exit 0).
- `cargo clippy --workspace --all-targets -- -D warnings` → PASS, 0 warnings, all workspace crates
  (book's new stub + new test file included).
- `bash scripts/verify_M-04.sh; echo "exit=$?"` → `VERDICT: FAIL (7 провалов)`, real exit code 1
  (verified via separate non-piped invocation, not the `tail`-swallowed exit code). This is one
  failure more than C-001's baseline of 6 — the delta is exactly the new book RED test now included
  in the T6 workspace-regression check, which is the documented, intentional pre-Task-2 state (per
  the new inline comment closing m1). T1a/T1b (fmt/clippy) still PASS; T2/T3/T4/T5a-c/T6 FAIL as
  expected pre-implementation; T7/T8 PASS. No unexpected failures, no placebo GREENs, script
  structure (set -euo pipefail equivalent aggregator, no echo-masking) unchanged from C-001's
  already-verified PASS.

## Confidence

**High** — every closure claim above is backed by a direct read of `f02c418`'s diff plus the
resulting files (`crates/book/src/lib.rs`, `crates/book/tests/test_top_n_depth.rs`,
`crates/research-cli/tests/red_research.rs`, `docs/fa/research-cli.md`,
`milestones/M-04-research-core.md`, `scripts/verify_M-04.sh`, `docs/05-contract-layer.md`), and by
actually executing `cargo test -p book`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `bash scripts/verify_M-04.sh` against
current HEAD, not by inference from the commit message.

## Next agent

Per `.claude/agents/critic.md` / `.claude/rules/critic-protocol.md` NOTE-tier discipline: findings
are closed, not merely advisory-carried-forward (this is a closure re-pass, not a fresh NOTE list).
**Next agent: dev dispatch is authorized** — `engine-dev` (Task 2, incl. the `top_n_depth`
carve-out), `signal-engineer` (Task 3), `research-dev` (Task 4) per the milestone's own Handoff
line ("architect → critic → (2,3,4 параллельно) → 5 → 6 tester → reviewer → 8"). No
architect-self-fix-loop implied by this NOTE; any future findings from tester/reviewer follow the
normal SVR-response cycle for architect-owned files.
