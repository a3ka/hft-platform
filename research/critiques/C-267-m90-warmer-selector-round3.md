<!-- GATE-META
milestone: M-90
audited_repo: a3ka/hft-platform
audited_base: 009d82a71377877d0b9ab239790f0e6a3c28a1b8
audited_head: 195157bd4b00785f5fb1cc080eb753ae7ed53652
verdict: NOTE
-->

# C-267 — M-90 warmer selector, round 3: A-043 §6 frozen checklist

## Verdict

**NOTE — the six frozen checks in `A-043` §6 pass.** This is the required
precondition for tester, subject to engine-dev first landing its GREEN production
implementation. At the audited head no engine-dev commit is in the branch, so
the sacred suite correctly remains **2 passed / 6 failed**; this is the
pre-GREEN state prescribed by §6(5), not a rejection.

This audit intentionally does not reopen the arbitrated decisions: CI-step
identity is the SHA-256 fingerprint of the full `run:` text, cadence RED is
sufficient, and engine-dev proceeds in parallel. The live module invariants
remain `VB-I-11` (writer and reader must share checkpoint history) and
`OPS-I-8` (alive-but-unproductive is an incident).

## Frozen checklist — A-043 §6

1. **K-1 — PASS.** `verify_M-90.sh` has no `"$first"` comparison that opens a
   `SKIP` or bypass. The `rfa_append` world executes the full block rather than
   taking the review-FA skip. The same 10-world probe against the `e128ee7`
   verifier has exactly the three prescribed failures.
2. **K-2/K-3 — PASS.** The probe prints `VERDICT: PASS — 10 сценариев`.
   Literal `|-` is parsed as a block and folded `>` fails explicitly. Replacing
   only the verifier with `e128ee7` yields exactly three failing new worlds.
3. **K-4 — PASS.** Milestone §9 names all four limits: full-text `run:`
   fingerprint without job/name, map scope limited to `run:`, aggregate
   `needs`/condition ownership, and opaque-fingerprint reviewer description.
   It cites `A-043`; §12 records the round-3 decision.
4. **K-5 — PASS.** The verify-step label contains no numeric world-count
   literal; the probe itself reports its count.
5. **Architect range / RED state — PASS.** The three `[architect]` commits
   after `e128ee7` modify only `scripts/verify_M-90.sh`,
   `scripts/tests/red_verify_M-90_ci_map.sh`, and the M-90 milestone. The
   gateway RED test file is unchanged. With engine-dev absent, `w2` and `w4e`
   pass while `w1`, `w3`, and `w4a`–`w4d` fail for their intended reasons.
6. **Closed findings — PASS.** No run reopened C-265-F1 or C-266-F1: the
   per-axis RED worlds and six-name legacy-source refusal remain intact. Their
   planned GREEN transition belongs to engine-dev, then tester.

## Artifact and scope check

The committed plan-time set is present: M-90 milestone, sacred gateway RED
suite, verifier, and CI-map anti-placebo probe. It adds no `contracts/**` T1
change, no new T2 type, and no trait signature; the existing
`CHECKPOINT_RUNNER` seam remains the test boundary. The post-`e128ee7`
architect scope is the arbitrator’s three-file carve-out. The distinct
`A-043` arbitration artifact is not an architect change.

The direct full verifier is expected to be red before implementation because
it includes the sacred RED suite. Its CI-map-only form is green (`55/55`), and
the frozen adversarial probe is green. Two attempted complete verifier runs
were stopped during simultaneous M-48 gateway test contention; no incomplete
whole-gate result is used for this verdict.

## Done Block

```text
$ git fetch origin --prune && git rev-parse origin/feat/M-90-warmer-selector-single-source
195157bd4b00785f5fb1cc080eb753ae7ed53652
$ git merge-base origin/main origin/feat/M-90-warmer-selector-single-source
009d82a71377877d0b9ab239790f0e6a3c28a1b8

$ git show --format='%h %s' --name-only --no-renames c161bf8 18d603b 195157b
c161bf8 ... [architect]
scripts/verify_M-90.sh
18d603b ... [architect]
scripts/tests/red_verify_M-90_ci_map.sh
195157b ... [architect]
milestones/M-90-warmer-selector-single-source.md
exit=0

$ git diff --exit-code e128ee7 -- crates/gateway/tests/
exit=0
$ rg -n '\\$first[[:space:]]*(=|==|!=)' scripts/verify_M-90.sh
(no output)
exit=1  # no prohibited comparison found
$ rg -n '[0-9]+[[:space:]]+мир' scripts/verify_M-90.sh
(no output)
exit=1  # no numeric world-count literal found

$ bash scripts/tests/red_verify_M-90_ci_map.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
PASS  строка дописана в блок базы ⇒ FAIL (C-266 F2) (exit=1)
PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
PASS  (i) check_review_fa + дописка ⇒ исполняется, не SKIP (exit=0)
PASS  (ii) run: |- — литеральный блок, исполняется (exit=0)
PASS  (iii) run: > — складывающий скаляр ⇒ FAIL (exit=1)
VERDICT: PASS — 10 сценариев
exit=0

$ # current 10-world probe, with only verify_M-90.sh replaced by e128ee7 in a clean scratch worktree
FAIL  (i) check_review_fa + дописка ⇒ исполняется, не SKIP: exit=0 ...
FAIL  (ii) run: |- — литеральный блок, исполняется: exit=0 ...
FAIL  (iii) run: > — складывающий скаляр ⇒ FAIL: exit=0 ...
VERDICT: FAIL (провалов: 3)
exit=1

$ cargo test -p gateway --test red_m90_warmer_cron_composition
test w2_default_dotenv_snapshot_is_found_positive_control ... ok
test w4e_depth_cadence_axis_single_source ... ok
test w1_cron_warmer_snapshot_is_found_by_server_with_prod_dotenv ... FAILED
test w3_script_refuses_own_selector_copy_and_names_it ... FAILED
test w4a_venue_axis_single_source ... FAILED
test w4b_symbol_axis_single_source ... FAILED
test w4c_timeframe_axis_single_source ... FAILED
test w4d_window_axis_single_source ... FAILED
test result: FAILED. 2 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
exit=101  # expected RED before engine-dev GREEN

$ VERIFY_M90_CI_DRY=1 bash scripts/verify_M-90.sh
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/next_artifact_id.sh C
C-267
exit=0
$ bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-267 ← 656921b9e6375841c45d69b711205112dc64a828
C-267
reserve: резерв C-267 взят
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-10-01T19:32Z
- Milestone: M-90-warmer-selector-single-source
- Статус: DONE — NOTE C-267; frozen A-043 §6 gate passed
- HEAD: 195157b — M-90 §9 limits and §12 round-3 entry

## §B — Что я сделал

- Audited exactly A-043 §6(1–6) against the fetched branch tip.
- Ran the 10-world probe and its `e128ee7` control; verified the intended
  pre-GREEN RED-suite state and the architect-only three-file range.

## §C — Артефакты / результаты

- `research/critiques/C-267-m90-warmer-selector-round3.md`
- Done Block above: probe exit=0; old-verifier control exit=1 with exactly
  three failures; RED suite exit=101 as expected; CI-map dry verifier exit=0.

## §D — Следующий агент + инвокация

- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-90 C-267 is NOTE on origin/feat/M-90-warmer-selector-single-source.
  Do not reopen A-043’s decided key form, cadence RED, or parallel route.
  When engine-dev has committed and pushed tasks 1–3, hand off tester on the
  fetched branch tip. Tester must run
  cargo test -p gateway --test red_m90_warmer_cron_composition and require all
  eight worlds GREEN, plus the relevant M-90 acceptance checks. Preserve the
  A-043 §2.4 three-file architect carve-out and the sacred RED files.
  ```
- Push-статус: pending this verdict commit and push to `origin/feat/M-90-warmer-selector-single-source`
- Кэш: pending cleanup after commit/push

## §E — Риски / открытые вопросы

- Engine-dev has not yet landed in audited head `195157b`; tester remains
  conditional on its GREEN commit, not on this NOTE alone.
- The whole verifier is expected RED before GREEN; concurrent M-48 test runs
  prevented a useful complete rerun and are explicitly not used as evidence.

=== END HANDOFF ===
