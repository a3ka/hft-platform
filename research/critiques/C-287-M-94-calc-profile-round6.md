<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: da8dbf60bc60b7d6e93a3abf2bbf771b9904253f
verdict: REJECT
-->

# C-287 — M-94 calc-profile, round 6 — REJECT

## Scope and object

Only the architect response to `R-245` was audited: the three commits in
`d27a58c..da8dbf6` on `origin/feat/M-94-calc-profile`.  Earlier rounds
`C-280`…`C-283` and the already-closed parts of `A-049` were not re-opened.

The new deploy RED is a real regression detector and the new `g1` is a useful
start, but this response is not dispatchable yet: §3.2 both conflicts with a
pre-existing sacred oracle and leaves its claimed two-mode boundary unproved.

## Verdict — REJECT

### B-1 — §3.2(a) contradicts the existing no-profile oracle, and g1 excludes the contradiction

Section 3.2 newly says that, for **each of the six values**, an absent **or
empty** value outside a profile retains the old default/offline behaviour; in a
profile all six keys are mandatory.  `g1` explicitly excludes absent/empty
values.  That exception is therefore not an asserted mode boundary.

More importantly, the existing sacred
`crates/gateway-serve/tests/red_heatmap_window_env.rs` asserts that both
`GATEWAY_HEATMAP_WINDOW=""` and `" "` refuse startup.  The current test passes.
The smallest implementation of the new §3.2(a) rule—treat a trimmed-empty
legacy heatmap value as the default—makes exactly that existing oracle fail.
This is not a hypothetical compatibility concern: the mutation failed on the
empty-string case, while the unmodified oracle is green.

The same audit found no old test that expects legacy `GATEWAY_BANDS` values
`<=0`, `NaN`, or `inf` to be accepted; the existing bands delivery tests use
canonical/absent inputs.  Thus the new fail-closed bands rule itself has no
such discovered corpus conflict.  The unaddressed heatmap conflict is enough
to make the six-value statement internally non-executable under the protected
artifact boundary: engine-dev may implement source changes in §10, but may not
silently rewrite the architect-owned RED/specification to choose a different
meaning for empty input.

**Required re-submission evidence:** reconcile the normative meaning of
legacy empty input with the existing heatmap oracle (rather than leaving both
claims true), and add executable coverage for the named absent/empty boundary
for every applicable one of the six values in both modes.  The evidence must
make clear which behaviour is retained, not merely omit empty values from the
comparison corpus.

### B-2 — g1 plus task 12's textual check cannot establish the claimed “no third difference”

The two named differences are correctly *named*: (a) absent/empty outside a
profile versus mandatory profile keys, and (b) profile policy—the allowed
timeframe/window/cadence triple plus the forbidden profile window `0`.  The
pre-existing cross-key rule is a third possible place for a mode split:
`GATEWAY_DEPTH_CADENCE_MS` must remain aligned with the timeframe (including
the accepted `timeframe=3000, cadence=1000` carve-out).  §3.2 says it applies
in both modes, but g1 changes one value at a time around the 1000/60000/1000
base triple.  It does not exercise that accepted pair in a profile, nor a
checkpoint no-profile entrypoint.

The 43-value corpus visits all six keys and contains plausible accepted and
refused values for each.  Its setup guard, however, checks only aggregate
`accepted >= 6 && refused >= 15`; it does not require an acceptance and a
refusal contribution from *each* key or from each mode.  A shared parser that
refused every value of one key could still satisfy that aggregate guard using
the other five keys.  This is an anti-placebo gap in the guard, not a claim
that all 43 cases are useless.

Task 12 adds `check_calls`, but its stated limit is material: it is a
whole-file `grep -c "calc_profile::parse_*(`.  A comment, dead helper, or a
profile-only call satisfies it; it does not show that the legacy/no-profile
branches consume the parsed result.  g1 executes `serve_config_from_env`, not
the checkpoint entrypoint.  Consequently g1 + this grep cannot prove §5's
single-grammar prohibition at both no-profile boundaries or prove that the
cross-key relation did not become a third mode difference.

**Required re-submission evidence:** add a behavioural proof for the
cross-key relation in both relevant modes/entrypoints and make the task-12
guard branch/result-sensitive enough that a textual or unreachable call cannot
pass.  Tighten the corpus setup assertion so every declared value grammar is
shown to have pressure on both accepted and refused sides.

## Confirmed non-blocking items

- The current `red_m94_single_grammar` fails on the five intended legacy bands
  divergences (`-0.1`, `0`, `NaN`, `inf`, `0.01,-0.02`).  Replacing the legacy
  bands parse with `gateway::calc_profile::parse_bands` makes it pass; a
  one-character shared-heatmap mutant accepting `1` then makes it fail.  It is
  a non-placebo detector for the single-value disagreement it actually tests.
- `red_m94_deploy_apply.sh` is an effective RED for `R-245` B-2: at the audited
  head exactly `a4`, `a4r`, `a4s`, `a4t`, and `a4m` fail for the missing
  post-`up@PREV` watchdog.  The isolated `full_rollback_prev` sample requested
  by the task makes all 13 scenarios pass.
- §5's prohibition on a second grammar is the correct policy.  Tasks 12 and
  13 map to paths allowed to engine-dev by §10 (including `gateway-serve`,
  `gateway-checkpoint`, `gateway/src/calc_profile.rs`, and deploy apply); the
  architect remains responsible for the sacred tests and milestone changes.
- The `VB-I-2` carrier in `docs/fa/viz-backend.md` now names the active
  calc-profile/config owner, so R-245's documentation carrier concern is
  addressed.

## Done Block

All mutations below were made only in disposable detached worktrees and were
reverted before their worktrees were removed.  The audited worktree remained
source-clean before this verdict was created.

```text
$ git rev-parse origin/feat/M-94-calc-profile
da8dbf60bc60b7d6e93a3abf2bbf771b9904253f
exit=0

$ git log --oneline d27a58c..origin/feat/M-94-calc-profile
da8dbf6 docs(M-94): R-245 — §3.2 one grammar per value, tasks 12-13, FA carrier [architect]
bd207c9 test(M-94): R-245 B-1 — one grammar per profile value, both modes [architect]
2dfb6d9 test(M-94): R-245 B-2 — health rollback must reinstall watchdog from PREV [architect]
exit=0

$ TMPDIR=/tmp cargo test -p gateway-serve --test red_m94_single_grammar
running 1 test
test g1_one_grammar_per_value_g2_profile_policy_bounded_window ... FAILED
M-94 / R-245 B-1 / спека §3.2: ... (5 расхождений):
  GATEWAY_BANDS="-0.1"       без профиля: Accepted; в профиле: Refused
  GATEWAY_BANDS="0"          без профиля: Accepted; в профиле: Refused
  GATEWAY_BANDS="NaN"        без профиля: Accepted; в профиле: Refused
  GATEWAY_BANDS="inf"        без профиля: Accepted; в профиле: Refused
  GATEWAY_BANDS="0.01,-0.02" без профиля: Accepted; в профиле: Refused
test result: FAILED. 0 passed; 1 failed
exit=101

$ # disposable reference: legacy bands calls gateway::calc_profile::parse_bands
$ TMPDIR=/tmp cargo test -p gateway-serve --test red_m94_single_grammar
running 1 test
test g1_one_grammar_per_value_g2_profile_policy_bounded_window ... ok
test result: ok. 1 passed; 0 failed
exit=0

$ # reference plus shared parse_heatmap_window_frac: >= 1.0 -> > 1.0
$ TMPDIR=/tmp cargo test -p gateway-serve --test red_m94_single_grammar
running 1 test
test g1_one_grammar_per_value_g2_profile_policy_bounded_window ... FAILED
M-94 / R-245 B-1 / спека §3.2: ... (1 расхождений):
  GATEWAY_HEATMAP_WINDOW="1"
    без профиля: Refused("... вне интервала (0, 1) ...")
    в профиле:   Accepted
test result: FAILED. 0 passed; 1 failed
exit=101

$ TMPDIR=/tmp cargo test -p gateway-serve --test red_heatmap_window_env
running 5 tests
test absent_heatmap_window_starts ... ok
test valid_heatmap_window_starts ... ok
test malformed_heatmap_window_is_rejected ... ok
test heatmap_window_is_declared_in_compose ... ok
test out_of_range_heatmap_window_is_rejected ... ok
test result: ok. 5 passed; 0 failed
exit=0

$ # disposable legacy heatmap sample: trimmed-empty -> DEFAULT_HEATMAP_WINDOW_FRAC
$ TMPDIR=/tmp cargo test -p gateway-serve --test red_heatmap_window_env
test malformed_heatmap_window_is_rejected ... FAILED
GATEWAY_HEATMAP_WINDOW="" принято молча. Значит переменная ... подменяется дефолтом ...
test result: FAILED. 4 passed; 1 failed
exit=101

$ for f in crates/gateway-serve/src/lib.rs crates/gateway/src/bin/gateway-checkpoint.rs; do ...; done
crates/gateway-serve/src/lib.rs parse_bands 0
crates/gateway-serve/src/lib.rs parse_timeframe_ms 0
crates/gateway-serve/src/lib.rs parse_window_ms 0
crates/gateway-serve/src/lib.rs parse_depth_cadence_ms 0
crates/gateway-serve/src/lib.rs parse_heatmap_window_frac 0
crates/gateway-serve/src/lib.rs parse_vp_bin_width_e8 0
crates/gateway/src/bin/gateway-checkpoint.rs parse_bands 0
crates/gateway/src/bin/gateway-checkpoint.rs parse_timeframe_ms 0
crates/gateway/src/bin/gateway-checkpoint.rs parse_window_ms 0
crates/gateway/src/bin/gateway-checkpoint.rs parse_depth_cadence_ms 0
exit=0

$ bash -n scripts/verify_M-94.sh
exit=0

$ TMPDIR=/tmp bash scripts/tests/red_m94_deploy_apply.sh
FAIL  a4  ... watchdog@PREV строка нет, up@PREV строка 7
FAIL  a4r ... watchdog@PREV строка нет, up@PREV строка 7
FAIL  a4s ... watchdog@PREV строка нет, up@PREV строка 7
FAIL  a4t ... watchdog@PREV строка нет, up@PREV строка 7
FAIL  a4m ... watchdog@PREV строка нет, up@PREV строка 7
сценариев: 13 (pass=8 fail=5)
VERDICT: FAIL
exit=1

$ # disposable full_rollback_prev sample: ${DEPLOY_WATCHDOG_INSTALL} || log "WARN: watchdog на PREV не установлен"
$ TMPDIR=/tmp M94_APPLY_UNDER_TEST=/tmp/hft-critic-m94r6-checksample/deploy/bin/deploy-apply.sh bash scripts/tests/red_m94_deploy_apply.sh
pass  a0 ...
pass  a1 ...
pass  a2 ...
pass  a3 ...
pass  a4 ...
pass  a4r ...
pass  a4s ...
pass  a4t ...
pass  a4m ...
pass  a2s ...
pass  a6 ...
pass  a5 ...
pass  a7 ...
сценариев: 13 (pass=13 fail=0)
VERDICT: PASS
exit=0

$ git -C /tmp/hft-critic-m94r6-checksample status --porcelain; git -C /tmp/hft-critic-m94r6-blankcheck status --porcelain
<both empty before git worktree remove>
exit=0
```

CI-form gate results after the first verdict commit (`dec2186`):

```text
$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=dec2186 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=dec2186 bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (045fef9..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=dec2186 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=dec2186 bash scripts/check_gate_meta.sh
── GATE-META: диапазон 045fef9a..HEAD, origin=a3ka/hft-platform
VERDICT: PASS — вердиктов проверено: 6, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=dec2186 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=dec2186 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 045fef9..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```

The same three CI-form barriers are repeated on the amended audit-trail commit
before push.
