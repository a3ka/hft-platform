<!-- GATE-META
milestone: C-101
audited_repo: a3ka/hft-platform
audited_base: 64bb362c8899ffdc56365fcae1af18e6f5e80851
audited_head: 107e5b00246b6fdcb71360cf0480b81cca58aa31
verdict: REJECT
-->

# C-177 — REJECT: harness adversary audit `harness-milestone-shape`, round 5

## Subject and route

- Route: `docs/workflow/harness-track.md` §§3 and 5.3.  This fresh-context
  adversary verdict is a required committed merge precondition.
- Audited branch: `feat/harness-milestone-shape`.
- Audited range: `64bb362c8899ffdc56365fcae1af18e6f5e80851..107e5b00246b6fdcb71360cf0480b81cca58aa31`.
- Audited artifacts: `scripts/check_milestone_shape.sh`,
  `scripts/tests/red_milestone_shape.sh`, and the `milestone-shape` /
  `status-check` wiring in `.github/workflows/ci.yml`.
- Artifact ID: allocated by `bash scripts/next_artifact_id.sh C` → `C-177`.

## Verdict: REJECT

The live checker correctly implements the new narrow grammar for all four
declared raw-HTML forms and exact section titles.  The probe does not pin two
of those promised properties.  It is therefore green against live-derived
semantic weakenings and fails the harness-track §5.1 anti-placebo requirement,
§5.2 mutation-control requirement, and `testing.md` gate-integrity property 3.

### B-12 — `<style>` is declared and implemented, but not tested or mutation-pinned

`check_milestone_shape.sh:160` declares `pre|script|style|textarea` as the
four raw-HTML block forms hidden from section recognition.  C-176's clearance
condition required fixtures for those forms.  Round 5 adds only `script`,
`pre`, and `textarea` at `red_milestone_shape.sh:347-349`; there is no
`style` fixture.

I derived a semantic stub from the audited live checker by changing only
`pre|script|style|textarea` to `pre|script|textarea`.  The source changed
(`cmp` exit 1), yet the complete ordinary probe under that stub is green:
`PASS=36 FAIL=0`, exit 0.  Its existing `htmlblind` battery mutation disables
all HTML hiding and is caught by the other three fixtures; it cannot establish
that the `style` branch itself remains present.

This leaves a future style-only regression silently green, contrary to the
declared grammar and C-176 B-9's explicit four-form condition.

### B-13 — `§Tasks` has no exact-title fixture and its dedicated prefix weakening is green

C-176 B-10 required negative prefix/suffix fixtures for **all four** required
section names.  Round 5 adds them only for `Allowed paths`, `Objective`, and
`Acceptance` (`red_milestone_shape.sh:352-354`), omitting `§Tasks`.

The generic `titleprefix` battery mutation removes the title boundary from all
four regexes, but its red result can be caused entirely by the three present
fixtures.  I derived a narrower live-code stub that changes only
`check_section "$f" "§Tasks"` from
`'^#{2,3} +§?Tasks[ \t]*#*[ \t]*$'` to `'^#{2,3} +§?Tasks'`.
The mutated line was confirmed, the source differed (`cmp` exit 1), and the
ordinary probe remained green at `PASS=36 FAIL=0`, exit 0.  Thus a committed
milestone with only `## §TasksNOT-A-SECTION` would be accepted by this future
regression without any probe failure.

## Required condition for re-audit

1. Add a committed `style` raw-HTML fixture in the same form as the other
   three B-9 cases.  Add a setup-guarded, live-derived style-only mutation
   (removing only `style` from the raw-HTML tag alternation) and require the
   full probe to fail against it.
2. Add an exact-title negative fixture for `§TasksNOT-A-SECTION` (and retain
   the existing three title fixtures).  Add a setup-guarded mutation that
   removes the boundary only from the `§Tasks` regex and require the probe to
   fail against it.
3. Re-run the honest probe and full mutation battery, commit and push the
   three harness artifacts, then request a new fresh-context adversary audit.

## Non-blocking note

- N-2 — `.github/workflows/ci.yml:229` still calls the self-test a battery
  “из четырёх ослаблений”, while the executable battery and step name say 12.
  It does not alter the exit code, but the explanatory comment should match
  the mechanism in the same correction.

## Done Block

```text
$ git diff --name-status 64bb362c8899ffdc56365fcae1af18e6f5e80851 107e5b00246b6fdcb71360cf0480b81cca58aa31
M	.github/workflows/ci.yml
M	scripts/check_milestone_shape.sh
M	scripts/tests/red_milestone_shape.sh
exit=0

$ bash scripts/tests/red_milestone_shape.sh
  батарея ослаблений: поймано 12 из 12
PASS=48 FAIL=0 (сценариев: 48)
VERDICT: PASS
уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: 0
exit=0

$ MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=<live checker with only style removed> bash scripts/tests/red_milestone_shape.sh
PASS=36 FAIL=0 (сценариев: 36)
VERDICT: PASS
style_mutant_probe_exit=0
# Expected: nonzero.  The B-9 `style` branch is not pinned.

$ rg -n 'check_section.*§Tasks' <§Tasks-prefix-stub>
185:  check_section "$f" "§Tasks"        '^#{2,3} +§?Tasks'
exit=0

$ MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=<§Tasks-prefix-stub> bash scripts/tests/red_milestone_shape.sh
PASS=36 FAIL=0 (сценариев: 36)
VERDICT: PASS
tasks_mutant_probe_exit=0
# Expected: nonzero.  The dedicated B-10 `§Tasks` boundary is not pinned.

$ bash -n scripts/check_milestone_shape.sh scripts/tests/red_milestone_shape.sh
exit=0

$ git diff --check 64bb362c8899ffdc56365fcae1af18e6f5e80851 107e5b00246b6fdcb71360cf0480b81cca58aa31
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=64bb362c8899ffdc56365fcae1af18e6f5e80851 bash scripts/check_milestone_shape.sh
OK: в диапазоне 64bb362..HEAD новых milestone-спек нет — проверять нечего
exit=0

$ bash scripts/next_artifact_id.sh C
C-177
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC, ISO-8601): 2026-08-29T10:19Z
- Milestone: C-101 / harness-track `harness-milestone-shape`
- Status: BLOCKED — REJECT
- Audited HEAD: 107e5b0 — fix(harness): C-176 — грамматика ОБЪЯВЛЕНА, а не догоняется правилом за правилом; круг 5 [architect]

## §B — What I did
- Audited the committed round-5 range, live checker, probe, CI membership, and C-173/C-175/C-176 conditions.
- Ran the honest probe and derived two single-property semantic mutations from the audited live checker.

## §C — Artifacts / results
- `research/critiques/C-177-harness-milestone-shape-r5.md`
- Done Block: raw command output and exit codes are recorded above.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  On feat/harness-milestone-shape, resolve REJECT C-177 before requesting another adversary pass. Preserve the three-artifact harness scope. Add a committed <style>-hidden required-section fixture and a setup-guarded style-only live-derived mutation that makes the probe fail. Add a §TasksNOT-A-SECTION fixture and a setup-guarded mutation removing only the §Tasks title boundary that makes the probe fail. Keep all previous C-173/C-175/C-176 coverage, correct the stale CI comment, run the honest probe and full battery, commit and push the corrected artifacts, then provide the full new SHA and base for a fresh audit.
  ```
- Push status: this REJECT verdict will be committed and pushed to `origin/feat/harness-milestone-shape` before handoff.
- Cache: no build cache created.

## §E — Risks / open questions
- Merge remains blocked by `docs/workflow/harness-track.md` §5.3 until B-12 and B-13 are resolved and a new fresh-context adversary verdict is committed.

=== END HANDOFF ===
