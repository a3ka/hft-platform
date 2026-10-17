<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: 47975eec9515b84b51c17243d836227e74bd692f
verdict: REJECT
-->

# C-280 — M-94 calc-profile — REJECT

## Verdict

**REJECT.** Ветка `origin/feat/M-94-calc-profile` не готова к dispatch engine-dev. Набор
содержит v1-профиль, RED-оракулы, `verify_M-94.sh`, milestone и адаптацию корпуса, однако
две обязательные architect-задачи отсутствуют в том же commit-chain, а два safety-оракула
не измеряют заявленные инварианты. Это не может быть исправлено «до PR»: по critic gate
полный architect-набор должен быть закоммичен до передачи реализации.

## Audited object

- Subject branch: `origin/feat/M-94-calc-profile`.
- Base: `045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0`.
- Audited head: `47975eec9515b84b51c17243d836227e74bd692f` (повторный `fetch` непосредственно
  перед записью подтвердил, что рабочая вершина с ним совпадает).
- T-contracts: в диапазоне нет изменения `contracts/**`; M-94 не вводит новый wire-contract,
  а поле `Snapshot` явно отложено в §3.6(в). Trait/signature surface в диапазоне также не
  меняется: реализация оставлена будущим задачам engine-dev.
- Present committed artifacts: `milestones/M-94-calc-profile.md`,
  `config/calc-profile/active.env`, три Rust RED-набора (`p*`, `s*`, `o1`),
  `scripts/tests/red_m94_deploy_gate.sh`, `scripts/verify_M-94.sh`,
  `scripts/tests/red_verify_M-94_ci_map.sh`, а также пять адаптированных корпусных оракулов.
- FA considered: `docs/fa/viz-backend.md:285` (`VB-I-2`, live == replay) and `:294`
  (`VB-I-11`, provenance history) apply to the profile/provenance design; no new FA
  statement is promised by this branch.

## Blocking findings

### R1 — architect-owned barrier and obsolete watcher are absent

`milestones/M-94-calc-profile.md:184-192` makes the per-commit calc-profile barrier a
mechanism for П-032 п.3: `scripts/check_calc_profile.sh`, its RED, CI aggregate wiring,
and a sacred scope-guard entry. The task table assigns all of that to architect
(`:418`), assigns removal of the obsolete M-70 compose-default watcher to architect
(`:419`), and requires tasks 9–10 on this branch before PR (`:422-423`). Neither
`scripts/check_calc_profile.sh` nor `scripts/tests/red_calc_profile.sh` is in the audited
range, CI has no wiring, the scope guard is unchanged, and the M-70 watcher remains.

This is executable, not a prose-only discrepancy: `scripts/verify_M-94.sh:52-66` marks
both omissions as failures. The full verifier ended `VERDICT: FAIL (провалов: 7)`, including
task9 and task10. A profile file is already committed, so no exception for a future
implementation exists.

**Required for re-submission:** commit the barrier, its adversarial RED, aggregate CI
wiring, the `FOUNDER-APPROVED: П-032 п.3` sacred entry, and removal/adaptation of the
obsolete M-70 watcher; then show the verifier at the new branch head.

### R2 — p8 checks differing argv text, not separation of the retention target

The safety property is physical: §3.5 says the next-profile run must not write the
retention path because a farther `covered_through_seq` may delete the active-profile tail
(`milestones/M-94-calc-profile.md:126-142`). Yet p8 replaces the runner with a shim that
only appends argv and always exits success
(`crates/gateway/tests/red_m94_calc_profile_warmer.rs:481-522`). Its decisive assertion is
only `next_cov != active_cov` (`:1095-1100`). It neither executes checkpoint/coverage nor
resolves the paths and observes the cursor used by retention.

Consequently an implementation whose two syntactically different paths alias the same
target (for example `dir/../dir/covered_through_seq`, or a symlink) passes p8 while the
next run advances the real retention cursor. That is exactly the data-loss path §3.5
forbids. The oracle therefore does not measure I-7 at the severity specified by
`testing.md`.

**Required for re-submission:** make the RED demonstrate, with the real coverage/retention
effect or an equivalent filesystem-level oracle, that active and next cannot address the
same resolved target and that only the active cursor can authorize deletion of the active
tail.

### R3 — g7 permits a pre-`up` failure which leaves the new checkout/image in place

§3.7 requires a calc-profile gate failure to occur before `up` **and** to restore checkout
and image tag to `PREV` with a rebuild; the stated reason is prevention of a new cron/profile
with an old serving process (`milestones/M-94-calc-profile.md:161-179`). But g7 is a static
line-order test (`scripts/tests/red_m94_deploy_gate.sh:131-145`): it accepts any `exit 1`
between the gate line and `up`. It does not execute the negative branch or assert
`git reset --hard "$PREV"`, the old-image rebuild, or absence of the new cron/profile after
failure.

In particular, placing a direct failing gate under `set -e` before `up` satisfies g7's
text search while bypassing the existing `docker compose up` failure handler, leaving the
new checkout and image prepared. That is the mixed-state failure §3.7 explicitly calls
out, so the RED is not a safety oracle for I-6.

**Required for re-submission:** exercise a failing calc-profile-gate deployment path and
assert the observable rollback to `PREV` (including rebuild/tag and no surviving new
cron/profile state), in addition to the ordering check.

## Executed checks

The three mandatory RED invocations are red on the implementation-free baseline for their
named missing behavior; that fact does not cure R2/R3:

- `cargo test -p gateway --test red_m94_calc_profile_warmer` — exit `101`:
  1 passed, 10 failed (`c1`, `p1`…`p9`), including missing profile delivery and absent
  profile validation.
- `cargo test -p gateway-serve --test red_m94_calc_profile_serve` — exit `101`:
  0 passed, 3 failed (`s1`…`s3`); profile environment keys are currently accepted and a
  broken profile does not refuse startup.
- `bash scripts/tests/red_m94_deploy_gate.sh` — exit `1`: `g0` reports that
  `deploy/bin/calc-profile-gate.sh` is absent, so later g-worlds cannot be judged yet.
- `cargo test -p ops --test red_m94_heartbeat_profile_compat` — exit `0`: `o1` passed.
- `bash scripts/verify_M-94.sh` — exit `1`, final raw verdict
  `VERDICT: FAIL (провалов: 7)`; task9/task10 fail as described above, while the
  CI-parity map reports 59/59 steps.

The diff of the five existing corpus oracles was also examined at `1880ca2`; it adapts
the measured production configuration to the profile and does not remove their prior
assertion purpose. It does not compensate for the three blockers above.

## Re-gate condition

Architect must commit all R1 artifacts and strengthen R2/R3 before asking for a new critic
round. The next critic audits the new remote tip, not this verdict's head. Repeating one of
these reasons after an attempted repair is a repeated REJECT reason and follows the
arbitration path in `gates.md`.

## Done Block

Raw command outcomes (UTC audit worktree `/tmp/hft-critic-m94`):

```text
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile
47975eec9515b84b51c17243d836227e74bd692f
exit=0

$ bash scripts/next_artifact_id.sh C
C-279
exit=0

$ bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-279 … занят; следующий кандидат — C-280
reserve: попытка 2/8 — C-280 …
C-280
reserve: резерв C-280 взят
exit=0

$ git ls-remote origin refs/reserved/C-280
532ad73eb91f9b684a6933b045a2a17475fd6671 refs/reserved/C-280
exit=0

$ cargo test -p gateway --test red_m94_calc_profile_warmer
test result: FAILED. 1 passed; 10 failed
exit=101

$ cargo test -p gateway-serve --test red_m94_calc_profile_serve
test result: FAILED. 0 passed; 3 failed
exit=101

$ bash scripts/tests/red_m94_deploy_gate.sh
FAIL  g0 SETUP: гейта .../deploy/bin/calc-profile-gate.sh нет
VERDICT: FAIL (pass=0 fail=1; остальные миры не исполнялись — судить нечего)
exit=1

$ cargo test -p ops --test red_m94_heartbeat_profile_compat
test result: ok. 1 passed; 0 failed
exit=0

$ bash scripts/verify_M-94.sh
VERDICT: FAIL (провалов: 7)
exit=1

$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile && git rev-parse HEAD
47975eec9515b84b51c17243d836227e74bd692f
47975eec9515b84b51c17243d836227e74bd692f
exit=0
```
