<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: 94504c7904f19351128de63f10bba07e61c2466c
verdict: REJECT
-->

# C-140 — three guards harness: REJECT

## Scope and audited artifact set

- Route: harness-track, §3 adversary; merge condition is §5.3, a verdict file committed
  to the subject branch.
- Subject: PR #91, branch harness/three-guards-2026-08-24, head
  94504c7904f19351128de63f10bba07e61c2466c, against merge-base
  c00b7267c2ea4ce938a3a3f0bd96be7359aea200. Merge preview executed only at
  refs/pull/91/merge = f979cee57b932c2de16db132d6ed6e547cc59e13.
- Audited committed set: scripts/gc_worktrees.sh; scripts/check_branch_health.sh;
  scripts/tests/red_gc_live_cwd.sh; scripts/tests/red_branch_health.sh;
  .github/workflows/ci.yml; docs/04-workflow.md.
- This is a harness-track change, so the usual T-contract / trait / milestone artifact
  set is not applicable. Its required equivalent is present: changed guards, executable
  RED probes, their CI wiring, and the narrow Close-out norm correction.

## Verdict: REJECT

### C-140-1 — individual unreadable /proc/PID/cwd fails open

The new destructive guard promises fail-closed behaviour when /proc is unavailable or
unreadable. Its implementation does not meet that promise for an individual PID:

- scripts/gc_worktrees.sh:72 turns a failed readlink of PID/cwd into continue;
- scripts/gc_worktrees.sh:76 treats merely seeing any numeric PID directory as sufficient
  evidence that /proc is readable;
- therefore, if a candidate PID directory is enumerable but its cwd link cannot be read,
  holder_pids returns “nobody holds it” and the caller reaches removal.

This is an executed false-green, not a theoretical concern. A disposable repository made
the candidate worktree clean, fully published, and merged to origin/main. Its injected
GC_PROC_ROOT contained a numeric PID directory whose cwd was permission-denied. The actual
guard reported WOULD-REMOVE for that worktree:

~~~text
unreadable-proc-readlink=denied
WOULD-REMOVE  wt (чист, на origin, смержен)
VERDICT: GC DRY-RUN
~~~

That contradicts the stated safety rule: inability to inspect a potentially relevant
process is “unknown”, not proof that no process holds the tree. The authored L6/L7 cover
missing and empty proc roots, but do not cover an enumerable PID whose cwd cannot be read.
The battery consequently does not pin this branch.

Condition to clear:

1. The implementation must not convert an unreadable candidate cwd into “no holder”.
2. Add a RED scenario using the existing GC_PROC_ROOT seam with an enumerable numeric PID
   and unreadable cwd; it must preserve both the worktree and target/reclaim path.
3. Add a mutation that restores the current skip path and demonstrate that this scenario
   fails against it.

### C-140-2 — bind-shaped cwd path is not recognised

The guard is a lexical comparison at scripts/gc_worktrees.sh:74. A supported GC_PROC_ROOT
adversarial run supplied a cwd path at a bind-mount alias of the candidate worktree:

~~~text
simulated-bind-cwd=/home/nous/.cache/paxio-tmp/critic-gc-paths.LjrNRb/bind
WOULD-REMOVE  wt (чист, на origin, смержен)
VERDICT: GC DRY-RUN
~~~

The same run proves that a non-matching path string is treated as no holder. A real
mount --bind could not be created in this environment (mount: must be superuser; unshare
also failed to write uid_map), so this is a seam-driven execution of the exact observed
/proc representation rather than an actual privileged mount. It remains blocking because
the guard claims “no live process holds cwd in the tree”, while the implementation has no
way to distinguish a bind alias of that tree from an unrelated path.

Condition to clear: either make the promised safety property hold for bind aliases and
pin it with a real mount-capable test environment, or explicitly narrow and name the
guarantee and its operational limit. A silently unsafe broad claim is not acceptable on
the GC path.

## Checks that passed

- Symlink cwd resolves to the actual worktree path and was retained:

~~~text
symlink-cwd=/home/nous/.cache/paxio-tmp/critic-gc-paths.LjrNRb/wt
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 2554820 )
VERDICT: GC DRY-RUN
~~~

- The same-prefix but different tree did not falsely retain the candidate:

~~~text
prefix-cwd=/home/nous/.cache/paxio-tmp/critic-gc-paths.LjrNRb/wt-foo
WOULD-REMOVE  wt (чист, на origin, смержен)
VERDICT: GC DRY-RUN
~~~

- The complete authored GC probe and its four-mutant battery passed with 15 scenarios;
  it cleaned red-gclive fixtures from the required TMPDIR.
- The complete branch-health probe and its five-mutant battery passed with 25 scenarios;
  it cleaned red-brhealth fixtures from the required TMPDIR.
- An independent S23-only mutant, not in the authored battery, restored the old
  unqualified aggregate phrase. It failed exactly S23-fraza-nazvana-porogom.
- The Close-out token correction is operationally correct: a range containing a passing
  verdict and scripts/verify_M-99.sh failed with ARCHIVED-VERDICT and passed with
  ALLOW-SUBJECT-CHANGE.
- On the PR merge preview, docs-freeze passed. The CI model confirms docs-freeze is both
  a status-check need and a fail-closed condition. branch-health, including the
  red_branch_health probe, remains deliberately outside that blocking aggregate; that
  residual risk is named by the PR and is not this verdict’s finding.
- The FOUNDER-APPROVED body is present on the sole subject commit. The docs diff corrects
  the token and documents the mechanism; it does not introduce a new normative unit.

## Done Block

~~~text
$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
сценариев исполнено: 15  ok: 15  FAIL: 0
каталогов red-gclive-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_branch_health.sh --battery
сценариев исполнено: 25  ok: 25  FAIL: 0
каталогов red-brhealth-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ independent S23 phrase mutant
s23-mutant-exit=1
FAIL       S23-фраза-названа-порогом нет «старше 1 сут не найдено»
сценариев исполнено: 25  ok: 24  FAIL: 1
каталогов red-brhealth-* до: 0, после уборки: 0
VERDICT: FAIL (сценариев: 1, мутантов с разошедшимся kill-set: 0)
exit=0  # the audit harness expected and asserted the mutant failure

$ subject-lock token fixture
wrong-token-exit=1
FAIL  research/critiques/C-999-subject-lock.md: subject-lock — после проходного вердикта (APPROVE) тронут класс «гейт»: scripts/verify_M-99.sh
      выход из лока — строка «ALLOW-SUBJECT-CHANGE: <причина>» в теле коммита диапазона
VERDICT: FAIL (1) — вердикт не привязан к предмету либо merge прошёл без вердикта.
right-token-exit=0
NOTE  research/critiques/C-999-subject-lock.md: subject-lock открыт явным ALLOW-SUBJECT-CHANGE (аудит-след, НЕ доказательство — F-064-6): scripts/verify_M-99.sh
VERDICT: PASS — вердиктов проверено: 1, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ unreadable-PID and bind-shaped-cwd GC adversarial fixture
unreadable-proc-readlink=denied
WOULD-REMOVE  wt (чист, на origin, смержен)
VERDICT: GC DRY-RUN
simulated-bind-cwd=/home/nous/.cache/paxio-tmp/critic-gc-paths.LjrNRb/bind
WOULD-REMOVE  wt (чист, на origin, смержен)
VERDICT: GC DRY-RUN
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=c00b7267c2ea4ce938a3a3f0bd96be7359aea200 GITHUB_SHA=f979cee57b932c2de16db132d6ed6e547cc59e13 bash scripts/check_docs_freeze.sh
docs-freeze-exit=0

$ CI wiring model on refs/pull/91/merge
docs_freeze_needs=True
docs_freeze_fail_closed_if=True
branch_health_in_needs=False
branch_probe_present=True
branch_live_observer_nonblocking=True
wiring-model-exit=0

$ bash -n scripts/gc_worktrees.sh scripts/check_branch_health.sh scripts/tests/red_gc_live_cwd.sh scripts/tests/red_branch_health.sh
bash-syntax-exit=0

$ git diff --check c00b7267c2ea4ce938a3a3f0bd96be7359aea200 HEAD
diff-check-exit=0
~~~

## Handoff

The next agent is architect. Do not dispatch dev or merge this PR. Repair both unsafe
cwd-observation paths, extend the RED probe with the missing individual-unreadable-PID
case and an actual bind-capable test where available, run the harness-track adversary
again against the new committed head, and retain this verdict as the prior round’s
audit trail.
