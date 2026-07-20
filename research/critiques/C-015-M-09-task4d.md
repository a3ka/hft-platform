# C-015 — M-09 task 4D metric-contract audit

**UTC:** 2026-07-20T14:48:34Z  
**Branch audited:** `origin/feat/M-09-task4d-metric-contract` at `4b7ed66`  
**Base:** `e61dd3a` (`origin/main`)  
**Worktree:** `/tmp/hft-critic-m09-t4d`  
**Verdict:** REJECT

## Scope Read

Read:

- `docs/fa/ops.md` §3 and §7.1
- `milestones/M-09-data-safety-net.md` task 3, task 4C, task 4D, and allowed paths
- `crates/recorder/tests/red_metrics_emission.rs`
- `scripts/verify_M-09.sh` OPS-I-5 / OPS-I-10 canaries
- `crates/ops/src/{metrics.rs,alerts.rs}` and `deploy/alerts/ops.rules.yml` for rename reachability

Repair chain over `main`:

```text
233b863 docs(fa/ops): NOTE-1/NOTE-2 (TD-027) — frames-rename + seq_gaps read-side
028fe08 test(M-09): task 4D — journal_frames_written_total (NOTE-1 rename)
447e9bf docs(M-09): task 4C DONE (прод); task 4D NOTE-1/NOTE-2; task 3 +seq_gaps продюсер
4b7ed66 docs(M-09): task 4D — coherence, frames-rename в task-4C spec-тексте (58/275)
```

## Finding

### BLOCKER — Milestone still tells engine-dev `run_writer` emits `journal_seq_gaps_total`

`milestones/M-09-data-safety-net.md:56-60` still says the task-4C metric-emission carve-out makes `run_writer` emit:

```text
journal_frames_written_total / journal_seq_current /
journal_segment_index / journal_disk_free_bytes / journal_write_errors_total / journal_seq_gaps_total
```

That contradicts the NOTE-2 repair in the same branch:

- `docs/fa/ops.md:77` says `journal_seq_gaps_total` is READ/REPLAY-side, deferred to restore-drill task 3, not writer.
- `docs/fa/ops.md:91` says writer seq is monotonic by construction and gaps are detected only by read/replay.
- `docs/fa/ops.md:446` says OPS-GAP producer is restore-drill/replay, task 3.
- `milestones/M-09-data-safety-net.md:103` correctly assigns `journal_seq_gaps_total` producer to task 3.
- `milestones/M-09-data-safety-net.md:106` correctly states task 4D reclassifies seq gaps as read-side.
- `scripts/verify_M-09.sh:125` keeps `journal_seq_gaps_total` in `EVENT_OR_ELSEWHERE`, which agrees with read-side/deferred, not steady writer emission.

This is not cosmetic. Task 4D exists to remove two false guarantees from TD-027. Leaving one live milestone instruction that says writer emits `journal_seq_gaps_total` reintroduces NOTE-2 ambiguity and can route engine-dev to implement the wrong producer. It also makes the allowed-path/spec text internally inconsistent: task 3 owns the read-side producer, while the allowed-path task-4C text still claims writer ownership.

Required repair:

1. Remove `journal_seq_gaps_total` from the `run_writer` emit list in `milestones/M-09-data-safety-net.md:58-60`.
2. Replace it with explicit text matching FA §3: `journal_seq_gaps_total` is read/replay-side, deferred to task 3 restore-drill; before task 3, value `0` is legitimate.
3. Keep `journal_write_errors_total` as writer event metric; that remains correct.

## Verified Good

### NOTE-1 rename RED is reachable

Against current branch head, before prototype, the RED suite fails in the intended way:

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
writer_emits_journal_and_md_metrics ... FAILED
journal_frames_written_total == 0 после 72 append'ов
exit=101

bash scripts/verify_M-09.sh; echo exit=$?
FAIL T4C OPS-I-10 живая ЭМИССИЯ метрик
FAIL OPS-I-5 §7.1 ссылается на метрику(и) вне METRICS: journal_frames_written_total
FAIL OPS-I-5 метрика(и) кода вне §3: journal_bytes_written_total
FAIL OPS-I-10 метрики без проверки РАНТАЙМ-эмиссии: journal_bytes_written_total
VERDICT: FAIL (4)
exit=1
```

Temporary prototype:

- `crates/ops/src/metrics.rs`: `journal_bytes_written_total` → `journal_frames_written_total`
- `crates/ops/src/alerts.rs`: TD-011 rule metric and PromQL renamed to `journal_frames_written_total`
- `crates/recorder/src/lib.rs`: `run_writer` increments `journal_frames_written_total`
- `crates/recorder/src/metric_emit.rs`: producer-map comment renamed
- `deploy/alerts/ops.rules.yml`: regenerated from `cargo run -p ops --example dump_rules`

Prototype results:

```text
cargo test -p recorder --test red_metrics_emission; echo exit=$?
running 5 tests ... ok
exit=0

cargo test -p ops --test red_ops_alerts; echo exit=$?
running 5 tests ... ok
exit=0

cargo test -p ops; echo exit=$?
all ops tests ok
exit=0

bash scripts/verify_M-09.sh; echo exit=$?
VERDICT: PASS
exit=0
```

All prototype changes were reverted before this verdict file was written.

### NOTE-1 active references are coherent after prototype

Full grep after the prototype showed active code/docs/deploy on the new name:

- `crates/ops/src/metrics.rs`
- `crates/ops/src/alerts.rs`
- `crates/recorder/src/lib.rs`
- `crates/recorder/src/metric_emit.rs`
- `crates/recorder/tests/red_metrics_emission.rs`
- `deploy/alerts/ops.rules.yml`
- `docs/fa/ops.md`
- `milestones/M-09-data-safety-net.md`

Remaining `journal_bytes_written_total` occurrences after prototype were historical/reviewer-owned or prior-audit text:

- `PROJECT-STATE.md`
- `TECH-DEBT.md`
- `research/critiques/C-014-M-09-task4c.md`
- task 4D prose explaining the rename

I am not treating those as a task 4D plan-time blocker. `PROJECT-STATE.md` and `TECH-DEBT.md` are reviewer-owned close-out surfaces; C-014 is historical audit context. Reviewer should update state/debt at close-out if the branch is approved after the milestone blocker above is fixed.

### NOTE-2 concept is honest in FA

The FA wording is correct: writer seq is monotonic by construction, so write-side gap detection is not a meaningful producer. OPS-GAP becomes a read/replay-side signal: `journal_seq_gaps_total == 0` before task 3 is legitimate because the restore-drill/replay producer is not yet implemented. That is an honest deferral, not a false guarantee, as long as the milestone does not simultaneously tell engine-dev to wire it in `run_writer`.

The verify emission canary agrees with the intended deferral by classifying `journal_seq_gaps_total` under `EVENT_OR_ELSEWHERE`.

## Verdict

REJECT until the milestone allowed-path/task-4C producer text is made coherent with NOTE-2.

After that repair, the plan is likely approvable: NOTE-1 rename reachability and anti-placebo are solid, and the NOTE-2 FA contract is honest.

## Re-audit — 2026-07-20T18:19:01Z

Verdict: APPROVE.

Audited repair head: `f7d828a` (`docs(M-09): C-015 repair — seq_gaps убран из run_writer emit-списка (read-side)`). The handoff's `6f9d68e` reference appears stale; the fetched remote head is `f7d828a`.

### Prior blocker

CLOSED. `milestones/M-09-data-safety-net.md:58-60` no longer lists `journal_seq_gaps_total` in the `run_writer` emission set. The writer list is now:

- `journal_frames_written_total`
- `journal_seq_current`
- `journal_segment_index`
- `journal_disk_free_bytes`
- `journal_write_errors_total` as a writer-event on append error
- `md_events_total`

This matches FA §3/§7.1 and NOTE-2: writer seq is monotonic by construction, so `journal_seq_gaps_total` is a read/replay-side signal, deferred to task 3 restore-drill/replay. `journal_write_errors_total` correctly remains in the writer path because append failure is a writer-side event.

### Grep audit

`rg -n "journal_seq_gaps_total|journal_frames_written_total|journal_bytes_written_total"` confirms the active plan surfaces are coherent:

- FA §3/§7.1 assigns `journal_seq_gaps_total` to restore-drill/replay, not writer.
- Milestone task 3 owns the `journal_seq_gaps_total` producer.
- Milestone task 4D documents NOTE-2 reclassification only.
- `scripts/verify_M-09.sh` keeps `journal_seq_gaps_total` in `EVENT_OR_ELSEWHERE`, consistent with read-side deferral.
- `crates/recorder/tests/red_metrics_emission.rs` still expects `journal_frames_written_total`, so NOTE-1 did not regress.

Remaining old-name or pre-implementation references are non-blocking in this plan-time state:

- Current `crates/ops/src`, `crates/recorder/src`, and `deploy/alerts` still use `journal_bytes_written_total` until engine-dev performs the approved rename.
- `PROJECT-STATE.md` and `TECH-DEBT.md` are reviewer-owned historical/close-out surfaces.
- Earlier C-015/C-014 critique text is historical audit context.

### NOTE-1 regression check

No regression found. The repair changed only `milestones/M-09-data-safety-net.md`; the already-validated NOTE-1 RED oracle remains intact. Active FA/milestone/test surfaces require `journal_frames_written_total`, while current implementation remains RED on the old name until engine-dev executes the rename.

### Next handoff

APPROVE to engine-dev:

- Rename `journal_bytes_written_total` → `journal_frames_written_total` in `ops::metrics::METRICS`, `ops::alerts`, deploy rule rendering, `recorder::run_writer`, and `metric_emit` comments/helpers.
- Do not add a writer producer for `journal_seq_gaps_total`.
- Keep NOTE-2 doc-only for task 4D; the `journal_seq_gaps_total` producer belongs to task 3 restore-drill/replay.

risk-critic N/A: MD-only contract repair and metrics rename plan, no order-egress path.
