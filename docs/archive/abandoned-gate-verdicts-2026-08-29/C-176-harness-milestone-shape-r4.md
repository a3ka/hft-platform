<!-- GATE-META
milestone: C-101
audited_repo: a3ka/hft-platform
audited_base: c4cfb8564fb5549060762c7056485065557afee0
audited_head: d0b9abb00bbe1763f7d435ab88adf427366af891
verdict: REJECT
-->

# C-176 — REJECT: harness adversary audit `harness-milestone-shape`, round 4

## Subject

- Route: `docs/workflow/harness-track.md` §§3 and 5.  This committed
  fresh-context adversary verdict is a merge precondition under §5.3.
- Audited branch: `feat/harness-milestone-shape`.
- Audit base: `c4cfb8564fb5549060762c7056485065557afee0`; it is the merge-base
  of the audited tip, not an inferred replacement.
- Audited tip: `d0b9abb00bbe1763f7d435ab88adf427366af891`.
- Audited revision range: `c7acbf57f180c895894d296e5b94f28938d2aa66..d0b9abb00bbe1763f7d435ab88adf427366af891`.
- Artifact-ID reservation: `bash scripts/reserve_artifact_id.sh C` → `C-176`.

## Verdict: REJECT

The round-4 patch correctly closes C-175's two fenced-code boundaries: the
honest probe is green at 36/36 and catches the nine supplied mutants.  It is
nevertheless green while the live barrier accepts hidden or non-existent
required sections, and it remains green against an additional live-derived
weakening.  This fails the harness-track §5.1 anti-placebo requirement and
the four gate-integrity properties in `testing.md`.

### B-9 — raw HTML block can supply the only `Allowed paths` heading (live false green)

`visible_body()` at `scripts/check_milestone_shape.sh:97-128` removes only
fenced code and HTML comments.  It prints the contents of a CommonMark raw
HTML `<script>` block.  Therefore the following newly committed milestone has
no visible `Allowed paths` section, but the live checker returns 0:

```markdown
# M-99 — adversarial
## Objective
body
## §Tasks
body
## Acceptance
body
<script>
## Allowed paths
</script>
```

CommonMark treats `script` (as well as `pre`, `style`, and `textarea`) as a
raw HTML block through its matching end tag; Markdown headings inside it are
not headings in the document.  The checker instead sees the raw line at
`check_section()` (`:143-146`) after `visible_body()` printed it.  The current
probe has comment and fenced-code cases (`red_milestone_shape.sh:295-307`),
but no raw-HTML-block case, so its 36/36 result does not observe this bypass.

**Condition to clear.** Define the accepted Markdown surface explicitly, then
make `visible_body()` match it.  If it continues to promise a visible
CommonMark body, it must hide all applicable raw-HTML block forms (at minimum
`pre`, `script`, `style`, and `textarea`, with their real end conditions), not
just comments.  Add setup-guarded committed fixtures for those forms and a
live-derived mutant that disables that handling; the probe must be red against
that mutant.  A deliberate narrower grammar is acceptable only if the
barrier's claim and tests name that restriction rather than calling the
remaining input a visible CommonMark body.

### B-10 — a different section name passes by sharing the required prefix (live false green)

The four header regexes at `scripts/check_milestone_shape.sh:143-146` end
immediately after their required text.  They do not require a title boundary.
Thus this is accepted as `Allowed paths`, although it is a distinct heading
named `Allowed pathsNOT-A-SECTION`:

```markdown
## Allowed pathsNOT-A-SECTION
```

The result is the same live false green as B-9: the checker returns 0 when
this is the only candidate.  The prose-only test at
`red_milestone_shape.sh:315-316` proves only that the name occurs in an ATX
heading, not that the heading has the required name.  The same prefix bug
exists for `Objective`, `§Tasks`, and `Acceptance`.

**Condition to clear.** Make each required heading match its complete title:
after the title permit only valid heading termination (end-of-line, permitted
space/tab padding, and, if supported, a valid closing ATX sequence).  Add
negative prefix/suffix fixtures for all four required names and a
live-derived boundary-removal mutant; the complete probe must fail against it.

### B-11 — the probe misses a CommonMark indentation weakening

The accepted-heading patterns at `scripts/check_milestone_shape.sh:143-146`
currently begin at byte column zero.  A derived mutant changing only their
prefix from `^#{2,3}` to `^[ ]*#{2,3}` leaves the entire honest probe green:
`PASS=36 FAIL=0`, including the advertised 9/9 battery.  Yet that mutant
accepts the only `Allowed paths` candidate below, which is four-space
indented code, not a heading:

```markdown
    ## Allowed paths
```

The live checker happens to reject this input, but the battery therefore does
not pin the ATX/code boundary it claims to enforce.  It also rejects valid
CommonMark ATX headings with one to three leading spaces or a tab; CommonMark
permits up to three indentation spaces, while four is indented code.  The
official rule also permits a tab after the opening hash run.

**Condition to clear.** State whether the harness intentionally narrows this
to column-zero headings.  Otherwise implement the CommonMark boundary by
column (including tabs): positive fixtures for 0, 1, and 3 indentation spaces
and a tab after the hashes; negative fixtures for four-space and tab-indented
code.  In either policy, add the four-space fixture and a live-derived
whitespace-widening mutant so the probe is red when indented code begins to
count as a section.

## Non-blocking note

- `N-1` — the author says the stale “four” prose was removed, but
  `.github/workflows/ci.yml:229` still says the self-check runs a battery “из
  четырёх ослаблений” while the executable battery and CI step name correctly
  say 9.  This does not change the exit code, but should be corrected with the
  re-audit fix so the surrounding explanation again matches the mechanism.

## Confirmed in this round

- C-175's matching-marker, closing-run-length, and blank-tail rules are
  implemented at `scripts/check_milestone_shape.sh:100-125` and their two
  direct mutants are caught by the probe.
- The known C-173 coverage remains: normal backtick/tilde fences, comments,
  prose substring, no-space and H4 headers, committed-HEAD versus dirty tree,
  Unicode filenames, and renames.
- The actual branch range changes only the three declared harness artifacts;
  the `milestone-shape` job is still both a `status-check.needs` member and a
  fail-closed prerequisite.

## Done Block

```text
$ git ls-remote origin refs/heads/feat/harness-milestone-shape
d0b9abb00bbe1763f7d435ab88adf427366af891	refs/heads/feat/harness-milestone-shape
exit=0

$ git diff --name-status c7acbf57f180c895894d296e5b94f28938d2aa66..d0b9abb00bbe1763f7d435ab88adf427366af891
M	.github/workflows/ci.yml
M	scripts/check_milestone_shape.sh
M	scripts/tests/red_milestone_shape.sh
exit=0

$ bash scripts/tests/red_milestone_shape.sh
  батарея ослаблений: поймано 9 из 9
PASS=36 FAIL=0 (сценариев: 36)
VERDICT: PASS
уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: 0
exit=0

$ committed <script>-hidden Allowed-paths fixture against the live checker
=== проверяю форму: milestones/M-99-adversary.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
checker_exit=0
# Expected: 1. The only candidate is inside a CommonMark raw HTML block.

$ committed `## Allowed pathsNOT-A-SECTION` fixture against the live checker
=== проверяю форму: milestones/M-99-adversary.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
checker_exit=0
# Expected: 1. The required section name is absent.

$ BARRIER_OVERRIDE=<live-derived ^#{2,3}→^[ ]*#{2,3} mutant> bash scripts/tests/red_milestone_shape.sh
  батарея ослаблений: поймано 9 из 9
PASS=36 FAIL=0 (сценариев: 36)
VERDICT: PASS
probe_exit=0

$ four-space indented-code fixture against that mutant
=== проверяю форму: milestones/M-99-indented-heading.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
checker_exit=0
# Expected: 1. Four spaces introduce an indented code block.

$ git diff --check c7acbf57f180c895894d296e5b94f28938d2aa66..d0b9abb00bbe1763f7d435ab88adf427366af891
exit=0

$ bash -n scripts/check_milestone_shape.sh scripts/tests/red_milestone_shape.sh
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=c7acbf57f180c895894d296e5b94f28938d2aa66 bash scripts/check_milestone_shape.sh
OK: в диапазоне c7acbf5..HEAD новых milestone-спек нет — проверять нечего
exit=0

$ bash scripts/reserve_artifact_id.sh C
C-176
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC, ISO-8601): 2026-08-28T20:00Z
- Milestone: C-101 / harness-track `harness-milestone-shape`
- Status: BLOCKED — REJECT
- HEAD: d0b9abb — fix(harness): C-175 — закрытие забора судится ТРЕМЯ условиями, круг 4 [architect]

## §B — What I did
- Audited the committed round-4 range and CI wiring, then ran the honest
  probe and fresh committed adversarial fixtures.
- Derived and executed a regex-widening mutant not represented by the nine
  supplied mutations.

## §C — Artifacts / results
- `research/critiques/C-176-harness-milestone-shape-r4.md`
- Done Block: raw command output and exit codes are recorded above.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  On feat/harness-milestone-shape, resolve REJECT C-176 before a fresh
  harness-track adversary pass. Extend the committed probe to reject required
  headings hidden in CommonMark raw HTML blocks, to require an exact required
  section title rather than a prefix, and to pin the chosen ATX indentation
  boundary (including a four-space code-block negative case) against
  live-derived mutants. Preserve all C-173/C-175 coverage, update the CI
  explanatory count, run the honest probe and complete mutation battery, then
  commit and push the three harness artifacts and provide the new full SHA.
  ```
- Push status: this REJECT verdict is committed and pushed to
  `origin/feat/harness-milestone-shape` as its own explicit-path commit.
- Cache: no build cache created.

## §E — Risks / open questions
- Merge remains blocked by `docs/workflow/harness-track.md` §5.3 until a new
  fresh-context adversary verdict resolves B-9 through B-11.

=== END HANDOFF ===
