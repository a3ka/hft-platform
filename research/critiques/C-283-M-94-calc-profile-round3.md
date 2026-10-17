<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: 62fa7e6e147bd41125fc492e96acaef113558e4b
verdict: NOTE
-->

# C-283 — M-94 calc-profile, round 3 — NOTE

## Verdict

**NOTE — dispatch to `engine-dev` is unblocked.** The six corrections ordered by
`A-049` are present in the committed subject branch and their oracles reject the
specified broken implementations. This verdict does not make the RED suites green:
they remain red, as expected, until tasks 1–6 are implemented.

## Audited object and artifact completeness

- Subject: `origin/feat/M-94-calc-profile`; handoff SHA and fetched tip both
  `62fa7e6e147bd41125fc492e96acaef113558e4b`.
- Base: `045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0`.
- T1 / trait surface: no `contracts/**`, `crates/contracts/**`, or
  `crates/*/src/**` change is committed in the architect artifact range; M-94 keeps
  the `Snapshot` wire-field explicitly deferred. No contract-RFC or trait signature
  is therefore missing at this plan-time gate.
- Present before dispatch: milestone, profile v1, three M-94 Rust RED suites, five
  adapted existing RED suites, deploy-gate and deploy-apply probes, calc-profile
  barrier plus probe and CI wiring, and `scripts/verify_M-94.sh`.
- Applicable live invariants: `VB-I-2` (`docs/fa/viz-backend.md:285`, live/replay use
  the same definition) and `VB-I-11` (`:294`, provenance must not misrepresent
  available history). The range changes tests, not `crates/<name>/src/**`; the
  `review-fa` mechanical check is consequently in its documented `SKIP` case.

## A-049 closure

1. **Р-1 — rollback rebuilds PREV.** `a4`, `a4r`, `a4s`, `a4t`, and `a4m` require a
   `build@PREV` before the final `up@PREV` (`scripts/tests/red_m94_deploy_apply.sh:185-219`).
   My independent reference passes all 13 worlds; M1 (health rollback without the
   rebuild) fails all five relevant worlds.
2. **Р-2 — health is a state sequence.** The docker stub records an `inspect` target
   per service and supplies `starting`, `missing`, and `starting:K,healthy` sequences
   (`:68-101`, `:216-232`). M2, which accepts every state except `unhealthy`, fails
   `a4t` and `a2s`.
3. **Р-3 — cron is validated before installation.** `a6` makes the validator reject a
   TARGET marker and requires no `up`, an untouched host cron, and checkout/image at
   PREV (`:234-252`). M3, which skips validation, fails `a6`.
4. **Р-4 / Р-5 — next is composable and writable.** `p8b` compares normalized active
   and next checkpoint directories, independently derives the expected next snapshot,
   asserts both next write paths are on RW volumes, and has a read-only setup guard
   (`crates/gateway/tests/red_m94_calc_profile_warmer.rs:1506-1538`, `:1213-1239`).
   The candidate passes `p8b`; Y1 (`--coverage-out` on `/journal:ro`) fails with a
   permission error and Y2 (`--ckpt-dir /ckpt/next`) fails the composition assertion.
5. **Р-6 — one parser domain is executable without a scope exception.** Milestone
   §3.2/§5/§10 places the four shared parsers in
   `crates/gateway/src/calc_profile.rs` and requires `gateway-serve` to call that one
   instance. The allowed engine-dev paths cover both callers and the new module; the
   rule neither asks gateway to depend on gateway-serve nor permits a duplicate parser.

## Note

`red_runbook_markers.sh` remains host-sensitive when `TMPDIR` is inside a git worktree.
`A-049` reproduced that behaviour on `origin/main` as well as this branch, so it is not
an M-94 regression or a plan-time blocker. It remains a reviewer-owned harness debt.

## Dispatch condition

Architect records this NOTE mechanically in the milestone appendix, then founder may
dispatch `engine-dev` only for M-94 tasks 1–6 and its listed allowed paths. The dev Done
Block must make all named RED suites green and include the milestone’s required mutation
controls; `task11` remains reviewer/architect post-merge work.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile
62fa7e6e147bd41125fc492e96acaef113558e4b
exit=0

$ git diff --name-status 045fef9..HEAD
M crates/gateway/tests/red_m94_calc_profile_warmer.rs
M docs/ROADMAP.md
M milestones/M-94-calc-profile.md
M scripts/tests/red_m94_deploy_apply.sh
M scripts/verify_M-94.sh
exit=0

$ bash scripts/tests/red_calc_profile.sh
сценариев: 30; провалов: 0; каталогов песочницы до уборки: 145 (убираются trap EXIT)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_verify_M-94_ci_map.sh
VERDICT: PASS — 10 сценариев
exit=0

$ bash scripts/tests/red_m94_deploy_gate.sh
FAIL  g0 SETUP: .../deploy/bin/calc-profile-gate.sh нет
VERDICT: FAIL (pass=0 fail=1; остальные миры не исполнялись — судить нечего)
exit=1  # expected RED: task 6 is not implemented

$ bash scripts/tests/red_m94_deploy_apply.sh
FAIL  a0 SETUP: .../deploy/bin/deploy-apply.sh нет
VERDICT: FAIL (pass=0 fail=1; миры не исполнялись — судить нечего)
exit=1  # expected RED: task 6 is not implemented

$ cargo test -p gateway --test red_m94_calc_profile_warmer -- p8b
test p8b_next_run_cannot_move_the_retention_cursor ... FAILED
exit=101  # expected RED: next-profile cron work is not implemented

$ M94_APPLY_UNDER_TEST=/tmp/m94-critic.Khv6cn/ref-apply.sh \
  M94_DEPLOY_YML_UNDER_TEST=/tmp/m94-critic.Khv6cn/deploy.fixture.yml \
  bash scripts/tests/red_m94_deploy_apply.sh
сценариев: 13 (pass=13 fail=0)
VERDICT: PASS
exit=0

$ M94_APPLY_UNDER_TEST=/tmp/m94-critic.Khv6cn/m1-no-rebuild.sh ... red_m94_deploy_apply.sh
VERDICT: FAIL  # a4/a4r/a4s/a4t/a4m: build@PREV absent before up@PREV
exit=1
$ M94_APPLY_UNDER_TEST=/tmp/m94-critic.Khv6cn/m2-starting-healthy.sh ... red_m94_deploy_apply.sh
VERDICT: FAIL  # a4t and a2s
exit=1
$ M94_APPLY_UNDER_TEST=/tmp/m94-critic.Khv6cn/m3-no-cron-validate.sh ... red_m94_deploy_apply.sh
VERDICT: FAIL  # a6
exit=1

$ (cd /tmp/hft-m94-corpus && cargo test -p gateway --test red_m94_calc_profile_warmer -- p7 p8b)
test p7_print_ckpt_name_without_journal ... ok
test p8b_next_run_cannot_move_the_retention_cursor ... ok
exit=0
$ Y1: next coverage on /journal:ro → p8b
FAILED: прогон next упал ... Permission denied
exit=101
$ Y2: next --ckpt-dir /ckpt/next → p8b
FAILED: next пишет слепки в /ckpt/next, а active и гейт — в /ckpt
exit=101

$ EVENT_NAME=pull_request PR_BASE_SHA=045fef9... bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=045fef9... bash scripts/check_docs_freeze.sh
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=045fef9... bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 3, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=045fef9... bash scripts/check_calc_profile.sh
VERDICT: PASS (коммитов с правкой профиля: 1)
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=045fef9... bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 045fef9..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ bash scripts/next_artifact_id.sh C
C-282
exit=0
$ bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-282 ... занят; следующий кандидат — C-283
C-283
reserve: резерв C-283 взят
exit=0
```
