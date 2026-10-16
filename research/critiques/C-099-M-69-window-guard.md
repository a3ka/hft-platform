<!-- GATE-META
milestone: M-69
audited_repo: a3ka/hft-platform
audited_base: 10bc072c7e008bce3feee80013fb187e3436fd17
audited_head: e555cb40b857c1b28f7365c1bf56e34f838fed78
verdict: REJECT
-->

# C-099 — M-69 window guard: REJECT

> *Правка формы, не содержания (арбитраж `A-010` §D). 2026-08-18 architect исправил в шапке
> `GATE-META` поле `milestone`: было `M-69-window-guard`, стало `M-69` — канон требует формы
> `КЛАСС-НОМЕР[буква]` (`scripts/check_gate_meta.sh:216-226`), и прежнее значение роняло джоб
> `gate-meta`, блокируя цепочку предмета. **Содержание вердикта — находки, обоснования,
> условия снятия, Done Block — не изменено ни на символ**; диф правки вне шапки и этой сноски
> пуст. Значение сверено с текстом самого вердикта, обе ревизии (`10bc072`, `e555cb4`)
> существуют и предковы HEAD.*

## Предмет и граница аудита

Проверен закоммиченный набор `983338b → a073f8a → e555cb4` поверх
`10bc072`, а не только текст плана. В наборе есть milestone, два sacred RED-оракула
и `scripts/verify_M-69.sh`; T1-контракты и их RFC не затронуты. `Selector` остаётся
существующим T2-типом без изменения wire-формы/trait-сигнатур — для этой задачи отдельный
T2-тип или trait не требуется.

Предмет действительно read-only/MD-only: RISK-BLOCK и RAW-гейт не применяются. Живой
инвариант предмета — **VB-I-10** из `docs/fa/viz-backend.md`: `Some(W)` ограничивает
память окном, а `None` — только легитимный offline-режим. `PL-I-5` требует fail-closed
для отсутствующего или невалидного лимита. Поэтому `GATEWAY_WINDOW_MS` нельзя
неявно превратить в ещё одну форму `None`.

## Вердикт: REJECT

Dev не назначать до нового закоммиченного набора architect'а. Все пункты ниже относятся к
спеке/RED/verify до реализации, а не являются предложением dev самостоятельно менять
священные файлы.

### B-1 — milestone не задаёт Allowed paths

`milestones/M-69-window-guard.md` содержит зону и запретный список, но не содержит
раздела `Allowed paths`. Это неполный milestone-контракт: `docs/04-workflow.md` §2
требует Allowed/Forbidden paths, а critic обязан проверить scope против таблицы ролей.
Одних путей в колонке §Tasks недостаточно: не зафиксировано, что именно разрешено
engine-dev, и некуда законно добавить необходимую синхронизацию factual-документа
(B-3).

Условие снятия: architect добавляет явный `## Allowed paths` с разделением
`crates/gateway/src/lib.rs` и `crates/gateway-serve/src/lib.rs` (engine-dev), sacred
тестов/verify/milestone (architect-only) и, если B-3 принимается, конкретного
`docs/plans/gateway-ws-contract.md` (architect). Запретный список сохраняется отдельным
разделом.

### B-2 — RED не закрепляет требуемое `"0" -> None`

Task #1 требует: unset/пусто/`"0"` дают **`None`**. Но
`offline_forms_still_start` в `red_window_guard_startup.rs` проверяет только
`window_lo_time_s(...) == None`. На текущем коде `"0"` проходит в `Some(0)`, а
`window_lo_time_s` всё равно возвращает `None`; поэтому оракул уже зелёный против
реализации, нарушающей названную форму результата. Это также оставляет отдельный
`selector_fingerprint` для `Some(0)` вместо канонического offline-селектора.

Условие снятия: RED должен для unset, пустой/whitespace и `"0"` явно утверждать
`cfg.selector.window_ms == None`; положительные значения — `Some(positive)`. После
этого падение текущего `"0"`-пути предъявляется в честном RED Done Block.

### B-3 — после фикса останется ложным предметный factual-документ

`docs/plans/gateway-ws-contract.md:132` утверждает, что `GATEWAY_WINDOW_MS` при
`unset/пусто/не парсится` даёт `None` и «НЕ ошибка», а §4 того же документа повторяет,
что невалидное число молча даёт `None`. Документ не помечен историческим; он назван
«фактура для RED-оракулов» и найден reading-map поиском предшественников. M-69 меняет
эту семантику, но не включает документ в задачу или разрешённую зону; Task #4 правит
только doc-comment исходника.

Условие снятия: architect в том же plan-time наборе обновляет обе factual-записи
`docs/plans/gateway-ws-contract.md` (либо явно и обоснованно помечает их историческим
снимком) и добавляет этот путь в Allowed paths. Новая формулировка обязана различать
offline (`unset`/пусто/`0`) и parse/overflow/negative (`Err` на старте).

### B-4 — канарейка verify проверяет присутствие, не гвард

Шаг verify извлекает тело `validate_selector` и ищет строку `window_ms`. Он принимает
инертное `let _ = &sel.window_ms;` при безусловном `Ok(())`; воспроизведение ниже вернуло
exit 0. Поведенческие тесты трёх входов хорошо держат результат, но не доказывают, что
централизованный precondition живёт именно в `validate_selector`, как того требует
milestone и как это сделано для GW-I-10.

Условие снятия: добавить в library RED прямой вызов `validate_selector` с
`Some(-1)`/`Some(-60000)` (InvalidInput + `window_ms`) и парный допустимый набор. После
этого канарейку presence-only убрать либо заменить проверкой, которую этот мутант
действительно роняет. Функциональные тесты `snapshot`/`frames_since`/`replay` остаются
отдельным доказательством проводки.

## Положительные результаты

- RED-набор действительно давит на parse garbage, overflow, negative и три библиотечных
  входа; текущий код даёт ожидаемый RED: 9/14 и 4/6.
- Парный vantage покрывает offline и положительные значения; после B-2 он должен
  закрепить также канонизацию offline-форм.
- `verify_M-69.sh` использует явный FAIL-счётчик и `exit 1`, содержит базовую тройку CI,
  регрессии GW-I-10/M-37/VB-I-10 и финальный `VERDICT`.
- `GW-I-14` свободен на audited base: в `10bc072` заняты только номера 4–13.

## Done Block

```text
$ git log --oneline 10bc072..e555cb4
e555cb4 test(M-69): acceptance-гейт verify_M-69.sh — паритет с CI + анти-байпас канарейка [architect]
a073f8a test(M-69): RED-набор GW-I-14 — две точки гварда, честный RED [architect]
983338b docs(M-69): спека — GW-I-14 fail-closed разбор GATEWAY_WINDOW_MS (PL-I-5, R7) [architect]
exit=0

$ git diff --name-status 10bc072..e555cb4
A	crates/gateway-serve/tests/red_window_guard_startup.rs
A	crates/gateway/tests/red_window_selector_guard.rs
A	milestones/M-69-window-guard.md
A	scripts/verify_M-69.sh
exit=0

$ rg -n '^## (Allowed|Forbidden) paths|^## Allowed paths|^## Forbidden paths' milestones/M-69-window-guard.md
(no output)
exit=1

$ cargo test -p gateway-serve --test red_window_guard_startup offline_forms_still_start --quiet
running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out
exit=0

$ sed -n '736,746p' crates/gateway-serve/src/lib.rs
let window_ms: Option<i64> = match get("GATEWAY_WINDOW_MS") {
    None => None,
    Some(s) if s.trim().is_empty() => None,
    Some(s) => s.trim().parse::<i64>().ok(),
};
exit=0

$ printf '<inert validate_selector body>' | awk '/pub fn validate_selector/,/^}/' | sed 's://.*::' | grep -q 'window_ms'
exit=0

$ bash scripts/verify_M-69.sh
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test -p gateway-serve --test red_window_guard_startup --quiet
  test result: FAILED. 5 passed; 9 failed
FAIL: cargo test -p gateway --test red_window_selector_guard --quiet
  test result: FAILED. 2 passed; 4 failed
FAIL: bash -c awk ... validate_selector ... grep -q window_ms
PASS: cargo test -p gateway --test red_timeframe_session_alignment --quiet (8 passed)
PASS: cargo test -p gateway-serve --test red_timeframe_guard_startup --quiet (6 passed)
PASS: cargo test -p gateway-serve --test red_serve_window_wiring --quiet (3 passed)
PASS: cargo test -p gateway --test red_gateway_window --quiet (6 passed)
FAIL: bash -c ! grep -q 'пусто/не парсится' crates/gateway-serve/src/lib.rs
PASS: bash -c grep -qE 'GATEWAY_WINDOW_MS:[[:space:]]*.*60000' docker-compose.yml
FAIL: cargo test --all --quiet
VERDICT: FAIL (5 проверок красных)
exit=1

$ git grep -n -o 'GW-I-[0-9][0-9]*' 10bc072 -- crates docs milestones | sed -E 's/.*GW-I-//' | sort -n -u | tail -10
4
5
6
7
8
9
10
11
12
13
exit=0

$ bash scripts/next_artifact_id.sh C
C-099
exit=0
```

## Следующий круг

Architect создаёт новый committed artifact set на этой же ветке: исправляет B-1..B-4,
показывает RED именно против текущего кода для канонизации `0`, затем снова передаёт
полную commit-chain reference + milestone path на plan-time audit. Это первый REJECT по
этим причинам; арбитр пока не нужен.
