# C-017 — M-18 L2Delta plan-time critic verdict

**UTC:** 2026-07-21T10:18:04Z  
**Agent:** critic  
**Branch audited:** `origin/feat/M-18-l2delta` at `3e65c66`  
**Base:** `d520dcb` (`origin/main`)  
**Worktree:** `/tmp/hft-critic-m18` (detached at `origin/feat/M-18-l2delta`)  
**Verdict:** REJECT

## Scope Read

Read:

- `docs/rfc/CT-RFC-04-l2delta.md`
- `milestones/M-18-l2delta-capture.md`
- `crates/contracts/tests/red_rfc04.rs`
- `crates/venue-binance/tests/red_l2delta_capture.rs`
- `crates/venue-binance-futures/tests/red_l2delta_futures.rs`
- `crates/journal/tests/red_l2delta_persist.rs`
- `scripts/verify_M-18.sh`
- `git diff origin/main..HEAD -- crates/contracts/src/lib.rs`

Protocol/context read:

- `CLAUDE.md`
- `.claude/agents/critic.md`
- `.claude/rules/gates.md`
- `.claude/rules/testing.md`
- `.claude/rules/scope-guard.md`
- `docs/04-workflow.md`
- `docs/05-contract-layer.md`
- `docs/fa/{contracts,venues,journal,sim,research-cli}.md`

The requested `.codex/agents/critic.toml` / `.claude/rules/critic-protocol.md` paths from the generic bootstrap do not exist in this hft worktree; the local critic profile is `.claude/agents/critic.md`.

Commit set over `d520dcb`:

```text
6af0aef feat(M-18): CT-RFC-04 — MdPayload::L2Delta T1 contract
3494731 test(M-18): sacred RED — venue L2Delta capture + journal persist gate
3e65c66 docs(M-18): milestone — L2Delta capture and live emit gate
```

## Pre-flight

PASS for committed artifact set:

- RFC exists and is T1-scoped: `docs/rfc/CT-RFC-04-l2delta.md`.
- Milestone exists and is `PROPOSED`: `milestones/M-18-l2delta-capture.md`.
- Contract package exists: `crates/contracts/src/lib.rs`, generated schema, fixtures, CHANGELOG, `red_rfc04`.
- Sacred RED exists for both venues and journal persist.
- Acceptance gate exists and is a real aggregator: `scripts/verify_M-18.sh` exits non-zero when any check fails.

Current RED state before implementation is correct at the aggregate level:

```text
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
FAIL  l2delta_event не вызван в src venue-адаптера (...)
FAIL  journal red_l2delta_persist (...)
FAIL  workspace собирается со всеми armами L2Delta (...)
VERDICT: FAIL (5)
exit=1
```

Contract RED is already GREEN, as expected for architect-owned T1:

```text
cargo test -p contracts --test red_rfc04 -- --nocapture; echo exit=$?
running 7 tests
... 7 passed
exit=0
```

## Verdict

REJECT before dispatching venue-dev / engine-dev.

The T1 additive form, historical-blob CT-I-3 protection, and core venue/journal RED reachability are materially good. The blocker is gate integrity:

1. RFC/milestone claim a complete E0004 consumer list, but the list is incomplete.
2. `verify_M-18.sh` claims to catch helper-only dead-code wiring, but its canary can pass on the helper definition alone.

Those are plan-time defects because they create late discovery or false-GREEN risk for the implementation chain.

## Reachability

I temporarily prototyped the requested implementation surface:

- `venue_binance::l2delta_event(&DepthDiff) -> EventKind`
- `venue_binance_futures::{pub DepthDiff, l2delta_event(&DepthDiff) -> EventKind}`
- spot/futures emit-path calls
- `journal::segment_last_ts` L2Delta timestamp arm
- `sim::Exchange::on_event` L2Delta ignore arm
- the extra E0004 consumer arms found during workspace build

Focused RED became GREEN:

```text
cargo test -p venue-binance --test red_l2delta_capture
running 1 test ... ok

cargo test -p venue-binance-futures --test red_l2delta_futures
running 1 test ... ok

cargo test -p journal --test red_l2delta_persist
running 1 test ... ok
```

With the temporary prototype plus all discovered E0004 arms, full verify passed:

```text
bash scripts/verify_M-18.sh; echo exit=$?
PASS ... 
VERDICT: PASS
exit=0
```

All prototype changes were reverted before this verdict file was written.

## Anti-placebo

PASS for the requested RED properties:

- Spot helper without implementation is compile-RED (`l2delta_event` missing).
- Futures helper without implementation is compile-RED (`l2delta_event` missing; `DepthDiff` private).
- `red_l2delta_capture` is not placebo: dropping the second bid makes `bids.len() == 2` fail.
- `red_l2delta_futures` is not placebo: mapping futures `pu` to `None` makes the `Some(500)` assertion fail.
- Journal persist is not a stub check: it uses real `Journal::append`, `flush`, and `read_all`, then exact `EventKind` equality.

The `.claude/rules/testing.md` fixture checklist is covered for the data-shape invariant:

- Asymmetry: spot has empty `asks`; journal fixture preserves empty side.
- Multiplicity: spot has two bids.
- Absence vs remove: empty side means unchanged; `size == 0` is explicit remove.
- Boundaries: spot `prev_final=None`; futures `prev_final=Some(pu)`; invalid fixture without `final_update_id` is rejected.

Production scale is handled as an operational volume/retention gate rather than a unit fixture. That is acceptable for critic scope because M-18 changes the event shape and capture path, not the journal open/read algorithm; RFC §5 quantifies the volume increase, milestone task 6 requires live write-rate proof, and risk-critic owns the go/no-go on starting before retention is delivered.

## Blocking Findings

### B1 — RFC §6 E0004 list is not complete

RFC §6 says the source-level breakage list is complete and checked by `cargo check`:

- `docs/rfc/CT-RFC-04-l2delta.md:107-117`

It lists only:

- `crates/journal/src/segments.rs:1515`
- `crates/sim/src/exchange.rs:223`

The temporary reachability prototype proved additional exhaustive-match sites are required before `cargo build --workspace --all-targets` can pass:

- `crates/research-cli/src/bin/latency_probe.rs:120` — add `L2Delta` to the ignored MD variants.
- `crates/journal/examples/dump.rs:18` — add/count/ignore `L2Delta` in the first diagnostic payload match.
- `crates/recorder/src/lib.rs:68` — `md_kind_label(&MdPayload)` needs `MdPayload::L2Delta { .. } => "l2delta"`.

The `recorder` site is not harmless compile noise. `md_kind_label` is the canonical label for `md_events_total{venue,symbol,kind}`; without an explicit `l2delta` label, the new high-volume stream has no correct runtime metric label after implementation.

Current `cargo check --workspace --all-targets` stops at the first visible E0004 (`sim`), but the source scan and temporary prototype exposed the rest. The milestone partially mitigates this with "любой оставшийся E0004-сайт" and the workspace-build canary, but the RFC still overclaims "полный список" and sends engine-dev into late failure discovery.

Required repair:

1. Update RFC §6 and milestone task 5 to list all known consumer arms:
   `journal/src/segments.rs`, `sim/src/exchange.rs`, `research-cli/src/bin/latency_probe.rs`, `journal/examples/dump.rs`, `recorder/src/lib.rs`.
2. Keep `cargo build --workspace --all-targets` as the final canary for any future undiscovered match site.
3. Explicitly require recorder metric label `l2delta`.

### B2 — `verify_M-18.sh` wiring canary can pass on helper-only code

The milestone task requires not only a pure translator, but an emit-path call for every parsed diff:

- `milestones/M-18-l2delta-capture.md:59-60`

The verify script claims the canary catches "function exists, but not called":

- `scripts/verify_M-18.sh:56-64`

But the predicate is only:

```bash
grep -qE "l2delta_event" crates/venue-binance/src/lib.rs
grep -qE "l2delta_event" crates/venue-binance-futures/src/lib.rs
```

A helper-only implementation must define `pub fn l2delta_event`, so it satisfies this grep even if the live WS/depth branch never calls it. The venue RED tests also call `l2delta_event` directly; they do not exercise the live parse/emit seam. Therefore M-18 can become verify-GREEN with no actual source wiring until reviewer §8 catches it on VPS.

This contradicts the script's own label and repeats the TD-014 class: unit-green does not prove live emit.

Required repair:

1. Strengthen the canary to prove a call site, not symbol presence. For example, check `tx.send(l2delta_event(` in spot and the futures `on_ws_text`/`SessionEffect::Emit` path that wraps `l2delta_event`.
2. Prefer adding a RED seam test that feeds a representative depth WS message through the adapter parse/emit path and asserts an emitted `MdPayload::L2Delta`.
3. Keep reviewer §8 as the decisive live gate; do not rely on §8 to compensate for a decorative verify predicate.

## Non-blocking Checks

### T1 additivity / CT-I-3

PASS. `MdPayload::L2Delta` is appended after `MarginRate`, and `red_rfc04` pins discriminants `0..5` and `L2Delta == 6`. The historical blob is real enough for this gate: it is an embedded byte array with a hand-coded expected `L2Snapshot`, not a reserialization of current code.

`SEGMENT_MAGIC=HFTJRN02` and `SCHEMA_VERSION=2` are intentionally unchanged and verified by `verify_M-18.sh`. Given the RFC's framing/version rationale and CT-RFC-01 precedent, critic does not block on the no-bump decision; risk-critic should still validate prod-read implications.

### Capture losslessness

PASS for plan-time RED. Spot and futures tests assert all fields needed for reconstruction:

- `U -> first_update_id`
- `u -> final_update_id`
- futures `pu -> prev_final_update_id`
- spot `prev_final_update_id == None`
- `E -> ts_exch_ms`
- bid/ask levels, including `size == 0`
- empty side preserved

### Milestone / RFC / verify coherence

PASS except for B1/B2.

There is at least one check per executable task:

- Task 1: `red_rfc04`, `red_schema`, fixtures, CHANGELOG, magic/schema grep.
- Task 2: venue capture RED, futures RED, journal persist RED, verify script.
- Task 3: spot capture test plus intended wiring canary.
- Task 4: futures capture test plus intended wiring canary.
- Task 5: journal persist plus `cargo build --workspace --all-targets`.
- Task 6: reviewer §8 live-emit gate; correctly not represented as a local unit test.
- Task 7: tester clean checkout gate.

### Scope / risk split

PASS as plan text. M-18 is correctly marked T1 + sacred live-path, with critic and risk-critic both mandatory. This verdict is not a risk-critic PASS. The retention/founder decision in RFC §5 remains open until risk-critic/founder resolution.

## Required Architect Repair

Before dispatching venue-dev / engine-dev:

1. Repair RFC §6 and milestone task 5 with the complete known E0004 consumer list, including recorder metrics.
2. Repair `verify_M-18.sh` so the wiring canary proves an emit-path call, or add a RED seam test that does.
3. Re-run critic on the repaired branch.

The expected post-repair outcome is likely APPROVE: the core contract and losslessness RED suite are reachable and mutation-resistant.

## Cleanup

Temporary reachability prototypes and anti-placebo mutations were fully reverted before this verdict. Pre-verdict worktree status was clean except for this critique file.
