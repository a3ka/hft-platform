# C-148 — PR #96: FA viz-backend truth gate

<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: 721461cb7d41a625b28026f73d1aa5a3e62091cd
audited_head: 8e19283c0c3f23254c17470651f6d5b16634f67f
verdict: REJECT
-->

## Verdict: REJECT

The PR correctly removes several stale references and the merge-tree design
claim gate is green, but three newly added factual claims are false or
overbroad. This is an FA for a carrying layer; it must not merge while it
asserts implementation or provenance that the code does not provide.

## Blocking findings

### B-1 — §2 marks unimplemented Volume Profile variants as implemented

`docs/fa/viz-backend.md:39` marks the row
`SVP/CVP/FRVP/Anchored/Composite: POC/VAH/VAL/HVN/LVN` as `✅ есть (M-24
DONE)`. M-24's committed objective is explicitly **SVP only**; it explicitly
excludes cumulative, fixed-range, anchored, composite, and HVN/LVN for later
work. The current implementation's relevant identifiers are likewise only
`VolumeProfileRow` and session-oriented `volume_profile`/`utc_session_id`.

Condition to clear: narrow the implemented status to the actually implemented
SVP surface, or individually evidence every named variant before calling the
whole row implemented.

### B-2 — §4 claims a provenance vocabulary that deep heatmap data does not use

`docs/fa/viz-backend.md:89-94` says every book series deeper than 1.3% has,
after P-014, either `diff-reconstructed, liveness=…` or `not-observed …`.
The actual heatmap/COB builder (`crates/gateway/src/lib.rs:1212,1228,1244`)
still emits the third literal `diff-reconstructed`. This is also the live
TD-161 discrepancy. The changed sentence is therefore false for a deep
book-derived series, and tells a consumer a stronger provenance vocabulary
than it receives.

Condition to clear: scope the statement to `DepthRow`/depth-series, or make
all described book-derived output use the stated vocabulary and audit that
separately.

### B-3 — §5 does not declare the GS-I family as the new callout states

The new callout (`docs/fa/viz-backend.md:190-192`) says `GS-I-*` are declared
in §5 of this FA and names `GS-I-1` as the canary. §5 contains only an
incidental `GS-I-4` reference inside VB-I-6, not a GS-I family declaration or
GS-I-1. M-65's committed close-out records the same fact: the FA has exactly
`GS-I-4` and calls the claimed `GS-I-3` nonexistent. Code and
`scripts/verify_M-28.sh` do contain GS-I-labelled checks, but that does not
make the stated FA declaration true.

Condition to clear: describe GS-I as code/test labels without claiming an FA
declaration, or route a genuine FA home through its required separate
form-changing gate. The latter is not chosen by this PR.

## Required checks and scope

- Audit range is exactly `721461c..8e19283`; it changes only
  `docs/fa/viz-backend.md` (+64/-22). No T1 contract, trait signature, RED
  suite, verify script, or milestone artifact is in this document-only §9
  range.
- No deleted factual item was found to be lost: the superseded status,
  postcard, M-31, line-number, and old-precondition statements are either
  corrected or retained as dated history. The blocking defects are additions,
  not deletion fallout.
- P-014 is correctly reflected on the points this verdict could verify:
  P-014 p.1's side/reach label exists; p.2 is still TD-158; p.4 is still
  blocked by TD-159. The live VPS container was checked and has
  `GATEWAY_BANDS=0.001`.
- A-002 Z-2 remains suspended. The revised FA does not change the record set,
  decide the GW-I home, or introduce a new invariant; those boundaries are
  respected. The false claim that GS-I is already declared must nevertheless
  be removed.

## §10 audit-axis disposition

| Audit axis left open by the author | Relevance to this revision | Result here |
|---|---|---|
| VPS `.env` / live `GATEWAY_BANDS` | Critical to the new claim about production non-inclusion | Closed by read-only VPS inspection: `GATEWAY_BANDS=0.001`. |
| `red_volume_profile.rs` read only by headings | Critical: the table newly changes implementation status | The committed M-24 scope, plus current code, show only SVP; B-1. |
| `research/exports/format.md` field-level parity | Not needed to establish either false assertion; no export form is changed by this PR | Remains an audit limit, not an independent blocker. |
| `gateway-serve` read only in fragments | Relevant to the new JSON-wire statement | All server output sends found in the audited source are `Message::Text` built from JSON; no contradiction found. |

## Done Block

```text
$ bash scripts/reserve_artifact_id.sh C
[stdout was empty in this harness; exit=0]

$ bash scripts/reserve_artifact_id.sh --list C | tail -4
C-142      0 дн  reserve C-142 nous 2026-08-25T10:22:28Z ...
C-143      0 дн  reserve C-143 nous 2026-08-25T10:23:33Z ...
C-144      0 дн  reserve C-144 nous 2026-08-25T10:24:21Z ...
C-148      0 дн  reserve C-148 nous 2026-08-25T11:44:00Z ...
reserve: резервов: 12
exit=0

$ git diff --name-only 721461c 8e19283
docs/fa/viz-backend.md
exit=0

$ git diff --check 721461c 8e19283
exit=0

$ git show refs/pull/96/merge --format='%H%n%P%n%s' -s
6b59e7fa8a81a082d97b1984e765843693398366
721461cb7d41a625b28026f73d1aa5a3e62091cd 8e19283c0c3f23254c17470651f6d5b16634f67f
Merge 8e19283c0c3f23254c17470651f6d5b16634f67f into 721461cb7d41a625b28026f73d1aa5a3e62091cd
exit=0

$ (on detached refs/pull/96/merge) bash scripts/verify_design_claims.sh
PASS  [2-ПОКРЫТИЕ] §22: GW-I — заявлено=0, в оракулах=13 — подтверждено замером (loose=13)
PASS  [7-RFC-PATH] путей-кандидатов ... всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ ssh ... 'docker inspect hft-gateway-serve ... | grep ^GATEWAY_BANDS='
GATEWAY_BANDS=0.001
exit=0

$ git show HEAD:docs/archive/M-24-volume-profile.md | sed -n '1,28p'
STATUS: **DONE — merged to main 2026-07-23** ...
**В scope M-24 (SVP):** ...
**НЕ в scope (аддитивно позже):** CVP (cumulative), FRVP (fixed-range), Anchored/Composite, HVN/LVN ...
exit=0

$ git grep -niE 'anchored|composite|fixed.range|cumulative volume|hvn|lvn|cvp|frvp' HEAD -- crates/gateway crates/research-cli
[no implementation matches; exit=1]

$ git grep -nE 'depth_band_provenance|diff-reconstructed' HEAD -- crates/gateway/src/lib.rs
...:1212:        let prov_str = "diff-reconstructed".to_string();
...:1228:                depth_band_provenance: deep.then(|| prov_str.clone()),
...:1244:                depth_band_provenance: deep.then(|| prov_str.clone()),
...:1382:        Some(format!("diff-reconstructed, liveness={}", liveness))
exit=0

$ sed -n '1,30p' milestones/M-65-ws-session.md
... `FA-WAIVER` ... несуществующий `GS-I-3` (в `docs/fa/viz-backend.md` есть ровно `GS-I-4`).
exit=0
```
