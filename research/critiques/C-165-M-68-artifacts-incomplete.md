<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: a5292aba59c476ba32f386a9747ce16c12acfcb7
audited_head: 59ef731c6af6aad790441ca2dbe2ccf80c2d396b
verdict: REJECT
-->

# C-165 — M-68 rev5: committed artifact set incomplete

## Verdict: REJECT — NOT REVIEWED — ARCHITECT ARTIFACTS INCOMPLETE

M-68 rev5 is not eligible for the plan-time gate.  The required milestone file,
existing RED suite, and verify script are present, and the milestone declares no
new T1/T2 contract or public trait signature.  Its only new committed delta,
however, is the milestone text.  That text adds tasks 12–16, each with a new
required oracle (`d9` through `d13`); no such RED oracle or verify step is
committed.  This is exactly the incomplete artifact set that the critic profile
requires to stop on, rather than a plan-only review.

`VB-I-2` remains applicable: task 15 explicitly forbids wall-clock cadence so
that live and replay stay bit-identical.  Without `d12`, that invariant is only
a statement in the milestone, not a committed RED oracle.

## Blocking findings

### B1 — rev5 requirements have no committed RED or acceptance coverage

`milestones/M-68-depth-from-book.md:433-437` adds these required oracles:

- task 12: `d11`, one book materialization per event;
- task 13: `d9`, no point for a one-sided book;
- task 14: `d10`, `depth_levels_visited` is per-call on `LiveReducer::pump`;
- task 15: `d12`, event-time, per-series depth cadence;
- task 16: `d13`, a consumer-visible cadence declaration for both series.

At the audited head, the candidate RED files are only
`red_depth_from_book.rs`, `red_depth_recompute_cost.rs`,
`red_depth_provenance_by_reach.rs`, and `red_gateway_schema_version.rs`.
`rg` finds no `d9`–`d13` reference anywhere in `crates/gateway/tests` or
`scripts/verify_M-68.sh` (exit 1).  The verify script retains its old literal
set of nine `d` cases and labels step A as tasks 1,2,3,4,5,7
(`scripts/verify_M-68.sh:41-50`); it therefore has neither one check per new
task nor a fail-closed check for the absence of any new oracle.

Reproduction:

```bash
git diff --name-status a5292ab..59ef731
git ls-tree -r --name-only 59ef731 -- crates/gateway/tests scripts \
  | rg '(^crates/gateway/tests/red_(depth|gateway_schema)|^scripts/verify_M-68\.sh$)'
rg -n 'd9|d10|d11|d12|d13' scripts/verify_M-68.sh crates/gateway/tests
```

Required resubmission: commit the RED tests for `d9`–`d13` and extend
`verify_M-68.sh` with a fail-closed, task-to-check mapping for tasks 12–16.
Each oracle must be RED against the current implementation before dev starts;
the d12 fixture must vary event time and demonstrate `VB-I-2`, and d13 must
assert the actual emitted form rather than only a reducer-local value.

### B2 — the claimed P-020 authority is not in the audited artifact set

M-68 rev5 states that a 100 ms heatmap bucket with depth bands would exceed the
signed 2 MB cap by about four times (`milestones/M-68-depth-from-book.md:73`).
But `docs/PENDING-SIGNATURE.md` at `59ef731` contains no `П-020` at all
(`git grep` exit 1).  The decision is in later `origin/main` commits
`bd20428` and `ddd899c`; `59ef731` is not an ancestor of `origin/main`.

The cap cannot be used as an audited, signed premise until the subject branch
contains the decision (or the milestone removes that premise and does not draw
conclusions from it).  This is especially material because the milestone uses
the cap to classify the Bookmap-resolution work as a separate prerequisite and
out of scope.

Reproduction:

```bash
git grep -n 'П-020' 59ef731 -- docs/PENDING-SIGNATURE.md
git grep -n 'П-020' 59ef731 -- milestones/M-68-depth-from-book.md
git merge-base --is-ancestor 59ef731 origin/main
git log --oneline 59ef731..origin/main -- docs/PENDING-SIGNATURE.md
```

Required resubmission: bring the signed P-020 decision into the audited branch
and then make the capacity prerequisite mechanically checkable, or remove the
P-020-based numerical conclusion from this milestone's committed text.

## Artifact-set disposition

| Required artifact | Audited status |
|---|---|
| Milestone | Present: `milestones/M-68-depth-from-book.md` |
| T-contracts / trait signatures | No new T1/T2 or public trait signature declared; no `crates/contracts/**` delta |
| Existing RED suite | Present, but stale for rev5 |
| RED suite for rev5 tasks 12–16 | Missing (`d9`–`d13`) |
| Acceptance script | Present, but stale for rev5 tasks 12–16 |
| Primary P-020 authority cited by rev5 | Absent from audited head |

Dev must not be dispatched on M-68 rev5 until the two blocking findings are
resolved in a committed artifact set and a new critic round is run.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-165
exit=0

$ git show -s --format='%H%n%s' 59ef731
59ef731c6af6aad790441ca2dbe2ccf80c2d396b
docs(M-68): спека rev5 — каденция задаётся НА СЕРИЮ; посылка «одна свежесть на обе» снята founder'ом [architect]
exit=0

$ git diff --name-status a5292ab..59ef731
M       milestones/M-68-depth-from-book.md
exit=0

$ git ls-tree -r --name-only 59ef731 -- crates/gateway/tests scripts | rg '(^crates/gateway/tests/red_(depth|gateway_schema)|^scripts/verify_M-68\\.sh$)' | sort
crates/gateway/tests/red_depth_from_book.rs
crates/gateway/tests/red_depth_provenance_by_reach.rs
crates/gateway/tests/red_depth_recompute_cost.rs
crates/gateway/tests/red_gateway_schema_version.rs
scripts/verify_M-68.sh
exit=0

$ rg -n 'd9|d10|d11|d12|d13' scripts/verify_M-68.sh crates/gateway/tests
exit=1

$ git grep -n 'П-020' 59ef731 -- docs/PENDING-SIGNATURE.md
exit=1

$ git grep -n 'П-020' 59ef731 -- milestones/M-68-depth-from-book.md
59ef731:milestones/M-68-depth-from-book.md:73:1.3 % и бакете 100 мс кадр выходит за подписанный предел 2 МБ (`П-020`) примерно вчетверо.
exit=0

$ git merge-base --is-ancestor 59ef731 origin/main
exit=1

$ git diff --check a5292ab..59ef731
exit=0

$ bash scripts/check_artifact_ids.sh 59ef731c6af6aad790441ca2dbe2ccf80c2d396b
OK: ни один коммит диапазона 59ef731..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ bash scripts/check_gate_meta.sh 59ef731c6af6aad790441ca2dbe2ccf80c2d396b
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ bash scripts/check_review_fa.sh 59ef731c6af6aad790441ca2dbe2ccf80c2d396b
SKIP (диапазон не трогает crates/**)
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-26T19:00Z
- Milestone: M-68-depth-from-book (rev5)
- Статус: BLOCKED
- HEAD: 59ef731 — docs(M-68): спека rev5 — каденция задаётся НА СЕРИЮ; посылка «одна свежесть на обе» снята founder'ом [architect]

## §B — Что я сделал
- Audited the committed M-68 rev5 artifact set at `59ef731`, not the handoff prose.
- Rejected the incomplete set and recorded the absent P-020 authority at the audited head.

## §C — Артефакты / результаты
- `research/critiques/C-165-M-68-artifacts-incomplete.md`
- Done Block: structural audit commands recorded above; no implementation acceptance result is used because the architect artifact set is incomplete.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-68 rev5 is REJECTED by C-165. On feat/M-68-rev4, commit a complete architect artifact set before requesting another critic round: RED tests d9–d13 for tasks 12–16; verify_M-68.sh checks mapped fail-closed to each of those tasks; and P-020 must be an ancestor of the audited subject head before the milestone relies on its 2 MB premise (or remove that premise). Keep implementation in crates/gateway/src/** out of the architect commit. Re-run the plan-time critic only after the complete set is committed and pushed.
  ```
- Push-статус: pending this verdict commit to `origin/feat/M-68-rev4`
- ⏸ кэш оставлен — `bash scripts/verify_M-68.sh` is still executing its workspace test target in this worktree.

## §E — Риски / открытые вопросы
- M-71 is explicitly incomplete in the handoff and was not audited.
- The acceptance script's current run is not a substitute for the missing rev5 RED/verify artifacts.

=== END HANDOFF ===
