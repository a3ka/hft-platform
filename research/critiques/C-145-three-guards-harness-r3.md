<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: 17105f290eac599d7e492dc2539fc831c402b978
verdict: REJECT
-->

# C-145 — three guards harness, round 3: REJECT

## Scope

- Route: harness adversary under `docs/workflow/harness-track.md` §§3 and 5.
- Subject: PR #91, `harness/three-guards-2026-08-24`; the audited delta is exactly
  `ffb738f..17105f2` (one commit, `17105f290eac599d7e492dc2539fc831c402b978`).
- Merge base: `c00b7267c2ea4ce938a3a3f0bd96be7359aea200`.

This is harness-track work, so the ordinary product-milestone T-contract / trait / separate
RED / verify set does not apply. The applicable set is the guard, executable probe,
mutation battery, and cleanup proof. The new probe closes R2-1 for ordinary GC, but not for
the reclaim invocation explicitly required by C-141.

## Verdict: REJECT

### R3-1 — L14 pins the dead-PID decision only outside `--reclaim`

`scripts/tests/red_gc_live_cwd.sh:324-331` constructs the enumerated-but-not-directory
dead-PID state and calls the subject once with no mode argument (`:327`). It creates neither
`target/` nor an eligible reclaim run. Yet `holder_pids` is also the precondition of the
reclaim loop in `scripts/gc_worktrees.sh:248`; in `--reclaim` mode the same decision must
permit both cache reclamation and the following ordinary-GC pass.

This leaves the requirement stated in C-141 condition 1 — an otherwise eligible **removal /
reclaim** must be permitted — only half tested.

#### Reproduction

I built a seventh-external mutant not present in the declared G1/G3–G8 battery. Immediately
after the failed-`readlink` entry in `holder_pids`, it adds:

```bash
if [ "${MODE:-gc}" = "reclaim" ] && [ ! -d "$p" ]; then
  echo "${pid}?"
  return 0
fi
```

The existing `G8` needle remains unchanged, so all seven declared mutants still build and
their declared kill-sets still match. The full probe reports `19 ok; 0 FAIL`, exit 0. The
mutant nevertheless makes the L14 state a holder when `--reclaim` is requested. In an
otherwise eligible fixture with an aged `target/`, its dry run produced:

```text
KEEP-CACHE  wt — ЖИВОЙ процесс держит cwd (PID: 4242? ), 1MB
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )
VERDICT: GC DRY-RUN
target-after=present
reclaim-observed=no
dead-pid-held-reclaim=yes
```

The fixture is deterministic and no scheduler race is needed: it is a numeric entry for
which `-e` succeeded but the later cwd observation is not a process directory. That is a
valid oracle for the response to the observable post-enumeration state; it does not claim to
reproduce the timing of PID disappearance. The ordinary-GC part is therefore sufficient:
the literal C-141 G8 mutant was independently built and L14 failed against it. The missing
mode coverage is the blocker.

#### Condition to clear

Add a reclaim variant of L14 (or extend L14 without hiding its two obligations) that:

1. builds the same numeric non-directory `GC_PROC_ROOT` entry and an aged eligible
   `target/`;
2. runs `--reclaim-dry 0` with the normal `pgrep` confounder controlled;
3. requires `WOULD-RECLAIM` and rejects both `KEEP-CACHE ... PID: 4242?` and the subsequent
   ordinary-GC false holder; and
4. adds the reclaim-only dead-PID-holder mutant to the declared battery with its exact
   kill-set.

## Checks that passed

- The literal R2-1 mutant (`[ -d "$p" ] || continue` → emit `"${pid}?"`, set `found=1`,
  continue) was built independently. It is syntactically valid and L14 alone fails, exit 1.
  The in-repository G8 likewise reports exactly `L14-мёртвый-PID-не-держит` as its kill-set.
- The committed probe is green: all 19 scenarios (the 18 prior cases plus L14) and all seven
  declared kill-sets pass. `red_gc_reclaim_args.sh` also passes 13/13.
- No `red-gclive-*` fixture remains under the required
  `TMPDIR=/home/nous/.cache/paxio-tmp` (count: 0). My private mutation and reproducer
  fixtures were removed after the measurements.

## Done Block

```text
$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
сценариев исполнено: 19  ok: 19  FAIL: 0
G1 G3 G4 G5 G6 G7 G8 — все семь kill-set'ов совпали
каталогов red-gclive-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ build C-141 literal G8 externally; TMPDIR=/home/nous/.cache/paxio-tmp bash red_gc_live_cwd.sh
g8-mutant-syntax-exit=0
FAIL       L14-мёртвый-PID-не-держит исчезнувший PID стал УНИВЕРСАЛЬНЫМ держателем — GC заблокирован навсегда
сценариев исполнено: 19  ok: 18  FAIL: 1
VERDICT: FAIL (сценариев: 1, мутантов с разошедшимся kill-set: 0)
g8-own-scenarios-exit=1

$ build reclaim-only dead-PID-holder external mutant; TMPDIR=/home/nous/.cache/paxio-tmp bash red_gc_live_cwd.sh --battery
mutant-syntax-exit=0
сценариев исполнено: 19  ok: 19  FAIL: 0
G1 G3 G4 G5 G6 G7 G8 — все семь kill-set'ов совпали
VERDICT: PASS
own-mutant-battery-exit=0

$ GC_PROC_ROOT=<numeric-nondirectory fixture> bash <reclaim-only-mutant> --reclaim-dry 0
KEEP-CACHE  wt — ЖИВОЙ процесс держит cwd (PID: 4242? ), 1MB
-----
/dev/md2        437G  304G  112G  74% /

KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )
VERDICT: GC DRY-RUN
reproducer-exit=0
target-after=present
reclaim-observed=no
dead-pid-held-reclaim=yes

$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_reclaim_args.sh
VERDICT: PASS (13/13 сценариев)
exit=0

$ find /home/nous/.cache/paxio-tmp -maxdepth 1 -type d -name 'red-gclive-*' | wc -l
0

$ bash -n scripts/gc_worktrees.sh
gc-syntax-exit=0

$ git diff --check
diff-check-exit=0
```

## Handoff

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata

- Date (UTC, ISO-8601): 2026-08-25T10:29Z
- Milestone: M-60a
- Status: BLOCKED — R3-1 reclaim-only dead-PID holder is a false green
- HEAD audited: 17105f290eac599d7e492dc2539fc831c402b978

## §B — What I did

- Audited the committed one-commit R3 delta and executed L14, every declared mutant, the
  literal R2 mutant, a distinct reclaim-only mutant, and an isolated eligible reclaim run.
- Confirmed that L14 models the relevant post-enumeration decision deterministically, but
  found no reclaim-mode assertion for that decision.

## §C — Artifact / result

- `research/critiques/C-145-three-guards-harness-r3.md`
- Verdict: REJECT; raw outputs and exit codes are recorded above.

## §D — Next agent + invocation

- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  For M-60a / PR #91 at committed C-145 R3-1, repair only the missing reclaim-mode oracle
  in scripts/tests/red_gc_live_cwd.sh. Preserve the deterministic L14 numeric
  non-directory state, add an eligible aged target and --reclaim-dry 0 assertion that
  requires WOULD-RECLAIM and no dead-PID KEEP in both reclaim and the following ordinary
  GC pass. Add the reclaim-only dead-PID-holder mutant to the declared battery with its
  exact kill-set. Run the full probe, its mutation battery, red_gc_reclaim_args, and the
  TMPDIR cleanup count. Commit and push the new head to harness/three-guards-2026-08-24;
  do not edit C-145; request a fresh critic round over the committed head.
  ```
- Push status: this critic verdict is committed and pushed with this response.
- Build cache: not created in this critic worktree; temporary private fixtures removed.

## §E — Risks / open questions

- The known bind-alias-subdirectory limit remains explicitly excluded by the guard and is
  not this finding.

=== END HANDOFF ===
