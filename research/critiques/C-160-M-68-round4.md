<!-- ПЕРЕНУМЕРОВАН architect'ом 2026-08-25: вынесен как `C-159`, но тот же номер в ту же
     минуту занял вердикт по M-71 (`research/critiques/C-159-M-71-egress-cap-rev3.md`,
     ветка `feat/M-71-egress-cap`, коммит на 69 секунд раньше). `gates.md` §12: идентификатор
     УНИКАЛЕН — два файла под одним номером есть нарушение независимо от того, об одном они
     предмете или о разных. Содержание вердикта не тронуто; изменены только идентификатор в
     имени файла и четыре его вхождения в тексте.
     ПРИЧИНА КОЛЛИЗИИ — МОЯ: я отправил два круга критика параллельно, дав обоим одинаковую
     инструкцию «взять номер механизмом», и не развёл резервы ДО диспетчеризации. CAS-резерв
     (`scripts/reserve_artifact_id.sh`) от этого защищает только пока носитель не приземлён;
     обе роли освободили резерв, отчитавшись «резерв снят», и окно открылось заново. -->

<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: fa2ada921a4779f6a6a8f10751415b73a38c28eb
verdict: REJECT
-->

# C-160 — M-68 rev3.1, круг 4: REJECT

## Вердикт

**REJECT — engine-dev не назначать.** C-156 F1 снят: независимый кандидат в
разрешённом crates/gateway/src/** проходит новые d6a/d6b, а раздельные мутации
доказывают, что два измерения различают фиктивный счётчик и проход на каждую
полосу. Но обязательный Task 9 (GATEWAY_SCHEMA_VERSION: 8 -> 9) не
сопровождается обновлением существующего sacred RED
crates/gateway/tests/red_gateway_schema_version.rs, который прибит к 8 в трёх
публичных путях. Честная реализация Task 9 неизбежно делает обязательный
cargo test --all красным. Это неполный committed artifact set, не работа
для engine-dev.

Судившиеся инварианты: **VB-I-2** (live == replay), **VB-I-5** (честный
provenance) и **VB-I-10** (ограниченный ресурс). Граница книги остаётся
**BK-I-4/BK-I-6**; C и RAW/T1 не открываются по A-018 §2.3.

## Предмет и полнота артефактов

Аудирован committed chain
3b496208a64edbf00a66b93986ff8529d0c93aa9..fa2ada921a4779f6a6a8f10751415b73a38c28eb,
а не один текст milestone.

| Обязательный артефакт | Результат |
|---|---|
| T-contract / signature | T1 не тронут по A-018; Task 8 фиксирует private all-bands anchor depth_from_book(&self, levels, mid, bands) -> (Vec<i64>, u64) и T3 ReadStats::depth_levels_visited. |
| RED | Есть red_depth_from_book.rs (9), red_depth_provenance_by_reach.rs и red_depth_recompute_cost.rs (d6a/d6b). Но sacred red_gateway_schema_version.rs не обновлён для 8 -> 9. |
| verify | scripts/verify_M-68.sh включает CI-тройку, B-мутацию, checks A--K и baseline с семью ожидаемыми RED. |
| milestone | milestones/M-68-depth-from-book.md rev3.1 включает Tasks 7--9 и лимит d6. |

Три дельта-коммита после C-156: c6e596e, beb9f58, fa2ada9.
git diff --check чист; verify_design_claims --merge-preview origin/main зелёный.
Продуктовый спор «каденция, не дальность» не переоткрывался.

## F1 — Task 9 не может пройти обязательный полный CI

Task 9 требует увеличить GATEWAY_SCHEMA_VERSION с 8 до 9. Однако
crates/gateway/tests/red_gateway_schema_version.rs остаётся вне дельты и
задаёт const EXPECTED_SCHEMA_VERSION: u32 = 8. Он независимо проверяет:

1. саму GATEWAY_SCHEMA_VERSION;
2. Snapshot.schema_version;
3. каждый live Frame.schema_version из frames_since.

Базовый sacred RED сейчас зелёный при 8. В независимом кандидате с честным
Task 9 все d6 проверки зелёные, но cargo test --all --quiet завершается 101:
именно эти три tests падают с left: 9, right: 8. verify_M-68.sh обязан
запускать полный CI как первый gate, поэтому acceptance и Task 9 несовместимы
уже на committed форме.

Это другой класс от C-156 F1: там был недостижимый/неразличающий resource
oracle. Здесь architect не обновил свой version oracle, а testing.md требует
делать это в том же committed наборе при смене контракта. Engine-dev не может
править sacred test, чтобы разблокировать bump.

**Условие снятия REJECT:** architect должен добавить в committed набор обновление
crates/gateway/tests/red_gateway_schema_version.rs, делающее 9 нормативным для
константы, Snapshot и live Frame, и отразить этот RED в Task 9 / acceptance и
rebaseline. После этого кандидат с Tasks 8--9 обязан проходить полный
cargo test --all; старый 8 обязан оставаться RED. Это architect-only изменение
теста, не задача dev.

## Проверка центральных вопросов

- **Достижимость d6:** кандидат с одним проходом по book.levels() прошёл
  d6a/d6b (2/2). Константный счётчик сделал d6a красным при зелёном d6b.
  Наивный проход на каждую полосу сделал d6b красным при зелёном d6a:
  51586 > 20048 * 1.5 (2.57x). Это настоящая двухосевая конструкция.
- **Якорь B:** C-M68-1 принимает все bands. Мутант, обнуляющий bands меньше
  0.60, дал FAIL именно d1 и d4 (exit=101); кандидат проходит 9/9. Шаг B
  различает честную реализацию и мутант.
- **Task 7:** оба rewritten основания provenance RED проверены в baseline:
  2 ожидаемых RED; gw_i_4_delta_only остаётся зелёным контролем VB-I-2.
  Отдельной находки нет.
- **Baseline:** verify_M-68.sh даёт ровно семь причин из §6bis: clippy,
  full tests, A, B, C, D и G; exit=1.
- **d6 setup guard:** named limit не скрывает прежнюю дыру: d6a валидирует
  измеритель, d6b — отсутствие умножения на bands. Stale T2 wording про
  depth_within, противоречащее all-bands helper, надо убрать при исправлении
  F1, но это не отдельный REJECT.

## Done Block

    $ git log --oneline 3b496208..fa2ada9  # delta after C-156
    fa2ada9 docs(M-68): спека rev3.1 — достижимость ресурсного контракта предъявлена ПРОГОНОМ [architect]
    beb9f58 feat(M-68): rev3.1 — якорь мутации принимает ВСЕ полосы разом; шаг B под новую сигнатуру [architect]
    c6e596e test(MD-I-8): rev3.1 — ресурсный контракт ЗАМЕНЁН по C-156 F1: d6 → d6a + d6b [architect]
    [exit=0]

    $ bash scripts/verify_M-68.sh
    PASS: cargo fmt --all -- --check
    FAIL: cargo clippy --all-targets --all-features -- -D warnings  # missing depth_levels_visited
    FAIL: cargo test --all --quiet                                  # missing depth_levels_visited
    FAIL: A red_depth_from_book                                     # 5 failed; 4 passed
    PASS: A contains 9 oracles
    FAIL: B anchor MUT-ANCHOR C-M68-1 missing
    FAIL: C red_depth_recompute_cost
    FAIL: D GATEWAY_SCHEMA_VERSION >= 9 (current 8)
    PASS: E bounded/noclone; F live==replay
    FAIL: G red_depth_provenance_by_reach                           # 2 failed; 7 passed
    PASS: H contracts; I GATEWAY_BANDS; J selector fingerprint; K scope
    VERDICT: FAIL (7)
    verify_exit=1

    $ temporary candidate: cargo test -p gateway --test red_depth_recompute_cost -- --nocapture
    running 2 tests
    test md_i8_d6b_cost_does_not_multiply_by_number_of_bands ... ok
    test md_i8_d6a_counter_actually_measures_visited_levels ... ok
    test result: ok. 2 passed; 0 failed
    d6_candidate_exit=0

    $ temporary constant-counter mutant
    d6b ... ok
    d6a ... FAILED  # deep 1250000 vs shallow 1250000
    [exit=101]

    $ temporary per-band-pass mutant
    d6a ... ok
    d6b ... FAILED  # seven=51586, one=20048, budget=30072 (2.57x)
    [exit=101]

    $ temporary C-M68-1 subset-bands mutant: cargo test -p gateway --test red_depth_from_book
    md_i8_d1_depth_series_follows_the_delta_tail_on_both_sides ... FAILED
    md_i8_d4_every_band_moves_not_only_the_far_one ... FAILED
    [exit=101]

    $ base: cargo test -p gateway --test red_gateway_schema_version -- --nocapture
    running 3 tests
    test schema_version_constant_matches_expected ... ok
    test frame_carries_expected_schema_version ... ok
    test snapshot_carries_expected_schema_version ... ok
    test result: ok. 3 passed; 0 failed
    schema_test_base_exit=0

    $ candidate with GATEWAY_SCHEMA_VERSION=9: cargo test --all --quiet
    schema_version_constant_matches_expected --- FAILED
    snapshot_carries_expected_schema_version --- FAILED
    frame_carries_expected_schema_version --- FAILED
    assertion left == right failed: GATEWAY_SCHEMA_VERSION обязан быть 8; left: 9, right: 8
    test result: FAILED. 0 passed; 3 failed
    candidate_full_ci_exit=101

    $ bash scripts/verify_design_claims.sh --merge-preview origin/main
    VERDICT: PASS (0 нарушений)
    [exit=0]

    $ git diff --check 3b496208..fa2ada9
    [exit=0]

    $ bash scripts/reserve_artifact_id.sh C
    C-160
    [exit=0]

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-08-25T23:09Z
- Milestone: M-68-depth-from-book
- Статус: BLOCKED (REJECT C-160 F1)
- Audited HEAD: fa2ada921a4779f6a6a8f10751415b73a38c28eb

## §B — Что проверено

- Аудирован committed rev3.1 artifact set: T/signature, RED, verify и milestone.
- Независимо воспроизведены достижимость d6a/d6b и оба различающих мутанта.
- Найден F1: честный 8 -> 9 ломает три не обновлённых sacred schema-version oracle в полном CI.

## §C — Следующий агент + paste-ready invocation

- **Следующий агент:** architect
- **Промпт:**

    M-68 rev3.1 заблокирован C-160 F1. В новом committed architect artifact set
    обнови sacred crates/gateway/tests/red_gateway_schema_version.rs одновременно
    с Task 9: он должен нормативно требовать schema version 9 для константы,
    Snapshot и live Frame. Укажи этот RED в Task 9/acceptance, обнови baseline
    verify и убери stale T2 wording про depth_within, противоречащий all-bands
    конструкции. Затем предъяви committed/pushed chain, где 8 остаётся RED против
    обновлённого oracle, а честный Task 8--9 кандидат проходит cargo test --all.
    Не поручай engine-dev менять sacred test и не переоткрывай A-018 cadence decision.

## §D — Риск

- Без исправления dev обязан выбрать между обязательным публичным bump и mandatory
  full CI; это известный процессный дефект версионного RED, не исполнимый план.

=== END HANDOFF ===
