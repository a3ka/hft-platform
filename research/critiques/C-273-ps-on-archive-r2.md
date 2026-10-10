<!-- GATE-META
milestone: R-213
audited_repo: a3ka/hft-platform
audited_base: 1107c40b2abed7d3d716ab7cbb31cca55a9fadc5
audited_head: 08402cf8e90dd296af81ddacf87248440de381cf
verdict: NOTE
-->

# C-273 — C-272 section-ownership regression is closed

**Verdict: NOTE.** C-272 B-1 is closed in the committed head. A CLOSED marker
now belongs only to a section whose first M-NN header ID matches it; a foreign
marker, an orphan marker, and simultaneous CLOSED/OPEN markers all fail.

## Audited committed subject

- Branch: origin/harness/ps-on-archive
- Audited base: origin/main =
  1107c40b2abed7d3d716ab7cbb31cca55a9fadc5
- Audited head: 08402cf8e90dd296af81ddacf87248440de381cf
- Delta since C-272: dc4a026 (RED), eefdf3e (barrier), 08402cf (plan).
- This is a harness-track adversarial check: the applicable artifact set is
  plan + barrier + RED probe + CI wiring, not a product milestone/T-contract
  set (docs/workflow/harness-track.md §3–4).

## C-272 B-1 disposition

C-272 showed that the old global marker list accepted:

    ## M-68 unrelated item — CLOSED
    <!-- MS-STATE: M-69 CLOSED -->

    ## M-69 probe — still IN PROGRESS; reviewer has not closed it

The new parser records each valid marker with the owner of its enclosing
section (scripts/check_ps_on_archive.sh:94–125) and rejects a marker whose
owner differs from its ID. It separately rejects both CLOSED and OPEN for the
same valid section owner (:130–133).

Evidence:

- The new barrier and probe pass all 38 scenarios.
- Running the new 38-scenario probe against the old C-272 head barrier yields
  exactly five missed checks, K13–K17: 33 pass, 5 fail, exit 1.
- M12 (remove section ownership) yields exactly K13–K16: 34 pass, 4 fail.
- M13 (remove CLOSED/OPEN contradiction check) yields exactly K17:
  37 pass, 1 fail.
- M14 (use the last header ID rather than the first) yields exactly P12 and
  K14: 36 pass, 2 fail.
- The real origin/main registry contains 29 numbered section headers. Its
  multi-ID M-89 header begins with M-89 and later mentions M-90 and M-62;
  P12 and M14 prove that those later references do not steal ownership.

The deliberate limit is sound: the barrier does not attempt to decide general
prose such as “in progress.” The plan documents that a generic prose criterion
is a false-positive source for historical P4/P5/P6; ownership plus marker
contradiction is the narrow machine-checkable invariant.

## Non-blocking notes

- The subject remains based on 18eb01d rather than current origin/main. Before
  PR merge, rerun the required merge-preview and acceptance checks on the
  resulting merge tree.
- roadmap-sync continues to use fetch-depth 0, passes event base data to this
  barrier, and is fail-closed by All checks passed. The aggregate probe remains
  green.

## Done Block

    $ bash scripts/next_artifact_id.sh C
    C-273
    exit=0

    $ bash scripts/tests/red_ps_on_archive.sh
    сценариев: 38   pass=38   FAIL=0
    VERDICT: PASS
    new_probe_exit=0

    $ PS_BARRIER=<016358d barrier> bash scripts/tests/red_ps_on_archive.sh
    сценариев: 38   pass=33   FAIL=5
    FAIL K13
    FAIL K14
    FAIL K15
    FAIL K16
    FAIL K17
    VERDICT: FAIL
    old_barrier_against_38_case_probe_exit=1

    $ PS_BARRIER=<M12 copy> bash scripts/tests/red_ps_on_archive.sh
    сценариев: 38   pass=34   FAIL=4
    FAIL K13; FAIL K14; FAIL K15; FAIL K16
    VERDICT: FAIL
    M12_probe_exit=1

    $ PS_BARRIER=<M13 copy> bash scripts/tests/red_ps_on_archive.sh
    сценариев: 38   pass=37   FAIL=1
    FAIL K17
    VERDICT: FAIL
    M13_probe_exit=1

    $ PS_BARRIER=<M14 copy> bash scripts/tests/red_ps_on_archive.sh
    сценариев: 38   pass=36   FAIL=2
    FAIL P12; FAIL K14
    VERDICT: FAIL
    M14_probe_exit=1

    $ EVENT_NAME=push PUSH_BEFORE=016358d... bash scripts/check_ps_on_archive.sh
    OK: переезд в диапазоне 016358d8..HEAD не обнаружен; маркеров «OPEN» у вынесенных нет
    VERDICT: PASS
    exit=0

    $ EVENT_NAME=push PUSH_BEFORE=016358d... bash scripts/check_docs_freeze.sh
    exit=0
    $ EVENT_NAME=push PUSH_BEFORE=016358d... bash scripts/check_protected_artifacts.sh
    OK: защищённые артефакты целы на HEAD (016358d..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
    exit=0
    $ bash scripts/tests/red_ci_aggregate.sh
    VERDICT: PASS — сценариев: 8, расхождений: 0
    exit=0
    $ bash scripts/verify_design_claims.sh --merge-preview origin/main
    VERDICT: PASS (0 нарушений)
    exit=0
    $ git show origin/main:PROJECT-STATE.md | awk '/^## / && /M-[0-9]/ {n++} END{print n+0}'
    29
    $ git diff --check 016358d..08402cf
    exit=0

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-10-02T00:00Z
- Milestone: R-213 / ps-on-archive harness track, circle 2
- Статус: DONE — NOTE
- HEAD: 08402cf — docs(plans): ps-on-archive — проверки 5–6…

## §B — Что я сделал

- Reproduced the C-272 fixture against both the repaired and old barriers.
- Executed the complete 38-case probe plus documented M12, M13, and M14 copies.
- Verified 29 real numbered registry headers, process locks, aggregate wiring,
  and merge-preview design claims.

## §C — Артефакты / результаты

- research/critiques/C-273-ps-on-archive-r2.md
- C-272 B-1 is closed; this NOTE is advisory and does not block the harness track.

## §D — Следующий агент + инвокация

- **Следующий агент:** architect
- **Paste-ready промпт:** C-273 is NOTE: C-272 B-1 is closed on
  origin/harness/ps-on-archive at 08402cf. Keep the C-273 verdict in the
  branch, rebase or construct the required merge preview with current main,
  rerun the harness acceptance and merge-preview checks on that tree, then
  open the track PR. No product milestone/T-contract work is required.
- Push-статус: ⏸ verdict commit pending at this point in the document; it is
  committed and pushed immediately after file creation.
- Кэш: ✅ this critic worktree created no target cache.

## §E — Риски / открытые вопросы

- The unmerged subject is behind current main. The merge-tree check remains
  required before PR creation.
- The marker still records reviewer judgment rather than proving its truth or
  author; that is the named, acceptable boundary of this harness mechanism.

=== END HANDOFF ===
