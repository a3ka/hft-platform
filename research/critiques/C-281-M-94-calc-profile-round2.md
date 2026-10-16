<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: 77a09ad252d6bd0dcb42c0b219b652ac1240feb5
verdict: REJECT
-->

# C-281 — M-94 calc-profile, round 2 — REJECT

## Verdict

**REJECT.** The complete architect artifact set is committed at the audited head, and
the three C-280 blockers have been addressed in substance.  However, the replacement
I-6 deploy oracle does not distinguish a deployment that checks only one of the two
required services from one that checks both.  It also does not observe the existing
failure logs or image prune that the new deployment contract is meant to preserve.
This is a fail-open RED oracle for a production deployment/rollback path, so dispatch
to engine-dev is blocked.

## Audited object and artifact completeness

- Subject branch: `origin/feat/M-94-calc-profile`.
- Base: `045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0`.
- Head: `77a09ad252d6bd0dcb42c0b219b652ac1240feb5`.  It matched the handoff SHA and
  the repeated pre-verdict `fetch`.
- T1 / trait surface: the range has no `contracts/**`, `crates/contracts/**`, or
  `crates/*/src/**` change.  `Snapshot.calc_profile` remains explicitly deferred, so
  no contract-RFC or trait signature is missing from this plan-time set.
- Present architect artifacts: milestone; profile v1; three new RED suites; the five
  adapted corpus suites; `verify_M-94.sh`; both deploy RED probes; the calc-profile
  barrier and its RED probe; CI job and aggregate wiring; and the scope-guard entry.
- Applicable live invariants: `VB-I-2` (one definition for writer and checkpoint
  reader) and `VB-I-11` (provenance of the calculated history), as named by the
  milestone.  This range does not alter a `crates/<name>/src/**` module, so the
  review-FA mechanical check is correctly in its documented `SKIP` case.

## C-280 closure check

### R1 — closed

`scripts/check_calc_profile.sh`, `scripts/tests/red_calc_profile.sh`, the
`calc-profile` CI job, its `status-check` membership/condition, and removal of the
obsolete M-70 compose-default watcher are all committed.  The barrier probe passes its
positive control and seven mutants; the aggregate and CI-map probes also pass.

The scope-guard change is authorized by the commit-local
`FOUNDER-APPROVED: П-032 п.3` trailer.  Its only removed prose is the historical
recorder milestone enumeration; the owner/path norm is unchanged.  This also satisfies
the required independent recheck of the locked process-layer change: code claims were
executed, authority is present, and the barrier is connected to CI/aggregate.

### R2 — closed for the direct target-alias defect

`p8b` resolves the retention consumer path from the real retention cron script, runs
the actual checkpoint binary on the fixture volumes, requires the next cursor to move
forward, and compares canonicalized targets before asserting that the active retention
cursor is unchanged.  The baseline correctly remains RED because task 5 is not yet
implemented.

**Note:** `run_cron_calls` uses a success shim to record argv
(`red_m94_calc_profile_warmer.rs:481-522`), then `p8b` separately executes extracted
checkpoint calls (`:1364-1404`).  Thus it does not exercise any post-run side effect
added to the cron wrapper itself, and its fixture does not seed a symlink alias despite
the comment at `:1311-1317`.  The engine implementation must keep the wrapper limited
to the checked runner calls, or architect should extend p8b to execute its full
non-print form against a seeded symlink fixture.

### R3 — original rollback gap closed, but replacement oracle is incomplete

Replacing static `g7` with an executable `deploy-apply` probe addresses the original
C-280 issue: its a1/a4 worlds require reset to PREV, PREV rebuild, PREV cron, and the
correct last `up`.  The finding below concerns new unobserved obligations of the same
I-6 contract.

## Blocking finding

### B1 — a4 cannot prove that *both* required services become healthy

`milestones/M-94-calc-profile.md:187` requires `docker compose up -d recorder
gateway-serve` **and** a health wait for both.  The `a4` docker shim instead returns
one shared `${health}` value for every `inspect` invocation
(`scripts/tests/red_m94_deploy_apply.sh:61-72`).  The assertion at `:148-160` checks
only that an all-unhealthy world rolls back; it never supplies `recorder=unhealthy,
gateway-serve=healthy` or the inverse, and it does not log/assert which service was
inspected.

I constructed an otherwise conforming `deploy-apply.sh` mutant which:

- calls `inspect` only for `hft-gateway-serve` (never `hft-recorder`);
- performs no `docker logs` calls on the unhealthy path; and
- performs no `docker image prune -f` on success.

Against a fixture `deploy.yml` with the required reset → `deploy-apply.sh "$PREV"`
plumbing, the current probe reported all six worlds green.  The oracle therefore
accepts a deploy that can leave an unhealthy recorder running, omits diagnostic logs
on failure, and silently drops the current image-prune behavior.  It likewise has no
negative world for a watchdog-install failure and a7 does not assert the existing
`command_timeout: 15m` contract.

This violates I-6 and the testing rule that an oracle must exercise the production
form and measure its own invariant, not a weaker proxy.  A server can remain live while
the recorder is unhealthy; that is precisely the `OPS-I-8` failure class.

**Required before re-gate:** extend `red_m94_deploy_apply.sh` with a stateful docker
stub that reports health by service.  It must run and fail independently for
`recorder=unhealthy/gateway-serve=healthy` and the converse, recording both inspect
targets.  It must also assert both failure-path log calls, successful prune, watchdog
failure handling, and the `deploy.yml` `command_timeout: 15m` requirement.  Add each
new assertion's mutation control.  Do not weaken the two-service requirement in the
milestone.

## Additional gate observation

The architect handoff said `verify_M-94.sh` fails only on five engine-dev items.  A
complete run at the audited head instead ends `VERDICT: FAIL (провалов: 6)`: the six
includes `scripts/tests/red_runbook_markers.sh`.  The same probe is already red at
`045fef9` (`origin/main`), so this is a pre-existing CI baseline defect, not an M-94
regression.  It nevertheless makes the handoff's claimed failure set false and must be
named rather than hidden; the verifier is CI-parity, not a five-task-only command.

## Re-gate condition

Architect must commit the strengthened deploy-apply RED oracle and its mutation
evidence, then request a new critic round against the remote tip.  This is a new
reason, not a repeated C-280 R1/R2/R3 reason; arbitration is not triggered by this
verdict alone.

## Done Block

Raw command outcomes from detached audit worktree `/tmp/hft-critic2-m94`:

```text
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile
77a09ad252d6bd0dcb42c0b219b652ac1240feb5
exit=0

$ bash scripts/tests/red_calc_profile.sh
сценариев: 30; провалов: 0; каталогов песочницы до уборки: 145 (убираются trap EXIT)
VERDICT: PASS
exit=0

$ cargo test -p gateway --test red_m94_calc_profile_warmer -- p8
test p8b_next_run_cannot_move_the_retention_cursor ... FAILED
test p8_cron_prewarms_next_profile_with_separate_coverage ... FAILED
test result: FAILED. 0 passed; 2 failed; 10 filtered out
exit=101

$ bash scripts/tests/red_m94_deploy_gate.sh
FAIL  g0 SETUP: гейта .../deploy/bin/calc-profile-gate.sh нет
VERDICT: FAIL (pass=0 fail=1; остальные миры не исполнялись — судить нечего)
exit=1

$ bash scripts/tests/red_m94_deploy_apply.sh
FAIL  a0 SETUP: .../deploy/bin/deploy-apply.sh нет
VERDICT: FAIL (pass=0 fail=1; миры не исполнялись — судить нечего)
exit=1

$ bash scripts/tests/red_m94_deploy_apply.sh  # stateful-health mutant: only gateway-serve inspected
pass  a0 скрипт деплоя существует
pass  a1 отказ гейта ⇒ PREV: чекаут, образ, cron, профиль; up не было
pass  a2 гейт пропустил ⇒ TARGET, build→gate→up→watchdog, cron из TARGET
pass  a3 при отказе гейта cron-файл хоста не перезаписывался
pass  a4 нездоровая выдача ⇒ откат к PREV, cron из PREV, up на PREV
pass  a7 deploy.yml: reset на TARGET → deploy-apply.sh "$PREV"; инлайнового docker compose up нет
сценариев: 6 (pass=6 fail=0)
VERDICT: PASS
exit=0

$ bash scripts/verify_M-94.sh
PASS  task7: config/calc-profile/active.env — восемь ключей
PASS  task9: проба барьера red_calc_profile.sh
PASS  task9: барьер проведён в ci.yml
PASS  task10: сторож verify_M-70 task #7 снят
FAIL  ci-parity: cargo test --all (exit=101)       # expected unimplemented M-94 RED suites
FAIL  ci-parity: bash scripts/tests/red_runbook_markers.sh (exit=1)
VERDICT: FAIL (провалов: 6)
exit=1

$ (cd /tmp/hft-codex-critic-1791220285 && bash scripts/tests/red_runbook_markers.sh)
FAIL  S17b SETUP НЕ СОСТОЯЛСЯ: каталог внутри git
FAIL  S17b корень — не рабочее дерево git — отказ есть, но не по причине «не рабочее дерево git»
VERDICT: FAIL
exit=1

$ bash scripts/check_context_budgets.sh .claude/rules/scope-guard.md
OK    .claude/rules/scope-guard.md  7590 B / 7600 B (запас 10 B)
VERDICT: PASS — 7 файлов, 112969 B из 114900 B бюджета (запас 1931 B)
exit=0

$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile && git rev-parse HEAD
77a09ad252d6bd0dcb42c0b219b652ac1240feb5
77a09ad252d6bd0dcb42c0b219b652ac1240feb5
exit=0
```
