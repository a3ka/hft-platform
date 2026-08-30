<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: ff2192abc4c0d81b68696b36628bbba87d5d4b71
verdict: REJECT
-->

# C-151 — round 7

## Verdict: REJECT (R7-1)

The declaration-derived whitelist can be used to legalize the forbidden external
handle. In a copy I changed the function declaration to include
`IDLE_H="$IDLE_H"` and added the dead-PID condition on `${IDLE_H}`. L16 reported
`IDLE_H` as allowed and passed. The complete scenario set itself was green (27/27),
but the mutant battery no longer matched its declared kill sets, proving the guard
has accepted a real argv-dependent holder decision:

```text
ok L16-решение-не-читает-argv ... IDLE_H ...
сценариев исполнено: 27 ok: 27 FAIL: 0
VERDICT: FAIL (мутантов с разошедшимся kill-set: 5)
```

The RHS is evaluated before the local assignment, so this is not an inert shadow:
`--reclaim 5` is imported into the local and controls the dead-PID branch. The
whitelist must distinguish values initialized from argv-independent sources rather
than trusting the identifier spelling alone.

Declaration parsing itself handled same-line declarations and `local -r` in my
copies, but a `declare honest=0` used inside the function was not recognized and
produced a false red (26/27, exit 1). Thus the advertised “honest local” property is
still incomplete for a valid shell declaration form.

The brace counter fixes the prior early-`}` defect. Its stated residual limit is real:
a brace at column zero inside a quoted string/comment is counted as syntax. That is
normally intentional-only in this function, not an accidental regression; it remains
an honestly named textual limitation.

Unmodified subject regression passed 27/27, all ten kill sets, and reclaim arguments
13/13. Cleanup was 0/0 in both required temp roots.

## Done Block

```text
TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
сценариев исполнено: 27 ok: 27 FAIL: 0
G8/G11/G10/G9 and other kill-sets matched; red-gclive before/after 0/0
VERDICT: PASS; exit=0

local IDLE_H="$IDLE_H" copy: L16 PASS; 27/27 scenarios, mutant kill-set
verification FAIL (five mismatches), probe exit=1
declare honest=0 copy: L16 FAIL, 26/27, exit=1

TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_reclaim_args.sh
VERDICT: PASS (13/13); exit=0

cleanup: red-gclive/red-brhealth = 0 in /home/nous/.cache/paxio-tmp and 0 in /tmp
```
