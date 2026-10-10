<!-- GATE-META
milestone: TD-222
audited_repo: a3ka/hft-platform
audited_base: 939d1b91df081731cb114dab956a8af60f93cf2c
audited_head: be53f7091fdbcd3e74728f8b695ff0dc908794d6
verdict: REJECT
-->

# C-257 — TD-222: CI aggregate all needs

## Verdict: REJECT

The dynamic aggregate itself closes the recorded 16/18 omission, but this
revision is not mergeable.  Its new probe can execute a different, preceding
Python heredoc and stay green while the real aggregate accepts failures.  In
addition, the new `toJSON(needs)` representation breaks two existing CI
anti-placebo checks, so the real PR's required `All checks passed` is red.

Harness-track route applies: the subject changes only `.github/workflows/**`
and `scripts/tests/**`; there is no production path, T1 contract, crate, or
milestone artifact to audit.

## Blocking findings

### R1 — probe is not uniquely bound to the actual aggregate body

`scripts/tests/red_ci_aggregate.sh:25` takes the first `python3 - <<'PY'`
after `status-check`, without binding it to the `run` step whose environment is
`NEEDS_JSON: ${{ toJSON(needs) }}` (`.github/workflows/ci.yml:631-634`) or
requiring one and only one candidate.

I inserted a preceding, valid but unrelated heredoc containing a healthy copy
of the aggregate, and changed the real body's
`.github/workflows/ci.yml:639` predicate to `bad = []`.  The probe still
returned `VERDICT: PASS — сценариев: 7, расхождений: 0`, exit 0.  Executing the
second (real) body with `secret-material=failure` then printed `All checks
passed (3 jobs)` and exited 0.  This is precisely the forbidden case: the
probe is green on foreign code rather than failing SETUP.

Required repair: select the aggregate step structurally by its `NEEDS_JSON`
binding (or a dedicated immutable marker), reject zero or multiple matches as
SETUP, and add this two-heredoc + broken-real-body mutant to the probe.  A
different indentation and a renamed job already fail SETUP; the selection must
have the same fail-closed property.

### R2 — the refactor leaves the required CI aggregate red

The new format invalidates two existing wiring/oracle consumers:

- `scripts/deploy_catchup.py:665-704` models only explicit
  `needs.<job>.result` substitutions.  With the JSON environment it supplies
  no `NEEDS_JSON`, so `check-aggregate` fails its all-success positive control
  with `JSONDecodeError` (`scripts/deploy_catchup.py:708`).
- `scripts/tests/red_review_fa.sh:1017-1019` still requires the literal
  `needs.review-fa.result`; its live-workflow `W8OK` is therefore red even
  though `review-fa` is in `status-check.needs`.

The live PR run `36318350195`, job `108618967702`, confirms that
`if: always()` ran the aggregate after the two failed dependencies and that
`toJSON(needs)` contained every direct dependency: it printed 18 outcomes,
including `failure deploy-catchup` and `failure review-fa`, followed by
`One or more checks failed: deploy-catchup, review-fa`, exit 1.  Consequently
the required check is red, not `All checks passed (18 jobs)`.

Update these two consumers and their RED scenarios for the dynamic form, then
show the real PR has a green aggregate.  This is not cosmetic: the
deploy-catchup check exists to prove that its own red state holds the required
check.

## Verified non-blocking properties

- On audit base, `status-check.needs` had 18 jobs while the handwritten result
  condition had 16; the exact omissions were `roadmap-sync` and
  `secret-material`.  The first artifact commit `4837ef2` made the new probe
  red against that old workflow (SETUP, exit 1); `be53f70` makes its seven
  ordinary scenarios pass.
- On the audited head, direct execution of the real body rejected a single
  `failure` for every one of the 18 declared dependencies (`passed=18,
  failed=0`) and all-success printed `All checks passed (18 jobs)`.
- Mutants that ignore `secret-material`, remove it from `needs`, or remove the
  empty-needs guard each made the probe red (exit 1).  Renaming the job or
  changing its indentation made it red as SETUP (exit 1).
- `branch-health` is the sole non-barrier exception: the workflow has 20 jobs,
  18 are `needs`, and the remaining two are `status-check` itself and
  `branch-health`.  Its `|| true` and its documented observer role agree with
  `docs/workflow/reading-map.md` §2, Ярус S.
- The change adds no `permissions:` declaration and `status-check` checks out
  then runs the probe; checkout failure cannot turn a failed dependency green.
- GitHub documents that a job with `needs` will be skipped after a failed or
  skipped dependency unless `jobs.<job_id>.if` uses `always()`; the `needs`
  context contains direct dependencies and each result is one of `success`,
  `failure`, `cancelled`, or `skipped`.
  [Using jobs](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-jobs) ·
  [Contexts](https://docs.github.com/en/actions/reference/workflows-and-actions/contexts)

## Done Block

```text
$ git fetch origin && git rev-parse origin/harness/ci-aggregate-all-needs
be53f7091fdbcd3e74728f8b695ff0dc908794d6

$ git merge-base origin/main origin/harness/ci-aggregate-all-needs
939d1b91df081731cb114dab956a8af60f93cf2c

$ git merge-base --is-ancestor be53f70 origin/harness/ci-aggregate-all-needs; echo exit=$?
exit=0

$ python3 <set comparison on base>
needs minus condition: ['roadmap-sync', 'secret-material']
condition minus needs: []

$ (cd /tmp/hft-critic-ci-aggregate-red && bash scripts/tests/red_ci_aggregate.sh); echo exit=$?
нет python-тела агрегата
FAIL  SETUP: тело агрегата не извлечено из .../.github/workflows/ci.yml
VERDICT: FAIL
exit=1

$ bash scripts/tests/red_ci_aggregate.sh; echo exit=$?
ok    всё success ⇒ агрегат проходит
ok    провал secret-material ⇒ отказ с именем джоба
ok    провал roadmap-sync ⇒ отказ с именем джоба
ok    исход cancelled ⇒ отказ
ok    исход skipped ⇒ отказ
ok    пустой needs ⇒ отказ
ok    все джобы ci.yml, кроме наблюдателей, — в needs агрегата
VERDICT: PASS — сценариев: 7, расхождений: 0
exit=0

$ isolated-mutants (each: cmp original ci.yml; then probe)
ignore-secret: cmp exit=1; VERDICT: FAIL — сценариев: 7, расхождений: 1; exit=1
remove-from-needs: cmp exit=1; VERDICT: FAIL — сценариев: 7, расхождений: 1; exit=1
remove-empty-guard: cmp exit=1; VERDICT: FAIL — сценариев: 7, расхождений: 1; exit=1
different-indent: cmp exit=1; FAIL SETUP: тело агрегата не извлечено; exit=1
renamed-status-job: cmp exit=1; FAIL SETUP: тело агрегата не извлечено; exit=1
second simple heredoc: cmp exit=1; VERDICT: FAIL — сценариев: 7, расхождений: 5; exit=1
second healthy heredoc + broken real body: cmp exit=1; VERDICT: PASS — сценариев: 7, расхождений: 0; exit=0

$ execute the real (second) body of the deceptive mutant with secret-material=failure
status-check_python_heredocs= 2
body_1_contains_actual_mutant= False
body_2_contains_actual_mutant= True
   success  build-test
   success  roadmap-sync
   failure  secret-material
All checks passed (3 jobs)
actual_last_body_exit=0

$ direct 18-dependency mutation sweep of audited body
needs_count=18
passed=18 failed=0
all_success_output= All checks passed (18 jobs)

$ python3 scripts/deploy_catchup.py check-aggregate; echo exit=$?
FAIL A3: АНТИ-ПЛАЦЕБО: при всех success агрегат вернул exit=1
('json.decoder.JSONDecodeError: Expecting value: line 1 column 1 (char 0)').
VERDICT: FAIL (1 нарушени(й) CI-агрегата)
exit=1

$ bash scripts/tests/red_review_fa.sh; echo exit=$?
FAIL  W8OK реальный ci.yml чекаута несёт полную проводку — wiring красный
      ↳ status-check if/run condition missing needs.review-fa.result
VERDICT: FAIL (1 нарушений; 49/50 сценариев прошли)
exit=1

$ gh pr checks 232; echo exit=$?
All checks passed  fail
Deploy catch-up ... fail
Review FA ... fail
... 18 other PR checks pass
exit=1

$ gh run view 36318350195 --job 108618967702 --log
   success  archived-refs
   success  artifact-ids
   success  build-test
   success  context-budgets
   success  contracts
   success  delivery
   failure  deploy-catchup
   success  design-claims
   success  docs-freeze
   success  gate-meta
   success  protected-artifacts
   success  reserve-ids
   success  resource-oracles
   failure  review-fa
   success  roadmap-sync
   success  rollout-composition
   success  secret-material
   success  security
One or more checks failed: deploy-catchup, review-fa
Process completed with exit code 1.

$ git ls-remote origin refs/reserved/C-257
4cf59f8d7092b999d2da8154afaa8ea3a65c4b83 refs/reserved/C-257
```
