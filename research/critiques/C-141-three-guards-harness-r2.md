<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: ffb738f8054dbd11198a27e8218ec38695144357
verdict: REJECT
-->

# C-141 — three guards harness, round 2: REJECT

## Scope

- Route: `docs/workflow/harness-track.md` §3 adversary. Merge requires every item
  of §5, including this committed verdict artifact (§5.3).
- Subject: PR #91, `harness/three-guards-2026-08-24`, audited head
  `ffb738f8054dbd11198a27e8218ec38695144357`.
- Base: `c00b7267c2ea4ce938a3a3f0bd96be7359aea200`; the only merge preview used was
  `refs/pull/91/merge` = `f979cee57b932c2de16db132d6ed6e547cc59e13`.
- Round-2 delta over the rejected `94504c7`: `955bc60`, `0d6067a`, `c161c98`,
  `ffb738f` plus this audit.

This is a harness-track subject, so the ordinary milestone T-contract / trait /
separate RED / verify / milestone-file set is not applicable. The required equivalent
is present: guard, executable adversarial probe, mutation battery, and CI/barrier
wiring. The rejection below is a false-green in that probe.

## Verdict: REJECT

### R2-1 — the new dead-PID distinction is not pinned

`scripts/gc_worktrees.sh:105` distinguishes a process which disappeared between the
numeric `/proc` glob and `readlink` from an unreadable live PID. The audited guard
does the right thing: the former reaches `WOULD-REMOVE`; an enumerable own PID whose
`cwd` is unreadable reaches `KEEP`.

But `red_gc_live_cwd.sh` contains neither the disappearing-PID fixture nor a mutant
for that branch. A valid external mutant that replaces the dead-PID `continue` with
`echo "${pid}?"; found=1; continue` passed all 18 scenarios and all six declared
kill-sets. Against the same executed race fixture, it produced:

~~~text
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )
VERDICT: GC DRY-RUN
~~~

This is a false green: a future regression can turn a dead PID into a universal
holder, disabling both ordinary GC and reclaim despite no live holder. It is precisely
the property claimed by `0d6067a`, and it must be protected by the harness rather than
by this transcript.

Condition to clear:

1. Add an executable RED case in the existing `GC_PROC_ROOT` seam where a numeric PID
   is enumerated and disappears before its `cwd` is read; it must permit the otherwise
   eligible dry-run removal/reclaim.
2. Add a mutation that restores this false holder outcome. The RED case must fail
   against it, and the declared kill-set must include that case.
3. Re-run the full harness battery and a clean fixture. A new critic round audits the
   committed head, not a pasted run.

## Checks that passed

### C-140-1 and C-140-2 are closed in the audited implementation

- An own unreadable PID holds, while a PID deliberately removed between glob and
  `readlink` does not. The former is fail-closed; the latter is not miscounted as
  “uninspected”.
- Root-alias identity is now `dev:inode`: the target and alias cwd both measured
  `2306:24672940`, produced `KEEP ... (PID: 4343~)`, and a same-prefix foreign path
  produced `WOULD-REMOVE`.
- The third C-140-2 alternative was actually taken: a subdirectory under a bind alias
  is a named limit, not a silently claimed protection. An ordinary symlink subdirectory
  resolves in `/proc/<pid>/cwd` to the physical target; an attempted non-root
  `unshare -Urnm` bind-mount construction was blocked by this runner's
  `uid_map: Operation not permitted`. Thus this audit did not construct the stated
  real-loss case here. The limit remains explicit in `gc_worktrees.sh:46-50`, which
  satisfies the narrowing option of C-140-2 rather than pretending it is covered.

### `/proc` probe shape and positive control

- Retention cases L2/L3/L5/L8 use a symlink to the live scenario PID under the probe
  root, so `readlink` and `stat` still consume kernel `/proc/<pid>` data; only unrelated
  runner PIDs are excluded.
- Deletion cases use `benign_proc`. That removes the in-battery real-`/proc` deletion
  positive control, but an independent actual-`/proc` dry run in an eligible disposable
  repository produced `WOULD-REMOVE`, while naming 581 unreadable foreign PIDs. No
  false red was reproduced at this head. This is an observed limitation of the test
  topology, not the R2 blocker.

### Process-layer correction

`docs/04-workflow.md` now names `ALLOW-SUBJECT-CHANGE`, matching
`scripts/check_gate_meta.sh:393`; `ARCHIVED-VERDICT` remains the separate
pre-normative-artifact exception. The full subject-lock battery and the actual PR merge
ref both passed. `94504c7` carries a qualifying `FOUNDER-APPROVED` body for its
`docs/04-workflow.md` change.

## Done Block

~~~text
$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
сценариев исполнено: 18  ok: 18  FAIL: 0
G1 G3 G4 G5 G6 G7 — все kill-set'ы совпали
каталогов red-gclive-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ own-unreadable PID / disappeared-between-glob-and-readlink fixture
-- unreadable-own-pid --
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )
VERDICT: GC DRY-RUN
-- disappeared-pid-between-glob-and-readlink --
WOULD-REMOVE  wt (чист, на origin, смержен)
VERDICT: GC DRY-RUN
exit=0

$ bind-shaped root alias / same-prefix foreign fixture
wt-dev-inode=2306:24672940
alias-cwd-dev-inode=2306:24672940
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4343~ )
WOULD-REMOVE  wt (чист, на origin, смержен)
exit=0

$ dead-PID holder mutant outside the declared six
mutant_syntax_exit=0
dead_pid_mutant_battery_exit=0
сценариев исполнено: 18  ok: 18  FAIL: 0
G1 G3 G4 G5 G6 G7 — все kill-set'ы совпали
VERDICT: PASS
-- mutant: dead PID is misclassified as holder --
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )
VERDICT: GC DRY-RUN
exit=0  # false-green reproduction; this is the blocker

$ actual /proc positive deletion control
      (wt: 581 чужих процессов не опрошено — cwd нечитаем; гарантия сторожа их НЕ покрывает)
WOULD-REMOVE  wt (чист, на origin, смержен)
VERDICT: GC DRY-RUN
exit=0

$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_reclaim_args.sh
VERDICT: PASS (13/13 сценариев)
exit=0

$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_branch_health.sh --battery
сценариев исполнено: 25  ok: 25  FAIL: 0
каталогов red-brhealth-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gate_meta.sh
VERDICT: PASS (48/48) — вердикт привязан к предмету, лок держит, отсутствие наблюдаемо
exit=0

$ cd /tmp/hft-critic-guards-r2-merge
$ EVENT_NAME=pull_request PR_BASE_SHA=c00b7267c2ea4ce938a3a3f0bd96be7359aea200 GITHUB_SHA=f979cee57b932c2de16db132d6ed6e547cc59e13 bash scripts/check_docs_freeze.sh
exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=c00b7267c2ea4ce938a3a3f0bd96be7359aea200 GITHUB_SHA=f979cee57b932c2de16db132d6ed6e547cc59e13 bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 0, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ find /home/nous/.cache/paxio-tmp -maxdepth 1 -type d -name 'red-gclive-*' | wc -l
0
$ find /home/nous/.cache/paxio-tmp -maxdepth 1 -type d -name 'red-brhealth-*' | wc -l
0
~~~

## Handoff

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC, ISO-8601): 2026-08-25T09:00Z
- Milestone: M-60a
- Status: BLOCKED — R2-1 false-green in the harness probe
- HEAD audited: ffb738f8054dbd11198a27e8218ec38695144357

## §B — What I did
- Executed the repaired unreadable-PID, dead-PID, root-alias, same-prefix, actual-`/proc`,
  declared-mutant, independent-mutant, cleanup, process-lock, and merge-ref checks.
- Found one unpinned branch: dead PID treated as a holder.

## §C — Artifact / result
- `research/critiques/C-141-three-guards-harness-r2.md`
- Verdict: REJECT; the raw command output and exit codes are above.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  For M-60a / PR #91 at the committed C-141 REJECT, repair only R2-1 in
  scripts/gc_worktrees.sh and scripts/tests/red_gc_live_cwd.sh. Add a deterministic
  executable RED case for a numeric GC_PROC_ROOT PID that disappears after enumeration
  and before cwd observation; demonstrate that its targeted false-holder mutant fails,
  then run the full 18+ scenario battery and all declared kill-sets. Commit and push the
  new head to harness/three-guards-2026-08-24. Do not edit C-141; request a fresh critic
  round over the committed head.
  ```
- Push status: pending this verdict commit by critic.
- Build cache: not created in this critic worktree.

## §E — Risks / open questions
- The named bind-alias-subdirectory limit is not coverage. It was not constructible in
  this runner without a permitted user namespace, but it is explicitly excluded rather
  than a silent broad guarantee.

=== END HANDOFF ===
