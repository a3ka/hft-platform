# C-016 — Order-flow plan batch critic verdict

**UTC:** 2026-07-20T23:37:02Z  
**Agent:** critic  
**Branch audited:** `origin/feat/orderflow-plan` at `a807d8e`  
**Base:** `3523668` (`origin/main`)  
**Worktree:** `/tmp/hft-critic-of` (detached at `origin/feat/orderflow-plan`)  
**Verdict:** REJECT

## Scope Read

Read:

- `milestones/M-16-historical-import.md`
- `milestones/M-17-orderflow-signals-phaseA.md`
- `milestones/M-19-frontend-cockpit.md`
- `milestones/BACKLOG.md` sections "Order-flow трек" and "Порядок исполнения"
- `crates/research-cli/tests/red_depth_series.rs`
- `crates/research-cli/tests/red_footprint.rs`
- `crates/research-cli/tests/red_ohlcv.rs`
- `scripts/verify_M-17.sh`

Commit set over `3523668`:

```text
cb3610f docs(M-16/M-17): order-flow план — историч. импорт + trade-flow сигналы+viz (PROPOSED)
6dc849a docs(M-17): убрать визуализацию из scope — только бэкенд + экспорт данных
e314da5 docs(M-17): экспорт-контракт под code2alpha (UDF 1s-бары + lightweight-charts серии)
0ee29de docs(M-19+BACKLOG): frontend cockpit на базе code2alpha (ПОЗЖЕ) + order-flow трек в роадмап
d85d0a4 test(M-17): OF-I-6 depth time-series RED — BID/ASK раздельно по полосам/таймфреймам
556a1e6 docs(BACKLOG): сквозной порядок исполнения (блоки 1-4 + кросс-cutting)
6809675 test(M-17): OF-I-2/3 trade-flow RED — footprint-дельта + cumulative delta
5a8d334 test(M-17): OF-I-4 OHLCV-бары RED — свечи из сделок под code2alpha DataFeed
a807d8e chore(M-17): verify_M-17 — гейт order-flow RED
```

The requested branch was already checked out at `/tmp/hft-arch-of`, so I audited in a separate detached worktree from `origin/feat/orderflow-plan`.

## Verdict

REJECT until M-17's export RED/verify is aligned with the milestone's own OF-I-4 and M-19 dependency.

The RED tests are reachable and the requested anti-placebo mutations are caught. The blocker is that the official M-17 gate can still pass with no per-price footprint bins and no documented `research/exports` schema/example, while the milestone and M-19 both promise those as the frontend contract.

## Pre-flight

PASS for committed artifact set:

- milestones: M-16, M-17, M-19 present
- roadmap/backlog: order-flow track and execution order present
- RED: M-17 `red_depth_series`, `red_footprint`, `red_ohlcv` present
- verify: `scripts/verify_M-17.sh` present
- scope: branch touches docs, M-17 research-cli tests, and M-17 verify only; no T1/contracts, risk, OMS, recorder, venue, or safety/order path changes

Current branch is correctly RED before implementation:

```text
bash scripts/verify_M-17.sh; echo exit=$?
FAIL  OF-I-6 depth time-series (...)
FAIL  OF-I-2/3 footprint + cumulative delta (...)
FAIL  OF-I-4 OHLCV-бары (...)
PASS  signals::obi существует (...)
NOTE  research/exports/ ещё не создан (...)
VERDICT: FAIL (3)
exit=1
```

## Reachability

I temporarily prototyped exactly the requested public surface:

- `research_cli::depth_series::compute`
- `research_cli::orderflow::{footprint_delta,cumulative_delta}`
- `research_cli::export::{ohlcv_bars,OhlcvBar}`

Focused RED became GREEN:

```text
cargo test -p research-cli --test red_depth_series
running 5 tests ... ok

cargo test -p research-cli --test red_footprint
running 6 tests ... ok

cargo test -p research-cli --test red_ohlcv
running 4 tests ... ok
```

Official verify also passed under the prototype:

```text
bash scripts/verify_M-17.sh; echo exit=$?
PASS  OF-I-6 depth time-series (...)
PASS  OF-I-2/3 footprint + cumulative delta (...)
PASS  OF-I-4 OHLCV-бары (...)
PASS  signals::obi существует (...)
NOTE  research/exports/ ещё не создан (research-dev task 5 ...)
VERDICT: PASS
exit=0
```

All prototype changes were reverted before this verdict file was written.

## Anti-placebo

The requested mutation checks PASS:

- `depth_series`: sum BID+ASK instead of side-specific depth fails `bid_and_ask_are_separate_not_summed`.
- `depth_series`: first snapshot instead of last snapshot per bucket fails `timeframe_bucketing_takes_last_per_bucket`.
- `orderflow`: swap buy/sell signs fails signed footprint and cumulative tests.
- `orderflow`: cumulative delta reset per bucket fails `cumulative_delta_accumulates_across_buckets`.
- `export`: `open=last` fails `ohlcv_fields_are_correct`.
- `export`: `volume=count` fails `ohlcv_fields_are_correct`.

Additional probe: `buy+sell` instead of signed `buy-sell` also fails `red_footprint`.

This part is strong. The RED suite is not placebo for the APIs it actually names.

## Blocking Finding

### B1 — M-17 verify can pass without the promised footprint-bin export contract

Evidence:

- M-17 OF-I-4 promises export data with `(price, buy_vol, sell_vol, delta)` per bin in a stable documented format (`milestones/M-17-orderflow-signals-phaseA.md:30`).
- M-17 task 5 repeats the same deliverable: `цена, buy_vol, sell_vol, delta per bin/бар` plus schema/example in `research/exports/` (`milestones/M-17-orderflow-signals-phaseA.md:52`).
- M-17 export contract says full per-price footprint bins are delivered as data; rendering is frontend work (`milestones/M-17-orderflow-signals-phaseA.md:69-72`).
- M-19 depends on M-17 footprint bins for the footprint/cluster and volume-profile tiers (`milestones/M-19-frontend-cockpit.md:31`, `milestones/M-19-frontend-cockpit.md:71`).
- The committed RED tests do not define or assert any per-price footprint-bin API. `red_footprint.rs` input is `(ts_ms, Side, size)`, with no price field, so it cannot test `(price, buy_vol, sell_vol, delta)`.
- The GREEN prototype had no footprint-bin type/function and no `research/exports` schema/example, yet `scripts/verify_M-17.sh` returned `VERDICT: PASS`.
- `scripts/verify_M-17.sh` explicitly downgrades missing `research/exports/*.md` to `NOTE`, even though M-17 task 5 makes it part of acceptance.

Worst case: research-dev implements only `depth_series`, bucket deltas, cumulative delta, and OHLCV bars; M-17 closes green; then M-19 starts and discovers the promised footprint/cluster data contract does not exist. That is a plan-time false-green, not a frontend implementation detail.

Required repair:

1. Add RED for per-price footprint bins, e.g. `research_cli::orderflow::footprint_bins(trades, timeframe_ms, price_tick) -> Vec<FootprintBin>` where `FootprintBin` exposes `time_s`, `price`, `buy_vol`, `sell_vol`, `delta`.
2. Test asymmetric same-price and different-price fixtures: buy/sell retained separately, delta signed, no invented price levels, correct bucket aggregation.
3. Make `scripts/verify_M-17.sh` include that RED test.
4. Make missing `research/exports/*.md` schema/example a failing condition once the M-17 GREEN gate is expected to pass, or add a Rust/doc fixture test that verifies the schema/example exists and matches the exported fields.

## Non-blocking Checks

### M-16 provenance / DET-I-1

PASS as plan text. M-16 is explicit that import is research-only, imported data is separate provenance-tagged segments, and live capture is not mixed with import (`milestones/M-16-historical-import.md:17-18`). It correctly states no new T1 variant is needed because CT-RFC-02 already provides provenance (`milestones/M-16-historical-import.md:20-23`).

M-16 RED is not committed yet. This is acceptable only because M-16 task 1/2 are architect-owned and must happen before research-dev implementation. Do not dispatch M-16 research-dev until `red_import.rs` and `verify_M-16.sh` are committed.

### Trade-flow vs book-flow boundary

PASS. M-17 states trade-flow is computable from existing `Trade.side`, while absorption/DOM/book-flow require raw book deltas and M-18 / CT-RFC-04 (`milestones/M-17-orderflow-signals-phaseA.md:8-17`, `milestones/M-17-orderflow-signals-phaseA.md:31`, `milestones/M-17-orderflow-signals-phaseA.md:83-85`). M-16 also states HL imported snapshots are not tick deltas and cannot support absorption/DOM (`milestones/M-16-historical-import.md:33`).

### M-19 frontend feasibility honesty

PASS. M-19 does not claim Bookmap parity on lightweight-charts. It marks DOM ladder and liquidity heatmap as bespoke grid/canvas/WebGL work backed by M-18 raw deltas (`milestones/M-19-frontend-cockpit.md:35-36`, `milestones/M-19-frontend-cockpit.md:72`, `milestones/M-19-frontend-cockpit.md:80`).

### Roadmap coherence

PASS with one dependency note. BACKLOG now states the execution order as data/signals -> proof -> frontend -> money barrier (`milestones/BACKLOG.md:157-182`). Putting M-16/M-17 before M-11 is coherent because they are research/export work with no order/safety path. M-18 is correctly separated as T1 + sacred live-path + risk-critic (`milestones/BACKLOG.md:143-145`, `milestones/BACKLOG.md:163-164`) and is not part of this M-16/M-17/M-19 implementation batch.

### risk-critic

N/A for M-16/M-17 implementation scope as written: research-only analytics/export, no OMS/risk/order path. Backtest reports over these datasets still need anti-overfit §6 + risk-critic, as both M-16 and M-17 state.

## Required Architect Repair

Before dispatching research-dev on M-17:

1. Add the missing footprint-bin RED/API and wire it into `verify_M-17.sh`.
2. Turn `research/exports` schema/example into a real GREEN requirement, not a NOTE, or explicitly remove schema/example from M-17 acceptance and from M-19's dependency claim.
3. Re-run critic on the repaired branch.

After repair, the plan is likely approvable. The depth-series, signed footprint delta, cumulative delta, and OHLCV RED oracles are reachable and mutation-resistant.

## Cleanup

Temporary prototypes and mutations were fully reverted before writing this verdict. Pre-verdict worktree status was clean except for this critique file.
