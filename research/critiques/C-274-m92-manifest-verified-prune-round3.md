<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 6ef8c672f3895e85ed73628c81ab3d3089638d69
audited_head: c0b201bf515b22dd3d4205fadfebd17a3b0b1841
verdict: REJECT
-->

# C-274 — M-92 manifest-verified prune, round 3: REJECT

## Verdict

**REJECT.** The branch is complete enough to judge and was stable at
`c0b201bf515b22dd3d4205fadfebd17a3b0b1841`, but its new audit-trail RED
oracles do not implement all of A-044 §3.1's mandatory properties. In
particular, they permit an apply run whose report is not one-row-per-name and
a second apply run whose consumed manifest is not retained as a new audit
record. Either defect defeats the required reconstructible proof for a journal
deletion; both are inside frozen F-1/F-3, not new requirements.

This is the RAW gate for a journal-deletion mechanism. The live invariants
named from `docs/fa/journal.md` are **JR-I-2** (no gaps in `seq`) and
**JR-I-11** (a segment catalogue is not silently stitched). The post-A-044
I-2bis RED work is coherent at its allotted scope: `p8`/`p9` fail against the
current per-candidate implementation, while `p2`/`p3`/`b2`/`c1`/`c4` select
the youngest candidate and preserve a contiguous suffix. No new `p*`/`b*`
requirement is imposed here.

## Subject and complete artifact set

The subject was fetched before review and again before verdict preparation:

```text
origin/feat/M-92-manifest-verified-prune
  c0b201bf515b22dd3d4205fadfebd17a3b0b1841
mandated reference c0b201b is an ancestor: yes
base (merge-base origin/main subject): 6ef8c672f3895e85ed73628c81ab3d3089638d69
```

| Required artifact | Evidence |
|---|---|
| milestone | `milestones/M-92-manifest-verified-prune.md` §3–§13, including I-2bis and the `3+` round entry |
| T2 contract / signatures | milestone §4 specifies `ColdManifest::parse` and `retention_execute_with_manifest`; `contracts/**` is untouched and no trait signature is introduced |
| RED suite | `crates/journal/tests/red_m92_manifest_prune.rs` (`p1`–`p9`, `b1`–`b2`, `c0`–`c5`) |
| acceptance gate | `scripts/verify_M-92.sh`, a `FAIL`-counting aggregator with CI-parity map and non-zero FAIL exit |
| CI-map probe | `scripts/tests/red_verify_M-92_ci_map.sh` (10 scenarios) |
| prior gate record | `C-268`, `C-270`, and binding decision `A-044` |

## Frozen-perimeter findings

### C-274-1 — O-1's one-row-per-plan-name property is not asserted (F-1 / F-3)

A-044 §3.1 O-1 requires **one report row for each plan name**, with a status,
name, and (for `kept`) a non-empty reason. M-92 §4 repeats the exact
`pruned <name>` / `kept <name> <reason>` form.

`c1` parses all report rows, but its lookup is `rows.iter().find(...)`
([red_m92_manifest_prune.rs](../../crates/journal/tests/red_m92_manifest_prune.rs):985).
It proves that at least one suitable row exists for the victim and each deleted
name ([line 993](../../crates/journal/tests/red_m92_manifest_prune.rs:993) through
[line 1008](../../crates/journal/tests/red_m92_manifest_prune.rs:1008)); it never
counts matching rows or rejects a second status row for the same plan name.

Consequently a report containing, for example, two `pruned <name>` rows (or
both `kept <name> <reason>` and `pruned <name>`) passes `c1` while violating
the fixed O-1 format. This is not the named `cp plan report` mutant—the
existing status assertions do reject that mutant—but it is still the mandatory
one-row O-1 property promised by the spec without a discriminating assertion.

### C-274-2 — O-2 does not require a new manifest record for run two (F-1 / F-3)

A-044 §3.1 O-2 requires that after the second apply run, the first run's
**records** remain byte-for-byte and the second run's records are present
separately under different names. The record set is plan, remote manifest, and
report; the manifest is the remote checksum evidence necessary to reconstruct
why a segment was deleted.

`c4` correctly preserves every first-run `(name, body)` pair
([line 1151](../../crates/journal/tests/red_m92_manifest_prune.rs:1151) through
[line 1158](../../crates/journal/tests/red_m92_manifest_prune.rs:1158)). However,
for the second run it requires new names only for `*plan.txt` and `*report.txt`
([line 1159](../../crates/journal/tests/red_m92_manifest_prune.rs:1159) through
[line 1169](../../crates/journal/tests/red_m92_manifest_prune.rs:1169)); it does
not require a new `*manifest.txt`, nor bind that run's archived manifest to its
consumed input.

An implementation can therefore preserve the first run's three records, write
new plan/report records on run two, give the binary a fresh manifest only in
the work directory, and omit the second audit-manifest record. `c4` passes,
but the second deletion has no recoverable remote-side checksum evidence. That
is precisely the A-044 O-2/O-3 audit-trail condition, not an expansion of the
frozen perimeter.

## Checks that do hold

- F-2 named-mutant coverage is otherwise discriminating by assertion reading:
  - `cp plan report` fails `c1`'s `kept`/`pruned` status checks;
  - replacing unique names with a same-name overwrite loses a first-run record
    in `c4`;
  - continuing after audit `mkdir`/`cp` failure loses the no-delete/non-zero/
    alert conditions in `c5`;
  - writing audit records after `unlink` leaves `audit-at-apply` without the
    plan/manifest snapshot in `c1`.
- F-3 is consistent for the intentionally RED task boundary: M-92 §3, §4,
  §5, §6, and §9 describe `c1`/`c4`/`c5` and the expected red task-3 state.
  The two gaps above are the specific places where §3/§4's promises outrun
  those assertions.
- The post-A-044 I-2bis check behaves as expected on the current subject:
  `p8` and `p9` are red against task 1's old per-segment continuation, proving
  the prefix mutant; this is expected until the re-opened task 1 is completed.

## Required condition for a new critic round

Architect must make the RED suite reject both faulty output shapes above, then
commit and push the revised test/spec artifact set. The re-review should judge
the new branch head. Task 3 remains withheld until this plan-time gate passes.

## Note outside the reject basis

The normal `verify_M-92.sh` invocation reached the expected M-92 red failures,
then its nested `verify_M-48.sh` remained in `cargo test -p gateway --quiet`
for more than eight minutes and was stopped as an audit-owned stalled test
process. This verdict does not diagnose that unrelated M-48 run; the isolated
M-92 suite, verifier syntax, and CI-map probe all completed and are reported
below.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-92-manifest-verified-prune
c0b201bf515b22dd3d4205fadfebd17a3b0b1841
exit=0

$ git merge-base --is-ancestor c0b201b origin/feat/M-92-manifest-verified-prune
exit=0

$ git merge-base origin/main origin/feat/M-92-manifest-verified-prune
6ef8c672f3895e85ed73628c81ab3d3089638d69
exit=0

$ cargo test -p journal --test red_m92_manifest_prune
running 17 tests
FAILED: c0_compose_topology_is_the_prune_topology
FAILED: p8_oldest_mismatch_prunes_nothing
FAILED: p9_middle_mismatch_prunes_exactly_the_older_prefix
FAILED: c1_cron_apply_verifies_against_remote_copy
FAILED: c4_audit_trail_survives_next_run
FAILED: c5_no_audit_no_delete
test result: FAILED. 11 passed; 6 failed
exit=101  # expected: p8/p9 and c0/c1/c4/c5 remain RED

$ VERIFY_M92_CI_DRY=1 bash scripts/verify_M-92.sh
PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_verify_M-92_ci_map.sh
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

$ bash -n scripts/verify_M-92.sh
exit=0

$ git diff --check 6ef8c672f3895e85ed73628c81ab3d3089638d69..HEAD
exit=0

$ bash scripts/next_artifact_id.sh C
C-274
exit=0

$ bash scripts/verify_M-92.sh
FAIL  task1-3: red_m92_manifest_prune (p1-p9, b1-b2, c0-c5) (exit=101)
... nested task3: verify_M-48.sh remained at cargo test -p gateway --quiet for >8m
stopped by critic; no final verifier exit code admitted
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC): 2026-10-02
- Milestone: M-92-manifest-verified-prune
- Status: BLOCKED — REJECT
- HEAD: c0b201b — docs(M-92): I-2bis, p8/p9, task 1 reopened [architect]

## §B — What I did
- Audited the committed artifact set, C-268/C-270, and binding A-044 against its frozen F-1/F-2/F-3 perimeter.
- Measured the expected six-test RED state and the acceptance gate's CI-map/syntax behavior.

## §C — Artifacts / results
- `research/critiques/C-274-m92-manifest-verified-prune-round3.md` — REJECT.
- Done Block: commands and exit codes above; normal verifier was stopped only after its nested M-48 test stalled.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```text
  Startup-протокол: CLAUDE.md · .claude/rules/{gates,testing,scope-guard,commit-discipline,branch-hygiene,handoff-block}.md · .claude/agents/architect.md · docs/fa/journal.md (JR-I-2, JR-I-11).
  Предмет: origin/feat/M-92-manifest-verified-prune; возьми вершину сам после fetch. Исполни C-274 REJECT только в frozen A-044 §5.2 perimeter: RED-набор обязан (1) отвергать более одной status-строки на имя плана в отчёте O-1 и (2) во втором apply-прогоне требовать новый архивный manifest record, связанный с consumed manifest, наряду с plan/report O-2/O-3. Сохрани I-2bis форму p8/p9 и не расширяй p*/b* сверх C-274. Обнови связанные обещания спеки, коммить+пушни, затем передай новый branch head для critic re-review.
  ```
- Push status: verdict commit is pushed to `origin/feat/M-92-manifest-verified-prune` in this gate response.
- Cache: cleanup follows the verdict push.

## §E — Risks / open questions
- Until both records are enforced, a second apply run can delete segments without a retained remote-manifest proof.
- Task 3 must not start before this REJECT is cleared.

=== END HANDOFF ===
