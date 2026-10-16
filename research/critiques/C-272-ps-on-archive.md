<!-- GATE-META
milestone: R-213
audited_repo: a3ka/hft-platform
audited_base: ef8a6f93aa0f04656e44e2b7483f8b504f79f974
audited_head: 45cd125930ee938b620ed363e7f2b03387a9296f
verdict: REJECT
-->

# C-272 — PROJECT-STATE close-out marker must belong to its milestone section

**Verdict: REJECT.** The new guard accepts a registry that says M-69 is in
progress when an unrelated M-68 section contains an M-69 CLOSED marker. That
is the false-green class the guard is intended to prevent.

## Audited committed subject

- Branch: origin/harness/ps-on-archive
- Base supplied for this audit: origin/main =
  ef8a6f93aa0f04656e44e2b7483f8b504f79f974
- Audited head: 45cd125930ee938b620ed363e7f2b03387a9296f
- Commit chain: a33fb85 → 34f1fa3 → ee2db4e → 45cd125
- Harness-track artifact set: plan, barrier, RED probe, and roadmap-sync CI
  wiring. The full milestone/T-contract/trait/verify_M-* set is not required
  for this non-production harness-track subject (harness-track.md §3–4).

## Blocking finding

### B-1 — CLOSED is global instead of belonging to M-NN’s own registry section

scripts/check_ps_on_archive.sh:94–107 collects every syntactically valid
marker outside a fenced code block into one global list. has_marker at :107,
and the archive trigger at :128, only ask whether that list contains the
matching ID. They do not associate the marker with the ## M-NN section. The
separate prose check at :133–148 rejects only the legacy literal
close-out за architect, so a different open-status statement also escapes it.

I committed an isolated two-commit fixture with this registry at the archive
event:

    ## M-68 unrelated item — CLOSED
    <!-- MS-STATE: M-69 CLOSED -->

    ## M-69 probe — still IN PROGRESS; reviewer has not closed it

The event moves milestones/M-69-probe.md to docs/archive/M-69-probe.md and has
no working-tree changes. The production barrier returns 0:

    OK    M-69: переезд, PROJECT-STATE.md несёт маркер CLOSED

    OK: переездов 1, каждый сопровождён маркером закрытия в PROJECT-STATE.md
    VERDICT: PASS
    exit=0

This contradicts the asserted invariant in
docs/plans/ps-on-archive-2026-10-02.md §3.1: the marker is supposed to record
the close-out judgment in the milestone’s registry section. It is also the
untested form of that plan’s own S3, “any CLOSED marker anywhere” (:135–139).
Existing K5–K8 use either the legacy phrase, a fenced marker, or a different
marker ID, so they do not exercise this valid-ID/wrong-section lie.

**Condition to lift B-1:** make the barrier and RED suite require a valid M-NN
CLOSED record in M-NN’s own non-code registry section and reject a
contradictory status in that section. Add the committed adversarial fixture
above, plus mutation proof, to the probe. The new test must be red against the
audited head and green only after the implementation correction.

## Checks that passed

- The real probe ran **30/30 PASS**: P1–P9, K1–K12, F1–F3, R0, and all five
  historical replays.
- An isolated S1 exit-0 copy makes the probe fail exactly where expected:
  20 failing expectations (K1–K12, F1–F3, and five replays), overall exit 1.
  Thus the probe detects that broad stub; it simply has no case for B-1.
- A single git-mv commit relocating M-69 and M-70 with only M-69 marked closed
  fails for M-70, so the loop does not stop after the first archived milestone.
- CI wiring is correct: roadmap-sync uses fetch-depth 0, passes the
  pull_request base SHA to the barrier, and status-check both needs and
  fail-closes on roadmap-sync (.github/workflows/ci.yml:47–77,625–635).
  red_ci_aggregate.sh passes all eight aggregate scenarios.
- check_docs_freeze.sh, check_protected_artifacts.sh, check_roadmap_sync.sh,
  check_ps_on_archive.sh (against actual ancestor 18eb01dc..45cd1259), and
  verify_design_claims.sh --merge-preview origin/main all exit 0. The sole
  process-layer commit, 45cd125, carries FOUNDER-APPROVED.

## Done Block

    $ git rev-parse origin/main origin/harness/ps-on-archive
    ef8a6f93aa0f04656e44e2b7483f8b504f79f974
    45cd125930ee938b620ed363e7f2b03387a9296f

    $ bash scripts/next_artifact_id.sh C
    C-272
    exit=0

    $ bash scripts/tests/red_ps_on_archive.sh
    сценариев: 30   pass=30   FAIL=0
    VERDICT: PASS
    exit=0

    $ PS_BARRIER=/tmp/hft-critic-ps-mut.../always-pass.sh bash scripts/tests/red_ps_on_archive.sh
    сценариев: 30   pass=10   FAIL=20
    VERDICT: FAIL
    always_pass_mutation_probe_exit=1

    $ EVENT_NAME=push PUSH_BEFORE=<fixture-base> bash scripts/check_ps_on_archive.sh
    OK    M-69: переезд, PROJECT-STATE.md несёт маркер CLOSED
    OK: переездов 1, каждый сопровождён маркером закрытия в PROJECT-STATE.md
    VERDICT: PASS
    exit=0                              # B-1: false green, expected FAIL

    $ EVENT_NAME=push PUSH_BEFORE=<two-archive-base> bash scripts/check_ps_on_archive.sh
    OK    M-69: переезд, PROJECT-STATE.md несёт маркер CLOSED
    FAIL  M-70 уехал в архив (docs/archive/M-70-probe.md), а PROJECT-STATE.md НЕ несёт «<!-- MS-STATE: M-70 CLOSED -->»
    нарушений: 1
    VERDICT: FAIL
    two_archive_one_marker_exit=1

    $ bash scripts/tests/red_ci_aggregate.sh
    VERDICT: PASS — сценариев: 8, расхождений: 0
    exit=0

    $ EVENT_NAME=push PUSH_BEFORE=18eb01d... bash scripts/check_docs_freeze.sh
    exit=0
    $ EVENT_NAME=push PUSH_BEFORE=18eb01d... bash scripts/check_protected_artifacts.sh
    OK: защищённые артефакты целы на HEAD (18eb01d..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
    exit=0
    $ EVENT_NAME=push PUSH_BEFORE=18eb01d... bash scripts/check_roadmap_sync.sh
    VERDICT: PASS
    exit=0
    $ EVENT_NAME=push PUSH_BEFORE=18eb01d... bash scripts/check_ps_on_archive.sh
    VERDICT: PASS
    exit=0
    $ bash scripts/verify_design_claims.sh --merge-preview origin/main
    VERDICT: PASS (0 нарушений)
    exit=0

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-10-02T00:00Z
- Milestone: R-213 / ps-on-archive harness track
- Статус: BLOCKED — REJECT
- HEAD: 45cd125 — docs(workflow): §Close-out — закрытость пишется reviewer'ом сразу…

## §B — Что я сделал

- Audited the committed harness-track subject, including the barrier, probe,
  process-layer edit, reviewer profile, historic R-213 finding, and CI aggregate.
- Executed the 30-scenario probe, five-incident replay, S1 stub check,
  two-archive case, process locks, aggregate probe, and merge-preview claims gate.
- Reproduced B-1 in an isolated committed fixture with the production barrier.

## §C — Артефакты / результаты

- research/critiques/C-272-ps-on-archive.md
- Done Block above; B-1 is a real false green, so no approval is available.

## §D — Следующий агент + инвокация

- **Следующий агент:** architect
- **Paste-ready промпт:** Repair REJECT C-272 on harness/ps-on-archive. The
  current check_ps_on_archive.sh accepts a valid M-69 CLOSED marker from an
  M-68 section and passes an M-69 section saying it is still IN PROGRESS. First
  add a RED probe with that exact committed fixture and mutation proof; it must
  fail against 45cd125. Then change only architect-owned harness/process
  artifacts so CLOSED belongs to M-NN’s own non-code section and a
  contradiction in that section fails. Preserve P4–P7 (quoted prose, inline
  code, and neighbouring IDs stay green). Run the complete probe, commit/push
  the green artifact set, then invoke a fresh critic on the new commit chain.
- Push-статус: ⏸ verdict commit pending at this point in the document; it is
  committed and pushed immediately after file creation.
- Кэш: ✅ кэш этого critic worktree не создавался.

## §E — Риски / открытые вопросы

- origin/main (ef8a6f93) is not an ancestor of audited head; rebase or
  merge-preview validation remains required before merge. This is not B-1’s basis.
- CI wiring itself is not a finding: it has fetch-depth 0 and the aggregate
  correctly consumes roadmap-sync.

=== END HANDOFF ===
