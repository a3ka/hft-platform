<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: f43c1191a05e6f05dac3ef10efe6b001f80cacc9
audited_head: cfbdd166ab9b16ec4d5465f2d5f085ced23e5377
verdict: REJECT
-->

# C-314 — M-99 restore-drill automation, round 5: REJECT

## Verdict

**REJECT.** The audited subject tip is
`cfbdd166ab9b16ec4d5465f2d5f085ced23e5377`; it has not moved.  After the
required fresh fetch, however, `origin/main` is
`2ec2eaec…` and the required design-claim merge preview fails with a real
three-way conflict in `docs/DESIGN.md`.  The M-99 change raises the OPS-I
oracle count while the now-merged M-100 change raises the adjacent VB-I count;
the conflict drops one of those claims unless it is resolved deliberately.

`gates.md §8` makes a merge-preview disagreement a blocker, not a note.  This
is an **other-class mergeability/document-integrity subject** under A-055 §6
p.4: it has its own count.  It is not an A-055 §4 p.7–10 positive-oracle hole,
does not invoke the `STOP, subject founder` exception, and does not authorize
a sixth M-99 plan-time repair round.  Architect must rebase/resolve this new
subject, retaining both invariant-count claims, before dispatching engine-dev.

## A-055 closure executed, not accepted from the table

I audited the committed full plan set: inherited T2 and reader contracts,
declared ops trait signatures, the RED shell and Rust tests, the real
FAIL-counting `scripts/verify_M-99.sh`, CI wiring, and M-99.  The probe is
unconditionally wired into CI `build-test`.

An isolated, uncommitted candidate with exactly the specified sketch passed
the complete shell battery: two-operation remote seam; no rename of the
delivered remote copy; bare cron (no `flock`, `JOURNAL_DRILL_REMOTE`, or
`JOURNAL_DRILL_SSH`); wrapper-owned nonblocking lock; new state path; recorder
state-directory mount; image-matched reader with `--network none` and the
same restore path mounted read-only.  It passed **33/33**.

Each required mutation was then applied to that candidate and the complete
battery re-run.  Each produced exactly its named failure and a nonzero result:

| A-055 condition exercised | Mutation | Observed catching oracle |
|---|---|---|
| §3 endpoint identity | `HOSTPART=wrong-user@wrong-host.invalid` | `RW`: endpoint differs from COLD |
| §5.2 fetch refusal | `fetch_one … || true` | `TF`: transport failed but drill reports `ok=1, rc=0, digest` |
| §5.2 diagnosis | fetch refusal recorded as `reader-rc-5` | `TF`: reason differs from `transport-failed` |
| §5.1 listing refusal | listing refusal recorded as `cold-empty`, exit 7 | `T`: reason/code differ from `transport-failed`/5 |
| §4 p.8 skip/tick | named mutations | `CL-M` accepts its valid form and rejects all 8 by the named (a)--(e)/timeout sign |
| §3 command and endpoint | named mutations | `RW-M` accepts its valid form and rejects all 11 command and all 10 endpoint mutations |

The delivered RED top is therefore correctly red for missing implementation:
`T`, `TF`, `RT`, `RW`, `CR`, `CL`, and `IM`; `T-M`, `RW-M`, and `CL-M` are
green.  The ops suite executes ten tests and is red only at v3/v4/v5 (seven
pass, three fail), as the milestone declares.  This is intended RED, not an
oracle vacuum.

## Other-class finding and scope

`git merge-tree` identifies the conflict at `docs/DESIGN.md` invariant table:
the M-99 side says `OPS-I | 11 | 10`, while current main says
`VB-I | 12 | 10`.  These adjacent edits conflict in the merge result.  A
branch-only design check passed before `origin/main` advanced; the mandatory
fresh merge-preview is authoritative and fails.  Consequently dangling-link
and claim checking on the merged tree cannot be claimed as completed.

This audit did not reject a local transport stub as proof of network delivery,
did not revisit A-032 alias forms, and did not rehear A-055 §14.  No new
forbidden-positive-oracle or promise-without-oracle hole was found in §4
p.7--10.

Advisory only: M-99 §9 task-2 prose still calls the image `hft-recorder`,
whereas §4 p.9 and the executable `IM` oracle correctly require the
`image:` value of `recorder` (currently `hft-platform-recorder:local`).  That
sentence predates the round-4 diff and the executable oracle is unambiguous;
correct the wording mechanically while resolving the mergeability subject.

## Gates §9, authority, and artifact scope

* **§9(a), merged claims:** FAIL as shown below.  A conflicted merge has no
  trustworthy claim or dangling-link result.
* **§9(b), authority:** the round-4 diff is limited to
  `milestones/M-99-restore-drill-automation.md` and
  `scripts/tests/red_restore_drill.sh`.  The full branch plan artifacts are
  architect-owned RED/gate/milestone paths; no `crates/contracts/**` change is
  present.
* **§9(c), linkage:** `OPS-I-3` is the operative invariant.  The task-4 cron
  check uses `crontab -n`, and the CI step is present.  Linkage on the merged
  document is blocked until the conflict is resolved.

The aggregate verifier's M-99 task checks correctly counted the expected RED
points before its all-workspace CI-parity tail was intentionally terminated on
an unrelated long-running `red_m89_warm_resume_seek` test.  That interrupted
tail is not used as evidence of a passing verifier; the targeted M-99 commands
above completed independently.

## Done Block

    $ git fetch origin && git rev-parse HEAD && git rev-parse origin/feat/M-99-restore-drill-automation
    cfbdd166ab9b16ec4d5465f2d5f085ced23e5377
    cfbdd166ab9b16ec4d5465f2d5f085ced23e5377
    $ git merge-base origin/main HEAD
    f43c1191a05e6f05dac3ef10efe6b001f80cacc9
    $ git rev-parse origin/main
    2ec2eaec211ba3540284a2a71934d59d0225bc9a
    exit=0

    $ bash scripts/tests/red_restore_drill.sh
    FAIL T; FAIL TF; FAIL RT; FAIL RW; FAIL CR; FAIL CL; FAIL IM
    PASS T-M (7 mutants); PASS RW-M (11 command + 10 endpoint mutants); PASS CL-M (8 mutants)
    VERDICT: FAIL (7 из 33)
    exit=1

    $ cargo test -p ops --test red_m99_restore_drill_watch
    test v3_default_state_path_matches_wrapper_contract ... FAILED
    test v4_fields_and_priority_follow_spec_table ... FAILED
    test v5_state_dir_is_mounted_into_recorder_at_same_path_read_only ... FAILED
    test result: FAILED. 7 passed; 3 failed
    exit=101

    $ (cd /tmp/m99-candidate.cptPf5 && CARGO_TARGET_DIR=/tmp/hft-critic-m99r5/target bash scripts/tests/red_restore_drill.sh)
    VERDICT: PASS (33/33)
    candidate_probe_exit=0

    $ candidate mutation: wrong endpoint
    FAIL RW: точка wrong-user@wrong-host.invalid вместо точки из COLD
    VERDICT: FAIL (1 из 33)
    exit=1

    $ candidate mutation: fetch_one || true
    FAIL TF: транспорт отказал, но drill ok=1 rc=0 digest
    VERDICT: FAIL (1 из 33)
    exit=1

    $ candidate mutation: refused fetch -> reader-rc-5
    FAIL TF: причина reader-rc-5 вместо transport-failed
    VERDICT: FAIL (1 из 33)
    exit=1

    $ candidate mutation: refused listing -> cold-empty / 7
    FAIL T: причина cold-empty вместо transport-failed; rc=7
    VERDICT: FAIL (1 из 33)
    exit=1

    $ bash scripts/verify_design_claims.sh --merge-preview origin/main
    FAIL  [SETUP] --merge-preview: слияние base-ref 'origin/main' + HEAD (cfbdd166ab9b16ec4d5465f2d5f085ced23e5377) КОНФЛИКТУЕТ
    VERDICT: FAIL (1 нарушений)
    design_claims_exit=1

    $ git merge-tree "$(git merge-base origin/main HEAD)" origin/main HEAD | rg '<<<<<<<|======='
    <<<<<<< .our
    =======
    >>>>>>> .their
    exit=0

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-13T20:54Z
- Milestone: M-99-restore-drill-automation
- Статус: BLOCKED
- HEAD: cfbdd166 — feat/M-99-restore-drill-automation

## §B — Что я сделал
- Executed all A-055 §3, §4, §5.1, §5.2, and §5.5 candidate mutations; the corrected candidate passed 33/33 and every named mutation was caught.
- Re-fetched the subject and current main, then ran the required merge-preview; it fails on the real `docs/DESIGN.md` conflict described above.

## §C — Артефакты / результаты
- `research/critiques/C-314-M-99-restore-drill-automation-round-5.md`
- Done Block: targeted RED suite exit=1; ops RED suite exit=101; candidate exit=0; four candidate mutants exit=1; merge-preview exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-99 round 5 is REJECT in C-314 at audited subject cfbdd166ab9b16ec4d5465f2d5f085ced23e5377.  A-055 §3/§4/§5.1/§5.2/§5.5 closures were executed and passed; do not open a sixth repair round for them.  The blocker is a new mergeability/document-integrity subject: after fetching origin/main=2ec2eaec, `bash scripts/verify_design_claims.sh --merge-preview origin/main` fails because M-99's OPS-I count and merged M-100's adjacent VB-I count conflict in docs/DESIGN.md.  Rebase/resolve the conflict retaining both claims, correct the stale M-99 §9 task-2 shorthand `hft-recorder` to the recorder service image wording without changing §4 p.9 semantics, rerun the merge-preview and the targeted M-99 RED suites, commit/push the new subject, then obtain its own critic gate.  Do not dispatch engine-dev while the merge-preview is red.
  ```

## §E — Риски / открытые вопросы
- The current branch cannot be merged as audited; resolving only one table row would silently lose either M-99 OPS-I-3 or M-100 VB-I evidence.
- This is not the A-055 §6 p.4 founder-stop class; it is a distinct subject with its own counter.

=== END HANDOFF ===
