<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: f43c1191a05e6f05dac3ef10efe6b001f80cacc9
audited_head: 99eb10dfa75220d09d9bb15e31d0373c60eee3a1
verdict: ESCALATE
-->

# C-313 — M-99 restore-drill automation, round 4: ESCALATE

## Verdict

**ESCALATE.** Gates §0 p.2 makes a fresh arbiter mandatory after the third round
for one subject.  M-99 §14 says that this trigger fired formally but was not
convened because there was no dispute.  That is not an exception present in the
rule.  Architect and critic are parties, not the adjudicator, so the next action
is an independent arbitration, not a fourth repair loop.

The fresh inspection also found two executable holes in the newly repaired
oracles.  They must be considered by that arbiter:

1. RW proves an allow-listed remote command transcript and unchanged local cold
   fixture, but not that the remote endpoint is the host/account in COLD.
2. CL proves only that a busy-lock invocation leaves the state and restore tree
   unchanged.  It does not require the specified exit 0 or SKIP log line.

## Artifact set and C-311 closure check

The audited branch tip is 99eb10dfa75220d09d9bb15e31d0373c60eee3a1.  The
round-3 diff contains the inherited M-74 T2 contract and M-99 §5 declared trait
signatures, RED tests, the M-99 verifier, and the milestone.  No implementation
source or contract path changed.

| C-311 condition | Result |
|---|---|
| RW whole-transcript whitelist plus cold immutability | The whitelist in red_restore_drill.sh:1074-1103 rejects every extra command, and the before/after tree check is real. RW-M accepts its good form and rejects 11 listed mutants. It remains incomplete for endpoint identity (F-1). |
| CL busy wrapper lock, bare cron, and CL-M | A cron-side flock is rejected; the no-wrapper-lock mutant is caught. CL remains incomplete for the specified successful skip result (F-2). |
| v5 exclusivity over all services and v5m | Closed. judge_state_mounts counts all mounts seeing the state directory, requires exactly recorder/same path/ro, rejects restore exposure and extra recorder sources. v5m accepts the valid form and rejects all 13 listed mutants. |
| retention-work / §7 volume rule | Closed. The v5m good fixture includes retention-work in retention and accepts it. Its coverage relation rejects state parents and descendants, including /var/lib and the state-file descendant; it rejects an extra /var/lib/hft source in recorder. This resolves the M-92 coexistence contradiction without weakening the state rule. |
| subject lock and mutation table | Closed. 691f99ca and b070e04d both carry an ALLOW-SUBJECT-CHANGE explanation; check_gate_meta is green. M-99 §8 records the required two-operation/no-rename/cron-without-flock/new-state-path mutation setup and its round-3 results. |

The stated expected RED baseline is reproduced: the six product gaps T, RT, RW,
CR, CL, and IM fail; RW-M and CL-M pass; the ops suite has v3/v4/v5 red and
v5m green.  That is appropriate for a plan artifact, not a reason to waive its
oracle precision.

## New executable findings

### F-1 — RW does not bind the remote endpoint to COLD

M-99 §4 p.7 requires the two remote operations to use the user@host parsed from
COLD.  The ssh stub records its complete argv in ssh-argv.txt
(red_restore_drill.sh:976-999), but RW judges only ssh-cmd.txt
(red_restore_drill.sh:1074-1103).  The stub discards the host before recording
the command (line 989).  Consequently a wrapper can hard-code another host and
still have its locally executed ls cold and rsync --server --sender commands
accepted; the cold tree also stays unchanged.

I built the minimal §8-style temporary candidate (two-operation seam, no remote
rename, bare cron, recorder state mount) and changed only its remote host.  The
candidate completed successfully against the local transport stub:

    wrapper_exit=0 state_ok=1
    expected_host=expected-user@expected-host.invalid
    observed_hosts=wrong-host.invalid wrong-user@wrong-host.invalid
    ssh-cmd:
    ls cold
    rsync --server --sender -logDtpre.iLsfxCIvu . cold/segment-00000003.jrnl.zst

Thus F-1 passes the present RW judgment while contacting a host other than the
production endpoint.  The repair, not made in this audit, needs an executable
assertion that both the listing invocation and rsync's ssh invocation resolve to
the exact account/host parsed from COLD, in addition to the existing command
allowlist and immutable-tree check.  It must include this wrong-host mutant.

### F-2 — CL accepts an error exit and does not require SKIP

M-99 §4 p.8 promises that a busy wrapper lock leaves no trace, exits 0, and
writes SKIP.  cl_run captures the exit status at lines 1213-1216 but its boolean
return at lines 1218-1220 compares only the state and restore snapshots.  The
caller at lines 1227-1235 never inspects drill.rc or drill-wrapper.log.

In the same isolated minimal candidate, changing only the busy-lock branch from
exit 0 to exit 1 left the two snapshots unchanged:

    busy_lock_wrapper_exit=1
    state_and_restore_unchanged=yes

That mutant satisfies the current CL predicate despite violating the stated
cron contract.  The RED scenario must assert rc == 0 and a deliberate SKIP log
record while retaining the unchanged-state/tree checks.  CL-M must then cover
both a missing wrapper lock and the erroneous busy-lock result.

## Gates §9 and authority

* **(a) Design claims:** bash scripts/verify_design_claims.sh --merge-preview
  origin/main passed.  The design-link and dead-link checks report no dangling
  references.  This does not prove F-1/F-2 because they are execution-oracle
  gaps.
* **(b) Authority:** the audited round changes only
  crates/ops/tests/red_m99_restore_drill_watch.rs,
  milestones/M-99-restore-drill-automation.md,
  scripts/tests/red_restore_drill.sh, and scripts/verify_M-99.sh.  These are
  architect-owned M-99 RED/gate/milestone paths.  The two changed gate paths
  are explicitly released by the two ALLOW-SUBJECT-CHANGE commit bodies.
* **(c) Linkage:** the milestone’s §7 narrow-volume rule is consistent with
  M-92 retention-work, as the passing v5m valid fixture demonstrates.  The
  merge-preview design check, protected-artifact check, docs-freeze check, and
  whitespace diff check are clean.

OPS-I-3 is the live invariant at stake: the cold copy is periodically restored
and read.  JR-I-6 remains the reason the current reader, rather than a file
checksum, must read the restored history.

## Arbitration questions

1. Does gates §0 p.2 allow the §14 “no disagreement” rationale to bypass its
   unconditional three-round trigger?  This critic finds no such exception.
2. Must the plan gate reject a remote success against a wrong COLD endpoint?
   This critic finds yes: otherwise the allegedly proven unique copy may never
   be contacted.
3. Is an error exit without SKIP a valid “tick skipped without trace” under
   §4 p.8?  This critic finds no.

No repair is proposed or committed by the critic.  The arbiter should issue the
independent decision before an architect changes the subject again.

## Done Block

    $ git rev-parse HEAD
    99eb10dfa75220d09d9bb15e31d0373c60eee3a1
    $ git rev-parse origin/feat/M-99-restore-drill-automation
    99eb10dfa75220d09d9bb15e31d0373c60eee3a1
    $ git merge-base origin/main HEAD
    f43c1191a05e6f05dac3ef10efe6b001f80cacc9
    exit=0

    $ git diff --stat d511820c..HEAD -- milestones/ scripts/ crates/ops/tests/
     crates/ops/tests/red_m99_restore_drill_watch.rs | 272 +++++++++++++++++++++---
     milestones/M-99-restore-drill-automation.md     |  26 ++-
     scripts/tests/red_restore_drill.sh              | 176 +++++++++++++--
     scripts/verify_M-99.sh                          |   9 +-
     4 files changed, 431 insertions(+), 52 deletions(-)
    $ git diff --check d511820c..HEAD
    exit=0

    $ bash scripts/tests/red_restore_drill.sh
    FAIL  CL строка cron'а держит СВОЙ flock («flock -n /var/lock/hft-journal-restore-drill.lock /root/hft-platform/deploy/bin/…») — замок обязан быть один и в обёртке (M-99 §4 п.8)
    PASS  CL-M обёртка без замка ПОЙМАНА: при занятом замке она прогнала drill (состояние переписано)
    FAIL  IM читатель по умолчанию зовётся из «hft-recorder», а прод поднимает recorder из «hft-platform-recorder:local» — такого образа на сервере нет (R-263 Б-3)
    VERDICT: FAIL (6 из 31)
    exit=1

    $ cargo test -p ops --test red_m99_restore_drill_watch
    test v3_default_state_path_matches_wrapper_contract ... FAILED
    test v4_fields_and_priority_follow_spec_table ... FAILED
    test v5_state_dir_is_mounted_into_recorder_at_same_path_read_only ... FAILED
    test v5m_state_mount_judge_rejects_every_mutant ... ok
    test result: FAILED. 7 passed; 3 failed
    exit=101

    $ bash scripts/verify_design_claims.sh --merge-preview origin/main
    PASS  [2-ПОКРЫТИЕ] §22: OPS-I — заявлено=11, в оракулах=10 — подтверждено замером (loose=10)
    PASS  [3-ССЫЛКИ] все 8 ссылок DESIGN.md §N указывают на существующие разделы
    PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 661 ссылок вида docs/*.md указывают на существующие файлы
    VERDICT: PASS (0 нарушений)
    exit=0

    $ EVENT_NAME=pull_request PR_BASE_SHA="$(git merge-base origin/main HEAD)" bash scripts/check_gate_meta.sh
    OK: subject-lock — C-306 ... ALLOW-SUBJECT-CHANGE найден в 691f99ca
    OK: subject-lock — C-306 ... ALLOW-SUBJECT-CHANGE найден в b070e04d
    VERDICT: PASS (0)
    exit=0

    $ independent RW wrong-host mutant
    wrapper_exit=0 state_ok=1
    expected_host=expected-user@expected-host.invalid
    observed_hosts=wrong-host.invalid wrong-user@wrong-host.invalid
    exit=0

    $ independent CL busy-exit mutant
    busy_lock_wrapper_exit=1
    state_and_restore_unchanged=yes
    exit=0

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC): 2026-10-13
- Milestone: M-99-restore-drill-automation
- Verdict: ESCALATE
- Critique: C-313-M-99-restore-drill-automation-round-4.md
- Audited head: 99eb10dfa75220d09d9bb15e31d0373c60eee3a1

## §B — Completed
- Audited the committed full artifact set, C-311 closure evidence, §7 volume
  wording, all-services v5/v5m, subject lock, and the gates §9 checks.
- Reproduced independent RW wrong-endpoint and CL nonzero-busy-exit mutants.

## §C — Blocking state
- Do not start an architect self-fix loop.  Gates §0 p.2 requires a fresh
  arbiter; §14's no-disagreement explanation is not a rule exception.

## §D — Next agent: architect

Paste-ready prompt:

    M-99 has C-313 ESCALATE at 99eb10dfa75220d09d9bb15e31d0373c60eee3a1.
    Do not repair it yourself.  Dispatch a fresh arbiter under gates §0 p.2
    with M-99, R-263, C-306, C-311, C-313 and the subject branch.  Ask the
    arbiter to decide the §14 bypass claim and to rule on F-1 (RW must bind
    both transport operations to the exact COLD host/account) and F-2 (busy
    lock must leave no trace, exit 0, and log SKIP).  Preserve the M-92
    retention-work exception while deciding the state-volume boundary.
