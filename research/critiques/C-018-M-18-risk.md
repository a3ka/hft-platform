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
