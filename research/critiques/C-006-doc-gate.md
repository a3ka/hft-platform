# C-006 — Critic Verdict — DOC-гейт + retro docs

**Date:** 2026-07-14T12:49Z  
**Audited branch:** `docs/doc-gate`  
**Audited commits:** `2542d2a` plus range `2542d2a..5ef9865`  
**Audited HEAD:** `5ef9865` — `docs(process): DOC-ГЕЙТ — архитектурные документы проходят critic → reviewer → founder`  
**Critic:** critic, plan-time doc gate, read-only except this verdict artifact.

## Verdict

**REJECT**

## Verdict Justification

The direction is correct: architecture documents are execution sources and need a gate. The current text should not become ACTIVE yet because the new doc-gate contradicts existing critic/push rules, M-09 contains hidden contract and rate-limit dependencies, and the backlog order has acceptance dependencies that are not expressible as written. These are process/spec defects, not objections to founder-owned priorities.

## Pre-flight Verification

- Commit set present: PASS. `git log 2542d2a^..5ef9865` shows:
  - `2542d2a` — FA ops, BACKLOG, P2.5 roadmap.
  - `5ef9865` — doc-gate rule and PROPOSED status updates.
- Changed files in audited range: `.claude/rules/gates.md`, `.claude/rules/commit-discipline.md`, `docs/DESIGN.md`, `docs/fa/ops.md`, `milestones/BACKLOG.md`.
- Verdict persistence path: PASS. `.claude/rules/branch-hygiene.md` requires critic verdicts under `research/critiques/` to be committed on the branch.

## Findings

### MAJOR

**M1 — DOC-гейт conflicts with existing critic trigger and push-scope text.**

- Evidence: `.claude/rules/gates.md:19-20` still says low-risk `docs-правка` skips critic, while the new doc-gate makes architecture/process docs class A with mandatory critic at `.claude/rules/gates.md:187-206`.
- Evidence: `.claude/rules/gates.md:136-137` still says the pusher can be `architect для docs/process-only`, while the new doc-gate says reviewer merges and pushes class A at `.claude/rules/gates.md:207-209`; commit-discipline repeats reviewer push for class A at `.claude/rules/commit-discipline.md:84-90`.
- Impact: future agents can make opposite routing decisions from the same rule file. A process doc edit can be read as "docs, skip critic, architect push" or "class A, critic → reviewer".
- Required fix: update §1 and §8 to explicitly defer architecture/process docs to §9, while preserving class B self-push for status columns, close-out proofs, typo-only edits, and broken-link fixes that do not change commands, scope, gates, or tasks.

**M2 — Class A/B boundary is mostly useful but still ambiguous for real commits.**

- Evidence: class A is semantic change: "what/how/order" at `.claude/rules/gates.md:187-198`; class B includes "опечатки/формат/битые ссылки" at `.claude/rules/gates.md:216-220`; the only classifier is "изменит ли этот текст то, что сделает следующий агент?" at `.claude/rules/gates.md:222`.
- Ambiguity: a "typo" in a path, command, threshold, role name, or invariant can absolutely change what the next agent does. A "broken link" fix can retarget authority to a different source.
- Required fix: split class B into mechanically non-semantic edits only. Examples that remain self-push: status columns, close-out proof append, formatting, spelling in prose with no command/path/threshold/role change. Examples that are class A even if small: command/path changes, role names, gate thresholds, invariant IDs, milestone task/acceptance text, or any cross-reference that changes source-of-truth.

**M3 — DOC-гейт does not define the self-amendment loop for edits to the gate itself.**

- Evidence: `.claude/rules/gates.md:198` classifies `.claude/rules/*` and `.claude/agents/*` as class A; `.claude/rules/gates.md:200-209` requires critic then reviewer.
- This is not a hard deadlock because the branch can stay PROPOSED and be re-audited, but the rule never says that critic-driven fixes to the same PROPOSED class-A document stay in the same gate instance rather than spawning an infinite nested gate.
- Required fix: add one sentence: "Edits made to address a doc-gate REJECT on the same PROPOSED branch are part of the same gate cycle and require re-critic/reviewer, not a new nested gate." Add emergency founder override only for production-breaking process fixes.

**M4 — OPS-I-1 is specified as a gate and then weakened into a calibration wish.**

- Evidence: `docs/fa/ops.md:76-84` requires `book_divergence_bps`; divergence `> ε` alerts, resyncs, and writes a `Sys` event.
- Evidence: `docs/fa/ops.md:133-134` says ε=5 bps is only an estimate and the first day is "metric only, without alert" for calibration.
- Evidence: `docs/DESIGN.md:233` and `milestones/BACKLOG.md:47-48` make injected book corruption alerting part of P2.5/M-09 acceptance.
- Impact: dev can satisfy "metric only" while acceptance says "alerts." That turns the main OPS-I-1 invariant into a non-gate for its highest-risk first day.
- Required fix: split calibration and enforcement. M-09 acceptance should pin a deterministic test threshold for injected corruption and best bid/ask mismatch, while production ε can be calibrated as a parameter with a minimum fail-closed fallback.

**M5 — OPS-I-1 / M-09 hides a T1 contract-RFC dependency for the promised `Sys` event.**

- Evidence: `docs/fa/ops.md:82-84` and `milestones/BACKLOG.md:41-43` require a recon/resync `Sys` event in the journal.
- Evidence: current `SysEvent` only has `Heartbeat`, `ConnUp`, and `ConnDown` in `crates/contracts/src/lib.rs:146-151`.
- Evidence: `EventKind` is T1 and new variants require contract-RFC per `crates/contracts/src/lib.rs:135-143` and `docs/05-contract-layer.md:46-61`.
- Evidence: `milestones/BACKLOG.md:45-46` says contract-RFC is not needed, then hedges "`Sys`-варианты — проверить, возможно нужен."
- Impact: M-09 cannot implement the promised audit event without a T1 change or without weakening the promise. "Maybe needed" is not operational enough for a milestone that will author RED tests.
- Required fix: either declare a contract-RFC as a blocking subtask of M-09 for recon/resync/degradation events, or change acceptance to use an already-existing observable.

**M6 — OPS-I-1 lacks a rate-limit/backoff gate despite TD-013 being the cited lesson.**

- Evidence: `docs/fa/ops.md:77-83` adds periodic REST snapshots per symbol; `docs/fa/ops.md:135-136` says the cost is negligible but should be counted in rate-budget.
- Evidence: TD-013 documents the actual failure class: HTTP 418 ban, immediate retry, 133 requests in 25 seconds, missing backoff, missing `Retry-After`, and no REST frequency cap at `TECH-DEBT.md:153-167`.
- Impact: "1 request / 5 min / symbol" is acceptable only for the scheduled path. The dangerous path is error/resync/retry behavior. Without a RED gate for cooldown/backoff/cap, recon can recreate the exact TD-013 class under failure.
- Required fix: M-09 acceptance must include a rate-budget/backoff oracle: honor 418/429/`Retry-After`, cap REST requests per venue/symbol, avoid concurrent resync storms, and prove injected repeated REST failure does not hot-loop.

**M7 — OPS-I-5 alert parity is not operational enough to be a sacred invariant.**

- Evidence: `docs/fa/ops.md:112` says every incident class from §1 has an alert rule and the regression test checks that the rule references `TD-NNN`.
- Evidence: §1 includes `C1 (M-08)` at `docs/fa/ops.md:24`, which is not a `TD-NNN`.
- Evidence: the concrete CI rule at `docs/fa/ops.md:125-126` only checks each P0/P1 rule references an existing metric; it does not prove every incident class has a rule.
- Impact: an implementation can pass alert↔metric parity while omitting an incident class, especially C1/book corruption or backup restore drill failures.
- Required fix: add an incident-to-alert matrix with stable IDs (`TD-011`, `TD-013`, `TD-014`, `TD-016`, `C1-M08`) and require CI to check both directions: each incident class has at least one P0/P1 rule, and each P0/P1 rule references an existing metric.

**M8 — BACKLOG M-11 requires testnet trading before the runner/arming milestone exists.**

- Evidence: `milestones/BACKLOG.md:61-68` defines M-11 as Risk + Killswitch + OMS and acceptance includes "48 ч HL testnet чисто."
- Evidence: `milestones/BACKLOG.md:70-76` puts runner, paper/live composition, and arming in M-12.
- Evidence: `docs/DESIGN.md:94` places runner in layer 6, and `docs/01-engine-architecture.md:149-155` defines runner as the mode/arming composition root.
- Impact: without runner or an explicitly scoped temporary harness, M-11 cannot honestly run 48h testnet trading. If the temporary harness is allowed, that is a hidden scope/acceptance dependency and risks diverging from `backtest == paper == live`.
- Required fix: either move minimal runner/arming into M-11, move the 48h HL testnet acceptance to M-12, or add an explicit M-11 harness contract that is later replaced and does not bypass runner invariants.

**M9 — BACKLOG places Hyperliquid depth after milestones that depend on it.**

- Evidence: `milestones/BACKLOG.md:78-81` says HL depth must be closed before P3 or the first live signal must stop relying on OBI bands.
- Evidence: the same backlog schedules M-13 after M-11 Risk+OMS and M-12 Runner; DESIGN P3 is Risk+OMS testnet at `docs/DESIGN.md:234`.
- Impact: if OBI/bands remain the first strategy, the venue-depth gap is a precondition to P3/testnet, not a post-P3 cleanup. If it is not a precondition, the backlog must explicitly say P3/M-11/M-12 use a non-OBI or Binance-only test signal.
- Required fix: move M-13 before P3-dependent testnet work, or state the strategy substitution and acceptance criteria for M-11/M-12.

**M10 — P2.5 founder marker is internally inconsistent and subordinate roadmap docs drift.**

- Evidence: the P2.5 row label says "critic → reviewer → founder ★" at `docs/DESIGN.md:233`, but the founder column for that row is `—`.
- Evidence: `docs/fa/ops.md:3-4` and `milestones/BACKLOG.md:3-4` require founder ★ for the same change.
- Evidence: subordinate roadmap docs still show P2 then P3 with no P2.5: `docs/01-engine-architecture.md:185-196` and `docs/04-workflow.md:122-132`.
- Impact: DESIGN is the master authority, but agents commonly read the detailed engine/workflow docs. Leaving old P3-next tables active creates a real chance of dispatching Risk+OMS next and bypassing the proposed data-safety phase.
- Required fix: set the P2.5 founder column to ★ or explain why founder is only for the doc-gate and not phase acceptance; update or mark subordinate roadmaps as superseded by DESIGN §10 + BACKLOG.

### MINOR / NOTES

**N1 — Data safety net before Risk/OMS is defensible.**

- Evidence: `docs/fa/ops.md:15-31`, `milestones/BACKLOG.md:33-48`, and `docs/DESIGN.md:241-250` consistently argue from five human-caught incidents plus single-copy data.
- Critic assessment: this is a founder priority decision, but the technical rationale is coherent. The rejection is about hidden dependencies and gates, not about the priority direction.

**N2 — OPS-I-6 does not contradict determinism if recon/degradation events are domain events.**

- Evidence: OPS-I-6 says metrics do not go into the journal because wall-clock/RSS are not domain events (`docs/fa/ops.md:113`).
- Evidence: DESIGN §1 requires deterministic replay from journal events (`docs/DESIGN.md:35-44`).
- Critic assessment: keeping Prometheus metrics out of the journal is correct. Recon mismatch/resync audit events can still be journal events if their T1 shape is governed. The problem is M5's missing contract path, not OPS-I-6 itself.

**N3 — Speed is preserved for true class B work.**

- Evidence: class B explicitly includes status columns, close-out reports/proofs, and typo/link fixes at `.claude/rules/gates.md:216-220`, and commit discipline preserves class B architect self-push at `.claude/rules/commit-discipline.md:88-90`.
- Required hardening: class B needs the non-semantic qualifier from M2 so it does not swallow tiny semantic edits.

## Requested Checks Matrix

### A. New DOC-гейт

- Recursion/deadlock: PARTIAL. No hard deadlock, but self-amendment/re-audit loop is unstated (M3).
- Class A/B operationality: PARTIAL. Core semantic test is useful, but typo/link/process edge cases are ambiguous (M2).
- Speed paralysis: PASS with caveat. Status columns and close-out proofs remain self-push; class B needs tighter wording (N3/M2).
- Contradiction with §1/§4/§8/branch-hygiene:
  - §1 contradiction: FAIL (M1).
  - §4 reviewer unconditional: PASS, doc-gate extends reviewer gate rather than removing it.
  - §8 push-scope/pusher wording: FAIL until architect docs/process-only text is reconciled (M1).
  - branch-hygiene: PASS. Verdict artifact is committed here per `.claude/rules/branch-hygiene.md`.

### B. Retro PROPOSED documents

- `docs/fa/ops.md`: REJECT until OPS-I-1, OPS-I-5, T1 `Sys` dependency, and rate-limit/backoff gates are made operational (M4-M7). OPS-I-6 is acceptable (N2).
- `milestones/BACKLOG.md`: priority direction is coherent, but M-11/M-12 and M-13 dependencies must be repaired before queue is executable (M8-M9).
- `docs/DESIGN.md` §10 P2.5: concept aligns with §2 layer 7 and §5 honesty, but founder marker and subordinate roadmap drift must be fixed (M10).

## Recommended Next Action

Architect should make a small doc repair set before reviewer/founder promotion:

1. Reconcile `gates.md` §1/§8 with new §9 and tighten class B semantics.
2. Add the doc-gate self-amendment/re-audit loop.
3. Repair M-09 gates: explicit contract-RFC or non-T1 observable, pinned OPS-I-1 alert threshold for tests, REST backoff/rate-budget RED, and incident-to-alert matrix.
4. Repair BACKLOG dependencies: M-11/M-12 runner/testnet ordering and M-13 HL-depth placement or substitution.
5. Fix DESIGN P2.5 founder column and mark/update old P3-next roadmap tables in subordinate docs.

## Confidence

High. I read the audited commits, `gates.md`, `commit-discipline.md`, `branch-hygiene.md`, `docs/fa/ops.md`, `milestones/BACKLOG.md`, `docs/DESIGN.md`, relevant contract/T1 definitions, TD-013, and cross-roadmap references in engine/workflow docs.

=== HANDOFF: critic -> reviewer ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-07-14T12:49Z
- Branch: `docs/doc-gate`
- Status: REJECT from critic; reviewer requested next by founder relay
- Audited HEAD: `5ef9865` — `docs(process): DOC-ГЕЙТ — архитектурные документы проходят critic → reviewer → founder`

## §B — Что я сделал
- Audited commit `2542d2a` and range `2542d2a..5ef9865`.
- Checked DOC-гейт against existing gates §1/§4/§8, commit discipline, and branch hygiene.
- Checked retro PROPOSED docs: `docs/fa/ops.md`, `milestones/BACKLOG.md`, `docs/DESIGN.md` §10 P2.5.

## §C — Артефакты / результаты
- Verdict file: `research/critiques/C-006-doc-gate.md`
- Verdict: REJECT.
- Blocking classes: rule contradictions, ambiguous class B, missing same-gate repair loop, OPS-I-1/5 false-gate risk, hidden T1 `Sys` dependency, TD-013 rate-limit gate gap, M-11/M-12 and M-13 ordering dependencies, P2.5 founder/roadmap drift.

## §D — Следующий агент + инвокация
- **Следующий агент:** `reviewer`
- **Then:** founder ★ after reviewer summarizes whether to require architect repairs or accept an explicit founder override.
- **Paste-ready prompt for reviewer:**
  ```
  Ты — reviewer. Worktree: take your own branch-hygiene worktree from docs/doc-gate.

  Review critic verdict `research/critiques/C-006-doc-gate.md` plus audited commits
  `2542d2a` and `5ef9865`.

  Focus:
  1. Confirm or dispute critic REJECT findings M1-M10.
  2. Check scope/status handling for PROPOSED -> ACTIVE.
  3. Decide whether docs/doc-gate can proceed to founder with notes, or must route back
     to architect for doc repairs first.
  4. Preserve founder-owned priority decision: do not re-rank BACKLOG; surface only
     untested assumptions and hidden dependencies.

  Code remains untouched. End with founder-facing summary and explicit next action.
  ```
- Push status: not pushed by critic; verdict is committed locally on `docs/doc-gate` per branch-hygiene unless a separate relay pushes the branch.

## §E — Риски / открытые вопросы
- If reviewer/founder overrides the REJECT, M-09 milestone authoring must still not hide the contract-RFC/rate-limit/runner-depth dependencies.
- The priority "data safety net before money path" is technically coherent, but only after the acceptance gates above are made executable.

=== END HANDOFF ===
