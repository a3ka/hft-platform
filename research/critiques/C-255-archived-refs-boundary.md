<!-- GATE-META
milestone: R-203
audited_repo: a3ka/hft-platform
audited_base: 29b0a93a17df5d499e607be05d9a51afbea4eb8e
audited_head: ac9488213013f2459164c571e5cb9e72fd14536e
verdict: REJECT
-->

# C-255 — archived-refs boundary

## Verdict

**REJECT.** The boundary, suffixed-gate recognition, baseline reduction, and the real-tree gate work at the audited head. The RED probe is nevertheless not an adequate harness oracle: it does not catch removal of its declared fail-closed `id` guard, and an uncommitted fixture can make scenario 4 pass for the wrong failure cause. This violates the harness-track mutation and per-scenario setup-guard requirements.

## Audited set and scope

- Subject ref: `origin/harness/archived-refs-boundary`.
- `ac94882` is an ancestor of the audited head; base is `29b0a93`.
- Diff contains only `scripts/check_archived_refs.sh`, `scripts/tests/red_archived_refs.sh`, and `scripts/lib/archived_refs_baseline.txt`.
- The `LIVE` pathspec block is byte-equivalent at base and head; no new exclusion was added.
- This is harness work: it does not execute in the production data or money path.

## Blocking findings

### B-1 — removal of the declared fail-closed `id` guard is invisible to the probe

- **Where:** `scripts/check_archived_refs.sh:108`; missing adversarial scenario in `scripts/tests/red_archived_refs.sh:48-217`.
- **Reproduction:** in an isolated copy, delete only the guard at line 108. `git diff --numstat` proves `0 2 scripts/check_archived_refs.sh`; the unmodified probe remains PASS: 13 scenarios and 0 discrepancies, exit 0.
- **Impact:** the mutation-control requirement says neutralising every check must fail its scenario. Here the check can be removed without an observable regression. The guard is unreachable under the current narrow selector, but it is explicitly relied on as fail-closed defence against parser or selector drift; the suite neither proves that defence nor establishes it as dead code.
- **Approval condition:** the probe must turn red when this guard is neutralised, or the artifact selection and guard contract must be made such that the guard is demonstrably unreachable and removed by the owner. Until then its fail-closed claim is unverified.

### B-2 — scenario 4 has no setup guard and accepts the wrong red failure

- **Where:** `scripts/tests/red_archived_refs.sh:82-89`; the shared `guard` exists at lines 42-45 but scenarios 1-7 do not invoke it.
- **Reproduction:** in an isolated copy, delete only `commit_all $R3` at original line 85. `git diff --numstat` is `0 1 scripts/tests/red_archived_refs.sh`; the complete probe still returns PASS, 13 scenarios and 0 discrepancies, exit 0.
- **Impact:** scenario 4 then passes because `git grep` cannot see the uncommitted fixture and the barrier fails for a different reason, not because the baseline is stale. This directly contradicts the file claim that every scenario has a tracked-fixture setup guard and violates `testing.md` gate-integrity property 3.
- **Approval condition:** each negative scenario must establish the specific tracked setup and distinguish its intended failure mode from another fail-closed failure.

## Confirmed behavior

- The positive probe passes: all 13 scenarios, 0 discrepancies, exit 0. It contains both sides of the boundary check: a live `M-99b` and `verify_M-99b.sh` are accepted, while links to archived `M-99` and `verify_M-99.sh` are rejected.
- PR-form real-tree barrier passes: 62 archived artifacts, no new dangling references, 21 current baseline entries, exit 0.
- Four required mutations are caught: search boundary removal gives 2 discrepancies; suffix-recognition removal gives 1; blind search gives 6; baseline-boundary removal gives 1. Each exits 1.
- The previous baseline had 9 `milestones/M-60` lines. The 8 removed rows each point only to live `M-60a`, `M-60b`, or `M-60c` paths; every boundary search returns no hit. The retained `scripts/tests/red_reserve_id.sh|milestones/M-60` is a literal one-repo fixture name `milestones/M-60-x.md`.
- With an empty baseline, exactly 21 detected `(file, old-path)` pairs exist; the current baseline has exactly 21, with 0 uncovered and 0 extra pairs.

## Identifier-form assessment

- No top-level archived `M-60A` or `M-60-2` artifact exists. The artifact-ID grammar allows only an optional lowercase letter, so uppercase is an unimplemented invalid-form limit, not a live defect.
- A name shaped `M-60-2-...md` is selected and maps to ID `M-60`; under the grammar its `-2` is a filename suffix, not an identifier suffix. A future grammar change would require a corresponding fail-closed selector/parser change and RED case.
- `docs/archive/M-10-obi-killscreen-retired-2026-07` is a directory, not a top-level artifact file. It contains the legacy gate inside; there are no live `M-10` old-path hits. This is not evidence that the current top-level selector lost a dangling reference.

## RED-first order

`ba9918c` (barrier/baseline) precedes `ac94882` (probe). In the harness track, commit order is deliberately not the rule; the required evidence is a red probe against broken implementation. The four caught mutations provide that evidence, but B-1 and B-2 leave it incomplete, so the exception does not cure the REJECT.

## Done Block

```text
$ git fetch origin && git rev-parse origin/harness/archived-refs-boundary
ac9488213013f2459164c571e5cb9e72fd14536e
$ git merge-base origin/main origin/harness/archived-refs-boundary
29b0a93a17df5d499e607be05d9a51afbea4eb8e
$ git merge-base --is-ancestor ac94882 origin/harness/archived-refs-boundary; echo exit=$?
exit=0

$ bash scripts/tests/red_archived_refs.sh; echo exit=$?
ok    известный случай: висячая ссылка в живом коде ⇒ отказ
ok    после починки пути: принятие
ok    новое нарушение при непустой базовой линии ⇒ отказ (амнистия не распространяется)
ok    протухшая строка базовой линии ⇒ отказ (список обязан сокращаться)
ok    отсутствие базовой линии ⇒ отказ (fail-closed)
ok    пустой архив ⇒ отказ (барьер не судит пустоту)
ok    исключение оснастки точечное: чужой файл под scripts/** по-прежнему судится
ok    граница спеки: ссылка на живой M-99b при вынесенном M-99 ⇒ принятие
ok    позитивный контроль: ссылка на вынесенный M-99 ⇒ отказ
ok    граница гейта: ссылка на живой verify_M-99b.sh при вынесенном verify_M-99.sh ⇒ принятие
ok    позитивный контроль гейта: ссылка на вынесенный verify_M-99.sh ⇒ отказ
ok    суффикс гейта: verify_M-98-umbrella-*.sh считается артефактом, ссылка на scripts/verify_M-98.sh ⇒ отказ
ok    ложная амнистия: строка на файл только с живым M-99b ⇒ отказ
VERDICT: PASS — сценариев: 13, расхождений: 0
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=29b0a93a17df5d499e607be05d9a51afbea4eb8e bash scripts/check_archived_refs.sh; echo exit=$?
PASS  вынесенных артефактов найдено: 62
PASS  НОВЫХ висячих ссылок нет (проверено 62 артефактов)
PASS  базовая линия актуальна (унаследованных ссылок: 21)
VERDICT: PASS
exit=0

$ mutation a_search_boundary; git diff --numstat; bash scripts/tests/red_archived_refs.sh; echo exit=$?
1  1  scripts/check_archived_refs.sh
VERDICT: FAIL — сценариев: 13, расхождений: 2
exit=1
$ mutation b_suffix_filter; git diff --numstat; bash scripts/tests/red_archived_refs.sh; echo exit=$?
1  1  scripts/check_archived_refs.sh
VERDICT: FAIL — сценариев: 13, расхождений: 1
exit=1
$ mutation c_blind_search; git diff --numstat; bash scripts/tests/red_archived_refs.sh; echo exit=$?
1  1  scripts/check_archived_refs.sh
VERDICT: FAIL — сценариев: 13, расхождений: 6
exit=1
$ mutation d_baseline_boundary; git diff --numstat; bash scripts/tests/red_archived_refs.sh; echo exit=$?
1  1  scripts/check_archived_refs.sh
VERDICT: FAIL — сценариев: 13, расхождений: 1
exit=1
$ mutation e_id_guard; git diff --numstat; bash scripts/tests/red_archived_refs.sh; echo exit=$?
0  2  scripts/check_archived_refs.sh
VERDICT: PASS — сценариев: 13, расхождений: 0
exit=0

$ mutation remove_case4_commit; git diff --numstat; bash scripts/tests/red_archived_refs.sh; echo exit=$?
0  1  scripts/tests/red_archived_refs.sh
VERDICT: PASS — сценариев: 13, расхождений: 0
exit=0

$ baseline coverage with empty baseline
detected_empty_baseline=21
current_baseline=21
uncovered_detected_pairs=0
extra_baseline_pairs=0

$ fixture directories before/after probe
before=26923
after=26923
fixture_dir_delta=0

$ git diff --name-status 29b0a93..ac94882
M scripts/check_archived_refs.sh
M scripts/lib/archived_refs_baseline.txt
M scripts/tests/red_archived_refs.sh
$ compare LIVE pathspec block at audited base and head
PASS  LIVE pathspec block unchanged
exit=0
```

## Handoff

REJECT → architect. Re-run the fresh-context adversary after both approval conditions are met; do not dispatch implementation or merge this harness revision.
