<!-- GATE-META
milestone: M-87
audited_repo: a3ka/hft-platform
audited_base: dcef20c49383595aaf6a4f449d3da196203d764d
audited_head: fe4e7fb39b23f8b38aaa34e6a43e9aab199968b5
verdict: REJECT
-->

# C-253 — M-87 round 8: A-040 execution leaves r7 bypassable

## Verdict: REJECT

This audit is limited to the sufficient set in `A-040` §Q4. The attribute
parser is not reopened: its P1/P2/P3 form guards reject as required, and its
allowance of `#[cfg(feature = "testing")]` is not a false positive. The
blocking result is instead the separately required string↔function mechanism
in `A-040` §Q3(3)/§Q4 P6.

`GW-I-9(б)` remains the relevant live invariant: an unsuitable checkpoint on
the serving path produces a named outcome rather than silently rebuilding.
The registry proof feeds the fixtures for that invariant; if a scenario can
select a neighbour's registered row, the fixture no longer proves the
scenario's claimed checkpoint state.

## R8-1 — r7 accepts a nonliteral foreign driver argument (REJECT)

`r7_driver_literal_equals_scenario_name()` is meant to make the driver call a
mechanism rather than a convention. Its implementation at
`crates/gateway-serve/tests/red_m87_registry.rs:548-565` does not inspect the
first argument of a call. It searches forward from `serve_for(`/
`ckpt_from_registry(`/`registry_tail(` for the first quotation mark anywhere
in the remaining body and compares that later literal to the enclosing test
name.

I changed only the c1 scenario source, with `git diff --numstat` confirming
`3 1` on `red_m87_entrypoint.rs`:

```rust
let critic_r7_foreign = "u1_warm_path_is_served_over_trapped_head";
let (addr, _ckpt_guard) = serve_for(critic_r7_foreign, dir.path()).await;
let _critic_r7_decoy =
    "c1_entry_cold_request_named_outcome_without_reading_journal";
```

The call consequently builds the c1 fixture from the existing u1 row, while
the later decoy is the first string r7 observes. The complete registry suite
still passed: all 9 tests, including r7, exit 0. This is not a new parser-text
form and does not reopen the parser class frozen by `A-040`; it is a direct
bypass of the distinct P6 mechanism whose stated job is to bind the scenario
function to the registry row.

The direct wrong-literal P6 mutation is caught (exit 101), but that is not
sufficient: the claimed condition is a driver *argument* equal to the
enclosing scenario, whereas r7 only finds some later quoted text. The required
correction is a fail-closed r7 condition that accepts a driver occurrence only
when its first argument is the direct string literal equal to the enclosing
scenario name, plus this nonliteral/decoy mutation as a red control. A
nonliteral argument must not silently drop out of `checked`.

## A-040 §Q4 sufficient-set results

| Required item | Independent result |
|---|---|
| P1 quickcheck | changed file `3 0`; r0b rejected the out-of-whitelist head; exit 101 |
| P2 attribute + `fn` on one line | changed file `2 0`; r0b rejected it; exit 101 |
| P3 `macro_rules!` | changed file `2 0`; r0b rejected it; exit 101 |
| lawful `#[cfg(feature = "testing")]` | changed file `3 0`; r0b remained green; exit 0 |
| A-039 missing attribute-consumer guard | changed file `2 0` by adding a dangling `#[tokio::test]`; `attrs=16`, `found=15`; r0b failed, exit 101 |
| P6 foreign direct literal | changed file `1 1`; r7 rejected the neighbour literal; exit 101 |
| P4/P5 | r6b passed its nested and one-line negative fixtures plus a positive after each; exit 0 |
| A-039 mutations | delete c9 (`0 7`), c9 `200→2_000` (`1 1`), threshold `1_000→150` (`1 1`), anchor `100→300` (`1 1`): each made the registry suite red, exit 101; restored suite: 9 passed, exit 0 |
| A-040 harness bijection | verify contains the `--features testing` `-- --list` step. The command is red on the declared COMPILE-RED state (`AdmissionPolicy` fields and `slots_handle` absent), exit 101; `verify_M-87.sh` prints `FAIL A-040: перечень харнесса недоступен …` |
| task 12 and stated limits | task 12 carries the original boundary mutation plus the two A-040 duties (bijection green; c1 reclassification fails by fixture). §14.1quinquies-septies contains all six limits from A-040 §4. |

## Required condition for the next architect revision

Close R8-1 with a red control that changes a scenario to pass a foreign row
through a nonliteral argument while placing a same-name unrelated literal
later in its body. The guard must fail on that mutation and the restored
registry suite must pass. Keep the scope limited to r7/P6; no new parser-form
round is requested.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-87-serving-circuit-breaker
fe4e7fb39b23f8b38aaa34e6a43e9aab199968b5
exit=0

$ bash scripts/next_artifact_id.sh C
C-253
exit=0

$ cargo test -p gateway-serve --test red_m87_registry r0b_subject_stays_inside_the_parsing_contract -- --exact
P1 #[quickcheck]: FAILED; exit=101 (file changed 3 0)
P2 #[test] fn same-line: FAILED; exit=101 (file changed 2 0)
P3 macro_rules!: FAILED; exit=101 (file changed 2 0)
cfg(feature = "testing") control: ok; exit=0 (file changed 3 0)
dangling #[tokio::test] consumer control: FAILED; attrs=16 found=15; exit=101 (file changed 2 0)

$ cargo test -p gateway-serve --test red_m87_registry r7_driver_literal_equals_scenario_name -- --exact
P6 direct foreign literal: FAILED; exit=101 (file changed 1 1)

$ cargo test -p gateway-serve --test red_m87_registry
r7 nonliteral foreign argument + later same-name decoy: 9 passed; 0 failed
exit=0  # unexpected; R8-1
A-039 mutations: delete c9 / c9 200→2_000 / threshold 1_000→150 / anchor 100→300
each: registry suite FAILED; exit=101 (file-change controls: 0 7 / 1 1 / 1 1 / 1 1)
after restoration: 9 passed; 0 failed
exit=0

$ cargo test -p gateway-serve --test red_m87_registry r6b_expectation_parser_distinguishes_assertion_from_its_negation -- --exact
1 passed; 0 failed
exit=0

$ cargo test -p gateway-serve --features testing --test red_m87_entrypoint -- --list
error[E0599]: no method named `slots_handle` found
error[E0560]: AdmissionPolicy has no field `max_tail_events`
error[E0560]: AdmissionPolicy has no field `expected_warmup_events`
exit=101

$ bash scripts/verify_M-87.sh
FAIL  A-040: перечень харнесса недоступен — предмет не собирается (ожидаемо до задач 12-13)
VERDICT: FAIL
exit=1

$ git diff --exit-code -- crates/gateway-serve/tests/red_m87_entrypoint.rs crates/gateway-serve/tests/m87_registry/mod.rs
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-23T18:34Z
- Milestone: M-87-serving-circuit-breaker, round 8 (A-040 §Q4 only)
- Статус: BLOCKED — REJECT
- HEAD: fe4e7fb — test(M-87): A-040 — истина полноты у ХАРНЕССА; парсер инвертирован в белый список и заморожен [architect]

## §B — Что я сделал
- Проверил каждый пункт перечня достаточности A-040 §Q4 отдельным прогоном и контролем изменения файла, включая четыре мутации A-039 и возврат в зелёное состояние.
- Подтвердил, что A-039-страж потребления тестовых атрибутов работает, и воспроизвёл обход r7 через непрямой аргумент драйвера и последующий одноимённый decoy-литерал.

## §C — Артефакты / результаты
- `research/critiques/C-253-m87-a040-execution.md`
- Done Block выше: P1/P2/P3/P6 direct и A-039 mutations red; r6b and restored registry green; r7 nonliteral-decoy bypass green unexpectedly (exit 0).

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исполни только C-253 R8-1 на feat/M-87-serving-circuit-breaker. В r7 страж связи
  строка↔функция должен fail-closed проверять ПЕРВЫЙ аргумент каждого serve_for/
  ckpt_from_registry/registry_tail: он обязан быть прямым строковым литералом, равным
  имени охватывающего сценария. Добавь RED-контроль: c1 передаёт foreign имя через
  переменную, а после вызова стоит несвязанный одноимённый literal; r7 обязан краснеть.
  После возврата registry-suite зелёный. Не переоткрывай класс парсера, не меняй A-040
  threshold/anchor/mutations/limits. Закоммить и запушь; затем передай critic круг 9.
  ```
- Push-статус: ✅ pushed to origin/feat/M-87-serving-circuit-breaker at 7a3639c
- ✅ кэш убран — `/tmp/hft-critic-m87-r8/target` удалён после push

## §E — Риски / открытые вопросы
- R8-1 оставляет доказательство `GW-I-9(б)` зависимым от дисциплины автора, хотя A-040 требовал механическую связь строки и функции.
- Парсерные формы вне белого списка не являются находками этого круга по A-040 §Q4; они не оценивались как новые defect-классы.

=== END HANDOFF ===
