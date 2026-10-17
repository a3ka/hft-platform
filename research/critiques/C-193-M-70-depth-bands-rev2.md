<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: 8610d9f3e531b6c823e4e6620a061eac49376470
verdict: REJECT
-->

# C-193 — M-70 depth-bands enablement rev2: REJECT

## Verdict

**REJECT — do not dispatch engine-dev.** The committed subject chain contains only
the revised milestone file. It does not contain the architect's required pre-dispatch
artifact set: T2/trait contract declarations, RED tests, or the real acceptance gate
`scripts/verify_M-70.sh`. Per the critic profile, this is **NOT REVIEWED — ARCHITECT
ARTIFACTS INCOMPLETE**, rather than a review of the plan prose.

The absence is material. The milestone's open tasks cover a client-controlled resource
boundary and a changed read-path form. In particular, the future oracle set must
protect `VB-I-2` (live equals replay) and `VB-I-10` (bounded-window snapshot), both
live invariants in `docs/fa/viz-backend.md` §5. No committed M-70 RED artifact yet
demonstrates either protection or the proposed `DB-I-*` properties against a stub.

The architect handoff did not supply `stakes: high|normal`; that omission does not
weaken the structural rejection above.

## Artifact-set audit

| Required artifact | Result | Evidence |
|---|---|---|
| Milestone | Present | `milestones/M-70-depth-bands-enablement.md` is the sole changed path in `d77398d..8610d9f`. |
| T2 contracts / trait signatures | Missing | No committed change under `crates/gateway/src/**`, `crates/gateway-serve/src/**`, or `contracts/**`; the milestone's behavioural prose is not a committed interface declaration. |
| RED tests | Missing | The audited range contains no `*/tests/**` change and no `DB-I-*` oracle in a test path. Existing M-68/M-71 tests cannot serve as the RED suite for M-70 tasks 0 and 3–8. |
| Acceptance script | Missing | `scripts/verify_M-70.sh` does not exist at `8610d9f`; therefore CI parity, one check per open task, intentional-RED accounting, and fail-closed exit behaviour cannot be inspected or run. |
| T1 / Block-C | Not triggered | No `contracts/**` change is in the range; this matches the T-designate boundary in `docs/05-contract-layer.md` §2. |

## Required disposition

Return to **architect**. Commit the complete M-70 pre-dispatch artifact set on
`docs/M-70-rev2`, then request a new critic round. The next round must audit those
committed files and their intentional RED behaviour; it must not rely on this
milestone text or on M-68/M-71 artefacts by reference alone.

## Done Block

```text
$ git log --format='%H %s' d77398d..8610d9f
8610d9f3e531b6c823e4e6620a061eac49376470 spec(M-70): rev2 — спека приведена к ФАКТУ, задачи 1/2 закрыл M-71 [architect]
exit=0

$ git diff --name-status d77398d..8610d9f
M	milestones/M-70-depth-bands-enablement.md
exit=0

$ git diff --name-only d77398d..8610d9f -- 'crates/**/tests/**' 'scripts/verify_M-70.sh' 'contracts/**'
exit=0

$ git diff --name-only d77398d..8610d9f -- crates/gateway/src crates/gateway-serve/src contracts
exit=0

$ git cat-file -e 8610d9f:scripts/verify_M-70.sh
fatal: path 'scripts/verify_M-70.sh' does not exist in '8610d9f'
exit=128

$ git grep -nE 'DB-I-[0-9]+' 8610d9f -- 'crates/*/tests/**'
exit=1

$ bash scripts/next_artifact_id.sh C
C-193
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31T12:xxZ
- Milestone: M-70-depth-bands-enablement
- Статус: BLOCKED — architect artifacts incomplete
- HEAD: 8610d9f — spec(M-70): rev2 — спека приведена к ФАКТУ, задачи 1/2 закрыл M-71 [architect]

## §B — Что я сделал
- Audited the committed range `d77398d..8610d9f`, not the milestone prose alone.
- Verified the mandatory ID allocation with `scripts/next_artifact_id.sh C` → `C-193`.

## §C — Артефакты / результаты
- `research/critiques/C-193-M-70-depth-bands-rev2.md`
- Done Block: artifact-range checks exit=0; absent verify script exit=128; absent M-70 RED oracle search exit=1; allocator exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-70 rev2 is REJECTED by C-193 as NOT REVIEWED — ARCHITECT ARTIFACTS INCOMPLETE. On docs/M-70-rev2, commit the full pre-dispatch set before requesting another critic: precise T2/trait interface declarations, sacred RED tests for the remaining M-70 tasks, and scripts/verify_M-70.sh as a real fail-closed CI-parity gate. The prior M-68/M-71 artifacts and milestone prose are not substitutes. Preserve VB-I-2 (live==replay) and VB-I-10 (bounded-window snapshot); then provide the new commit-chain reference, milestone path, and stakes: high|normal.
  ```
- Push-статус: pending commit and push to `origin/docs/M-70-rev2`.
- Кэш: N/A — no build cache created.

## §E — Риски / открытые вопросы
- The handoff omitted `stakes: high|normal`; include it in the next critic invocation.
- No fifth/terminal-round rule is implicated by this first recorded rejection of the incomplete rev2 artifact set.

=== END HANDOFF ===
