<!-- GATE-META
milestone: M-69
audited_repo: a3ka/hft-platform
audited_base: 10bc072c7e008bce3feee80013fb187e3436fd17
audited_head: f3102f0d4febe2d1a929c9273ba9dc01fa351674
verdict: NOTE
-->

# C-132 — M-69 window guard, круг 4: NOTE

## Предмет и обязательная граница круга

Проверен закоммиченный набор `10bc072..f3102f0` на
`origin/feat/M-69-window-guard`, а не только milestone-текст. В нём есть milestone,
два sacred RED-оракула и реальный acceptance-скрипт. `contracts/**` и T1 не менялись;
`Selector` остаётся существующим T2-типом, поэтому новых trait-сигнатур, wire-формы или
contract-RFC не требуется.

Это ограниченный круг 4 по обязательному решению `A-014` §5 п.2. REJECT здесь допустим
только за незакрытие B-6/B-7/B-8, красный механический барьер диапазона либо предъявленное
исполнением ложное зелёное/красное на прод-форме. Ни одного такого основания не найдено.

Живой инвариант предмета — **VB-I-10** (`docs/fa/viz-backend.md` §5): при
`Selector.window_ms=Some(W)` память ограничена окном, а `None` — только легитимный
offline-режим. `GW-I-14` не меняет эту оконную арифметику: он закрывает входы, которые
сегодня превращают parse-error или отрицательное значение в unbounded.

## Verdict: NOTE — dev may proceed

### A-014 B-6 — закрыт

`f3102f0` меняет только документальный счётчик §22 `GW-I` с `12` на `13` и добавляет
`docs/DESIGN.md` в architect-only Allowed paths. `verify_design_claims.sh` зелёный и на
вершине, и на merge-preview с `origin/main`; следовательно, созданное набором RED-покрытие
больше не делает `design-claims` красным.

### A-014 B-7 — закрыт в предписанной форме

`docs/plans/gateway-ws-contract.md` теперь отделяет действующий дефект R7 от целевой,
ещё не реализованной политики M-69. Это соответствует исполняемому состоянию:
`gateway-serve` всё ещё использует `.parse::<i64>().ok()`, а `validate_selector` ещё не
проверяет `window_ms`. Позитивный и негативный grep task #5 остаются зелёными. Новый
харнесс, запрещающий фактуре опережать код, не требуется: `A-014` §2 прямо отклонил его.

### A-014 B-8 — закрыт

Milestone §Acceptance теперь исчерпывающе называет четыре допустимых plan-time RED-шага:
два новых RED-target, task #4 grep документации исходника и дублирующий их
`cargo test --all`. Исполнение `verify_M-69.sh` вернуло именно эти четыре FAIL; остальные
его шаги PASS. Скрипт — настоящий агрегирующий гейт: считает `FAIL`, печатает
`VERDICT: PASS|FAIL` и возвращает ненулевой код при наличии FAIL.

## Полнота и scope набора

- Milestone задаёт Allowed/Forbidden paths, запретный список и задачи #1–#5; addition
  `docs/DESIGN.md` принадлежит architect и явно разрешён.
- Sacred RED: `red_window_guard_startup.rs` воспроизводит parse-error → `None` на
  конфигурационном входе; `red_window_selector_guard.rs` воспроизводит принятие
  отрицательного окна во всех трёх публичных входах и прямом `validate_selector`.
  Оба честно RED против `f3102f0`; парные валидные/offline кейсы удерживают от заглушки
  «всегда Err».
- `scripts/verify_M-69.sh` покрывает каждую задачу, базовую тройку CI и соседние
  GW-I-10/M-37/VB-I-10 регрессии. Тестовая RED-краснота не является красным барьером
  plan-time: она исчерпывающе объявлена milestone до назначения dev.

## Механические барьеры

На диапазоне `10bc072..f3102f0` зелёны `check_gate_meta`, `check_artifact_ids`,
`check_docs_freeze` и `check_protected_artifacts`. `verify_design_claims.sh` зелёный и
на ветке, и с `--merge-preview origin/main`. Проверка T1-диффа пуста; `git diff --check`
также чист.

## Done Block

```text
$ bash scripts/verify_design_claims.sh
VERDICT: PASS (0 нарушений)
design_claims_branch_exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
design_claims_merge_preview_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=10bc072c7e008bce3feee80013fb187e3436fd17 bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 4, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
gate_meta_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=10bc072c7e008bce3feee80013fb187e3436fd17 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 10bc072..HEAD не ввёл второй носитель под занятым идентификатором
artifact_ids_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=10bc072c7e008bce3feee80013fb187e3436fd17 bash scripts/check_docs_freeze.sh
docs_freeze_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=10bc072c7e008bce3feee80013fb187e3436fd17 bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (10bc072..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
protected_artifacts_exit=0

$ bash scripts/verify_M-69.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test -p gateway-serve --test red_window_guard_startup --quiet
FAIL: cargo test -p gateway --test red_window_selector_guard --quiet
PASS: task #5 factual-document greps
PASS: GW-I-10 / M-37 / VB-I-10 regressions
FAIL: bash -c ! grep -q 'пусто/не парсится' crates/gateway-serve/src/lib.rs
PASS: production default canary
FAIL: cargo test --all --quiet
VERDICT: FAIL (4 проверок красных)
verify_exit=1

$ git diff --name-only 10bc072..f3102f0 -- contracts crates/contracts
(no output)
t1_diff_exit=0

$ git diff --check 10bc072..f3102f0
(no output)
diff_check_exit=0
```

=== HANDOFF: CRITIC → ENGINE-DEV ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-23T17:24Z
- Milestone: M-69-window-guard
- Статус: DONE — NOTE; plan-time gate passed under A-014 §5 п.2
- HEAD: f3102f0 — docs(M-69): A-014 шаг 1 — B-6/B-7/B-8 закрыты одним коммитом, нового харнесса ноль [architect]

## §B — Что я сделал
- Проверил полный committed artifact set и все verdict-файлы прошлых кругов.
- Проверил закрытие B-6/B-7/B-8 и механические барьеры ветки и merge-preview.

## §C — Артефакты / результаты
- `research/critiques/C-132-M-69-window-guard-r4.md`
- Done Block: plan-time RED в `verify_M-69.sh` — ровно 4 ожидаемых FAIL, exit=1; все механические барьеры — exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `engine-dev`
- **Paste-ready промпт:**
  ```
  M-69-window-guard, plan-time gate C-132 NOTE (A-014 §5 п.2) passed on f3102f0.
  Реализуй ТОЛЬКО задачи #1–#4 в crates/gateway-serve/src/lib.rs и crates/gateway/src/lib.rs:
  invalid/overflow/negative GATEWAY_WINDOW_MS -> named startup Err; unset/empty/0 ->
  canonical None; validate_selector rejects negative window_ms; update only its source
  doc-comment. Не меняй sacred RED, verify, milestone, docs, contracts, Docker/Cargo.
  Перед handoff прогони bash scripts/verify_M-69.sh и передай raw Done Block с exit-кодами.
  ```
- Push-статус: ✅ verdict will be pushed to `origin/feat/M-69-window-guard` with its commit.
- Кэш: ⏳ будет убран после push.

## §E — Риски / открытые вопросы
- До dev `verify_M-69.sh` обязан быть RED ровно в четырёх объявленных местах; это не merge-ready состояние.
- После GREEN architect обязан сделать предписанный A-014 §5 п.3 factual flip до PR-time.

=== END HANDOFF ===
