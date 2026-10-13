<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: e620661d62d4aa8080f8de6f1fd8d533eea55e46
verdict: ESCALATE
-->

# C-164 — M-68 rev3.2, круг 6: ESCALATE

## Вердикт

**ESCALATE — engine-dev не назначать до решения свежего арбитра.** Факты
закрытия `C-162 F1` неоспоримы, но сам C-162 закрыт не полностью: `e620661`
исправил прежний запрет на private all-bands helper, однако оставил два
нормативных остатка в §3 (`задача 6` вместо задачи 8). Он также объявляет,
что число нормы больше нигде не названо prose, хотя literal `9` всё ещё
присутствует в prose sacred-oracle четырежды помимо единственной константы.

Это второй подряд гейт по тому же классу несогласованных committed
артефактов (`C-162 F1`), поэтому нельзя выдать третий прямой REJECT и снова
направить architect на self-fix. Маршрут — арбитр со свежим контекстом по
`gates.md` §0 и профилю critic. Это не Граница C: состав записи, GW-I/GS-I,
состав выдачи, T1/RAW, П-017 A3 и решение `A-018` «каденция, не дальность»
не переоткрывались.

Живые проверенные инварианты: **VB-I-2** (live == replay), **VB-I-5**
(provenance описывает наблюдение, породившее число) и **VB-I-10** (bounded
window). Candidate сохраняет их контрольные RED-наборы зелёными.

## Полнота committed artifact set

Аудирован весь committed набор
`3b496208a64edbf00a66b93986ff8529d0c93aa9..e620661d62d4aa8080f8de6f1fd8d533eea55e46`,
а непосредственно предметный ответ C-162 — единственный последующий commit
`e620661`. В нём только
`milestones/M-68-depth-from-book.md` и
`crates/gateway/tests/red_gateway_schema_version.rs`; кода реализации нет.

| Обязательный артефакт | Результат |
|---|---|
| T-contract / signatures | Новых T1/T2 public signatures нет; private `depth_from_book(levels, mid, bands)` зафиксирован. `ReadStats::depth_levels_visited` — T3. Но §3 дважды назначает его задаче 6, хотя таблица и RED назначают задаче 8. |
| RED | d1–d8b (9), d6a/d6b (2), provenance (9) и sacred runtime assertions (3) содержательны. Три runtime assertions не тронуты e620. Sacred prose ложно утверждает, что единственное literal-число нормы осталось только в константе. |
| verify | Реальный FAIL-aggregator, CI-тройка, executed B mutation, C, D/D2 и по меньшей мере одна проверка на задачу. Baseline воспроизведён как `FAIL (8)`, exit 1. |
| milestone | `depth_within`/all-bands противоречие снято верно, но task-owner counter/budget остался `6` в двух местах. |
| Block-C / scope | `crates/contracts/**` диапазоном не тронут; M-68 остаётся в `gateway` + architect paths. |

## Факты для решения арбитра

### F1a — прежний конфликт helper снят, но T3 назначен несуществующему владельцу

Полный grep milestone показывает:

- `:224`: «**задача 6** добавляет поле `ReadStats::depth_levels_visited`»;
- `:243`: «бюджет **задачи 6** достигается конструкцией внутри
  `gateway::Reducer`»;
- `:291`: таблица §Tasks назначает это поле и d6a+d6b именно **задаче 8**;
- `:289`: задача 6 — checkpoint stale/replay, а не ресурсный счётчик.

Значит больше нет старого предписания «вызвать `OrderBook::depth_within`
по полосе»: e620 правильно фиксирует private `depth_from_book` для всех bands
и запрещает per-band `depth_within` на L2Delta. Но §3 — место, которое
должно снимать догадку dev о T-contract — всё ещё даёт другому task-owner
поля и бюджет. Это буквально не выполненная часть условия снятия C-162 F1
«привести §3 к таблице задач и §4: назвать counter задачей 8».

### F1b — «число названо один раз» опровергается тем же sacred файлом

В `red_gateway_schema_version.rs:20` e620 пишет: «Норма названа ОДИН раз — в
`EXPECTED_SCHEMA_VERSION`, и проза её не дублирует числом», а `:60` содержит
единственную intended norm `const EXPECTED_SCHEMA_VERSION: u32 = 9`.
Однако grep всего файла на `\b9\b` находит также prose `:32`, `:35`, `:37` и
`:55` (`8 → 9`, `bump 8→9`, `left: 9, right: 8`, `M-68: 8→9`). Это может быть
полезной историей, но тогда утверждение «проза её не дублирует числом» и
«теперь его просто нет» — неверное. Требование C-162 и этого раунда было
сильнее: prose должна ссылаться на константу, а число должно быть названо
ровно один раз.

Три executable assertions сохранены корректно: все сравнивают
`GATEWAY_SCHEMA_VERSION`, `Snapshot.schema_version` и `Frame.schema_version`
с `EXPECTED_SCHEMA_VERSION`; e620 не поменял ни одной их проверки. Их
сохранение не делает верным противоречащий им документирующий контракт.

## Проверенные положительные свойства

- Глобальный grep `crates/*/tests/**` не нашёл четвёртого fixed gateway-schema
  oracle, несовместимого с задачами 1–9. `red_gateway_export_v2` и
  checkpoint-test соотносят значение с `GATEWAY_SCHEMA_VERSION` динамически;
  d8 намеренно требует `>= 9`. Другие literal schema values относятся к
  независимым journal/contracts epoch fixtures.
- Честный временный candidate из разрешённого `crates/gateway/src/**` делает
  private all-bands one-pass helper, счётчик `ReadStats` и bump 9. Его M-68
  RED-наборы прошли: depth 9/9, provenance 9/9, d6 2/2, sacred 3/3.
  До C-160 этот же честный candidate с pre-C-160 sacred expected value 8
  падает на всех трёх assertions; после обновления expected value до 9 —
  проходит. Следовательно C-160 F1 по исполнимости действительно закрыт.
- d6a/d6b различают две независимые мутации: constant counter красит только
  d6a; реальный отдельный обход `levels` под каждую band красит только d6b.
  Ручная B-мутация в anchor красит d1 и d4.
- В candidate `depth_reach_*` снимается в том же
  `refresh_depth_from_book` после чисел, вызываемом и из L2Snapshot, и из
  L2Delta; оба provenance tests зелёные. Задача 7 этим не потеряла coverage.

## Что должен решить арбитр

1. Считать ли два сохранившихся `задача 6` в нормативном §3 невыполнением
   точного условия C-162 F1, когда §Tasks требует задачу 8.
2. Считать ли four prose literal `9` рядом с утверждением «их нет»
   невыполнением условия C-162 F1b «число нормы названо ровно раз», либо
   допускается более слабое толкование исторической prose.

До решения нельзя отправлять dev: либо арбитр подтверждает остатки и задаёт
architect минимальное norm-only исправление, либо его DECISION обосновывает,
почему фактический текст всё же удовлетворяет условию C-162. Новый вопрос о
каденции, coverage bands или форме выдачи сюда не добавляется.

## Done Block

    $ git rev-parse HEAD; git rev-parse origin/main; git merge-base HEAD origin/main
    e620661d62d4aa8080f8de6f1fd8d533eea55e46
    3b496208a64edbf00a66b93986ff8529d0c93aa9
    3b496208a64edbf00a66b93986ff8529d0c93aa9
    [exit=0]

    $ git diff --name-status 95c50e8..e620661
    M crates/gateway/tests/red_gateway_schema_version.rs
    M milestones/M-68-depth-from-book.md
    M research/critiques/C-162-M-68-round5.md
    [exit=0]

    $ rg -n -i 'задач[аи]? ?№? ?[0-9]+|задач[аи]? [0-9]+|\bd6\b|\bd6a\b|\bd6b\b|depth_levels_visited|depth_within' milestones/M-68-depth-from-book.md
    224:исключением: задача 6 добавляет поле `ReadStats::depth_levels_visited`
    232:по-полосный вызов `depth_within` на пути `L2Delta` ЗАПРЕЩЁН
    236:... задача 8 одновременно требовала ...
    243:... бюджет задачи 6 достигается конструкцией внутри `gateway::Reducer`
    289:| 6 | Чекпоинт со СТАРОЙ семантикой отвергается ... |
    291:| 8 | Счётчик посещённых уровней честен ... `d6a`+`d6b` ... |
    [exit=0]

    $ rg -n '\\b9\\b|EXPECTED_SCHEMA_VERSION|assert_eq!' crates/gateway/tests/red_gateway_schema_version.rs
    20://! Норма названа ОДИН раз — в `EXPECTED_SCHEMA_VERSION`, и проза её не дублирует числом.
    32://! ## M-68 ... 8 → 9 ...
    35://! ... bump 8→9 ...
    37://! ... (`left: 9, right: 8`) ...
    55:/// ... **M-68: 8→9** ...
    60:const EXPECTED_SCHEMA_VERSION: u32 = 9;
    110:    assert_eq!(
    132:    assert_eq!(
    [exit=0]

    $ temporary honest candidate: focused M-68 RED recheck
    cargo test -p gateway --test red_depth_from_book --quiet       # 9 passed
    cargo test -p gateway --test red_depth_provenance_by_reach --quiet # 9 passed
    cargo test -p gateway --test red_depth_recompute_cost --quiet # 2 passed
    cargo test -p gateway --test red_gateway_schema_version --quiet # 3 passed
    CANDIDATE_M68_FOCUSED_RECHECK_EXIT=0

    $ temporary pre-C-160 sacred oracle (EXPECTED_SCHEMA_VERSION=8) against honest v9 candidate
    schema_version_constant_matches_expected --- FAILED
    snapshot_carries_expected_schema_version --- FAILED
    frame_carries_expected_schema_version --- FAILED
    PRE_C160_ORACLE_WITH_HONEST_V9_CANDIDATE_EXIT=101

    $ temporary constant-counter mutant: cargo test -p gateway --test red_depth_recompute_cost --quiet
    md_i8_d6a_counter_actually_measures_visited_levels --- FAILED
    ... deep 25 against shallow 25 ...
    D6A_CONSTANT_COUNTER_MUTANT_EXIT=101
    # d6b passed in the same 2-test invocation

    $ temporary per-band-pass mutant: cargo test -p gateway --test red_depth_recompute_cost --quiet
    md_i8_d6b_cost_does_not_multiply_by_number_of_bands --- FAILED
    ... seven=140336, one=20048, budget=30072 ...
    D6B_PER_BAND_MUTANT_EXIT=101
    # d6a passed in the same 2-test invocation

    $ temporary manual C-M68-1 mutation at MUT-ANCHOR: cargo test -p gateway --test red_depth_from_book --quiet
    md_i8_d1_depth_series_follows_the_delta_tail_on_both_sides --- FAILED
    md_i8_d4_every_band_moves_not_only_the_far_one --- FAILED
    B_MANUAL_ANCHOR_MUTANT_EXIT=101

    $ bash scripts/verify_M-68.sh
    PASS: cargo fmt --all -- --check
    FAIL: cargo clippy --all-targets --all-features -- -D warnings
    FAIL: cargo test --all --quiet
    FAIL: cargo test -p gateway --test red_depth_from_book --quiet
    FAIL: B SETUP НЕ СОСТОЯЛСЯ — якоря мутации 'MUT-ANCHOR C-M68-1' в реализации НЕТ.
    FAIL: cargo test -p gateway --test red_depth_recompute_cost --quiet
    FAIL: D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)
    FAIL: cargo test -p gateway --test red_gateway_schema_version --quiet
    FAIL: cargo test -p gateway --test red_depth_provenance_by_reach --quiet
    VERDICT: FAIL (8)
    VERIFY_M68_EXIT=1

    $ bash scripts/verify_design_claims.sh --merge-preview origin/main
    VERDICT: PASS (0 нарушений)
    VERIFY_DESIGN_CLAIMS_EXIT=0

    $ cargo test -p journal --test red_floor_work_budget --quiet
    running 5 tests
    .....
    test result: ok. 5 passed; 0 failed; finished in 183.68s
    CANDIDATE_FLOOR_WORK_BUDGET_EXIT=0

    $ git diff --check 3b496208a64edbf00a66b93986ff8529d0c93aa9..e620661d62d4aa8080f8de6f1fd8d533eea55e46
    SUBJECT_DIFF_CHECK_EXIT=0

    $ find /tmp/hft-critic-m68-r6/target -depth -delete   # exact cache path; literal rm -rf rejected by environment
    9.8G /tmp/hft-critic-m68-r6/target
    TARGET_CACHE_REMOVED_EXIT=0

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-08-26T10:01:41Z
- Milestone: M-68-depth-from-book rev3.2, круг 6
- Статус: BLOCKED (ESCALATE после второго подряд C-162 F1 класса)
- Audited HEAD: e620661d62d4aa8080f8de6f1fd8d533eea55e46

## §B — Что проверено

- Проверен committed набор T-contract/signature, RED, verify и milestone, включая
  единственный ответный commit e620661, а не только текст handoff.
- Воспроизведены baseline FAIL(8), merge-preview PASS, honest candidate задач 8–9,
  обе d6-мутации и ручная B-мутация.
- Установлено, что прежняя helper-коллизия исправлена, но task-owner и literal-number
  условия C-162 остались неисполненными.

## §C — Артефакты / результаты

- `research/critiques/C-164-M-68-round6-escalate.md` — этот ESCALATE и raw Done Block.
- Нет fourth fixed gateway schema oracle; task 7 coverage снимается там же, где depth values.
- Full `cargo test --all` honest candidate был начат; все M-68 targets прошли до долгого
  unrelated `journal::red_floor_work_budget`. Его полный агрегат не объявляется зелёным
  без exit 0; сам оставшийся target затем прошёл отдельно за 183.68 s.

## §D — Следующий агент + инвокация

- **Следующий агент:** арбитр на сильной модели, со свежим контекстом.
- **Промпт:**

    Разреши узкий спор M-68 rev3.2 на HEAD e620661d62d4aa8080f8de6f1fd8d533eea55e46:
    выполнены ли буквально условия снятия C-162 F1. Прочитай A-018, C-162,
    C-164, milestones/M-68-depth-from-book.md §3/§4 и sacred oracle целиком.
    Измерь два факта: §3 ещё называет counter/budget задачей 6 при таблице task 8;
    sacred prose заявляет, что 9 названо только в EXPECTED_SCHEMA_VERSION, но содержит
    9 на строках 32/35/37/55. Не переоткрывай решение A-018 о каденции, Границу C,
    GW-I/GS-I, состав выдачи или П-017 A3. Запиши и закоммить DECISION A-NNN на
    subject branch: либо назови exact norm-only обязательства architect, либо объясни,
    почему literal conditions C-162 всё же выполнены. Dev до DECISION не назначать.

## §E — Риски / открытые вопросы

- Если два остатка сочтены достаточными, next architect commit остаётся строго norm-only:
  без implementation, contracts, bands и изменения формы выдачи.
- Если literal historical `9` допустимы, decision должен явно отменить или уточнить
  невозможное обещание самого e620 «проза её не дублирует числом», иначе следующий bump
  снова получит неисполняемый prose-contract.

=== END HANDOFF ===
