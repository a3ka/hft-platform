<!-- GATE-META
milestone: TD-222
audited_repo: a3ka/hft-platform
audited_base: 939d1b91df081731cb114dab956a8af60f93cf2c
audited_head: 5a2636c6c08a56b0482162d3ba443975a4007f3b
verdict: NOTE
-->

# C-261 — TD-222: CI aggregate all needs, round 4

## Verdict: NOTE

`A-041` §2(д) is implemented and its mandatory frozen review passes.  The
former open-ended shell-reachability test is replaced by the closed form of
the two gate jobs: `status-check` has exactly the condition, checkout, and
aggregate-probe steps, while `deploy-catchup` has exactly checkout and its
five canonical commands.  `_invokes` is absent from the aggregate check.

The honest form and A0b pass; A1–A29, including A15–A29, fail as required.
Each of A-041's four mutation axes is reproducible.  The real PR has all 21
checks passing.  No new form-level REJECT is opened: under the A-041 freeze
rule, a further `run`/step/key form is non-canonical and is rejected by the
closed-form check by construction.

Harness-track routing applies: the audited range contains CI and repository
harness artifacts only, not a production process, `crates/**`, contracts, or
normative documentation.  Therefore the full milestone T-contract / trait /
separate-RED artifact chain is intentionally replaced by the harness-track
barrier-plus-probe route.  No `crates/<name>/**` path is touched, so the
critic FA-invariant requirement is not applicable.

## A-041 §2(д) verification

1. The current `ci.yml` form matches `_FORM_JOBS`: `status-check` pins
   `All checks passed`, `ubuntu-latest`, and `always()`, with exactly the
   condition, `actions/checkout@v4`, and the one-line probe.  `deploy-catchup`
   pins its six-step sequence.  The condition remains separately executed by
   A3 and `red_ci_aggregate.sh`; the implementation does not mistake a string
   match for the aggregate decision.
2. `red_deploy_catchup.sh` is green for all 71 scenarios.  It includes A0b
   and makes all of A15–A29 red; its `--battery` control reports 11 mutants,
   zero kill-set mismatches.  The older A1–A14 remain red on their mutants.
3. Four independently edited scratch copies of `deploy_catchup.py` were each
   proved distinct with `cmp` before execution.  Disabling exact `run`
   comparison kills A13–A19 and A28; disabling forbidden step-key detection
   kills A7, A8, A12, A20, A22, and A23; disabling step count kills A11;
   disabling only the name pin kills A27.
4. The required regression checks are green: `red_ci_aggregate.sh` is 8/8,
   `CI=true red_review_fa.sh` is 50/50, and `gh pr checks 232` reports 21
   passing checks on the audited head.

## Non-blocking note

The canonical-form docstring names the terminal A6 link as
`COGNITIVE-ONLY`: a single change can alter both the guard and its own
invocation.  This is the stated, non-blocking limit in A-041 §4, not a claim
of mechanical closure.  A-041 also places its proposed A3 narrowing outside
this decision; it is not a condition of this verdict.

## Done Block

```text
$ git fetch origin; git rev-parse origin/harness/ci-aggregate-all-needs
5a2636c6c08a56b0482162d3ba443975a4007f3b

$ git merge-base origin/main origin/harness/ci-aggregate-all-needs
939d1b91df081731cb114dab956a8af60f93cf2c

$ repeat fetch before verdict
audited_head_before_fetch=5a2636c6c08a56b0482162d3ba443975a4007f3b
audited_head_after_fetch=5a2636c6c08a56b0482162d3ba443975a4007f3b
drift=none
git merge-base --is-ancestor 5a2636c origin/harness/ci-aggregate-all-needs
handoff_sha_is_ancestor_exit=0

$ python3 scripts/deploy_catchup.py check-aggregate; echo exit=$?
OK: джоб `deploy-catchup` зовёт предмет и пробу; он в `status-check.needs`; его красный результат РОНЯЕТ агрегат (проверено исполнением условия, не грепом)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_deploy_catchup.sh; echo exit=$?
ok   A0-честный-ci-агрегат exit=0
ok   A0b-честная-форма-с-name-и-переводом-строки exit=0
ok   A1–A29: каждый объявленный мутант exit=1
сценариев: 71   PASS: 71   FAIL: 0
своих каталогов (red-catchup-*): до 0, после 0 (уборка — trap EXIT + реестр)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_deploy_catchup.sh --battery; echo exit=$?
ok   базовая линия                     честный предмет: kill-set пуст
ok   M1–M11: kill-set совпал
--- батарея: мутантов 11, расхождений kill-set 0
VERDICT: PASS
exit=0

$ mutation controls (each: cmp original mutated; then red_deploy_catchup.sh)
run-comparison: cmp exit=1; A13–A19,A28 failed; 63/71 passed; exit=1
step-key prohibition: cmp exit=1; A7,A8,A12,A20,A22,A23 failed; 65/71 passed; exit=1
step count: cmp exit=1; A11 failed; 70/71 passed; exit=1
name pin only: cmp exit=1; A27 failed; 70/71 passed; exit=1

$ bash scripts/tests/red_ci_aggregate.sh; echo exit=$?
VERDICT: PASS — сценариев: 8, расхождений: 0
exit=0

$ CI=true bash scripts/tests/red_review_fa.sh; echo exit=$?
PASS  W8OK реальный ci.yml чекаута несёт полную проводку — wiring OK
VERDICT: PASS (50/50)
exit=0

$ gh pr checks 232; echo exit=$?
All checks passed ... pass
Deploy catch-up ... pass
... 19 further checks pass
exit=0

$ git diff --check 939d1b9..5a2636c; echo exit=$?
exit=0

$ bash scripts/reserve_artifact_id.sh C
C-261
exit=0
```

## Handoff condition

`NOTE` does not block the harness-track merge.  Architect may proceed with
the PR's normal status-check gate; no source-file change is requested from
this critic.
