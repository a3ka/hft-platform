# C-008 — Critic Verdict — CT-RFC-03 Recon audit event

**Date:** 2026-07-16T09:42Z  
**Role:** critic, plan-time T1 contract-RFC gate  
**Branch:** `origin/feat/M-09`  
**Audited HEAD:** `64c0a9e` — `contract(CT-RFC-03): SysEvent::ReconDivergence — аудит сверки с биржей (M-09 task 1, T1)`  
**Trigger:** `gates.md` §1.1 — `crates/contracts/**` T1 change, critic mandatory.  
**Boundary:** OPS-I-1..9 RED oracles for M-09 task 2 are intentionally not present yet and were not audited as missing.

## Verdict

**APPROVE**

## Verdict Justification

CT-RFC-03 is a complete atomic T1 package and the executed checks validate its core safety claims: the new `SysEvent` variant is appended at postcard discriminant 3, old events roundtrip byte-for-byte, schema generation is in sync with Rust types, fixtures validate/reject as expected, and the anti-placebo compile-RED fails when `ReconDivergence` is removed. The `schema_version` no-bump is acceptable here because this RFC does not change the segment envelope introduced by CT-RFC-02; it is an additive `EventKind`/`SysEvent` extension consistent with CT-RFC-01 precedent.

## Pre-flight Verification

- Commit ancestry: PASS. `git rev-list --count origin/main..HEAD` returned `1`.
- Audited commit: `64c0a9e`.
- Changed paths are scoped to the RFC package:
  - `crates/contracts/src/lib.rs`
  - `crates/contracts/schema/event.schema.json`
  - `crates/contracts/fixtures/valid/event-recon.json`
  - `crates/contracts/fixtures/invalid/event-recon-unknown-action.json`
  - `crates/contracts/tests/red_rfc03.rs`
  - `crates/contracts/CHANGELOG.md`
  - `docs/rfc/CT-RFC-03-recon-audit.md`
- Contract-RFC present: PASS. `docs/rfc/CT-RFC-03-recon-audit.md` is in the same atomic commit as the T1 changes.
- Critic artifact path: PASS. This verdict is under `research/critiques/` per `scope-guard.md` and `branch-hygiene.md` §3.

## Executed Checks

### 1. Contract and workspace tests

- `cargo test -p contracts` → PASS, exit 0.
  - `red_rfc03`: 6 passed, 0 failed.
  - `red_schema`: 2 passed, 0 failed.
  - Existing RFC suites remained green.
- `cargo test --workspace` → PASS, exit 0.

### 2. Anti-placebo compile-RED

Temporary mutation in the critic worktree:

- Removed `SysEvent::ReconDivergence` from `crates/contracts/src/lib.rs`.
- Re-ran `cargo test -p contracts --test red_rfc03`.
- Result: PASS as a RED oracle. Compilation failed with `E0599` at `red_rfc03.rs:35` and `red_rfc03.rs:131`: no variant or associated item named `ReconDivergence` found for enum `SysEvent`.
- Restored the file and re-ran `cargo test -p contracts --test red_rfc03` → PASS, exit 0.

Conclusion: `red_rfc03` is not placebo; it does not pass without the new T1 variant.

### 3. Additivity and serialization

Independent one-off Rust probe linked against the restored `contracts` artifact and serialized with `postcard`.

Observed output:

```text
schema_version=2 sys_discriminants=0/1/2/3 action_discriminants=0/1 old_roundtrip_bytes=heartbeat:10 trade:37
exit=0
```

Critic conclusion:

- `SysEvent::{Heartbeat,ConnUp,ConnDown}` remain discriminants `0/1/2`.
- `SysEvent::ReconDivergence` is appended as discriminant `3`.
- `ReconAction::{AlertOnly,Resynced}` are `0/1`.
- Old `Heartbeat` and `Md(Trade)` events deserialize and reserialize byte-for-byte.
- CT-I-3 additivity is satisfied.

### 4. Schema generation and fixtures

- `cargo run -p contracts --example gen_schema` wrote the schema files.
- `git diff --exit-code -- crates/contracts/schema` → PASS, exit 0.
- `cargo test -p contracts --test red_schema` → PASS, exit 0.

Critic conclusion:

- `event.schema.json` is generated from Rust types, not hand-drifted.
- The valid recon fixture parses.
- The invalid unknown-action recon fixture rejects.

## T1 Package Completeness

PASS against `docs/05-contract-layer.md` §4/§6:

- Rust T1 types: `ReconAudit`, `ReconAction`, and `SysEvent::ReconDivergence`.
- Generated JSON Schema: `crates/contracts/schema/event.schema.json`.
- Fixtures: valid recon event and invalid unknown action.
- CHANGELOG entry: `crates/contracts/CHANGELOG.md`.
- RED tests: `crates/contracts/tests/red_rfc03.rs` plus schema/fixture parity in `red_schema.rs`.
- Migration/version note: CT-RFC-03 §3 states why `schema_version` remains 2 and ties that to CT-RFC-01/02 semantics.

## Schema Version Decision

APPROVED: no bump required for CT-RFC-03.

Reasoning:

- CT-RFC-02 bumped `schema_version` because it changed the segment envelope by adding `SegmentHeader`.
- CT-RFC-03 does not change `SegmentHeader` or the `Event` envelope; it appends a `SysEvent` variant.
- The old-event serialization probe validates that existing `Heartbeat` and `Md(Trade)` bytes remain stable under the new code.
- CT-RFC-01 is the relevant precedent for additive event payload/venue expansion without segment-format bump.

Non-blocking note: `docs/05-contract-layer.md` §4 says every T1 change includes a version bump. The implemented convention in CT-RFC-01/02/03 treats `SCHEMA_VERSION` as journal segment/envelope version, not every additive enum extension. That convention is coherent, but a future doc cleanup could make the wording less broad.

## Recon Event Shape

APPROVED: the `ReconAudit` shape is sufficient for OPS-I-1.

Evidence:

- `docs/fa/ops.md` §4 requires a durable `Sys` event when recon divergence crosses threshold and says offline tooling aggregates those events to answer which data can be trusted.
- `ReconAudit` carries the necessary provenance and action fields: `venue`, `symbol`, event time/order from `Event.seq` and timestamps, `divergence_bps`, `best_price_diverged`, and `action`.
- `best_price_diverged` is correctly separated from band-sum magnitude. Best bid/ask divergence is the C1 corruption class and should not be collapsed into distant-band noise.
- Fixed struct fields are accepted. Because postcard struct extension is not additive for old records, any future need for bid/ask side, per-band magnitudes, or richer forensic payload must be a new contract-RFC, not an ungoverned field append.

## Findings

### CRITICAL

- None.

### MAJOR

- None.

### MINOR / Notes

- N1 — `docs/05-contract-layer.md` §4 has broad "bump version for any T1 change" wording. CT-RFC-03's no-bump decision is still approved because repo precedent and `SCHEMA_VERSION` usage make it a segment/envelope version. Treat this as a future wording cleanup, not a blocker.

## Recommended Next Action

Proceed to reviewer Contract Block-C:

- Verify all `crates/contracts/**` changes are inside this RFC package.
- Merge `feat/M-09` into `main` if reviewer passes.
- Then architect continues M-09 task 2: RED oracles OPS-I-1..9 plus `verify_M-09.sh`.

## Confidence

High. I read `CLAUDE.md`, `gates.md`, `scope-guard.md`, `handoff-block.md`, `branch-hygiene.md`, `docs/05-contract-layer.md`, `docs/fa/ops.md` §4/§6, CT-RFC-03, the full contract diff, RED tests, schema, fixtures, and changelog. I executed the requested cargo gates, anti-placebo mutation, schema generation parity, and an independent postcard serialization probe.

=== HANDOFF: critic → reviewer ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-07-16T09:42Z
- Milestone: M-09 task 1 / CT-RFC-03
- Статус: APPROVE from critic; reviewer Contract Block-C pending
- HEAD: `64c0a9e` before this verdict commit — `contract(CT-RFC-03): SysEvent::ReconDivergence — аудит сверки с биржей (M-09 task 1, T1)`

## §B — Что я сделал
- Audited CT-RFC-03 against `docs/fa/ops.md` §4 and `docs/05-contract-layer.md` §4/§6.
- Ran contract tests, workspace tests, anti-placebo compile-RED, schema generation parity, and independent postcard serialization checks.
- Wrote this verdict artifact under `research/critiques/`.

## §C — Артефакты / результаты
- Verdict file: `research/critiques/C-008-CT-RFC-03.md`
- Verdict: APPROVE.
- Done Block exit-codes:
  - `cargo test -p contracts` → exit 0.
  - `cargo test --workspace` → exit 0.
  - anti-placebo `cargo test -p contracts --test red_rfc03` with `ReconDivergence` removed → compile-RED exit 101 (`E0599`) as expected.
  - restored `cargo test -p contracts --test red_rfc03` → exit 0.
  - `cargo run -p contracts --example gen_schema` + schema diff check → exit 0.
  - independent postcard serialization probe → exit 0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `reviewer`
- **Paste-ready промпт:**
  ```
  Ты — reviewer проекта hft-platform. Возьми свой worktree от origin/feat/M-09.

  Review CT-RFC-03 at branch origin/feat/M-09 after critic APPROVE:
  - Contract commit: 64c0a9e
  - Critic verdict commit: branch tip containing research/critiques/C-008-CT-RFC-03.md
  - Critic verdict file: research/critiques/C-008-CT-RFC-03.md

  Run Contract Block-C:
  1. Verify all crates/contracts/** changes are inside atomic RFC docs/rfc/CT-RFC-03-recon-audit.md.
  2. Verify no non-RFC T1 edits are present.
  3. Re-run appropriate contract/workspace tests or audit the critic Done Block as your protocol requires.
  4. If APPROVED, merge feat/M-09 into main and push per branch hygiene.

  Expected post-critic HEAD on origin/feat/M-09: branch tip containing research/critiques/C-008-CT-RFC-03.md.
  After reviewer merge, hand back to architect for M-09 task 2: RED oracles OPS-I-1..9 + scripts/verify_M-09.sh.
  ```
- Push-статус: verdict commit will be pushed by critic to `origin/feat/M-09` in this turn.

## §E — Риски / открытые вопросы
- OPS-I-1..9 RED oracles are intentionally next architect block and were not audited as missing.
- Future expansions of `ReconAudit` require a new contract-RFC because postcard struct field append is not additive for old records.
- Storage Box remains founder-owned and still blocks M-09 task 3 / M-08 retention tail, not this CT-RFC-03 merge.

=== END HANDOFF ===
