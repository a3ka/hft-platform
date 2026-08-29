<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: c00b7267c2ea4ce938a3a3f0bd96be7359aea200
audited_head: 9e408118b2eb8e122c6316c62eda4a37cee9e4e3
verdict: REJECT
-->

# C-146 — three guards harness, round 4: REJECT

## Scope

- Route: harness adversary under `docs/workflow/harness-track.md` §§3 and 5.
- Subject: PR #91, `harness/three-guards-2026-08-24`; audited head
  `9e408118b2eb8e122c6316c62eda4a37cee9e4e3`.
- Merge base: `c00b7267c2ea4ce938a3a3f0bd96be7359aea200`.
- Delta over C-145: the single commit `9e40811`, adding L15 and G9.

This is harness-track work. Its applicable artifact set is the guard, executable probe,
mutation battery, CI/barrier wiring, and cleanup proof rather than product T-contracts and
traits.

## Verdict: REJECT

### R4-1 — dead-PID behavior is still unpinned for every dry invocation

C-145 R3-1 is closed: L15 (`scripts/tests/red_gc_live_cwd.sh:341-351`) runs an eligible
reclaim fixture, and the exact C-145 `MODE=reclaim` mutant fails L15 with both declared
assertions. G9 declares that same kill-set.

There are exactly two calls of `holder_pids`: the reclaim loop at
`scripts/gc_worktrees.sh:248` and the ordinary-GC loop at `:290`; no third call site exists.
But the dry forms set a second control variable, `DRY=1` (`:190-192`), and neither L14 nor
L15 executes it. L14 has no flags; L15 invokes `--reclaim 0` (`red_gc_live_cwd.sh:346`).

I built a ninth external mutant, absent from G1/G3–G9, which only turns the numeric
non-directory dead-PID state into a holder when `DRY=1`:

```bash
if [ "${DRY:-0}" = "1" ] && [ ! -d "$p" ]; then
  printf '%s?\n' "$pid"
  found=1
  continue
fi
```

It is syntactically valid and preserves every existing battery needle. The full probe still
reports 21 assertions green and all eight declared kill-sets matching, exit 0. Yet an
otherwise eligible fixture gives the following false green in every dry form:

```text
$ gc_worktrees.sh --dry-run
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )

$ gc_worktrees.sh --reclaim-dry 0
KEEP-CACHE  wt — ЖИВОЙ процесс держит cwd (PID: 4242? ), 1MB
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )

$ gc_worktrees.sh --dry-run --reclaim 0
KEEP-CACHE  wt — ЖИВОЙ процесс держит cwd (PID: 4242? ), 1MB
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )

$ gc_worktrees.sh --reclaim 0 --dry-run
KEEP-CACHE  wt — ЖИВОЙ процесс держит cwd (PID: 4242? ), 1MB
KEEP  wt — ЖИВОЙ процесс держит cwd in дереве (PID: 4242? )
```

All four commands exit 0 and print `VERDICT: GC DRY-RUN`, so the dry-run audit falsely
reports a holder and suppresses the eligible removal/reclaim plan. Against the unmutated
guard, the same fixture emits `WOULD-REMOVE` for `--dry-run` and both `WOULD-RECLAIM` and
`WOULD-REMOVE` for every reclaim-dry spelling. The two flag orders therefore parse correctly;
they are nevertheless unpinned against the dead-PID regression.

#### Condition to clear

Add executable dry-mode coverage for the same numeric non-directory state. It must require:

1. `--dry-run` to emit `WOULD-REMOVE`, not a holder;
2. `--reclaim-dry 0`, `--dry-run --reclaim 0`, and `--reclaim 0 --dry-run` each to emit both
   `WOULD-RECLAIM` and the subsequent `WOULD-REMOVE`; and
3. a declared mutant for the `DRY=1` false holder with its exact kill-set.

## Checks that passed

- The C-145 `MODE=reclaim` mutant was constructed independently. It fails exactly
  `L15-мёртвый-PID-в-reclaim` and `L15-target-забран`, exit 1.
- The committed suite is green: 21 assertions (the prior 19 plus L15's two assertions) and
  G1/G3–G9 all match their declared kill-sets. `red_gc_reclaim_args.sh` is green 13/13.
- Fixture cleanup is clean at the required
  `TMPDIR=/home/nous/.cache/paxio-tmp`: `red-gclive-*` count is 0. Private mutation and
  reproducer fixtures have been removed.

## Done Block

```text
$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_live_cwd.sh --battery
сценариев исполнено: 21  ok: 21  FAIL: 0
G1 G3 G4 G5 G6 G7 G8 G9 — все восемь kill-set'ов совпали
каталогов red-gclive-* до: 0, после уборки: 0
VERDICT: PASS
exit=0

$ build C-145 MODE mutant externally; TMPDIR=/home/nous/.cache/paxio-tmp bash red_gc_live_cwd.sh
r3-mode-mutant-syntax-exit=0
FAIL       L15-мёртвый-PID-в-reclaim мёртвый PID стал держателем В РЕЖИМЕ reclaim — кэш и дерево заперты навсегда
FAIL       L15-target-забран      кэш остался — сторож переблокировал в reclaim
сценариев исполнено: 21  ok: 19  FAIL: 2
VERDICT: FAIL (сценариев: 2, мутантов с разошедшимся kill-set: 0)
r3-mode-mutant-scenarios-exit=1

$ build ninth DRY mutant externally; TMPDIR=/home/nous/.cache/paxio-tmp bash red_gc_live_cwd.sh --battery
ninth-dry-mutant-syntax-exit=0
сценариев исполнено: 21  ok: 21  FAIL: 0
G1 G3 G4 G5 G6 G7 G8 G9 — все восемь kill-set'ов совпали
VERDICT: PASS
ninth-dry-mutant-battery-exit=0

$ healthy fixture: --dry-run | --reclaim-dry 0 | --dry-run --reclaim 0 | --reclaim 0 --dry-run
WOULD-REMOVE  wt (чист, на origin, смержен)
WOULD-RECLAIM  wt — 1MB, молчит 58282ч
WOULD-REMOVE  wt (чист, на origin, смержен)
exit=0 for each form

$ ninth-DRY-mutant fixture: same four forms
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )
KEEP-CACHE  wt — ЖИВОЙ процесс держит cwd (PID: 4242? ), 1MB
KEEP  wt — ЖИВОЙ процесс держит cwd в дереве (PID: 4242? )
exit=0 for each form; target-after-dry-runs=present

$ TMPDIR=/home/nous/.cache/paxio-tmp bash scripts/tests/red_gc_reclaim_args.sh
VERDICT: PASS (13/13 сценариев)
exit=0

$ find /home/nous/.cache/paxio-tmp -maxdepth 1 -type d -name 'red-gclive-*' | wc -l
0

$ bash -n scripts/gc_worktrees.sh
exit=0

$ git diff --check
exit=0
```

## Handoff

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata

- Date (UTC, ISO-8601): 2026-08-25T10:46Z
- Milestone: M-60a
- Status: BLOCKED — R4-1 dry-mode dead-PID holder is a false green
- HEAD audited: 9e408118b2eb8e122c6316c62eda4a37cee9e4e3

## §B — What I did

- Executed the full committed probe and mutation battery, independently rebuilt C-145's
  mode mutant, enumerated both call sites, and executed all dry flag forms on a clean fixture.
- Built the distinct ninth `DRY=1` mutant; it passed the complete current probe but suppresses
  every dry-mode removal/reclaim plan.

## §C — Artifact / result

- `research/critiques/C-146-three-guards-harness-r4.md`
- Verdict: REJECT; raw outputs and exit codes are recorded above.

## §D — Next agent + invocation

- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```
  For M-60a / PR #91 at committed C-146 R4-1, repair only the missing dry-mode oracle in
  scripts/tests/red_gc_live_cwd.sh. Exercise the numeric non-directory dead-PID fixture
  through --dry-run, --reclaim-dry 0, --dry-run --reclaim 0, and --reclaim 0 --dry-run;
  require the eligible WOULD-REMOVE / WOULD-RECLAIM results and no false holder. Add the
  DRY=1 dead-PID-holder mutant to the declared battery with its exact kill-set. Run the
  complete probe, battery, reclaim-argument probe, and TMPDIR cleanup count. Commit and
  push to harness/three-guards-2026-08-24 without editing C-146, then request a fresh
  critic round over the committed head.
  ```
- Push status: this critic verdict is committed and pushed with this response.
- Build cache: not created in this critic worktree; temporary private fixtures removed.

## §E — Risks / open questions

- `holder_pids` has no third call site at this head. The finding is the unpinned DRY state
  shared by the two proven call sites, not an undiscovered third loop.

=== END HANDOFF ===
