<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: 6299dbfb2550781a0cc311d858ddf2ab747c0cbc
verdict: NOTE
-->

# C-244 — M-87 serving circuit breaker, round 7: NOTE

## Verdict

**NOTE — A-038 D-2/D-3 is executed; M-87 may proceed to implementation.**

This round judges only the execution required by `A-038`: the FIFO release form,
the panic-safe cleanup, restoration before C4's positive control, and the explicit
slot-to-catalog order. It does not reopen A-037's transport/library boundary,
R3-1, R4-2, or R5-1.

The sole non-blocking note is platform expression. `LatchGuard` uses the Linux
numeric `O_NONBLOCK = 0o4000`, but the exact `#[cfg(target_os = "linux")]` form
required by A-038 §5 U-1 is not present. This does not weaken this execution on
the declared target—C4 already depends on `mkfifo`, `std::os::unix`, and CI is
Ubuntu—but architect should make the target restriction explicit before any
non-Linux test target is introduced.

## Audited committed artifact set

| Required artifact | Evidence | Result |
|---|---|---|
| Milestone and T2 contract/trait forms | `milestones/M-87-serving-circuit-breaker.md:94-236` specifies the ordered public path and the `ServingOutcome`, `AdmissionPolicy`, `CallBudget`, `Cancel`, and `ServingSlots` forms. | PASS |
| RED oracles | `crates/gateway-serve/tests/red_m87_admission.rs` and `red_m87_entrypoint.rs`; C4 is a real WS entrypoint oracle. | PASS / expected COMPILE-RED |
| Acceptance gate | `scripts/verify_M-87.sh` is a fail-counting gate with CI parity and returns non-zero for its RED baseline. | PASS |
| T1 / Block-C | `d5163b5..6299dbf` contains no `contracts/**`; the range changes only the C4 oracle and milestone. | PASS |

`gateway-serve` has no dedicated FA. This audit applies `PL-I-4` (`docs/DESIGN.md:940`:
N clients must not create N journal scans), `VB-I-11`, and `OPS-I-8/10`; this is
the same declared FA waiver boundary as A-038.

## A-038 execution

### D-2(a) and D-2(b) — PASS

`release_latch` opens only with `OpenOptions::write(true)`—the write-only FIFO
rendezvous—and never combines it with `read(true)` / `O_RDWR`. In the same writer
thread the committed order is `open → remove_file → drop(f)`. Thus the directory
entry disappears while the writer is still open; a late reader cannot find a
retained FIFO after close.

### D-2(c) — PASS, executed forced-failure probe

`LatchGuard` is created immediately after `latch_segment` and its `Drop` does a
non-blocking write open, `remove_file`, then close. The source RED target cannot
be run to C4 yet because its prescribed future modules intentionally do not
compile, so I compiled an isolated Rust probe containing the committed
`LatchGuard::drop` body verbatim. It held a FIFO reader, forced the C4
hold-guard panic, then joined the blocked reader—the equivalent of the runtime
teardown that previously hung. Cleanup reached EOF and the deliberate test
failure exited `101` under `timeout 10`, not `124`.

### D-2(d) — PASS

Before C4 sends C, the oracle asserts that the latch path is absent. It then
enumerates `journal::list_segments` on a `std::thread`, bounds the receive to ten
seconds, and asserts that the resulting segment count equals the pre-latch
catalog. A retained FIFO, a blocked listing, or a changed catalog is therefore a
named failure before the free-slot positive control.

### D-3 — PASS

Milestone §4.0bis explicitly chooses `admit → try_acquire → readiness → work`,
with catalog access inside the acquired slot and rationale through `PL-I-4`.
C4 holds the first public request at the first segment access and requires
`slots_in_flight == 1`; an implementation that runs readiness before acquiring
the slot fails that assertion.

## Done Block

```text
$ git rev-parse HEAD; git merge-base origin/main HEAD
6299dbfb2550781a0cc311d858ddf2ab747c0cbc
d5163b5b35abbca204a8981e974bd6e5a97eb9de
exit=0

$ git diff --name-status 21ef0531172ad2c95c98970677f9d6e95eccfe1f..6299dbfb2550781a0cc311d858ddf2ab747c0cbc
M	crates/gateway-serve/tests/red_m87_entrypoint.rs
M	milestones/M-87-serving-circuit-breaker.md
exit=0

$ bash scripts/next_artifact_id.sh C
C-244
exit=0

$ bash scripts/check_gate_meta.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de
── GATE-META: диапазон d5163b5b..HEAD, origin=a3ka/hft-platform
VERDICT: PASS — вердиктов проверено: 8, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ timeout 10 /tmp/hft-critic-m87-r7-probe-*/latch_guard_probe
PROBE: forced C4 hold-guard failure
PROBE: forced C4 hold-guard failure completed cleanup; no hang
probe_exit=101

$ CARGO_TARGET_DIR=/tmp/hft-critic-m87-r7-ZyIVRi-target bash scripts/verify_M-87.sh
FAIL  task1+3+4+7: red_m87_admission КРАСЕН — компиляция (11 unresolved future-form errors)
FAIL  task2+3+5+6+8: red_m87_entrypoint КРАСЕН — компиляция (3 unresolved future-form errors)
PASS  D-1(а): библиотечный корпус тронут только добавлениями red_m87_*
PASS  D-1(б): библиотечный корпус зелен — passed=223 failed=0
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 11)
exit=1 (expected pre-implementation COMPILE-RED baseline)
```

## Handoff

- Result: NOTE; the A-038 execution gate is passed and does not block dev.
- Architect owns the non-blocking `#[cfg(target_os = "linux")]` expression note and
  the mechanical appendix to the milestone.
- Next implementation agent must preserve C4's order: acquire the serving slot before
  readiness/catalog access, then release only with the completion of work.
