<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: 7e471beb707c7b88301f3854adfcf2990ddb9dd7
verdict: REJECT
-->

# C-158 — M-71 egress cap rev2: REJECT

## Scope audited

Committed chain `3b496208a64edbf00a66b93986ff8529d0c93aa9..7e471beb707c7b88301f3854adfcf2990ddb9dd7` contains rev2 of the milestone, library RED suite, separate boundary RED suite, startup RED suite, verifier, and C-157. This is non-T1: no new contract type or trait signature is required, and `crates/contracts/**` is untouched. The changed code-facing artifacts are confined to the allowed `gateway` and `gateway-serve` test surfaces; `GATEWAY_BANDS` is unchanged.

`VB-I-2` remains material: live and replay must represent the same gateway result. A byte cap proved only for a bare replay `Snapshot`, but not for the actual `ServeMsg::Frame` / live-push message, does not cap the observable egress resource.

## Verdict: REJECT

R4 is repaired at plan time and the boundary test is genuinely separate COMPILE-RED. R1 and R3 remain blocking: the red suite does not measure the complete serialized wire response on every response shape, and its new honest-multiband control still permits a count-proxy rejection of a valid, cheap request. Do not dispatch dev from this artifact set.

### R1 — “bytes of full response” still means bare `Snapshot`, not emitted wire bytes

The helper in `red_egress_cap.rs` and the boundary test use `serde_json::to_vec(&Snapshot)`. Actual egress is `gateway-serve::wire::ServeMsg::{Snapshot,Frame}`:

- setup serializes `serde_json::to_vec(&ServeMsg::Snapshot(...))`;
- the push loop serializes each `ServeMsg::Frame`;
- public adapters `serve::snapshot_msg` and `serve::frames_msgs` construct those envelopes.

This is observable, not a naming objection. A disposable executable probe on the audited HEAD built 25,000 valid distinct trades under the normal selector and printed:

```text
bare_snapshot=2804765 wire_snapshot=2804778 wire_frame=2804666 live_wire_frame=2804666 cap=2000000
```

The wire envelope is already 13 B outside the quantity the RED tests call “full response.” More importantly, A-2 covers this dense resource only through `gateway::snapshot`. The same resource reaches the omitted egress shapes — `gateway::frames_since` and actual `LiveReducer::resume → pump` — at **2,804,666 B**, 1.402× the proposed 2,000,000-B cap. A fix can reject A-2's snapshot and the wide-book C calls while leaving dense replay/live frames uncapped; the committed set remains green for that mutant.

R2's independent grep finds no seventh *gateway-library* response builder beyond the five free functions plus `LiveReducer`, and its live chain now appears in C. But the complete response-builder list also has the two public `gateway-serve::serve` wrappers above; C does not invoke their `ServeMsg` serialization. Since that omitted wrapping changes the measured resource, R2 is not closed for the stated full-wire-response object.

Repair the existing byte oracle — not with a new normative unit — to measure the exact serialized `ServeMsg` sent by transport, for snapshots and frames. Run the dense-trade scenario through every library/serve/live egress form, enumerate and execute that list in C, and make the boundary use the same object.

### R3 — E-3 catches “more than one band,” but not valid eight-band service

`CT-RFC-09` §2.7 accepts a sorted duplicate-free sequence with `0 < b < 1`; it specifies no maximum count. On audited HEAD this selector is served:

```text
[0.0001, 0.0002, 0.0004, 0.0008, 0.0016, 0.0032, 0.0064, 0.0128]
```

The independent deep-book run measured only `152588` wire bytes. In a disposable mutation I inserted a count proxy in `gateway::validate_selector` that returns `InvalidInput` for `sel.bands.len() > 7`. The committed E, E-2, and E-3 controls all passed (`3 passed; 0 failed`), while the eight-band probe failed with `InvalidInput: "critic: too many bands"`. Thus the fixed seven-band vector does not distinguish a resource cap from a `bands.len() <= 7` proxy. Add the eight-band control, or an equivalent cardinality-independent cheap valid request.

### R4 — repaired for the present plan-time state

I inserted the exact anchor into a deliberately uncalled `#[allow(dead_code)]` function in a disposable copy. C printed only:

```text
FAIL: C НЕ ГОТОВ — набор КРАСЕН и без мутации, судить нейтрализацию предела не по чему.
```

`rg -c '^PASS: C '` returned no match. This closes the round-1 false PASS: C no longer treats pre-existing RED as mutation evidence. The positive half cannot be executed against an honest implementation because none exists; after implementation C must retain `base green`, `mutated all_red=1`, `mutated E green`, extended to the repaired R1/R3 subject set.

### Boundary — topology correct, resource incorrect

`red_egress_cap_boundary.rs` is genuinely COMPILE-RED: `gateway::DEFAULT_MAX_RESPONSE_BYTES` does not exist (`E0425`). Its exponential then binary search maintains `lo accepted / lo+1 rejected`, therefore narrows its input to one trade. But it serializes bare `Snapshot`, never a `ServeMsg::Frame`; it proves a one-entity boundary for the wrong resource and cannot establish the advertised full-wire byte boundary.

## Assessment of founder-owned 2 MB

`2,000,000 B` is reasonable as a provisional operating magnitude; it is not approved here.

- Executed normal deep-book default: **12,500 B**, hence **160×** headroom.
- Executed valid eight-band deep-book request: **152,588 B**, hence **13.1×** headroom.
- Executed dense-trade frame: **2,804,666 B**, hence refusal at **1.402×** cap — a high-cost response the cap should control.
- With 16 subscriptions, the nominal cap budget is **32,000,000 B** per connection; the currently served wide fixture is 7,841,085 B per subscription (about 125 MB for 16).

The number is neither obviously too small for demonstrated normal work nor too large to constrain the reproduced harmful load. Its safety claim is conditional on R1: 2 MB must bound actual emitted `ServeMsg` bytes, including frame and envelope, not a proxy serialization.

## Checks that passed

- Artifact set is complete for this non-T1 scope; contracts and `GATEWAY_BANDS` are untouched.
- Original defect remains reproducible: `[0.99]` serves 59,980 heatmap cells versus 100, while A-2 serves 2,804,765 bare bytes with empty heatmap.
- The declared verifier baseline is accurate: `FAIL (7)`, exit 1.
- Design-claims merge preview passes.
- R4's dead anchor no longer produces C PASS before a green base exists.

## Done Block

```text
$ bash scripts/reserve_artifact_id.sh C
C-158
exit=0

$ git diff --name-status 3b496208a64edbf00a66b93986ff8529d0c93aa9..7e471beb707c7b88301f3854adfcf2990ddb9dd7
A	crates/gateway-serve/tests/red_egress_cap_startup.rs
A	crates/gateway/tests/red_egress_cap.rs
A	crates/gateway/tests/red_egress_cap_boundary.rs
A	milestones/M-71-egress-cap.md
A	research/critiques/C-157-M-71-egress-cap.md
A	scripts/verify_M-71.sh
exit=0

$ cargo test -p gateway --test red_egress_cap -- --nocapture
running 8 tests
test pl_i_5_e2_degenerate_books_are_served ... ok
test pl_i_5_e3_honest_multi_band_request_is_served ... ok
test pl_i_5_e_prod_default_selector_is_served ... ok
test result: FAILED. 3 passed; 5 failed
PL-I-5 НАРУШЕН: bands=[0.99] обслужен — 59980 ячеек heatmap против 100 у прод-дефолта (×600)
PL-I-5 A-2 НАРУШЕН: ответ 2804765 Б обслужен при ПУСТОМ heatmap
exit=101

$ cargo test -p gateway --test red_egress_cap_boundary -- --nocapture
error[E0425]: cannot find value `DEFAULT_MAX_RESPONSE_BYTES` in crate `gateway`
exit=101

$ cargo test -p gateway-serve --test critic_m71_wire_gaps -- --nocapture
running 2 tests
eight_band_wire_snapshot=152588
test eight_valid_narrow_bands_are_currently_served ... ok
bare_snapshot=2804765 wire_snapshot=2804778 wire_frame=2804666 live_wire_frame=2804666 cap=2000000
test dense_frame_and_wire_envelope_are_real_uncovered_egress ... ok
test result: ok. 2 passed; 0 failed
exit=0

$ [disposable count-proxy mutation] cargo test -p gateway --test red_egress_cap pl_i_5_e -- --nocapture
running 3 tests
test pl_i_5_e2_degenerate_books_are_served ... ok
test pl_i_5_e3_honest_multi_band_request_is_served ... ok
test pl_i_5_e_prod_default_selector_is_served ... ok
test result: ok. 3 passed; 0 failed
exit=0

$ [same mutation] cargo test -p gateway-serve --test critic_m71_wire_gaps eight_valid_narrow_bands_are_currently_served -- --nocapture
eight sorted 0<b<1 bands are currently a valid served request: Custom { kind: InvalidInput, error: "critic: too many bands" }
test eight_valid_narrow_bands_are_currently_served ... FAILED
exit=101

$ [disposable dead anchor] bash scripts/verify_M-71.sh | grep -E '^(PASS|FAIL): C|^VERDICT:'
FAIL: C НЕ ГОТОВ — набор КРАСЕН и без мутации, судить нейтрализацию предела не по чему.
VERDICT: FAIL (7)
exit=0

$ bash scripts/verify_M-71.sh | grep -E '^(PASS|FAIL|VERDICT)'; echo "exit=${PIPESTATUS[0]}"
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: cargo test -p gateway --test red_egress_cap --quiet
FAIL: cargo test -p gateway --test red_egress_cap_boundary --quiet
PASS: A состав набора — 8 оракулов (ожидалось ровно 8: A A-2 B C F E E-2 E-3)
FAIL: cargo test -p gateway-serve --test red_egress_cap_startup --quiet
PASS: B состав набора — 10 оракулов (ожидалось ровно 10: 8 отказов + 2 vantage)
FAIL: C НЕ ГОТОВ — набор КРАСЕН и без мутации, судить нейтрализацию предела не по чему.
FAIL: D GATEWAY_MAX_RESPONSE_BYTES объявлен в docker-compose.yml
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
PASS: cargo test -p gateway-serve --test red_max_subs_config --quiet
PASS: cargo test -p gateway-serve --test red_window_guard_startup --quiet
PASS: F crates/contracts не тронут
PASS: G GATEWAY_BANDS не тронут (граница C, предмет M-70)
PASS: H book/venue/journal не тронуты диапазоном
VERDICT: FAIL (7)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --check 3b496208a64edbf00a66b93986ff8529d0c93aa9..7e471beb707c7b88301f3854adfcf2990ddb9dd7
exit=0

$ git ls-remote origin refs/reserved/C-158
2217aebf4b859a5cf6352ff63d1aee3b716e5d9b	refs/reserved/C-158
exit=0
```

## Handoff

REJECT → architect. Amend only M-71 plan artifacts: use exact outbound `ServeMsg` bytes in the existing byte-cap oracle and boundary, cover dense snapshot/replay/frame/live output through every enumerated response entry point, and add an inexpensive valid eight-band anti-false-red request. Keep the current C baseline condition and rerun the full verifier. Commit the amended artifact set on `feat/M-71-egress-cap`, then request a fresh critic audit. The 2 MB value remains founder-owned.
