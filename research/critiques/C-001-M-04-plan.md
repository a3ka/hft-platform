# C-001 — Critic Verdict — M-04 Research core (plan-time, post-Task-1 artifact audit)

**Date:** 2026-07-10
**Milestone:** `milestones/M-04-research-core.md`
**Audited range:** `HEAD~5..HEAD` (`5df5684`..`ef0d9f2`) — milestone+spec, signals scaffold+RED,
sim scaffold+RED, research-cli scaffold+RED, verify_M-04.sh.
**Critic:** critic (plan-time gate), per `.claude/agents/critic.md` + `.claude/rules/gates.md`.
**Trigger match:** ≥5 atomic commits (yes, 5) + new crates introduced (yes, 3: sim/signals/research-cli)
→ critic mandatory per `.claude/rules/gates.md` §1. Milestone header self-declares the same triggers.

## Verdict

**REJECT**

## Verdict Justification

Architect-committed artifact set is materially complete (types/traits/RED-suites/verify-script/
milestone/spec all present, and every behavioral RED test empirically fails on the `todo!()`
scaffold — genuine RED, not placebo). However, the OBI `TopN` mode — the ONLY track the
hypothesis card itself says is computable right now (Track B/bands is explicitly degenerate on
current data, per `research/hypotheses/H-20260710-obi-asym.md`) — depends on a `book::OrderBook`
primitive that does not exist and that `signal-engineer` (Task 3) is structurally forbidden from
adding (`crates/book/**` is listed forbidden for **all** dev agents in the milestone's own scope
table). Four of five OBI RED tests exercise exactly this mode. Task 3 cannot be completed as
currently specified without either an unauthorized change to a sacred crate or a
`!!! SCOPE VIOLATION REQUEST !!!` round-trip that a plan-time gate exists precisely to catch
before dev dispatch. This is a fast, narrow fix (one architect-added method on `OrderBook`); once
added, re-submit for critic re-pass.

## Findings

### CRITICAL @ HIGH-confidence

**C1 — `book::OrderBook` has no top-N-levels depth primitive; blocks Task 3 (`ObiMode::TopN`) as scoped.**

- Evidence: `crates/signals/src/obi.rs:23` declares `TopN { n_levels: usize }`.
  `crates/signals/tests/test_obi_determinism.rs` exercises `ObiMode::TopN { n_levels: 5 }` in
  `test_obi_determinism`, `test_obi_no_signal_below_theta`,
  `test_obi_direction_range_and_status_tag`, `test_obi_no_lookahead` (4 of 5 OBI RED tests).
  `crates/book/src/lib.rs`'s only depth-aggregation primitives are `depth_within(side, pct)`
  (percentage-band from mid) and `n_levels(side)` (a **count**, not a size sum). `OrderBook`'s
  `bids`/`asks` fields (`BTreeMap<i64, i64>`) are private with no accessor/iterator exposed
  (checked full file, 213 lines — no `levels()`/`iter()`/`top_n`-shaped public method exists).
  There is no way to compute "sum of sizes of the top N discrete price levels per side" from
  `book`'s current public API.
- Scope conflict: the milestone's own "Allowed / Forbidden paths" table lists
  `crates/book/**` under "Forbidden" for "все dev" (all dev agents), and `signal-engineer`'s
  allowed paths are `crates/signals/src/**`, `crates/signals/Cargo.toml`,
  `research/specs/` only. Task 3 as authored requires a capability signal-engineer is
  forbidden from adding.
- Why this matters now, not later: `research/hypotheses/H-20260710-obi-asym.md` itself found
  (5959 live snapshots, 2026-07-10) that Track B (price bands 3%/8%) is currently **degenerate**
  — top-20-level books reach only ~0.1% from mid, two orders of magnitude short of 3% — so
  bands cover the *entire* book on both sides. Track A (`TopN`) is explicitly the only
  immediately-backtestable track. Blocking exactly this mode blocks the milestone's actual
  near-term payload (Task 8's OBI Track A run).
- Suggested fix: architect adds a primitive to `crates/book/src/lib.rs`, e.g.
  `pub fn top_n_depth(&self, side: Side, n: usize) -> i64` (sum of the `n` best levels' sizes
  per side, mirroring the existing `depth_within`/`notional_within` shape), before Task 3
  dispatch. `book/` is architect/engine-dev territory per scope-guard, so this is a
  same-day fix, not a redesign.

### MAJOR

**M1 — Milestone's "T1 НЕ трогается" claim conflicts with `docs/05-contract-layer.md` §2's own
table and with `docs/fa/research-cli.md` §N's own classification.**

- `docs/05-contract-layer.md` §2 (the canonical T1 table) lists **`ValidationReport`
  (`metrics.json`)** and **`TrialsLedger` entry** explicitly as T1 contracts, with
  research-cli as producer.
- `docs/fa/research-cli.md` §N "Интерфейсные контракты" itself labels these the same way:
  *"`ValidationReport`/`metrics.json` (T1 → risk-critic + founder читают, 05 §2);
  `TrialsLedger` записи (T1, append → глобальный, ...)"*.
- The milestone's "Contract impact (T1)" section nonetheless states flatly **"T1 НЕ трогается"**
  and keeps `TrialRecord`/`ValidationReport` Rust types living in
  `crates/research-cli/src/types.rs` rather than `crates/contracts/`, deferring promotion to a
  future contract-RFC "when a Python consumer appears," with a TECH-DEBT entry planned at merge.
- This is **not silent** (explicitly named + committed to a follow-up), and it does not touch
  `crates/contracts/` (so it does not trip the literal Block-C/RFC-gate on edits to that path).
  But it does unilaterally reclassify — via milestone prose, not an FA amendment or RFC — two
  forms that the FA (a STABLE/APPEND-ONLY document) and the master contract table both already
  call T1. The "single source of truth" for T1 vs T2 should not fork between `05 §2`/`research-cli.md §N`
  and a milestone's own contrary paragraph.
- Suggested resolution (either is acceptable, pick one before merge): (a) promote now via a
  minimal contract-RFC (schema + `report_schema_version` already exists, so the mechanical
  lift is small), or (b) amend `docs/fa/research-cli.md` §N with a named annotation
  ("T1-designate; promotion deferred pending a cross-language consumer — see TECH-DEBT-XXX")
  so the FA and the milestone agree, and log the tech-debt entry architect-side rather than
  leaving it to reviewer's memory at merge time.

### MINOR

**m1 — `scripts/verify_M-04.sh` has no check for Task 8.**
`.claude/rules/gates.md` §3 requires "минимум 1 проверка на задачу". Tasks 1–7 each have ≥1
labelled `check` in the script; Task 8 (OBI Track A run + `research/reports/R-001*`) has none.
This is very likely intentional (Task 8 is human/risk-critic/founder-gated and data-accumulation-gated,
not mechanically verifiable on the same schedule as the others) — but the exemption is only
stated in milestone prose ("Задача 8 гейтится накоплением данных..."), not as an inline comment
in the verify script itself, unlike the pattern used for T5/T7's task-to-check mapping. Add a
one-line comment in the script noting Task 8 is out-of-band by design, for future readers of the
script alone.

**m2 — RC-I-2 (`test_ledger_append_only`) verifies the append-only *effect*, not the *mechanism*.**
The test (`crates/research-cli/tests/red_research.rs::test_ledger_append_only`) checks that
`bytes2.starts_with(&bytes1)` after a second `append()` call. `docs/fa/research-cli.md` §6 states
the file is opened "ТОЛЬКО `O_APPEND`" (a stronger, mechanism-level claim). A hypothetical
implementation that reconstructs the whole file on each write (read old bytes + old bytes +
new record, then full rewrite) would satisfy this test's assertions without ever opening the
fd in append mode. Not a placebo — no no-op/never-write stub could pass it — but it is a weaker
oracle than the FA's own wording promises. Not blocking; noting for awareness, since RC-I-2's
integrity claim is used elsewhere (D8 hash-chain tamper-detection) and a rewrite-based
implementation is harder to reason about for crash-safety.

### Open Questions

**OQ1 —** `docs/fa/signals.md` §O still lists `horizon_ms` ownership (signal vs. `alpha`) as an
open question to be fixed "before M-04", while D2 in the milestone already assigns it to the
research-harness for M-04's scope specifically. Fine for this milestone's scope (research-cli is
the only consumer right now), but the RED test `test_obi_direction_range_and_status_tag` already
locks in exact horizon_ms passthrough behavior — worth flagging that this test's contract may
need revisiting once a live owner is decided in P3, not a defect now.

**OQ2 —** T5a/b/c (latency/fees artifact existence+provenance) correctly FAIL right now since
Task 5 is `⏳ OPEN` — this is the expected pre-implementation state per the milestone's own
commit message ("ожидаемо до задач 2-5"), not a new finding.

## Pre-check results

- **B4-pre-equivalent (cross-module type without RFC):** see M1 above — no NEW type added
  directly to a module's `types.*` claiming to be shared; the finding is about an EXISTING
  FA-declared T1 form being kept out of `contracts/` past its stated tier. Logged as MAJOR,
  not REJECT, since no `contracts/` edit occurred and the deferral is explicit.
- **B5-pre (edits to `contracts/` outside RFC):** PASS — no commit in range touches
  `crates/contracts/**`.
- **Acceptance-script-as-real-gate:** PASS (structure) — `set -euo pipefail` + explicit
  `check()` FAIL-counter aggregator, no `cmd && echo PASS || echo FAIL` masking, `VERDICT:`
  final line with matching exit code. Empirically re-ran: `bash scripts/verify_M-04.sh` →
  `VERDICT: FAIL (6 провалов)`, `exit=1` — matches expected pre-Task-2..5 state exactly
  (T2/T3/T4 RED-suite fail + T5a/b/c artifacts absent; T1/T6/T7/T8-grep all PASS). Task
  coverage gap noted as MINOR m1 above.
- **Anti-placebo re-run (all 3 new crates, empirically executed, not inferred):**
  - `cargo test -p sim` → all 13 behavioral RED tests FAIL on `todo!()` (`fill_model.rs`,
    `latency.rs`, `fees.rs`, `divergence.rs`); `cargo test -p sim --test structural` → 4/4 PASS
    (canaries correctly green from day one).
  - `cargo test -p signals` (`red_signals.rs` + `test_obi_determinism.rs`) → 11/11 behavioral
    tests FAIL on `todo!()` (`lib.rs::SignalId::parse`, `registry.rs`); `--test structural` →
    4/4 PASS.
  - `cargo test -p research-cli` (`red_research.rs`) → 12/12 FAIL on `todo!()`
    (`ledger.rs`, `split.rs`, `metrics.rs`, `report.rs`, and transitively `sim`'s `latency.rs`
    for the grid-running tests); `--test structural` → 3/3 PASS.
  - No behavioral RED test passes GREEN against the current no-op scaffold anywhere in the
    three new crates — anti-placebo gate (`.claude/rules/gates.md` §2) holds for the
    as-committed state.
  - `cargo fmt --all -- --check` → PASS. `cargo clippy --workspace --all-targets -- -D warnings`
    → PASS (0 warnings across all 9 workspace crates).
  - `cargo test -p contracts -p journal -p book` (regression) → PASS, no breakage from the new
    crates' addition.
- **Scope-guard table cross-check (`docs/04-workflow.md` §1` / `.claude/rules/scope-guard.md`):**
  Milestone's own Allowed/Forbidden table matches the canonical role table (architect:
  milestones+tests+types+verify; engine-dev: `crates/sim/src/**`; signal-engineer:
  `crates/signals/src/**`+`research/specs/`; research-dev: `crates/research-cli/src/**`+
  `research/{latency,fees}/`; sacred zones `contracts/`, `journal/`, `book/`, `venue-*/`,
  `*/tests/**`, `scripts/**` correctly forbidden to all devs) — PASS, **except** see C1: the
  scope table is internally consistent but Task 3 as scoped cannot be completed without
  touching the forbidden `book/` zone.
- **D1-D11 decisions vs. FA/DESIGN:** D1 (score range), D3 (code_hash = sha256 of module
  source), D6 (walk-forward defaults), D10 (SplitMix64, no `rand` dep — verified via
  `test_no_rand_crate` PASS), D11 (registry scope) — all consistent with `signals.md`/`sim.md`.
  D9 (OBI reads `book` primitives, doesn't reimplement bands) is consistent for `Bands` mode
  (`depth_within` exists and is used) but is **not actually satisfiable** for `TopN` mode today
  — see C1. D7 (latency artifact honesty + provenance) is consistent with `sim.md` §6 and is
  grep-enforced (`test_no_hardcoded_default_latency` PASS).
- **H-card ↔ S-001 spec ↔ RED-tests consistency:** grid values match 1:1 (n_levels
  `{1,5,10,20}`, horizon `{500,1000,2000,5000}ms` = H-card's `{0.5,1,2,5}s`, bands
  `{0.005..0.08}` including the founder's 3%/8% case gated on TD-004 full-book data). No
  contradiction found between the three documents.

## Confidence

**High** — every claim above is backed by direct file reads (milestone, both FA docs,
governance doc, all `crates/{sim,signals,research-cli}` source + test files, `book/src/lib.rs`
in full) and by actually executing `cargo test`/`cargo fmt`/`cargo clippy`/`bash
scripts/verify_M-04.sh` against the current `HEAD` (`ef0d9f2`), not by inference from
commit messages.

## Next agent

Per `.claude/agents/critic.md` Handoff discipline: REJECT → **architect** (Fable). No
architect-self-fix-loop delegation implied; architect revises (primarily: add the missing
`book` top-N primitive per C1, and pick a resolution for M1), then re-invokes critic on the
revised commit range before founder dispatches dev agents.
