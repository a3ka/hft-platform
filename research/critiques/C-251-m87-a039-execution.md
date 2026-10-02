<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: 46a989a0764eafe2ba3b2010f557a2ffeabe1932
audited_head: 4284cedf954cbf98ba0ff37f27bdc61814f7d5e0
verdict: REJECT
-->

# C-251 — M-87: A-039 execution remains non-mechanical

## Verdict: REJECT

Scope is deliberately limited to execution of the binding arbitration decision
`research/arbitration/A-039-m87-common-policy-proof.md`. The decision's threshold,
anchor, boundary-mutation form, and post-task-12 tester duty were not reconsidered.

The A-039 artifact set exists: shared registry, green meta-test, changed COMPILE-RED
entrypoint test, A-039 verify step, and milestone appendix. `GW-I-9(б)` remains the
live invariant for the affected `gateway-serve` path: an unsuitable snapshot must give a
named readiness outcome rather than silently rebuild on the live path.

The verdict is nevertheless REJECT because two required A-039 mechanisms can be bypassed
while `red_m87_registry` stays green.

## Blocking findings

### R6-1 — registry does not feed checkpoint-state fixtures

**Decision violated:** A-039 §Q2(2), §Q3, and implementation step 2 require fixture
checkpoint state (`None` / corrupt / incompatible / warm), not just tail length, to be
selected from the registry. This is the mechanism which makes a `ShortCircuit` row a fact
about the fixture rather than a declaration beside it.

`registry_checkpoint()` and `CheckpointFixture` implement that mapping in
`crates/gateway-serve/tests/m87_registry/mod.rs:276-307`, but have no caller outside their
own definitions. The entrypoint still decides its state independently: for example the
registered `CkptMissing` scenario calls `serve(..., None)` at
`red_m87_entrypoint.rs:793-796`, and the corrupt case writes its own checkpoint at
`:809-822`.

Reproduction changed only the registry row
`u1_entry_missing_checkpoint_over_trap_gives_named_outcome` from
`ShortCircuit { CkptMissing, ... }` to `Freshness { tail: 0, served: true }`. The source
fixture remained `serve(..., None)`, but all five registry tests passed (exit 0). Thus
`ShortCircuit` has no numeric field *only after the author has chosen that variant*;
writing an unjustified zero through `Freshness` remains possible and is unobserved.

This is the exact class A-039 ruled out: the registry describes a fixture whose decisive
state is still authored elsewhere. Dev dispatch is blocked until the registry actually
drives, and the meta-test proves, the relevant fixture state.

### R6-2 — the bijection parser silently misses a valid test form

**Decision violated:** A-039 §Q2(3) requires the parser setup-guard to equate the number
of test attributes with extracted names. `red_m87_registry.rs:32,67` recognises only a
direct `#[tokio::test...]` or exactly `#[test]`; both the attribute count and extractor
therefore omit a valid conditional attribute.

Temporary addition to the subject file:

```rust
#[cfg_attr(test, tokio::test)]
async fn critic_cfg_attr_parser_blind_spot() {}
```

left `cargo test -p gateway-serve --test red_m87_registry` green (5 passed, exit 0), even
though `#[cfg_attr(test, test)] fn cfg_attr_is_a_real_test() {}` compiled with
`rustc --test` and appeared in `--list` as a test. An unregistered scenario can therefore
be introduced in a legal form without being counted by either side of the claimed
setup-guard. This is not a parser limitation that is named in A-039; it invalidates the
claimed bijection.

## Checks that do hold

- Entry-point tail writes take their tail from `registry_tail()` at
  `red_m87_entrypoint.rs:707-710,891-894,963-966`; the exact literal-loop canary reports
  zero matches. `policy()` takes both numeric fields from the shared constants at
  `:142-143` (two of two form matches).
- Adding a direct `#[tokio::test]` without a registry line makes `r1` red; renaming a
  registry line also makes `r1` red. The four required A-039 §Q2(5) mutations each changed
  the file, then made the meta-test red; restoration returned the five-test suite green.
- The A-039 verify step invokes the registry binary by exit code and printed FAIL after
  deleting the c9 registry row. Its two present canaries inspect source form, rather than
  comments.
- The named limit for `SlotOverloaded` is honest: `W_SLOT` cites c4 itself and the absence
  of an external witness is stated in both registry and milestone. A-039 permits that
  declared limit; it is not a separate finding.

## Required resubmission evidence

1. A mutation changing a checkpoint-stage registry row to `Freshness { tail: 0, ... }`
   must make the A-039 meta-gate red, while the entrypoint fixture is demonstrably still in
   its original checkpoint state.
2. A valid conditional test attribute in the subject file must be counted or otherwise
   rejected fail-closed by the parser setup-guard; it may not leave the gate green.
3. Re-run the two direct-bijection probes, all four A-039 §Q2(5) mutations with
   file-changed checks, and the A-039 acceptance step with a deleted registry row.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-87-serving-circuit-breaker
4284cedf954cbf98ba0ff37f27bdc61814f7d5e0
exit=0

$ bash scripts/next_artifact_id.sh C
C-251
exit=0

$ cargo test -p gateway-serve --test red_m87_registry
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ add direct #[tokio::test] critic_unregistered_registry_scenario; cargo test -p gateway-serve --test red_m87_registry
file_changed_exit=1 (expected 1)
сценарии файла НЕ НАЗВАНЫ в реестре: ["critic_unregistered_registry_scenario"]
test result: FAILED. 4 passed; 1 failed
exit=101

$ rename the c9 registry line; cargo test -p gateway-serve --test red_m87_registry
file_changed_exit=1 (expected 1)
сценарии файла НЕ НАЗВАНЫ в реестре: ["c9_four_positions_differ_and_stale_snapshot_does_not_stop_serving"]
test result: FAILED. 4 passed; 1 failed
exit=101

$ A-039 §Q2(5) mutations (each preceded by git diff --quiet)
(a) delete c9 row: file_changed_exit=1; r1 + r3 FAILED; exit=101
(b) c9 tail 200 -> 2_000: file_changed_exit=1; r2 FAILED; exit=101
(c) MAX_TAIL_EVENTS 1_000 -> 150: file_changed_exit=1; r2 FAILED; exit=101
(d) EXPECTED_WARMUP_EVENTS 100 -> 300: file_changed_exit=1; r3 FAILED; exit=101
restore: test result: ok. 5 passed; 0 failed; exit=0

$ add #[cfg_attr(test, tokio::test)] unregistered scenario; cargo test -p gateway-serve --test red_m87_registry
cfg_attr_probe_file_changed_exit=1 (expected 1)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
registry_exit=0

$ printf '#[cfg_attr(test, test)]\nfn cfg_attr_is_a_real_test() {}\n' | rustc --test -o /tmp/critic-cfg-attr-probe - && /tmp/critic-cfg-attr-probe --list
cfg_attr_is_a_real_test: test
1 test, 0 benchmarks
rustc_exit=0

$ change u1_missing registry row to Freshness { tail: 0, served: true }; cargo test -p gateway-serve --test red_m87_registry
entrypoint fixture: let addr = serve(dir.path(), None).await;
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
registry_exit=0

$ delete c9 registry row; bash scripts/verify_M-87.sh
FAIL  A-039: реестр общего порога КРАСЕН — test result: FAILED. 3 passed; 2 failed
сценарии файла НЕ НАЗВАНЫ в реестре: ["c9_four_positions_differ_and_stale_snapshot_does_not_stop_serving"]
A-039 substep cargo exit=101

$ grep -cE 'for i in 0\.\.[0-9_]+i64' crates/gateway-serve/tests/red_m87_entrypoint.rs
0
$ grep -cE 'max_tail_events: m87_registry::MAX_TAIL_EVENTS|expected_warmup_events: m87_registry::EXPECTED_WARMUP_EVENTS' crates/gateway-serve/tests/red_m87_entrypoint.rs
2

$ git diff --exit-code -- crates/gateway-serve/tests/m87_registry/mod.rs crates/gateway-serve/tests/red_m87_entrypoint.rs
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-23T15:44Z
- Milestone: M-87-serving-circuit-breaker, round 6 (A-039 execution only)
- Статус: BLOCKED — REJECT
- HEAD: 4284ced — test(M-87): A-039 — реестр ПИТАЕТ фикстуру; полнота предъявлена прогоном СЕГОДНЯ [architect]

## §B — Что я сделал
- Audited only the committed A-039 execution set and reran its required direct-bijection and mutation probes.
- Reproduced two green bypasses of required A-039 mechanisms; all temporary source mutations were restored.

## §C — Артефакты / результаты
- `research/critiques/C-251-m87-a039-execution.md`
- Done Block above: registry baseline exit=0; required mutations exit=101; bypass probes exit=0 where a fail-closed gate was required.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-87 round 6 is REJECTED by C-251 on A-039 execution only. Read A-039 and C-251.
  Correct the two reproduced mechanisms without reopening A-039's fixed threshold, anchor,
  boundary-mutation form, or tester duty: (1) registry checkpoint-stage data must drive and
  be mechanically tied to the entrypoint fixture; (2) the registry parser must not silently
  omit a valid conditional test attribute. Add RED evidence for both bypasses, rerun all
  A-039 mutations plus the deleted-row acceptance step, commit and push the artifact set,
  then request the next critic round.
  ```
- Push-статус: pending this verdict commit to `origin/feat/M-87-serving-circuit-breaker`
- ⏸ кэш оставлен — нужен до commit/push гейта

## §E — Риски / открытые вопросы
- No arbitration escalation: these are concrete failures to implement binding A-039 requirements, not a reopened methodological dispute.
- The `SlotOverloaded` self-witness remains a named, accepted A-039 limit.

=== END HANDOFF ===
