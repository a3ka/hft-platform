<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 6ef8c672f3895e85ed73628c81ab3d3089638d69
audited_head: 0110eff470a2b632be1da543e0223f07be3a428d
verdict: NOTE
-->

# C-275 — M-92: narrow R-217 closure / JR-I-13 RAW gate

**Date (UTC):** 2026-10-03
**Subject:** `origin/feat/M-92-manifest-verified-prune` at
`0110eff470a2b632be1da543e0223f07be3a428d` (fetched, not supplied SHA).
**Scope:** only the five checks mandated for the narrow post-`R-217` round. `A-045`
and `A-044` O-1…O-3 are not re-adjudicated.

## Verdict: NOTE — the narrow artifact gate is clear

The committed set is complete and internally coherent: public T2 surface
(`ColdManifest`, `retention_execute_with_manifest` and its re-export), RED oracle,
real acceptance guard, milestone contract, and the `JR-I-13` amendment are all on
the audited head. The current task-1/task-3 implementation is deliberately RED;
those failures are specified work, not a defect in this plan-time artifact set.

The applicable live FA invariants are **JR-I-2** (a gap is journal corruption) and
**JR-I-13** (offsite-manifest-verified deletion only). `JR-I-11` remains relevant
context: it protects monotonic ordering, not a missing catalog position, which is
why this retention oracle must prevent the hole itself.

## Artifact-set completeness

- **T-contract/signature:** `ColdManifest::parse` and
  `retention_execute_with_manifest(dir, plan, policy, manifest, mode)` are defined
  in `segments.rs` and re-exported by `crates/journal/src/lib.rs`.
- **RED:** `red_m92_manifest_prune.rs` contains the new `p10`/`p11` oracles and the
  strengthened `p1`; its existing `assert_contiguous` now judges catalog **names**
  where an intentionally corrupt header makes `list_segments` unavailable.
- **Acceptance:** `scripts/verify_M-92.sh` is an explicit fail-count aggregator.
  `task1-proof` extracts the function body, removes line comments, and rejects
  direct `remove_file`, absent `prune_segment(`, and an inline
  `ColdCopyProof { … }`.
- **Milestone + FA:** M-92 §3/§4/§9 and round-journal row 5 carry the new contract;
  `docs/fa/journal.md` introduces JR-I-13 and moves “first free” to **JR-I-14**.

## Required checks

### 1. R-217 Б-1 — PASS

`I-2ter` makes the deletion boundary the catalog rather than the plan. `p10`
corrupts the header at the middle name, proves that that name falls out of
`retention_plan` while planned neighbours exist on both sides, requires deletion
of exactly the older name-prefix, requires `blocked-by` for younger candidates,
and calls name-based `assert_contiguous`.

The implementation files under `crates/journal/src/` are byte-identical between
`b318f50` and this head. With the new oracle applied to that reference
implementation, `p10` fails only on its intended discriminator: it deleted
`{0,1,3}` instead of exactly `{0,1}`. This is an independent RED against the
plan-boundary bug, not a failure inherited from `p1` or `p11`.

### 2. R-217 Б-2 — PASS

The normative §4 text requires the proof path, and `task1-proof` independently
guards the previously black-box-only property. Its exact classifier rejected all
three prescribed bodies:

```
direct_remove: FAIL direct-remove_file
missing_prune: FAIL missing-prune_segment
inline_proof: FAIL inline-ColdCopyProof
honest_shape: PASS
current_body: FAIL direct-remove_file
```

The current pre-fix function therefore makes `task1-proof` red as required. The
script honestly declares the boundary of this guard: it is text-based; a renamed
call or macro can evade it, so code review remains the backstop.

### 3. R-217 Н-1 / Н-2 — PASS

`p11` builds equivalent worlds with a corrupt oldest offsite sum, confirms that
DryRun changes no names, and compares the entire failed-name set with Apply. On
the current implementation it fails with `dry = {}` versus all planned names in
Apply—the intended anti-placebo result for a no-hash dry run.

`p1` checks the normal fully matched case and requires `offloaded.is_empty()`.
It fails against the current code because four local paths are reported as
offloaded although this manifest path copies nothing. Both oracles therefore
measure their named invariants rather than merely observing an error.

### 4. §3 / §4 / §9 / JR-I-13 agreement — PASS

`I-2ter`, the exact public form, and `p10` use the same name-indexed catalog
boundary. `I-4`, `p11`, and the strengthened `p1` state the same dry-run and
`offloaded` properties. JR-I-13 promises no extra behavior: it names equality to
an offsite-computed sha256, `retention_plan` as the exclusive candidate source,
name-indexed prefix deletion, the private proof path, and no cold-directory copy;
each is represented by the oracle and/or `task1-proof`. It does not claim that
readers detect a pre-existing cross-segment sequence hole.

The next identifier is correctly **JR-I-14**, not a reuse of JR-I-9 or JR-I-13.

### 5. Satisfiable task boundary — PASS

The RED table separates the work correctly. The complete current test run reports
seven red tests: `p1`, `p10`, and `p11` are the reopened task-1 discriminators;
the only `c*` failures are `c0`, `c1`, `c4`, and `c5`, all task 3’s absent cron /
compose / audit-path work. `c2` and `c3` pass. The existing task-1 shape can
therefore be corrected without requiring task 3 to satisfy a library oracle; the
prospective honest implementation has a non-contradictory target.

## Disposition

No REJECT or ESCALATE finding arises within the requested five checks. The next
implementation cycle must still make task 1 GREEN before task 3, tester, and
reviewer progression; this NOTE does not approve a merge or production deletion.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-92-manifest-verified-prune
0110eff470a2b632be1da543e0223f07be3a428d

$ git merge-base origin/main origin/feat/M-92-manifest-verified-prune
6ef8c672f3895e85ed73628c81ab3d3089638d69

$ git diff --exit-code b318f50470640a1adba821519eb5fc0c7ada0e11 HEAD -- crates/journal/src
source_since_b318_exit=0

$ cargo test -p journal --test red_m92_manifest_prune -- p1_matching_manifest_prunes_exactly_the_plan
test p1_matching_manifest_prunes_exactly_the_plan ... FAILED
M-92 (R-217 Н-2): offloaded = [four local segment paths] на пути без копирования
p1_exit=101

$ cargo test -p journal --test red_m92_manifest_prune -- p10_catalog_position_outside_plan_blocks_younger
test p10_catalog_position_outside_plan_blocks_younger ... FAILED
left:  {"segment-00000000.jrnl", "segment-00000001.jrnl", "segment-00000003.jrnl"}
right: {"segment-00000000.jrnl", "segment-00000001.jrnl"}
p10_exit=101

$ cargo test -p journal --test red_m92_manifest_prune -- p11_dry_run_failed_set_equals_apply
test p11_dry_run_failed_set_equals_apply ... FAILED
M-92 (R-217 Н-1): failed пробного прогона расходится с настоящим
left: {} ; right: {"segment-00000000.jrnl", "...01", "...02", "...03"}
p11_exit=101

$ task1-proof classifier probe
direct_remove: FAIL direct-remove_file
missing_prune: FAIL missing-prune_segment
inline_proof: FAIL inline-ColdCopyProof
honest_shape: PASS
current_body: FAIL direct-remove_file

$ bash scripts/verify_M-92.sh
PASS  task1: red_retention + red_retention_checkpoint_coverage + red_retention_compacted + red_retention_operator
FAIL  task1-3: red_m92_manifest_prune (exit=101)
FAIL  task1-proof: retention_execute_with_manifest удаляет напрямую (remove_file) — в обход ColdCopyProof (R-217 Б-2)
FAIL  task3: verify_M-48.sh (exit=1)
PASS  task5: JR-I-13 определён; «первый свободный» сдвинут
PASS  ci-map: проба карты CI-паритета
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
FAIL  ci-parity: cargo test --all (exit=101; only p1/p10/p11 + c0/c1/c4/c5 in red_m92)
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 49, исключено по карте 6)
VERDICT: FAIL (провалов: 4)
verify_exit=1  # expected pre-implementation RED baseline

$ bash scripts/check_artifact_ids.sh
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC): 2026-10-03
- Milestone: M-92-manifest-verified-prune
- Status: DONE — NOTE; narrow R-217/JR-I-13 artifact gate clear
- Audited HEAD: 0110eff — docs(fa/journal): JR-I-13 … [architect]

## §B — What I did
- Audited only the five prescribed post-R-217 points on the fetched subject head.
- Exercised p1, p10, p11, the task1-proof matching logic, and the complete M-92 acceptance script.

## §C — Artifact / results
- `research/critiques/C-275-m92-r217-narrow.md`.
- Done Block above; expected RED baseline is explicit and does not authorize merge.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  Subject: origin/feat/M-92-manifest-verified-prune after C-275 NOTE (audited head 0110eff before verdict commit). Add C-275 mechanically to M-92's round journal, then issue the existing-scope engine-dev mandate to make task 1 GREEN: catalog-name boundary I-2ter, manifest-derived ColdCopyProof only through prune_segment, DryRun/Apply failed-set parity, and empty offloaded. Do not alter c0/c1/c4/c5: they remain task 3. Re-run the named p1/p10/p11 and task1-proof; push the branch before tester handoff.
  ```

## §E — Risks / open questions
- The current task-1/task-3 implementation remains intentionally RED; NOTE is a plan-time result only.
- The text guard’s renamed-call/macro limit remains reviewer-backstopped, exactly as declared by the milestone.

=== END HANDOFF ===
