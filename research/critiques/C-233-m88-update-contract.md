<!-- GATE-META
milestone: M-88
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: a9fa558ffe4fb6b14061c170371ddb01927e5c90
verdict: REJECT
-->

# C-233 — M-88 update-contract: REJECT

## Verdict

**REJECT — dev dispatch is blocked.** The committed set is structurally complete: the M-88
milestone, T-designate form/signature, two RED binaries, and `verify_M-88.sh` are present;
the baseline is genuinely RED. But the acceptance contract does not meet plan §15.2 and the
verify gate can pass unfinished tasks. It also omits a mandatory degenerate lifecycle case.

`VB-I-2` is the applicable live invariant: client-side state assembled from snapshot + frames
must be bit-identical to replay of the same journal window (`docs/fa/viz-backend.md:205-206`).
The gateway has no own FA; this verdict relies on viz-backend FA as required by the M-66
exception.

## Findings

### R1 — §15.2's real-client acceptance is explicitly absent

`milestones/M-88-liquidity-removal-contract.md:197-198` exercises producer → JSON wire →
Rust `Snapshot::apply`, but `:248-250` explicitly says that the actual Next.js consumer is
not run and is outside the test scope. This conflicts with plan §15.2, which requires the
affected **real client**, not only the server-side merge function or a Rust model of it.

The gap matters for the new semantics: `ApplyOutcome::OutOfOrder` is only useful if the client
observes it and follows a specified recovery/resnapshot path. The RED helper drops the outcome
with `let _ = acc.apply(&wired)` at
`crates/gateway/tests/red_m88_update_contract.rs:149-156`; `#[must_use]` does not protect that
form. A JSON frame that deserializes with omitted/default observed-bucket metadata can therefore
be treated as `NoChange` while a full replay is empty. No test proves schema-version rejection
or recovery in the consumer that users actually run.

Required correction: name the production consumer and its allowed path; add an executable
wire-to-real-client oracle that proves (a) empty observed slice clears, (b) missing observation
does not clear, and (c) duplicate/stale frame causes the declared recovery rather than a silent
freeze. Add these components to the forbidden list as well.

### R2 — `verify_M-88.sh` has demonstrated false-green tasks

Task 9 is OPEN (`milestones/M-88-liquidity-removal-contract.md:188-191`) and its decision table
is still the placeholder at `:306-310`, yet `scripts/verify_M-88.sh:85-90` passes merely because
the heading exists. The raw baseline printed `PASS  task9` while the task was unfinished.

Other independently insufficient checks are present:

- Task 2 only rejects one phrase, `замещает раннее` (`scripts/verify_M-88.sh:41-45`); deleting
  the documentation or replacing it with another merge instruction passes without requiring the
  promised full-slice meaning.
- Task 7 searches `#[must_use]` anywhere in the file rather than adjacent to `Snapshot::apply`
  (`:71-75`), so an unrelated annotation can satisfy it.
- Task 8 accepts any file with any three digits (`:77-83`), not a before/after measurement on
  the stated production-form fixture.

Required correction: turn each requirement into an affirmative, scoped oracle. Task 9 must
reject the placeholder and require a non-empty decision for every affected expectation.

### R3 — required degenerate input, several lives of one level, is absent

The RED corpus covers asymmetric updates (S1: bid-only delta at `:201-209`), multiple levels
(S1/S3), no observation (S4 at `:337-355`), and time-bucket boundaries (S1/S2). It does not
cover the required separate lifecycle case from `testing.md`: the same price is present,
removed, and present again inside one bucket.

Concrete missing input: at `T`, snapshot has bid `(64_990, 5)`; at `T+1`, delta is
`(64_990, 0)`; at `T+2`, delta restores `(64_990, 7)`. Split the two deltas into distinct
frames. Full replay must contain exactly the final `(64_990, 7)` cell. This is not exercised:
the only same-bucket second delta in S9 changes a *different* price
(`red_m88_update_contract.rs:502-503`). Numeric overflow/truncation boundaries are also absent.

The stated Replace rule is sufficient for the named normal input **only if** each producer emits
the complete final column, including empty columns, and the actual consumer rejects the wrong
schema/recoverably rejects a cursor gap. The current implementation is the known counterexample:
the S3 input (all levels removed in the same `T`) recomputes to `{}`, while live merge retains
the old cells. The new untested/default-metadata consumer case above would recreate the same
divergence after the form bump.

### R4 — mutation control is only partially evidenced on the RED baseline

The first required mutant is exactly the audited implementation:
`crates/gateway/src/lib.rs:2735-2743` builds `merge_heatmap` from
`existing.iter().chain(incoming.iter())`. The RED run therefore proves that the set is red on
that mutant: S1, S1b, S3, S5 and S9 fail.

The second defective semantic is also present in the audited code:
`crates/gateway/src/lib.rs:2753-2757` maps an empty incoming COB to existing state. S3 is red
on the all-levels-removed input. However, a separately corrected GREEN implementation does not
exist yet, so I could not run an *isolated post-GREEN* mutation that flips only
`empty full slice → NoChange`; the current S3 failure first reports heatmap. This is not a
claim of completed mutation control. The dev Done Block must include the two isolated mutations
specified in milestone §9.2 and show their named failing test(s).

## Required answers to the audit questions

| Question | Measured result |
|---|---|
| Seven plan scenarios | Covered by S1 removal, S2 closed past, S3 empty full slice, S4 no observation, S5 leaves window, S6 duplicate, S7 stale. S8/S9, S1b, and `different_frame_boundaries_*` are additional guards. |
| RED anti-placebo | `cargo test -p gateway --test red_m88_update_contract` gives exactly 4 passed / 7 failed, exit 101. |
| Verify baseline | Exactly 9 FAIL and `VERDICT: FAIL (провалов: 9)`, exit 1, as claimed. It is nevertheless unsound per R2. |
| Merge-preview code claims | `bash scripts/verify_design_claims.sh --merge-preview origin/main` passed, exit 0. `origin/main` is an ancestor and `crates/gateway/src` is unchanged in the subject diff, so the cited source facts are on that merge tree. |
| Form / RFC | Correct: source is v10 (`crates/gateway/src/lib.rs:117`), the planned 10→11 bump is required by `VB-I-4`, and no `contracts/` path is changed. `SeriesBundle` is T-designate under `docs/05-contract-layer.md` §2, so a contract-RFC is not required. |

## Scope and artifact checks

- Subject diff contains only the two architect-owned RED files, milestone, verify script, and
  the P0-CORR roadmap row; it does not touch `contracts/`.
- T-designate contract: fields `SeriesBundle.heatmap_observed_time_s`,
  `SeriesBundle.cob_observed`, and `Snapshot::apply -> ApplyOutcome` are specified in
  milestone `:102-145`; the separate compile-RED test makes the absent form fail without
  masking behavioural RED.
- No RISK-BLOCK path is changed. The critic trigger is the new milestone/form specification.

## Done Block

```text
$ git fetch origin --quiet && git rev-parse origin/feat/M-88-liquidity-removal-contract
a9fa558ffe4fb6b14061c170371ddb01927e5c90
exit=0

$ cargo test -p gateway --test red_m88_update_contract
running 11 tests
test result: FAILED. 4 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out
failures: S1, S1b, S3, S5, S6, S7, S9
exit=101

$ bash scripts/verify_M-88.sh
FAIL  task1: нет полей heatmap_observed_time_s / cob_observed in crates/gateway/src/lib.rs
FAIL  task1: GATEWAY_SCHEMA_VERSION не равен 11
FAIL  task2: источник наблюдения по-прежнему предписывает объединение
FAIL  task3-6: red_m88_update_contract КРАСЕН — 4 passed; 7 failed
FAIL  task1+7: red_m88_contract_form КРАСЕН — 9 compile errors
FAIL  task7: apply не возвращает ApplyOutcome либо не помечен must_use
FAIL  task8: нет замера размера кадра с числами
PASS  task9: раздел ревизии оракулов заполнен в спеке
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 9)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ git merge-base --is-ancestor origin/main HEAD; echo $?
0
$ git diff --quiet origin/main..HEAD -- crates/gateway/src; echo $?
0

$ git ls-remote origin refs/reserved/C-233
90d1de1ea1fcb4236447d0bbc31007519e5833bb  refs/reserved/C-233
exit=0
```
