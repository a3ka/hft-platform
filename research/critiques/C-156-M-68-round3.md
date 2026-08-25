<!-- GATE-META
milestone: M-68
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: 4c13c0366f15d1c5bc2a94cc483bfbca091f4168
verdict: REJECT
-->

# C-156 — M-68 rev3, круг 3: REJECT

## Вердикт

**REJECT — engine-dev не назначать.** Новый набор действительно снял причины
`C-094`/`C-138`, но его обязательный ресурсный oracle `d6` требует от реализации
несовместимых свойств и сам не защищён от фиктивной метрики. Это не RED, который dev
может честно сделать GREEN в объявленной зоне: `crates/book/src/**` прямо запрещён, а
зафиксированный M-68 helper обязан вызывать существующий `OrderBook::depth_within`,
который обходит диапазон `BTreeMap`.

Живые инварианты, которыми судился набор: **VB-I-2** (live == replay, включая warm
resume), **VB-I-5** (честный provenance) и **VB-I-10** (ограниченное окно). Граница
книги остаётся в области **BK-I-4/BK-I-6**: M-68 не имеет права незаметно менять
семантику resync/книги ради ресурса.

## Предмет и полнота артефактов

Аудирован commit-chain
`3b496208a64edbf00a66b93986ff8529d0c93aa9..4c13c0366f15d1c5bc2a94cc483bfbca091f4168`,
а не один текст milestone.

| Обязательный артефакт | Результат |
|---|---|
| T1/T2 и trait signatures | T1 не затронут; T2 decision явный: переиспользуется `book::OrderBook::depth_within`, новая публичная сигнатура N/A; единственное новое поле `ReadStats::depth_levels_visited` названо T3. |
| RED | `red_depth_from_book.rs` (9 d-oracles), `red_depth_provenance_by_reach.rs` (два намеренно RED setup-guard) и compile-RED `red_depth_recompute_cost.rs` предъявлены. |
| Acceptance | `scripts/verify_M-68.sh` содержит CI-тройку, literal-count=9, исполняемую мутацию и H–K scope guards. |
| Milestone | rev3 в `milestones/M-68-depth-from-book.md`; Allowed/Forbidden paths соответствуют ролям. |

Шесть требований `A-018` §2.3 исполнены: cadence-objective, production-form RED,
перенос `d6`, реально исполняемая B-мутация с `cargo test --all`, дословно перенесённые
три gate-артефакта, baseline с семью поимёнными красными; `docs/09-roadmap-v2.md` вне
диапазона. `git diff --check` также чист.

## Блокер C-156-F1 — d6 обещает постоянную цену, но зафиксированный путь линейный по книге

`crates/book/src/lib.rs:199-212` реализует `OrderBook::depth_within` через
`BTreeMap::range(...).map(...).sum()`. Для `FAR_BAND=0.60` это обход всех уровней
стороны в пределах ±60 %. M-68 §4 и §3.1 одновременно фиксируют helper
`depth_from_book(&self, side, band) -> i64 { self.book.depth_within(side, band) }`,
требуют считать все полосы на каждой L2Delta и запрещают менять `crates/book/src/**`.

`d6` при этом объявляет обязательным `deep <= shallow * 4` для 10 против 400 уровней
и 24 одинаковых delta. Следовательно честное измерение числа посещённых уровней на
объявленном пути растёт с глубиной и нарушает oracle. Ни milestone, ни RED не задают
допустимую конструкцию в разрешённых путях, которая способна сохранить точные суммы
полос и этот бюджет.

Кроме того, `d6` доверяет новому полю `ReadStats::depth_levels_visited`: единственные
содержательные проверки — `a > 0` и `b <= a * 4`. Реализация, возвращающая постоянную
ненулевую величину, сделает d6 зелёным, не измерив ни один уровень. Независимый
anti-placebo/mutation control именно для счётчика отсутствует. Это нарушает
`testing.md` («оракул меряет обещанный ресурс», «падает против сломанного setup'а»)
и не позволяет считать d6 реальным гейтом VB-I-10.

**Условие снятия REJECT:** architect должен переписать ресурсный контракт так, чтобы
его заявленный бюджет был достижим в Allowed paths и измерялся различающим oracle; либо
явно расширить subject по процедуре owner'а. Нельзя отправлять dev выбирать между
нарушением обязательной спецификации и фиктивным счётчиком.

## Проверенные пункты арбитра и инвокации

- `A-018` §2.4(a): `cargo clippy -p gateway --test red_depth_from_book -- -D warnings`
  проходит (`exit=0`); д5-class больше не содержит constant assertion.
- `A-018` §2.4(b): `d8b` пишет checkpoint через `advance_to`, а setup guard требует
  `events_decoded < n_events`; изолированный прогон `d8b` зелёный (`exit=0`). Это
  отличает путь от старой тавтологии пустого каталога.
- `A-018` §2.4(c), `C-M68-1`: в временной копии с вручную добавленным anchor базовый
  кандидат прошёл 9/9. Мутант `band < 0.60 => 0` дал `d1` и `d4` FAIL (`exit=101`).
  Второй представитель класса — обнуление только `band == 0.03` — дал те же два FAIL
  (`exit=101`). Следовательно набор судит класс «подмножество полос», а не только один
  пример; B-скрипт действительно вносит именно такую мутацию и запускает тест.
- Анти-плацебо production-form: `Emitter::delta` сначала применяет изменения к
  `FixtureBook`, а `Emitter::snapshot` берёт `self.book.project()`. Таким образом
  реальная фикстура проецирует накопленную книгу. При намеренной подмене `project()` на
  пустой snapshot `d1` остановился на своём setup-guard (`exit=101`), а не дал ложный
  GREEN.
- Задача 7 не принята на веру: оба переписанных oracle provenance сейчас красны на
  собственных setup-guards (`2 failed; 7 passed`), потому что delta ещё не двигает
  `depth_series`; это ожидаемая RED форма. `d7` и `d7b` покрывают shrink и grow, а
  `red_gateway_live_eq_replay` остаётся зелёным контролем VB-I-2.
- §3.1 содержит требуемые запреты: `Snapshot::apply`/close semantics, heatmap и COB как
  независимый эталон, `VB-I-10`, selector fingerprint, `L2DELTA_CAPTURE_SYMBOLS` и
  раздельные точки чисел/охвата. По этому пункту отдельной находки нет.
- Граница C и Allowed paths соблюдены: диапазон не трогает `contracts`, book/venue/journal,
  `docker-compose.yml`/`GATEWAY_BANDS` или roadmap; H, I, J, K acceptance зелёные.

## Done Block

```text
$ git log --oneline 3b496208..4c13c03
4c13c03 feat(M-68): задача 11 — acceptance-гейт rev3: мутация ИСПОЛНЯЕТСЯ, паритет с CI целиком [architect]
b21a0cb test(MD-I-8): семантика метки — наблюдение, ПРОИЗВОДЯЩЕЕ числа; два оракула инвертированы [architect]
aad0d89 test(MD-I-8): RED rev3 — прод-форма фикстуры и дельта-хвост вместо синтетики [architect]
b47fa0a docs(M-68): спека rev3 — дефект в КАДЕНЦИИ, не в дальности; посылка rev2 снята [architect]
cbb0d7d docs(M-68): rev3 — аудит-трейл предмета перенесён на новую ветку ДОСЛОВНО [architect]
[exit=0]

$ git diff --name-only 3b496208..4c13c03
crates/gateway/tests/red_depth_from_book.rs
crates/gateway/tests/red_depth_provenance_by_reach.rs
crates/gateway/tests/red_depth_recompute_cost.rs
milestones/M-68-depth-from-book.md
research/arbitration/A-018-m68-cadence-not-reach.md
research/critiques/C-094-M-68.md
research/critiques/C-138-M-68-round2-escalate.md
scripts/verify_M-68.sh
[exit=0]

$ bash scripts/verify_M-68.sh
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
error[E0609]: no field `depth_levels_visited` on type `gateway::ReadStats`
FAIL: cargo test --all --quiet
FAIL: cargo test -p gateway --test red_depth_from_book --quiet  # 5 failed; 4 passed
PASS: A состав набора — 9 оракулов
FAIL: B SETUP НЕ СОСТОЯЛСЯ — якоря мутации 'MUT-ANCHOR C-M68-1' в реализации НЕТ.
FAIL: cargo test -p gateway --test red_depth_recompute_cost --quiet
FAIL: D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)
PASS: E red_gateway_bounded; red_snapshot_noclone
PASS: F red_gateway_live_eq_replay
FAIL: G red_depth_provenance_by_reach  # 2 failed; 7 passed
PASS: H crates/contracts не тронут
PASS: I GATEWAY_BANDS в docker-compose.yml не тронут
PASS: J selector_fingerprint не переписан
PASS: K book/venue/journal/роадмап не тронуты диапазоном
VERDICT: FAIL (7)
VERIFY_EXIT=1

$ cargo clippy -p gateway --test red_depth_from_book -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s)
[exit=0]

$ cargo test -p gateway --test red_depth_from_book md_i8_d8b_warm_resume_equals_full_replay_across_the_delta_tail --quiet
running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out
[exit=0]

$ temporary candidate with MUT-ANCHOR C-M68-1: cargo test -p gateway --test red_depth_from_book --quiet
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
[exit=0]

$ temporary C-M68-1 mutant (band < 0.60 => 0)
md_i8_d1_depth_series_follows_the_delta_tail_on_both_sides --- FAILED
md_i8_d4_every_band_moves_not_only_the_far_one --- FAILED
[exit=101]

$ temporary second subset mutant (band == 0.03 => 0)
md_i8_d1_depth_series_follows_the_delta_tail_on_both_sides --- FAILED
md_i8_d4_every_band_moves_not_only_the_far_one --- FAILED
[exit=101]

$ temporary non-projecting fixture: cargo test -p gateway --test red_depth_from_book md_i8_d1_depth_series_follows_the_delta_tail_on_both_sides --quiet
md_i8_d1_depth_series_follows_the_delta_tail_on_both_sides --- FAILED
SETUP НЕ СОСТОЯЛСЯ: лучшие цены сдвинулись
[exit=101]

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
design_claims_exit=0

$ git diff --check 3b496208..4c13c03
[exit=0]

$ git show -s --format='%H%n%s' refs/reserved-cache/C-156
a62e6f09fb50a5789ebefe8467d619e59b080078
reserve C-156 nous 2026-08-25T16:31:54Z Ubuntu-2404-noble-amd64-base 1373125 1afa7089-ea7d-403e-814a-e9cbb0d1ed6d
[exit=0]
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-25T16:37Z
- Milestone: M-68-depth-from-book
- Статус: BLOCKED (REJECT C-156)
- HEAD: 4c13c0366f15d1c5bc2a94cc483bfbca091f4168 — feat(M-68): задача 11 — acceptance-гейт rev3: мутация ИСПОЛНЯЕТСЯ, паритет с CI целиком [architect]

## §B — Что я сделал
- Аудировал committed rev3 artifact set и исполнением проверил обязательные пункты A-018 §2.3/§2.4.
- Нашёл F1: d6 требует бюджет, несовместимый с фиксированной точкой проводки, и не валидирует собственную метрику.

## §C — Артефакты / результаты
- research/critiques/C-156-M-68-round3.md
- Done Block выше; `verify_M-68.sh` ожидаемо RED, exit=1; `verify_design_claims --merge-preview` PASS, exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-68 rev3 заблокирован C-156 F1. Перепиши ТОЛЬКО plan artifact set так, чтобы ресурсный RED d6 задавал достижимый в Allowed paths контракт и различающим прогоном валидировал свою метрику; либо оформи расширение зоны через владельца. Не отправляй engine-dev выбирать между фиксированной `depth_within`-проводкой и невозможным budget. Добавь RED на саму находку, обнови verify/M-68 baseline и верни новый committed/pushed набор на fresh critic круг.
  ```
- Push-статус: ⏸ verdict commit follows on `feat/M-68-rev3`; dev is blocked by C-156.
- Кэш: ⏸ будет убран после commit/push verdict.

## §E — Риски / открытые вопросы
- F1 — ложный resource gate способен либо вынудить невозможную реализацию, либо принять постоянный фиктивный счётчик.

=== END HANDOFF ===
