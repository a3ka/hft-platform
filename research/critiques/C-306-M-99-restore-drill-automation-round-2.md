<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: eb6769f705b2c64f4fcc077c4a719fb387583336
verdict: NOTE
-->

# C-306 — M-99 restore-drill automation, round 2: NOTE

## Verdict

**NOTE — dispatch may proceed.** `C-298` B-1 is closed on the audited branch head. No new plan-time finding was identified in the committed artifact set.

The only substantive round-2 change after `C-298` is constrained to its stated repair:

- `docs/DESIGN.md` §22 now measures `OPS-I` as 10 oracle-backed items and names the M-99 `OPS-I-3` oracle;
- M-99 §6 permits exactly that DESIGN row and its round log records the repair;
- no RED oracle, contract, signature, acceptance gate, CI wiring, retention path, or boundary-C subject changed after the prior verdict.

`bash scripts/verify_design_claims.sh --merge-preview origin/main` now passes with `OPS-I — заявлено=11, в оракулах=10`. The declared total (11) and the oracle-backed subtotal (10) therefore agree with the measured state; the previous `9` was the stale subtotal found by C-298.

## Artifact-set audit

- **T-contracts and signatures:** M-99 §3 inherits M-74's T2 state-file contract, including mandatory 64-hex `digest` for `ok=1`; M-74 declares the producer and reader forms before implementation. M-99 §5 supplies the typed `RestoreDrillVerdict`, `restore_drill_verdict`, `check_restore_drill`, `run_cycle_with_drill`, and unchanged `sample_restore_drill` signature. No `crates/contracts/**` path is in the audited range, so Block-C is not implicated.
- **RED tests:** the committed journal fixture, recorder metric oracle, ops watchdog oracle, and `red_restore_drill.sh` probe are present. The declared COMPILE-RED surfaces remain expected pre-implementation state, not a vacuum: the milestone names the absent APIs and `verify_M-99.sh` distinguishes vacuum, compile-red, and executed failure.
- **Acceptance gate:** `scripts/verify_M-99.sh` uses an explicit `FAIL` accumulator, self-tests its helpers, has task checks for the open work, maps every `ci.yml` `run:` step or an exact exception, and exits nonzero on any failure. Its dedicated CI-map probe passes all ten worlds, including mutated added commands, stale exceptions, multi-line additions, and folded YAML.
- **Module invariants:** the planned cold-copy read directly serves `JR-I-6` (old journal remains readable by new code) and `OPS-I-3` (cold copy is periodically restored and read); producer emission is explicitly tied to `OPS-I-10`. The current head's `docs/fa/journal.md` and `docs/fa/ops.md` contain those IDs.
- **Scope and authority:** the full range from `a660b3a` contains only the milestone's stated tests, harness/CI files, M-74 evidence transfer, roadmap/design documentation, and gate artifacts. The M-99 changes do not alter Storage Box contents, retention, journal layout, T1 contracts, or a founder-only boundary-C decision.

Per the round-2 mandate, C-219 B-1/B-2 and the nine mutation cases were not reopened: no relevant test or probe changed after C-298, and C-298 remains the committed evidence for those executions.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-99-restore-drill-automation
eb6769f705b2c64f4fcc077c4a719fb387583336
exit=0

$ git merge-base --is-ancestor eb6769f7 origin/feat/M-99-restore-drill-automation
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: OPS-I — заявлено=11, в оракулах=10 — подтверждено замером (loose=10)
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/tests/red_verify_M-99_ci_map.sh
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

$ VERIFY_M99_CI_DRY=1 bash scripts/verify_M-99.sh
PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются
PASS: ci-parity: учтено шагов 65 из 65 (исполнено 58, исключено по карте 6)
VERDICT: PASS
exit=0

$ git diff --check a660b3a7b78b116a0a98bc7c9d9eb1e03153725c..eb6769f705b2c64f4fcc077c4a719fb387583336
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-11T00:00Z
- Milestone: M-99-restore-drill-automation
- Статус: DONE — NOTE; plan-time gate is clear
- HEAD: eb6769f7 — docs(M-99): C-298 B-1 — DESIGN §22 OPS-I «в оракулах» 9 → 10 (оракул OPS-I-3), путь в Allowed, журнал кругов [architect]

## §B — Что я сделал
- Audited the committed M-99/M-74 contract, signature, RED, acceptance, CI-map, scope, and invariant artifacts at the branch head.
- Re-ran the required merge-preview, CI-map, and dry CI-parity checks; confirmed the C-298 B-1 repair.

## §C — Артефакты / результаты
- `research/critiques/C-306-M-99-restore-drill-automation-round-2.md`
- Done Block: merge-preview exit=0; CI-map exit=0; dry acceptance exit=0; diff-check exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `engine-dev`
- **Paste-ready промпт:**
  ```
  M-99-restore-drill-automation has passed plan-time critic round 2 by C-306 (NOTE) at audited head eb6769f705b2c64f4fcc077c4a719fb387583336. Implement only M-99 tasks 2, 2b, 3, 4, 5, and 6 in the milestone Allowed paths. Do not modify sacred RED tests or verify scripts; preserve the inherited T2 digest contract, M-99 §5 signatures, C-219 B-1/B-2 guard construction, and the ban on retention/Storage Box mutations. Drive the committed RED suites to GREEN and run `bash scripts/verify_M-99.sh` to exit 0 before handoff.
  ```
- Push-статус: pending this C-306 artifact commit to `origin/feat/M-99-restore-drill-automation`.
- Кэш: ✅ кэш не создан в audit worktree.

## §E — Риски / открытые вопросы
- The planned first live remote drill remains reviewer close-out task 8; no founder boundary-C decision is requested by this plan.

=== END HANDOFF ===
