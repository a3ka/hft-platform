<!-- GATE-META
milestone: TD-222
audited_repo: a3ka/hft-platform
audited_base: 939d1b91df081731cb114dab956a8af60f93cf2c
audited_head: 116ac4621a108126ba2d0253495d8bc8b49d053e
verdict: ESCALATE
-->

# C-259 — TD-222: CI aggregate all needs, round 3

## Verdict: ESCALATE

R3 closes the concrete C-258 wiring hole: the independent `check-aggregate`
oracle rejects A11--A14, while the honest workflow and the prior C-257
oracles are green.  A new, executable bypass remains, however: `_invokes()`
accepts an invocation occurring after `true ||` even though Bash never runs
that command.  Consequently, the only CI invocation of
`red_ci_aggregate.sh` can be made unreachable without either
`check-aggregate` or `red_deploy_catchup.sh` becoming red.

This is the third critic round for this subject (`C-257`, `C-258`, and this
audit).  `gates.md` §0 requires a fresh-context arbiter after three rounds,
regardless of verdict combination.  The route is therefore **ESCALATE to an
arbiter**, not a third architect↔critic loop and not a founder decision: the
subject is harness wiring, not Boundary C.

Harness-track route applies.  The audited range changes only CI/harness
artifacts and critic files; it introduces no production process, T1 contract,
or normative document.  The track deliberately substitutes the executable
adversary gate for the normal milestone/T-contract/trait-signature set.

## Blocking finding for arbitration

### R4 — `_invokes` mistakes a syntactic segment for a reachable command

`scripts/deploy_catchup.py:612-634` splits a shell line at `&&`, `||`, `;`,
and `|`, then accepts any resulting segment that starts with the requested
needle.  The following replacement of the real `status-check` probe step is
therefore accepted:

```yaml
run: true || bash scripts/tests/red_ci_aggregate.sh
```

The independently constructed mutant (provided to the checker through
`CATCHUP_CI_YML=/dev/stdin`, without editing the subject) returned
`VERDICT: PASS`, exit 0.  But the production runner form shown by the project
is Bash, and `bash --noprofile --norc -eo pipefail -x -c 'true || bash ...'`
printed only `+ true`, then exited 0.  The probe is unreachable; the
`status-check` job remains green.

This is the exact built-not-wired outcome C-258 R3 was intended to prevent.
The current A11--A14 only cover deletion, step-level
`continue-on-error`, terminal `|| true`, and `echo`; none covers an
unreachable command-position segment.  The `_invokes` docstring honestly
names its lack of full shell parsing (`eval` and variable command names), but
this bypass uses neither: it is a literal command accepted by the checker’s
own `||` splitting.

For the next artifact set, the required observable condition is: a
`true || bash scripts/tests/red_ci_aggregate.sh` mutation must make the
independent checker and its RED suite fail.  The arbiter should resolve the
appropriate fail-closed definition of a reachable invocation before another
critic cycle.

## R3 and prior protections verified

- RED-first order is intact: `17b854f` adds only A11--A14 and is red against
  pre-A4 behaviour; `116ac46` changes only `scripts/deploy_catchup.py` to add
  A4.
- On `116ac46`, `red_deploy_catchup.sh` passes all 55 scenarios, including
  A11--A14; `red_ci_aggregate.sh` is 8/8; and `CI=true red_review_fa.sh` is
  50/50.  Thus C-257 R1/R2 were not reopened by R3.
- PR #232 currently has 21 passing checks, including `All checks passed` and
  `Deploy catch-up`.
- A probe-step `${{ false }}` is caught by A4 (exit 1), as is moving the
  invocation to `build-test` (A4 reports no `status-check` invocation,
  exit 1).
- A local `timeout-minutes: 0` mutation is **not** modelled by
  `check-aggregate` (it returned exit 0).  This audit did not establish
  whether GitHub would accept that workflow value; the checker does not
  validate Actions schema or scheduling.  This is a stated modelling limit,
  not the R4 reproduction above.

## Done Block

```text
$ git fetch origin && git rev-parse origin/harness/ci-aggregate-all-needs
116ac4621a108126ba2d0253495d8bc8b49d053e

$ git merge-base origin/main origin/harness/ci-aggregate-all-needs
939d1b91df081731cb114dab956a8af60f93cf2c

$ git merge-base --is-ancestor 116ac46 origin/harness/ci-aggregate-all-needs; echo exit=$?
exit=0

$ repeat fetch before verdict
audited_head_before_fetch=116ac4621a108126ba2d0253495d8bc8b49d053e
audited_head_after_fetch=116ac4621a108126ba2d0253495d8bc8b49d053e
drift=none
audited_base_after_fetch=939d1b91df081731cb114dab956a8af60f93cf2c
exit=0

$ git log --oneline 17b854f^..116ac46
17b854f test(harness): red_deploy_catchup A11-A14 ... (C-258 R3) [architect]
116ac46 fix(harness): check-aggregate A4 ... (C-258 R3) [architect]

$ (at 17b854f) bash scripts/tests/red_deploy_catchup.sh; echo exit=$?
FAIL A11-проба-агрегата-удалена exit=0, ожидалось 1
FAIL A12-проба-агрегата-continue-on-error exit=0, ожидалось 1
FAIL A13-проба-агрегата-глушит-код exit=0, ожидалось 1
FAIL A14-проба-агрегата-echo exit=0, ожидалось 1
сценариев: 55   PASS: 51   FAIL: 4
VERDICT: FAIL
exit=1

$ python3 scripts/deploy_catchup.py check-aggregate; echo exit=$?
OK: джоб `deploy-catchup` зовёт предмет и пробу; он в `status-check.needs`; его красный результат РОНЯЕТ агрегат (проверено исполнением условия, не грепом)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_deploy_catchup.sh; echo exit=$?
ok   A11-проба-агрегата-удалена exit=1
ok   A12-проба-агрегата-continue-on-error exit=1
ok   A13-проба-агрегата-глушит-код exit=1
ok   A14-проба-агрегата-echo exit=1
сценариев: 55   PASS: 55   FAIL: 0
VERDICT: PASS
exit=0

$ bash scripts/tests/red_ci_aggregate.sh; echo exit=$?
VERDICT: PASS — сценариев: 8, расхождений: 0
exit=0

$ CI=true bash scripts/tests/red_review_fa.sh; echo exit=$?
PASS  МАНИФЕСТ ⇄ исполнение: 50 сценариев, состав совпал
VERDICT: PASS (50/50)
exit=0

$ generated `${{ false }}` probe-step mutant | check-aggregate
FAIL A4: шаг `status-check`, зовущий пробу агрегата, обезврежен (if: '${{ false }}' — шаг никогда не исполняется)
VERDICT: FAIL (1 нарушени(й) CI-агрегата)
exit=1

$ generated moved-to-build-test mutant | check-aggregate
FAIL A4: `status-check` не ЗОВЁТ `bash scripts/tests/red_ci_aggregate.sh` в позиции команды
VERDICT: FAIL (1 нарушени(й) CI-агрегата)
exit=1

$ generated timeout-minutes: 0 mutant | check-aggregate
VERDICT: PASS
exit=0

$ generated `true || bash scripts/tests/red_ci_aggregate.sh` mutant | check-aggregate
VERDICT: PASS
exit=0

$ bash --noprofile --norc -eo pipefail -x -c 'true || bash scripts/tests/red_ci_aggregate.sh'; echo exit=$?
+ true
exit=0

$ gh pr checks 232; echo exit=$?
All checks passed  pass
Deploy catch-up     pass
... 19 further checks pass
exit=0

$ git ls-remote origin refs/reserved/C-259
baae28be8ddfa04dbf6630b3c2cbdbecf54e3d9f  refs/reserved/C-259
exit=0
```

## Required route

Fresh-context **arbiter**: resolve R4’s reachability requirement and direct
the next artifact set.  Do not merge PR #232 while this ESCALATE verdict is
open.
