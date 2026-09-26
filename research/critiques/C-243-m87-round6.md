<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: 23a915a36cb93bed17e300683c78d0566cc11d04
verdict: REJECT
-->

# C-243 — M-87 serving circuit breaker, round 6: REJECT

## Scope and verdict

This round judges only C-242's two requested closures at
`23a915a36cb93bed17e300683c78d0566cc11d04`: R5-1 (bounded FIFO-latch
release) and R5-2 (the third request receives the freed slot). A-037's
transport/library boundary, R3-1, and R4-2 are not reopened.

**Verdict: REJECT.** R5-1 is closed. R5-2 remains an unexecutable positive
control: C4 leaves its FIFO in the journal directory after releasing A. A
correct journal reader cannot then produce C's required `snapshot`; it either
waits for another FIFO writer or rejects an undeclared foreign `.jrnl`
segment. The test goes green only if implementation bypasses that real path.

`gateway-serve` has no dedicated FA: `DESIGN` §22 records `GW-I` as
`0 / 13`. This audit applies `VB-I-10/11`, `OPS-I-10`, and
`PL-I-4/5/8`. A public request cannot become green by silently skipping a
journal segment; that violates the real-path oracle and fail-closed premise.

## Artifact set and gates

- `milestones/M-87-serving-circuit-breaker.md:99-200` provides the T2 forms,
  `Cancel`, `ServingSlots`, `bind_with_policy`, and `ServingCounters`
  (including `successes` and `slots_in_flight`).
- The task table binds task 5 to C4 and task 6 to external counters.
  `red_m87_entrypoint.rs` is the real WS entrypoint RED oracle;
  `red_m87_admission.rs` carries the pure policy form.
- `verify_M-87.sh` is a fail-counting, non-zero-on-FAIL gate with CI parity.
  Its pre-implementation baseline remains expected COMPILE-RED, exit 1.
- The audited range has no `contracts/**`, so Block-C is not triggered.
  `check_gate_meta.sh` passes for the stated base/head.

## R5-1 — PASS: release is bounded and names the cause

`release_latch` moves write-open into a detached thread and its main thread
waits only `recv_timeout(Duration::from_secs(10))`
(`red_m87_entrypoint.rs:445-469`). The limit starts independently of reader
arrival. The failure explicitly says the reader did not reach the latch and
the scenario did not occur; it is not a generic timeout.

No latch wait remains unbounded: the post-release `recv` is bounded by
`BUDGET` (`:217-221`) and the free-slot poll is 40 × 50 ms
(`:523-535`). If the helper writer remains blocked, the main test panics
after ten seconds; its dropped `JoinHandle` does not join the helper or keep
the failed Rust test binary alive. R5-1 is closed.

## R5-2 — FAIL: C's required snapshot is impossible on this fixture

C4 now correctly requires C's `type == "snapshot"` and
`successes > successes_before` (`red_m87_entrypoint.rs:537-557`). An
implementation that returns `not_ready`, `unsupported`, or another error
therefore fails. This necessary strengthening is not a runnable positive
control, however.

The fixture creates a FIFO named `segment-NNNNNNNN.jrnl` (`:420-432`).
`release_latch` opens a writer but writes zero bytes (`:445-457`), so A's
reader gets EOF while the FIFO entry remains. The real journal path then:

1. Enumerates every indexed `*.jrnl` and passes it to `classify_segment`
   (`crates/journal/src/segments.rs:1260-1266`).
2. Calls `read_magic_prefix`, whose `File::open` blocks on a FIFO with no
   writer (`:998-1007`).
3. If an empty writer has closed, gets no magic; the undeclared filename takes
   the legacy branch and is rejected by `foreign_err` (`:1046-1069`).

The first request deliberately must reach that FIFO to prove holding. It can
terminate after release and free its slot, but C uses the same directory
without removing or replacing the FIFO. Its correct path either blocks at the
next read-open or rejects the foreign segment. Neither produces `snapshot`
or increments success. C's bounded `recv` makes the test fail rather than
hang, but a correct implementation cannot make the test GREEN.

The POSIX probe in the Done Block reproduces this exact transition: an empty
writer releases the first reader, while a new reader on the retained FIFO has
no writer and blocks. It uses a specified one-second timeout, not a
host-duration oracle.

**Condition for return:** before C is sent, C4 must restore a known-valid,
unlatched journal path after A terminates and guard that restoration. Only then
can C's snapshot plus success counter prove slot issuance rather than require
fail-open treatment of the FIFO.

This is the second consecutive REJECT on named closure R5-2 (C-242, then
C-243). Under `gates.md` §0, the next step is a fresh-context arbiter, not a
third architect↔critic loop.

## Done Block

```text
$ git rev-parse HEAD; git merge-base origin/main HEAD
23a915a36cb93bed17e300683c78d0566cc11d04
d5163b5b35abbca204a8981e974bd6e5a97eb9de
exit=0

$ git diff --name-status 269b1d06d48a78e4440b5367bc2edd8b20ce9298..HEAD
M	crates/gateway-serve/tests/red_m87_entrypoint.rs
exit=0

$ bash scripts/next_artifact_id.sh C
C-243
exit=0

$ bash scripts/check_gate_meta.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de
── GATE-META: диапазон d5163b5b..HEAD, origin=a3ka/hft-platform
   якорь main-стороны НЕ применён (прод-форма merge-ref не подтверждена) — судится весь диапазон
NOTE  research/arbitration/A-037-m87-entrypoint-boundary.md: subject-lock открыт явным ALLOW-SUBJECT-CHANGE (аудит-след, НЕ доказательство — F-064-6): scripts/verify_M-87.sh

VERDICT: PASS — вердиктов проверено: 6, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m87-round6-target-1790083480 bash scripts/verify_M-87.sh
FAIL  task1: нет модуля admission.rs с пятью исходами ServingOutcome
PASS  task2: библиотечный оракул изъят; готовность судится в транспорте
FAIL  task1+3+4+7: red_m87_admission КРАСЕН — компиляция
error[E0432]: unresolved import `gateway_serve::admission`
error[E0432]: unresolved import `gateway_serve::metrics`
error[E0432]: unresolved import `gateway_serve::auth::key_material`
error: could not compile `gateway-serve` (test "red_m87_admission") due to 11 previous errors
FAIL  task2+3+5+6+8: red_m87_entrypoint КРАСЕН — компиляция
error[E0432]: unresolved import `gateway_serve::admission`
error[E0432]: unresolved import `gateway_serve::metrics`
error[E0432]: unresolved import `gateway_serve::server::bind_with_policy`
error: could not compile `gateway-serve` (test "red_m87_entrypoint") due to 3 previous errors
FAIL  task4: в ReadStats нет payload_bytes_read — бюджет по байтам мерить нечем
FAIL  task5: ограничителя параллелизма нет ни в одном файле crates/gateway-serve/src (найдено: 0)
FAIL  task6: нет metrics.rs с раздельными счётчиками отказов
FAIL  task7: key_material отсутствует либо зонд её не зовёт — трактовок по-прежнему две (TD-207)
FAIL  task8: свежести нет ни в одном файле выдачи (совпадений: 0)
FAIL  task9: нет пары лимитов уровня сервиса у: gateway-serve(0) recorder(0) gateway-checkpoint(0) — ключи внутри labels/environment не считаются
PASS  task10: решение с токеном записано по каждому из 17 файлов и по двум тестам D-4
PASS  D-1(а): библиотечный корпус тронут только добавлениями red_m87_*
PASS  D-1(б): библиотечный корпус зелен — passed=223 failed=0
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 11)
exit=1 (expected COMPILE-RED baseline)

$ FIFO retained-entry probe
first_reader_after_empty_writer_exit=0
next_reader_without_writer_exit=124
cleanup_exit=0
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-09-22T14:02Z
- Milestone: M-87-serving-circuit-breaker
- Статус: BLOCKED
- HEAD: 23a915a — test(M-87): C-242 R5-1/R5-2 — защёлка падает явно, слот доказан получением [architect]

## §B — Что я сделал

- Судил только R5-1/R5-2; A-037, R3-1 и R4-2 не переоткрывались.
- Подтвердил R5-1 и показал, что retained FIFO делает R5-2 неисполняемым на честном journal-path.

## §C — Артефакты / результаты

- `research/critiques/C-243-m87-round6.md`
- Done Block: `check_gate_meta` exit=0; expected COMPILE-RED baseline exit=1; FIFO probe exit=0 with next-reader exit=124.

## §D — Следующий агент + инвокация

- **Следующий агент:** `arbiter` (strong model, fresh context)
- **Paste-ready промпт:**
  ```
  Resolve the second consecutive R5-2 REJECT for M-87 on feat/M-87-serving-circuit-breaker. Read A-037, C-242, C-243, milestones/M-87-serving-circuit-breaker.md, crates/gateway-serve/tests/red_m87_entrypoint.rs:420-557, and crates/journal/src/segments.rs:998-1010,1023-1098,1260-1266. Decide the factual question: after C4 releases its zero-byte FIFO writer but leaves segment-NNNNNNNN.jrnl in place, can a correct journal path produce C's required snapshot, or does it block/reject the retained foreign segment? Preserve A-037's boundary, R3-1, and R4-2. Write and push an A-NNN verdict to the subject branch.
  ```
- Push-статус: verdict commit pending; this critic will push C-243 to `origin/feat/M-87-serving-circuit-breaker` in this turn.
- ✅ кэш убран: `/tmp/hft-critic-m87-round6-target-1790083480` will be removed after commit/push.

## §E — Риски / открытые вопросы

- BLOCKED by R5-2. This is a fixture/path fact, not a reopening of A-037's boundary.

=== END HANDOFF ===
