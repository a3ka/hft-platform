<!-- GATE-META
milestone: C-101
audited_repo: a3ka/hft-platform
audited_base: c4cfb8564fb5549060762c7056485065557afee0
audited_head: 679b114d3de59b8098785baf2b4825b4ef615c1d
verdict: REJECT
-->

# C-172 — REJECT: adversary audit `harness-milestone-shape`, round 2

## Subject and route

- Route: `docs/workflow/harness-track.md` §3, mandatory fresh-context adversary.
  Its §5.3 makes this committed verdict file a merge condition.
- Audited branch: `feat/harness-milestone-shape`; base
  `c4cfb8564fb5549060762c7056485065557afee0`; audited tip
  `679b114d3de59b8098785baf2b4825b4ef615c1d`.
- Audited artifact set: `scripts/check_milestone_shape.sh`,
  `scripts/tests/red_milestone_shape.sh`, and the `milestone-shape` job plus
  `status-check` wiring in `.github/workflows/ci.yml`.  The diff is confined
  to those harness paths; this track intentionally needs neither T-contracts
  nor a product milestone file.

## Verdict: REJECT

The C-101 normal-fence/comment, prose-substring, and rename cases are now
covered, but the barrier still gives false green on the form it claims to
enforce, and its anti-placebo battery leaves material weakenings green.

### B-5 — a mixed fence closes the wrong code block (live false green)

`visible_body()` records only `fence=1`; its closing branch at
`scripts/check_milestone_shape.sh:92` accepts either ``` or ~~~.  It does not
retain the opening marker.  Therefore `~~~` inside an open ``` fence is treated
as a close.  The following `## Allowed paths` is still fenced Markdown, not a
visible section, but the live barrier returns success.

This is a direct recurrence of C-101 B-1's promise on an untested fence
boundary.  The test suite has equal-marker ``` and ~~~ cases but no mismatched
marker / nested-fence case.

### B-6 — `##Allowed paths` is accepted as a section (live false green)

The required-header regex at
`scripts/check_milestone_shape.sh:112` is `^#{2,3} *Allowed paths`: ` *`
permits zero spaces.  `##Allowed paths` is not an ATX Markdown heading, hence
is not the visible `Allowed paths` section required by `docs/04-workflow.md`
§6.  Nevertheless the live barrier prints `OK` and exits 0.  No probe fixture
pins the separator between the hash run and section name.

### B-7 — the battery does not pin three material properties

All three stubs below return a fully green probe, including its advertised
four-stub self-check.  This violates the harness-track §5.1 requirement that
the probe be red against deceptive stubs, and `testing.md`'s anti-placebo and
setup-guard requirements.

1. **Committed object, not working tree.**  Replacing only
   `git show "HEAD:$1"` in `visible_body()` with `cat "$1"` leaves
   `red_milestone_shape.sh` green `24/24`.  Against an added incomplete
   milestone whose dirty working-tree copy has `Allowed paths`, the live
   barrier correctly exits 1 while this stub exits 0.  The suite never makes
   working tree and `HEAD:<file>` differ, despite this being an audit of a
   committed range.
2. **NUL-safe non-ASCII paths.**  Replacing the paired `--name-only -z` and
   `mapfile -d ''` with newline handling leaves the complete probe green
   `24/24`.  A complete `milestones/M-99-кириллица.md` then passes live
   (`exit=0`) but the stub rejects it (`exit=1`) because text-mode Git quotes
   the filename.  C-101 explicitly recorded non-ASCII behavior; this revision
   no longer pins it with a fixture.
3. **Permitted heading depth.**  Widening only each `#{2,3}` to `#{2,4}` leaves
   the entire probe green `24/24`.  That stub accepts `#### Allowed paths`,
   although the checker itself declares at lines 81–82 that the accepted form
   is only `## X` or `### X`.  The positive depth case alone cannot distinguish
   this weakening.

These are semantic stubs derived from the live file, not copies of an old
implementation.  The existing `sed` self-check still runs successfully
against each, which is why its current four mutations are insufficient.

## Confirmed closed / non-blocking checks

- The ordinary ``` fence, ~~~ fence, and HTML-comment pseudo-heading cases
  are detected; substituting the whole visible-body call causes exactly three
  scenario failures and `FAIL=3`, so the repaired counter agrees with its raw
  output.
- The current prose-only `Acceptance` case kills the prior substring stub.
- `--diff-filter=AR` kills the C-101 rename regression.  A copied new file is
  classified `A` under the checker’s actual no-`-C` invocation and is rejected;
  status `C` is not a demonstrated bypass here.
- The obsolete `e555cb4`/`10753df` assertion and the stale literal `14
  сценариев` are absent.  The honest run has 24 PASS lines, `FAIL=0`, and no
  residual `/tmp/red-mshape-*` objects.
- CI wiring is present: the job uses `fetch-depth: 0`, passes the event-base
  variables to the barrier, and is in both `status-check.needs` and its
  fail-closed condition.

## Condition for re-audit

Architect must first extend the RED suite so it is red against B-5 and B-6,
and against the three B-7 semantic stubs above, with setup guards that prove
each fixture and mutation occurred.  Re-run the honest probe and the full
adversarial battery, then present the new committed head for another fresh
adversary pass.

## Done Block

```text
$ git diff --name-status c4cfb8564fb5549060762c7056485065557afee0 679b114d3de59b8098785baf2b4825b4ef615c1d
M	.github/workflows/ci.yml
A	scripts/check_milestone_shape.sh
A	scripts/tests/red_milestone_shape.sh
exit=0

$ bash scripts/tests/red_milestone_shape.sh
...
  батарея ослаблений: поймано 4 из 4

PASS=24 FAIL=0 (сценариев: 24)
VERDICT: PASS
уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: 0
exit=0

$ mixed ``` / ~~~ fence fixture against live checker
=== проверяю форму: milestones/M-99-mixed-fence-live.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
checker_exit=0
# expected: exit=1; the only Allowed-paths candidate remains inside the ``` fence.

$ no-space heading fixture against live checker
=== проверяю форму: milestones/M-99-no-space.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
exit=0
# subject has `##Allowed paths`, not a Markdown heading; expected exit=1.

$ BARRIER_OVERRIDE=<working-tree-reader-stub> bash scripts/tests/red_milestone_shape.sh
...
PASS=24 FAIL=0 (сценариев: 24)
VERDICT: PASS
probe_exit=0

$ committed incomplete M-99 plus dirty complete worktree copy
HEAD_has_Allowed_paths=false
$ live checker
FAIL  milestones/M-99-dirty.md: отсутствует обязательный раздел «Allowed paths»
live_exit=1
$ working-tree reader stub
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
stub_exit=0

$ BARRIER_OVERRIDE=<newline-filename-stub> bash scripts/tests/red_milestone_shape.sh
...
PASS=24 FAIL=0 (сценариев: 24)
VERDICT: PASS
probe_exit=0

$ complete Unicode filename fixture
$ live checker
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
live_exit=0
$ newline filename stub
FAIL  "milestones/M-99-\\320...md": отсутствует обязательный раздел «Objective»
stub_exit=1

$ BARRIER_OVERRIDE=<h4-header-stub> bash scripts/tests/red_milestone_shape.sh
...
PASS=24 FAIL=0 (сценариев: 24)
VERDICT: PASS
probe_exit=0

$ h4 heading fixture against h4-header-stub
=== проверяю форму: milestones/M-99-h4-header-stub.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
checker_exit=0

$ MSHAPE_SELFTEST=1 BARRIER_OVERRIDE=<fenceblind-stub> bash scripts/tests/red_milestone_shape.sh
  FAIL: раздел только в ```-фенсе → отказ — ожидался exit=1, получен exit=0
  FAIL: раздел только в ~~~-фенсе → отказ — ожидался exit=1, получен exit=0
  FAIL: раздел только в HTML-комментарии → отказ — ожидался exit=1, получен exit=0
PASS=17 FAIL=3 (сценариев: 20)
VERDICT: FAIL
probe_exit=1

$ copy candidate under the checker’s actual diff invocation
A	milestones/M-99-copy.md
=== проверяю форму: milestones/M-99-copy.md ===
FAIL  milestones/M-99-copy.md: отсутствует обязательный раздел «Objective»
checker_exit=1

$ rg -n 'e555cb4|10753df|14 сценариев' scripts/check_milestone_shape.sh scripts/tests/red_milestone_shape.sh .github/workflows/ci.yml
exit=1

$ find /tmp -maxdepth 1 -name 'red-mshape-*' -print | wc -l
0
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC, ISO-8601): 2026-08-28T15:20Z
- Milestone: C-101 / harness-track `harness-milestone-shape`
- Status: BLOCKED — REJECT
- Audited HEAD: 679b114 — feat(harness): барьер формы milestone-спеки + проба, четыре блокера C-101 закрыты [architect]

## §B — What I did
- Audited the committed harness artifact set, CI invocation, and C-101 claims.
- Executed live false-green fixtures and semantic mutants against the full probe.

## §C — Artifacts / results
- `research/critiques/C-172-harness-milestone-shape-r2.md`
- Done Block: raw command output and exit codes are recorded above.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  On feat/harness-milestone-shape, address REJECT C-172 before requesting another adversary pass. Preserve the harness-track scope. Extend the RED probe with setup-guarded cases for: a mismatched/invalid closing marker inside a fenced block; a non-heading `##Allowed paths`; a committed incomplete artifact whose dirty worktree is complete; a complete non-ASCII filename; and a forbidden H4 section heading. The probe must be red against the corresponding live-derived semantic stubs, then green against the honest barrier. Commit and push the corrected artifact set; provide its full SHA and base for a fresh critic audit.
  ```
- Push status: this REJECT verdict is committed and pushed to `origin/feat/harness-milestone-shape` before handoff.
- Cache: no build cache created.

## §E — Risks / open questions
- Dev/merge remain blocked by harness-track §5.3 until C-172 is resolved and a new adversary verdict exists.

=== END HANDOFF ===
