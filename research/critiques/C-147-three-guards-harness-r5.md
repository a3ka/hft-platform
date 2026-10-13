<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: 97f54b355f0525a530f22213c177f784c3090132
verdict: REJECT
-->

# C-147 — three-guards harness, round 5

## Verdict: REJECT (R5-1)

The new finite sweep closes the previously found MODE/DRY paths, but it does not
exhaust the parser state. `IDLE_H` is a third state variable: in reclaim mode one
valid positional threshold is accepted and changes the reclaim decision. The sweep
only exercises threshold `0` (or the default), so a dead-PID holder can still make
the probe green for another valid threshold.

I built an external copy of the exact script with this threshold-conditioned mutant:

```sh
if [ "${IDLE_H:-2}" = "5" ] && [ ! -d "$p" ]; then
  printf '%s?\n' "$pid"; found=1; continue
fi
```

The complete 24-scenario battery passed (`24 ok`, all declared G1/G3/G4/G5/G6/G7/G8/G9/G10
kill-sets matched, `VERDICT: PASS`, exit 0). A direct `--reclaim 5` reproduction then
left the tracked target present while reporting `ЖИВОЙ процесс держит cwd`; exit was 0.
That is a false green on a reachable seventh family of argv states (reclaim with a
non-zero threshold), and is a new cause, distinct from C-141/C-145/C-146.

The parser has exactly two `holder_pids` call sites (ordinary GC and reclaim), and
all six swept forms reach those sites. Repeated mode flags are idempotent; repeated
thresholds refuse, while `--reclaim 5` and `--reclaim-dry 5` remain valid states not
represented by the sweep. `--`/unknown flags refuse before holder inspection.

The exact C-146 DRY-conditioned mutant now fails the four dry forms, and the nine
declared mutants' kill sets match. Each swept form's exit code and marker assertion
passed; the shared `WOULD-RECLAIM` marker is reached by all four labelled forms.
The fixture `.gitignore` contains only `target/`, so it hides only the intended build
directory. The residual bind-alias subdirectory limitation remains explicitly named
as a limitation, not claimed coverage.

Adjacent reclaim-argument and branch-health batteries passed (13/13 and 25/25).
Cleanup counts were zero before/after in both `/home/nous/.cache/paxio-tmp` and `/tmp`.
The M-60a structural/docs-freeze verifier passed its parser and wiring checks (the
long fmt/clippy/test section was still running when this report was prepared).

## Done Block

Raw evidence (all commands run from the audited worktree):

```text
TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
scenarios executed: 24  ok:24  FAIL:0
G1 G3 G4 G5 G6 G7 G8 G10 G9 all kill-sets matched
cleanup red-gclive before 0 after 0
VERDICT: PASS
exit=0

TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_reclaim_args.sh
VERDICT: PASS (13/13 сценариев)
exit=0

TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_branch_health.sh --battery
сценариев исполнено: 25  ok: 25  FAIL: 0
каталогов red-brhealth-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

threshold mutant: full battery PASS, exit=0
threshold mutant direct --reclaim 5: target-after-threshold-run=present,
ЖИВОЙ процесс держит cwd, exit=0

cleanup counts: red-gclive/red-brhealth = 0 in /home/nous/.cache/paxio-tmp and 0 in /tmp
```

