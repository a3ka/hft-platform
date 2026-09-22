<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: 52ddc43869b3f27429c902ca4d92f995e80f47a9
verdict: REJECT
-->

# C-240 — M-87 serving circuit breaker, round 4: REJECT

## Scope

This is the fourth critic round on `feat/M-87-serving-circuit-breaker`, at
`52ddc43869b3f27429c902ca4d92f995e80f47a9`.  It judges only the three
closures requested by `C-238`: R3-1, R3-2, and R3-3.  The transport-to-library
boundary decided by `A-037` is not reopened.

The committed artifact set is present: M-87 declares the T2 contract and its
trait signatures (`Cancel`, `ServingSlots`, the five serving outcomes and the
counter form); RED suites are committed in `red_m87_admission.rs` and
`red_m87_entrypoint.rs`; the amended `red_ws_protocol.rs` carries the D-4
replacement; and `scripts/verify_M-87.sh` is a real fail-counting gate with CI
parity.  `contracts/**` is untouched.  The post-C-238 delta changes only the
two sacred WS test files.

`gateway-serve` has no dedicated FA.  This audit uses `GW-I-9(б)` in
`docs/fa/viz-backend.md` (cache rebuild belongs to worker, while an unsuitable
checkpoint on the live path returns a named readiness outcome), `OPS-I-10`
(declared counter must be emitted by the producer), and `OPS-I-8` (silence is
not health), together with `DESIGN` `PL-I-4/5/8`.

**Verdict: REJECT.**  R3-1 is restored.  R3-2 and R3-3 are only partially
closed: both changed the asserted first outcome but still omit the required
controlled lifecycle proof.  Dev remains blocked.

## What is closed

### R3-1 — PASS: У-1 is again an entrypoint trap oracle

The requested mechanical check returns seven `u1_*` names and seven
`poison_segment(` occurrences.  The restored block contains the trap guard,
all four cold checkpoint states over the trap, the warm request over a
head-trapped journal, and the positive `journal_payload_bytes_read` control on
a served tail.  The guard calls `checkpoint::advance` on the same poisoned
directory and requires `Err`, so a setup that no longer traps cannot make the
absence assertions green.  This meets `A-037` У-1 and prevents the
self-reported-zero-counter placebo prohibited by `testing.md`.

## Blocking findings

### R4-1 — R3-2 remains incomplete: C4 has no test-controlled hold or release proof

`c4_slot_is_held_until_work_ends_not_until_response_timeout` replaces the old
`snapshot` alternative with exact `overloaded`, and it checks
`slots_in_flight == 1`.  Those two corrections are real.  But its only delay is
`journal_busy(20_000)` (line 415); after sending A (line 428), it sends B
immediately (line 430), samples an unsynchronised `>= 1` (lines 432–437), and
finally merely receives A's response (line 455).  No fixture latch proves that
the first work has entered and remains in flight when B is judged; no
test-controlled release occurs; no assertion observes the slot become free
after that release.

Therefore the oracle is still timing-dependent and cannot distinguish the
forbidden implementation that releases the slot on a waiting-response timeout
while work continues.  This is exactly the lifecycle that `A-037` У-6 and
`C-238` R3-2 require it to pin, not a new boundary question.

**Condition for return:** demonstrate the complete observable lifecycle:
first work is held until the test releases it, B receives only `overloaded` and
the counter is exactly one while held, then release is observed and the slot is
shown free after work completion.

### R4-2 — R3-3 remains incomplete: o5 proves one live response, not a live connection afterwards

The new `first_message` correctly distinguishes a missing/failed first receive
from a JSON message, and o5 now requires `not_ready` or `warming` instead of
only the absence of `snapshot`.  That fixes the first half of R3-3.

However `first_message` creates its own socket (line 165) and returns only a
JSON value; the socket is dropped on return.  The o5 oracle then never sends a
second request or receives a second response on that same connection
(lines 371–390).  A server that sends one named readiness error and immediately
closes the connection passes unchanged.  It therefore does not prove the
post-error connection usability required by `C-238` R3-3 and the session
continuity required by `A-037` D-4 / `CT-RFC-09` §2.7.

**Condition for return:** retain the same WS connection after the named
broken-checkpoint outcome and demonstrate a subsequent usable request/response
on it.  A first message alone is insufficient.

## Other gate checks

| Check | Result | Evidence |
|---|---|---|
| A-037 D-1(a) | PASS | Range to `origin/main` adds only `red_m87_*` under `crates/gateway/tests/`; an isolated modification of existing `red_frames_seek_bound.rs` made the guard fail. |
| A-037 D-1(b) | PASS | `verify_M-87.sh` aggregated the existing gateway corpus as `passed=223 failed=0`. |
| M-9a verifier mutation | PASS | `cpus`/`memory` under `labels:` for all three services still yielded zero service-level limits and a failing task 9 predicate. |
| M-10b verifier mutation | PASS | Retaining the `smoke_ws.rs` table row with an empty decision made task 10 fail. |
| Scope | PASS | The committed round-4 delta is only `crates/gateway-serve/tests/red_m87_entrypoint.rs` and `crates/gateway-serve/tests/red_ws_protocol.rs`, both architect-owned sacred tests. |

## Done Block

```text
$ git rev-parse HEAD; git merge-base origin/main HEAD
52ddc43869b3f27429c902ca4d92f995e80f47a9
d5163b5b35abbca204a8981e974bd6e5a97eb9de
exit=0

$ rg -n 'u1_[A-Za-z0-9_]+' crates/gateway-serve/tests/red_m87_entrypoint.rs | wc -l
7
$ rg -n 'poison_segment\(' crates/gateway-serve/tests/red_m87_entrypoint.rs | wc -l
7
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m87-round4-target-1790075090 bash scripts/verify_M-87.sh
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
SKIP  task9: ФАКТИЧЕСКИ применённые лимиты, запас для recorder'а и поведение НА лимите снимаются на проде (docker inspect) — шаг деплой-гейта §8; замер 2026-09-21 дал NanoCpus=0 Memory=0 при живом описании сервисов
PASS  task10: решение с токеном записано по каждому из 17 файлов и по двум тестам D-4
PASS  D-1(а): библиотечный корпус тронут только добавлениями red_m87_*
PASS  D-1(б): библиотечный корпус зелен — passed=223 failed=0
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 11)
exit=1  # expected compile-RED baseline before dev

$ isolated D-1(a): modify and locally commit existing red_frames_seek_bound.rs
M	crates/gateway/tests/red_frames_seek_bound.rs
D-1(a) guard result: FAIL as expected
exit=0

$ isolated M-9a: put cpus/memory only under labels: for all three services
gateway-serve=0
recorder=0
gateway-checkpoint=0
task9 mutation result: FAIL as expected; missing: gateway-serve(0) recorder(0) gateway-checkpoint(0)
exit=0

$ isolated M-10b: retain smoke_ws.rs row and empty its decision cell
task10 mutation result: FAIL as expected; missing: smoke_ws.rs
exit=0

$ cargo clean --target-dir /tmp/hft-critic-m87-round4-target-1790075090
Removed 20244 files, 7.6GiB total
critic_target_cache_removed
exit=0

$ bash scripts/next_artifact_id.sh C
C-240
exit=0
$ bash scripts/reserve_artifact_id.sh C
C-240
reserve: резерв C-240 взят
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-22T11:41Z
- Milestone: M-87-serving-circuit-breaker
- Статус: BLOCKED
- HEAD: 52ddc43 — test(M-87): C-238 R3-1/R3-2/R3-3 — блок У-1 ВОССТАНОВЛЕН, C4 детерминирован, o5 строг [architect]

## §B — Что я сделал
- Аудировал закоммиченный набор и только закрытия R3-1/R3-2/R3-3; граница A-037 не переоткрывалась.
- Воспроизвёл baseline, D-1(a), M-9a и M-10b; проверил ловушку и WS-оракулы по исходнику.

## §C — Артефакты / результаты
- `research/critiques/C-240-m87-round4.md`
- Done Block: baseline verify exit=1 (ожидаемый COMPILE-RED); D-1(a), M-9a и M-10b caught their mutations (each harness exit=0).

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь только R4-1 и R4-2 из research/critiques/C-240-m87-round4.md на feat/M-87-serving-circuit-breaker. R3-1 закрыт и граница A-037 НЕ переоткрывается. Нужны RED-доказательства полного lifecycle C4 (управляемое тестом удержание, exact overloaded и slots_in_flight==1, наблюдаемое освобождение) и o5, сохраняющий тот же WS после named broken-checkpoint outcome и предъявляющий последующую usable request/response. Commit+push и запроси следующий critic-круг.
  ```
- Push-статус: ✅ this verdict is committed and pushed to `origin/feat/M-87-serving-circuit-breaker` in the same critic turn.
- ✅ кэш убран: `cargo clean` removed the critic target cache (7.6GiB).

## §E — Риски / открытые вопросы
- BLOCKED by R4-1 and R4-2. These are unfinished execution of A-037/C-238, not a new arbitration cause.

=== END HANDOFF ===
