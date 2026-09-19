<!-- GATE-META
milestone: M-71
audited_repo: a3ka/hft-platform
audited_base: 3b496208a64edbf00a66b93986ff8529d0c93aa9
audited_head: e0c02accf97d46d69aa48c7eb1d5ba3c3641a927
verdict: NOTE
-->

# C-163 — M-71 egress cap: финальный plan-time gate A-022

## Verdict: NOTE

Плановый гейт M-71 закрыт в рамках, заранее заданных `A-022`: найденные остатки
не являются REJECT, а проверенная дельта `7d80cea..e0c02ac` не оставляет
клиентски-достижимого одиночного v1-сообщения свыше 2 000 000 Б без оракула.
Это **не** утверждение или выбор величины предела: величина остаётся решением founder.

## Предмет и комплект артефактов

Аудирован committed set от `3b49620` до `e0c02ac`; предмет этого круга — три коммита
после C-161: `7d80cea`, `15b560b`, `e0c02ac`.

| Требование critic | Предъявленный committed artifact | Результат |
|---|---|---|
| T-contracts / trait signatures | Нового T1/T2 или trait signature дельта не вводит; существующие `gateway::Selector`, `ServeConfig`, `wire_v1::{snapshot_msg,frame_msg,error_msg}` являются достаточной границей теста. `crates/contracts/**` не затронут. | PASS |
| RED level 1 | `crates/gateway/tests/red_egress_cap.rs`, `red_egress_cap_boundary.rs` | PASS: девять oracle cases, ожидаемо RED до реализации |
| RED level 2 | `crates/gateway-serve/tests/red_egress_cap_wire.rs` | PASS: семь cases, реальные websocket bytes |
| Startup RED | `crates/gateway-serve/tests/red_egress_cap_startup.rs` | PASS: восемь invalid-limit RED + два vantage |
| Verify | `scripts/verify_M-71.sh` | PASS как gate: явный агрегатор `FAIL`, CI parity, состав cases и ожидаемый `FAIL (8)` |
| Door probe | `scripts/tests/red_egress_doors.sh` | PASS: L2 запрещает builders и требует socket markers |
| Milestone | `milestones/M-71-egress-cap.md`, rev5 §0quater/§7.1 | PASS: измерения, sensitivity table и A-022 limit записаны |

`FA-WAIVER: crates/gateway — dedicated FA file absent; VB-I-10 is the applicable bounded-output invariant.`

`FA-WAIVER: crates/gateway-serve — dedicated FA file absent; socket egress is audited via VB-I-10.`

Живой применимый инвариант: `docs/fa/viz-backend.md` `VB-I-10` (bounded
snapshot/frames). Также сверены `DESIGN` §16/§22 и `PL-I-4`/`PL-I-5`: отсутствующий
или неверный лимит не может означать unbounded transport.

## Исполнение: предел и достижимость

### §0quater воспроизведён

| Wire scenario | Снято с сокета | Ожидание §0quater | Итог |
|---|---:|---:|---|
| W1 v1 snapshot, 25 000 trades | 425 B | ≈425 B | совпало |
| W2 v1 frame after append | 29 454 B | ≈29 454 B | совпало |
| W3 legacy snapshot | 2 804 778 B | 2 804 778 B | совпало, ожидаемо RED |
| W4 huge-venue error | 2 100 084 B | 2 100 084 B | совпало, ожидаемо RED |

Независимая adversarial проба v1: 50 000 записей, `timeframe_ms=1`, bands
`[0.001, 0.01, 0.99]`, `window_ms` отсутствует, subscribe в grace window:
W1 = 432 B и W2 = 29 454 B. Отдельная проба с `window_ms=1`,
`timeframe_ms=86_400_000`, теми же bands и 50 000 записями дала W1 = 436 B.
Запоздалый на 300 ms subscribe не прошёл молча: W1 завершился
`SETUP НЕ СОСТОЯЛСЯ`, поскольку пришёл legacy snapshot, а не v1 snapshot.

Причина проверена и в коде, и socket execution: v1 subscribe создаёт `LiveReducer` с
cursor `START`, но первый snapshot снимается до первого `pump`; после этого push loop
вызвает `live.pump(..., PUSH_MAX_EVENTS=256)`. Поэтому плотный backlog приходит
серией кадров, а не одним v1 snapshot, и один frame ограничен packetization. При
проверенных вариациях не получен один клиентски-доставленный v1 message > 2 000 000 B.

### Наблюдаемость не вакуумна

Табличные две мутации воспроизведены в отдельном disposable worktree.

- `UnknownVenue(name) → "unknown venue"`: W4 стал GREEN (`exit=0`), W-C3 остался
  GREEN (`exit=0`). Значение W4 зависит от prod error path.
- Legacy serializer `ServeMsg::Snapshot(...) → {"Snapshot":{}}`: W3 стал GREEN
  (`exit=0`), W-C1 остался GREEN (`exit=0`). Значение W3 зависит от legacy path.
- Отсутствующая в таблице v1 мутация: к `Message::Text` v1 snapshot и frame sends
  добавлены 2 100 000 JSON-whitespace bytes. W1 стал RED на 2 100 425 B, W2 — RED
  на 2 123 014 B. Значит W1/W2 наблюдают production v1 egress, а не тестовую
  реконструкцию.

Серверная мутация «принять subscribe, но не послать snapshot» привела W1 к RED
`SETUP НЕ СОСТОЯЛСЯ ... последнее сообщение: None` (`exit=101`); отсутствие
сообщения не даёт зелёного. Это выполняет требование `subscribed_snapshot`.

### Door check и W5

Обратная проба не вакуумна. Вставка lexical builder-call
`wire_v1::snapshot_msg(` в L2 oracle дала `red_egress_doors.sh` `FAIL (1)`;
удаление всех `connect_async` tokens из L2 oracle также дало `FAIL (1)` с явным
«сокетного прогона нет». Первый мутант является именно lexical пробой, потому что
private builder недоступен integration test; проверяемое правило также lexical.

W5 честно судит **вердикт публичных wrapper functions** `serve::snapshot_msg` /
`serve::frames_msgs`, а не исходящий text. Это не скрытый wire path: socket paths
раздельно исполняются W1--W4. В текущем RED W5 получает `Ok` там, где level 1
должен вернуть refusal; его объект и предел явно названы в тесте и milestone.

## Число 2 000 000 B — evidence, не решение

При текущем поведении 2 000 000 B в 67.9 раза больше крупнейшего воспроизведённого
v1 frame (29 454 B) и в 4 705.9 раза больше v1 snapshot (425 B). Одновременно он
ловит измеренный legacy snapshot на 804 778 B сверх отсечки (1.402×) и error message
на 100 084 B сверх отсечки (1.050×). Следовательно, это технически различимая
рабочая отсечка при packetized v1, а не число, выведенное из v1 noise. Я не утверждаю
и не меняю величину: выбор 2 MB остаётся founder-owned Boundary C.

## NOTE residuals, ограниченные A-022

Не понижаю NOTE до REJECT и не открываю новый круг: `A-022` Question 4 заранее
классифицировал как advisory остатки length sub-id, macro/trait doors, proxy above
`N_MAX_BANDS`, output text outside `*_msg` builder и истинность таблицы
«door → scenario». L2 self-construction finding неприменима по построению: судимая
величина снята клиентом из `Message`, пришедшего через настоящий socket.

## Done Block

```text
$ git diff --check 3b496208a64edbf00a66b93986ff8529d0c93aa9..e0c02accf97d46d69aa48c7eb1d5ba3c3641a927
exit=0

$ bash scripts/verify_M-71.sh | grep -E '^(PASS|FAIL|VERDICT)'
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: cargo test -p gateway --test red_egress_cap --quiet
FAIL: cargo test -p gateway --test red_egress_cap_boundary --quiet
PASS: A состав набора — 9 оракулов
FAIL: cargo test -p gateway-serve --test red_egress_cap_wire --quiet
PASS: A2 состав набора — 7 оракулов
PASS: bash scripts/tests/red_egress_doors.sh
FAIL: cargo test -p gateway-serve --test red_egress_cap_startup --quiet
PASS: B состав набора — 10 оракулов
FAIL: C НЕ ГОТОВ — набор КРАСЕН и без мутации
FAIL: D GATEWAY_MAX_RESPONSE_BYTES объявлен в docker-compose.yml
PASS: cargo test -p gateway --test red_gateway_bounded --quiet
PASS: cargo test -p gateway --test red_snapshot_noclone --quiet
PASS: cargo test -p gateway --test red_gateway_live_eq_replay --quiet
PASS: cargo test -p gateway-serve --test red_max_subs_config --quiet
PASS: cargo test -p gateway-serve --test red_window_guard_startup --quiet
PASS: F crates/contracts не тронут
PASS: G GATEWAY_BANDS не тронут
PASS: H book/venue/journal не тронуты диапазоном
VERDICT: FAIL (8)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [7-RFC-PATH] путей-кандидатов ... всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m71-r5/target cargo test -p gateway-serve --test red_egress_cap_wire pl_i_5_w1_v1_snapshot_over_cap_is_not_delivered -- --nocapture
M71MEASURE W1 425
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m71-r5/target cargo test -p gateway-serve --test red_egress_cap_wire pl_i_5_w2_v1_frame_over_cap_is_not_delivered -- --nocapture
M71MEASURE W2 29454
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out
exit=0

$ socket adversarial variants (50k, wide bands, timeframe/window variations)
M71MEASURE V1_VARIANT attempt=1 n=436
M71MEASURE V1_VARIANT attempt=1 n=432
M71MEASURE W2_ADVERSARIAL 29454
all targeted tests: exit=0

$ late subscribe (+300ms)
SETUP НЕ СОСТОЯЛСЯ: v1-снапшот не получен за 8 попыток попасть в grace-окно
test result: FAILED
exit=101

$ observation mutations
UnknownVenue truncate: W4 exit=0; W-C3 exit=0
legacy snapshot truncate: W3 exit=0; W-C1 exit=0
v1 egress whitespace: W1 2100425 B RED exit=101; W2 2123014 B RED exit=101
silent server: SETUP НЕ СОСТОЯЛСЯ ... последнее сообщение: None; W1 RED exit=101

$ bash scripts/tests/red_egress_doors.sh (mutants)
builder-call mutant: FAIL: L2 cap-оракул зовёт строители исходящих форм 1 раз; exit=1
socket-call removal: FAIL: L2 оракул не содержит 'connect_async'; exit=1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-26T09:57Z
- Milestone: M-71-egress-cap
- Статус: DONE
- HEAD: e0c02ac — audited subject feat/M-71-egress-cap

## §B — Что я сделал
- Аудировал полный committed artifact set и финальную дельту после C-161 по рамкам A-021/A-022.
- Воспроизвёл socket measurements, adversarial v1 variants, silence, три observation mutations и reverse door checks.

## §C — Артефакты / результаты
- `research/critiques/C-163-m71-a022-final-gate.md` — NOTE; plan-time gate закрыт, residuals dispatchable.
- Done Block выше: verify ожидаемо `FAIL (8)`, design-claims merge-preview `PASS (0)`.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  На ветке feat/M-71-egress-cap прими C-163 как финальный NOTE plan-time gate M-71: внеси только mechanical appendix-ссылку на research/critiques/C-163-m71-a022-final-gate.md в milestones/M-71-egress-cap.md без изменения obligations, числа 2 MB или состава выдачи. Затем передай M-71 engine-dev для реализации существующих RED/oracle/verify artifacts. Не созывай новый арбитраж: A-022 Question 4 закрыл этот этап; Boundary C (величина лимита) остаётся founder-owned.
  ```
- Push-статус: ✅ pushed to origin/feat/M-71-egress-cap at `43b9840` (C-163 record).
- Кэш: ✅ убран (`rm -rf /tmp/hft-critic-m71-r5/target`; эквивалентно выполнен `git clean -fdX -- target`, вывод `Removing target/`).

## §E — Риски / открытые вопросы
- Founder должен отдельно принять или изменить числовую величину; этот verdict даёт только измеренное обоснование.
- NOTE residuals перечислены выше и по A-022 не требуют нового critic/arbitration round.

=== END HANDOFF ===
