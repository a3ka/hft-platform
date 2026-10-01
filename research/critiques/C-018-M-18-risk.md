# C-018 — M-18 L2Delta risk-critic verdict

**UTC:** 2026-07-21T11:04:14Z  
**Agent:** risk-critic  
**Branch audited:** `origin/feat/M-18-l2delta` at `39f6515`  
**Base:** `d520dcb` (`origin/main`)  
**Worktree:** `/tmp/hft-riskcritic-m18` (detached at `origin/feat/M-18-l2delta`)  
**Verdict:** CONCERNS

## Scope Read

Read:

- `docs/rfc/CT-RFC-04-l2delta.md`
- `milestones/M-18-l2delta-capture.md`
- `crates/contracts/tests/red_rfc04.rs`
- `git diff origin/main..HEAD -- crates/contracts/src/lib.rs`

Risk context read:

- `CLAUDE.md`
- `.claude/agents/risk-critic.md`
- `.claude/rules/gates.md`
- `.claude/rules/scope-guard.md`
- `docs/DESIGN.md` §1/§4/§6
- `docs/03-integration-contract.md` §6
- `docs/fa/{risk,venues,sim,journal}.md`

I did not work in `/tmp/hft-arch-m18`; that worktree is still the checked-out owner of `feat/M-18-l2delta`.

## Commands

```text
cargo test -p contracts --test red_rfc04 -- --nocapture; echo exit=$?
running 7 tests
... 7 passed
exit=0

bash scripts/verify_M-18.sh; echo exit=$?
PASS  CT-RFC-04 red_rfc04 (...)
PASS  CT-I-4 red_schema (...)
PASS  MdPayload::L2Delta присутствует в contracts
PASS  SEGMENT_MAGIC=HFTJRN02 и SCHEMA_VERSION=2 НЕ тронуты (...)
PASS  CHANGELOG несёт запись CT-RFC-04
PASS  фикстура fixtures/valid/event-l2delta-spot.json
PASS  фикстура fixtures/valid/event-l2delta-futures.json
PASS  фикстура fixtures/invalid/event-l2delta-missing-final-id.json
FAIL  venue-binance red_l2delta_capture (...)
FAIL  venue-binance-futures red_l2delta_futures (...)
FAIL  venue-binance: l2delta_event только ОПРЕДЕЛЁН, не вызван (...)
FAIL  venue-binance-futures: l2delta_event только ОПРЕДЕЛЁН, не вызван (...)
FAIL  journal red_l2delta_persist (...)
FAIL  workspace собирается со всеми armами L2Delta (...)
VERDICT: FAIL (6)
exit=1
```

The verify failure is the expected RED-first state before venue-dev / engine-dev implementation, not a risk finding by itself.

## Risk Verdict

CONCERNS, not KILL.

The M-18 plan is safe on the requested primary checklist: BTC-only makes the volume increase bounded, the current committed diff is MD-only, sim is required to ignore L2Delta, magic/version remain stable, and discriminants 0..5 are pinned by a real historical blob test.

The blocking concern is deployment rollback compatibility after the first L2Delta is written. M-18 explicitly relies on `new-code reads old journal`; it does not require `old-code reads new journal`. That is acceptable as a contract rule only if the deploy/rollback procedure forbids rolling the recorder back to a pre-M18 binary after L2Delta events have entered the persistent journal.

## Checklist

### 0. Volume / Retention

PASS with a bounded note.

RFC §5 estimates the full 4-symbol plan as:

- current baseline: about `2.8 GB/day`
- full L2Delta add: about `+1.5..2.5 GB/day`
- full total: about `4.5..5.5 GB/day`
- disk timer: about `40 days -> 20 days`

Founder selected option `(a)`: BTCUSDT only, spot + perp. That halves the incremental L2Delta stream before implementation. Using the RFC's own range, BTC-only is roughly:

- add: about `+0.75..1.25 GB/day`
- total: about `3.55..4.05 GB/day`
- 111 GB free timer: roughly `27..31 days`

So "disk timer almost does not accelerate" is optimistic wording. It still shortens materially from about 40 days. But this is not a KILL: the time horizon is weeks, not hours, and the failure mode is stopped data capture, not unsafe trading.

Disk-guard + heartbeat are sufficient as a safety backstop for starting before TD-020 retention, if reviewer §8 treats these as hard checks:

- `storage_status().writable == true`
- `free_bytes` visible and sane
- observed write-rate within BTC-only budget
- BTC L2Delta present for spot + futures
- non-BTC L2Delta absent

TD-020 remains urgent next work, not optional cleanup.

### 1. MD-only / Order Path

PASS.

The committed branch diff contains no implementation changes under `crates/venue-*/src`, `crates/recorder/src`, `crates/journal/src`, or `crates/sim/src`; it only adds T1 contract artifacts, RED tests, RFC/milestone, verify, and critic verdict.

`crates/risk`, `crates/killswitch`, and `crates/oms` do not exist in this repo state. A source scan over Binance venue adapters found no `submit`, `cancel`, `auth`, `signature`, API-key, `OrderGateway`, `RiskApproved`, or placement surface. M-18 tasks also forbid order-path changes and limit venue-dev to MD emit.

### 2. Backtest Inertness

PASS as plan/gate.

RFC §6 and milestone task 5 require `sim/src/exchange.rs` to add:

```rust
MdPayload::L2Delta { .. } => {}
```

That is the correct risk posture. The existing sim fills from `L2Snapshot + Trade`; feeding raw book deltas into the current fill path would double-count book motion and make backtests more optimistic or simply different from live. `cargo build --workspace --all-targets` remains the compiler canary for the exhaustive arm.

### 3. Magic / Version / Prod Old Segments

PASS for the asked direction: new code reading existing production segments.

`SEGMENT_MAGIC` remains `HFTJRN02`, `SCHEMA_VERSION` remains `2`, and `verify_M-18.sh` greps both. RFC §3 correctly states that segment framing is unchanged and that existing segments with variants 0..5 must remain readable. `red_rfc04` passed.

### 4. Additive Discriminants / Historical Blob

PASS.

`MdPayload::L2Delta` is appended after `MarginRate`; `red_rfc04` pins postcard discriminants:

- `Trade = 0`
- `L2Snapshot = 1`
- `Funding = 2`
- `OpenInterest = 3`
- `Liquidation = 4`
- `MarginRate = 5`
- `L2Delta = 6`

The CT-I-3 historical blob is non-circular enough for this gate: it is an embedded byte array and a hand-coded expected `L2Snapshot`, not current-code reserialization.

## Blocking Concern

### R1 — Rollback to pre-M18 binary after first L2Delta can corrupt or stall recorder recovery

M-18's compatibility contract is one-way: new code reads old journal. RFC §6 explicitly says old-code/new-journal compatibility is not required. That is normal for a T1 additive enum, but the deploy workflow still has rollback semantics:

- `.github/workflows/deploy.yml` resets to previous SHA and rebuilds recorder if the new deploy does not become healthy.
- The journal volume is persistent.

Once a new recorder writes `MdPayload::L2Delta` into the active segment, a pre-M18 binary does not know enum discriminant 6. In current journal recovery, `scan_tail_for_last_seq` / `tail_last_seq_of` deserialize `Event` payloads while scanning the segment tail. Unknown variant 6 is a postcard deserialize error. Depending on flush/meta state and tail contents, rollback can at minimum fail to consume the newest records, and in a bad crash window can recover `next_seq` from an older known event or meta. That risks duplicate seq or a recorder that cannot inspect/diagnose the active segment cleanly.

This does not endanger order submission because M-18 is MD-only. It does endanger the journal integrity path, which is sacred for this system.

Required repair before merge:

1. Add an explicit M-18 deploy/rollback gate: after first L2Delta write, rollback target must be M-18-aware. Do not automatically rollback the recorder to a pre-M18 binary against the same persistent journal.
2. If rollback to pre-M18 is ever needed, require a named operator procedure: stop recorder, preserve/quarantine the post-M18 active segment, and only then start old code; no silent `git reset "$PREV" && docker compose up` over a journal containing variant 6.
3. Put this in the milestone §8 reviewer gate or deployment notes so reviewer can verify it before close-out.

This is a CONCERNS-level finding: easy to close with a procedural gate, but it should not be left implicit.

## Non-blocking Notes

- BTC-only is acceptable for starting before TD-020 retention, but the text "disk timer almost does not accelerate" should not be used as an operational assumption. The actual acceptance condition is measured write-rate and free-space timer in §8.
- §8 must verify both sides of the founder decision: BTC L2Delta present and non-BTC L2Delta absent. If non-BTC appears, rollback/fix; do not treat it as a harmless extra data stream.
- This verdict does not replace critic C-017. Critic has already approved the plan-time RED/gate repair; this is the separate RISK-BLOCK verdict.

## Cleanup

No temporary source changes were made. The worktree was clean before writing this risk verdict.

---

## Re-audit C-018 Resolution

**UTC:** 2026-07-21T11:28:07Z  
**Agent:** risk-critic  
**Branch audited:** `origin/feat/M-18-l2delta` at `e92e7a3`  
**Worktree:** `/tmp/hft-riskcritic-m18-r2` (detached at `origin/feat/M-18-l2delta`)  
**Verdict:** CONCERNS

### Closure Checks

The original rollback blocker is partially closed.

PASS: `red_l2delta_rollback_boundary` is reachable. Baseline RED fails at the expected
pre-implementation E0004 in `journal::segment_last_ts`. With the five temporary RFC §6 arms
(`journal/segments` ts, `sim/exchange` ignore, recorder label `"l2delta"`, dump ignore,
`latency_probe` continue), this command passes:

```text
cargo test -p journal --test red_l2delta_rollback_boundary -- --nocapture
running 1 test
test l2delta_isolated_in_new_provenance_segment ... ok
test result: ok. 1 passed
```

PASS: anti-placebo for the actual invariant works. Temporarily changing
`decide_open_segment` to ignore `header.provenance` makes the same test fail with:

```text
M-18 provenance ОБЯЗАН открыть НОВЫЙ сегмент ... получено сегментов: 1
test result: FAILED. 0 passed; 1 failed
```

PASS: `verify_M-18.sh` now includes `red_l2delta_rollback_boundary` and a structural
runbook check. On the RED branch it still exits `1`, as expected before venue-dev/engine-dev:

```text
FAIL  journal red_l2delta_rollback_boundary (...)
PASS  rollback-runbook задокументирован (RFC §10 + ops.md §5.1) — C-018 mitigation
VERDICT: FAIL (7)
```

PASS: milestone task 6 now carries the live rollback check: first BTC L2Delta must land in
a new M-18-provenance segment; post-M18 segment must be identifiable; reviewer must not
treat deploy auto-rollback as blind safety.

NOTE: startup schema-guard is recorded as reviewer-owned follow-up in RFC §10 and milestone
gates, but not as a concrete `TECH-DEBT.md` entry in this branch. That is acceptable only
because `TECH-DEBT.md` is reviewer-owned post-merge; reviewer must assign the actual TD at
merge.

### Remaining Blocking Concern

R2 — `ops.md` §5.1 / RFC §10 overclaim the re-forward step after quarantine.

The quarantine procedure is executable for the narrow rollback boundary:

1. stop recorder;
2. move post-M18 segments identified by provenance git-sha / `created_wall_ms` out of the active journal;
3. start pre-M18 binary against a clean pre-M18 tail.

But the documented step "return quarantined segments on re-forward; seq is continuous" is false
if the pre-M18 binary writes any event while the M-18 segment is quarantined. Temporary probe:

```text
pre-M18 wrote seq 0..4
M-18 wrote quarantined segment seq 5..6
pre-M18 rollback wrote one Trade after quarantine
after restoring quarantined segment, read order was [0, 1, 2, 3, 4, 7, 5, 6]
```

Reason: `journal.meta` remains advanced past the quarantined segment, so the old binary can
write seq `7` into the pre-M18 segment. Reattaching the quarantined segment by filename then
puts seq `5..6` after seq `7` in segment-index order. In a rotation case it can also collide
with a newly-created `segment-00000001.jrnl`.

This is not the original "old binary decodes L2Delta and reuses seq" failure; that part is
closed by provenance isolation. It is still a rollback-safety defect because the runbook gives
an operator a false recovery action for the preserved L2Delta data.

Required repair before PASS:

1. Amend `docs/fa/ops.md` §5.1 and RFC §10: quarantined post-M18 segments must not be blindly
   returned to the active journal after the pre-M18 binary has written anything.
2. State one allowed policy explicitly:
   rollback is no-write/emergency only until re-forward, OR quarantined segments remain a forked
   cold artifact requiring a named reconciliation/import procedure.
3. Extend milestone task 6 rollback-check: if rollback occurred, reviewer must prove no pre-M18
   writes happened before reattach, otherwise reattach is forbidden.

Risk verdict stays CONCERNS, not KILL: the MD-only/order-path, sim-inertness, magic/version,
BTC-only volume posture, discriminant pinning, and original provenance-boundary RED are sound.
The remaining issue is a procedural false guarantee in the rollback runbook.

---

## Re-audit rev3

**UTC:** 2026-07-21T12:40Z
**Agent:** risk-critic
**Branch audited:** `origin/feat/M-18-l2delta` at `de8629c`
**Worktree:** `/tmp/hft-riskcritic-m18-r3` (detached at `origin/feat/M-18-l2delta`)
**Verdict:** PASS (conditional on two documented reviewer-owned merge obligations, below — both
already written into the branch as follow-up TD + §8 gate; no code change required to reach PASS)

### What rev3 changed

rev3 (`de8629c`) rewrites RFC §10 and `ops.md` §5.1 and updates milestone L2D-I-8 / risk-critic
note. Diff `24d0426..de8629c` touches ONLY `docs/rfc/CT-RFC-04-l2delta.md`, `docs/fa/ops.md`,
`milestones/M-18-l2delta-capture.md` — no code, no test, no verify change. The prior `concern-2`
(runbook prescribed a re-forward step that silently corrupts) is addressed by DELETING the false
promise and replacing it with an honest one-way policy.

### concern-2 — CLOSED (the defect was a lying runbook; the lie is gone)

rev2's defect was that `ops.md` §5.1 instructed the operator to "return quarantined segments on
re-forward; seq is continuous" — an action that silently disorders the live journal
(`[0,1,2,3,4,7,5,6]`, my rev2 probe). rev3 removes that step entirely. The replacement policy is
one-way (fix-forward preferred; post-M18 data = terminal archive of a separate epoch, offline
research only; re-stitch into the live journal FORBIDDEN; preserve `journal.meta` high-water so the
old binary does not reuse seq values). I verified every load-bearing claim against the actual code,
not just the prose:

1. **meta-preserve → seq-gap not seq-reuse.** `resolve_next_seq_with` (`crates/journal/src/
   segments.rs:1127-1138`) computes `next_seq = meta_seq.max(tail_last_seq_of(latest_segment)+1)`.
   After the M-18 segment is moved to the archive, `latest_segment` is the pre-M18 segment; with
   `journal.meta` preserved at the high-water, the old binary starts at `max(meta, pre_tail+1) =
   meta` → a GAP over the archived range, not a reuse. Claim is code-accurate.
2. **re-stitch breaks `first_seq`.** `stream` / `read_all` assemble segments in INDEX order
   (`segments()` → `iter_segments_sorted`, `segments.rs:586`, `846-862`) with a blind append and NO
   monotonic-`first_seq` check. A returned archive segment carrying lower seqs at a higher index is
   yielded out of order silently. Claim is code-accurate — and it is exactly why re-stitch must be
   forbidden until a machine barrier exists (see TD-b).
3. **"data is not lost" + "terminal archive is a separate epoch, does not weaken DET-I-1."** The
   archived segment is a contiguous, CRC-framed, provenance-tagged segment; read on its own it is
   internally seq-monotonic. DET-I-1 (bit-identical replay) is a per-journal-stream property; a cold
   forked epoch replayed by itself is consistent, and it is never spliced into the live stream. First-
   window L2Delta lives in the archive; rollback-window data lives in the live journal. Correct.

### Reachability + anti-placebo — RE-VERIFIED by prototype run (not taken on trust)

Baseline: `cargo build -p journal --tests` fails at the expected pre-impl `E0004` in
`segment_last_ts` (`segments.rs:1515`) — normal RED phase.

- Added ONLY the temporary `MdPayload::L2Delta { ts_exch_ms, .. } => *ts_exch_ms` arm to
  `first_event_data_ts`. `cargo test -p journal --test red_l2delta_rollback_boundary` → `1 passed`
  (`l2delta_isolated_in_new_provenance_segment ... ok`). Test is reachable and green against a
  correct impl.
- Anti-placebo: with the arm in place, I additionally removed the `header.provenance == cfg.
  provenance` clause from `decide_open_segment` (make reuse ignore provenance). Same test → `FAILED`
  with the documented message `M-18 provenance ОБЯЗАН открыть НОВЫЙ сегмент ... получено сегментов:
  1`. The invariant genuinely bites; it is not a stub-green placebo.
- Restored `crates/journal/src/segments.rs` via `git checkout --`; `git status --porcelain` clean;
  baseline `E0004` back. No impl was committed.

So `concern-1` (silent seq-reuse because a pre-M18 binary can't decode variant 6) remains
MACHINE-enforced by provenance isolation, and the enforcement is real.

### Residual concern → accepted as TD, NOT required in M-18

The one-way policy's ENFORCEMENT is, today, operator discipline (runbook) + machine ISOLATION of
the write boundary (concern-1 RED). There is NO machine barrier stopping an operator who violates
the runbook and re-stitches an archived segment; that barrier is deferred to follow-up **TD-b**
(reader `first_seq`-guard: fail-closed `Err` on non-monotonic `first_seq` in `read_all`/`stream`).
rev3 also carries **TD-a** (recorder startup schema-guard: loud fail if the active segment carries
undecodable events).

Adversarial judgment — deferring TD-b to a separate journal-hardening milestone is correct, and
rushing it into M-18 would be the more dangerous choice:

- **Trigger is narrow and operator-initiated, not silent/automatic.** The residual corruption
  requires an operator to (a) roll a recorder back PAST a schema-forward deploy AND (b) actively
  defy an explicit written prohibition by re-attaching an archived segment. It is not a failure of
  normal operation.
- **MD-only blast radius.** No order-path/risk/oms/money-path is touched (branch diff: only
  `crates/contracts/src/lib.rs` additive enum variant + tests/docs/fixtures; zero `submit/cancel/
  auth/signature/OrderGateway/RiskApproved` tokens; no `venue-*/src`, `recorder/src`, `journal/src`,
  `sim/src` impl). Worst case is out-of-order RESEARCH data, recoverable (archive preserved) — not
  unsafe trading or lost capital.
- **TD-b touches the sacred prod read-path and the legacy `first_seq=0` sentinel — TD-011 class.**
  A naive monotonic-`first_seq` guard would trip on legacy segments whose `first_seq` defaults to 0
  (`segments.rs:509-511`). Forcing a fail-closed change into `read_all`/`stream` under M-18 time
  pressure risks a WORSE prod regression in the hot read path than the discipline gap it closes.
  This is precisely the reviewer↔architect boundary (`gates.md` §4): defect described, defense
  designed RED-first in its own milestone, not bolted on here.

Net: the genuinely dangerous, silent failure (seq-reuse via undecodable variant) is closed by a
green, anti-placebo-verified machine invariant. The remaining item is a defense-in-depth machine
barrier against operator error, appropriately scheduled as TD.

### Non-blocking finding (new this round) — meta-preserve guarantee is not absolute

The runbook states the rollback yields a "seq-ПРОПУСК (НЕ reuse)". That holds only when
`journal.meta` is at the TRUE high-water. `flush()` (`crates/journal/src/lib.rs:291-297`) orders
`sync_data()` BEFORE `write_meta()`, so a crash in that window leaves durable segment data ahead of
`meta` by up to one batch (≤64 events). If the operator then archives that segment and starts the
old binary, `resolve_next_seq_with = max(meta, pre_tail+1)` can land on seq VALUES that also exist
in the archived segment — i.e. cross-epoch value-reuse, not a clean gap. This is BENIGN because the
never-merge rule keeps live and archive as separate epochs (the live stream stays internally
monotonic; the archive is a forked cold epoch), so no single replayed stream sees a duplicate. But
the doc should not phrase "gap, never reuse" as an absolute — under the torn-flush window it is
cross-epoch reuse that is safe ONLY because epochs never merge, not because values are globally
unique. Recommend softening the §5.1 / §10 wording. Non-blocking: the actual safety property
(per-epoch monotonicity + never-merge) is preserved either way; no code or gate change needed to
merge M-18.

### Other checklist items — confirmed unchanged and sound

- **MD-only / order path:** PASS. Branch diff scope confirmed above; contracts change is a pure
  additive `MdPayload::L2Delta` variant with no order surface.
- **Backtest inertness:** PASS as plan/gate. RFC §6 + milestone task 5 require `sim/exchange.rs` to
  IGNORE L2Delta (raw deltas into the fill path would double-count book motion); exhaustive-match
  compiler canary enforces an arm exists, RED enforces it is an ignore.
- **Magic / version:** PASS. `SEGMENT_MAGIC=HFTJRN02`, `SCHEMA_VERSION=2` unchanged; verify greps
  both; `red_rfc04` green.
- **Additive discriminant / historical blob:** PASS. `L2Delta = 6` appended after `MarginRate = 5`;
  pinned by `red_rfc04`; historical blob is a hand-coded byte array, non-circular.
- **Volume:** PASS with bounded note (unchanged from rev1). BTC-only (founder ★ option (a)) keeps
  the increment bounded; disk-guard + heartbeat + §8 measured write-rate are the operational
  backstop; TD-020 retention remains urgent-next, not optional.

### Merge conditions (reviewer must honor at merge — these gate the PASS)

1. **Register both follow-up TDs in `TECH-DEBT.md`** (reviewer-owned; risk-critic/architect cannot
   write that file): TD-a (recorder startup schema-guard) and TD-b (reader `first_seq`-guard —
   the machine barrier that makes "no re-stitch" enforced rather than disciplinary; must respect the
   legacy `first_seq=0` sentinel, TD-011 class). Both are separate journal-hardening milestones, NOT
   M-18.
2. **§8 post-merge rollback-check (milestone task 6):** verify the first live BTC L2Delta lands in a
   NEW M-18-provenance segment; the post-M18 segment is provenance-identifiable; reviewer does not
   treat `deploy.yml` auto-rollback as blind safety for a schema-forward deploy. Also verify BTC
   L2Delta present (spot+perp) and non-BTC L2Delta absent.
3. (Recommended, non-blocking) Soften the §5.1 / §10 "gap, never reuse" wording per the non-blocking
   finding above.

### Escalation

No founder escalation required. concern-2 is resolved at the documentation/policy level; the
residual is a correctly-scoped TD, not a founder-signature decision. The path to money is untouched
(MD-only), so no border-C signature is implicated by this verdict. Handoff: PASS → reviewer, who
executes the merge conditions above and runs the §8 deploy gate.

### Cleanup

Temporary arm + anti-placebo edit were made only to `crates/journal/src/segments.rs`, both reverted
via `git checkout --`; worktree clean before writing this verdict (only this file changed).
