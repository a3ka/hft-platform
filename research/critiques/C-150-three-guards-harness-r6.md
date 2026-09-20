<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: f8aba879e92d505a7e8ff999baf1f755a85d031f
verdict: REJECT
-->

# C-150 — round 6

## Verdict: REJECT (R6-1)

L16 is now a useful negative guard, but its whitelist rejects a legitimate future
local used by the holder calculation. I added the honest local `honest=0` and used it
in a no-op condition inside `holder_pids`; the real implementation remained otherwise
unchanged. The probe then failed only L16 (26/27, exit 1):

```text
FAIL L16-решение-не-читает-argv решение о держателе читает постороннее: honest
VERDICT: FAIL (сценариев: 1, мутантов с разошедшимся kill-set: 0)
```

This is a false red on an honest, argv-independent local variable. A guard that blocks
such a legitimate implementation will be disabled or worked around, so it violates
the gate's validity requirement. This is a new cause, not C-146/C-147's missing-state
coverage; ordinary REJECT applies.

The requested alias attack `x="$IDLE_H"` outside the function with `$x` inside was
executed: L16 catches `x` as an unlisted identifier (26/27, exit 1). The author’s
textual limit therefore remains accurately disclosed for that alias shape. However,
the extractor is not structurally robust: `awk '/^holder_pids\(\) \{/,/^\}/'` stops at
an unindented `}`. In an external copy I inserted a nested helper whose closing brace
starts at column zero, followed by a forbidden `$IDLE_H` read. L16 passed and the full
27-scenario probe was green (exit 0), demonstrating a separate textual false negative.

The eight forms, including `--reclaim 5` and `--reclaim-dry 5`, are reachable and all
27 scenarios plus the ten declared mutant kill-sets pass on the unmodified subject.
The adjacent reclaim-argument battery is 13/13. Cleanup found zero `red-gclive-*` and
`red-brhealth-*` directories in both `/home/nous/.cache/paxio-tmp` and `/tmp`.

## Done Block

```text
TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
сценариев исполнено: 27  ok: 27  FAIL: 0
G8/G11/G10/G9 kill-set совпали; cleanup red-gclive before 0 after 0
VERDICT: PASS; exit=0

x="$IDLE_H" alias copy: L16 failed (26/27), exit=1
honest local copy: L16 failed (26/27), exit=1
nested-helper/early-'}' copy: L16 passed, 27/27, exit=0

TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_reclaim_args.sh
VERDICT: PASS (13/13 сценариев); exit=0

cleanup counts: /home/nous/.cache/paxio-tmp = 0, /tmp = 0
```
