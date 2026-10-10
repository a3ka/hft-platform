<!-- GATE-META
milestone: M-89
audited_repo: a3ka/hft-platform
audited_base: dcb435f80109030ab6efb3157f86df2f53b609d9
audited_head: 2fabb672f0b609a1e264a5a162c91a04b4c32c55
verdict: REJECT
-->

# C-260 — M-89 S0: volume guard and serving observability

## Verdict

**REJECT.** The committed set is complete, the measurement has a stated basis,
and the baseline is correctly RED: `verify_M-89.sh` exits 1, while the
`task-status` probe is 8/8 and the old journal/gateway corpora named by the
milestone remain green.  That is not sufficient to start dev.  Four contracts
can be made superficially green while either contradicting their own stated
semantics or failing to observe the production path:

1. `ServingInputs::default()` is simultaneously the legacy no-integration
   state and a missing heartbeat, although the tests require opposite results.
2. The new public journal API can be a stub; no RED test calls it.
3. The heartbeat composition accepts a directory as a file path, and `h1`
   replaces the compose path with a temporary good path before starting the
   binary.
4. The reader-byte counters are only bounded by a fixture-specific interval,
   not proved to be the bytes counted at the reader.

Do not implement tasks 1--11 from this artifact set.  Replan and amend the
milestone, RED set, and verify gate first; then submit a new architect chain.

## Audited subject

- Subject: `origin/feat/M-89-s0-volume-guard-observability`.
- Mandate SHA `2fabb67` was fetched as
  `2fabb672f0b609a1e264a5a162c91a04b4c32c55`; it is the audited tip and is
  an ancestor of that tip (exit 0).  The pre-write refetch found no drift.
- Base: `dcb435f80109030ab6efb3157f86df2f53b609d9`.
- This is a RAW journal-read gate: the intended change reaches
  `crates/journal/src/segments.rs`.  The live invariants are **JR-I-2** (no
  silent gap/reorder or partial delivery on corruption) and **JR-I-11**
  (every public catalog-stitching read path rejects a non-monotonic catalog).

The branch diff contains the milestone, measurement basis, seven new RED
files, the M-87 `u5_*` revision, the verify script, and its task-status probe.
No implementation path was audited as delivered; the RED baseline is the
object of this plan-time gate.

## Blocking findings

### R1 — `Option` cannot encode both disabled and missing heartbeat

**Evidence.** The required form makes `ServingInputs.heartbeat` an
`Option<ServingHeartbeatSample>` and states
`run_cycle(..) == run_cycle_full(.., &ServingInputs::default(), ..)`
([M-89 §4.3](../../milestones/M-89-s0-volume-guard-observability.md:175)).
`default()` is consequently `None`.  But `g1` requires `None` to emit
`ServingHeartbeatMissing` ([red_m89_serving_silence.rs](../../crates/ops/tests/red_m89_serving_silence.rs:88)),
and `g4` explicitly requires that same default input to emit Missing
([red_m89_serving_silence.rs](../../crates/ops/tests/red_m89_serving_silence.rs:237)).
Conversely `g4b` requires the old `run_cycle()` wrapper to emit neither
Missing nor Silence ([red_m89_serving_silence.rs](../../crates/ops/tests/red_m89_serving_silence.rs:255)).

The claimed equivalence supplies the same `None` to both calls.  Therefore an
implementation must either alert in the legacy wrapper (breaking g4b) or
suppress the real missing-file alert (breaking g1/g4 and OPS-I-8/OPS-I-10).
This is a contract contradiction, not an implementation choice.

**Required correction.** Replace the overloaded option with a three-state
form, for example `Disabled`, `ConfiguredMissing`, and
`Present(ServingHeartbeatSample)`.  `run_cycle()` must pass `Disabled`;
the binary, which is the only production full entrypoint, must pass
`ConfiguredMissing` after a configured path cannot be read.  Split g1/g4/g4b
and g5 accordingly, including an executable configured-missing binary case.

### R2 — the declared public journal API has no oracle and may be a no-op

**Evidence.** M-89 declares a public
`locate_after_seq(seg_path, after_seq) -> io::Result<Option<TailHint>>`
([milestone](../../milestones/M-89-s0-volume-guard-observability.md:92)) and
calls the exact-position guard JR-I-2 ([milestone](../../milestones/M-89-s0-volume-guard-observability.md:211)).
The only verify form check is a text grep for the signature
([verify_M-89.sh](../../scripts/verify_M-89.sh:81)).  The 894-line gateway RED
file contains no `locate_after_seq` reference (raw `rg` exits 1).  Its useful
end-to-end tests exercise `LiveReducer::pump`, but do not prove that the
public contract is the primitive used there.

**Passing mutant.** Add the exact public signature returning `Ok(None)`
unconditionally, then put a separate private search in `LiveReducer` (or in
the catalog path) which makes w1/d* pass.  The grep, all current M-89
behavioural RED tests, and the verify script go green although the declared
public API is a stub.  This violates `gates.md` §2: every public function
needs a test and a no-op must not pass.

**Required correction.** Add direct journal REDs through the exported
function for raw active and raw closed segments: exact `after+1`, empty tail
at EOF, wrong-seq candidate/fallback, CRC corruption, torn target frame, and
read-only input.  Separately prove that the first production pump invokes
this contract (or make the actual API private and specify/test the real
public entrypoint instead).  The implementation must retain the second
`resolve_active_start_offset` validation.

### R3 — heartbeat path composition can pass with an unusable producer path

**Evidence.** `h0` only requires a nonempty path beginning with a writable
mount destination ([red_m89_heartbeat_entrypoint.rs](../../crates/gateway-serve/tests/red_m89_heartbeat_entrypoint.rs:331)).
Thus `/state/` passes for a `gateway-state:/state` mount even though it names
a directory, not the heartbeat file required for `<path>.tmp -> rename`.
The production-binary helper then overwrites both heartbeat environment
variables with a temporary good file before spawn
([red_m89_heartbeat_entrypoint.rs](../../crates/gateway-serve/tests/red_m89_heartbeat_entrypoint.rs:300)).
The verify composition repeats the same prefix test and builds a host path
without requiring a nonempty leaf
([verify_M-89.sh](../../scripts/verify_M-89.sh:157)).

**Passing mutant.** Set compose to `GATEWAY_HEARTBEAT_PATH: /state/` and cron
to the volume root.  h0 and task11 accept the prefix; h1 still writes its
temporary `gateway-serve.heartbeat`.  In deployment the atomic rename targets
a directory, the writer swallows/logs the error as the milestone requires,
and the watchdog only observes MISSING.  The claimed external heartbeat is
therefore not established.

**Required correction.** Make h0/task11 reject a mount root, trailing slash,
or empty relative leaf.  Make h1 use the exact compose value on an actual
writable mount fixture (do not override it); assert the file can be created,
atomically replaced, and maps to the exact cron host path.  Preserve the
named honest limit that installation in `/etc/cron.d` is manual.

### R4 — `payload_bytes_read` is not proved to come from the reader

**Evidence.** The form says the counter is incremented at the read site and
includes frames, headers, and boundary probes
([milestone](../../milestones/M-89-s0-volume-guard-observability.md:102)); it
then promises the value is fed from `EventStream` on every path
([milestone](../../milestones/M-89-s0-volume-guard-observability.md:273)).
`p1` only asserts `tail_bytes <= payload_bytes_read <= rchar`
([red_m89_warm_resume_seek.rs](../../crates/gateway/tests/red_m89_warm_resume_seek.rs:423)).
The WS oracle has the similarly loose
`checkpoint + tail <= counter <= rchar` range
([red_m89_read_volume_truth.rs](../../crates/gateway-serve/tests/red_m89_read_volume_truth.rs:270)).
Neither calls `EventStream::payload_bytes_read` directly.

**Passing mutant.** On the M-89 fixtures, report `events_scanned * 64` (or
another synthetic estimate above the fixed tail lower bound and below the
kernel rchar allowance) instead of incrementing at reads.  It is not the
declared byte count, omits header/probe accounting, and still satisfies both
ranges.  A duplicated counter can similarly feed transport while the public
reader method returns zero.

**Required correction.** Test the reader method directly with deliberately
different frame sizes and a probe/fallback case; compare it to a controlled
counting reader or exact framed byte accounting.  Then make the WS test prove
that it receives that same per-pump reader delta, not merely a plausible
interval.

### R5 — verify does not substantiate its stated CI parity

M-89 calls the final three commands “CI-parity”
([verify_M-89.sh](../../scripts/verify_M-89.sh:192)), but CI also contains,
among others, audit, delivery, artifact-ID, protected-artifact, gate-meta,
resource-oracle, and rollout-composition commands.  The raw CI enumeration is
in the Done Block.  `gates.md` §3 requires an explicit verify point for each
CI `run`, with specialised jobs selected or explicitly waived by touched
scope; this M-89 script has neither a mapping nor waivers.  It touches
`scripts/**` and deployment composition, so the omission cannot be treated as
an unexamined base-Rust-only plan.  Add a checked mapping/waiver table and
execute the applicable specialised points.

## Required non-blocking repairs / coverage notes

### N1 — corruption coverage stops at the sixth tail event

The exact-position guard itself is sound for a false candidate: accepting
only `seq == after + 1`, otherwise failing back to `header_end`, prevents a
CRC coincidence/bisection from silently jumping a valid event.  d6 also
distinguishes an incomplete target frame from corruption.  d7 corrupts only
tail event `k=0` and d7b only `k=5`
([red_m89_warm_resume_seek.rs](../../crates/gateway/tests/red_m89_warm_resume_seek.rs:687)).

A mutant confined to the new seek path can return EOF after a frame error
only once more than 16 tail events have been delivered.  d7/d7b still fail
closed, while corruption at `TAIL/2` is silently truncated.  Add d7c with
corruption materially later in the tail and assert `Err`, unchanged cursor,
and equality with the independent `gateway::snapshot` failure.  This is
needed to substantiate the blanket §5.2 claim for corruption farther in the
tail and JR-I-2.

### N2 — reader-scope answer: stream/stream_from are protected; stream_from_at needs naming

Current `stream()` delegates to `stream_from`, which has its own body
([segments.rs](../../crates/journal/src/segments.rs:1827)); it does not enter
`stream_from_at_with_catalog`.  The existing `red_stream_from` all-after and
boundary corpus plus `red_stitch_monotonic` exercise JR-I-2/JR-I-11, and M-89
runs `cargo test -p journal`.  Thus the §5.3 restriction and forbidden list
are adequate for `stream`/`stream_from` if held as written.

However public `stream_from_at()` delegates directly to the proposed changed
function ([segments.rs](../../crates/journal/src/segments.rs:1947)), while
§5.3/§10 names only `stream`/`stream_from`.  State its required compatibility
and add a direct `stream_from_at(..., None)` equivalence/no-loss test; that is
the remaining public reader which could otherwise inherit seek behaviour.

### N3 — structural delivery and the removed M-87 budget

`s1` is a genuine v1 WS producer-path oracle with setup guards and kernel
`rchar` ([red_m89_structural_bound.rs](../../crates/gateway-serve/tests/red_m89_structural_bound.rs:221)).
The legacy WS session is a second ingress mode, but both modes call the shared
`LiveReducer::pump`; no separate journal-read implementation was found.  Keep
an explicit legacy warm-resume assertion, or state that common-pump coverage
is the intended transfer argument.

The `u5_*` deletion itself is correctly narrow: only the old budget form
tests and `NeverCancel` were removed, the M-87 registry stays green, and s1
is the replacement product oracle.  The task5 source scanner is nevertheless
not a proof of complete removal: it matches only particular `struct`/`enum`/
`trait` spellings and strips only `//` comments.  A `type CallBudget = ...`,
renamed copy, or block-comment artifact can evade/misclassify it.  Keep s1,
but add a precise source prohibition (or compile-negative surface test) for
the public budget names if deletion is contractual.

### N4 — TD-225 guard counts, but does not bind each lock to its test

The SERIAL guard compares only the number of `#[tokio::test]` attributes to
the total number of `SERIAL.lock().await` lines
([red_m89_serial_guard.rs](../../crates/gateway-serve/tests/red_m89_serial_guard.rs:39)).
An async test can read global counters before locking while another test adds
a second lock; equality still holds.  Require a per-function lexical check
that the first executable synchronization action is the lock, or keep SERIAL
until the i1 mutation proves process-global contamination cannot matter.

## Done Block

```text
$ git fetch origin; git rev-parse origin/feat/M-89-s0-volume-guard-observability
2fabb672f0b609a1e264a5a162c91a04b4c32c55
$ git merge-base origin/main 2fabb672f0b609a1e264a5a162c91a04b4c32c55
dcb435f80109030ab6efb3157f86df2f53b609d9
$ git merge-base --is-ancestor 2fabb67 origin/feat/M-89-s0-volume-guard-observability
exit=0
$ pre-write git fetch origin; git rev-parse origin/feat/M-89-s0-volume-guard-observability
2fabb672f0b609a1e264a5a162c91a04b4c32c55
drift=none

$ artifact presence check
PRESENT milestone; measurement basis; red_m89_warm_resume_seek;
PRESENT five gateway-serve M-89 RED files; red_m89_serving_silence;
PRESENT red_m87_admission revision; verify_M-89; task-status probe
exit=0

$ bash scripts/tests/red_verify_M-89_task_status.sh
ok s1-all-done rc=0
ok s2-open-with-remainder rc=0
ok s3-open-silent rc=1
ok s4-2bis-silent rc=1
ok s5-yellow-silent rc=1
ok s6-yellow-with-remainder rc=0
ok s7-2б-silent rc=1
ok s8-names-offender
итого ok=8 fail=0
VERDICT: PASS
probe_exit=0

$ bash scripts/verify_M-89.sh
FAIL  task1: в crates/journal/src/segments.rs нет pub fn locate_after_seq / EventStream::payload_bytes_read — форма §4.1 не выполнена
PASS  task1: sacred-корпус journal зелен (JR-I-2/JR-I-11 не задеты) — test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
FAIL  task1+2+3: red_m89_warm_resume_seek (w1/p1/d1..d9) КРАСЕН — test result: FAILED. 4 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out; finished in 100.63s
FAIL  task2: в ReadStats нет seek_fallbacks — «сдвиг не сработал, всё перечиталось» невидимо в статистике
FAIL  task3: payload_bytes_after_cursor всё ещё в crates/gateway/src/lib.rs (3 упоминаний) — опись каталога на каждую выдачу
PASS  task2/D-1(б): существующий корпус gateway зелен (hint, тик, каталог не регрессировали)
FAIL  task4: red_m89_read_volume_truth (v) КРАСЕН — test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.02s
PASS  task4: red_m87_read_volume_truth::q2 остался зелёным — test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.19s
FAIL  task5: бюджет-плацебо ещё жив — упоминаний в admission.rs: 6, файлов тестов с вызовами: 0 (TD-219: подключать его значило бы подключить дефект)
FAIL  task5/I-3: red_m89_structural_bound (s1, прод-форма 200k, rchar) КРАСЕН — test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 82.75s
FAIL  task6: red_m89_counters_instance (i1/i2) КРАСЕН — компиляция
error[E0425]: cannot find type `ServingCountersHandle` in module `gateway_serve::metrics`
error[E0599]: no method named `counters_handle` found for struct `gateway_serve::server::Server` in the current scope
PASS  task7: red_m87_entrypoint под --features testing — test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.10s
PASS  task7: red_m87_entrypoint В ФОРМЕ CI (без флага) — test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.14s
PASS  task7: red_m87_registry (биекция сценариев цела) — test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
PASS  task7/TD-225: red_m89_serial_guard (tokio::test == SERIAL.lock, пока SERIAL объявлен) — test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
FAIL  task8+9: red_m89_heartbeat_entrypoint (h0 compose, h1 прод-бинарь) КРАСЕН — test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.08s
PASS  task9: red_m87_prod_entrypoint_argv остался зелёным — test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.03s
FAIL  task10: crates/ops целиком (red_m89_serving_silence + существующие) КРАСЕН — компиляция
FAIL  task11: compose не объявляет GATEWAY_HEARTBEAT_PATH у gateway-serve — композицию сверять не с чем
SKIP  task11: установка deploy/cron.d/watchdog в /etc/cron.d и сборка ops-watchdog на VPS — ручной шаг founder ★
SKIP  task12: прод-замер ДО/ПОСЛЕ — reviewer §8-gate
PASS  task-status: каждая из 12 открытых/частичных задач НАЗЫВАЕТ свой остаток
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 13)
verify_exit=1

$ rg -n 'locate_after_seq' crates/gateway/tests/red_m89_warm_resume_seek.rs
locate_red_reference_exit=1
$ grep -E '^\s*run:' .github/workflows/ci.yml | head
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo audit
bash scripts/verify_delivery_M-08.sh
bash scripts/check_roadmap_sync.sh
bash scripts/check_protected_artifacts.sh
bash scripts/check_artifact_ids.sh
bash scripts/check_gate_meta.sh
bash scripts/check_resource_oracles.sh

$ bash scripts/reserve_artifact_id.sh C
C-260
exit=0
$ git show -s --format='%s' refs/reserved-cache/C-260
reserve C-260 nous 2026-09-27T14:24:47Z Ubuntu-2404-noble-amd64-base 603094 b82c44bb-001b-4a72-a4ec-56932db509e3
```
