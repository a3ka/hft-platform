<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: 5b017f960be876462f1547bacce953c16ed64dcb
verdict: REJECT
-->

# C-157 — M-71 egress cap: REJECT

## Scope audited

Committed artifact chain `3b496208a64edbf00a66b93986ff8529d0c93aa9..5b017f960be876462f1547bacce953c16ed64dcb` contains exactly the milestone, library RED suite, startup RED suite, and acceptance script. No T1 contract or trait signature is introduced; `crates/contracts/**` is untouched. The relevant live invariant is `PL-I-5` (and `PL-I-4`) in `docs/DESIGN.md` §22. `docs/fa/viz-backend.md` confirms that the gateway is read-only and that `VB-I-2` requires live/replay parity.

## Verdict: REJECT

The present RED set does reproduce the reachable hole, but it does not yet measure the complete response resource, does not protect all response-construction paths, misses a legitimate multi-band request, and lets its mutation step report a false green. Dev must not be dispatched until the oracle set is complete.

### R1 — resource oracle measures `heatmap.len()`, not the built response

`A`, `B`, and `F` in `crates/gateway/tests/red_egress_cap.rs` judge only `series.heatmap.len()`. `SeriesBundle` also serializes `depth_series`, `volume_profile` (including `bins`), `volume_bubbles`, OHLCV, CVD and VWAP. Therefore a heatmap-only cap can pass the proposed suite while the wire response is still unbounded.

Execution in a disposable copy of the audited tree constructed 25,000 distinct valid trades under the default selector. It was served with `heatmap.len()==0`, while both `volume_profile[0].bins` and `volume_bubbles` exceeded 20,000. That is a response-resource bypass, not a width-proxy concern.

Define the actual bounded quantity and make it cover every serialized response component (encoded bytes, or a documented total of all emitted entities). Add a RED oracle for a dense non-heatmap response and boundary cases at limit / limit+1. A cap on only heatmap cells is insufficient for the stated Objective.

### R2 — oracle C does not cover every response-construction entry point

Oracle C calls only `gateway::frames_since` and `gateway::snapshot_from_checkpoint`; A indirectly calls `gateway::snapshot`. It does not call `gateway::frames_since_with_stats`, `gateway::replay`, or the actual gateway-serve path `LiveReducer::resume` → `LiveReducer::pump` → `LiveReducer::snapshot`.

The latter is not theoretical: `crates/gateway-serve/src/lib.rs` uses it for legacy setup and v1 `subscribe`/switch, then emits its snapshot and frames. A disposable execution against the audited code showed this live builder accepts `bands=[0.99]`, pumps it, and produces more than 50,000 heatmap cells. C contains no `LiveReducer` call, so an implementation that caps the three tested library paths but leaves the production live builder open can turn the suite green.

Extend the anti-bypass RED oracle to every public response builder that accepts a `Selector`, including the live WS path; include replay and the stats-returning frame builder unless their API is intentionally made incapable of client egress and that fact is mechanically pinned. The mutation gate must exercise the same complete set.

### R3 — E/E-2 do not prove honest multi-band service

The milestone requires a working range and explicitly names one band versus seven at equal total volume. The committed suite calls its normal non-degenerate selector only with one band (`[0.001]`); E-2 covers only empty `[0.99]` and one-sided `[0.001]` books. It has no exact-cap boundary and no seven-band control.

Execution in a disposable copy showed that the valid sorted selector `[0.0002, 0.0004, 0.0008, 0.0016, 0.0032, 0.0064, 0.0128]` is served today and builds fewer than 20,000 heatmap cells. A faulty implementation that rejects every non-empty response with more than one band can keep A/B/C/E/E-2/F green yet reject this honest request. Add this control (or an equivalent documented working range) and the specified same-resource one-band/seven-band comparison.

### R4 — C can print PASS for an anchor unrelated to the response limit

The requested manual anchor check does not establish the claimed two-sided connection. In a disposable copy I inserted the exact `MUT-ANCHOR M-71-LIMIT` plus a private `enforce_response_limit(cells, limit)` function, but deliberately made no response builder call it. The verifier's C step then printed `PASS: C набор КРАСЕН без предела, а анти-ложное-КРАСНОЕ E остаётся ЗЕЛЁНЫМ`; every E neighbour was green. The four subject RED tests were already red before and after the mutation, so C merely observed their pre-existing redness, not a limit being neutralized. Clippy independently identified the inserted function as unused.

Do not allow C to report PASS unless its target is mechanically tied to the complete-resource enforcement on each protected builder, and prove a suitable unmutated baseline before judging the mutation. At plan time C should remain an explicit not-ready condition; after implementation it must fail if an unused syntactic anchor can satisfy it.

## Assessment of the proposed 20,000-cell value

`20,000` is a reasonable provisional founder value, not a reject reason by itself. It is 11.8× the reported production `heatmap len=1,697`; the M-71 full-window estimate implies roughly 120 bytes per heatmap cell, hence about 2.4 MB per capped response and about 38 MB at the existing 16-subscription cap. It rejects the reproduced 359,880-cell / 43.2-MB request decisively.

That assessment is conditional on R1: the value is meaningful only after "cell" denotes the complete resource being capped and the normal selector range is proved below it. It must not be silently reinterpreted as a heatmap-only limit.

## Checks that passed

- Artifact set is complete for this non-T1 milestone; no contract-RFC is required.
- The stated hole is real: the committed RED suite served `bands=[0.99]` as 59,980 heatmap cells versus 100 for the fixture's default (×600).
- `scripts/verify_M-71.sh` reproduces its declared plan-time baseline: `VERDICT: FAIL (5)`, with the five stated causes.
- `bash scripts/verify_design_claims.sh --merge-preview origin/main` passed.
- Scope is confined to the architect's milestone/test/verify artifacts; neither `contracts/**` nor `GATEWAY_BANDS` changed.

## Done Block

```text
$ git diff --name-status 3b496208a64edbf00a66b93986ff8529d0c93aa9..5b017f960be876462f1547bacce953c16ed64dcb
A	crates/gateway-serve/tests/red_egress_cap_startup.rs
A	crates/gateway/tests/red_egress_cap.rs
A	milestones/M-71-egress-cap.md
A	scripts/verify_M-71.sh
exit=0

$ cargo test -p gateway --test red_egress_cap -- --nocapture
running 6 tests
test pl_i_5_e2_degenerate_books_are_served ... ok
test pl_i_5_e_prod_default_selector_is_served ... ok
PL-I-5 НАРУШЕН: bands=[0.99] обслужен — 59980 ячеек heatmap против 100 у прод-дефолта (×600)
test result: FAILED. 2 passed; 4 failed
exit=101

$ cargo test -p gateway-serve --test red_egress_cap_startup -- --nocapture
running 10 tests
test absent_limit_starts_with_default ... ok
test valid_limits_start ... ok
test result: FAILED. 2 passed; 8 failed
exit=101

$ bash scripts/verify_M-71.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: cargo test -p gateway --test red_egress_cap --quiet
FAIL: cargo test -p gateway-serve --test red_egress_cap_startup --quiet
FAIL: C SETUP НЕ СОСТОЯЛСЯ — якоря мутации 'MUT-ANCHOR M-71-LIMIT' в реализации НЕТ.
FAIL: D GATEWAY_MAX_RESPONSE_CELLS объявлен в docker-compose.yml
VERDICT: FAIL (5)
exit=1

$ [disposable copy: insert only the exact MUT-ANCHOR + unused enforce_response_limit; then] bash scripts/verify_M-71.sh
FAIL: cargo clippy --all-targets --all-features -- -D warnings
PASS: C набор КРАСЕН без предела, а анти-ложное-КРАСНОЕ E остаётся ЗЕЛЁНЫМ
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
PASS: cargo test -p gateway-serve --test red_max_subs_config --quiet
PASS: cargo test -p gateway-serve --test red_window_guard_startup --quiet
VERDICT: FAIL (5)
exit=1

$ cargo test -p gateway --test red_egress_cap critic_adversarial_seven_valid_bands_are_served_below_proposed_cap -- --nocapture
running 1 test
test critic_adversarial_seven_valid_bands_are_served_below_proposed_cap ... ok
exit=0

$ cargo test -p gateway --test red_egress_cap critic_actual_ws_live_reducer_path_serves_abusive_selector_today -- --nocapture
running 1 test
test critic_actual_ws_live_reducer_path_serves_abusive_selector_today ... ok
exit=0

$ cargo test -p gateway --test red_egress_cap critic_other_series_can_exceed_cell_cap_with_empty_heatmap_today -- --nocapture
running 1 test
test critic_other_series_can_exceed_cell_cap_with_empty_heatmap_today ... ok
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --check 3b496208a64edbf00a66b93986ff8529d0c93aa9..5b017f960be876462f1547bacce953c16ed64dcb
exit=0

$ git ls-remote origin refs/reserved/C-157
3a4d29ad357fd6cba6143133dd8652678f614791	refs/reserved/C-157
exit=0
```

## Handoff

REJECT → architect. Add RED coverage for the complete serialized response resource, every Selector-bearing response builder (especially `LiveReducer`), and the valid multi-band/boundary controls. Repair C so an unused syntactic anchor cannot produce its two-sided PASS. Commit the amended artifact set on `feat/M-71-egress-cap`; then request a fresh critic round. The proposed number remains founder-owned.
