<!-- GATE-META
milestone: M-84
audited_repo: a3ka/hft-platform
audited_base: 4196ff79244dd3b45f331d5417aae243bad7d1ab
audited_head: 2c709d72d95fff80db73a8eace87ff602d829eec
verdict: REJECT
-->

# C-220 — M-84 fixed depth bands, round 1

## VERDICT: REJECT

The five committed plan artifacts are present: one milestone, three architect-owned RED
suites, and `scripts/verify_M-84.sh`. No T1 contract changes are proposed; the planned T2
surface is the public `gateway::CANONICAL_DEPTH_BANDS` declaration. The RED state is honest:
each suite fails only with missing `CANONICAL_DEPTH_BANDS` (one error class, not a second
compile defect). `other_axes_still_split` is correctly intended to remain GREEN and protects
the boundary that M-84 removes one of six fingerprint axes, not every per-instrument split.

The bundle nevertheless does not pin its most important promise: a client-supplied set must
produce an explicit wire rejection, never silently become the canonical set. Three further
acceptance obligations are declared but have no executable oracle.

`MD-I-8` is the live invariant named from `docs/fa/viz-backend.md`: the depth series follows
the book on `L2Delta` at its declared cadence. Its cadence prerequisite is genuinely
**executed**, not live: M-68 (`7ed83f1`) is an ancestor of the audited head. In contrast,
`TD-159` remains OPEN/MAJOR and the FA explicitly states that the emission barrier during
depth recovery does not exist. M-84 states this distinction correctly; the verify gate does
not prove it.

## What was audited

- Subject branch: `feat/M-84-fixed-bands`.
- Range: `4196ff79244dd3b45f331d5417aae243bad7d1ab..2c709d72d95fff80db73a8eace87ff602d829eec`.
- Commits: `67e4e3d` (milestone) and `2c709d7` (three RED files plus verify gate).
- Paths: `milestones/M-84-fixed-depth-bands.md`, the two `crates/gateway/tests/` RED files,
  `crates/gateway-serve/tests/red_fixed_bands_wire.rs`, and `scripts/verify_M-84.sh`.

## Blockers

### B-1 — the transport oracle can be bypassed by silent parser-side canonicalization

**Where.** `crates/gateway-serve/tests/red_fixed_bands_wire.rs:20-22` explicitly declines to
execute the socket/parser path and calls `session::validate_selector` directly at :32, :49,
and :58. The real subscribe path parses at `crates/gateway-serve/src/lib.rs:742` and invokes
that validator only later at :808. `wire_v1::parse_selector` is the writable seam at
`crates/gateway-serve/src/wire_v1.rs:120-143`.

**Reproduction.** In an isolated detached worktree at the audited head, I supplied the
planned canonical constant and crate guard, made the direct session validator reject any
non-empty client bands, and then silently replaced `sel.bands` with the canonical vector in
`parse_selector` before the real validator. The input wire path would therefore accept a
foreign set and calculate a canonical one, while every M-84 RED suite passed.

```text
canonical rc=0
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
fingerprint rc=0
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
wire rc=0
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
silent replacement location:
135:    // Deliberately adversarial mutation: erase every client-supplied set before validation.
136:    sel.bands = gateway::CANONICAL_DEPTH_BANDS.to_vec();
```

This is the forbidden behavior in `milestones/M-84-fixed-depth-bands.md:99`, but the test
suite accepts it. The equality in `red_fixed_bands_fingerprint.rs:42-49` is also tautological:
both selectors are already canonical, so it cannot show that the client boundary discarded no
input.

**Condition to remove.** Add a RED entrypoint oracle that sends a v1 `subscribe` containing a
foreign `selector.bands` through `parse_message` → `parse_selector` → the session dispatch and
asserts `invalid_selector` with a bands-specific explanation and no `LiveReducer` construction.
Run the same path with absent `bands`; it must construct the canonical selector and emit the
canonical fourteen `(band, side)` rows. A direct unit call to `session::validate_selector` is
not a substitute.

### B-2 — no oracle rejects f32-rounded look-alikes of the canonical set

**Where.** The crate test checks a different one-element set at
`crates/gateway/tests/red_fixed_bands_canonical.rs:101-112`, a subset at :115-124, and a
reordered exact set at :127-134. It never presents all seven canonical values after f32
rounding. The comparison at :61-67 validates the constant against the document; it is not an
input-validation case.

**Reproduction.** The isolated implementation used an elementwise tolerance of `1e-6` in the
crate guard. The actual f32 representation of `0.015` differs by only
`0.000000000335276126`, so this non-canonical client set is accepted and gets its own
fingerprint. All twelve M-84 RED tests still passed.

```text
canonical rc=0; test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
fingerprint rc=0; test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
wire rc=0; test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
got=0.014999999664723873 delta=0.000000000335276126
2739:            .all(|(got, want)| (got - want).abs() < 1e-6);
```

**Condition to remove.** Add a crate-level RED case whose seven input values are each
`(CANONICAL_DEPTH_BANDS[i] as f32) as f64` and require rejection. The implementation must
compare the ordered values exactly for this invariant (for example, `to_bits`), not by length
or tolerance. The existing reorder oracle correctly catches a length-only guard; that mutation
failed as required.

```text
length_only_mutation rc=101
test crate_rejects_same_values_in_other_order ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s
```

### B-3 — the promised fourteen-row wire result has no RED oracle

**Where.** The milestone promises it at
`milestones/M-84-fixed-depth-bands.md:142-145` and :157-160. The wire suite expressly says it
does not test frame composition at `crates/gateway-serve/tests/red_fixed_bands_wire.rs:20-22`.
The verify gate merely runs that suite at `scripts/verify_M-84.sh:48` and contains no separate
frame assertion.

**Reproduction.** The M-84 test files contain no invocation of `parse_message`,
`parse_selector`, `LiveReducer`, snapshot/frame serialization, or a count of 14. Therefore a
transport that rejects the direct unit input but emits zero, one, or a subset of depth rows on
the actual success path can satisfy the committed set.

**Condition to remove.** Add an entrypoint RED oracle on an accepted no-bands subscribe with a
depth fixture that exercises both sides. Assert, after the real snapshot/frame serialization,
exactly fourteen rows in canonical band order and `(band, side)` identity. Wire it as its own
verify step; executing the validator-only suite must not stand in for it.

### B-4 — rollout and the blocked production enablement are asserted by text, not behavior

**Where.** M-84 requires coexistence of the old and prewarmed new checkpoint at
`milestones/M-84-fixed-depth-bands.md:114` and :159-160; the measured cold build is 965 s
against a 900 s interval at :42-54. Yet `scripts/verify_M-84.sh:62-67` only searches
`deploy/README.md` for the words `заранее` and `ckpt-`. The script itself says it does not
judge the real checkpoint operation at :7-11.

For task 6, the specification correctly keeps only `TD-159` and the missing emission barrier
live (`milestones/M-84-fixed-depth-bands.md:64-75`); it correctly names M-68 as executed.
However, the purported reverse guard at `scripts/verify_M-84.sh:69-73` checks only the compose
default and the literal `TD-159` in the milestone. It neither checks the second barrier nor
distinguishes an OPEN card from text saying that the card is closed.

**Reproduction.** On the audited revision M-68 is an ancestor, while TD-159 remains MAJOR and
the FA says the emission barrier is absent. The current verify command records twelve expected
pre-implementation failures, including both task-5 keyword searches; replacing those keywords
with prose after implementation would make the rollout step pass without prewarming or
coexisting snapshots.

```text
m68_ancestor_rc=0
TD-159: Severity: MAJOR
TD-159: Блокирует `П-014` п.4
Барьера, удерживающего эмиссию до восстановления глубины, по-прежнему НЕТ
verify_rc=1
FAIL: grep -qi 'заранее' deploy/README.md
FAIL: grep -q 'ckpt-' deploy/README.md
PASS: grep -q 'GATEWAY_BANDS:-0.001' docker-compose.yml
PASS: grep -qE 'TD-159' milestones/M-84-fixed-depth-bands.md
VERDICT: FAIL (12)
```

**Condition to remove.** Add an executable checkpoint oracle that creates/prewarms the new
fingerprint while the old checkpoint remains readable, then proves both filenames coexist and
the switch selects the ready new file. The deployment Done Block must execute the production
argv/runbook on the VPS, not only grep it. Add a fail-closed task-6 guard that models both
preconditions separately: TD-159 point-level provenance and recovery-time emission suppression.
It must keep `GATEWAY_BANDS=0.001` until both concrete mechanisms are GREEN; a milestone-text
match is not evidence of either.

## Checks that passed

- Scope: the audited range modifies only architect-owned milestone, RED-test, and verify paths.
- Artifact identity and history: the two declared commits and five paths match the supplied
  range; whitespace check passed.
- Compile-RED causality: all three suites have only `E0425` for the intentionally absent
  canonical declaration. There is no repeat of the old private-`Venue` compile failure.
- `other_axes_still_split` is a useful positive boundary check and passed under the adversarial
  canonicalization implementation.
- The canonical reorder case rejects a length-only implementation.
- M-68 is not misrepresented as live; `TD-159` and emission-barrier status are factually
  represented in the milestone.

## Required resubmission

Architect must add the four missing RED/verify oracles above, keep the intended single-cause
COMPILE-RED state, and resubmit the committed artifact set for critic round 2. Do not dispatch
dev before this REJECT is cleared.

## Done Block

```text
C-220
exit=0
```

```text
2c709d72d95fff80db73a8eace87ff602d829eec
A	crates/gateway-serve/tests/red_fixed_bands_wire.rs
A	crates/gateway/tests/red_fixed_bands_canonical.rs
A	crates/gateway/tests/red_fixed_bands_fingerprint.rs
A	milestones/M-84-fixed-depth-bands.md
A	scripts/verify_M-84.sh
exit=0
```

```text
red_fixed_bands_canonical rc=101
6
error[E0425]: cannot find value `CANONICAL_DEPTH_BANDS` in crate `gateway`
red_fixed_bands_fingerprint rc=101
5
error[E0425]: cannot find value `CANONICAL_DEPTH_BANDS` in crate `gateway`
red_fixed_bands_wire rc=101
1
error[E0425]: cannot find value `CANONICAL_DEPTH_BANDS` in crate `gateway`
exit=0
```

```text
verify_rc=1
VERDICT: FAIL (12)
exit=0
```

```text
canonical rc=0
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
fingerprint rc=0
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
wire rc=0
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
length_only_mutation rc=101
test crate_rejects_same_values_in_other_order ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s
exit=0
```

```text
m68_ancestor_rc=0
diff_check_rc=0
exit=0
```
