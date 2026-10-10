# C-269 — M-91 legacy serving counters: REJECT

<!-- GATE-META
milestone: M-91
audited_repo: a3ka/hft-platform
audited_base: 395323c5f6a403f9b6c17306cc7f9f98f521e155
audited_head: 89ac1ae119015b78879cef2a6efc7c1745d8b6a9
verdict: REJECT
-->

## Verdict

**REJECT.** The set is otherwise complete and the three supplied RED cases are
behaviorally red, but they do not protect the stated classification of the
`Warming` readiness refusal. An implementation can account `NotReady` correctly
and silently omit `Warming`, passing l1–l3 while breaking the same serving-
silence signal the milestone claims to repair.

## Subject, scope, and artifact completeness

The audited subject is `origin/feat/M-91-legacy-serving-counters` at
`89ac1ae119015b78879cef2a6efc7c1745d8b6a9`. The architect committed the
complete pre-dev set in order:

| required artifact | evidence |
|---|---|
| milestone | `milestones/M-91-legacy-serving-counters.md` — `70d14cc` |
| T1/trait impact | none: the diff adds no `contracts/**` type or trait signature; it consumes the existing per-instance `ServingCountersHandle` API |
| RED suite | `crates/gateway-serve/tests/red_m91_legacy_counters.rs` — `6c7b19b` |
| acceptance gate | `scripts/verify_M-91.sh` — `bcefefc` |
| CI-map adversarial probe | `scripts/tests/red_verify_M-91_ci_map.sh` — `89ac1ae` |

The relevant live operational invariants are `OPS-I-8` (a live service with a
silent flow must alert) and `OPS-I-10` (declared observability has to be
emitted by the actual producer). `gateway-serve` has no separate FA on this
revision; the milestone names that existing documentation debt explicitly.

## Finding

### C-269-1 — only `NotReady`, not the stated readiness class, is tested (BLOCKER)

Milestone §3 says that both `not_ready` and `warming` are supported refusals,
matching the v1 path. The v1 code indeed increments
`refusals_supported` for every non-`Ready` readiness result
(`crates/gateway-serve/src/lib.rs:1195-1201`), and the legacy path already
distinguishes a `Warming` response (`:2054-2058`). `ServingOutcome::Warming`
is a real enum variant (`crates/gateway-serve/src/admission.rs:20-25`).

But l2 creates only the empty-checkpoint `NotReady` world and asserts its
literal `"not_ready"` response (`red_m91_legacy_counters.rs:251-270`). There
is no `Warming` world or an injected readiness outcome. This incorrect shape
passes every supplied test:

```rust
metrics::inc_attempts(counters.as_ref());
if !matches!(ready_outcome, ServingOutcome::Ready) {
    if matches!(ready_outcome, ServingOutcome::NotReady) {
        metrics::inc_refusals_supported(counters.as_ref());
    }
    // send the existing `warming` / `not_ready` error
}
```

It yields l1 `(1,1,0,0)`, l2 `(1,0,1,0)`, and l3 `(2,2,0,0)`, yet a warming
legacy request yields `(1,0,0,0)`. That removes the supported-refusal input
from `check_serving_silence`, contrary to the objective and `OPS-I-8`/`OPS-I-10`.
The present `readiness` implementation documents `Warming` as a later
worker-path outcome; that is precisely why the contract needs a deliberate
injection or seam now instead of becoming unprotected when that path arrives.

## Required before re-review

1. Add a RED oracle that deterministically reaches or injects `Warming` on
   the legacy path and asserts response `warming` plus exact instance deltas
   `(1,0,1,0)`.
2. Include the `Warming` mutation in the milestone's mutation evidence and
   retain the current l1–l3 exact-delta checks.
3. Commit and push the revised artifact set, then provide a fresh branch-head
   handoff for the re-audit.

## Done Block

```text
$ git fetch origin --prune && git rev-parse origin/feat/M-91-legacy-serving-counters
89ac1ae119015b78879cef2a6efc7c1745d8b6a9
exit=0

$ cargo test -p gateway-serve --test red_m91_legacy_counters
test l2_legacy_not_ready_counts_attempt_and_supported_refusal ... FAILED
test l1_legacy_success_counts_one_attempt_and_one_success ... FAILED
test l3_two_legacy_sessions_count_exactly_two ... FAILED
assertion `left == right` failed: TD-228: отказ not_ready в legacy-пути дал дельты (0, 0, 0, 0) вместо (1, 0, 1, 0)
assertion `left == right` failed: TD-228: успешная legacy-сессия … дала дельты (0, 0, 0, 0) вместо (1, 1, 0, 0)
assertion `left == right` failed: TD-228: две успешные legacy-сессии дали (0, 0, 0, 0) вместо (2, 2, 0, 0)
test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
exit=101  # expected RUNTIME-RED

$ bash scripts/tests/red_verify_M-91_ci_map.sh
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

$ git status --short
<empty before verdict creation>
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Metadata
- Date (UTC): 2026-10-02
- Milestone: M-91-legacy-serving-counters
- Status: BLOCKED — REJECT
- HEAD: 89ac1ae — `test(M-91): red_verify_M-91_ci_map.sh … [architect]`

## §B — What I did
- Audited the committed M-91 milestone, RED suite, acceptance gate, CI-map
  probe, legacy/v1 serving paths, counters, and serving-silence consumer.
- Measured the intentional runtime RED and the adversarial CI-map probe.

## §C — Result
- `C-269-m91-legacy-serving-counters.md` — REJECT.

## §D — Next agent + invocation
- **Next agent:** `architect`
- **Paste-ready prompt:**
  ```text
  Repair C-269 on M-91. Add a deterministic legacy Warming RED case (or a
  narrow readiness injection seam) asserting response `warming` and exact
  per-instance deltas (attempts, successes, refusals_supported,
  refusals_unsupported) = (1,0,1,0). Demonstrate that omitting the Warming
  refusal increment makes that test fail. Commit/push the revised complete
  artifact set and hand off the exact current origin branch head to critic.
  ```
- Push status: pending this verdict commit.
- Cache: no build-cache cleanup performed; this detached audit worktree remains available.

## §E — Risks / open questions
- The milestone's named limitation about disconnecting exactly between counting
  and send remains honest; it does not excuse the separate missing Warming
  classification case.

=== END HANDOFF ===
