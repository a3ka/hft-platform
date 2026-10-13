<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: 6fa01520c3560c9ff82c72c20c698502675255c2
verdict: NOTE
-->

# C-154 — round 9 (A-020 final delta)

## Verdict: NOTE

C-153 is closed. `readlink -f` resolves the script through a symlink, and missing
`gc_holder.sh` exits 2 before any worktree scan/removal. L18 and L19 pass. L20 proves
that child rc=3 is reported as `?(gc_holder rc=3)` and treated fail-closed. The parent
contract is now the safe triple: 0 hold/unknown, 1 definitely none, anything else hold.

I also exercised hostile exported parser variables, empty PATH, a failing `readlink`,
and `bash < script`: these paths fail closed (exit 2) rather than reaching removal.
Child stdout containing non-PID text at rc=0 is conservatively a hold (possible false
red, not false green); oversized output has the same resource-boundary character and is
not a named A-020 blocker. The known fd/uid/alias/GC_PROC_ROOT limits remain named and
are not reclassified.

An independent copy mutant mapping the non-zero child status to “no holder” is caught by
the L20 refusal scenario; the committed G12/G13 and all composite mutants likewise match.

## Done Block

```text
TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
сценариев исполнено: 30  ok: 30  FAIL: 0
13 kill-set'ов совпали; red-gclive до/после 0/0; VERDICT: PASS; exit=0

TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_reclaim_args.sh
VERDICT: PASS (13/13); exit=0

L18 symlink: exit=0; L19 missing child: exit=2 GC REFUSED; L20 child rc=3: fail-closed
empty PATH: exit=2; broken readlink: exit=2; bash < script: exit=2
cleanup red-gclive/red-brhealth: 0 in /home/nous/.cache/paxio-tmp, 0 in /tmp
```
