<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: 2ec2eaec211ba3540284a2a71934d59d0225bc9a
audited_head: 40fde15f2493eba3d3aa4d43aeff93ebd93a8761
verdict: NOTE
-->

# C-317 — M-99 restore-drill: RP gate

## Verdict: NOTE

The committed plan-time artifact set is admissible for implementation.  No
REJECT finding was found in the narrow A-056 §5(2) subject:

- M-99 §4, row 11 is a direct tightening of the M-74 T2 state contract and
  reader/wrapper contract: a reader result is successful only with `rc=0`, a
  64-character lowercase-hex digest, numeric `events_read >= 1`, and
  `intra_segment_gap=true`.  Every other `rc=0` result becomes persisted
  `ok=0`, empty digest, `reason=reader-protocol-violation`, and exit `7`.
  `7` is declared not to overlap reader `0/1/4/5/6` or wrapper `1/4/5`.
- The RED probe has one intended red: current head is `35/36`, with **only**
  `RP` failing.  A disposable copy of the production wrapper implementing that
  exact row is `36/36`; the six A-056 §4.2 mutations each return red with the
  named symptom below.  The audited worktree was never modified and the copy
  was restored cleanly.
- The committed set contains the T2/reader/wrapper contracts (M-74), the
  M-99 milestone row and Task 2c, RED suite `red_restore_drill.sh`, and real
  verifier `verify_M-99.sh` (`set -uo pipefail`, explicit aggregation and
  non-zero failure exit).  It ties the acceptance work to the RAW requirement
  (`OPS-I-3`, `OPS-I-10`) and journal readability (`JR-I-6`).
- `03b66a19..40fde15` changes exactly the permitted milestone, RED probe, and
  verifier.  There is no expansion into network delivery, aliases, §4(7–10),
  or the threshold question resolved by A-055/A-056.

### Mutation evidence

| A-056 §4.2 mutant in disposable wrapper | Required named red symptom | Result |
|---|---|---|
| empty/malformed digest reported as `ok=1` | `объявлен УСПЕХ` | red, `RP` |
| foreign failure reason | `причина ... вместо reader-protocol-violation — диагноз сочинён` | red, `RP` |
| protocol violation exits `0` | `код 0 вместо 7` | red, `RP` |
| malformed digest copied into failed state | `отпечаток ... проброшен из неверного вывода при ok=0` | red, `RP` |
| `events_read=0` accepted | `events_read=0: объявлен УСПЕХ` | red, `RP` |
| state write omitted | `состояние НЕ ЗАПИСАНО` | red, `RP` |

`RP-M` also accepts the reference wrapper and rejects its nine self-mutants.

### Note

`bash scripts/verify_M-99.sh` exits `1` at the plan head as designed: Task 1
is RED (`RP`), Task 2c correctly observes that production has not yet gained
the protocol gate, and CI parity repeats the same RED result.  Its fourth
failure is `cargo audit`; R-266 N-1 records the same rustls advisories with a
`Cargo.lock` identical to `origin/main`.  This is an existing integration
condition, not a defect in the narrow artifact set, but it remains visible to
the implementer/PR owner before an all-green implementation claim.

## Done Block

Raw command outcomes (the plan-head RED result is intentional):

```text
$ git fetch origin && git rev-parse origin/feat/M-99-restore-drill-automation
40fde15f2493eba3d3aa4d43aeff93ebd93a8761
exit=0

$ git merge-base origin/main 40fde15f2493eba3d3aa4d43aeff93ebd93a8761
2ec2eaec211ba3540284a2a71934d59d0225bc9a
exit=0

$ git diff --stat 03b66a19..40fde15f2493eba3d3aa4d43aeff93ebd93a8761
 milestones/M-99-restore-drill-automation.md |  26 ++++-
 scripts/tests/red_restore_drill.sh          | 152 +++++++++++++++++++++++++++-
 scripts/verify_M-99.sh                      |   7 ++
 3 files changed, 182 insertions(+), 3 deletions(-)
exit=0

$ git diff --check 03b66a19..40fde15f2493eba3d3aa4d43aeff93ebd93a8761
exit=0

$ bash -n scripts/tests/red_restore_drill.sh scripts/verify_M-99.sh
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  RP ... malformed reader output ... -> ok=1
VERDICT: FAIL (1 из 36)
exit=1

$ [disposable exact-§4-row-11 wrapper] bash scripts/tests/red_restore_drill.sh
VERDICT: PASS (36/36)
exit=0

$ [each of six A-056 §4.2 disposable mutations] bash scripts/tests/red_restore_drill.sh
VERDICT: FAIL (1 из 36)    # every run; the sole failed group is RP, with the named symptom above
exit=1

$ git -C /tmp/hft-critic-m99-sketch-rsAOK5 restore --source=HEAD -- deploy/bin/journal-restore-drill-cron.sh
$ git -C /tmp/hft-critic-m99-sketch-rsAOK5 status --short
exit=0

$ bash scripts/verify_M-99.sh
FAIL task-1: RED RP (1/36)
FAIL task-2c: production wrapper has no reader-protocol-violation gate yet
FAIL ci-parity: bash scripts/tests/red_restore_drill.sh (exit=1)
FAIL ci-parity: cargo audit (exit=1; known R-266 N-1 / main-equivalent lockfile)
VERDICT: FAIL (провалов: 4)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
merge_preview_exit=0

$ bash scripts/reserve_artifact_id.sh C
C-317
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2ec2eaec211ba3540284a2a71934d59d0225bc9a bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 2ec2eae..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```
