<!-- GATE-META
milestone: M-100
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: 3c1d690200da778231f77ec79165d9a4ef033877
verdict: REJECT
-->

# C-303 — M-100 three serving contracts: REJECT

## Scope and subject

RAW-gate review of `origin/feat/M-100-three-serving-contracts`, not of an uncommitted plan.
The handoff SHA `3c1d6902` is the fetched branch tip and is its own ancestor.  The audited
range from `a660b3a7` contains the milestone, CT-RFC-09 rev3, five RED files, the acceptance
script and its CI-map probe.  This is T-designate gateway/WS work, not a `contracts/` T1 change;
the declared public shapes are present in the milestone/RFC and exercised by the RED imports and
wire test.

`VB-I-2`, `VB-I-4`, and `VB-I-12` from `docs/fa/viz-backend.md` are the live invariants used
for this review.

## Verdict: REJECT

Dev dispatch is blocked until the following RED/acceptance gaps are corrected and the artefacts
are recommitted for a new critic round.

### R1 — `h1` proves one frame boundary and omits the required boundary-invariance oracle

`h1` intentionally forces both L2 observations into one `LiveReducer::pump(..., usize::MAX)`
frame and asserts `frames.len() == 1` (`crates/gateway/tests/red_m100_heatmap_frame_close.rs:123-138`).
It correctly reproduces the current defect: on `a660b3a7`, the client retains bid `64990 × 5`
while full replay does not.  But SCALE plan §15.9 requires the *same events split into different
message/frame boundaries* to reach the same final state on the live path.  No M-100 test varies
the pump boundary; the exact-target search finds no such scenario.

This matters specifically for the planned repair: a change can make the one-frame case green
while making the result depend on whether the producer delivered the removal in the next frame.
That is another violation of `VB-I-2`/`VB-I-12`, not a cosmetic extra case.

Condition to clear: add a live-path oracle that drives the same journal sequence through at least
two frame partitions, applies the serialized frames as a client does, and compares each result
both with the other partition and with independent full replay.  Retain the current one-frame
case as the removal-within-a-frame forcing case.

### R2 — `sv1…sv3` do not cover the production admission branch for a pre-M-100 checkpoint

The current code has two independent version decisions.  `read_and_validate_inner` rejects
bytes `12..16` against `GATEWAY_SCHEMA_VERSION` (`crates/gateway/src/lib.rs:4866-4869`), and
the public serving gate independently does the same in `admission::readiness`
(`crates/gateway-serve/src/admission.rs:201-223`).  M-100 promises both must instead use
`CALC_STATE_VERSION = 11` while the wire becomes 12.

`sv2` only calls `LiveReducer::resume`; it therefore exercises the library decision.  Every
wire fixture first writes its checkpoint through `checkpoint::advance`, so after the intended
implementation it will contain the newly-written state version and does not demonstrate that
`readiness` accepts a legacy byte value 11 under wire 12.  A mutation of *only*
`admission::readiness` back to `GATEWAY_SCHEMA_VERSION` therefore leaves the asserted fixture
at its matching wire value and can pass `w1…w7`.

The mutation-table row in `milestones/M-100-three-serving-contracts.md` §8 consequently
overclaims that `sv2` and “all `w*`” pin this error.  It only holds if both independent branches
are simultaneously mutated, which is not a mutation test of either branch.

Condition to clear: add a real WS/admission scenario whose checkpoint header is explicitly
legacy state version 11 while the server emits schema 12, and separate negative scenarios for
foreign state versions 10 and 12.  The former must reach the warm snapshot; the latter must
reach the named not-ready path.  Update the mutation table so a one-branch mutation names the
specific oracle it makes red.

### R3 — the required merge-preview claim gate is red on the proposed artefact set

`bash scripts/verify_design_claims.sh --merge-preview origin/main` exits 1:

```
FAIL  [2-ПОКРЫТИЕ] §22: семейство VB-I — документ заявляет 'в оракулах'=9,
реальный замер (анти-плацебо) strict=10, loose (любое упоминание)=10
VERDICT: FAIL (1 нарушений)
```

This is introduced by this branch: the base has nine unique `VB-I-*` identifiers in crate tests;
the new `red_m100_heatmap_frame_close.rs` adds `VB-I-12`, making ten.  `docs/DESIGN.md:920`
still says `9`.  The acceptance script’s CI-parity pass includes this design-claim gate, so the
new `verify_M-100.sh` cannot become green merely by implementing tasks 1–5.

Condition to clear: reconcile the authoritative coverage claim and the milestone’s allowed-path
set, then show the merge-preview command green.  Do not suppress this check or relabel the new
oracle as an analogy; it is explicitly the `VB-I-12` oracle in the M-100 test.

## Findings that do not independently block this round

### N1 — calc-key golden vectors and checkpoint-name guard are sound

I recalculated the RFC §2.10.4 byte strings with `python3 hashlib.sha256`:

| vector | independently calculated digest | RED expectation |
|---|---|---|
| k1 production selector | `201e0f954c40013f39fdfc8c7db729c19fe026c85a44d79d665a1aca7653c081` | matches |
| k2 `window_ms=none`, `depth_cadence_ms=none` | `0d5eec7944b3d012b435aebc10c01bbd81b3b62bc9a30928482d20d68b98e4b2` | matches |
| k3 ETH selector | `75a8d817d372048e837375a5ddd98a69fdc1c4c83f81ea18ca49b864e451da0c` | matches |

`k5` correctly guards the still-current selector-fingerprint path:
`checkpoint::ckpt_path_for_pub` formats `ckpt-{fp:016x}.bin`
(`crates/gateway/src/lib.rs:4063-4071`), and the current selector fingerprint retains all six
axes (`crates/gateway/src/lib.rs:4294-4322`).  It is appropriately a no-rename guard for S1a,
not a premature S3 migration.

### N2 — the canonical-client compatibility exception needs an explicit predecessor resolution

CT-RFC-09 §2.10.1 and `w4` deliberately accept a client-supplied band list when it exactly
equals the current profile.  That is a defensible compatibility choice only if it consciously
supersedes the contrary M-84 arbitration behaviour: `A-033` recorded a mutation that accepted
canonical client bands and required `even_canonical_bands_from_client_rejected_at_entrypoint` to
turn red (`research/arbitration/A-033-M-84-fixed-bands.md:218-223`); its binding D-2 located the
composition guard at product inputs (`:355-366`).

The current RFC states the new behaviour but does not identify this as a supersession or state
the transition invariant that replaces A-033’s “client must not know the grid” rationale.  Make
that relationship explicit in the RFC/milestone before implementation.  If architect and critic
disagree on whether the old decision applies, the next route is the fresh-context arbiter under
`gates.md` §0, not a silent reinterpretation.

### N3 — map semantics were checked against the current code

RFC §2.10.6’s bucket formula, exact-price cells, close semantics, observed-empty distinction,
and `Replace` framing are consistent with the intended source areas.  The concrete current
contradiction is exactly the one `h1` exposes: `book_series_in` inserts cells from every
observation into a map keyed by `(time_s, side, price_e8)` (`crates/gateway/src/lib.rs:2110-2137`),
so it cannot express an empty later column.  R1 is therefore a coverage defect around an
identified real semantic failure, not a request to change the RFC’s meaning.

The named M-94 carry-over also has a stale nonblocking cross-reference: its §7 calls the future
view shape “CT-RFC-09 §2.7”, whereas rev3 defines it in §2.10.2.  The direct M-100 links point to
§2.10 and no missing-file link was found; correct the archival prose only under its applicable
close-out/document route.

## Artifact-set and gate checks

- Present and committed before review: `milestones/M-100-three-serving-contracts.md`, CT-RFC-09
  rev3, five M-100 RED test files, `scripts/verify_M-100.sh`, and
  `scripts/tests/red_verify_M-100_ci_map.sh`.
- The acceptance script is an explicit FAIL-count aggregator and its CI-map anti-placebo probe
  passes all ten scenarios.  It is expectedly red before implementation; its task REDs are
  not being treated as a completed implementation gate.
- T1 Block-C: no path under `crates/contracts/**` changed.  The new public shapes are T-designate
  gateway/WS forms; a contract-RFC is nevertheless present because the external wire form is
  changed.
- Authority: no founder boundary-C choice is changed.  `П-029`, `П-030`, and `П-032` remain the
  source for server-resolved computation, named `unsupported`, and profile identification.
- Link/claim recheck: merge-preview found no broken RFC file path or missing FACTS SHA, but it is
  red for the R3 coverage-count contradiction.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-100-three-serving-contracts
3c1d690200da778231f77ec79165d9a4ef033877
exit=0

$ git merge-base --is-ancestor 3c1d6902 origin/feat/M-100-three-serving-contracts
exit=0

$ bash scripts/next_artifact_id.sh C
C-297
exit=0

$ cargo test -p gateway --test red_m100_heatmap_frame_close -- --nocapture
test h1_level_removed_within_one_live_frame_disappears_for_client ... FAILED
left:  ... (1752000010, "bid", 6499000000000): 500000000
right: ... no (1752000010, "bid", 6499000000000)
test result: FAILED. 0 passed; 1 failed
exit=101

$ python3 hashlib check of k1/k2/k3
k1     201e0f954c40013f39fdfc8c7db729c19fe026c85a44d79d665a1aca7653c081
k2     0d5eec7944b3d012b435aebc10c01bbd81b3b62bc9a30928482d20d68b98e4b2
k3_eth 75a8d817d372048e837375a5ddd98a69fdc1c4c83f81ea18ca49b864e451da0c
exit=0

$ cargo test -p gateway --test red_m100_calc_key_id
error[E0432]: unresolved import `gateway::calc_key`
exit=101

$ cargo test -p gateway --test red_m100_state_version
error[E0425]: cannot find value `CALC_STATE_VERSION` in module `gateway::checkpoint`
exit=101

$ cargo test -p gateway --test red_m100_view_projection
error[E0432]: unresolved import `gateway::view`
exit=101

$ cargo test -p gateway-serve --test red_m100_three_contracts_wire
test result: FAILED. 0 passed; 7 failed
exit=101

$ bash scripts/tests/red_verify_M-100_ci_map.sh
VERDICT: PASS — 10 сценариев
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
FAIL  [2-ПОКРЫТИЕ] §22: семейство VB-I — документ заявляет 'в оракулах'=9, реальный замер strict=10, loose=10
VERDICT: FAIL (1 нарушений)
exit=1

$ git diff --check a660b3a7..3c1d6902
exit=0

$ git fsck --no-dangling --no-reflogs
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-10-10T22:00Z
- Milestone: M-100-three-serving-contracts
- Статус: BLOCKED — REJECT C-297
- HEAD: 3c1d6902 — docs(ROADMAP): строка S1a-K → M-100 [architect]

## §B — Что я сделал

- Audited the committed M-100 artefact set at the fetched subject-branch tip, including the
  RFC, all RED files, acceptance script and CI-map probe.
- Reproduced h1 on the stated base, independently recomputed all calc-key golden digests, and
  ran the merge-preview document-claim gate.
- Recorded the plan-time verdict as the committed audit trail; no product artefact was changed.

## §C — Артефакты / результаты

- `research/critiques/C-297-M-100-three-serving-contracts.md`
- Done Block: raw command output and exit codes above.

## §D — Следующий агент + инвокация

- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-100 C-297 REJECT. Work only in architect scope. Amend the committed M-100 artefacts before
  dev dispatch: (1) add a live-path frame-partition invariance oracle required by SCALE §15.9;
  (2) make sv/wire RED cover old checkpoint state=11 through admission with wire=12 and split
  the one-branch mutation claims; (3) reconcile the new VB-I-12 oracle with the authoritative
  DESIGN coverage count/allowed paths so verify_design_claims --merge-preview origin/main is
  green; (4) explicitly resolve the canonical-client acceptance exception against A-033.
  Re-run the relevant RED/verify commands, commit and push the amended artefact set to
  feat/M-100-three-serving-contracts, then request a fresh critic round. Do not implement
  production code.
  ```
- Push-статус: this verdict commit is pushed to `origin/feat/M-100-three-serving-contracts`.
- Кэш: `target/` created by this audit is removed after verification.

## §E — Риски / открытые вопросы

- The current `verify_M-100.sh` is intentionally RED before dev; R3 is separate because the
  merge-preview design-claim gate is structurally red even after task implementation.
- If the A-033 supersession is disputed after the architect amendment, use the arbiter path in
  `gates.md` §0 rather than a third interpretation.

=== END HANDOFF ===
