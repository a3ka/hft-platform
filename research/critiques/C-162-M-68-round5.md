<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: 90d6eb59383578b0b67e327d4119690ce4fa3afa
verdict: REJECT
-->

# C-162 — M-68 rev3.2, круг 5: REJECT

## Вердикт

**REJECT — engine-dev не назначать.** `C-160 F1` по исполнимости закрыт:
честный временный кандидат задач 8--9, включая sacred oracle, поднятый до
9, завершил `cargo test --all --quiet` с кодом 0. Но архитектор не довёл
до согласованного состояния обязательный набор, который должен направлять
этого dev.

В committed milestone одновременно сказано, что поле
`ReadStats::depth_levels_visited` добавляет **задача 6**, что точка проводки
-- существующий `book::OrderBook::depth_within`, который надо переиспользовать
как есть, и что dev не вправе изобретать свой путь; там же задача 8 требует
приватный all-bands `depth_from_book(&self, levels, mid, bands) ->
(Vec<i64>, u64)`. Эти предписания несовместимы. Проверенный честный кандидат
делает именно требуемый all-bands helper по `book.levels()`, а не вызов
`OrderBook::depth_within`.

Отдельно sacred RED исполняемо пиннит 9 в трёх публичных путях, но его
нормативная шапка и док-комментарии всё ещё утверждают, что эти же три пути
обязаны быть 7. Греп по всем `crates/*/tests/**` нашёл единственные literal
ожидания устаревшей gateway-версии именно в этом файле. Это тот же класс
«священный оракул/спека говорит не то число», который C-160 просил закрыть
всем набором, а не одной константой.

Судившиеся инварианты: **VB-I-2**, **VB-I-5**, **VB-I-10**. Решение
`A-018` «каденция, не дальность», T1/RAW и мораторий П-017 A3 не
переоткрывались; GW-I/GS-I, состав выдачи и TD-159/TD-161 не судились.

## Полнота committed artifact set

Аудирован именно chain
`3b496208a64edbf00a66b93986ff8529d0c93aa9..90d6eb59383578b0b67e327d4119690ce4fa3afa`,
включая `f625c39`, а не только текст плана.

| Обязательный артефакт | Результат |
|---|---|
| T-contract / signatures | T1/T2 public additions нет; private all-bands signature явно зафиксирована. Но §3 ошибочно называет её задачей 6 и запрещает её через `depth_within`; **F1**. |
| RED | `red_depth_from_book` содержит 9 oracle, d6a/d6b есть, provenance RED есть, sacred runtime-asserts требуют 9. Его собственные normative comments всё ещё требуют 7; **F1**. |
| verify | `verify_M-68.sh` содержит CI-тройку, исполняемую B-мутацию, C, D и новый D2; baseline фактически `FAIL (8)`, exit 1. |
| milestone | rev3.2 отражает C-160 в §4.2 и baseline, но не исправляет противоречащий §3. |

`git diff --check` для audit range чист. `verify_design_claims.sh --merge-preview
origin/main` зелёный. Номера не выделялись: C-162 был выдан заранее, allocator не вызывался.

## F1 — после C-160 остаётся взаимоисключающая нормативная спецификация

### F1a — T-contract велит делать противоположное задаче 8

`milestones/M-68-depth-from-book.md:221-227` предписывает, что
`ReadStats::depth_levels_visited` добавляет задача 6, называет
`book::OrderBook::depth_within` единственной точкой проводки и запрещает dev
изобретать свою. Но таблица задач относит счётчик к задаче 8
(`:278`), а `:306-316` фиксирует обязательный private helper, принимающий
**все** bands разом. All-bands helper по уже материализованным
`book.levels()` не является и не может быть «`OrderBook::depth_within`
переиспользован как есть».

Это не редакционная мелочь: §3 -- T-contract, прямо предназначенный снять
догадку dev. Следование его запрету делает обязательную сигнатуру задачи 8
нарушением; следование задаче 8 нарушает §3. C-160 в своём paste-ready
условии прямо потребовал убрать этот stale текст, а `f625c39` его не тронул.

### F1b — sacred test оставляет ложную норму 7 рядом с исполнимой нормой 9

`crates/gateway/tests/red_gateway_schema_version.rs:14-17` по-прежнему
говорит, что constant, `Snapshot` и live `Frame` «прибиты к 7»;
`:100`, `:110`, `:132-133` и `:152` повторяют 7 в описаниях и setup-guard.
В том же RED `EXPECTED_SCHEMA_VERSION: u32 = 9` (`:53`) и все три runtime
assert используют именно эту константу. Текст C-160/f625 утверждает, что
архитектор обновляет **все** свои oracle в том же committed наборе. Оставить
в sacred спецификации две разные нормы для одного контракта -- не такое
обновление.

### Условие снятия REJECT

Architect-only commit должен:

1. привести §3 к таблице задач и §4: назвать `depth_levels_visited` задачей
   8 и заменить запрет/`OrderBook::depth_within` на согласованное с
   обязательной private all-bands signature описание без расширения зоны
   `crates/book/**`;
2. обновить все перечисленные normative comments sacred test с 7 на 9 и
   нынешнее обоснование M-68, сохранив три runtime assert;
3. заново предъявить baseline `FAIL (8)` и честный кандидат задач 8--9 с
   зелёным полным CI.

Это исправление артефактов architect, не задача engine-dev и не повод менять
каденцию, bands или поверхность выдачи.

## Проверка обязательных вопросов

- **C-160 F1:** закрыт исполнением. В кандидате с Task 8 source и Task 9
  schema bump + updated sacred expected value полный workspace CI зелёный.
  Значит прежний блокер «9 невозможно совместить с full CI» не повторён.
- **Все test-факты:** grep всего `crates/*/tests/**` не нашёл исполняемых
  gateway ожиданий 8; нашёл stale 7 только в описании
  `red_gateway_schema_version.rs` (F1b). Остальные совпадения 7/8 -- история
  иных contract epochs, размеры фикстур или нерелевантные schema constants.
- **C-156 F1 / d6:** константный счётчик красит d6a (101), оставляя d6b
  зелёным (0); проход на каждую полосу оставляет d6a зелёным (0) и красит
  d6b (101). Оси по-прежнему различены.
- **Шаг B:** в присутствующий candidate anchor вручную внесена мутация
  C-M68-1 (узкие bands становятся нулевыми); она красит d1 и d4 по
  отдельности, оба с 101. Это различает кандидата и мутанта.
- **Задача 7:** candidate обновляет `depth_reach_*` из того же
  `refresh_depth_from_book` вызова в snapshot- и delta-ветке; d7/d7b входят
  в зелёный полный кандидат. Находки нет.

## Done Block

    $ git rev-parse HEAD; git merge-base HEAD origin/main; git rev-parse origin/main
    90d6eb59383578b0b67e327d4119690ce4fa3afa
    3b496208a64edbf00a66b93986ff8529d0c93aa9
    3b496208a64edbf00a66b93986ff8529d0c93aa9
    [exit=0]

    $ CARGO_TARGET_DIR=/tmp/hft-critic-m68-r5/target cargo test --all --quiet
      # temporary honest candidate: Task 8 source + Task 9 schema version/oracle = 9
    ... all workspace test targets passed ...
    CANDIDATE_CARGO_TEST_ALL_EXIT=0

    $ temporary constant-counter mutant: red_depth_recompute_cost d6a; d6b
    md_i8_d6a_counter_actually_measures_visited_levels --- FAILED
    MD-I-8 d6a: deep=50, shallow=50; counter does not measure visited levels
    D6A_CONSTANT_EXIT=101
    md_i8_d6b_cost_does_not_multiply_by_number_of_bands ... ok
    D6B_CONSTANT_EXIT=0

    $ temporary per-band-pass mutant: red_depth_recompute_cost d6a; d6b
    md_i8_d6a_counter_actually_measures_visited_levels ... ok
    D6A_PER_BAND_EXIT=0
    md_i8_d6b_cost_does_not_multiply_by_number_of_bands --- FAILED
    MD-I-8 d6b: seven=140336, one=20048, budget=30072
    D6B_PER_BAND_EXIT=101

    $ temporary C-M68-1 mutant at MUT-ANCHOR: red_depth_from_book d1; d4
    md_i8_d1_depth_series_follows_the_delta_tail_on_both_sides --- FAILED
    C_M68_1_D1_EXIT=101
    md_i8_d4_every_band_moves_not_only_the_far_one --- FAILED
    C_M68_1_D4_EXIT=101

    $ CARGO_TARGET_DIR=/tmp/hft-critic-m68-r5/target bash scripts/verify_M-68.sh
    PASS: cargo fmt --all -- --check
    FAIL: cargo clippy --all-targets --all-features -- -D warnings
    error[E0609]: no field depth_levels_visited on type gateway::ReadStats
    FAIL: cargo test --all --quiet
    FAIL: A red_depth_from_book (5 of 9 RED)
    FAIL: B SETUP — MUT-ANCHOR C-M68-1 absent before implementation
    FAIL: C red_depth_recompute_cost
    FAIL: D GATEWAY_SCHEMA_VERSION >= 9 (current 8)
    FAIL: red_gateway_schema_version (three paths expect 9)
    FAIL: G red_depth_provenance_by_reach (2 of 9 RED)
    PASS: E/F/H/I/J/K and A count
    VERDICT: FAIL (8)
    VERIFY_M68_EXIT=1

    $ bash scripts/verify_design_claims.sh --merge-preview origin/main
    VERDICT: PASS (0 нарушений)
    VERIFY_DESIGN_MERGE_PREVIEW_EXIT=0

    $ git diff --check 3b496208a64edbf00a66b93986ff8529d0c93aa9..90d6eb59383578b0b67e327d4119690ce4fa3afa
    SUBJECT_DIFF_CHECK_EXIT=0

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-08-26T08:17:06Z
- Milestone: M-68-depth-from-book rev3.2
- Статус: BLOCKED (REJECT C-162 F1)
- Audited HEAD: 90d6eb59383578b0b67e327d4119690ce4fa3afa

## §B — Что проверено

- Аудирован committed T-contract/signature, RED, verify, milestone и delta после C-160.
- Воспроизведён зелёный честный кандидат задач 8--9 для полного `cargo test --all`.
- Воспроизведены обе независимые d6-мутации и исполняемая C-M68-1 мутация.
- Найден F1: stale T2 запрещает обязательный Task-8 helper, а sacred RED оставляет
  ложную норму 7 рядом с исполнимой нормой 9.

## §C — Артефакты / результаты

- `research/critiques/C-162-M-68-round5.md` — REJECT и полный Done Block.
- `cargo test --all` честного кандидата задач 8--9: exit 0; две d6-мутации и
  C-M68-1: различены; baseline verify: exit 1, `FAIL (8)`; design merge-preview: exit 0.

## §D — Следующий агент + инвокация

- **Следующий агент:** architect
- **Промпт:**

    M-68 rev3.2 заблокирован C-162 F1. Сделай только architect-only committed
    согласование артефактов: в milestones/M-68-depth-from-book.md §3 замени stale
    «Task 6 / OrderBook::depth_within reuse / dev cannot invent» на описание,
    согласованное с Task 8 и фиксированной all-bands private signature; не меняй
    crates/book/**. В crates/gateway/tests/red_gateway_schema_version.rs обнови все
    normative comments и setup texts, всё ещё называющие 7, до нормы M-68=9,
    сохранив три runtime assert. Rebaseline verify_M-68 как FAIL(8), снова покажи
    честный кандидат задач 8--9 с cargo test --all=0. Не переоткрывай A-018 и не
    расширяй предмет на GW-I/GS-I, выдачу, TD-159/TD-161.

- Push-статус: ✅ C-162 опубликован в `origin/feat/M-68-rev3` начиная с `95c50e8`;
  этот scope-only format fix отправляется следующим fast-forward commit.
- Кэш: ✅ убран (`rm -r -- /tmp/hft-critic-m68-r5/target`,
  `TARGET_CACHE_REMOVED=1`; требуемый `rm -rf` был отвергнут средой из-за `-f`).

## §E — Риски / открытые вопросы

- Пока §3 и §4 требуют взаимоисключающие реализации, dev не получает единого T-contract;
  а следующая смена версии снова начнётся с sacred файла, чья документация объявляет другое
  значение нормы. Это повторный fail-open процессного класса, хотя runtime assertions уже верны.

=== END HANDOFF ===
