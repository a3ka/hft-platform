# C-270 — M-92 manifest-verified prune, round 2: ESCALATE

<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 6ef8c672f3895e85ed73628c81ab3d3089638d69
audited_head: f8bce5de289313a72f6c150e0eabae1e695c36a3
verdict: ESCALATE
-->

## Verdict

**ESCALATE.** This is the second round on the same audit-trail cause as C-268-3; it is therefore not a third architect/critic loop. Per gates.md §0, an independent arbiter with fresh context must decide the adequacy of the required proof before this RAW journal-deletion plan goes to development.

C-268 R1 and R2 are closed in the committed subject. c0 rejects a read-only /journal, a missing read-only /ckpt, a missing /work, a remaining /cold, or a wrong entrypoint. c2 reads the actual cron file without a mode override; c3 mutates its RETENTION_MODE=apply line while no switch file exists. The C-268 R3 repair remains incomplete.

## Subject and artifact completeness

The fetched subject was origin/feat/M-92-manifest-verified-prune at f8bce5de289313a72f6c150e0eabae1e695c36a3; the final fetch reported the same head.

| Required artifact | Evidence |
|---|---|
| milestone | milestones/M-92-manifest-verified-prune.md, updated at f8bce5d |
| T2 API shape / trait signatures | §4 specifies ColdManifest::parse and retention_execute_with_manifest; no contracts/** T1 or trait change |
| RED suite | crates/journal/tests/red_m92_manifest_prune.rs; intended compile-RED |
| acceptance gate | scripts/verify_M-92.sh, explicit FAIL-counting aggregator |
| CI-map anti-plaque probe | scripts/tests/red_verify_M-92_ci_map.sh |

This is a RAW gate for journal deletion. Relevant live FA invariants are JR-I-2 (no sequence gaps) and JR-I-11 (no silent stitching of a non-monotonic segment catalogue).

## Repeated finding

### C-270-1 — remote-manifest evidence is not bound to every planned/deleted name

Milestone §4/I-7 requires three audit records: the candidate plan, the **remote-side manifest as obtained**, and the result report. C-268 required all three records to bind the selected names and retain the mismatch evidence.

c1 now requires every plan name in *plan.txt (red_m92_manifest_prune.rs:857-861) and every successful deletion in *report.txt (:871-875). But it examines *manifest.txt only for the deliberately corrupted victim (:863-866). It never requires the remote checksum line for each remaining planned/deleted name.

A broken implementation can write all candidates to plan.txt, write only the corrupted victim's remote checksum to manifest.txt, delete all other candidates, and list them in report.txt. The one mismatch is reconstructible, but the remote checksum evidence that authorised every actual deletion is gone. A report naming a deletion is not the offsite checksum proof that authorised it.

This is C-268-3's audit-trail defect, narrowed from an empty directory to a partial manifest. The minimum discriminating oracle would require every plan name in manifest.txt paired with the remote-fixture checksum, then classify the same name in the report. Whether this is the only adequate proof shape is for the arbiter, not an architect self-fix loop.

## Boundary C

П-031 delegates only the operator's checked enablement action to architect. M-92 does not allow code or deployment to create the host mode-switch file, and c3 makes a cron-file RETENTION_MODE=apply mutation non-authorising without that file. No new Boundary-C bypass was found.

## Done Block

    $ git fetch origin --prune && git rev-parse origin/feat/M-92-manifest-verified-prune
    f8bce5de289313a72f6c150e0eabae1e695c36a3
    exit=0

    $ cargo test -p journal --test red_m92_manifest_prune
    error[E0432]: unresolved import journal::ColdManifest
    error[E0425]: cannot find function retention_execute_with_manifest in crate journal
    error[E0425]: cannot find function retention_execute_with_manifest in crate journal
    error[E0425]: cannot find function retention_execute_with_manifest in crate journal
    error[E0425]: cannot find function retention_execute_with_manifest in crate journal
    error[E0425]: cannot find function retention_execute_with_manifest in crate journal
    error[E0425]: cannot find function retention_execute_with_manifest in crate journal
    error: could not compile journal (test red_m92_manifest_prune) due to 7 previous errors
    exit=101  # expected compile-RED; only the two new API names are absent

    $ bash scripts/tests/red_verify_M-92_ci_map.sh
    PASS  честный ci.yml: всё учтено (exit=0)
    PASS  новый шаг с expressions outside map ⇒ FAIL (exit=1)
    PASS  исключение без шага ⇒ карта протухла (exit=1)
    PASS  строка дописана в блок базы ⇒ FAIL (exit=1)
    PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
    PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
    PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
    PASS  check_review_fa + дописка ⇒ исполняется, не SKIP (exit=0)
    PASS  run: |- ⇒ EXEC
    PASS  run: > ⇒ FAIL
    VERDICT: PASS — 10 scenarios
    exit=0

    $ bash -n scripts/verify_M-92.sh
    exit=0

    $ VERIFY_M92_CI_DRY=1 bash scripts/verify_M-92.sh
    PASS  ci-parity: учтено шагов 55 из 55 (исполнено 48, исключено по карте 6)
    VERDICT: PASS
    exit=0

    $ git diff --check origin/main..HEAD
    exit=0

    $ git status --short
    <empty before verdict creation>
    exit=0

=== HANDOFF: CRITIC → ARBITER ===

## §A — Metadata
- Date (UTC): 2026-10-02
- Milestone: M-92-manifest-verified-prune
- Status: BLOCKED — ESCALATE
- HEAD: f8bce5d — docs(M-92): спека круг 2 — I-7 ... [architect]

## §B — What I did
- Audited the committed M-92 set, C-268, the Compose/cron path, Boundary-C delegation, RED suite, and acceptance gate.
- Reproduced the intentional seven-error compile RED and 10-world CI-map probe.

## §C — Artifacts / results
- research/critiques/C-270-m92-manifest-verified-prune-round2.md — ESCALATE.

## §D — Next agent + invocation
- **Next agent:** arbiter (strong model, fresh context).
- **Paste-ready prompt:**
    Arbitrate M-92 round 2 with fresh context. Read origin/feat/M-92-manifest-verified-prune, C-268, C-270, milestone §4/I-7/§8, docs/fa/journal.md (JR-I-2, JR-I-11), and П-023/П-028/П-031. Decide whether c1 is sufficient when it checks every plan/report name but only the corrupted victim's remote manifest line; measure the partial-manifest mutant. This repeats C-268-3, so issue the binding arbitration decision rather than a third critic loop.
- Push status: pending this verdict commit and push to feat/M-92-manifest-verified-prune.
- Cache: target/ will be removed after the commit/push.

## §E — Risks / open questions
- A partial manifest cannot reconstruct offsite proof for the removed segments.
- П-031 does not delegate changing which segments are retained/deleted.

=== END HANDOFF ===
