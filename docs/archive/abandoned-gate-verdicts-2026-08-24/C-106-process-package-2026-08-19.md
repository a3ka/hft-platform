<!-- GATE-META
milestone: PR-42
audited_repo: a3ka/hft-platform
audited_base: af1fd0683881810909f020a69213c7d00331414b
audited_head: 06b74ef982a738956bc3e032ea7cf75ced926307
verdict: REJECT
-->

# C-106 — PR #42 process package: REJECT

## Verdict

**REJECT.** Do not land A1–A3 or the branch-health observer in their current form.

The branch carries the appropriate harness-track pair for B (plan, observer, hermetic
probe); no T1 contract, T2 trait signature, product milestone, or production-path code is
introduced.  That does not make the package passable: A2 proposes a new binding rule without
its required mechanism, and B reports a false PASS when either live PR data source fails.

## Answers to the five PR questions

| PR question | Decision |
|---|---|
| 1. Is A2 needlessly four classes rather than three? | **No on the count:** `mixed` is the necessary fail-closed override rather than a permissive carve-out. **REJECT A2 as written:** its mandatory declaration/routing rule has no delivered enforcement mechanism (F-106-1). |
| 2. May `process-polish` include `milestones/*.md` and `docs/plans/**`? | **No.** Those globs include Objective, Allowed/Forbidden paths, tasks, RED/acceptance, and factual plan claims; the stated exclusions only cover invariants/boundaries/phases (F-106-2). |
| 3. Does A3 weaken existing carve-outs? | **No.** A substantive path forcing the substantive route is strictly stronger. It remains dependent on a repaired A2 classification/enforcer. |
| 4. Are B's probe and the named `ВИСЯК` limit sufficient/honest? | **No.** The checks-vs-verdicts limit is honest, but incomplete: an unavailable `gh` source is silently converted to “no stale green PR” or to `red`, followed by `VERDICT: PASS` (F-106-3). |
| 5. Does section D hold? | **Only D4 holds.** D1–D3 are not completed actions while PR #41 is open and all three refs still exist; D5 contradicts A-010 §J (F-106-5). |

## Blockers

### F-106-1 — A2 adds binding routing without a mechanism

The proposed `РАБОТА-КЛАСС` declaration is mandatory, defaults an absent declaration to a
stricter route, and selects the final role.  This is an obligatory process rule.  Its only
`COGNITIVE-ONLY` qualification is for the *truth* of the declaration; the text explicitly
says presence and diff-path conformance are mechanically checkable, then names only a
candidate future checker.

`docs/workflow/binding-requires-mechanism.md` requires a co-delivered mechanism or an
explicit whole-section `COGNITIVE-ONLY` explanation where automation is impossible; reviewer
backstop does not count.  B observes a downstream stale branch, not whether the declaration
was present or routed the gate.  Add a tested checker/CI or hook that requires exactly one
class declaration, rejects incompatible paths, and exercises absent/lying/mixed stubs.  Do
not land the mandatory A2 prose ahead of that mechanism.

### F-106-2 — `process-polish` is an unsafe path-classification rule

`milestones/*.md` is not prose-only: outside the dev Status-cell carve-out it carries the
implementation contract (Objective, allowed paths, tasks, RED tests, and acceptance).  A
change to an existing milestone can therefore alter scope or an oracle without changing an
invariant, boundary, or phase.  `docs/plans/**` likewise contains operational and factual
claims consumed by agents.  The table would route either as author-only process polish.

Restrict the class to named status/close-out/editorial fields, or make a change to Objective,
allowed/forbidden paths, tasks, RED, acceptance, factual decision/order, or any executable
instruction substantive.  The checker required by F-106-1 must enforce the same boundary.

### F-106-3 — B treats a failed live `gh` source as a successful observation

The production path suppresses `gh pr list` failure with `|| true`; no PR rows then means no
stale-green finding.  Separately, any non-pending failure from `gh pr checks` is labelled
`red`, indistinguishable from a known red check.  Both violate the script's own fail-closed
claim and `testing.md` gate-integrity property 3 (failed setup must be red).

The committed probe injects `BRANCH_HEALTH_PRS` and tests only an unreadable injected file;
it never exercises either real `gh` failure path.  It therefore passes 11/11 and both existing
mutants while missing the production-form failure below.

Repair: make failed `gh pr list` and a check-state transport/error result increment `FAILED`
and exit nonzero; distinguish actual completed-red from unavailable state; add hermetic stubs
for both failures and a mutation proving that each scenario kills its owning check.

### F-106-4 — the plan's mechanism source is a dead cross-reference

The plan cross-reference names `.claude/rules/binding-requires-mechanism.md`, which does not
exist at the audited head.  The actual source is
`docs/workflow/binding-requires-mechanism.md`.  Correct the path before this document is used
as a landing specification.

### F-106-5 — section D must distinguish completed preconditions from requested actions

| Action | Result at audit head |
|---|---|
| D1 delete `salvage/M-59-research-dev-uncommitted` | **Not confirmed / do not execute yet.** R-095's content conclusion is supported, but U-1 preservation and the delete are not completed by open PR #41; the remote ref remains `dc646cb`. |
| D2 delete `feat/M-10-rebased` | **Not confirmed / do not execute yet.** PR #41 is still open and the remote ref remains `51c21dc`; U-1/U-2 are not completed facts. |
| D3 delete `feat/M-60-mechanisms` | **Partly confirmed, action not complete.** PR #41's roadmap is byte-identical to the branch and marks the C-083 condition as fulfilled, but the remote ref `f0e915b` remains and PR #41 is open. Its text claiming deletion on 2026-08-19 is premature. |
| D4 do not merge PR #28 | **Confirmed.** PR #28 is open; its head is unchanged since R-095, and H-11 still leaves the harness-track cleanup measurement unclosed. |
| D5 `M-66 → M-65 → M-69 → M-68 → harness` | **Refuted.** A-010 §J orders the outstanding chain `M-65` work before M-66, then M-69, then harness work; it does not put M-68 in that sequence. None of those subject branches is an ancestor of `origin/main`, so no merge establishes a replacement order. |

## Required resubmission

1. Deliver the A2 classifier/enforcer and its hermetic RED/battery proof in the same landing
   chain, then narrow `process-polish` as above.
2. Fix B's two live-source failure modes; extend `red_branch_health.sh --battery` with both
   stable `gh` outage stubs and their kill-sets.
3. Correct the binding-mechanism cross-reference.
4. Rewrite D1–D3 as conditional future actions until PR #41 is merged, U-1 is recorded, and
   the remote refs are actually deleted; replace D5 with the current dependency order or cite
   a newer primary decision that supersedes A-010 §J.

## Done Block

```text
$ git rev-parse HEAD
06b74ef982a738956bc3e032ea7cf75ced926307

$ git diff --name-status af1fd068..06b74ef
A  docs/plans/process-package-2026-08-19.md
A  scripts/check_branch_health.sh
A  scripts/tests/red_branch_health.sh
exit=0

$ bash scripts/check_branch_health.sh
NOTE  ВИСЯК: feat/harness-doc-integrity (PR #33, 1 сут) — все чеки зелёные, merge'а нет. Работа готова, приземления не случилось
NOTE  ВИСЯК: fix/resource-oracle-barrier (PR #28, 1 сут) — все чеки зелёные, merge'а нет. Работа готова, приземления не случилось
NOTE  ДУБЛЬ: предмет M-66 живёт на 4 ветках — docs/M-66-protocol-attestation feat/M-66 feat/M-66-fix-td097 feat/M-66-fixture
веток кроме main: 19; замечаний: 3
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
exit=0

$ bash scripts/tests/red_branch_health.sh --battery
сценариев исполнено: 11  ok: 11  FAIL: 0
ok         M1-ВИСЯК              kill-set совпал (S1-висяк-зелёный )
ok         M2-ДУБЛЬ              kill-set совпал (S4-дубль )
каталогов red-brhealth-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ PATH=<outage-gh>:$PATH BRANCH_HEALTH_ROOT=<isolated-origin> bash scripts/check_branch_health.sh
# gh pr list exits 1; branch is three days old.
feat/M-88-gh-outage  0  1  0  —  3 сут
ok    ВИСЯК: веток с зелёным PR и без merge'а не найдено
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
exit=0

$ PATH=<check-outage-gh>:$PATH BRANCH_HEALTH_ROOT=<isolated-origin> bash scripts/check_branch_health.sh
# gh pr list returns feat/M-88-gh-outage/PR #77; gh pr checks exits 1.
feat/M-88-gh-outage  0  1  0  #77/red  3 сут
ok    ВИСЯК: веток с зелёным PR и без merge'а не найдено
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
exit=0

$ test ! -e .claude/rules/binding-requires-mechanism.md; echo $?
0
$ test -e docs/workflow/binding-requires-mechanism.md; echo $?
0

$ gh pr view 41 --json number,state,mergedAt,headRefName
PR #41 state=OPEN mergedAt=null head=docs/branch-disposition-2026-08-19
$ git ls-remote --heads origin salvage/M-59-research-dev-uncommitted feat/M-10-rebased feat/M-60-mechanisms
51c21dccb8763690f231cd932bf1b974ac9cf510  refs/heads/feat/M-10-rebased
f0e915bf834506642740b798bf5e17242d1cf73f  refs/heads/feat/M-60-mechanisms
dc646cb6c86a128777ac84626811c6473ca5a2ba  refs/heads/salvage/M-59-research-dev-uncommitted
$ git diff --exit-code origin/docs/branch-disposition-2026-08-19:docs/09-roadmap-v2.md origin/feat/M-60-mechanisms:docs/09-roadmap-v2.md
exit=0

$ git merge-base --is-ancestor origin/docs/M-66-protocol-attestation origin/main; echo $?
1
$ git merge-base --is-ancestor origin/fix/M-65-battery-recalibration origin/main; echo $?
1
$ git merge-base --is-ancestor origin/feat/M-69-window-guard origin/main; echo $?
1
$ git merge-base --is-ancestor origin/feat/M-68-depth-from-book origin/main; echo $?
1
$ git merge-base --is-ancestor origin/feat/harness-doc-integrity origin/main; echo $?
1

$ git fetch origin +refs/reserved/C-106:refs/reserved-cache/C-106
$ git log -1 --format='%H%n%s' refs/reserved-cache/C-106
a692f7e644c772c26a8aef909e71396391ff322a
reserve C-106 nous 2026-08-19T12:49:01Z Ubuntu-2404-noble-amd64-base 1930753 e4b864dd-e61c-4689-ac18-c272d5633c19
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-19T12:50Z
- Milestone: PR-42 / process-package-2026-08-19
- Статус: BLOCKED — REJECT
- HEAD: 06b74ef — docs+harness: пакет A1-A3 (приземление) + наблюдатель веток

## §B — Что я сделал
- Аудировал закоммиченный набор PR #42, а не только текст плана; выполнил прод-форму B, его пробу и мутации.
- Построил два изолированных `gh`-outage stubs; оба дали ложный PASS.
- Перепроверил все пять D-действий по R-095, A-010, PR #41/#28 и origin refs.

## §C — Артефакты / результаты
- `research/critiques/C-106-process-package-2026-08-19.md`
- Done Block: в этом файле; RED/battery текущего B exit=0, два adversarial live-source прогона ошибочно exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь REJECT C-106 на ветке docs/process-package-2026-08-19. Не приземляй A2 без
  механизма: добавь проверяемый class declaration/path router с hermetic RED+--battery,
  сузь process-polish до безопасных редакционных/статусных полей, а mixed оставь
  fail-closed. В check_branch_health.sh сделай ошибки gh pr list и недоступное состояние
  gh pr checks FAIL, не empty/red; добавь стабильные stubs обеих форм в red_branch_health.sh
  и докажи их kill-set. Исправь docs/workflow/binding-requires-mechanism.md cross-reference.
  В §D оставь D1-D3 условными до merge PR #41, U-1 и фактического удаления refs; замени D5
  на порядок из A-010 §J либо предъяви более новое решение. Закоммить и push полный набор,
  затем запроси новый critic круг.
  ```
- Push-статус: ⏳ verdict commit/push follows this audit file validation.
- Кэш: ✅ убран (по инвокации); временные adversarial fixtures не являются build-cache.

## §E — Риски / открытые вопросы
- D1–D3 необратимы только после отдельного доказательства У-1 и удаления remote refs.
- D4 остаётся блокировкой PR #28 до закрытия H-11.

=== END HANDOFF ===
