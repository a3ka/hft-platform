<!-- GATE-META
milestone: M-77
audited_repo: a3ka/hft-platform
audited_base: 4e4eff8fd64ebe0627970c24fec2e4e8a94094d4
audited_head: d3b0d122cbb69650b170d5abc376552a8289d98e
verdict: NOTE
-->

# C-213 — M-77 B-1 follow-up: named GREEN-phase checks now fail closed

**Verdict: NOTE.** `C-212 B-1` is closed. The one-commit follow-up changes only
`scripts/verify_M-77.sh` and replaces every prior GREEN-phase decision that was based on a
whole test binary's exit code with a per-named-test three-outcome decision. Dev dispatch for
tasks 3–4 is not blocked by this follow-up.

## Scope audited

This is the prescribed diff-only follow-up under `M-77` §11, not a new review of the selected
resolution B, §6bis, the three RED suites, §2bis, or §13. The exact audited range is
`4e4eff8fd64ebe0627970c24fec2e4e8a94094d4..d3b0d122cbb69650b170d5abc376552a8289d98e`:
one commit and one changed file, `scripts/verify_M-77.sh`.

`VB-I-2` remains the relevant live invariant: a live-tail series is bit-identical to replay of
the same journal window (`docs/fa/viz-backend.md:199`). The artifact set judged by the prior
round remains committed at the audited head: the M-77 milestone declares §6bis trait
signatures, the three RED suites, and the acceptance script. This follow-up changes the
acceptance gate only.

## B-1 closure

`run_named` distinguishes `OK`, `FAILED`, and `VACUUM`; `VACUUM` covers zero executed tests,
including `#[ignore]`. The three former bare-exit consumers are now all closed:

- **T5 GREEN:** each of the seven subject tests plus the two controls is executed by name,
  recorded in `GREEN_OUTCOME`, and fails on either `FAILED` or `VACUUM`.
- **T7 GREEN:** reuses the named T5 outcomes for W2 and W3; neither a vacuum nor a failed
  outcome can satisfy task 2.
- **T7 RED:** still requires exact failed entries for both W2 and W3. Ignoring either leaves
  the count below two and fails the step.

The remaining test-result forms were checked as well: T2 and T8 call `run_named`; T5 RED
requires every member of the seven-name subject list to be present in the workspace failure
set and rejects foreign red tests; T4 selects the phase only, after which T5 is the named
fail-closed decision. There is no remaining outcome-bearing test run that accepts a bare
zero exit as a successful M-77 oracle.

## Executed checks

The committed head reproduces the intended RED phase. It has 13 PASS lines; the only failure
is the declared unimplemented task-3 T4, so the script returns 1 rather than masking RED:

```text
$ bash scripts/verify_M-77.sh
PASS  T0 все три набора на месте
PASS  T0 состав полон: 2 контроля + 1 сторож цены + 7 предметных
PASS  T1 оба набора исполняют resume+pump в ПРОД-ФОРМЕ (Р-2)
PASS  T2 контроль снимочного хвоста ЗЕЛЁН (исполнено 1)
PASS  T2 дискриминатор окна отказа ЗЕЛЁН (исполнено 1) — окно достижимо
PASS  T3 cargo fmt --all -- --check (exit=0)
PASS  T3 cargo clippy --all-targets --all-features -D warnings (exit=0)
INFO  RED-ФАЗА: cargo test --all exit=101 — задача 3 не исполнена, это ОЖИДАЕМО
PASS  T5 краснота локализована: ровно 7 предметных теста M-77, посторонних нет
FAIL  T4 задача 3 не исполнена — милестоун не закрыт (RED-фаза, см. INFO выше)
PASS  T6 запретные пути не тронуты (contracts / gateway-serve/src / docker-compose.yml)
PASS  T7 контракт развязки Б объявлен в спеке (§6bis, сигнатура названа) — присутствие
PASS  T7 RED-фаза: оба предметных теста окна доставки красны, дискриминатор зелён
PASS  T8 сторож цены на границе pump ЗЕЛЁН (исполнено 1) — работа тика не растёт с числом батчей
---
VERDICT: FAIL (1)
exit=1
```

The named map is complete, syntactically valid, and points every listed name at an existing
test function:

```text
$ bash -n scripts/verify_M-77.sh
$ <TEST_BIN source/function check>
TEST_BIN entries=10
red_m77_frame_book_continuity ... exists=true  # 6 entries
red_m77_delivery_window ... exists=true        # 3 entries
red_m77_pump_cost ... exists=true               # 1 entry
bash_n_exit=0
map_exit=0
```

## Mutation control — GREEN branch entered and rejected the vacuum

In a separate disposable worktree at the audited head, I marked all seven subject tests
ignored (including W2/W3). This makes the workspace suite truly GREEN, rather than faking
`SUITE_RC`. Cargo first reproduced the problematic historic shape for W2/W3:

```text
$ cargo test -p gateway --test red_m77_delivery_window -- --test-threads=1
test vb_i_2_w1_refusal_by_cap_is_reachable_and_signals_terminality ... ok
test vb_i_2_w2_client_equals_replay_after_refusals_are_retried ... ignored
test vb_i_2_w3_client_equals_replay_when_refusal_hits_a_batch_rollover ... ignored
test result: ok. 1 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
mutation_window_exit=0
```

The actual mutated acceptance run then entered T4 GREEN and failed nine times: T5 rejected
each ignored subject test by name, and T7 separately rejected W2 and W3. T2/T8 still passed,
so the mutation did not weaken the adjacent control, RED-localization, or cost checks.

```text
$ bash scripts/verify_M-77.sh
PASS  T4 cargo test --all ЗЕЛЁН (exit=0) — развязка внесена (задачи 2-3 исполнены)
FAIL  T5 'vb_i_2_client_depth_values_equal_replay_in_prod_steady_state' НЕ ИСПОЛНЯЛСЯ (исполнено 0) — вакуум, не успех
FAIL  T5 'vb_i_2_client_keeps_the_point_when_the_tail_delta_is_one_sided' НЕ ИСПОЛНЯЛСЯ (исполнено 0) — вакуум, не успех
FAIL  T5 'vb_i_2_client_equals_replay_across_a_resync_then_delta_tail' НЕ ИСПОЛНЯЛСЯ (исполнено 0) — вакуум, не успех
FAIL  T5 'vb_i_2_client_bundle_equals_replay_in_prod_steady_state' НЕ ИСПОЛНЯЛСЯ (исполнено 0) — вакуум, не успех
FAIL  T5 'vb_i_2_client_equals_replay_when_the_tick_spans_a_batch_rollover' НЕ ИСПОЛНЯЛСЯ (исполнено 0) — вакуум, не успех
FAIL  T5 'vb_i_2_w2_client_equals_replay_after_refusals_are_retried' НЕ ИСПОЛНЯЛСЯ (исполнено 0) — вакуум, не успех
FAIL  T5 'vb_i_2_w3_client_equals_replay_when_refusal_hits_a_batch_rollover' НЕ ИСПОЛНЯЛСЯ (исполнено 0) — вакуум, не успех
FAIL  T7 'vb_i_2_w2_client_equals_replay_after_refusals_are_retried' НЕ ИСПОЛНЯЛСЯ — контракт окна доставки не предъявлен (C-212 B-1)
FAIL  T7 'vb_i_2_w3_client_equals_replay_when_refusal_hits_a_batch_rollover' НЕ ИСПОЛНЯЛСЯ — контракт окна доставки не предъявлен (C-212 B-1)
PASS  T8 сторож цены на границе pump ЗЕЛЁН (исполнено 1) — работа тика не растёт с числом батчей
---
VERDICT: FAIL (9)
exit=1
```

The temporary worktree was removed after the run; its seven-line mutation was retained only
as `/tmp/m77-b1-ignore-mutation.patch` during this audit and is not part of the milestone.

## Done Block

```text
$ git diff --check 4e4eff8..d3b0d12
exit=0

$ git diff --name-status 4e4eff8..d3b0d12
M	scripts/verify_M-77.sh
exit=0

$ bash scripts/next_artifact_id.sh C
C-213
artifact_id_exit=0

$ bash scripts/verify_M-77.sh
VERDICT: FAIL (1)
exit=1

$ <full-GREEN ignore mutation> bash scripts/verify_M-77.sh
VERDICT: FAIL (9)
exit=1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-06T21:00Z
- Milestone: M-77-frame-book-continuity
- Статус: DONE — C-212 B-1 closed; NOTE
- HEAD: d3b0d12 — fix(M-77): C-212 B-1 — зелёная фаза проверяется ПОИМЁННО [architect]

## §B — Что я сделал
- Audited only `4e4eff8..d3b0d12`; no resolved M-77 scope was reopened.
- Reproduced the committed RED phase and a real GREEN-phase ignore mutation.
- Confirmed all ten TEST_BIN entries resolve to existing named test functions.

## §C — Артефакты / результаты
- `research/critiques/C-213-m77-b1-followup.md`
- Done Block: committed gate exit=1 (only declared T4 RED); full-GREEN ignore mutation exit=1 (T5/T7 fail closed).

## §D — Следующий агент + инвокация
- **Следующий агент:** `engine-dev`
- **Paste-ready промпт:**
  ```
  Ты — engine-dev hft-platform. Реализуй задачи 3–4 M-77-frame-book-continuity строго по
  milestones/M-77-frame-book-continuity.md §6bis и RED-наборам. Рабочая зона: только
  crates/gateway/src/lib.rs. Не меняй RED-тесты, verify, milestone, contracts,
  gateway-serve или docker-compose. После GREEN-коммита запусти bash scripts/verify_M-77.sh,
  приложи raw Done Block с exit-кодами и push на feat/M-77-frame-book-continuity.
  ```
- Push-статус: recorded with this verdict commit on `origin/feat/M-77-frame-book-continuity`.
- Кэш: ⏸ оставлен — shared audit target was used for the executed workspace gates.

## §E — Риски / открытые вопросы
- The milestone remains intentionally RED until engine-dev implements tasks 3–4; `verify` must stay FAIL in that state.
- Founder decision on the heatmap scope remains outside this diff-only follow-up.

=== END HANDOFF ===
