# C-149 — PR #96: повторный FA truth gate (B-1..B-3)

<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: 68db5e6a1d54afb8b0c74f99266eec8a4a2f587b
audited_head: b8432c073cea9aa7edd38efc94d0168d83ddd988
verdict: REJECT
-->

## Verdict: REJECT

`b8432c0` correctly closes B-1 and B-3, but its B-2 replacement introduces a
new false claim in the carrying FA.  The depth-series and heatmap have two
runtime provenance producers; COB has none.  The merge gate cannot accept a
sentence that assigns COB a label and calls the observable vocabulary "two
literals" when `DepthRow` alone has confirmed, unconfirmed, and
`not-observed` outcomes.

## Findings

### B-1 — PASS: implementation status is now accurately scoped to SVP

The changed row states only per-session SVP with POC/VAH/VAL/VA%.  M-24 calls
that exact surface in scope and explicitly places CVP, FRVP,
Anchored/Composite, and HVN/LVN out of scope.  A source scan of `gateway` and
`research-cli` found no implementation of those excluded variants.  Nothing
implemented was lost by this narrowing.

### B-2 — REJECT: `heatmap/COB` and "two literals" are both false

The new text in `docs/fa/viz-backend.md:95-101` says the heatmap/**COB** label
is attached only by width and declares that there are two literals.  The code
has a `depth_band_provenance` field on `HeatmapCell` and attaches
`"diff-reconstructed"` only to deep heatmap cells (`lib.rs:1212,1228,1244`).
`CobLevel` contains only `side`, `price_e8`, and `size_e8`: it has no
provenance field and the COB construction supplies no label.

`DepthRow` is the other producer, via `depth_provenance_label` (`:1106`,
`:1345-1382`).  For deep rows it can produce all of:

- `diff-reconstructed, liveness=confirmed`;
- `diff-reconstructed, liveness=unconfirmed`; and
- `not-observed band=… reach=…`.

Together with the heatmap's plain `diff-reconstructed`, this is not a two
literal vocabulary under any useful consumer-facing reading.  It is also
contradicted by the preceding unchanged paragraph, which names
`not-observed`.

The requested exhaustive boundary check found **no third runtime producer**:
neither `research-cli` nor export-v1's `research/exports/format.md` contains
this provenance field or emits these strings.  Thus the accurate statement is
two producers (`DepthRow`, `HeatmapCell`), not a common heatmap/COB path.

`П-014`'s condition (a) concerns the depth-series label.  Calling it "not
closed for heatmap" does not report an open condition of `П-014`; it extends
that precondition to a separate M-23 output without a signed basis.  It must
not be stated as a P-014 fact.

**Condition to clear:** replace B-2 with a factual, non-normative statement:
`DepthRow` has the side/reach-aware three-outcome provenance above;
`HeatmapCell` has only width-gated `diff-reconstructed`; `CobLevel` has no
provenance field.  Do not call those values two literals, do not attribute a
label to COB, and do not create a heatmap sub-condition of П-014.

### B-3 — PASS: GS-I location and the M-28 split are accurately stated

M-28's invariant table maps `GS-I-1` to `VB-I-9a` and `GS-I-2` to
`VB-I-9b`.  This FA declares no `GS-I-*` table row (the only occurrence is an
incidental `GS-I-4` mention inside VB-I-6).  Saying the M-28 milestone table
is the present home of those labels describes the committed state; it does
not decide a new FA home or alter the output contract.  The unchanged
`verify_M-28.sh` canary remains outside CI, as the paragraph says.

## Scope and new-claim disposition

The audited one-commit range changes only `docs/fa/viz-backend.md`
(+17/-5).  B-1 and B-3 are supported by the implementation and milestone.
Apart from B-2, I found no new false implementation claim in the 17 added
lines.  No record-set change, GW-I-home decision, or new invariant is made by
the corrected B-1/B-3 text.  `verify_design_claims.sh --merge-preview
origin/main` passes, but it does not validate this semantic provenance claim.

## Done Block

```text
$ bash scripts/reserve_artifact_id.sh C
[stdout was empty in this harness; exit=0]

$ bash scripts/reserve_artifact_id.sh --list C | tail -4
C-143      0 дн  reserve C-143 nous 2026-08-25T10:23:33Z ...
C-144      0 дн  reserve C-144 nous 2026-08-25T10:24:21Z ...
C-148      0 дн  reserve C-148 nous 2026-08-25T11:44:00Z ...
C-149      0 дн  reserve C-149 nous 2026-08-25T11:45:03Z ...
reserve: резервов: 13
exit=0

$ git diff --stat 68db5e6 b8432c0
 docs/fa/viz-backend.md | 22 +++++++++++++++++-----
 1 file changed, 17 insertions(+), 5 deletions(-)
exit=0

$ git diff --check 68db5e6 b8432c0
exit=0

$ sed -n '10,22p' docs/archive/M-24-volume-profile.md
**В scope M-24 (SVP):** ... POC + VAH/VAL + VA%.
**НЕ в scope (аддитивно позже):** CVP (cumulative), FRVP (fixed-range),
Anchored/Composite, HVN/LVN ...
exit=0

$ rg -ni 'CVP|FRVP|HVN|LVN|anchored|composite|fixed.range|fixed_range|cumulative.volume' crates/gateway crates/research-cli
[only unrelated session-anchored comments; no CVP/FRVP/HVN/LVN implementation]
exit=0

$ sed -n '49,53p' milestones/M-28-gateway-serve.md
| **GS-I-1** (VB-I-9a) | ... |
| **GS-I-2** (VB-I-9b) | ... |
| **GS-I-3** | ... |
| **GS-I-4** | ... |
| **GS-I-5** | ... |
exit=0

$ rg -n -C 4 'struct (HeatmapCell|CobLevel|DepthRow)|depth_band_provenance|prov_str|diff-reconstructed|not-observed' crates/gateway/src/lib.rs
210:pub struct HeatmapCell { ... depth_band_provenance: Option<String>, }
221:pub struct CobLevel { side, price_e8, size_e8 }
239:pub struct DepthRow { ... depth_band_provenance: Option<String>, }
1106:depth_band_provenance: depth_provenance_label(...)
1212:let prov_str = "diff-reconstructed".to_string();
1228:depth_band_provenance: deep.then(|| prov_str.clone())
1244:depth_band_provenance: deep.then(|| prov_str.clone())
1351:... bid ... `diff-reconstructed, liveness=confirmed`
1352:... ask ... `diff-reconstructed, liveness=unconfirmed`
1353:... `not-observed band=… reach=…`
exit=0

$ rg -n -i 'depth_band_provenance|diff-reconstructed|not-observed|liveness=' crates/research-cli research/exports
[no matches]
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: GW-I — заявлено=0, в оракулах=13 — подтверждено замером (loose=13)
PASS  [7-RFC-PATH] путей-кандидатов ... все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0
```
