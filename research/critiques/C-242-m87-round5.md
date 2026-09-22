<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: f24bde85794fb86fe947ce9a07ec644c6853f0e7
verdict: REJECT
-->

# C-242 — M-87 serving circuit breaker, round 5: REJECT

## Scope and verdict

This is round 5 for `feat/M-87-serving-circuit-breaker`, judging the two
closures requested by `C-240` at
`f24bde85794fb86fe947ce9a07ec644c6853f0e7`.  `A-037`'s transport/library
boundary and the already closed R3-1 trap are not reopened.

**Verdict: REJECT.** R4-2 is closed. R4-1 has substantially improved—C4 now
uses a FIFO latch rather than a duration—but it still cannot honestly report a
failed latch setup, and it does not prove that the released slot is acquired by
the next request. Dev remains blocked.

`gateway-serve` has no FA. This audit applies `VB-I-10/11`, `OPS-I-8/10`, and
`DESIGN` `PL-I-4/5/8` as declared by M-87; in particular, `PL-I-4` forbids a
per-client journal scan and `OPS-I-10` requires a real producer oracle rather
than a constructed counter.

## Artifact-set and gate checks

The committed plan-time set is complete:

- `milestones/M-87-serving-circuit-breaker.md` specifies the five named
  outcomes, `CallBudget`, `BudgetStop`, `Cancel`, `ServingSlots`, and the six
  `ServingCounters` fields, plus allowed/forbidden paths and all ten tasks.
- `red_m87_admission.rs` compiles against the T2 form and trait signatures;
  `red_m87_entrypoint.rs` is the real WS entrypoint RED suite; and
  `red_ws_protocol.rs` carries the D-4 replacement for O-5.
- `scripts/verify_M-87.sh` is a fail-counting, non-zero-on-FAIL gate with CI
  parity, D-1(a)/(b), and task coverage. `contracts/**` is absent from the
  audited range, so Block-C is not triggered.

`check_gate_meta.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de` passes.  Its
subject-lock note names the meaningful `ALLOW-SUBJECT-CHANGE` reason: execution
of `A-037` U-8/D-1, not an unexplained change to the gate.

The expected pre-implementation baseline remains COMPILE-RED: admission,
metrics, `key_material`, and `bind_with_policy` do not exist yet. That red
state is expected and is not this verdict's finding.

## What is closed

### R4-2 — PASS: O-5 retains and uses the same connection

`open_and_first` returns the live `ProbeWs` together with the first message.
O-5 requires `not_ready|warming`, then writes a second `v:999` subscription to
that *same* stream and requires a second `type: error` frame. A server that
sends the first named outcome and closes immediately now fails either on the
write or on `ws.next()`. This closes `C-240` R4-2 and is consistent with the
session continuity rule in CT-RFC-09 §2.7.

## Blocking findings

### R5-1 — C4's FIFO setup guard can hang instead of declaring that the scenario did not occur

The new latch is a sound direction. `latch_segment` creates a FIFO named like a
segment; `segments_counted` classifies each segment and `read_magic_prefix`
opens it before the worker can continue. On this Linux host, a FIFO reader was
held until a writer opened it.

But C4 does not prove that its worker has become that reader. Its only pre-B
guard is `timeout(700 ms, a.next())`: no WS frame for 700 ms proves neither
that the FIFO was opened nor that the work is blocked there. More importantly,
`release_latch` synchronously opens the FIFO for writing without a timeout. A
writer with no reader blocks on this platform (`timeout 1 sh -c 'exec 3>fifo'`
returned 124). Therefore an implementation/platform where the segment FIFO is
ignored—or where the work is stuck before reaching it—can pass the negative
700-ms observation and then hang at release. It never produces the required
honest “setup did not hold work” test failure.

This violates the setup-failure half of `testing.md`'s gate-integrity property
3 and the explicit C-240 closure condition that the first work be *actually*
held by the test-controlled latch.

**Condition for return:** make release itself bounded and observable (for
example, a separately driven release whose completion is required within the
test's liveness budget). If no FIFO reader has been reached, C4 must fail with
the setup diagnostic instead of blocking the suite. The held-state assertion
must remain before B is sent.

### R5-2 — C4 observes a free counter but not issuance of that slot to C

After release, C4 first polls `slots_in_flight == 0`, which proves the counter
became free. It then sends canonical request C but accepts every response other
than `code: overloaded`:

```rust
assert_ne!(mc.get("code").and_then(|x| x.as_str()), Some("overloaded"));
```

That is not a proof that C acquired the released `ServingSlots` guard. A
`not_ready`, `unsupported`, generic error, or other pre-admission response
passes without ever attempting acquisition. This is especially material here:
the test leaves the released FIFO as a named journal segment, so a later
readiness/path error is a realistic alternative to slot acquisition.

**Condition for return:** after the observed release, demonstrate acquisition
on a known-ready, unlatched path—e.g. require the expected successful
`snapshot`, or use a second controlled hold and observe `slots_in_flight == 1`
for C. “Not overloaded” alone is insufficient for the required
“slot is issued to the next request” transition.

## Reproduced checks

| Check | Result | Evidence |
|---|---|---|
| C-240 R4-2 | PASS | O-5 retains `ProbeWs`, sends `v:999`, and requires a second error frame. |
| C-240 R4-1: exact B refusal while held | PASS | C4 checks `overloaded` and `slots_in_flight == 1` before its release call. |
| C-240 R4-1: honest setup failure | FAIL | no reader acknowledgement/bounded release; a no-reader FIFO writer blocks. |
| C-240 R4-1: slot issued to C | FAIL | C accepts anything except `overloaded`; it need not acquire a slot. |
| A-037 D-1(a) mutation | PASS | non-additive existing gateway test path is caught. |
| M-9a mutation | PASS | `cpus`/`memory` solely below `labels:` yields zero service-level limits. |
| M-10b mutation | PASS | an empty `smoke_ws.rs` decision cell is caught on the same table row. |
| Scope / Block-C | PASS | audited delta is architect-owned sacred tests, milestone journal, and critic artifact; no `contracts/**`. |

## Done Block

```text
$ git rev-parse HEAD; git merge-base origin/main HEAD
f24bde85794fb86fe947ce9a07ec644c6853f0e7
d5163b5b35abbca204a8981e974bd6e5a97eb9de
exit=0

$ git diff --name-status 52ddc43869b3f27429c902ca4d92f995e80f47a9..HEAD
M	crates/gateway-serve/tests/red_m87_entrypoint.rs
M	crates/gateway-serve/tests/red_ws_protocol.rs
M	milestones/M-87-serving-circuit-breaker.md
A	research/critiques/C-240-m87-round4.md
exit=0

$ bash scripts/check_gate_meta.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de
── GATE-META: диапазон d5163b5b..HEAD, origin=a3ka/hft-platform
NOTE  research/arbitration/A-037-m87-entrypoint-boundary.md: subject-lock открыт явным ALLOW-SUBJECT-CHANGE (аудит-след, НЕ доказательство — F-064-6): scripts/verify_M-87.sh
VERDICT: PASS — вердиктов проверено: 5, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m87-round5-target-1790080927 bash scripts/verify_M-87.sh
FAIL  task1: нет модуля admission.rs с пятью исходами ServingOutcome
PASS  task2: библиотечный оракул изъят; готовность судится в транспорте
FAIL  task1+3+4+7: red_m87_admission КРАСЕН — компиляция
FAIL  task2+3+5+6+8: red_m87_entrypoint КРАСЕН — компиляция
FAIL  task4: в ReadStats нет payload_bytes_read — бюджет по байтам мерить нечем
FAIL  task5: ограничителя параллелизма нет ни в одном файле crates/gateway-serve/src (найдено: 0)
FAIL  task6: нет metrics.rs с раздельными счётчиками отказов
FAIL  task7: key_material отсутствует либо зонд её не зовёт — трактовок по-прежнему две (TD-207)
FAIL  task8: свежести нет ни в одном файле выдачи (совпадений: 0)
FAIL  task9: нет пары лимитов уровня сервиса у: gateway-serve(0) recorder(0) gateway-checkpoint(0) — ключи внутри labels/environment не считаются
PASS  task10: решение с токеном записано по каждому из 17 файлов и по двум тестам D-4
PASS  D-1(а): библиотечный корпус тронут только добавлениями red_m87_*
VERDICT: FAIL (expected COMPILE-RED baseline)
exit=1

$ bash scripts/next_artifact_id.sh C
C-242
exit=0

$ M-10b same-row predicate replay (empty smoke_ws.rs decision)
M-10b=CAUGHT_missing_same_row_token
exit=0

$ D-1(a) predicate replay (non-additive red_frames_seek_bound.rs)
D-1(a)=CAUGHT
M	crates/gateway/tests/red_frames_seek_bound.rs
exit=0

$ M-9a predicate replay (cpus/memory only under labels:)
gateway-serve=0
recorder=0
gateway-checkpoint=0
M-9a=CAUGHT_missing: gateway-serve(0) recorder(0) gateway-checkpoint(0)
exit=0

$ FIFO platform probes
fifo_reader=held_before_release
fifo_reader=released_exit=0
fifo_writer_without_reader_exit=124 (124 means blocked)
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-22T12:31Z
- Milestone: M-87-serving-circuit-breaker
- Статус: BLOCKED
- HEAD: f24bde8 — docs(M-87): журнал кругов гейта; снятие замка предмета после A-037 [architect]

## §B — Что я сделал
- Аудировал только закрытия R4-1/R4-2 и замок предмета на закоммиченном наборе; границу A-037 и R3-1 не переоткрывал.
- Проверил полный набор T2/trait/RED/verify/milestone, GATE-META, baseline и D-1(a)/M-9a/M-10b.

## §C — Артефакты / результаты
- `research/critiques/C-242-m87-round5.md`
- Done Block: `check_gate_meta` exit=0; expected COMPILE-RED baseline exit=1; D-1(a), M-9a and M-10b each caught.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь только R5-1 и R5-2 из research/critiques/C-242-m87-round5.md на feat/M-87-serving-circuit-breaker. Не переоткрывай границу A-037, R3-1 или закрытый R4-2. C4 обязан fail-closed объявлять setup несостоявшимся, если FIFO-reader не достигнут/защёлка не держит работу (не зависать на release), и после контролируемого release доказать, что следующий запрос РЕАЛЬНО получил освобождённый слот, а не только не получил overloaded. Commit+push и запроси новый critic-круг.
  ```
- Push-статус: ⏸ commit этого verdict-файла готовится в этом critic turn.
- ⏸ кэш оставлен до завершения commit/push; будет убран перед handoff.

## §E — Риски / открытые вопросы
- BLOCKED by R5-1 and R5-2: both are incomplete lifecycle proof required by C-240 R4-1, not a new arbitration cause.

=== END HANDOFF ===
