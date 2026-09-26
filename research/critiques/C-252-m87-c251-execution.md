<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: 524ab949e0468b772cd313e7d87e55ca3747414b
audited_head: 135ba1b9a7d512195be370f38ed10e78ec597312
verdict: ESCALATE
-->

# C-252 — M-87: C-251 execution still has a textual-parser bypass

## Verdict: ESCALATE

This is round 7 and is deliberately limited to closing the two findings of
`C-251-m87-a039-execution.md`. The A-039 threshold, anchor, boundary-mutation
form, and the tester duty after task 12 were neither reconsidered nor changed.

`GW-I-9(б)` remains the applicable live invariant: an unsuitable checkpoint
must produce a named readiness outcome and must not silently rebuild on the
serving path.

One C-251 mechanism is now proved: registry checkpoint state drives the
fixture. The other is not closed. More specifically, the current construction
still treats heuristic text inspection as a complete fail-closed parser. This
is the same class as C-251 R6-2, now found in two independent places. A further
critic self-fix loop would be the prohibited repeated-REJECT pattern; the next
step is an independent arbiter under `gates.md` §0.

## What holds

### C-251 R6-1 is closed — the driver is called

`ckpt_from_registry()` calls `m87_registry::registry_checkpoint()`
(`red_m87_entrypoint.rs:203-250`), `serve_for()` calls that driver
(`:260-264`), and the c4 special case calls the same driver (`:603-605`).
The only checkpoint constructions are the driver's warm/incompatible branches
and the named trap guard: three `checkpoint::advance` occurrences at
`:213`, `:229`, and `:835`; the checkpoint file path is constructed once at
`:255`. There is no scenario-body choice of `None`, corrupt bytes, or a foreign
schema version outside that driver.

The original bypass was reproduced with a file-change control: changing the
registry row for
`c1_entry_cold_request_named_outcome_without_reading_journal` from
`ShortCircuit { CkptMissing, ... }` to
`Freshness { tail: 0, served: true }` made `r6` fail (exit 101) because the
body does not expect a snapshot. Restoring the row returned the eight-test
registry suite to green. This is the requested present-day proof that the
driver is not merely adjacent to the fixture.

### C-251 R6-2 is partly closed

Adding exactly `#[cfg_attr(test, tokio::test)]` over a dummy scenario changed
the subject file and made `r0b_unrecognised_test_attribute_is_refused_not_ignored`
fail (exit 101), naming the unrecognised attribute. The unmodified subject
already contains four `#[cfg(feature = "testing")]` attributes
(`red_m87_entrypoint.rs:299,577,584,587`), and the baseline `r0b` run is green;
the head-based classifier therefore does not reproduce the former false
positive for that feature gate.

All four binding A-039 mutations were also repeated with a file-change check:
deleting the c9 row, making its tail 2,000, setting the limit to 150, and
setting the warmup anchor to 300 each made the registry suite red (exit 101).
Restoration returned it to 8 passed / exit 0. Deleting c9 and running
`scripts/verify_M-87.sh` reported `FAIL A-039` with the missing scenario and
ended `VERDICT: FAIL`, exit 1. The other intentional COMPILE-RED failures in
that script are not findings in this narrowly scoped round.

## Blocking continuation of C-251 R6-2

### R7-1 — an unknown proc-macro test attribute is still silently classified as Other

`classify_attr()` only calls an attribute suspicious when its head starts or
ends in `test`, or when `cfg_attr` contains that substring
(`red_m87_registry.rs:47-79`). A valid property-test proc-macro form such as
`#[quickcheck]` (when `quickcheck_macros` is in scope) has neither spelling.
The classifier returns `Other`; `scenarios_in()` neither arms nor rejects it.

The subject is intentionally COMPILE-RED, so the registry binary consumes its
source via `include_str!` rather than compiling it. I added the following dummy
scenario temporarily, with a file-change check:

```rust
#[quickcheck]
fn critic_quickcheck_parser_probe() {}
```

The whole `red_m87_registry` suite remained green (8 passed, exit 0). Thus an
unregistered scenario under a legal proc-macro test form can again be absent
from both sides of the claimed bijection. The documented promise is
fail-closed on an unrecognised test form; this is a direct continuation of
C-251 R6-2, not a new A-039 policy question.

### R7-2 — r6b covers one indentation shape, not assert_eq!/assert_ne! forms

`expects_snapshot()` removes an `assert_ne!` by searching for the literal
newline-plus-four-spaces `\n    );` (`red_m87_registry.rs:365-386`). This is not
a parser for the two claimed macro forms. A nested, otherwise ordinary
`assert_ne!` ending at eight spaces before a later `assert_eq!(..., "snapshot")`
makes it consume through the later four-space close and return false.

I changed r6b's positive fixture temporarily to that shape, with a file-change
check. `r6b_expectation_parser_distinguishes_assertion_from_its_negation` then
failed (exit 101): `разборщик не увидел ПОЛОЖИТЕЛЬНОГО ожидания снимка`.

The present limit is not named honestly. The text says the helper distinguishes
the two forms used in the corpus and only limits itself as “text, not types”.
It does not say that it accepts only a specific non-nested, four-space closing
layout. Consequently a valid layout of those same two forms gets a wrong
classification. Its own happy-path pair is insufficient anti-placebo evidence.

## Required arbitration question

This is the seventh circle around a construction intended to make a claim about
the entire test file. Decide the construction, not the threshold or policy:

1. Replace head/substring heuristics with a syntax/token-tree parser for outer
   attributes and assertion macro invocations, with an explicit allow-list for
   non-test attributes; every other outer attribute must reject until classified.
2. Make the AST/token parser's probes include `cfg_attr`, a test macro whose
   name does not contain `test`, and nested/multiline `assert_ne!` followed by
   a positive `assert_eq!`. Each must have a file-changed mutation and an
   observed red result.
3. Preserve the existing independent scenario-body assertion as the source
   checked against the registry. Do not “solve” the parser by taking the
   expected result from the registry too: that would recreate the declaration
   beside the fact that C-251 and A-039 rejected.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-87-serving-circuit-breaker
135ba1b9a7d512195be370f38ed10e78ec597312
exit=0

$ cargo test -p gateway-serve --test red_m87_registry
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ mutate c1 missing row -> Freshness { tail: 0, served: true }
$ git diff --numstat -- crates/gateway-serve/tests/m87_registry/mod.rs
3 3 crates/gateway-serve/tests/m87_registry/mod.rs
$ cargo test -p gateway-serve --test red_m87_registry
test r6_request_class_matches_scenario_expectation ... FAILED
«c1_entry_cold_request_named_outcome_without_reading_journal» объявлен в реестре как ОБСЛУЖИВАЕМЫЙ, но его тело не ждёт снимка
exit=101

$ add #[cfg_attr(test, tokio::test)] over dummy scenario; run r0b
3 0 crates/gateway-serve/tests/red_m87_entrypoint.rs
test r0b_unrecognised_test_attribute_is_refused_not_ignored ... FAILED
похожие на тестовые, но не распознанные: ["#[cfg_attr(test, tokio::test)"]
exit=101

$ add #[quickcheck] over dummy scenario; run registry suite
3 0 crates/gateway-serve/tests/red_m87_entrypoint.rs
test result: ok. 8 passed; 0 failed
exit=0  # unexpected: unknown proc-macro test form was silently missed

$ mutate r6b with nested eight-space assert_ne! before positive assert_eq!
1 2 crates/gateway-serve/tests/red_m87_registry.rs
test r6b_expectation_parser_distinguishes_assertion_from_its_negation ... FAILED
разборщик не увидел ПОЛОЖИТЕЛЬНОГО ожидания снимка
exit=101

$ A-039 mutations, each after git diff --numstat confirmed the named file changed
(a) delete c9 row -> r1 + r3 FAILED; exit=101
(b) c9 tail 200 -> 2_000 -> r2 FAILED; exit=101
(c) MAX_TAIL_EVENTS 1_000 -> 150 -> r2 FAILED; exit=101
(d) EXPECTED_WARMUP_EVENTS 100 -> 300 -> r3 FAILED; exit=101
restore: cargo test -p gateway-serve --test red_m87_registry
test result: ok. 8 passed; 0 failed
exit=0

$ delete c9 row; bash scripts/verify_M-87.sh
FAIL  A-039: реестр общего порога КРАСЕН
сценарии файла НЕ НАЗВАНЫ в реестре: ["c9_four_positions_differ_and_stale_snapshot_does_not_stop_serving"]
VERDICT: FAIL (провалов: 10)
exit=1

$ git diff --exit-code -- crates/gateway-serve/tests/m87_registry/mod.rs crates/gateway-serve/tests/red_m87_registry.rs crates/gateway-serve/tests/red_m87_entrypoint.rs
exit=0

$ bash scripts/next_artifact_id.sh C
C-252
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-23T17:00Z
- Milestone: M-87-serving-circuit-breaker, round 7 (C-251 execution only)
- Статус: BLOCKED — ESCALATE
- HEAD: 135ba1b — test(M-87): C-251 — драйвер слепка ЗОВЁТСЯ; парсер отказывает на нераспознанной форме [architect]

## §B — Что я сделал
- Аудировал только закоммиченный набор C-251 на вершине 135ba1b и воспроизвёл обязательные мутации с контролем изменения файла.
- Подтвердил закрытие связи реестр→драйвер и воспроизвёл два оставшихся обхода текстовых парсеров.

## §C — Артефакты / результаты
- `research/critiques/C-252-m87-c251-execution.md`
- Done Block above: baseline registry exit=0; driver/cfg_attr/A-039 mutations exit=101; quickcheck bypass exit=0; acceptance deleted-row exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter`
- **Paste-ready промпт:**
  ```
  Fresh-context arbitration for M-87 round 7. Read A-039, C-251, C-252, commit 135ba1b,
  crates/gateway-serve/tests/{red_m87_registry.rs,m87_registry/mod.rs,red_m87_entrypoint.rs},
  and milestone §14.1quinquies-sexies. Decide only the repeated C-251 parser-construction
  issue: C-252 proves #[quickcheck] bypasses r0b and an indented nested assert_ne! makes
  r6b misclassify a later assert_eq!(..., "snapshot"). Do not reopen the A-039 threshold,
  anchor, boundary mutation, or post-task-12 tester duty. Write and push an A-NNN decision
  to origin/feat/M-87-serving-circuit-breaker.
  ```
- Push-статус: pending this critic verdict commit to origin/feat/M-87-serving-circuit-breaker
- ✅ кэш убран — `/tmp/hft-critic-m87-r7/target` удалён после commit, до push

## §E — Риски / открытые вопросы
- This is a repeated same-class parsing failure after C-251; gates.md §0 requires arbitration rather than a third self-fix loop.

=== END HANDOFF ===
