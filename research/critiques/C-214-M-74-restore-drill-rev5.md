<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: e758b568d78eb057be70374058d9b4989ef9b2ca
audited_head: ceac650b64555a660f5c9c30f862aba401fc1525
verdict: REJECT
-->

# C-214 — M-74 restore-drill rev 5: REJECT

## Verdict

**REJECT — do not dispatch engine-dev.** This is round 1 of the authorized rev-5 reconstruction. The new digest oracle accepts a wrapper that does not restore anything: it aliases `restore` to `cold` and returns a real digest read from `cold`. That proves cold data is readable, not that a delivered restore is readable. The relevant live invariant is **JR-I-6** (`docs/fa/journal.md`, §I); M-74 must additionally prove the operational after-restoration boundary.

## B-1 — cold-as-restore bypass passes the delivery cases

`run_drill` gives the wrapper both paths, but H only compares state digest with `truth_digest "$restore"` (`scripts/tests/red_restore_drill.sh:214-225,305-334`). The truth reader follows a symlink and no assertion requires `restore` to be a distinct materialized result. N invokes the future reader separately on **cold** (`:205-210,253-267`), independent of the wrapper, so it cannot observe the wrapper's reader argument.

I made a temporary adversarial wrapper in a separate disposable worktree. It removes restore, creates `restore -> cold`, computes the real digest from cold with the direct journal helper, writes matching success state, and invokes neither `JOURNAL_DRILL_READER` nor a delivery copy. The unmodified probe accepted H/C/E/F/A. It was red only because task 2b's reader is intentionally absent. Once task 2b exists, N becomes green independently and this same bypass can make the probe green. The production-equivalent bypass is simply to run the new reader on cold, not restore; the current oracle observes neither.

This violates the still-live wrapper contract requiring the reader directory to be the delivery directory (`milestones/M-74-restore-drill.md:315-319`). F only models unreadable cold files, so it cannot detect the alias in H, where both the wrapper and the independent truth reader use the same cold data.

**Condition to clear:** commit a RED oracle that rejects a wrapper whose restore path aliases, resolves to, or derives digest from cold rather than a separately materialized delivery result. It must remain red after task 2b exists and judge the actual reader path/argument, not wrapper text or a self-reported field.

## B-2 — active milestone is contradictory

The plan says rev 5 removes the observer/W (`milestones/M-74-restore-drill.md:160-161`) but also claims the active probe still has `S/W/P/H/C/E/F/A` and rev-4 observer coverage (`:492,530,583,635`). The rev-5 script contains neither observer nor W. These are active coverage claims rather than harmless history, so a dev cannot derive one unambiguous mandatory artifact set. Update active contract, task inventory, coverage map, route/handoff and round count to rev 5 only; preserve rev 4 as explicitly historical.

## Artifact-set audit

| Required artifact | Result | Evidence |
|---|---|---|
| T2 contracts / signatures | Present | State, reader and wrapper contracts are in the milestone; no `contracts/**` change. |
| RED tests | **Incomplete — REJECT** | Fixture and D/R exist, but no oracle rejects cold-as-restore / wrong reader directory. |
| Acceptance gate | Present, real | Counted FAIL, nonzero exit, CI parity; it delegates to the same incomplete probe. |
| Milestone | **Contradictory — REJECT** | B-2 retains removed rev-4 coverage as current. |
| Scope / T1 | Pass | Only allowed journal test, shell probe and milestone changed; no T1/risk path. |

`verify_M-74.sh` returns `FAIL (15)`, exit 1 at the audited head, not the handoff's claimed 16. That is a NOTE, not a rejection basis.

## Required disposition

Architect corrects B-1 and B-2, commits the amended artifacts, and returns for critic round 2. Engine-dev remains blocked. This does not reopen A-028's old round count: founder authorized a distinct rev-5 reconstruction.

## Done Block

```text
$ git log --oneline e758b56..ceac650
ceac650 test(M-74): задачи 1b/6b/1c — rev 5, свидетельство стало ПРОДУКТОМ действия [architect]

$ git diff --name-status e758b56..ceac650
M	crates/journal/tests/fixture_restore_drill_cold.rs
M	milestones/M-74-restore-drill.md
M	scripts/tests/red_restore_drill.sh
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  обёртки deploy/bin/journal-restore-drill-cron.sh НЕ СУЩЕСТВУЕТ — RED задачи 1
PASS  D выдуманный отпечаток ПОЙМАН при ЧЕСТНО восстановленном каталоге
PASS  R отпечаток прошлого прогона ОТВЕРГНУТ — фикстура одноразова, признак различает
VERDICT: FAIL (1 из 3)
exit=1

$ cargo test -p journal --test fixture_restore_drill_cold --quiet
running 6 tests
......
test result: ok. 6 passed; 0 failed
exit=0

$ adversarial wrapper: restore -> cold; digest from cold; bash scripts/tests/red_restore_drill.sh
PASS  H здоровая копия ⇒ drill прошёл: отпечаток состояния СОВПАЛ с отпечатком прод-читателя
PASS  C повреждённый сегмент ⇒ drill отказал rc=4, ok=0, отпечатка НЕТ
PASS  E пустое восстановление ⇒ отказ с ОТДЕЛЬНОЙ причиной: empty
PASS  F доставка молча не сработала ⇒ drill отказал rc=5 (ПУСТОТА), ok=0, отпечатка нет
PASS  A оборванное состояние ПЕРЕЗАПИСАНО целиком
FAIL  N ПРОД-ЧИТАТЕЛЬ не выдал отпечаток (бинаря journal-drill-read ещё нет — задача 2b)
VERDICT: FAIL (1 из 12)
exit=1

$ mutation: h_verdict digest equality replaced with true
453:  [ "${ok}" = "1" ] && [ "${ev:-0}" -gt 0 ] && [ -n "${d_truth}" ] && true
FAIL  D обёртка, восстановившая копию, но ВЫДУМАВШАЯ отпечаток, признана честной
FAIL  R отпечаток ЧУЖОГО прогона принят как свой
VERDICT: FAIL (3 из 12)
exit=1

$ bash scripts/verify_M-74.sh
PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются
PASS: cargo fmt --all -- --check
PASS: фикстура прод-формы читается journal::stream (исполнено тестов: 6)
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: bash scripts/tests/red_restore_drill.sh
VERDICT: FAIL (15)
exit=1

$ bash scripts/next_artifact_id.sh C
C-214
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-07T23:41Z
- Milestone: M-74-restore-drill
- Статус: BLOCKED — REJECT, round 1 of the rev-5 reconstruction
- HEAD: ceac650 — test(M-74): задачи 1b/6b/1c — rev 5, свидетельство стало ПРОДУКТОМ действия [architect]

## §B — Что я сделал
- Audited `e758b56..ceac650`, reproduced D/R, and built the cold-as-restore bypass.

## §C — Артефакты / результаты
- `research/critiques/C-214-M-74-restore-drill-rev5.md`
- Done Block: intentional RED exit=1; fixture 6/6 exit=0; bypass probe exit=1 only for missing task-2b reader; mutation exit=1; verify exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- Push-статус: recorded with this verdict commit on `origin/docs/M-73-closeout-architect`.
- Кэш: ⏸ no dedicated critic target; disposable attack worktree is removed.
- **Paste-ready промпт:**
  ```
  M-74 rev-5 critic round 1 is REJECTED by C-214. Do not dispatch engine-dev. Read C-214. Add a RED oracle that rejects restore->cold / reader-on-cold bypasses and update stale rev-4/W observer claims so there is one active rev-5 artifact set. Commit and push amended artifacts, then request critic round 2 with new SHAs.
  ```

## §E — Риски / открытые вопросы
- B-1 is an OPS-I-3 delivery gap, not expected task RED.
- A-028's former limit is historical; this is the first REJECT of the authorized rev-5 reconstruction.

=== END HANDOFF ===
