<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: fe2b6b4ec673e2cbc7c12101e872accafbb5cd16
verdict: REJECT
-->

# C-168 — M-71 egress-cap rev7: REJECT

## Scope audited

- Subject: origin/feat/M-71-rev6 at fe2b6b4ec673e2cbc7c12101e872accafbb5cd16.
- Base: 3b496208a64edbf00a66b93986ff8529d0c93aa9.
- Handoff supplied no required stakes: high|normal; this is recorded as a handoff
  deficiency, not inferred.
- The artifact set is present: milestone, seven RED artifacts, and
  scripts/verify_M-71.sh. The range does not touch crates/contracts/**; this is
  a T-designate gateway/serve change, so no T1 contract-RFC is required.
- VB-I-2 (live == replay) and VB-I-11 (the meaning of history_truncated) are
  live FA invariants named and exercised by the set.

## REJECT — dev must not be dispatched

### R1 — the normative policy for an absent cap variable contradicts its RED suite

milestones/M-71-egress-cap.md:537 assigns task 4 as “absence/invalid value must
refuse production startup”; the D oracle repeats that at :594.

The committed RED artifacts specify the opposite for absence:

- crates/gateway-serve/tests/red_egress_cap_startup.rs:139-147 requires startup
  with the signed default when the variable is absent.
- crates/gateway-serve/tests/red_egress_cap_governed.rs:139-157 requires the
  same default through the complete env → serve-config → gateway chain.

Invalid and empty values can fail closed while an absent value selects the signed
default, but the milestone currently demands both outcomes for absence. No dev
implementation can satisfy the task table and these two controls at once.
Select one normative outcome, then align task 4, oracle D, and the RED controls.

### R2 — base CI has six unrelated sacred-test errors that remain after task 10

The rev7 baseline claims the new bridge is the sole cause of its Clippy RED.
It is not. cargo clippy --all-targets --all-features -- -D warnings also rejects
dead code in already-committed RED tests:

- crates/gateway-serve/tests/red_egress_cap_utf8.rs:53 (PROPOSED_CAP); :57
  (DENSE_TRADES); :191 (subscribed_snapshot); and :213 (type_of).
- crates/gateway/tests/red_egress_cap_paths.rs:123 (PumpRefused payload) and
  :124 (SnapshotRefused payload).

Those six diagnostics are independent of the missing
gateway::effective_max_response_bytes() bridge. Once task 10 supplies that
bridge, CI still cannot become green; engine-dev cannot repair sacred
*/tests/**. The architect must correct the RED artifacts and reshoot the
baseline until the intended unimplemented bridge is the only remaining
implementation-dependent red condition.

### R3 — the required cross-crate T2 bridge has no complete signature contract

Task 10 says only that gateway has “a setter”; the sole compile-red surface is
the getter gateway::effective_max_response_bytes() in
red_egress_cap_governed.rs:134-136. Neither the milestone nor a RED oracle
names the gateway-facing installation API that serve_config_from_env must call,
including its argument/result form and one-startup installation semantics.

This is a public T2 boundary between gateway-serve and gateway, not T1, but it
still must be a committed signature contract before dev. State and pin that
bridge surface; do not leave the transport-side call shape to implementation
choice.

## What passed

- scripts/verify_M-71.sh is syntactically valid, uses an explicit FAIL aggregate
  and returns non-zero on its expected RED baseline.
- The verifier includes the CI fmt / Clippy / test triplet, checks the committed
  contract range, and names all 11 tasks.
- The planned direct gateway enforcement preserves the relevant read-only
  boundary; no contracts/**, order-egress, or risk path is in scope.

## Re-review condition

Architect must commit a coherent absence policy, a fully specified/pinned T2
bridge surface, and a corrected RED suite whose post-bridge CI state can become
green without dev editing sacred tests. Then provide the new pushed head and
baseline output for another plan-time audit.

## Done Block

    $ git rev-parse HEAD
    fe2b6b4ec673e2cbc7c12101e872accafbb5cd16

    $ git merge-base fe2b6b4 origin/main
    3b496208a64edbf00a66b93986ff8529d0c93aa9

    $ bash -n scripts/verify_M-71.sh
    exit=0

    $ cargo clippy -p gateway --test red_egress_cap_paths -- -D warnings
    error: field 0 is never read
       --> crates/gateway/tests/red_egress_cap_paths.rs:123:17
    error: field 0 is never read
       --> crates/gateway/tests/red_egress_cap_paths.rs:124:21
    error: could not compile gateway (test red_egress_cap_paths) due to 2 previous errors
    exit=101

    $ bash scripts/verify_M-71.sh
    FAIL: cargo clippy --all-targets --all-features -- -D warnings
      error[E0425]: cannot find function effective_max_response_bytes in crate gateway
      error: constant PROPOSED_CAP is never used
      error: constant DENSE_TRADES is never used
      error: function subscribed_snapshot is never used
      error: function type_of is never used
      error: field 0 is never read (PumpRefused)
      error: field 0 is never read (SnapshotRefused)
    FAIL: cargo test --all --quiet
    FAIL: cargo test -p gateway --test red_egress_cap_paths --quiet
    FAIL: cargo test -p gateway-serve --test red_egress_cap_utf8 --quiet
    FAIL: cargo test -p gateway-serve --test red_egress_cap_governed --quiet
    PASS: bash scripts/tests/red_egress_doors.sh
    PASS: C база зелена, мутация роняет набор, честная нагрузка (E) цела
    PASS: F crates/contracts не тронут
    PASS: G GATEWAY_BANDS не тронут (граница C, предмет M-70)
    PASS: H book/venue/journal не тронуты диапазоном
    VERDICT: FAIL (5)
    exit=1
