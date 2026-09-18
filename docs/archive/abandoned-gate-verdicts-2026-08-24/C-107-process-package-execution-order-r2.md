<!-- GATE-META
milestone: PR-42
audited_repo: a3ka/hft-platform
audited_base: af1fd0683881810909f020a69213c7d00331414b
audited_head: 935043eef901f20443ef9410a4805a8c53658027
verdict: REJECT
-->

# C-107 — PR #42 execution-order round 2: REJECT

## Verdict

**REJECT.** The new plan correctly accepts every finding of C-106 and correctly reads
`A-010` §J: its M-65 → M-66 → M-69 → harness chain does not contain M-68.  It is not yet
safe to execute.  Stage 0 would first merge PR #41 even though that PR states the three
still-live refs were already deleted, and its stated preconditions omit required parts of
R-095 У-1…У-3.  The proposed A2 mechanism also cannot enforce the boundary it introduces.

This is a harness/process plan.  No T1/T2 contract or trait signature is introduced; a
product milestone file is N/A under the harness track.  The committed B observer and its
RED probe are present and currently green, but C-106's repairs remain a proposal, not a
completed implementation.

## Answers to §6

| Question | Decision |
|---|---|
| 1. Is §2's order faithful to A-010 §J? | **Factually yes for steps 2–5.** §J has M-65, M-66, M-69, then harness, and zero `M-68` occurrences. M-68 may remain an explicitly new scheduling proposal, not an arbitration consequence; it needs its own stated dependency/rationale before dispatch. |
| 2. Is Stage 0 sufficient, and is a window needed? | **No; a window is required.** Do not merge PR #41 in its present factual form. Before each delete, meet and record the applicable R-095 conditions, publish the raw proof, then give an independent reader a defined review window before the delete command. |
| 3. Are two `gh` failure stubs sufficient? | **No.** Add a partial-checks fixture: list succeeds for at least two PRs, checks succeeds for one and transport-fails for another. It must preserve the known result, report the latter unavailable/unknown, and exit non-zero. |
| 4. Is `check_work_class.sh` sufficiently specified? | **No.** Its source of truth and semantic boundary are contradictory/incomplete; a path-only checker cannot enforce the promised process-polish limitation. |
| 5. Must mechanism and norm be separated? | **Yes.** First land and independently validate the harness mechanism; only from that `main` head land A1–A3 as a separately tokened §11 norm change and submit it to the applicable critic gate. A preceding commit in one PR is not proof that the mechanism protected the norm. |
| 6. Are earlier verdicts fully covered? | **No.** C-101, C-098, C-089, C-094, and R-095 all have outstanding named conditions missing or collapsed into non-executable shorthand below. |

## Blockers

### B-1 — PR #41 must not be merged before its premature deletion claims are corrected

`docs/plans/execution-order-2026-08-19.md:69` makes merging PR #41 the first irreversible
step, while `:73` defers removal of the premature “deleted 2026-08-19” inventory claims
until *after* all three deletion commands.  At audit time, execution contradicts those
claims: PR #41 is `OPEN` (`mergedAt: null`) and the three remote refs remain at `dc646cb`,
`51c21dc`, and `f0e915b`.

The PR #41 diff also adds a `SESSION-HANDOFF` statement that the three refs were deleted;
merging it would make that false claim part of `main` before the evidence and deletes exist.
This repeats C-106 F-106-5 in a more dangerous order.  Correct the factual state before
merging the carrier, rather than using a later cleanup step to repair an already-landed
falsehood.

### B-2 — Stage 0 does not operationalize R-095 У-1…У-3

The table at `execution-order-2026-08-19.md:69-77` never requires the U-1 patch and source
commit capture for **each** ref before deletion.  For M-10 and M-60 it also reduces U-2 to
absence in `BACKLOG` and `07-cockpit`, omitting the named `SESSION-HANDOFF` live-address
updates.  A dated historical snapshot may remain only when it is explicitly preserved as
history; it cannot be silently used to satisfy a condition that names the active addresses.

For M-60, U-3 requires both the selected preservation/discard alternative and the two
C-083 completion marks before deletion.  The plan's postcondition “guard marked” does not
name the required proof or prevent its being filled after the ref is gone.  Name every
precondition, its raw command, durable evidence carrier, responsible role, and the point at
which an independent reader may reject it.  Define the window (not merely “after them”):
the delete must remain a later, separately observable action.

### B-3 — A2's checker contract is internally inconsistent and too weak

At `:89-92`, the plan requires “exactly one” declaration yet also makes absence default to
`substantive`; it permits the declaration in a handoff even though CI cannot read a chat
handoff; and it promises only path conformance.  These are three different contracts, not
one enforcement rule.

More importantly, C-106 F-106-2 requires the same boundary to be enforced as the narrowed
`process-polish` prose.  A diff path of `milestones/*.md` cannot distinguish a Status/
close-out edit from Objective, Allowed paths, §Tasks, RED, or acceptance.  Nor can a generic
`docs/plans/**` path establish that a factual decision/order was not changed.  The plan must
choose a committed, machine-readable declaration source and a rule whose measurable surface
matches its claimed carve-outs; otherwise route ambiguous/document-semantic edits through
`substantive`.  The RED/battery set must cover those boundaries, the absent-declaration
policy, lying declarations for every class, and route evidence—not only one
`process-polish`-versus-`crates/*/src` case.

### B-4 — the dependency table omits live REJECT work

The plan claims to cover C-104/C-101/C-098/C-094/C-089, but its schedule is incomplete:

- `:50` names only A-1/A-3 for M-65.  A-010 §A requires A-1…A-4; C-098 B-1/B-3 remain
  concrete oracle/format work and A-2 is the required form/critic route.
- `:51` reduces M-66 to “rebuild.”  The plan must name the A-010 B-1…B-5 chain, including
  the successor-branch/PR lifecycle and the deterministic `C-089` B-1 condition rather
  than assuming a historical fix without merge-preview evidence.
- `:53` lists E-3/E-4 but not the unresolved C-101 `milestone-shape` REJECT conditions
  (fenced/comment pseudoheaders, substring stubs, rename detection, and a reproducible
  negative measurement).  E-3/E-4 concern ID reservation and do not repair that oracle.
- `:54` schedules M-68 without its C-094 B1…B6 artifact set: explicit T2/signature decision,
  verify gate, narrow-delta and checkpoint RED, resource oracle, complete forbidden scope,
  and separation of the unrelated roadmap decision.  “architect → critic” is not an
  executable prerequisite chain.

`C-104` B-6…B-8 are at least named at `:52`; retain their specific factual/acceptance
conditions when expanding the other rows.

## Required resubmission

1. Replace Stage 0 with an order that makes PR #41 truthful before its merge; enumerate and
   prove U-1/U-2/U-3 for each ref, then expose a concrete independent-review window before
   any deletion.
2. Specify an enforceable A2 data source and boundary, then expand its RED/battery plan to
   cover content-sensitive process-polish exclusions and the partial `gh pr checks` outage.
3. Expand the dependency table into the outstanding, named conditions of C-101, C-098,
   C-089, C-094, C-104, and R-095.  Do not dispatch a row whose required artifact set is
   still only implicit.
4. Keep M-68 explicitly labelled as a new proposal until its placement has a recorded
   rationale; do not attribute it to A-010 §J.

## Done Block

```text
$ git rev-parse HEAD
935043eef901f20443ef9410a4805a8c53658027
exit=0

$ git diff --name-status af1fd068..935043e
A  docs/plans/execution-order-2026-08-19.md
A  docs/plans/process-package-2026-08-19.md
A  research/critiques/C-106-process-package-2026-08-19.md
A  scripts/check_branch_health.sh
A  scripts/tests/red_branch_health.sh
exit=0

$ git show origin/main:research/arbitration/A-010-nine-disputes-2026-08-18.md | sed -n '/^## §J\./,/^## §K\./p' | grep -c 'M-68'
0
exit=1  # expected: grep found no M-68 in §J

$ gh pr view 41 --repo a3ka/hft-platform --json number,state,mergedAt,headRefName
{"headRefName":"docs/branch-disposition-2026-08-19","mergedAt":null,"number":41,"state":"OPEN"}
exit=0

$ git ls-remote --heads origin salvage/M-59-research-dev-uncommitted feat/M-10-rebased feat/M-60-mechanisms
51c21dccb8763690f231cd932bf1b974ac9cf510  refs/heads/feat/M-10-rebased
f0e915bf834506642740b798bf5e17242d1cf73f  refs/heads/feat/M-60-mechanisms
dc646cb6c86a128777ac84626811c6473ca5a2ba  refs/heads/salvage/M-59-research-dev-uncommitted
exit=0

$ bash scripts/tests/red_branch_health.sh --battery
сценариев исполнено: 11  ok: 11  FAIL: 0
ok         M1-ВИСЯК              kill-set совпал (S1-висяк-зелёный )
ok         M2-ДУБЛЬ              kill-set совпал (S4-дубль )
каталогов red-brhealth-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ bash scripts/check_branch_health.sh
NOTE  ВИСЯК: feat/harness-doc-integrity (PR #33, 1 сут) — все чеки зелёные, merge'а нет. Работа готова, приземления не случилось
NOTE  ВИСЯК: fix/resource-oracle-barrier (PR #28, 1 сут) — все чеки зелёные, merge'а нет. Работа готова, приземления не случилось
NOTE  ДУБЛЬ: предмет M-66 живёт на 4 ветках — docs/M-66-protocol-attestation feat/M-66 feat/M-66-fix-td097 feat/M-66-fixture
веток кроме main: 19; замечаний: 3
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
exit=0

$ bash scripts/check_docs_freeze.sh af1fd0683881810909f020a69213c7d00331414b
exit=0

$ bash scripts/check_gate_meta.sh af1fd0683881810909f020a69213c7d00331414b
── GATE-META: диапазон af1fd068..HEAD, origin=a3ka/hft-platform
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ bash scripts/check_protected_artifacts.sh af1fd0683881810909f020a69213c7d00331414b
OK: защищённые артефакты целы на HEAD (af1fd06..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ bash scripts/check_artifact_ids.sh af1fd0683881810909f020a69213c7d00331414b
OK: ни один коммит диапазона af1fd06..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ bash scripts/verify_design_claims.sh
VERDICT: PASS (0 нарушений)
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-19T00:00Z
- Milestone: PR-42 / process-package-2026-08-19
- Статус: BLOCKED — REJECT
- HEAD: 935043e — docs(plans): план исполнения после C-106 — НА СОГЛАСОВАНИЕ КРИТИКУ [architect]

## §B — Что я сделал
- Аудировал коммит 935043e и весь предмет C-106, план, B observer/probe, A-010 §J, R-095 У-1…У-6 и названные прежние verdicts.
- Проверил исполнением состояние PR #41/#28, все три remote refs, §J absence of M-68, probe/battery and structural gates.

## §C — Артефакты / результаты
- `research/critiques/C-107-process-package-execution-order-r2.md`
- Done Block: raw commands and exit codes are recorded above.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь план `docs/plans/execution-order-2026-08-19.md` по C-107, не исполняя
  необратимые действия. Сначала сделай PR #41 правдивым до его merge: распиши для каждого
  ref все применимые У-1/У-2/У-3 R-095, носитель сырого доказательства, роль и отдельное
  окно независимой проверки до delete. Для A2 выбери машиночитаемый committed source
  declaration и проверяемую границу process-polish; добавь в план RED/battery для
  content-sensitive exclusions и частичного gh checks outage. Разверни §2 в полный список
  незакрытых условий C-101/C-098/C-089/C-094/C-104/R-095. M-68 оставь явно новым
  предложением с обоснованием, не частью A-010 §J. Закоммить и push только изменённый план,
  затем запроси новый critic круг.
  ```
- Push-статус: ✅ verdict will be pushed to `origin/docs/process-package-2026-08-19` with this response.
- Кэш: ✅ кэш не создавался; временный worktree содержит только git-метаданные.

## §E — Риски / открытые вопросы
- Не удалять `dc646cb`, `51c21dc`, или `f0e915b`: все три refs живы, PR #41 OPEN.
- `check_branch_health` is green as a non-blocking observer; that does not discharge its planned C-106 outage repair.

=== END HANDOFF ===
