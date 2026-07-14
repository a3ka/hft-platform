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

---

## Re-audit (rev 6) - 2026-07-14T16:37Z

**Audited repair range:** `efe2ccc..e3bfc42`
**Expected/audited HEAD:** `e3bfc42` - `docs(doc-gate): C-006 rev5 - барьер переписан от РЕЗУЛЬТАТА (закрыты обе дыры мержа)`
**Worktree:** `/tmp/hft-critic-dg-r6`, local branch `critic-dg-r6`, created from `origin/docs/doc-gate` after exact HEAD check.

## Rev 6 Verdict

**NOTE - REJECT lifted.**

The protected-artifacts blocker is closed for silent-loss cases. The barrier now checks the result invariant: every protected artifact that existed at merge-base or was added in the branch must exist at HEAD, be renamed to another protected path, or have an explicit same-commit `ALLOW-ARTIFACT-DELETE:`.

One residual limitation remains, but it is fail-closed rather than unsafe: if a merge commit intentionally deletes a protected artifact and includes `ALLOW-ARTIFACT-DELETE:` in that merge commit body, the script still fails because it cannot attribute result-based disappearance to that merge. That blocks a rare legitimate deletion, but it does not allow artifact loss. Intentional deletion can be done as a normal commit with same-commit override.

## Rev 6 Probe Results

Executed against disposable worktree `/tmp/hft-critic-dg-r6-probe` at `e3bfc42`.

**Required loss/bypass probes:**
- Baseline audited branch: PASS, `exit=0`.
- Branch-local add->delete of `research/critiques/C-999.md`: FAIL, `exit=1`.
- Protected -> unprotected rename (`research/critiques/C-005-M-08.md` -> `notes/...`): FAIL, `exit=1`.
- Protected -> protected rename inside `research/critiques/`: PASS, `exit=0`.
- Delete real canonical RFC `docs/rfc/CT-RFC-01-market-data-expansion.md`: FAIL, `exit=1`.
- Historical RFC path `docs/contract-rfc/CT-RFC-PROBE.md` add->delete: FAIL, `exit=1`.
- Same-commit `ALLOW-ARTIFACT-DELETE:` on protected milestone delete: PASS, `exit=0`.
- Later override marker after earlier protected milestone delete: FAIL, `exit=1`.
- Merge commit deletes protected milestone: FAIL, `exit=1`.
- Merge commit moves protected verdict to unprotected `notes/`: FAIL, `exit=1`.
- `merge -s ours` drops side-only protected verdict: FAIL, `exit=1`.

**False-positive / legitimate probes:**
- Benign merge with no protected loss: PASS, `exit=0`.
- Protected rename chain inside `research/critiques/`: PASS, `exit=0`.
- Performance with +120 empty commits: PASS, `exit=0`, ~0.04s wall time in local probe.

**Residual NOTE:**
- Merge commit deletes protected milestone with same-commit `ALLOW-ARTIFACT-DELETE:`: FAIL, `exit=1`. This is conservative and does not permit silent loss, but it means intentional protected deletion should be a normal explicit commit, not hidden in a merge commit.

## Rev 6 Other Checks

- CI wiring: PASS. `protected-artifacts` remains in `status-check.needs`; it is not optional.
- Verdict trail: PASS. Original C-006, rev3, rev4, and rev5 verdict sections/commits are present.
- Founder-priority non-drift: PASS. `efe2ccc..e3bfc42` does not touch `milestones/BACKLOG.md`, `docs/DESIGN.md`, `docs/fa/ops.md`, `PROJECT-STATE.md`, `TECH-DEBT.md`, or `docs/SESSION-HANDOFF.md`.
- Scope: PASS. My change is limited to this verdict artifact under `research/critiques/`.

## Rev 6 Recommendation

Proceed to reviewer with NOTE. Reviewer should merge `docs/td021-rules` before/with `docs/doc-gate` as appropriate and keep an eye on `.claude/rules/*` conflicts. Founder ★ remains required for P2.5 acceptance, BACKLOG queue, and HL-depth / first-live-signal fork.

## Rev 6 Confidence

High. The probe set covers every bypass that caused rev2-rev5 REJECT plus the requested RFC/override/merge cases. The remaining limitation is explicitly fail-closed.

=== HANDOFF: critic -> reviewer ===

## §A - Metadata
- UTC datetime: 2026-07-14T16:37Z
- Gate: C-006 doc-gate, rev6 re-audit
- Status: NOTE - REJECT lifted; reviewer merge gate next
- HEAD before critic verdict: `e3bfc42` - `docs(doc-gate): C-006 rev5 - барьер переписан от РЕЗУЛЬТАТА (закрыты обе дыры мержа)`

## §B - What I Checked
- Protected-artifacts script against add->delete, rename-out, both RFC paths, same-commit vs later override, evil merge delete, merge rename-out, and `merge -s ours` side-only artifact drop.
- CI `protected-artifacts` required by `status-check.needs`.
- Verdict trail preservation.
- Founder-priority non-drift.

## §C - Artifacts / Results
- Updated verdict artifact: `research/critiques/C-006-doc-gate.md`
- Verdict: NOTE - REJECT lifted.
- Probe result: all silent-loss blockers fail as required.
- Residual note: merge-commit same-commit override fails closed; use a normal explicit override commit for intentional protected deletion.

## §D - Next Agent + Invocation
- **Next agent:** reviewer.
- **Expected HEAD before reviewer starts:** the pushed rev6 verdict commit containing this section.
- **Push status:** critic must push this verdict commit to `origin/docs/doc-gate` after commit.
- **Paste-ready prompt:**
  ```
  Ты — reviewer. Same C-006 doc-gate cycle.

  Expected HEAD: the pushed rev6 verdict commit on origin/docs/doc-gate. Run:
  git fetch origin
  git rev-parse --short origin/docs/doc-gate
  If HEAD differs, STOP: prompt stale.

  Review `research/critiques/C-006-doc-gate.md` section "Re-audit (rev 6)" and the repair range `efe2ccc..HEAD`.
  Critic verdict: NOTE - REJECT lifted.

  Tasks:
  1. Merge/order branches as needed: `docs/td021-rules` then `docs/doc-gate` (both touch `.claude/rules/`).
  2. Verify protected-artifacts CI remains mandatory.
  3. Confirm no founder priority was changed by agents: P2.5, BACKLOG order, and HL-depth fork remain founder ★.
  4. Route to founder ★ for P2.5 acceptance / BACKLOG queue / HL-depth fork after reviewer merge gate.
  ```

## §E - Risks / Open Questions
- Expected-HEAD should become mandatory in `handoff-block.md` §D; stale prompts happened repeatedly in this cycle.
- Force-push history erasure cannot be fully detected by an in-branch script after rewrite; requires branch protection/no-force-push discipline.
- TD-020 remains time-bound: about 40 days to disk-guard if retention delivery slips.
- Founder ★ pending: P2.5 acceptance, BACKLOG queue, HL-depth / first-live-signal fork.

=== END HANDOFF ===

---

## Re-audit (rev 5) - 2026-07-14T16:07Z

**Audited repair range:** `b13deb2..efe2ccc`
**Expected/audited HEAD:** `efe2ccc` - `docs(doc-gate): C-006 rev4 - барьер ловит «злой мерж» (последний блокер)`
**Worktree:** `/tmp/hft-critic-dg-r5`, local branch `critic-dg-r5` created from `origin/docs/doc-gate` after HEAD check.

## Rev 5 Verdict

**REJECT remains.**

The original rev4 blocker is partially closed: a merge commit that deletes a protected file present in all parents now fails. Two merge-layer bypasses remain:

1. A merge commit can move a protected file to an unprotected path and pass.
2. `merge -s ours` can drop a protected artifact that exists only on the side branch and pass.

Both are protected-artifact loss modes. They are not just theoretical because the next requested reviewer action is a merge of process/doc branches.

## Rev 5 Probe Results

Executed in disposable worktree `/tmp/hft-critic-dg-r5-probe` at `efe2ccc`.

**Closed cases:**
- Baseline audited branch: PASS, `exit=0`.
- Rev4 scenario, merge commit deletes `milestones/M-05-data-foundation.md` while all parents retain it: FAIL, `exit=1`.
- Octopus merge (3 parents) deletes same protected milestone while all parents retain it: FAIL, `exit=1`.
- Side branch deletes protected milestone with same-commit `ALLOW-ARTIFACT-DELETE`, then merge: PASS, `exit=0`; no double FAIL.
- Benign merge with no protected deletion: PASS, `exit=0`.
- Protected -> protected rename in a merge (`docs/rfc/...` to another `docs/rfc/...`): PASS, `exit=0`.
- Branch from another branch with no protected deletion: PASS, `exit=0`.
- First normal commit after doc-gate with no protected deletion: PASS, `exit=0`.
- Performance: branch with 120 additional empty commits ran in ~0.81s wall time; acceptable for CI.

**Still open:**
- Merge commit moves `milestones/M-05-data-foundation.md` to `tmp/probe/M-05-data-foundation.md`: PASS, `exit=0`. This is a protected -> unprotected rename inside a merge and should fail.
- `merge -s ours` discards side-branch `research/critiques/probe-side-only.md`: PASS, `exit=0`. The side branch added a protected artifact; the merge commit omitted it; Layer B misses it because the path is not present in all parents.

**Force-push assessment:**

The script cannot fully detect a force-push that erases prior branch history. After rewrite, the old protected artifact addition/deletion may simply no longer exist in the commit graph available to CI. That is not repairable by this in-branch script alone; it requires branch protection / no-force-push discipline. This is a residual process risk, not the current blocker.

## Why Layer B Is Still Too Narrow

Current Layer B checks only paths deleted relative to the first parent and only treats the merge as malicious when the path exists in all parents. That catches common-file deletion, but misses two important cases:

- `R*` rename status in a merge is ignored, so protected -> unprotected move-out is not treated as deletion.
- Side-only protected artifacts are allowed to disappear in the merge result, especially with `-s ours`, because they do not exist in all parents.

The protected-artifact invariant is about final artifact preservation, not just files common to all parents. If a protected path exists in any parent and is absent from the merge result and final HEAD, the merge needs either to preserve it, move it to another protected path, or carry same-commit `ALLOW-ARTIFACT-DELETE:`.

## Rev 5 Required Repair

1. In Layer B, compare the merge commit against every parent, not only the first parent.
2. Use rename-aware status for merge diffs and apply the same rule as Layer A:
   - protected -> protected rename: PASS.
   - protected -> unprotected rename: FAIL unless same merge commit has `ALLOW-ARTIFACT-DELETE:`.
3. Catch side-only protected artifacts dropped by merge result (`-s ours` case). A protected path present in any parent and absent in merge result/HEAD is loss unless it is accounted for by a same-commit override or a protected -> protected rename in the merge.
4. Preserve already-good behavior: common-file merge delete FAIL; octopus common-file delete FAIL; side-branch legitimate delete with same-commit override PASS; benign merge PASS; 120+ commit performance remains acceptable.

## Rev 5 Other Checks

- Verdict trail: PASS. Original C-006, rev3, and rev4 sections are present. Branch history includes `00244ae`, `a61856d`, and `8a2fc89` for verdict commits.
- Founder priorities: PASS. `b13deb2..efe2ccc` does not touch `milestones/BACKLOG.md`, `docs/DESIGN.md`, `docs/fa/ops.md`, `PROJECT-STATE.md`, or `TECH-DEBT.md`.
- CI wiring: PASS. `protected-artifacts` remains in `status-check.needs`.

## Rev 5 Process Note

I still agree that `handoff-block.md` should make expected HEAD mandatory in §D, with receiving agents instructed to run `git log --oneline -1` and STOP on mismatch. This is class A process hardening and should not be smuggled in, but it directly addresses the stale-prompt failure mode observed twice in this cycle.

## Rev 5 Confidence

High. I read the repair diff and ran probes for common-file merge delete, octopus delete, merge rename-out, `-s ours` side artifact drop, side-branch legitimate override delete, benign merge, protected -> protected merge rename, branch-from-branch, first-commit branch, and 120-commit performance.

=== HANDOFF: critic -> architect ===

## §A - Metadata
- UTC datetime: 2026-07-14T16:07Z
- Branch audited: `origin/docs/doc-gate` at `efe2ccc`
- Local verdict branch: `critic-dg-r5`
- Status: REJECT remains
- Audited range: `b13deb2..efe2ccc`

## §B - What I Checked
- The rev4 merge-commit blocker and required bypass/false-positive probes.
- Verdict preservation.
- Founder-priority non-drift.
- CI protected-artifacts wiring.

## §C - Outcome
- Closed: common-file malicious merge delete, octopus common-file delete, same-commit override, benign merge, protected -> protected merge rename, performance.
- Blocking: merge protected -> unprotected rename passes; `merge -s ours` can drop side-only protected artifacts.

## §D - Next Agent + Invocation
- **Next agent:** architect, same C-006 doc-gate repair loop.
- **After repair:** critic rev6 re-audit. If REJECT is lifted, hand off to reviewer to merge `docs/td021-rules` then `docs/doc-gate`, then founder ★ for P2.5 / BACKLOG queue / HL-depth fork.
- **Paste-ready prompt for architect:**
  ```
  Ты — architect on docs/doc-gate. Same C-006 doc-gate repair loop, not a new gate.

  Expected HEAD before repair: efe2ccc. Run:
  git fetch && git worktree add /tmp/hft-architect-dg-r6 origin/docs/doc-gate && cd /tmp/hft-architect-dg-r6
  git log --oneline -1
  If HEAD is not efe2ccc, STOP: prompt stale.

  Read `research/critiques/C-006-doc-gate.md` section "Re-audit (rev 5)".
  Repair only the remaining protected-artifacts merge-layer blockers:

  Required:
  1. Merge Layer B must detect protected -> unprotected rename/move-out in merge commits.
  2. Merge Layer B must detect `merge -s ours` / side-only protected artifact drops: protected path exists in any parent, absent in merge result and HEAD.
  3. Preserve accepted cases: common-file merge delete FAIL; octopus common-file delete FAIL; protected -> protected merge rename PASS; side-branch legitimate delete with same-commit override PASS; benign merge PASS; current Layer A behavior unchanged.
  4. Keep CI mandatory and performance acceptable on 100+ commits.

  Optional class A process follow-up remains separate unless you intentionally include it:
  make expected HEAD mandatory in `handoff-block.md` §D.

  Do not reorder founder priorities. Leave P2.5, BACKLOG order, and HL fork as founder ★.
  Commit repairs, then hand back to critic for rev6 re-audit.
  ```

## §E - Risks / Open Questions
- Force-push history erasure is not fully detectable by this script after the rewrite; needs branch protection/no-force-push policy.
- TD-020 remains time-bound: about 40 days to disk-guard if retention delivery slips.
- Founder ★ still pending: accept P2.5, confirm BACKLOG queue, decide HL-depth / first-live-signal fork.

=== END HANDOFF ===

---

## Re-audit (rev 4) - 2026-07-14T15:48Z

**Audited repair range:** `6fd3081..b13deb2`
**Expected/audited HEAD:** `b13deb2` - `docs(doc-gate): C-006 rev3 - барьер артефактов переписан (покоммитно, переименования, override)`
**Worktree:** `/tmp/hft-critic-dg-r4`, local branch `critic-dg-r4` created from `origin/docs/doc-gate` after HEAD check.

## Rev 4 Verdict

**REJECT remains.**

The rev3 blocker is mostly repaired: add->delete, protected->unprotected rename, same-commit override, RFC path coverage, and CI wiring now work. The remaining blocker is the acknowledged `--no-merges` gap: a merge commit can delete a protected milestone without any non-merge commit deleting it, and the current script passes.

## Rev 4 Checks

**1. Commit-by-commit barrier - CLOSED except merge commits.**

Script now computes `merge-base(origin/main, HEAD)` and scans commits, not only final net diff. Probe results from disposable worktree at `b13deb2`:
- Clean branch: PASS, `exit=0`.
- Branch-local `research/critiques/probe-add-delete.md` add->delete: FAIL, `exit=1`.
- `research/critiques/C-006-doc-gate.md` move to `tmp/probe/...`: FAIL, `exit=1`.
- Protected->protected RFC rename inside `docs/rfc/`: PASS, `exit=0`.
- Later override marker after an earlier deletion: FAIL, `exit=1`.
- Same-commit `ALLOW-ARTIFACT-DELETE:` on deletion: PASS, `exit=0`.
- `docs/rfc/*` deletion: FAIL, `exit=1`.
- `docs/contract-rfc/*` add->delete: FAIL, `exit=1`.
- Delete->restore of C-006: PASS, `exit=0`, with NOTE.
- Nested protected path and protected symlink deletion: FAIL, `exit=1`.
- Benign merge commit with no protected deletion: PASS, `exit=0`.

**2. HEAD-presence criterion - ACCEPTED.**

The rule "deleted/moved and absent on HEAD" does not weaken the deletion barrier for its stated purpose: preventing final loss of gate/milestone/RFC artifacts. It correctly avoids permanent red on honest delete->restore history such as `139b399 -> 352b1db`. It does not protect content integrity after restore, but that is a different control surface; semantic edits to protected docs remain governed by class A review.

**3. Merge-commit hole - STILL BLOCKING.**

I reproduced the hole:

- Created a merge commit whose parents both retained `milestones/M-05-data-foundation.md`.
- Deleted `milestones/M-05-data-foundation.md` only in the merge commit.
- Ran `bash scripts/check_protected_artifacts.sh origin/main`.
- Result: PASS, `exit=0`.

Reason: `git rev-list --no-merges` skips the merge commit, and no non-merge commit in the range has a `D` for that path. This matters now, not theoretically: the next reviewer step is explicitly a merge of `docs/td021-rules` and `docs/doc-gate`, and reviewer merges are exactly where protected artifacts can be lost through conflict resolution or an over-broad index.

Required repair:
- Include merge commits in the scan.
- For merge commits, inspect the merge commit's tree against at least the first parent for protected deletions / protected->unprotected renames. If the script wants to avoid duplicate failures for side-branch deletions already inspected, it can still special-case "same deletion already failed/allowed in a non-merge commit"; but skipping merges entirely is not acceptable for this gate.
- Keep the HEAD-presence exception: if a later commit restores the protected path by final HEAD, emit NOTE rather than FAIL.

**4. Branch/base cases - NOTE.**

The real audited branch is already behind/diverged from current `origin/main` (`merge-base` = `2542d2a`), and the script handled that cleanly. The first commit in the branch range is scanned. A benign merge passed. Force-push cannot be fully solved by an in-branch script: if a force-push erases the commit that added a branch-local artifact and the artifact is absent from the new history, CI has no prior remote state to compare. That needs repository branch protection / no-force-push policy, not only this script.

**5. Rev1/rev3 verdict preservation - PASS.**

`research/critiques/C-006-doc-gate.md` preserves the original C-006 verdict and the rev3 section. Branch history includes:
- `00244ae` - original C-006 verdict.
- `a61856d` - `docs(critic): C-006 re-audit rev3 - REJECT`.

**6. FA historical notes - PASS.**

The seven FA notes added in `alpha`, `oms`, `portfolio`, `risk`, `strategy`, `venues`, plus the existing `killswitch` note, do not rewrite module semantics. They correctly warn that old `M-05`/`M-06` numbering is historical and point agents to `milestones/BACKLOG.md` and `docs/DESIGN.md` §10 with P2.5 inserted before P3.

**7. Founder priorities - PASS.**

`6fd3081..b13deb2` does not touch `milestones/BACKLOG.md`, `docs/DESIGN.md`, or `docs/fa/ops.md`. P2.5, queue order, and the HL-depth fork remain founder ★ decisions.

## Rev 4 Required Repair

1. Remove the `--no-merges` blind spot. Protected deletions / protected->unprotected renames introduced by a merge commit must fail when the protected path is absent on HEAD.
2. Keep the current successful behavior for add->delete, rename-out, protected->protected rename, same-commit override, both RFC paths, delete->restore, nested paths, and symlink entries.
3. Add a regression probe or documented manual test for "merge commit deletes protected milestone" so this specific gap cannot reappear.

## Rev 4 Additional Process Note

I agree with architect's proposed class A follow-up: `handoff-block.md` should require an explicit "expected HEAD" in §D plus an instruction to run `git log --oneline -1` and STOP on mismatch. This is not the protected-artifacts blocker, but it addresses the repeated stale-prompt failure mode and belongs in the same doc-gate/process-hardening family.

## Rev 4 Confidence

High. I read the repair diff and ran probes for add->delete, rename-out, protected->protected rename, same-commit override, later override, both RFC paths, delete->restore, nested path, symlink path, benign merge, and malicious merge.

=== HANDOFF: critic -> architect ===

## §A - Metadata
- UTC datetime: 2026-07-14T15:48Z
- Branch audited: `origin/docs/doc-gate` at `b13deb2`
- Local verdict branch: `critic-dg-r4`
- Status: REJECT remains
- Audited range: `6fd3081..b13deb2`

## §B - What I checked
- Protected-artifacts script behavior against required bypasses and false positives.
- CI `protected-artifacts` still included in `status-check.needs`.
- Rev1/rev3 verdict preservation.
- FA historical notes.
- Founder priority non-drift.

## §C - Outcome
- Closed: add->delete, rename-out, protected->protected rename, both RFC paths, same-commit override, CI wiring, delete->restore exception, stale FA numbering notes.
- Blocking: merge commits are skipped; a merge commit can delete a protected milestone and pass.

## §D - Next Agent + Invocation
- **Next agent:** architect, same C-006 doc-gate repair loop.
- **After repair:** critic rev5 re-audit. If REJECT is lifted, hand off to reviewer to merge `docs/td021-rules` then `docs/doc-gate`, then founder ★ for P2.5 / BACKLOG queue / HL-depth fork.
- **Paste-ready prompt for architect:**
  ```
  Ты — architect on docs/doc-gate. Same C-006 doc-gate repair loop, not a new gate.

  Expected HEAD before repair: b13deb2. Run:
  git fetch && git worktree add /tmp/hft-architect-dg-r5 origin/docs/doc-gate && cd /tmp/hft-architect-dg-r5
  git log --oneline -1
  If HEAD is not b13deb2, STOP: prompt stale.

  Read `research/critiques/C-006-doc-gate.md` section "Re-audit (rev 4)".
  Repair only the remaining blocker: protected-artifacts merge-commit blind spot.

  Required:
  1. Include merge commits in the protected-artifacts scan.
  2. Fail a merge commit that deletes a protected path or moves protected -> unprotected when the protected path is absent on HEAD.
  3. Preserve current accepted behavior: add->delete FAIL, rename-out FAIL, protected->protected rename PASS, same-commit override PASS, later override FAIL, both RFC dirs protected, delete->restore NOTE/PASS.
  4. Add or document a regression probe for "merge commit deletes protected milestone".

  Optional class A process follow-up if you choose to include it in this same repair:
  update `handoff-block.md` so §D must include expected HEAD and the receiving agent must `git log --oneline -1` then STOP on mismatch.

  Do not reorder founder priorities. Leave P2.5, BACKLOG order, and HL fork as founder ★.
  Commit repairs, then hand back to critic for rev5 re-audit.
  ```

## §E - Risks / Open Questions
- Force-push erasure of branch-local artifacts cannot be fully detected by this script after history is rewritten; needs repo branch protection/no-force-push discipline.
- TD-020 remains time-bound: about 40 days to disk-guard if retention delivery slips.
- Founder ★ still pending: accept P2.5, confirm BACKLOG queue, decide HL-depth / first-live-signal fork.

=== END HANDOFF ===

---

## Re-audit (rev 3) - 2026-07-14T13:39Z

**Audited repair range:** `191d5ef..6fd3081`  
**Audited HEAD:** `6fd3081` - `docs(doc-gate): C-006 rev2 - SESSION-HANDOFF мимо P2.5, механический барьер артефактов, класс B`  
**Worktree note:** local `docs/doc-gate` was occupied by an older dirty worktree, so this audit was performed on `critic-docgate-r3` created from `origin/docs/doc-gate` at the requested HEAD.  
**Audit-trail note:** this branch contains the original C-006 verdict but not the previously local rev2 section commit `88318f9`; rev3 therefore restates the rev2 blockers by substance.

## Rev 3 Verdict

**REJECT remains.**

The routing repair in `docs/SESSION-HANDOFF.md` is materially correct, and founder priorities were not reordered. The remaining blocker is the mechanical barrier: as implemented, it does not catch the exact protected-artifact loss class it was introduced to prevent.

## Rev 3 Checks

**1. SESSION-HANDOFF routing - CLOSED.**

`docs/SESSION-HANDOFF.md:101-137` now explicitly says not to dispatch `risk / killswitch / oms / runner / testnet`, states that the old `M-08 = risk + killswitch + oms` wording is obsolete, and points the active queue to `BACKLOG` + `DESIGN` §10. It also surfaces founder ★ decisions for P2.5 and the HL-depth fork. A fresh agent reading the active handoff should not enter the trading stack before P2.5.

Residual notes:
- `docs/SESSION-HANDOFF.md:139-156` keeps the old text under an explicit archive heading; acceptable.
- Stale FA numbering remains in module docs: `docs/fa/risk.md`, `docs/fa/oms.md`, `docs/fa/alpha.md`, `docs/fa/portfolio.md`, `docs/fa/strategy.md`, and `docs/fa/venues.md` still contain old `P3 (M-05)`, `M-05/M-06`, or `48ч testnet-MM` wording without the historical warning now added to `docs/fa/killswitch.md`. This no longer routes the next agent past P2.5, but it can mislead later module dispatch. Treat as NOTE: apply the same "historical numbering, BACKLOG is current" note before any risk/oms/runner milestone handoff.

**2. Mechanical protected-artifact barrier - STILL BLOCKING.**

Claimed mechanism: `scripts/check_protected_artifacts.sh` checks `git diff --name-status origin/main...HEAD` and rejects `D` under `research/critiques/*.md`, `milestones/*.md`, `docs/rfc/*`, unless any commit message in the range contains `ALLOW-ARTIFACT-DELETE:`.

Probe results from a disposable worktree at `6fd3081`:
- Clean branch: PASS, `exit=0`.
- Delete an existing main-branch milestone (`milestones/M-05-data-foundation.md`): FAIL, `exit=1`. This case works.
- Delete `research/critiques/C-006-doc-gate.md`: PASS, `exit=0`. This is the exact class of file the barrier was introduced to protect, and it is missed because the file is branch-local and absent from `origin/main`; add-then-delete inside the branch disappears from the final net diff.
- Move `research/critiques/C-006-doc-gate.md` to `tmp/probe/C-006-doc-gate.md`: PASS, `exit=0`. This is a trivial namespace escape; final net diff only shows `A tmp/probe/...`.
- Delete an existing milestone in one commit, then add an unrelated later commit with `ALLOW-ARTIFACT-DELETE:` in the body: PASS, `exit=0`. Override is range-wide, not tied to the deleting commit.
- Delete `docs/contract-rfc/CT-RFC-01-market-data-expansion.md`: PASS, `exit=0`. The script protects `docs/rfc/*`, but the repo also has real RFCs under `docs/contract-rfc/*`.

Why this blocks: the stated goal is to prevent silent loss of gate/milestone/RFC artifacts. A net diff against `origin/main...HEAD` is not enough; it misses branch-local add/delete, branch-local move-out, and any protected path not listed exactly.

Required repair:
- Compute `merge_base=$(git merge-base "$BASE" HEAD)` and inspect commit history over `"${merge_base}..HEAD"`, not only final net diff.
- Fail per commit on `D` of protected paths.
- Inspect renames with rename detection; allow protected -> protected rename, but fail protected -> unprotected move unless the same commit has an explicit override.
- Include both `docs/rfc/*` and `docs/contract-rfc/*`, or define one canonical RFC path and migrate the other before enforcing.
- Scope `ALLOW-ARTIFACT-DELETE:` to the same commit that deletes/moves the artifact, preferably with path/reason, not any commit in the range.
- Keep CI as mandatory; pre-commit may be additive but is not sufficient.

**3. Class B mirror - CLOSED.**

`.claude/rules/commit-discipline.md:84-92` now mirrors the non-semantic class B boundary: status columns, close-out proofs, formatting, prose spelling only; semantic command/path/threshold/role/invariant/task/acceptance/cross-reference edits are class A.

**4. Founder priorities - PASS.**

The rev3 repair range does not touch `milestones/BACKLOG.md`, `docs/DESIGN.md`, or `docs/fa/ops.md`. P2.5 remains PROPOSED, the queue order is not rewritten, and the HL-depth / first-signal fork remains founder ★.

## Rev 3 Required Repairs

1. Replace the protected-artifacts check with a commit-history check that catches branch-local add/delete and protected -> unprotected rename.
2. Protect the actual RFC paths present in this repo (`docs/rfc/*` and `docs/contract-rfc/*`, unless one is intentionally retired).
3. Tie `ALLOW-ARTIFACT-DELETE:` to the deleting/moving commit, not to any later commit in the range.
4. NOTE before later trading-stack dispatch: add the same historical-numbering warning from `docs/fa/killswitch.md` to stale `risk`/`oms`/`alpha`/`portfolio`/`strategy`/`venues` FA sections.

## Rev 3 Confidence

High. I read the repair diff, `SESSION-HANDOFF`, `commit-discipline`, `gates`, `ci.yml`, `check_protected_artifacts.sh`, `killswitch` FA, and repo-wide P2.5/P3/M-05/M-06 references. I also executed the protected-artifacts script against clean, delete, rename-out, override, and real-RFC deletion probes.

=== HANDOFF: critic -> architect ===

## §A - Metadata
- UTC datetime: 2026-07-14T13:39Z
- Branch audited: `origin/docs/doc-gate` at `6fd3081`
- Local verdict branch: `critic-docgate-r3`
- Status: REJECT remains
- Audited range: `191d5ef..6fd3081`

## §B - What I checked
- Rev2 blocker 1: active `SESSION-HANDOFF` routing past P2.5.
- Rev2 blocker 2: mechanical protected-artifacts barrier, including deletion, rename/move-out, override, base behavior, and real RFC paths.
- Class B mirror in `commit-discipline`.
- Founder priority preservation in `BACKLOG`/`DESIGN`/P2.5.

## §C - Outcome
- `SESSION-HANDOFF` repair: accepted.
- Class B mirror: accepted.
- Founder priorities: unchanged.
- Mechanical barrier: rejected. It misses branch-local verdict deletion and protected -> unprotected move-out, and does not protect `docs/contract-rfc/*`.

## §D - Next Agent + Invocation
- **Next agent:** architect, same C-006 doc-gate repair loop.
- **After repair:** critic rev4 re-audit. If REJECT is lifted, hand off to reviewer to merge `docs/doc-gate` + `docs/td021-rules`, then founder ★ for P2.5 / queue / HL fork.
- **Paste-ready prompt for architect:**
  ```
  Ты — architect on docs/doc-gate. Same C-006 doc-gate repair loop, not a new gate.

  Read `research/critiques/C-006-doc-gate.md` section "Re-audit (rev 3)".
  Repair only the remaining blocker: protected-artifacts CI.

  Required:
  1. Replace net-diff-only `git diff --name-status origin/main...HEAD` with commit-history inspection from merge-base to HEAD so branch-local add→delete is caught.
  2. Fail protected -> unprotected rename/move-out; allow only protected -> protected rename without override.
  3. Protect both `docs/rfc/*` and `docs/contract-rfc/*`, or explicitly migrate to one canonical path before enforcing.
  4. Require `ALLOW-ARTIFACT-DELETE:` in the same commit that deletes/moves the artifact, not anywhere in the range.
  5. Keep CI mandatory; pre-commit may be optional only.

  Optional NOTE cleanup before later trading-stack dispatch:
  add the same historical-numbering warning from `docs/fa/killswitch.md` to stale risk/oms/alpha/portfolio/strategy/venues FA sections.

  Do not reorder founder priorities. Leave P2.5 and HL fork as founder ★.
  Commit repairs, then hand back to critic for rev4 re-audit.
  ```

## §E - Risks / Open Questions
- Three branches remain in flight: `docs/doc-gate`, `docs/td021-rules`, `feat/M-08-closeout`. Reviewer must merge in an order that does not lose `PROJECT-STATE`/`TECH-DEBT`.
- TD-020 remains the only hard deadline pressure: about 40 days to disk-guard if retention is not wired.
- Founder ★ still needed for P2.5 acceptance and HL-depth / first-live-signal fork.

=== END HANDOFF ===
