<!-- GATE-META
milestone: TD-222
audited_repo: a3ka/hft-platform
audited_base: 939d1b91df081731cb114dab956a8af60f93cf2c
audited_head: 1df8e6d2af52bfe94b958e9d6dbb2ff607d77013
verdict: REJECT
-->

# C-258 — TD-222: CI aggregate all needs, round 2

## Verdict: REJECT

C-257 R1 and R2 are closed: the probe now binds the condition step uniquely,
executes it for every `needs` member, and the two old consumers and the live
PR are green.  The new protection is nevertheless not mergeable yet.  A
one-line deletion of its only CI invocation leaves the aggregate, the probe
when run locally, `deploy_catchup.py check-aggregate`, and `red_review_fa` all
green.  Thus the property introduced by this revision can silently cease to
run on every future PR.

Harness-track route applies.  The audited range changes only
`.github/workflows/ci.yml`, `scripts/tests/red_ci_aggregate.sh`, and the prior
critic artifact; no production path, T1 contract, or product milestone
artifact is in scope.

## Blocking finding

### R3 — the new probe has no independent wiring oracle

The sole production invocation of the new probe is
`.github/workflows/ci.yml:630`.  `rg -n 'red_ci_aggregate' .github scripts`
returns only that line and the probe file itself.  The probe checks the
workflow's condition body only after somebody has already invoked it
(`scripts/tests/red_ci_aggregate.sh:42-83`); it cannot establish that its own
step still exists.

The neighbouring guards do not cover this edge.  The catch-up checker proves
that *itself* is invoked from `deploy-catchup`
(`scripts/deploy_catchup.py:731-741`), while W8 only requires `review-fa` in
`status-check.needs` and in the handwritten condition
(`scripts/tests/red_review_fa.sh:1013-1019`).  Neither requires an executable
`bash scripts/tests/red_ci_aggregate.sh` step in `status-check`.

I copied the real `ci.yml` and removed only line 630.  The mutant differs from
the original (`cmp` exit 1), yet all three surviving consumers were green:

```text
$ CI_AGG_YML=<deleted-step.yml> bash scripts/tests/red_ci_aggregate.sh
VERDICT: PASS — сценариев: 8, расхождений: 0
exit=0

$ CATCHUP_CI_YML=<deleted-step.yml> CATCHUP_REPO_ROOT=$PWD \
    python3 scripts/deploy_catchup.py check-aggregate
VERDICT: PASS
exit=0

$ CI=true REVIEW_FA_CI=<deleted-step.yml> bash scripts/tests/red_review_fa.sh
VERDICT: PASS (50/50)
exit=0
```

This is not a harmless limitation: harness-track §3 says `status-check` runs
the same probe on a clean checkout at every push, and §5 requires the probe's
honest/stub controls before merge.  After this deletion the original
16-versus-18 omission can recur without the new anti-placebo test ever being
run.  The first condition step still guards today's jobs, but the regression
oracle is built-not-wired.

Required repair: add an independent, executable-wiring check for the
`status-check` invocation of this probe, with a RED mutant that deletes or
disarms that step.  It must be outside `red_ci_aggregate.sh` itself; a probe
cannot certify its own CI wiring by being run manually.  Re-run the deleted
step mutant against that external check.

## C-257 findings closed

### R1 — bound to the real condition step

`red_ci_aggregate.sh:52-56` selects exactly one `status-check` step containing
`needs.<job>.result`; zero or multiple candidates are SETUP failures.  The
required healthy decoy before a broken real condition produced `SETUP: ... 2`
and exit 1.  Other alternate forms also fail closed: condition moved entirely
to `env` gives zero candidates; two condition steps give two; a broken body
with a `needs` reference only in a comment is executed and all 18 failures are
reported blind.

### R2 — existing consumers and live CI remain green

The handwritten first aggregate step remains first
(`.github/workflows/ci.yml:618-622`); checkout and the new probe follow it
(`:628-630`).  `deploy_catchup.py check-aggregate`, its 51-scenario RED probe,
and `CI=true red_review_fa.sh` pass.  PR #232 is green: run `36319368312`, job
`108621772484` (`All checks passed`) executed the new step and printed
`VERDICT: PASS — сценариев: 8, расхождений: 0`.

## Verified non-blocking properties

- Base has 18 `needs` but 16 condition references; exactly
  `roadmap-sync` and `secret-material` were missing.  Head has 18 and 18, with
  identical members.  The previous 16 checks remain in both sets.
- The real probe passes eight scenarios.  It rejects a removed arbitrary
  condition member (`rollout-composition`), removed `contracts` from `needs`,
  missing `always()`, and `continue-on-error` on the condition step.
- The diff adds no `permissions:` declaration.  The live log executes the
  condition before `actions/checkout@v4`; the checkout cannot alter that prior
  decision.  The live status job completed in 5 seconds; the local probe took
  under one second.
- `git diff --check` is clean.  No source file was edited during this audit.

## Done Block

```text
$ git fetch origin && git rev-parse origin/harness/ci-aggregate-all-needs
1df8e6d2af52bfe94b958e9d6dbb2ff607d77013
exit=0

$ git merge-base origin/main origin/harness/ci-aggregate-all-needs
939d1b91df081731cb114dab956a8af60f93cf2c
exit=0

$ git merge-base --is-ancestor 1df8e6d origin/harness/ci-aggregate-all-needs
exit=0

$ bash scripts/tests/red_ci_aggregate.sh
ok    реальный агрегат: каждый джоб needs роняет условие, все success проходят, cancelled/skipped — отказ, состав полон
ok    мутант «secret-material выпал из условия» пойман
ok    мутант «roadmap-sync выпал из условия» пойман
ok    мутант «джоб выпал из needs» пойман
ok    мутант «условие «падать всегда»» пойман
ok    мутант «снят if: always()» пойман
ok    мутант «здоровая копия-приманка + сломанное настоящее» пойман
ok    мутант «шаг-условие continue-on-error» пойман
VERDICT: PASS — сценариев: 8, расхождений: 0
exit=0

$ CI_AGG_YML=<base-ci.yml> bash scripts/tests/red_ci_aggregate.sh
FAIL  реальный агрегат: провал джоба НЕ роняет агрегат — выпали из условия: roadmap-sync, secret-material
VERDICT: FAIL — сценариев: 8, расхождений: 3
exit=1

$ isolated real-ci.yml mutants (each starts with cmp original <mutant>; cmp exit=1)
drop rollout-composition from condition  -> FAIL, named rollout-composition; exit=1
drop contracts from needs                -> FAIL, named contracts; exit=1
drop status-check.if: always()           -> FAIL, named always(); exit=1
condition-step continue-on-error         -> FAIL, named обезврежен; exit=1
healthy decoy + broken real body          -> SETUP 2 candidates; exit=1
condition moved into env                  -> SETUP 0 candidates; exit=1
two steps share condition                 -> SETUP 2 candidates; exit=1
broken body, needs ref only in comment    -> FAIL, all 18 jobs blind; exit=1

$ python3 scripts/deploy_catchup.py check-aggregate
OK: джоб `deploy-catchup` зовёт предмет и пробу; он в `status-check.needs`; его красный результат РОНЯЕТ агрегат (проверено исполнением условия, не грепом)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_deploy_catchup.sh
сценариев: 51   PASS: 51   FAIL: 0
своих каталогов (red-catchup-*): до 0, после 0 (уборка — trap EXIT + реестр)
VERDICT: PASS
exit=0

$ CI=true bash scripts/tests/red_review_fa.sh
PASS  W8OK реальный ci.yml чекаута несёт полную проводку — wiring OK
PASS  МАНИФЕСТ ⇄ исполнение: 50 сценариев, состав совпал
SPEC_ROWS=50
PASS  СПЕКА⇄МАНИФЕСТ: 50 строк §4 совпали в обе стороны
VERDICT: PASS (50/50)
exit=0

$ gh pr checks 232
All checks passed  pass  5s  https://github.com/a3ka/hft-platform/actions/runs/36319368312/job/108621772484
... 19 further checks pass
exit=0

$ gh run view 36319368312 --job 108621772484 --log
All checks passed  Run if [[ ... ]]; then
All checks passed  All checks passed
All checks passed  Проба агрегата — каждый джоб needs роняет условие
VERDICT: PASS — сценариев: 8, расхождений: 0
exit=0

$ git fetch origin; git rev-parse origin/harness/ci-aggregate-all-needs
1df8e6d2af52bfe94b958e9d6dbb2ff607d77013
audited_head_before_fetch=1df8e6d2af52bfe94b958e9d6dbb2ff607d77013
drift=none
exit=0

$ bash scripts/reserve_artifact_id.sh C
C-258
exit=0
```
