# C-078 — M-65 ws-session round-2 critic re-check

**Critic:** Codex/GPT-5  
**Date (UTC):** 2026-08-11  
**Subject:** `milestones/M-65-ws-session.md` at `5a90bae` on `origin/feat/M-65-ws-session`  
**Previous verdict:** `research/critiques/C-077-M-65-ws-session.md` — REJECT  
**Gate:** `gates.md` §9 — new milestone spec. RAW-gate N/A: no journal layout, no `contracts` impact.  
**Verdict:** **NOTE**

## §0 — Pre-flight verification

5-item commit artifact set check:

1. Commit-chain resolves on branch: `50dae79` (spec) → `2cd2555` (`C-077`) → `5a90bae` (rev2).
2. Audited checkout matches prompt: `HEAD == origin/feat/M-65-ws-session == 5a90bae`.
3. Previous critic artifact exists on branch: `research/critiques/C-077-M-65-ws-session.md`.
4. Rev2 artifact set is plan-time and scoped: `milestones/M-65-ws-session.md | +55/-8`; no code, no contracts, no journal layout.
5. This critic round is narrow: only closure of `C-077` B-1, B-2, N-1 plus explicit `unsubscribe` semantics; full spec is not re-opened.

Einhard pre-checks, adapted to this HFT plan-time gate:

- Pick-rationale: narrow re-check is the right path; no third-round structural-axis dispute is raised.
- B4-pre: N/A for code behavior; no implementation diff exists.
- B5-pre: N/A for runtime tests; only RED/acceptance sufficiency is audited.
- I0-pre: no implementation surface changed, so no hidden code regression surface.
- Cross-module: no `contracts`, journal, `gateway`, risk, killswitch, OMS, or venue module impact found in the rev2 diff.

## §1 — Closure Results

### B-1 — CLOSED: connection boundary is now a structural axis

**Evidence:** `milestones/M-65-ws-session.md:89`, `milestones/M-65-ws-session.md:106-111`,
`milestones/M-65-ws-session.md:124-147`, `milestones/M-65-ws-session.md:191-192`,
`milestones/M-65-ws-session.md:235`

Rev2 added axis 7, **"Граница соединения"**, as a member of the result invariant rather than as
an extra case. The row names both required violation values:

- `подписка другого соединения меняет выдачу текущего`
- `одинаковый sub id в двух соединениях делит состояние`

It also names the legitimate scenario: two connections may use the same `sub id` with different
selectors and must still receive independent streams.

**Reproduction from C-077:** a process-global map keyed by `sub id`, or one shared selector
changed by another connection. Rev2 now kills both classes: `O-9` requires two connections and
checks subscribe, selector change, error, and `unsubscribe` in one connection against the other;
`connshare` and `crosstalk` are explicit battery mutants. The sharpened forbidden list now bans
catalog/process/sub-id-global subscription state and ties the prohibition to axis 7 and `O-9`.

**Condition from C-077:** satisfied. The one-connection suite is no longer blind by construction
to cross-talk.

### B-2 — CLOSED: unsubscribe lifecycle is now observable

**Evidence:** `milestones/M-65-ws-session.md:77`, `milestones/M-65-ws-session.md:90`,
`milestones/M-65-ws-session.md:106-110`, `milestones/M-65-ws-session.md:134-153`,
`milestones/M-65-ws-session.md:193-198`

Rev2 added axis 8, **"Жизненный цикл подписки"**, task `6bis`, oracle `O-10`, and mutants
`unsubmute` / `capleak`.

`O-10` covers all four points required by `C-077`:

1. after `unsubscribe(id)`, no further `snapshot` or `frame` carries that `sub`;
2. the neighboring subscription on the same connection continues;
3. capacity under the 16-subscription limit is freed and reusable;
4. repeated or unknown `id` returns a machine-readable `error`.

The explicit semantic choice for repeated/unknown `unsubscribe` is also justified, not merely
present. Lines 149-153 reject silence because §4.1 forbids failures that look like no response,
and reject "success" because it makes client-side subscription accounting unprovable. Keeping the
connection and neighboring subscriptions alive remains consistent with axis 5.

**Reproduction from C-077:** parse `unsubscribe` as no-op, or stop the stream but leak capacity.
Rev2 now names both as separate failure classes through `unsubmute` and `capleak`; the unknown-id
silence value is guarded by direct `O-10` assertions, with the no-mutant boundary explicitly named.

**Condition from C-077:** satisfied.

### N-1 — CLOSED: unknown `op` is explicit

**Evidence:** `milestones/M-65-ws-session.md:71`, `milestones/M-65-ws-session.md:92-96`,
`milestones/M-65-ws-session.md:142`

Task 1 now states that unknown `op` returns `error` with a code. `§3.1` explicitly expands `O-3`
from unknown version to unknown version plus unknown operation, and axis 3 still carries the
unknown-operation violation value. The previous "axis table has it, oracle table does not" gap is
closed.

### N-2 — NOTE: acceptance row F still says O-1..O-8

**Evidence:** `milestones/M-65-ws-session.md:78`, `milestones/M-65-ws-session.md:82-90`,
`milestones/M-65-ws-session.md:246-249`

Task 7 correctly says `red_ws_session.rs (O-1..O-10)`, and §3.1 defines O-9/O-10. But acceptance
step `F` still says:

```text
red_ws_session.rs (O-1..O-8) GREEN
```

This is a stale range after rev2. I am not making it a blocker because `§3.1`, task 7, step `N`,
and the battery all pin the new oracles structurally. Still, leaving row `F` stale invites an
implementation of `verify_M-65.sh` that reports the main RED suite green while only running the old
eight named oracles.

**Condition to remove:** update acceptance step `F` to `red_ws_session.rs (O-1..O-10) GREEN`.

## §2 — 4-phase rubric

1. Artifact integrity and scope: PASS. Rev2 is one milestone-spec diff, no forbidden code or
contract surface.
2. Substantive sufficiency: NOTE. The three `C-077` items in scope are closed; only stale
acceptance wording remains.
3. Gating discipline: PASS with N-2 note. No arbiter route is triggered because this round does not
repeat the same structural-axis dispute.
4. Output discipline: PASS. This verdict is recorded as a `research/critiques/` artifact and must
be committed/pushed before handoff.

## §3 — Verdict rationale

The prior REJECT blockers are removed. Axis 7 makes cross-connection state isolation testable, and
axis 8 makes subscription removal testable in both directions: output stops and capacity is
returned. Unknown `op` is now named directly in O-3.

Verdict is **NOTE**, not REJECT: M-65 may proceed after architect records this verdict and either
fixes the stale `§6` acceptance range mechanically or carries that note into the RED/verify task
without losing O-9/O-10.

## §4 — Next action

Architect should append the critic verdict mechanically and fix `§6` step `F` from `O-1..O-8` to
`O-1..O-10`. After that, dispatch the appropriate dev/test-writing sequence for M-65.
