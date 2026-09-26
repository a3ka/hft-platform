# C-083 — M-60 rebuild plan-time gate

Date: 2026-08-14
Agent: critic
Model: codex/gpt-5, strong per `gates.md` §1 mandate
Branch: `origin/docs/M-60-rebuild-2026-08-14`
Audited base: `origin/main @ c6c62b8`
Audited head: `f98af2c`

## Verdict

REJECT.

The rebuild fixes the main logical shape of the dispatcher mandate: G2 is excluded because it is
already wired in CI, the order is `M-60c′ → G1 → G3 → G6.2`, the M-60c numbers are from the
2026-08-14 tree, G3 names the K-3/K-4 corrections, G4/G5 are template lines and
`COGNITIVE-ONLY`, the preservation table is section-based, and `C-062` is byte-identical to the
old `feat/M-60-mechanisms` copy.

It is still not a passable plan-time artifact set for dispatch to dev.

## Findings

### F-083-1 — BLOCKER — RED and verify artifacts are not committed on the audited subject

`critic.md` says the critic is not launched before architect has committed the milestone file,
RED tests, verify script, and applicable T-contract/trait signatures; absent artifacts produce
`NOT REVIEWED — ARCHITECT ARTIFACTS INCOMPLETE`. `gates.md` §1 says architect commits
milestone + RED tests + verify script before dev.

The audited diff contains only:

- `milestones/M-60b-gate-mechanisms.md`
- `milestones/M-60c-corpus-cleanup.md`
- `research/critiques/C-062-M-60-mechanisms.md`

There is no committed `scripts/tests/red_context_budgets.sh`,
`scripts/tests/red_gate_meta.sh`, `scripts/tests/red_disk_budget.sh`,
`scripts/verify_M-60b.sh`, or `scripts/verify_M-60c.sh` in this branch. The specs explicitly defer
them: M-60b says RED sets are committed later on `feat/M-60b-mechanisms`
(`milestones/M-60b-gate-mechanisms.md:54-58`), and M-60c says `verify_M-60c.sh` is written
later at dispatch time (`milestones/M-60c-corpus-cleanup.md:139-143`).

That leaves the critic auditing plan text instead of the artifact set. I can confirm the prose
names how tests should go red, but I cannot audit anti-placebo behavior, setup guards, executable
verify shape, CI parity, or mutation hooks as committed artifacts.

Condition to clear: commit the declared RED tests and verify scripts on the audited subject
before dev dispatch, or explicitly rescope this branch as a pre-spec document review and run the
real plan-time critic only after the artifact set exists.

T-contracts / trait signatures: N/A for the current committed diff; both milestones declare
`Contract impact: нет` / no code-contract changes.

### F-083-2 — BLOCKER — `feat/M-60-mechanisms` still has non-RED unique cargo; deletion is not safe after current salvage

The dispatcher asked for a fact check here. Current salvage is only `C-062`, and it is identical,
but old `feat/M-60-mechanisms` still carries unique material beyond the two RED sets:

- `milestones/M-60-mechanisms.md`
- `scripts/verify_M-60.sh`
- stale but unique deltas in `docs/09-roadmap-v2.md` and `milestones/BACKLOG.md`
- older divergent carriers for `M-60a`, `M-60b`, and `M-60c`

This matters because the new specs still reference the old branch as a source:
`M-60b` names `origin/feat/M-60-mechanisms @ f0e915b` for the RED sets
(`milestones/M-60b-gate-mechanisms.md:54-58`), and `M-60c` reuses only the normalization
technique from `scripts/verify_M-60.sh:67-72` (`milestones/M-60c-corpus-cleanup.md:147-149`).
Those referenced sources are not committed or archived in the audited branch.

Condition to clear: do not recommend deleting `feat/M-60-mechanisms` until the RED sets and
`verify_M-60.sh` normalization source are landed or archived. The committed specs should say this
explicitly, not just leave branch fate to a later memory-dependent decision.

## Checklist A-H

- A: PASS. Recomputed corpus: rules = 1069 lines / 94274 bytes; `CLAUDE.md` = 100 lines /
  8716 bytes; total = 1169 lines / 102990 bytes. M-60c carries these facts.
- B: FAIL as artifact-set, PASS as prose. Each mechanism names what should go red, but the RED
  files and verify scripts are not committed here.
- C: PASS. M-60c has a `Запрещено / Почему` table.
- D: PASS. Preservation is by sections, not only by aggregate volume.
- E: PASS. Limits are named; G4/G5 are marked `COGNITIVE-ONLY`.
- F: PASS. `C-062` is byte-identical to `origin/feat/M-60-mechanisms`.
- G: FAIL for deletion safety. Old `feat/M-60-mechanisms` still has non-RED unique cargo.
- H: PASS. `TD-105`, `TD-106`, `TD-102`, `C-064`, `A-003`, and `A-005` resolve on the
  fast-forward merge tree.

## Raw Evidence

```
$ git log --oneline origin/main..HEAD
f98af2c docs(M-60b): пересборка спеки от main — G1/G3/G6.2 без G2 (сделан), правки К-3/К-4, GM-16 сожжён [architect-clone]
243eb51 docs(M-60c): пересборка спеки от main — чистка первой, числа от факта 14.08, G4/G5 строками шаблонов [architect-clone]
fda1a68 docs(M-60): salvage C-062 с feat/M-60-mechanisms — аудит-трейл «критик в чужом репо», единственный экземпляр [architect-clone]

$ git diff --stat origin/main...HEAD
 milestones/M-60b-gate-mechanisms.md         | 210 ++++++++++++++++++++++++++++
 milestones/M-60c-corpus-cleanup.md          | 180 ++++++++++++++++++++++++
 research/critiques/C-062-M-60-mechanisms.md | 191 +++++++++++++++++++++++++
 3 files changed, 581 insertions(+)

$ wc -l .claude/rules/*.md CLAUDE.md
   108 .claude/rules/branch-hygiene.md
   112 .claude/rules/commit-discipline.md
   422 .claude/rules/gates.md
   118 .claude/rules/handoff-block.md
    85 .claude/rules/scope-guard.md
   224 .claude/rules/testing.md
   100 CLAUDE.md
  1169 total

$ wc -c .claude/rules/*.md CLAUDE.md
 12171 .claude/rules/branch-hygiene.md
  7509 .claude/rules/commit-discipline.md
 40127 .claude/rules/gates.md
  6610 .claude/rules/handoff-block.md
  7268 .claude/rules/scope-guard.md
 20589 .claude/rules/testing.md
  8716 CLAUDE.md
102990 total

$ git diff --exit-code origin/feat/M-60-mechanisms:research/critiques/C-062-M-60-mechanisms.md HEAD:research/critiques/C-062-M-60-mechanisms.md; echo exit=$?
exit=0

$ git diff --name-status HEAD origin/feat/M-60-mechanisms -- docs/09-roadmap-v2.md milestones/BACKLOG.md milestones/M-60-mechanisms.md milestones/M-60a-docs-freeze.md milestones/M-60b-gate-mechanisms.md milestones/M-60c-corpus-cleanup.md scripts/tests/red_context_budgets.sh scripts/tests/red_gate_meta.sh scripts/verify_M-60.sh research/critiques/C-062-M-60-mechanisms.md
M	docs/09-roadmap-v2.md
M	milestones/BACKLOG.md
A	milestones/M-60-mechanisms.md
M	milestones/M-60a-docs-freeze.md
M	milestones/M-60b-gate-mechanisms.md
M	milestones/M-60c-corpus-cleanup.md
A	scripts/tests/red_context_budgets.sh
A	scripts/tests/red_gate_meta.sh
A	scripts/verify_M-60.sh

$ bash scripts/check_artifact_ids.sh origin/main; echo exit=$?
OK: ни один коммит диапазона origin/..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ bash scripts/check_protected_artifacts.sh origin/main; echo exit=$?
OK: защищённые артефакты целы на HEAD (c6c62b8..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0
```
