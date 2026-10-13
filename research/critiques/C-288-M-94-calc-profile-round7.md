<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: 03a9dbdcc34bcb6b7183cd3ca03720f84a116dba
verdict: ESCALATE
-->

# C-288 — M-94 calc-profile, round 7 — ESCALATE

## Scope and audited object

This is the narrowed round-7 audit of the three committed architect commits
`8a3402b..03a9dbd` on `origin/feat/M-94-calc-profile`, solely for C-287 B-1
and B-2.  The branch tip was fetched and resolved by command to
`03a9dbdcc34bcb6b7183cd3ca03720f84a116dba`; the handoff reference `f448ac3`
is its ancestor.  No implementation commit was included.

The complete architect artifact set is present: the T2 `ProfileError` and
declared six-parser contract in milestone §3.2, the two executable RED files,
the mutation probe, `scripts/verify_M-94.sh`, and the milestone.  No
`contracts/**` T1 change is in this range; no contract-RFC is required.

## Technical result — C-287 B-1/B-2 are adequately specified

There is no remaining technical REJECT for the narrowed C-287 scope.

- **B-1:** `g3` executes every row of §3.2(a) for the six serve values in
  both modes; `w3` does the four values that the warmer actually reads, in its
  own existing env semantics.  The heatmap empty/whitespace-to-default mutant
  fails `g3`, preserving `red_heatmap_window_env`'s fail-closed behavior.
- **B-2:** `g1` and `w1` exercise accepted and refused inputs per value and
  per applicable mode/form, including accepted and refused
  timeframe×cadence pairs.  The warmer test runs the production binary via
  environment, flags, and profile.  `red_m94_shared_grammar_probe.sh` replaces
  the former text count with a result-path mutation: an injected sentinel must
  reach `g4` and `w4`; a comment, a dead branch, or a profile-only call cannot
  satisfy it.
- **Anti-placebo confirmation:** in a disposable copy, replacing only the
  no-profile bands parsing in `serve_config_from_env` and both checkpoint
  inputs with `gateway::calc_profile::parse_bands` changes both new RED suites
  to PASS.  A separate mutation that defaults trimmed-empty heatmap input makes
  `g3` fail for both `""` and `" "`.

`VB-I-2` is the live invariant carrier for the touched gateway/gateway-serve
surface: live and replay must use one calculation definition.  The above
result-path proof directly protects that property; the module has no separate
FA, as milestone §14 records.

## Verdict — ESCALATE

`gates.md` §0(1) and the critic profile require an arbiter after **two
consecutive REJECTs on the same cause**, before a third critic round.  R-245
and C-287 were consecutive REJECTs on B-1 (the duplicate/divergent grammar);
C-287 additionally recorded the condition in milestone §16.  The literal
trigger contains no exception for architect acceptance or for absence of a
substantive disagreement.  §16's conclusion that no arbitration is needed
therefore conflicts with the binding routing rule.

This is a process-routing question, not a founder boundary-C decision and not
a further request for implementation.  An independent fresh-context arbiter
must decide whether the rule permits this round-7 critic route despite its
literal trigger.  The arbiter may use the technical audit above; it must not
re-open C-287 B-1/B-2 without new evidence.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile
03a9dbdcc34bcb6b7183cd3ca03720f84a116dba
exit=0

$ git log --oneline f448ac3..03a9dbd
03a9dbd docs(M-94): C-287 — §3.2 absence table per binary, signature, cross-key; round 6 [architect]
9e41ef5 test(M-94): C-287 B-2 — shared-grammar probe by mutation replaces text count [architect]
8a3402b test(M-94): C-287 — absence table, cross-key pairs, warmer entrypoint, per-key pressure [architect]
exit=0

$ git diff --name-status f448ac3..03a9dbd
M crates/gateway-serve/tests/red_m94_single_grammar.rs
A crates/gateway/tests/red_m94_single_grammar_warmer.rs
M milestones/M-94-calc-profile.md
A scripts/tests/red_m94_shared_grammar_probe.sh
M scripts/verify_M-94.sh
exit=0

$ cargo test -p gateway-serve --test red_m94_single_grammar
test g1_agreement_g2_policy_g3_absence ... FAILED
  GATEWAY_BANDS="-0.1" ... без профиля Accepted; в профиле Refused
  GATEWAY_BANDS="0" ... без профиля Accepted; в профиле Refused
  GATEWAY_BANDS="NaN" ... без профиля Accepted; в профиле Refused
  GATEWAY_BANDS="inf" ... без профиля Accepted; в профиле Refused
  GATEWAY_BANDS="0.01,-0.02" ... без профиля Accepted; в профиле Refused
exit=101

$ cargo test -p gateway --test red_m94_single_grammar_warmer
test w1_agreement_w2_policy_w3_absence ... FAILED
  GATEWAY_BANDS="-0.1" ... окружение/флаги Accepted; профиль Refused
  GATEWAY_BANDS="0" ... окружение/флаги Accepted; профиль Refused
  GATEWAY_BANDS="inf" ... окружение/флаги Accepted; профиль Refused
  GATEWAY_BANDS="0.01,-0.02" ... окружение/флаги Accepted; профиль Refused
exit=101

$ M94_PROBE_FUNCS="parse_bands:GATEWAY_BANDS:0.777 parse_heatmap_window_frac:GATEWAY_HEATMAP_WINDOW:0.0077" bash scripts/tests/red_m94_shared_grammar_probe.sh
PASS  setup: сторож вживлён в 2 из 2 общих функций
FAIL  g4: выдача — сторожевой отказ общей функции доходит до результата без профиля и в профиле (exit=101)
FAIL  w4: прогреватель — сторожевой отказ доходит в окружении, флагах и профиле (exit=101)
сценариев: 3, провалов: 2
VERDICT: FAIL
exit=1

$ # disposable bands-only shared-parser sample
$ cargo test -p gateway-serve --test red_m94_single_grammar
test result: ok. 1 passed; 0 failed; 1 ignored
exit=0
$ cargo test -p gateway --test red_m94_single_grammar_warmer
test result: ok. 1 passed; 0 failed; 1 ignored
exit=0

$ # sample mutant: trimmed-empty GATEWAY_HEATMAP_WINDOW -> default
$ cargo test -p gateway-serve --test red_m94_single_grammar
M-94 / C-287 B-1 / спека §3.2 (а): ... Some(""): ждали отказ, получили Accepted
M-94 / C-287 B-1 / спека §3.2 (а): ... Some(" "): ждали отказ, получили Accepted
exit=101

$ bash -n scripts/verify_M-94.sh scripts/tests/red_m94_shared_grammar_probe.sh
exit=0
$ git diff --check f448ac3..03a9dbd
exit=0

$ bash scripts/next_artifact_id.sh C
C-288
exit=0

$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=65f7810 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=65f7810 bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (045fef9..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=65f7810 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=65f7810 bash scripts/check_gate_meta.sh
── GATE-META: диапазон 045fef9a..HEAD, origin=a3ka/hft-platform
VERDICT: PASS — вердиктов проверено: 7, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=65f7810 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=65f7810 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 045fef9..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Metadata

- Date (UTC, ISO-8601): 2026-10-06T18:34Z
- Milestone: M-94-calc-profile
- Status: BLOCKED — routing decision required
- HEAD: 03a9dbd — docs(M-94): C-287 §3.2 absence table per binary, signature, cross-key; round 6 [architect]

## §B — What the critic did

- Audited the committed round-7 artifact set and executed C-287 B-1/B-2 RED and mutation controls.
- Confirmed the technical C-287 remedies; found the §0(1) arbitration-routing conflict.

## §C — Artifacts / results

- `research/critiques/C-288-M-94-calc-profile-round7.md`
- Done Block above: expected RED exits and mutation controls recorded.

## §D — Next agent + invocation

- **Next agent:** `arbiter` (strong model, fresh context)
- **Paste-ready prompt:**
  ```
  Resolve only the routing question for M-94 round 7.  Read `.claude/rules/gates.md` §0,
  `.claude/agents/critic.md`, `research/reviews/R-245-M-94-calc-profile.md`,
  `research/critiques/C-287-M-94-calc-profile-round6.md`,
  `research/critiques/C-288-M-94-calc-profile-round7.md`, and milestone §16 at
  origin/feat/M-94-calc-profile.  Decide whether §0(1)'s two-consecutive-REJECT trigger
  requires arbitration even when architect accepts the findings and no technical dispute
  remains.  Do not redesign M-94 or re-audit C-287 B-1/B-2 absent new evidence.  Write and
  push an A-NNN decision artifact on feat/M-94-calc-profile.
  ```
- Push status: pending this critic verdict commit.
- Cache: pending cleanup after the verdict is pushed.

## §E — Risks / open questions

- The subject branch may receive an engine-dev commit while the verdict is committed; verify
  that the audited `03a9dbd` remains an ancestor before pushing.
- No boundary-C founder signature is implicated.

=== END HANDOFF ===
