<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: 405186bed47c8737b8124ad1a374505bccca8a12
verdict: REJECT
-->

# C-153 — round 8 (A-020 delta)

## Verdict: REJECT

The process boundary and `env -i` isolation work for the normal path: exported
`MODE= reclaim`, `DRY=1`, `IDLE_H=5` do not reach `gc_holder.sh`; L17 passes. The
27-scenario battery and all ten composite kill sets pass, as does reclaim-args 13/13.

However, `SCRIPT_DIR` is derived directly from `$0` without resolving a symlink. Running
the committed script through a symlink in another directory makes the child path point
to the symlink directory, where `gc_holder.sh` does not exist. I executed:

```text
ln -s .../scripts/gc_worktrees.sh /home/nous/.cache/paxio-tmp/r8-link/renamed.sh
(cd repo && PATH=repo/scripts:$PATH r8-link/renamed.sh --dry-run)
bash: .../r8-link/gc_holder.sh: No such file or directory
VERDICT: GC DRY-RUN
exit=0
```

At both call sites the parent uses `if hp="$(holder_pids ...)"; then ... fi`; the
child's 127 is therefore treated exactly like “no holder” and execution continues.
On an otherwise eligible clean/merged worktree this is fail-open: a missing safety
decision can reach `WOULD-REMOVE`/`git worktree remove`. This is outside A-020's named
limits and is a new construction/contract defect.

The child contract itself is otherwise correct for normal results (0 means hold/unknown,
1 means definitely none), and the hostile-environment test proves the closed whitelist.
The brace/lexical canary class is not used for this finding. A separately constructed
composite mutant that adds a fourth parser variable to the env whitelist plus a matching
child condition is caught by the existing L14/L17 composite mechanism; no new variable
finding is claimed.

## Done Block

```text
TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
сценариев исполнено: 27  ok: 27  FAIL: 0
all ten kill-set'ов совпали; red-gclive до/после 0/0
VERDICT: PASS; exit=0

TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_reclaim_args.sh
VERDICT: PASS (13/13 сценариев); exit=0

hostile env (MODE/DRY/IDLE_H + L17): решение совпало с чистой средой; exit=0
symlink/renamed invocation: child No such file or directory, parent VERDICT: GC DRY-RUN, exit=0
cleanup: red-gclive/red-brhealth = 0 in /home/nous/.cache/paxio-tmp and 0 in /tmp
```
