<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: 18a68daa3369be224214ec191bc0f1d2625e4ff7
audited_head: 2ed50a9bbf2115118f048ce57f9b831c10d09894
verdict: REJECT
-->

# C-318 — M-99 task 2d: `checked` still accepts stale restore bytes — REJECT

## Verdict

**REJECT.** The committed RED/verify/milestone set correctly exposes the `pipefail`
selection failure, silent successful delivery, and permanently suppressed stderr.  Its
temporary conforming wrapper passes 41/41 and each stipulated mutant turns the named
oracle red.  However, the class is not closed: a failed cleanup of the prior
`RESTORE` directory lets stale files satisfy the proposed `-f` delivery check after a
transport that returns success without copying any current-run bytes.  The wrapper
then writes `ok=1, checked=3` and runs the reader.

This is the same observable violation as TD-260: an index is credited as checked even
though the current selection did not deliver it.  It is fail-open for **OPS-I-3**
(the cold copy is periodically restored and read), and contradicts the inherited T2
meaning of `checked` plus `A-055` §5.2: incomplete delivery must be
`transport-failed`/5, not success.

Founder’s 2026-10-15 decision to route this round through critic rather than an
arbiter is accepted as given.  This finding does not re-litigate `A-056` §2.3; it is
an executable hole in the artifact set now under audit.

## Finding B-1 — stale `RESTORE` makes the new delivery proof vacuous

`deploy/bin/journal-restore-drill-cron.sh:179` runs
`rm -rf "${RESTORE}" && mkdir -p "${RESTORE}"` without handling a false result.
The script does not use `set -e`, so it proceeds to selection and fetch.  The planned
task-2d rule only asks whether a selected file exists in `RESTORE` after a successful
`fetch_one`; it cannot distinguish a file delivered in this run from one left by an
unsuccessful cleanup.

Reproduction against the 41/41 temporary task-2d sketch:

1. Create a valid old restore containing the three selected filenames.
2. Make `rm` return 1, so the old restore remains.
3. Make `cp` return 0 without copying any selected file.
4. Use a success-form reader stub.

Observed result:

```text
wrapper_rc=0
state={"ok":1,"ts_wall_ms":1792101305000,"checked":3,"events_read":7,"digest":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","reason":""}
stale_files=segment-00000001.jrnl segment-00000000.jrnl segment-00000002.jrnl
```

`PS` removes `restore` before each scenario and `DV` starts from an empty restore.
Therefore neither scenario exercises cleanup failure plus stale local bytes; `PS-M` and
`DV-M` only mutate their judges, so they do not close that setup path.  The assertion
in §4 line 12 says “after selection”, which requires current-run delivery rather than
mere historical presence.

### Condition to clear B-1

Add a RED scenario with all of the following in the same execution:

- cleanup of a non-empty prior `RESTORE` fails or leaves it intact;
- the selected current-run transport reports exit 0 without materialising one selected
  file; and
- the wrapper fails closed before the reader: `ok=0`, empty digest,
  `reason=transport-failed`, exit 5, `checked < 3`, and a positive guard proves the
  reader was not called.

The production implementation must make that scenario GREEN without weakening PS,
DV, DV-E, their judge mutations, or the stated §7 transport restrictions.  Re-submit
the changed milestone, RED suite, verify script, and any necessary task-2d acceptance
checks for a fresh critic round.

## What is sound in this round

- The range changes only the architect-owned artifact trio:
  `scripts/tests/red_restore_drill.sh`, `scripts/verify_M-99.sh`, and
  `milestones/M-99-restore-drill-automation.md`; no T1 contract, trait signature, or
  implementation is silently changed.  The applicable T2 contract, reader signature,
  and `checked` semantics are inherited from M-74.
- §4 line 12 is a legitimate consequence of the inherited T2 contract and `A-055`
  §5.2, not a new code/reason literal.  It is consistent with §4 line 11: the latter
  judges a reader’s `rc=0` protocol, while line 12 rejects incomplete transport before
  the reader is judged.
- The current production wrapper gives the intended RED evidence:
  `PS`, `DV`, and `DV-E` fail.  `PF` passed 10/10 in this run; that is expressly
  permitted because §11 declares it probabilistic rather than a RED oracle.
- A detached temporary task-2d sketch (`grep … <<<"${LISTING}"`, per-index
  want/got/file existence, and a scoped lock-file stderr redirection) passes 41/41.
  Required mutations are effective: restored membership pipeline → PS/PF fail;
  removed delivery proof, `want=0` only, and `got` without file check → DV/DV-E fail;
  restored global stderr redirect → DV-E fail.  PS repeated three times loses index 1
  deterministically (`rc=0`, `checked=3` each time).
- `scripts/verify_M-99.sh` has a real FAIL counter and a task-2d acceptance step; its
  CI-parity map dry-run covers 65/65 steps.  The design-claims merge preview passes.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-99-td260-sampling
2ed50a9bbf2115118f048ce57f9b831c10d09894
$ git merge-base origin/main HEAD
18a68daa3369be224214ec191bc0f1d2625e4ff7
$ git diff --name-status origin/main..HEAD
M       milestones/M-99-restore-drill-automation.md
M       scripts/tests/red_restore_drill.sh
M       scripts/verify_M-99.sh
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  PS ... 00000001(0 из 1) ... ok=1, checked=3, rc=0
PASS  PF прод-форма листинга (1 624 строки): 10 из 10 прогонов — полный состав, ok=1, checked=3
FAIL  DV ... весь-индекс ... ok=1, rc=0, checked=3
FAIL  DV-E stderr обёртки на отказе доставки пуст
VERDICT: FAIL (3 из 41)
exit=1

$ bash /tmp/hft-critic-m99-candidate/scripts/tests/red_restore_drill.sh
VERDICT: PASS (41/41) — копия читается прод-читателем, исходы различимы
exit=0

$ mutation: restore printf | grep -qx
FAIL  PS ... ok=0, checked=0, rc=5
FAIL  PF ... ok=0, checked=0, rc=5
VERDICT: FAIL (2 из 41)
exit=1

$ mutation: remove delivery proof / check only want=0 / increment got without -f
FAIL  DV ... ok=1, rc=0, checked=3
FAIL  DV-E stderr обёртки на отказе доставки пуст
VERDICT: FAIL (2 из 41)
exit=1 (each)

$ mutation: restore 'exec 9>… 2>/dev/null'
FAIL  DV-E stderr обёртки на отказе доставки пуст
VERDICT: FAIL (1 из 41)
exit=1

$ /tmp/hft-critic-m99-ps-repeat.sh
PS-repeat run=1 rc=0 first_index_present=0 checked=3
PS-repeat run=2 rc=0 first_index_present=0 checked=3
PS-repeat run=3 rc=0 first_index_present=0 checked=3
ps_repeat_exit=0

$ /tmp/hft-critic-m99-stale-restore-repro.sh
wrapper_rc=0
state={"ok":1,"ts_wall_ms":1792101305000,"checked":3,"events_read":7,"digest":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","reason":""}
stale_files=segment-00000001.jrnl segment-00000000.jrnl segment-00000002.jrnl
repro_exit=0

$ bash -n scripts/tests/red_restore_drill.sh scripts/verify_M-99.sh
exit=0
$ VERIFY_M99_CI_DRY=1 bash scripts/verify_M-99.sh
PASS: ci-parity: учтено шагов 65 из 65 (исполнено 58, исключено по карте 6)
VERDICT: PASS
exit=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git merge-base origin/main HEAD) bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=$(git merge-base origin/main HEAD) bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 18a68da..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```
