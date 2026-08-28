<!-- GATE-META
milestone: C-101
audited_repo: a3ka/hft-platform
audited_base: 767ebae1e7c0cd5a9127fea0ada55d5a0f300549
audited_head: 9ec104f79bb00636f64130bac239d72d053672a1
verdict: REJECT
-->

# C-175 — REJECT: adversary audit `harness-milestone-shape`, round 3

## Subject and route

- Route: `docs/workflow/harness-track.md` §§3, 5.3.  This is the required
  fresh-context adversary artifact and is a condition of merge.
- Audited range: `767ebae1e7c0cd5a9127fea0ada55d5a0f300549..9ec104f79bb00636f64130bac239d72d053672a1`
  on `feat/harness-milestone-shape`.
- Audited artifacts: `scripts/check_milestone_shape.sh`,
  `scripts/tests/red_milestone_shape.sh`, and the `milestone-shape` / `status-check`
  wiring in `.github/workflows/ci.yml`.

## Verdict: REJECT

### B-8 — a fence is identified only by character, not by its complete closing rule (live false green)

`visible_body()` now distinguishes backticks from tildes, closing C-173 B-5,
but it stores only ````` or `~~~` at
`scripts/check_milestone_shape.sh:86-96`.  A matching run of *any* three
characters is accepted as a close.  The parser neither retains the opening
run length nor requires a close to have only whitespace after its marker.

Both inputs below leave `## Allowed paths` inside a fenced code block, so the
barrier must reject the incomplete milestone.  The live barrier instead
prints `OK` and returns 0.

1. An opening four-backtick fence is closed by three backticks.  A closing
   run may not be shorter than its opening run.
2. A three-backtick line followed by `not-a-closing-fence` is treated as a
   close.  That trailing non-whitespace makes it code content, not a closing
   fence.

The round-3 RED suite covers only the marker-*character* mismatch
(````...~~~`).  It contains neither length mismatch nor trailing-text case.
Thus the honest current barrier is already a semantic weakening that the
7-stub battery does not observe: its complete probe remains green at 32/32.
This violates the harness-track §5.1 anti-placebo requirement and
`testing.md`'s requirement that a gate measure the form it promises.

## Required condition for re-audit

1. Make `visible_body()` retain the opening fence character and run length;
   accept a close only when it uses that character, has length at least the
   opening run, and its remainder is whitespace only.
2. Add setup-guarded RED fixtures for both reproductions above.  After the
   fix, derive live-code mutations that remove the run-length check and the
   trailing-text check; the probe must be red against each, not merely reject
   the honest fixtures.
3. Re-run the honest probe and the complete adversarial battery, then present
   a new committed tip for a fresh audit.

## Confirmed in this revision

- C-173's character-mismatch fence case, no-space heading, H4 heading,
  committed-HEAD, and Unicode-name cases are present and the honest probe
  reports 32 PASS / 0 FAIL plus 7/7 battery kills.
- The self-test counter now mutates `FAIL` in the parent shell; it is no
  longer hidden inside command substitution.
- The CI step name matches the live count: `32 scenarios + battery of 7`.
  The old `14` and `24` step-name literals are absent.  Two surrounding
  comments still call the battery "four"; this is stale prose, not the
  blocking defect.
- `milestone-shape` remains in both `status-check.needs` and its fail-closed
  condition.

## Done Block

```text
$ git diff --check 767ebae1e7c0cd5a9127fea0ada55d5a0f300549 9ec104f79bb00636f64130bac239d72d053672a1
exit=0

$ bash scripts/tests/red_milestone_shape.sh
  батарея ослаблений: поймано 7 из 7
PASS=32 FAIL=0 (сценариев: 32)
VERDICT: PASS
уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: 0
exit=0

$ four-backtick opening / three-backtick pseudo-close against live checker
=== проверяю форму: milestones/M-99-four-fence.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
checker_exit=0
# Expected: exit=1; `## Allowed paths` remains inside the four-backtick fence.

$ trailing text after a three-backtick pseudo-close against live checker
=== проверяю форму: milestones/M-99-trailing-fence.md ===
OK: все введённые milestone-спеки несут обязательные разделы формы (§6)
checker_exit=0
# Expected: exit=1; a closing fence may not have non-whitespace trailing text.

$ bash -n scripts/check_milestone_shape.sh scripts/tests/red_milestone_shape.sh
exit=0

$ rg -n 'Проба барьера \(14|Проба барьера \(24' .github/workflows/ci.yml
exit=1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC, ISO-8601): 2026-08-28T15:31Z
- Milestone: C-101 / harness-track `harness-milestone-shape`
- Status: BLOCKED — REJECT
- Audited HEAD: 9ec104f — fix(harness): C-173 — пять находок адверсария закрыты, круг 3 [architect]

## §B — What I did
- Audited the committed harness range and CI wiring independently of the author narrative.
- Executed the honest probe and two live fence-boundary false-green fixtures.

## §C — Artifacts / results
- `research/critiques/C-175-harness-milestone-shape-r3.md`
- Done Block: raw commands and exit codes are recorded above.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  On feat/harness-milestone-shape, resolve REJECT C-175 before requesting a new adversary audit. Fix visible_body() so a fenced block closes only with the same marker character, a run at least as long as the opening run, and whitespace-only suffix. Add setup-guarded RED fixtures for (1) four-backtick opening followed by three-backtick pseudo-close and (2) a three-backtick pseudo-close with trailing text; derive live-code mutations that remove each rule and make the probe red against them. Re-run the honest probe and complete battery, commit and push the artifact set, then supply the new full SHA and base.
  ```
- Push status: pending this verdict commit on `origin/feat/harness-milestone-shape`.
- Cache: no build cache created.

## §E — Risks / open questions
- Merge remains blocked by `docs/workflow/harness-track.md` §5.3 until this verdict is resolved and a new adversary artifact is committed.

=== END HANDOFF ===
