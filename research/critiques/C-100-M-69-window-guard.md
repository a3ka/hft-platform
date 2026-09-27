<!-- GATE-META
milestone: M-69
audited_repo: a3ka/hft-platform
audited_base: 10bc072c7e008bce3feee80013fb187e3436fd17
audited_head: 10753df25570e43b5a6209d4498e135878e04786
verdict: REJECT
-->

# C-100 — M-69 window guard: REJECT

## Verdict

**REJECT — dev не назначать.** Все четыре блокера `C-099` закрыты в закоммиченном
наборе `983338b..10753df`, а не только в тексте milestone. Но весь commit-chain не проходит
механический GATE-META-барьер: `C-099` содержит неканоническое поле `milestone`, поэтому PR
будет красным вне четырёх ожидаемых RED-шагов M-69.

Предмет остаётся MD-only read-path: RISK-BLOCK и RAW-гейт не применяются. T1-контракты,
`contracts/**` и wire-форма не менялись; `Selector` остаётся существующим T2-типом, его
публичные сигнатуры/traits не изменены. Живой инвариант: **VB-I-10**
(`docs/fa/viz-backend.md`) — `Some(W)` ограничивает окно, а `None` допустим только как
offline-представление; `PL-I-5` требует отказа вместо неограниченного режима на
невалидном лимите.

## Проверка B-1…B-4 из C-099

| Блокер | Замер | Результат |
|---|---|---|
| B-1 | `milestones/M-69-window-guard.md:137-151` содержит отдельные `## Allowed paths` и `**Forbidden paths**`: два `src/lib.rs` назначены engine-dev, factual-документ — architect, RED/verify/milestone — architect-only. | Закрыт по scope-guard. |
| B-2 | `offline_forms_still_start` в `red_window_guard_startup.rs:164-211` для unset, `""`, пробелов и `"0"` утверждает `cfg.selector.window_ms == None`; `valid_windows_start` закрепляет `Some(positive)`. На текущем коде "0" даёт `Some(0)`, поэтому RED честно красный. | Закрыт по форме, не только по `window_lo_time_s`. |
| B-3 | `docs/plans/gateway-ws-contract.md:132` описывает `None` только для offline-форм и `Err` для invalid; `:341` исправлен на `docker-compose.yml:139`; `:351-359` отделяет исторический дефект от действующей политики и называет оба RED-оракула. | Закрыт во всех трёх требовавшихся местах. |
| B-4 | Text-presence канарейка удалена. `validate_selector_itself_rejects_negative_window` (`red_window_selector_guard.rs:213-242`) напрямую вызывает централизованный precondition; парный тест проверяет допустимые `None`/`0`/positive. Изолированный мутант `let _ = &sel.window_ms;` без проверки уронил этот тест. | Закрыт поведенческим оракулом. |

### B-5 — `C-099` не проходит GATE-META

`research/critiques/C-099-M-69-window-guard.md:2` содержит
`milestone: M-69-window-guard`. Поле GATE-META принимает идентификатор предмета
`M-69`, а не filename/slug: реальная CI-форма
`EVENT_NAME=pull_request PR_BASE_SHA=10bc072… bash scripts/check_gate_meta.sh` вернула
`FAIL` для `C-099`. Это отдельный блокер от B-1…B-4: продуктовый RED ожидаемо красный,
но мета-гейт обязан быть зелёным уже на plan-time наборе.

`C-099` — существующий артефакт критика, поэтому этот круг его не редактирует. Требуется
корректный GATE-META у каждого verdict-файла, введённого цепочкой, до следующего аудита.

## Полнота plan-time набора

- Milestone: `milestones/M-69-window-guard.md`.
- T-контракт/trait-сигнатуры: новых нет и не требуется — проверка `git diff` подтверждает
  отсутствие `contracts/**`; изменение формы `Selector` прямо запрещено milestone.
- Sacred RED: `crates/gateway-serve/tests/red_window_guard_startup.rs` и
  `crates/gateway/tests/red_window_selector_guard.rs`, оба сознательно RED против вершины.
- Acceptance: `scripts/verify_M-69.sh` — явный FAIL-счётчик, `exit 1`, финальный
  `VERDICT`, базовая тройка CI, проверка каждой задачи и регрессии GW-I-10/M-37/VB-I-10.

## Done Block

```text
$ git rev-parse 10bc072 10753df
10bc072c7e008bce3feee80013fb187e3436fd17
10753df25570e43b5a6209d4498e135878e04786
exit=0

$ git log --oneline 10bc072..10753df
10753df fix(M-69): C-099 B-1..B-4 — Allowed paths, канонизация offline, синхронизация фактуры, прямой оракул [architect]
e38f800 docs(M-69): C-099 — plan-time REJECT [critic]
e555cb4 test(M-69): acceptance-гейт verify_M-69.sh — паритет с CI + анти-байпас канарейка [architect]
a073f8a test(M-69): RED-набор GW-I-14 — две точки гварда, честный RED [architect]
983338b docs(M-69): спека — GW-I-14 fail-closed разбор GATEWAY_WINDOW_MS (PL-I-5, R7) [architect]
exit=0

$ git diff --name-status 10bc072 10753df
A	crates/gateway-serve/tests/red_window_guard_startup.rs
A	crates/gateway/tests/red_window_selector_guard.rs
M	docs/plans/gateway-ws-contract.md
A	milestones/M-69-window-guard.md
A	research/critiques/C-099-M-69-window-guard.md
A	scripts/verify_M-69.sh
exit=0

$ git diff --name-only 10bc072 10753df -- contracts crates/contracts
(no output)
exit=0

$ git diff --check 10bc072 10753df
(no output)
exit=0

$ nl -ba docs/plans/gateway-ws-contract.md | sed -n '132p;341p;350,359p'
132 | `GATEWAY_WINDOW_MS` | `None` | `unset`/пусто/`"0"` → `None` (offline, канонизировано); **fail-closed гвард GW-I-14** (M-69): parse-error / переполнение `i64` / отрицательное → `Err` на старте; иначе `Some(положительное)` | `lib.rs:609-613` |
341 - Прод: `GATEWAY_WINDOW_MS=60000` (`docker-compose.yml:139`; замер на VPS 2026-08-18 подтверждает то же значение в живом контейнере) ⇒ ~60 бакетов при `timeframe_ms=1000`.
350 - вызыватель (`crates/gateway-serve/src/main.rs:21`).
351 - **Асимметрия, названная здесь 03.08, закрыта milestone'ом M-69 (`GW-I-14`).** Исторически
352 - `GATEWAY_WINDOW_MS` с невалидным числом молча давал `None` (`.parse::<i64>().ok()`), то есть
353 - опечатка в env возвращала прод в unbounded-режим БЕЗ отказа старта — в отличие от
354 - `GATEWAY_TIMEFRAME_MS`, fail-closed с M-47. Приглашение «RED-оракул может атаковать» принято:
355 - оракулы — `crates/gateway-serve/tests/red_window_guard_startup.rs` (старт прод-бинаря) и
356 - `crates/gateway/tests/red_window_selector_guard.rs` (`validate_selector`, анти-байпас для
357 - чекпоинтера/shared-tailer/research-cli). Действующая политика — строка `GATEWAY_WINDOW_MS`
358 - в таблице §1: offline выражается тремя формами (`unset`/пусто/`"0"`), всё прочее обязано быть
359 - корректным положительным `i64`, иначе отказ на старте (`PL-I-5`, `DESIGN.md:940`).
exit=0

$ cargo test -p gateway --test red_window_selector_guard validate_selector_itself_rejects_negative_window --quiet  # isolated inert-field mutant
running 1 test
validate_selector_itself_rejects_negative_window --- FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out
mutant_exit=101

$ bash scripts/verify_M-69.sh; status=$?; echo "verify_exit=$status"
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test -p gateway-serve --test red_window_guard_startup --quiet
FAIL: cargo test -p gateway --test red_window_selector_guard --quiet
PASS: bash -c grep -qE '`GATEWAY_WINDOW_MS`.*fail-closed.*GW-I-14' docs/plans/gateway-ws-contract.md
PASS: bash -c ! grep -qE '`GATEWAY_WINDOW_MS`.*graceful, НЕ ошибка' docs/plans/gateway-ws-contract.md
PASS: cargo test -p gateway --test red_timeframe_session_alignment --quiet
PASS: cargo test -p gateway-serve --test red_timeframe_guard_startup --quiet
PASS: cargo test -p gateway-serve --test red_serve_window_wiring --quiet
PASS: cargo test -p gateway --test red_gateway_window --quiet
FAIL: bash -c ! grep -q 'пусто/не парсится' crates/gateway-serve/src/lib.rs
PASS: bash -c grep -qE 'GATEWAY_WINDOW_MS:[[:space:]]*.*60000' docker-compose.yml
FAIL: cargo test --all --quiet
VERDICT: FAIL (4 проверок красных)
verify_exit=1

$ bash scripts/next_artifact_id.sh C
C-100
allocator_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=10bc072c7e008bce3feee80013fb187e3436fd17 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 10bc072..HEAD не ввёл второй носитель под занятым идентификатором
artifact_ids_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=10bc072c7e008bce3feee80013fb187e3436fd17 bash scripts/check_gate_meta.sh
── GATE-META: диапазон 10bc072c..HEAD, origin=a3ka/hft-platform
FAIL  research/critiques/C-099-M-69-window-guard.md: milestone «M-69-window-guard» не похож на идентификатор артефакта (КЛАСС-НОМЕР[буква])
gate_meta_exit=1
```

## Next step

После исправления B-5 нужна повторная проверка полного commit-chain. До неё RED-набор не
передаётся `engine-dev`.

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-18T09:18Z
- Milestone: M-69-window-guard
- Статус: BLOCKED — REJECT, B-5 GATE-META
- HEAD: 10753df — fix(M-69): C-099 B-1..B-4 [architect]

## §B — Что я сделал
- Проверил закоммиченный набор `10bc072..10753df`, B-1…B-4 из C-099 и CI-форму
  GATE-META-барьера.
- Прогнал полный RED acceptance и независимый инертный мутант B-4.

## §C — Артефакты / результаты
- `research/critiques/C-100-M-69-window-guard.md`
- Done Block: `verify_M-69.sh` → ожидаемый RED, exit=1; мутант B-4 → FAILED, exit=101;
  `check_gate_meta.sh` → unexpected FAIL, exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`, затем `engine-dev`
- **Paste-ready промпт:**
  ```
  M-69-window-guard: C-100 REJECT. B-1…B-4 закрыты, но полный chain красный в CI-форме
  check_gate_meta.sh: C-099 содержит milestone: M-69-window-guard вместо формата M-69.
  Не назначай engine-dev. Организуй исправление GATE-META существующего C-099 в пределах
  полномочий владельца verdict-артефакта, затем передай полный обновлённый commit-chain
  critic на повторный audit. Не меняй RED-тесты, verify, milestone, docs/plans, contracts,
  docker-compose или Cargo.toml в ответ на этот блокер.
  ```
- Push-статус: ⏸ verdict commit prepared; critic pushes to `origin/feat/M-69-window-guard` before handoff.
- Кэш: ⏸ оставлен до commit/push verdict; будет убран до handoff.

## §E — Риски / открытые вопросы
- RED ожидаемо красный до реализации; четыре красных шага перечислены в Done Block.
- B-5: GATE-META CI-барьер красный на существующем C-099; это блокирует продолжение.

=== END HANDOFF ===
