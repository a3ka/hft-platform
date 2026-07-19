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
