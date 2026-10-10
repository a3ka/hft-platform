<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: ed0309b436c8e0c3bc85d23b80d180d0420f2778
verdict: REJECT
-->

# C-298 — M-99 restore-drill automation: REJECT

## Verdict

**REJECT — do not dispatch engine-dev.** The committed artifact set closes `C-219` B-1/B-2 and its declared mutation table is live. It nevertheless makes the merge-preview design-claims gate red: the new `OPS-I-3` oracle raises the real §22 `OPS-I` count from 9 to 10 while `docs/DESIGN.md:921` still declares 9. This is a gates.md §9(a) contradiction, not an advisory documentation nit: `bash scripts/verify_design_claims.sh --merge-preview origin/main` exits 1.

### B-1 — the declared OPS-I oracle count is stale

`crates/ops/tests/red_m99_restore_drill_watch.rs` names and tests `OPS-I-3`; `crates/recorder/tests/red_restore_drill_metric.rs` names the complementary `OPS-I-10` producer obligation. The design-claims gate measures **10** strict `OPS-I` oracle identifiers at the audited merge-preview, but §22 retains `OPS-I | ... | 11 | 9 |`.

The mismatch is introduced by this artifact set, so the RED state is not an inherited unrelated failure. Leaving the count at 9 would make the required CI parity fail after implementation too; moving the new test out of the count would instead hide the live `OPS-I-3` evidence. The plan must make the documented measurement true.

**Condition to clear:** architect must add `docs/DESIGN.md` to M-99 Allowed paths, update its §22 OPS-I count to the measured value (10), and rerun `bash scripts/verify_design_claims.sh --merge-preview origin/main` on the revised branch head. Because both the milestone form and a `gates.md` §9 document change, this subject requires a new critic pass over that new head.

## Confirmed artifact set and closures

- The inherited T2 state, reader, producer, wrapper, selection, and `INV-DELIVERY` contracts are present in `milestones/M-74-restore-drill.md`; M-99 §3 names exactly what is inherited and §4 lists every change.
- M-99 §5 supplies the typed `RestoreDrillVerdict`, watchdog-cycle, metric-producer, and binary entry-point signatures. Its RED files are committed before implementation: journal fixture, recorder metric oracle, ops watchdog oracle, and shell probe. `scripts/verify_M-99.sh` is a fail-counting gate with CI parity and one task check per open task.
- **C-219 B-1:** the source is removed, not hidden; `retire_and_read_watched` starts an inotify watcher before removal and `source_stayed_absent` rejects a re-created `cold` name across the whole independent read. A11 passes normally and fails when only that guard is neutralized.
- **C-219 B-2:** `snapshot_source` records each pre-wrapper `*.json` SHA-256; `delivery_sidecars_identical` judges the restored files after source retirement. A12 (omission) and A13 (same-meaning byte substitution) pass normally and fail only when that guard is neutralized.
- The nine declared mutations were reproduced against in-memory copies of the probe. M1→A11; M2→A12/A13; M3 (rename instead of delete) leaves A11 caught as `rebound-hidden`; M4→A11; M5→D/R; M6→A8; M7→A9; M8→A10; M9→A1. For M9, the mutation retained the watcher setup and neutralized only `d_after == d_live`; otherwise deleting the whole helper would merely make the adjacent watcher uninitialized and would not test the stated guard. No declared guard was inert.

## gates.md §9 recheck

- **(a) Code claims:** merge-preview design claims is red solely at the stale OPS-I total above.
- **(b) Authority:** the changed `crates/*/tests/**`, `scripts/**`, `.github/workflows/ci.yml`, milestone, inherited artifacts, and roadmap-order row match the architect-owned allowed paths. No T1 contract, retention/Storage Box path, boundary-C decision, or process-lock path changed. `check_docs_freeze` and `check_protected_artifacts` pass.
- **(c) Linkage:** `red_verify_M-99_ci_map.sh` passes all ten fixture worlds, including added/changed CI commands and folded YAML. The CI step calls `bash scripts/tests/red_restore_drill.sh` unconditionally in `build-test`; it is not merely named in the milestone.

## Named limits verified

M-99 §11 correctly retains the `A-032` limits: a local probe does not prove Storage Box network delivery; the three-segment selection is not full-copy coverage; direct aliases to an implementation stash are outside the settled pseudonym-form boundary; source destruction is a construction measure rather than a separately claimed oracle; and monthly detection is not coupled to daily retention. `OPS-I-11` also honestly leaves the watchdog alert on the host log rather than an external notification channel.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-99-restore-drill-automation
ed0309b436c8e0c3bc85d23b80d180d0420f2778
exit=0

$ bash scripts/next_artifact_id.sh C
C-298
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

$ mutation table (§8): M1/M2/M3/M4/M5/M6/M7/M8/M9
M1: FAIL A11 ... признана честной
M2: FAIL A12 ... признана честной; FAIL A13 ... признана честной
M3: PASS A11 ... ПОЙМАН стражем окна (rebound-hidden)
M4: FAIL A11 ... признана честной
M5: FAIL D ... признана честной; FAIL R ... принят как свой
M6: FAIL A8 ... признана честной
M7: FAIL A9 ... признана честной
M8: FAIL A10 ... признана честной
M9 (only d_after == d_live neutralized; watcher retained): FAIL A1 ... признана честной
mutation_expectation_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=a660b3a7... bash scripts/check_docs_freeze.sh
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=a660b3a7... bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (a660b3a..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
FAIL  [2-ПОКРЫТИЕ] §22: семейство OPS-I — документ заявляет 'в оракулах'=9, реальный замер (анти-плацебо) strict=10, loose (любое упоминание)=10
VERDICT: FAIL (1 нарушений)
exit=1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-10T00:00Z
- Milestone: M-99-restore-drill-automation
- Статус: BLOCKED — REJECT
- HEAD: ed0309b4 — docs(ROADMAP): Блок 1 строка 1 — остаток M-74 живёт в M-99 [architect]

## §B — Что я сделал
- Audited the committed branch head, inherited M-74 contracts, M-99 milestone, RED files, verify gate, CI wiring, C-219, A-032, OPS/JR invariants, and TD-193.
- Reproduced the C-219 closures and all nine declared guard mutations; rechecked gates.md §9(a)–(c).

## §C — Артефакты / результаты
- `research/critiques/C-298-M-99-restore-drill-automation.md`
- Done Block: CI-map probe exit=0; protected/process checks exit=0; merge-preview design-claims exit=1 (blocking OPS-I count drift).

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- Push-статус: ✅ pushed to origin/feat/M-99-restore-drill-automation with this C-298 verdict commit.
- Кэш: ⏸ кэш оставлен — shared test target is in use by another active worktree.
- **Paste-ready промпт:**
  ```
  M-99 is REJECT by C-298 at audited head ed0309b436c8e0c3bc85d23b80d180d0420f2778. Before dispatch, add docs/DESIGN.md to M-99 Allowed paths and update DESIGN §22 OPS-I oracle count from 9 to the measured 10; do not suppress the new OPS-I-3 RED oracle. Push the amended artifact set, run `bash scripts/verify_design_claims.sh --merge-preview origin/main` to exit 0, then request a new strong-model critic pass over the branch head. Do not alter the settled A-032 alias/network boundaries or the C-219 guard construction.
  ```

## §E — Риски / открытые вопросы
- The declared production RED lines (missing wrapper and reader, plus compile-RED consumer/watchdog APIs) are expected before engine-dev and are not this REJECT.
- M-99 remains outside boundary C: retention, Storage Box contents, and the founder choice of drill frequency are unchanged.

=== END HANDOFF ===
