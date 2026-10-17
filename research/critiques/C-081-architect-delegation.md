# C-081 — critic: architect delegation process patch

- **Subject:** `origin/docs/architect-delegation`, final state `1d54271`.
- **Range audited:** `origin/main..HEAD` = 5 commits (`5f3f747`, `8d474e7`, `b64ffce`, `bda5e52`, `1d54271`).
- **Verdict:** **NOTE** — ship as interim process text; do not treat it as the final control. The milestone-bypass barrier named in the profile should be a follow-up mechanism round.
- **R-064:** read as required, not treated as binding. Final tree implements its material replacement.

## Artifact Set

This is not a normal code milestone, so the standard T-contract / trait signature / RED-test /
verify-script / milestone-file set is N/A. The committed artifact set is process-layer only:

- `.claude/agents/architect.md`
- `.claude/rules/gates.md`
- `research/reviews/R-064-architect-delegation.md`

No `contracts/**`, `crates/**`, `*/tests/**`, `scripts/verify_M-*.sh`, or `milestones/**`
files are changed in the final diff.

## Judgment

### N-1 — carrier is correct

The rule belongs in `.claude/agents/architect.md`, not the shared rule core.

Measurements:

```text
origin/main architect profile: 4246 bytes
HEAD        architect profile: 7470 bytes
origin/main rules+CLAUDE core: 102869 bytes
HEAD        rules+CLAUDE core: 102990 bytes
```

The shared core paid only the `gates.md` asymmetry fix (+121 bytes). The delegation rule is paid
by architect/architect-clone sessions and not by the other roles. That matches the hierarchy in
`docs/plans/process-layer-audit-2026-08-13.md`: role profile is a better carrier than core text
when no stronger carrier exists.

Checked stronger carriers:

- `.claude/wrappers/*` cover pi roles; native architect/reviewer/risk-critic do not pass through them.
- Done Block templates, verify scripts, and CI run after work has happened; they do not observe the moment where architect chooses a subagent type.
- No repo `.claude/settings.json` hook exists for Agent-tool enforcement in this tree.

So `COGNITIVE-ONLY, барьера нет` is honest for the launch-time authorship rule as implemented today.

### N-2 — text alone is not a solution, but the final tree does not pretend it is

The concern is real: `.claude/agents/architect.md:25` already said not to bypass the critic gate,
and the author still did. Adding more text to the same profile would be ritual if it stopped
there.

Final state avoids that in two ways:

- `gates.md:357-360` fixes the asymmetry at the source: critic verdict can satisfy §9 recheck, but §9 recheck cannot replace critic.
- `architect.md:59-60` names the candidate barrier: non-status milestone-spec changes in a push range require a `C-*` naming the milestone or a waiver trailer.

That makes the text acceptable as an interim rule. It is not a substitute for the barrier.

### N-3 — candidate barrier is cheap and mostly well-scoped

The final predicate is better than the rejected `§4.1/§4.2` predicate from R-064. It is implementable with the same push-range pattern as `check_docs_freeze.sh`:

```text
milestones/M-NN-*.md changed non-status
AND no research/critiques/C-* mentions M-NN
AND no waiver trailer
=> FAIL
```

Expected edge cases:

- status-column-only changes should be carved out mechanically;
- close-out proofs, renumbering, and pure exposition need the waiver path;
- the mechanism round should name the exact waiver trailer string and test both false-positive and false-negative cases.

The barrier should not replace the whole architect-profile section. It covers milestone-spec form changes, not the launch rule (`subagent_type: architect`) or the authorship-vs-judgment boundary.

### N-4 — independence condition is satisfied

The final text handles the same-profile concern:

- fresh-context clone required; fork is explicitly disallowed for §0/§9 (`architect.md:38-40`);
- model is named in the launch, not inferred from frontmatter (`architect.md:39-41`);
- clone mandate must narrow inherited Writes (`architect.md:47-48`);
- self-profile edits are judged by a clone only while the subject is on a branch (`architect.md:48`).

That preserves the point of independence: the clone shares the role contract, not the author
context or the working-tree self-reference.

### N-5 — numeric claims: one accurate, two stale

M-61 critique count is accurate:

```text
research/critiques/C-069-M-61-artifact-ids.md
research/critiques/C-070-M-61-rev2.md
research/critiques/C-071-M-61-rev3.md
```

The audit/commit-body numbers for corpus size and launch rate no longer match current fact:

```text
audit claim: 100461 bytes core; ~6.3 launches/day (88 / 14)
current fact: 102869 bytes core at origin/main; 99 gate artifacts in 14 days ~= 7.1/day
```

This is not a blocker because the final normative text does not embed those numbers, and the
updated facts strengthen the carrier argument rather than weakening it. Future references should
say "audit-time measurement" or rerun the commands.

## Validation

```text
$ git log --oneline origin/main..HEAD
1d54271 docs(rules): §9 — асимметрия названа явно: перепроверка критика НЕ заменяет [architect]
bda5e52 docs(agents): раздел переписан по вердикту R-064 — 32 строки в 27, пять фактических ошибок снято [architect]
b64ffce docs(review): R-064 — ВНЕСТИ СОКРАЩЁННЫМИ ОБА, REJECT текущей редакции по §9 [fable-recheck]
8d474e7 docs(agents): вердикт клона не закрывает plan-time гейт — цепочка после него [architect]
5f3f747 docs(agents): граница делегирования клонам — в профиль architect'а, а не в общий корпус [architect]

$ EVENT_NAME=push PUSH_BEFORE=$(git merge-base origin/main HEAD) bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0

$ EVENT_NAME=push PUSH_BEFORE=$(git merge-base origin/main HEAD) bash scripts/check_protected_artifacts.sh; echo exit=$?
OK: защищённые артефакты целы на HEAD (0f907d1..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=push PUSH_BEFORE=$(git merge-base origin/main HEAD) bash scripts/check_artifact_ids.sh; echo exit=$?
OK: ни один коммит диапазона 0f907d1..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ git diff --check origin/main...HEAD
<no output>

$ bash scripts/install_hooks.sh
VERDICT: PASS (8/8 сценариев)
```

## Final Verdict

**NOTE.** The final patch is rational as targeted, explicitly cognitive interim text plus the
canonical `gates.md` asymmetry fix. It should proceed with one follow-up: implement the
milestone-spec critic barrier, or the known failure mode remains guarded only by memory.
