<!-- GATE-META
milestone: M-100
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: 183f0f90cb5c17bac7aa5c8fb1afab27051a994c
verdict: ESCALATE
-->

# C-308 — M-100 three serving contracts, round 3: ESCALATE

## Subject and complete artifact set

This is the RAW plan-time audit of the committed subject
`origin/feat/M-100-three-serving-contracts` at
`183f0f90cb5c17bac7aa5c8fb1afab27051a994c`. The handoff reference
`183f0f90` is that fetched tip; it is judged rather than the plan prose alone.
The audit base is `a660b3a7b78b116a0a98bc7c9d9eb1e03153725c`.

The architect set is complete before dev dispatch:

- `milestones/M-100-three-serving-contracts.md` supplies the objective, ownership,
  allowed and forbidden paths, concrete T-designate signatures, RED suite, acceptance,
  and task map.
- `docs/rfc/CT-RFC-09-ws-session.md` §2.10 defines the three serving interfaces and
  v12. It is T-designate gateway/WS surface, not a mutation under `crates/contracts/**`;
  therefore T1 Block-C and a contract-RFC change are not implicated.
- The five committed RED files cover h1/h2, k1–k5, sv1–sv3, p1–p4, and w1–w9. The
  proposed interfaces include `resolve_calc_key`, `ViewSpec::validate`, `project_view`,
  `canonical_calc_key_bytes`, and `calc_key_id`; no trait signature is left unstated.
- `scripts/verify_M-100.sh` is an explicit `FAIL` aggregator (`set -uo pipefail`), has
  a task check for every planned implementation task, uses exit status rather than text,
  and its CI map is itself covered by the ten-world probe.

The applicable live invariants are `VB-I-2` (live equals replay), `VB-I-4`
(additive wire form), and `VB-I-12` (Replace semantics for observed heatmap columns)
from `docs/fa/viz-backend.md`.

## Round-two R1: cleared

`fold_partition` now makes a fresh `pump` call only after appending one declared group.
It requires that call to return a frame, advances the delivery cursor exactly to that
group's final sequence, requires its last frame to cover that sequence, and applies every
frame through the wire/client path. The preamble is pumped before groups begin; later
groups do not exist in the journal when an earlier pump runs. Thus a group cannot be
silently absorbed by a neighbour. The targeted executable result matches M-100 §8:
only `[1 2 3]` diverges; `[1][2][3]`, `[1 2][3]`, and `[1][2 3]` equal replay.

R2, R3, and N2 remain cleared as recorded in C-305. This audit has no new fact reopening
any of them.

## New blocking finding — R4: w1 does not prove frame projection delivers the selected row

`w1_server_resolves_calc_key_and_view_selects_rows` promises that the selected depth row
is delivered in both snapshot and each live frame. It proves the snapshot, but its frame
check is vacuous:

```rust
let fr = &f["data"]["delta"]["depth_series"];
if let Some(a) = fr.as_array() {
    for r in a { /* every emitted row must be the selected one */ }
}
```

at `crates/gateway-serve/tests/red_m100_three_contracts_wire.rs:296-308`. A frame with an
absent/non-array `depth_series`, or an empty array, passes that block. The fixture appends
three bidirectional L2 snapshots after the subscription, so this is not a legitimate
empty-input case. A transport implementation that drops every selected depth row from its
frames can therefore pass w1 while still satisfying its snapshot assertions.

That contradicts the committed milestone §4.2 (`view` applies to every subscription frame)
and CT-RFC-09 §2.10.2 (each frame contains only selected `depth_series` rows). It violates
the non-vacuous/oracle-integrity requirement in `testing.md`: the RED test observes absence
of unselected rows but not presence of the selected row.

Condition to clear: the wire RED oracle must make the post-append frame’s depth-row payload
mandatory and prove that the requested `(15_000_000, bid)` row is delivered, while retaining
the existing rejection of unselected rows. It must fail if a frame omits or empties the
selected projection.

## Gates.md §9 recheck

- **(a) Code claims on the merge tree:** the specified h2 execution reproduces the exact
  four-partition classification in §8; `verify_design_claims.sh --merge-preview origin/main`
  passes. The expected RED failure is evidence of the pre-dev defect, not an acceptance pass.
- **(b) Authority:** the range stays within the milestone’s architect-owned RFC, RED,
  verify, DESIGN count, roadmap, and milestone paths. It changes neither T1 code nor a
  founder boundary-C decision (no data composition, money, phase, promotion, weights, or
  limits).
- **(c) Coherence:** merge-preview claims, checked RFC paths/SHA references, whitespace,
  and object integrity pass. The only unresolved issue is the R4 oracle gap above.

## Verdict: ESCALATE

R4 is a dev-blocking RED-oracle defect. This is also the third critic circle on M-100;
`gates.md` §0 requires a fresh-context arbiter after three circles on one subject, rather
than a further architect↔critic loop. The arbiter should decide the R4 condition before
dev dispatch. No founder decision is requested because the issue is engineering coverage,
not boundary C.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-100-three-serving-contracts
183f0f90cb5c17bac7aa5c8fb1afab27051a994c
exit=0

$ git merge-base --is-ancestor 183f0f90 origin/feat/M-100-three-serving-contracts
exit=0

$ cargo test -p gateway --test red_m100_heatmap_frame_close h2 -- --nocapture
H2_PARTITIONS [1 2 3]: РАСХОДИТСЯ (снятый 64990 у клиента: true) | [1][2][3]: == пересчёт (снятый 64990 у клиента: false) | [1 2][3]: == пересчёт (снятый 64990 у клиента: false) | [1][2 3]: == пересчёт (снятый 64990 у клиента: false)
M-100 / VB-I-2 / §15.9 п.2: разбиения ["[1 2 3]"] расходятся с полным пересчётом.
test result: FAILED. 0 passed; 1 failed
exit=101 (expected RED)

$ VERIFY_M100_CI_DRY=1 bash scripts/verify_M-100.sh
PASS  ci-parity: учтено шагов 64 из 64 (исполнено 57, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/tests/red_verify_M-100_ci_map.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
PASS  строка дописана в блок базы ⇒ FAIL (C-266 F2) (exit=1)
PASS  строка дописана в блок агрегата ⇒ FAIL (exit=1)
PASS  агрегат: новый джоб в условии — законно ⇒ PASS (exit=0)
PASS  агрегат: команда в строке условия ⇒ FAIL (exit=1)
PASS  (i) check_review_fa + дописка ⇒ исполняется, не SKIP (exit=0)
PASS  (ii) run: |- — литеральный блок, исполняется (exit=0)
PASS  (iii) run: > — складывающий скаляр ⇒ FAIL (exit=1)
VERDICT: PASS — 10 сценариев
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: VB-I — заявлено=12, в оракулах=10 — подтверждено замером (loose=10)
PASS  [6-RFC-SHA] SHA-подобных токенов: всего=40 проверено=40 пропущено=0
PASS  [7-RFC-PATH] путей-кандидатов: всего=283 проверено=183 пропущено=100
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --check origin/main...HEAD
exit=0

$ git fsck --no-dangling --no-reflogs
exit=0

$ bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-307 ← 2efd6ef4741137b42462efd0319e91ef2182bc02
reserve:   C-307 занят; следующий кандидат — C-308
reserve: попытка 2/8 — C-308 ← 2a9eba3455fa49f76eca6a4265156725919b7be3
C-308
reserve: резерв C-308 взят
exit=0

$ git fetch origin && git rev-parse origin/feat/M-100-three-serving-contracts
183f0f90cb5c17bac7aa5c8fb1afab27051a994c
exit=0
```
