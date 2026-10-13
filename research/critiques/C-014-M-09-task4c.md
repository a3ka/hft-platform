# C-014 — M-09 Task 4C metric emission critic verdict

**Date:** 2026-07-19T16:42:00Z  
**Agent:** critic  
**Audited branch/head:** `origin/feat/M-09-task4c-metric-emission @ 8ef0669`  
**Local audit worktree:** `/tmp/hft-arch-m09-t4c` (`feat/M-09-task4c-metric-emission`, tracking origin)  
**Audited commits:** `9eb4d25` + `f28e78d` + `6d6b73c` + `8ef0669` over `1919350`  
**Scope:** M-09 task 4C, OPS-I-10 / TD-027 live metric emission

## Verdict

**REJECT**

The task is directionally right and the gross TD-027 no-op class is reachable: a real writer/feeder/sampler prototype makes task 4C green, and a producer no-op mutation fails. However, the committed RED set still permits two material false-green implementations:

1. labeled metrics can be emitted without their required labels and the full verify gate still passes;
2. `book_levels` and `recorder_rss_anon_bytes` can be implemented as helper seams but never wired into `recorder::main` live runtime, and the full verify gate still passes.

Both are the same family as TD-027: the gate can report "metric emission covered" while the live `/metrics` surface is not carrying the promised production signal shape.

## Pre-flight

PASS with one branch-hygiene note.

- Branch/head: local `feat/M-09-task4c-metric-emission` matched `origin/feat/M-09-task4c-metric-emission` at `8ef0669`; base was `1919350`.
- Commit chain: four architect commits over main, all doc/test/verify.
- FA artifact present: `docs/fa/ops.md` updates TD-027, producer map, and OPS-I-10.
- RED artifact present: `crates/recorder/tests/red_metrics_emission.rs` plus sacred `run_writer` call-site updates in existing recorder RED tests.
- Verify artifact present: `scripts/verify_M-09.sh` includes task 4C and OPS-I-10 canary.
- Scope: audited commits touched only `docs/fa/ops.md`, `milestones/M-09-data-safety-net.md`, `scripts/verify_M-09.sh`, and `crates/recorder/tests/*`. No impl, contracts, risk, killswitch, OMS, or order-egress changes.

The requested `/tmp/hft-critic-m09-t4c` worktree could not be added because the feat branch was already checked out at `/tmp/hft-arch-m09-t4c`. That worktree was clean and on the exact requested branch, so I audited and committed here rather than creating a separate critic branch.

## Current RED Shape

On the unmodified audited branch:

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
=> compile-RED:
   unresolved import `recorder::metric_emit`
   `run_writer` takes 4 arguments but 5 were supplied
exit=101
```

`red_rss_bounded` is also compile-RED before implementation because the sacred test call sites were updated to the new `run_writer(..., Arc<Metrics>, shutdown)` signature.

## Reachability

I temporarily prototyped:

- `recorder::run_writer(..., Arc<Metrics>, shutdown)` emitting `journal_*` and `md_*`;
- `recorder::metric_emit::{emit_book_levels, sample_rss}`;
- minimal main wiring for the first prototype: `run_writer` receives the shared metrics, feeder calls `emit_book_levels`, sampler emits `recorder_rss_anon_bytes`.

Focused oracles went green:

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 3 tests
test rss_sampler_emits_anon_bytes ... ok
test feeder_emits_book_levels ... ok
test writer_emits_journal_and_md_metrics ... ok
exit=0

cargo test -p recorder --test red_rss_bounded; echo exit=$?
test e7_writer_loop_memory_is_bounded_and_event_count_independent ... ok
exit=0
```

Full verify also became green under the prototype:

```text
bash scripts/verify_M-09.sh; echo exit=$?
...
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
PASS  OPS-I-10 каждая §3-метрика покрыта emission-оракулом или классифицирована event/elsewhere
VERDICT: PASS
exit=0
```

The prototype was reverted before this verdict was written.

## Anti-placebo

### A — Producer no-op is caught

Temporary mutation: keep the new signatures and real journal append, but make `run_writer` metric producers, `emit_book_levels`, and `sample_rss` no-op.

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 3 tests
test feeder_emits_book_levels ... FAILED
test rss_sampler_emits_anon_bytes ... FAILED
test writer_emits_journal_and_md_metrics ... FAILED
exit=101
```

This closes the broad "declared but no sample" form of TD-027.

### B — Label-loss placebo is not caught

Temporary mutation: emit real SAMPLE lines, but drop required labels from the labeled producer metrics:

- `md_events_total 30` instead of `md_events_total{venue,symbol,kind}`;
- `md_event_age_ms 1` instead of `md_event_age_ms{venue}`;
- `book_levels 5` instead of `book_levels{venue,symbol,side}`.

Result:

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 3 tests
test feeder_emits_book_levels ... ok
test rss_sampler_emits_anon_bytes ... ok
test writer_emits_journal_and_md_metrics ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
...
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
PASS  OPS-I-10 каждая §3-метрика покрыта emission-оракулом или классифицирована event/elsewhere
VERDICT: PASS
exit=0
```

This violates the FA producer map at `docs/fa/ops.md:78-83` and the metric contract in `crates/ops/src/metrics.rs` where these specs carry labels. `crates/ops/tests/red_ops_metrics.rs` verifies that the `Metrics` renderer can render labels when labels are passed, but task 4C does not verify that producers pass the required labels.

Required fix: `red_metrics_emission.rs` must assert label-bearing sample lines for every labeled steady producer it covers. At minimum:

- `md_events_total` contains `venue=`, `symbol=`, `kind=` and the expected `BTCUSDT` / L2 kind;
- `md_event_age_ms` contains `venue=`;
- `book_levels` contains `venue=`, `symbol=`, `side=`, and should check both bid and ask side samples, not only the first matching line.

### C — Helper-only "not live" wiring is not caught

Temporary mutation: implement correct `run_writer` emission and correct `metric_emit::{emit_book_levels, sample_rss}`, but only update `main` enough to pass `Arc<Metrics>` into `run_writer`. Do not spawn the RSS sampler in `main`, and do not call `emit_book_levels` from the live books-feeder loop.

Result:

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 3 tests
test rss_sampler_emits_anon_bytes ... ok
test feeder_emits_book_levels ... ok
test writer_emits_journal_and_md_metrics ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
...
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
PASS  OPS-I-10 каждая §3-метрика покрыта emission-оракулом или классифицирована event/elsewhere
VERDICT: PASS
exit=0
```

This is a direct live-emission hole for two steady metrics. The FA says `book_levels` is produced by `recorder::recon_loop::apply_md_to_books` after `apply_snapshot` and `recorder_rss_anon_bytes` is produced by a periodic sampler (`docs/fa/ops.md:81-83`). The milestone allowed paths also name `main.rs` wiring for the sampler and feeder (`milestones/M-09-data-safety-net.md:56-68`). But the current RED only calls helper seams directly (`crates/recorder/tests/red_metrics_emission.rs:148-170`); it does not fail if `recorder::main` never connects those helpers to the live `/metrics` instance.

Required fix: pin the live wiring with an executable seam that `main` must use. Two acceptable shapes:

- make `apply_md_to_books` take `&Metrics` as the milestone text says at `milestones/M-09-data-safety-net.md:267`, and assert `book_levels` after calling that exact function; or
- introduce tested production seams such as `run_books_feeder_once/run_books_feeder` and `run_rss_sampler_once/spawn_rss_sampler`, then have `main` call those seams.

The test must fail if `emit_book_levels` and `sample_rss` exist but are never invoked by live recorder wiring.

## Checks Against Prompt

### `has_sample`

PASS for HELP/TYPE separation, FAIL for full labeled contract.

`has_sample` ignores comment lines and accepts only `name ` or `name{` sample lines (`crates/recorder/tests/red_metrics_emission.rs:30-41`). A registry-only implementation with only `# HELP` / `# TYPE` cannot pass. But `name value` is accepted for metrics that are specified as `name{labels} value`, which enables blocker B.

### OPS-I-10 verify canary

Current branch is not merely mentioning names in comments: `rg` shows the steady names are in the assertion list or direct `has_sample/sample_value` checks in `red_metrics_emission.rs:116-173`. However, the shell canary at `scripts/verify_M-09.sh:118-134` is still a quoted-name grep. It cannot detect missing labels or helper-only live wiring, as shown by mutations B and C.

### Event / elsewhere whitelist

PASS.

The whitelist is defensible for this task:

- `venue_ws_reconnects_total` is event-driven and belongs to venue-dev / supervisor wiring;
- `venue_http_status_total` is already wired in venue recon REST;
- `journal_write_errors_total` and `journal_seq_gaps_total` are event/recovery conditions;
- `book_divergence_bps` and `book_resync_total` are already covered in `ops::sink`;
- `backup_restore_drill_ok` remains task 3 / Storage Box deferred.

### Carve-out / OPS-I-6 / OPS-I-7

PASS with the blockers above.

The carve-out is minimal and correctly bounded to recorder metric emission plus venue reconnect counter (`milestones/M-09-data-safety-net.md:56-68`). The successful prototype kept `red_rss_bounded` green, and the intended implementation can use `metrics.inc_counter/set_gauge` atomics without writing metrics into the journal. No risk-critic is needed for the documented read-side/MD-only scope.

## Required Architect Repair

Before dispatching engine-dev/venue-dev:

1. Strengthen `red_metrics_emission.rs` so labeled producer metrics must emit with required label keys and expected values.
2. Add a RED seam that fails when `book_levels` and `recorder_rss_anon_bytes` helper functions exist but are not wired into live recorder runtime.
3. Optionally strengthen `scripts/verify_M-09.sh` so OPS-I-10 coverage is not a raw quoted-name grep, or explicitly document that the Rust oracle is the semantic gate and the shell canary is only a coverage smoke check.

After these RED changes, re-run critic. Do not dispatch engine-dev on the current artifact set.

## Cleanup

All temporary prototypes and mutations were reverted before this verdict. Final pre-verdict worktree status was clean except this critique file.

---

## Re-audit — 2026-07-19T23:26:11Z

**Audited repair head:** `39fdc07` — `docs(M-09): C-014 repair — task 4C spec (run_books_feeder + labels + live-wiring)`
**Repair commits:** `ea3799f` + `5a61782` + `39fdc07` over verdict `4c864a9`
**Local audit worktree:** `/tmp/hft-critic-m09-t4c-r2` detached at `origin/feat/M-09-task4c-metric-emission`

### Re-audit verdict

**REJECT remains.**

The repair closes the two exact C-014 blockers in their broad form, but it leaves a narrower false-green in the same label-shape family. The current RED proves that a metric has a labeled sample with the required keys; it does not prove the required label values/cardinality or the required non-zero counter value.

### What is fixed

**Gap 1, unlabeled `md_*`: CLOSED for missing labels.**

The repair added `has_labeled_sample(text, name, keys)` and moved `md_events_total` / `md_event_age_ms` from plain `has_sample` to labeled checks (`crates/recorder/tests/red_metrics_emission.rs:60-72`, `:143-153`).

Temporary mutation: `run_writer` emitted `md_events_total` and `md_event_age_ms` as unlabeled samples.

```text
cargo test -p recorder --test red_metrics_emission writer_emits_journal_and_md_metrics; echo exit=$?
test writer_emits_journal_and_md_metrics ... FAILED
md_events_total НЕ несёт labeled SAMPLE `{venue,symbol,kind}` ...
exit=101
```

After restoring labels, the same test passed:

```text
test writer_emits_journal_and_md_metrics ... ok
exit=0
```

**Gap 2, helper-only/non-live: CLOSED for missing main calls.**

The repair introduced `run_books_feeder` as the tested live-loop seam and added a verify canary that greps `main.rs` for `run_books_feeder` and `sample_rss` (`scripts/verify_M-09.sh:139-152`). A correct temporary prototype with `run_books_feeder` + `sample_rss` called from `main` reached:

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 3 tests ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
PASS  OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main
VERDICT: PASS
exit=0

cargo test -p recorder --test red_rss_bounded; echo exit=$?
test e7_writer_loop_memory_is_bounded_and_event_count_independent ... ok
exit=0
```

Temporary mutation: leave helper seams working, but remove `run_books_feeder` and `sample_rss` calls/imports from `main`.

```text
bash scripts/verify_M-09.sh; echo exit=$?
FAIL  OPS-I-10 продюсеры НЕ вызваны в живом main: run_books_feeder(book_levels) sample_rss(recorder_rss_anon_bytes) ...
VERDICT: FAIL (1)
exit=1
```

### Remaining blocker — label cardinality/value false-green

Severity: BLOCKER.

The repair still accepts a producer that carries label keys but collapses the `side` dimension or never increments the MD counter.

**Mutation A: `book_levels` side collapse.**

Temporary implementation wrote both book sides with `side="bid"`:

```text
metrics.set_gauge("book_levels", &[("venue", venue), ("symbol", &symbol), ("side", "bid")], bids);
metrics.set_gauge("book_levels", &[("venue", venue), ("symbol", &symbol), ("side", "bid")], asks);
```

Result:

```text
cargo test -p recorder --test red_metrics_emission live_feeder_loop_emits_book_levels; echo exit=$?
test live_feeder_loop_emits_book_levels ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
PASS  OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main
VERDICT: PASS
exit=0
```

This is not sufficient. `book_levels{venue,symbol,side}` means bid and ask are distinct time series. If both writes use `side="bid"`, the ask write overwrites the bid series and one side disappears from observability. The current fixture is symmetric (`5` bid levels and `5` ask levels), and `sample_value(text, "book_levels") == Some(5)` reads the first matching sample without binding labels (`crates/recorder/tests/red_metrics_emission.rs:184-187`), so the collapse is invisible.

**Mutation B: `md_events_total` labeled sample with value 0.**

Temporary implementation emitted the correct label keys but incremented by `0`.

```text
cargo test -p recorder --test red_metrics_emission writer_emits_journal_and_md_metrics; echo exit=$?
test writer_emits_journal_and_md_metrics ... ok
exit=0
```

This regresses part of the original task text: milestone task 4C says `md_events_total{venue,symbol,kind}` must be `>0` after the writer processes MD events (`milestones/M-09-data-safety-net.md:265-270`). The old `sample_value(... "md_events_total") >= 1` check was removed during the label repair and was not replaced with a label-aware value check.

Required architect repair:

1. Add a label-aware extractor such as `labeled_sample_value(text, name, labels)`.
2. Make the book fixture asymmetric, for example bid depth `3`, ask depth `5`.
3. Assert both exact series:
   - `book_levels{venue="binance",symbol="BTCUSDT",side="bid"} == 3`;
   - `book_levels{venue="binance",symbol="BTCUSDT",side="ask"} == 5`.
4. Assert `md_events_total{venue="binance",symbol="BTCUSDT",kind="<L2 label>"} >= 1`; do not accept a labeled zero sample after 30 MD events.
5. Keep the current live-wiring canary; it is a useful guard, even though it remains a grep smoke check rather than the semantic oracle.

### Scope / cleanup

The repair commits stayed in architect-owned docs/tests/verify paths. No implementation, contracts, risk, killswitch, OMS, or order-egress files were committed.

All temporary prototypes and mutations were reverted before this re-audit section was appended. Final pre-verdict worktree status was clean except `research/critiques/C-014-M-09-task4c.md`.

---

## Re-audit #2 — 2026-07-19T23:36:49Z

**Audited repair head:** `7da7444` — `test(M-09): C-014 re-audit repair — label-aware values + асимметричная фикстура`
**Repair commits:** `7da7444` over re-audit verdict `dd6c543`
**Local audit worktree:** `/tmp/hft-critic-m09-t4c-r3` detached at `origin/feat/M-09-task4c-metric-emission`

### Re-audit #2 verdict

**REJECT remains.**

The repair closes the two blocker mutations from the first re-audit: side-collapse for `book_levels` and labeled-zero `md_events_total` now fail. A fourth false-green remains: `md_events_total` can use the wrong `kind` label value while still passing the RED suite and `verify_M-09.sh`.

### What is fixed

**Side-collapse: CLOSED.**

The repair added `labeled_sample_value`, changed the book fixture to asymmetric depth (`bid=5`, `ask=3`), and asserts both exact labeled series (`crates/recorder/tests/red_metrics_emission.rs:74-89`, `:199-233`).

Temporary mutation: `emit_book_levels` emitted both sides with `side="bid"`.

```text
cargo test -p recorder --test red_metrics_emission live_feeder_loop_emits_book_levels; echo exit=$?
test live_feeder_loop_emits_book_levels ... FAILED
book_levels{side=bid} != 5
left: Some(3)
right: Some(5)
exit=101
```

**`md_events_total` value 0: CLOSED.**

The repair restored a value assertion using `labeled_sample_value(... "md_events_total" ...) >= 1` (`crates/recorder/tests/red_metrics_emission.rs:179-186`).

Temporary mutation: `md_events_total{venue,symbol,kind}` was incremented by `0`.

```text
cargo test -p recorder --test red_metrics_emission writer_emits_journal_and_md_metrics; echo exit=$?
test writer_emits_journal_and_md_metrics ... FAILED
md_events_total{venue=binance,symbol=BTCUSDT} == 0 после 30 Md-событий ...
exit=101
```

**Correct prototype remains reachable.**

With a temporary correct implementation (`book_levels{side="bid"} = 5`, `book_levels{side="ask"} = 3`, `md_events_total` increment by `1`, feeder/sampler wired in `main`):

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 3 tests ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
PASS  OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main
VERDICT: PASS
exit=0

cargo test -p recorder --test red_rss_bounded; echo exit=$?
test e7_writer_loop_memory_is_bounded_and_event_count_independent ... ok
exit=0
```

### Remaining blocker — `kind` label value is not pinned

Severity: BLOCKER.

The current writer test verifies that `md_events_total` has a `kind=` key (`has_labeled_sample`) and that some `md_events_total{venue="binance",symbol="BTCUSDT",...}` value is non-zero. It does not require the `kind` value to match the event payload.

Temporary mutation: classify `MdPayload::L2Snapshot` as `kind="trade"` while still incrementing by `1`.

```text
cargo test -p recorder --test red_metrics_emission writer_emits_journal_and_md_metrics; echo exit=$?
test writer_emits_journal_and_md_metrics ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
PASS  OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main
VERDICT: PASS
exit=0
```

This is a real OPS-I-10 / TD-014 hole, not cosmetic label strictness. `md_events_total{venue,symbol,kind}` is the metric used to detect that an event class disappeared. If all L2 snapshots, funding, open interest, or liquidations are counted under the wrong `kind`, the metric is live but semantically blind by class.

Required architect repair:

1. Include the expected `kind` label in the value assertion for the L2 fixture, for example `kind="l2_snapshot"` if that is the canonical label.
2. Prefer adding at least one second MD payload kind in the writer test, e.g. `Funding`, and assert a distinct labeled series for it. That catches "all payloads use one fixed kind" as well as a wrong L2 label.
3. Add a small comment in the test naming the canonical kind labels expected from engine-dev so implementation does not invent incompatible spellings.

### Scope / cleanup

The `7da7444` repair commit touched only `crates/recorder/tests/red_metrics_emission.rs`. Scope is clean.

All temporary prototypes and mutations were reverted before this re-audit #2 section was appended. Final pre-verdict worktree status was clean except `research/critiques/C-014-M-09-task4c.md`.

---

## Re-audit #3 — 2026-07-20T12:13:10Z

**Audited repair head:** `4979dbd` — `test(M-09): C-014 re-audit #2 repair — kind-aware счётчики + канон kind-строки`
**Repair commits:** `4979dbd` over re-audit #2 verdict `5dfb375`
**Local audit worktree:** `/tmp/hft-critic-m09-t4c-r4` detached at `origin/feat/M-09-task4c-metric-emission`

### Re-audit #3 verdict

**REJECT remains.**

The repair closes the wrong-kind blocker from re-audit #2. The updated writer oracle now requires exact per-kind counters: `md_events_total{kind="l2snapshot"} == 30` and `md_events_total{kind="trade"} == 10`. However, fifth false-greens remain in the same "declared/labeled but semantically wrong" class:

1. all venue labels can collapse to `venue="binance"` and the suite still passes;
2. `journal_seq_current`, `journal_disk_free_bytes`, and `md_event_age_ms` can emit dead zero values and the suite still passes.

### What is fixed

**Wrong `kind` value: CLOSED.**

The repair added a second MD payload (`Trade`), canonical kind strings in the test comments, and exact label-aware assertions for both L2 and trade series (`crates/recorder/tests/red_metrics_emission.rs:118-134`, `:144-152`, `:201-223`). The milestone also documents the canonical `kind` label values at `milestones/M-09-data-safety-net.md:57-64`.

Temporary mutation: classify `MdPayload::L2Snapshot` as `kind="trade"` while still incrementing by `1`.

```text
cargo test -p recorder --test red_metrics_emission writer_emits_journal_and_md_metrics; echo exit=$?
test writer_emits_journal_and_md_metrics ... FAILED
md_events_total{kind=l2snapshot} != 30 ...
left: None
right: Some(30)
exit=101
```

**Correct prototype remains reachable.**

With a temporary correct implementation (`L2Snapshot -> l2snapshot`, `Trade -> trade`, `book_levels` bid/ask exact values, feeder/sampler wired in `main`):

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 3 tests
test rss_sampler_emits_anon_bytes ... ok
test live_feeder_loop_emits_book_levels ... ok
test writer_emits_journal_and_md_metrics ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
PASS  OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main
VERDICT: PASS
exit=0

cargo test -p recorder --test red_rss_bounded; echo exit=$?
test e7_writer_loop_memory_is_bounded_and_event_count_independent ... ok
exit=0
```

### Remaining blocker A — `venue` label collapse

Severity: BLOCKER.

Temporary mutation: both writer and book-level producer mapped every `Venue` to `venue="binance"`.

```text
fn venue_label(_v: Venue) -> &'static str { "binance" }
```

Result:

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 3 tests ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
PASS  OPS-I-10 live-wiring: отдельные продюсеры (feeder/sampler) вызваны в живом main
VERDICT: PASS
exit=0
```

This is materially the same class as wrong `kind`: the metric is live and labeled, but the label value hides which producer/venue failed. `md_events_total{venue,symbol,kind}`, `md_event_age_ms{venue}`, and `book_levels{venue,symbol,side}` all rely on `venue` for TD-014/OPS-SILENCE/TD-016 localization. The current fixtures only use `Venue::Binance`, so they cannot detect venue collapse.

Required repair:

1. Add at least one non-Binance fixture to the writer test, preferably `Venue::Hyperliquid` or `Venue::BinanceFutures`, and assert exact `md_events_total{venue=...,symbol=...,kind=...}` values for both venues.
2. Add at least one non-Binance `L2Snapshot` through `run_books_feeder` and assert exact `book_levels{venue=...,symbol=...,side=bid/ask}` for that venue.
3. Assert `md_event_age_ms` has a labeled sample for every venue fed in the writer test, not only `venue="binance"`.

### Remaining blocker B — dead zero values for steady writer gauges

Severity: BLOCKER.

Temporary mutation: keep samples present, but set:

```text
journal_seq_current = 0
journal_disk_free_bytes = 0
md_event_age_ms = 0
```

Results:

```text
cargo test -p recorder --test red_metrics_emission writer_emits_journal_and_md_metrics; echo exit=$?
test writer_emits_journal_and_md_metrics ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
PASS  T4C OPS-I-10 живая ЭМИССИЯ метрик (...)
VERDICT: PASS
exit=0
```

`journal_segment_index == 0` is legitimate on the first segment, so sample-presence is enough there. But `journal_seq_current == 0` after the writer drains 60 events is not legitimate, and `journal_disk_free_bytes == 0` on the tempdir means the disk-guard alert surface is dead by value while the gate is green. The milestone text says `journal_seq_current` is expected `>0`, and §8 requires steady metrics non-zero; the RED should pin that.

`md_event_age_ms == 0` is also suspicious for this fixture because every MD event uses `ts_exch_ms = 1`; a real age calculation should be positive. More importantly, a permanently zero age gauge blinds OPS-I-8. If architect wants task 4C to remain "emission only" for this metric, document that value semantics are deferred; otherwise pin `md_event_age_ms{venue="binance"} > 0` under the current fixture.

Required repair:

1. Assert `journal_seq_current >= 60` or exact expected next sequence after the test batch.
2. Assert `journal_disk_free_bytes > 0` in the tempdir-backed writer test.
3. Either assert `md_event_age_ms{venue=...} > 0` for the synthetic `ts_exch_ms = 1` events, or explicitly carve value semantics for age out of task 4C and leave it to a separate OPS-I-8 producer/value oracle.
4. Keep `journal_segment_index` as sample-only unless the fixture forces rotation; `0` is valid for the first segment.

### Scope / cleanup

The `4979dbd` repair commit touched only `crates/recorder/tests/red_metrics_emission.rs` and `milestones/M-09-data-safety-net.md`. Scope is clean.

All temporary prototypes and mutations were reverted before this re-audit #3 section was appended. Final pre-verdict worktree status was clean except `research/critiques/C-014-M-09-task4c.md`.

---

## Re-audit #4 — 2026-07-20T12:44:34Z

**Audited repair head:** `d630ca2` — `test(M-09): C-014 re-audit #3 repair — мульти-вендор/символ фикстура + dead-zero пины + md_age sampler`
**Repair commits:** `d630ca2` over re-audit #3 verdict `7ca5f11`
**Local audit worktree:** `/tmp/hft-critic-m09-t4c-r5` detached at `origin/feat/M-09-task4c-metric-emission`

### Re-audit #4 verdict

**REJECT remains.**

The repair closes the two blockers from re-audit #3:

1. writer-side `venue` / `symbol` / `kind` collapse is now pinned by exact per-series `md_events_total` values;
2. dead-zero for `journal_seq_current`, `journal_disk_free_bytes`, and `md_event_age_ms` is now pinned.

However, there is still a sixth false-green in the same label-value collapse class: `book_levels{venue,symbol,side}` can ignore the `symbol` argument and always emit `symbol="BTCUSDT"`. The full task-4C oracle and `verify_M-09.sh` remain green.

### Requested blocker checks

**Correct prototype is reachable.**

Temporary correct implementation: `run_writer(..., Arc<Metrics>, shutdown)`, `metric_emit::{emit_book_levels,sample_md_age,sample_rss}`, `run_books_feeder`, and live `main` calls for feeder/samplers.

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 4 tests
test md_age_sampler_emits_real_age_per_venue ... ok
test rss_sampler_emits_anon_bytes ... ok
test live_feeder_loop_emits_book_levels ... ok
test writer_emits_journal_and_md_metrics ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
VERDICT: PASS
exit=0

cargo test -p recorder --test red_rss_bounded; echo exit=$?
test e7_writer_loop_memory_is_bounded_and_event_count_independent ... ok
exit=0
```

**Venue collapse: CLOSED.**

Temporary mutation: `metric_emit::venue_label(BinanceFutures|Hyperliquid) -> "binance"`.

```text
cargo test -p recorder --test red_metrics_emission writer_emits_journal_and_md_metrics -- --exact; echo exit=$?
writer_emits_journal_and_md_metrics ... FAILED
left: Some(35)
right: Some(30)
exit=101

cargo test -p recorder --test red_metrics_emission live_feeder_loop_emits_book_levels -- --exact; echo exit=$?
live_feeder_loop_emits_book_levels ... FAILED
left: Some(4)
right: Some(5)
exit=101
```

**Writer `symbol` / `kind` collapse: CLOSED.**

Temporary mutations:

1. writer emits every `md_events_total` with `symbol="BTCUSDT"`;
2. writer maps `Trade -> "l2snapshot"`.

```text
symbol-collapse:
writer_emits_journal_and_md_metrics ... FAILED
left: Some(37)
right: Some(30)
exit=101

kind-collapse:
writer_emits_journal_and_md_metrics ... FAILED
left: Some(40)
right: Some(30)
exit=101
```

**Dead-zero: CLOSED.**

Temporary mutations:

1. `journal_seq_current = 0`;
2. `journal_disk_free_bytes = 0`;
3. `sample_md_age(...) -> 0`.

```text
seq_current=0:
writer_emits_journal_and_md_metrics ... FAILED
journal_seq_current == 0 после 72 append'ов
exit=101

disk_free=0:
writer_emits_journal_and_md_metrics ... FAILED
journal_disk_free_bytes == 0
exit=101

md_age=0:
md_age_sampler_emits_real_age_per_venue ... FAILED
left: Some(0)
right: Some(200)
exit=101
```

### Remaining blocker — `book_levels` symbol collapse

Severity: BLOCKER.

Temporary mutation:

```rust
pub fn emit_book_levels(metrics: &Metrics, venue: Venue, symbol: &str, book: &OrderBook) {
    let venue = venue_label(venue);
    metrics.set_gauge(
        "book_levels",
        &[("venue", venue), ("symbol", "BTCUSDT"), ("side", "bid")],
        book.n_levels(Side::Buy) as i64,
    );
    metrics.set_gauge(
        "book_levels",
        &[("venue", venue), ("symbol", "BTCUSDT"), ("side", "ask")],
        book.n_levels(Side::Sell) as i64,
    );
}
```

Result:

```text
cargo test -p recorder --test red_metrics_emission live_feeder_loop_emits_book_levels -- --exact; echo exit=$?
test live_feeder_loop_emits_book_levels ... ok
exit=0

cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 4 tests ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
VERDICT: PASS
exit=0
```

This is not cosmetic. The metric contract is `book_levels{venue,symbol,side}`. The repair fixture varies `venue` and `side`, but both feeder events use `symbol="BTCUSDT"`, so a defective producer can publish all book-depth series under one symbol and still pass. That hides per-instrument TD-016 localization in the same way previous repairs hid venue/kind/side failures.

Required repair:

1. Add a second symbol to `live_feeder_loop_emits_book_levels`, with distinct bid/ask counts, e.g. `Venue::Binance, "ETHUSDT", 6, 1`.
2. Assert exact `book_levels{venue="binance",symbol="ETHUSDT",side="bid"} == 6` and `side="ask" == 1`.
3. Keep the existing BinanceFutures fixture; it is still needed for venue-collapse.

### Non-blocking notes

`journal_bytes_written_total` remains value-under-specified. A temporary mutation that increments it only once after the first append (`final value == 1`) still passes the writer test and full task-4C oracle. I am not making this the rejection reason because task 4C and §8 currently require non-zero emission, and no §7.1 alert consumes byte-accurate values. If the intended contract is truly bytes, not liveness/append activity, add a separate value oracle with a large payload and require the counter to exceed the event count or otherwise reflect written frame bytes.

`sample_md_age(...).max(0)` is not a blocker here. Negative age comes from clock skew / timestamp ordering; clamping an age gauge to non-negative is a reasonable implementation choice unless a separate clock-skew invariant is introduced. The current positive `now-last` assertions are enough for OPS-I-10 emission/value reachability.

### Scope / cleanup

The `d630ca2` repair commit touched `crates/recorder/tests/red_metrics_emission.rs`, `milestones/M-09-data-safety-net.md`, and `scripts/verify_M-09.sh`. Scope is coherent for architect RED/verify repair.

All temporary prototypes and mutations were reverted before this re-audit #4 section was appended. Final pre-verdict worktree status was clean except `research/critiques/C-014-M-09-task4c.md`.

---

## Re-audit #5 — 2026-07-20T12:57:16Z

**Audited repair head:** `f815a87` — `test(M-09): C-014 re-audit #4 repair — book_levels symbol-collapse pin`
**Repair commits:** `f815a87` over re-audit #4 verdict `f5ecb88`
**Local audit worktree:** `/tmp/hft-critic-m09-t4c-r6` detached at `origin/feat/M-09-task4c-metric-emission`

### Re-audit #5 verdict

**REJECT remains.**

The repair closes the re-audit #4 blocker: `book_levels{venue,symbol,side}` now has a three-book matrix that independently pins venue, symbol, and side. The requested `symbol="BTCUSDT"` anti-placebo fails.

However, a seventh false-green remains: `recorder_rss_anon_bytes` is still only sample-presence checked. A sampler that always emits `0`, or a sampler that reads `VmRSS` instead of the mandated `RssAnon`, leaves `red_metrics_emission` and `verify_M-09.sh` green. That is material because FA §3 fixes the source to `/proc/<pid>/status` `RssAnon` specifically, and §7.1 TD-016 consumes this metric as the anonymous-heap leak signal.

### What is fixed

**Correct prototype remains reachable.**

Temporary correct implementation: `run_writer(..., Arc<Metrics>, shutdown)`, `metric_emit::{emit_book_levels,sample_md_age,sample_rss}`, `run_books_feeder`, and live `main` calls for feeder/samplers.

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 4 tests
test md_age_sampler_emits_real_age_per_venue ... ok
test rss_sampler_emits_anon_bytes ... ok
test live_feeder_loop_emits_book_levels ... ok
test writer_emits_journal_and_md_metrics ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
VERDICT: PASS
exit=0

cargo test -p recorder --test red_rss_bounded; echo exit=$?
test e7_writer_loop_memory_is_bounded_and_event_count_independent ... ok
exit=0
```

**`book_levels` symbol-collapse: CLOSED.**

Temporary mutation:

```rust
metrics.set_gauge(
    "book_levels",
    &[("venue", venue), ("symbol", "BTCUSDT"), ("side", "bid")],
    book.n_levels(Side::Buy) as i64,
);
metrics.set_gauge(
    "book_levels",
    &[("venue", venue), ("symbol", "BTCUSDT"), ("side", "ask")],
    book.n_levels(Side::Sell) as i64,
);
```

Result:

```text
cargo test -p recorder --test red_metrics_emission live_feeder_loop_emits_book_levels -- --exact; echo exit=$?
live_feeder_loop_emits_book_levels ... FAILED
left: Some(4)
right: Some(5)
exit=101

cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 4 tests
test live_feeder_loop_emits_book_levels ... FAILED
exit=101
```

The failure hits the first collapsed Binance/BTCUSDT bid assertion because the later Binance/ETHUSDT sample overwrites the hard-coded BTCUSDT series. That is enough: the oracle now detects symbol collapse.

### Remaining blocker — RSS sampler can be dead or wrong-source

Severity: BLOCKER.

Current `rss_sampler_emits_anon_bytes` asserts only `has_sample("recorder_rss_anon_bytes")`. That catches registry-only / helper-not-called, but not value liveness and not the source identity required by FA §3.

**Anti-placebo A: always-zero sampler still passes.**

Temporary mutation:

```rust
pub fn sample_rss(metrics: &Metrics) {
    metrics.set_gauge("recorder_rss_anon_bytes", &[], 0);
}
```

Result:

```text
cargo test -p recorder --test red_metrics_emission rss_sampler_emits_anon_bytes -- --exact; echo exit=$?
rss_sampler_emits_anon_bytes ... ok
exit=0

cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 4 tests ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
VERDICT: PASS
exit=0
```

This is the same dead-zero class already closed for `journal_seq_current`, `journal_disk_free_bytes`, and `md_event_age_ms`.

**Anti-placebo B: wrong source (`VmRSS`) still passes.**

Temporary mutation:

```rust
.find(|line| line.starts_with("VmRSS:"))
```

instead of:

```rust
.find(|line| line.starts_with("RssAnon:"))
```

Result:

```text
cargo test -p recorder --test red_metrics_emission rss_sampler_emits_anon_bytes -- --exact; echo exit=$?
rss_sampler_emits_anon_bytes ... ok
exit=0

cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 4 tests ... ok
exit=0
```

This is material, not stylistic. `docs/fa/ops.md` §3 says the source is fixed to `RssAnon` and explicitly rejects cgroup/docker-style memory because page cache produces false leak signals (TD-021). `VmRSS` is not identical to `RssAnon`; it includes additional resident pages and can reintroduce the same false-observation class.

Required repair:

1. Make `rss_sampler_emits_anon_bytes` assert the sampled value is `> 0` after `sample_rss(&metrics)`.
2. Add a deterministic parser/source oracle for `/proc/self/status`, e.g. `metric_emit::parse_rss_anon_bytes(status: &str) -> Option<i64>` or an equivalent testable seam.
3. Feed a fixture where `VmRSS` and `RssAnon` differ and assert the emitted/parsed value equals `RssAnon * 1024`, not `VmRSS * 1024`.

Example fixture shape:

```text
VmRSS:      9999 kB
RssAnon:     123 kB
RssFile:    9876 kB
```

Expected `recorder_rss_anon_bytes == 123 * 1024`.

### Scope / cleanup

The `f815a87` repair commit touched only `crates/recorder/tests/red_metrics_emission.rs`. Scope is clean.

All temporary prototypes and mutations were reverted before this re-audit #5 section was appended. Final pre-verdict worktree status was clean except `research/critiques/C-014-M-09-task4c.md`.

---

## Re-audit #6 — 2026-07-20T13:11:03Z

**Audited repair head:** `948896c` — `test(M-09): C-014 re-audit #5 repair — RssAnon parser-оракул (не VmRSS) + rss>0`
**Repair commits:** `948896c` over re-audit #5 verdict `b4fafcd`
**Local audit worktree:** `/tmp/hft-critic-m09-t4c-r7` detached at `origin/feat/M-09-task4c-metric-emission`

### Re-audit #6 verdict

**APPROVE.**

The re-audit #5 blocker is closed. `recorder_rss_anon_bytes` is no longer sample-only: the oracle now pins both source identity (`RssAnon`, not `VmRSS`) and positive live sample value. I found no eighth false-green of a new steady-emission class after this repair.

This approval is scoped to the plan-time RED oracle for M-09 task 4C. Event metrics remain correctly outside this steady-emission oracle unless their trigger is injected (`journal_write_errors_total`, `journal_seq_gaps_total`, `venue_ws_reconnects_total`, `book_resync_total`). Reviewer §8 still owns the live `/metrics` curl on VPS: all steady series present, non-zero where required, with correct labels/dimensions.

### What is fixed

**Correct prototype is reachable.**

Temporary correct implementation: `run_writer(..., Arc<Metrics>, shutdown)`, `metric_emit::{emit_book_levels,sample_md_age,parse_rss_anon,sample_rss}`, `run_books_feeder`, and live `main` calls for feeder/samplers.

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 5 tests
test rss_parser_reads_rss_anon_not_vmrss ... ok
test rss_sampler_emits_positive_anon_bytes ... ok
test md_age_sampler_emits_real_age_per_venue ... ok
test live_feeder_loop_emits_book_levels ... ok
test writer_emits_journal_and_md_metrics ... ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
VERDICT: PASS
exit=0

cargo test -p recorder --test red_rss_bounded; echo exit=$?
test e7_writer_loop_memory_is_bounded_and_event_count_independent ... ok
exit=0
```

**Parser `VmRSS` false-green: CLOSED.**

Temporary mutation:

```rust
if !trimmed.starts_with("VmRSS:") {
    return None;
}
```

instead of `RssAnon:`.

Result:

```text
cargo test -p recorder --test red_metrics_emission rss_parser_reads_rss_anon_not_vmrss -- --exact; echo exit=$?
rss_parser_reads_rss_anon_not_vmrss ... FAILED
left: 1023998976
right: 12641280
exit=101

cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 5 tests
test rss_parser_reads_rss_anon_not_vmrss ... FAILED
exit=101
```

This is the required TD-021 guard: the fixture has `VmRSS != RssAnon`, and the oracle requires `RssAnon * 1024`.

**Sampler dead-zero false-green: CLOSED.**

Temporary mutation:

```rust
pub fn sample_rss(metrics: &Metrics) {
    metrics.set_gauge("recorder_rss_anon_bytes", &[], 0);
}
```

Result:

```text
cargo test -p recorder --test red_metrics_emission rss_sampler_emits_positive_anon_bytes -- --exact; echo exit=$?
rss_sampler_emits_positive_anon_bytes ... FAILED
recorder_rss_anon_bytes == 0 после sample_rss
exit=101

cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 5 tests
test rss_sampler_emits_positive_anon_bytes ... FAILED
exit=101
```

### Convergence boundary

The steady-emission oracle now pins the classes that caused the C-014 sequence:

1. registry-only / HELP-TYPE-only output is rejected by sample-line checks;
2. labeled metrics require labels, not unlabeled samples;
3. label values are independently pinned for `md_events_total{venue,symbol,kind}` and `book_levels{venue,symbol,side}`;
4. side, kind, venue, and symbol collapse have anti-placebo coverage;
5. dead-zero is pinned for `journal_seq_current`, `journal_disk_free_bytes`, `md_event_age_ms`, and `recorder_rss_anon_bytes`;
6. `md_event_age_ms` value semantics are pinned by `now-last`;
7. RSS source identity is pinned to `RssAnon`, not `VmRSS`;
8. `journal_segment_index` remains presence-only because `0` is legitimate on the first segment;
9. `journal_bytes_written_total` remains `>0` by contract wording here: FA §3 uses it as "are we writing at all"; exact byte accounting beyond liveness is not required for this plan-time gate.

Remaining event/elsewhere metrics need trigger-specific coverage or reviewer §8, not another steady-emission oracle round. That includes `venue_ws_reconnects_total`, `journal_write_errors_total`, `journal_seq_gaps_total`, `book_resync_total`, `venue_http_status_total`, `book_divergence_bps`, and `backup_restore_drill_ok`.

### Scope / cleanup

The `948896c` repair commit touched `crates/recorder/tests/red_metrics_emission.rs` and `milestones/M-09-data-safety-net.md`. Scope is coherent for architect RED/spec repair.

All temporary prototypes and mutations were reverted before this re-audit #6 section was appended. Final pre-verdict worktree status was clean except `research/critiques/C-014-M-09-task4c.md`.
