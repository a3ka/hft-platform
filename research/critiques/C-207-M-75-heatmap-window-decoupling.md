<!-- GATE-META
milestone: M-75
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: 94a105f2e439c5a33b49c58656bbe2889ff9fca9
verdict: NOTE
-->

# C-207 — M-75 heatmap-window-decoupling, круг 5: NOTE

## Verdict

**NOTE.** Plan-time gate пройден: новых ослаблений предмета не найдено, а
`C-201` B-6/B-7 и предписание `A-031` §1 п.1 закрыты committed-артефактами.
Это не разрешение на реализацию: tasks 2/3/4 намеренно RED до engine-dev, и
их будущий зелёный путь остаётся обязанностью `verify_M-75.sh`.

## Судимый набор и контракт

Проверен `origin/feat/M-75-heatmap-window-decoupling` на
`94a105f2e439c5a33b49c58656bbe2889ff9fca9`, merge-base с текущим
`origin/main` — `d77398d7b22396c452d2651e90498033186055dd`.

Набор содержит T2-обязательства и literal signatures из milestone §5, четыре
RED-target'а (`red_heatmap_window_decoupled`,
`red_heatmap_window_server_owned`, `red_heatmap_window_env`,
`red_heatmap_window_effective_setting`), `scripts/verify_M-75.sh` и сам
milestone. T1/RFC/schema/GATEWAY_BANDS здесь не требуются и не меняются.
Применены **VB-I-10** (bounded-window snapshot) и **MD-I-8** (не терять
legacy witnesses) из единственного применимого FA `docs/fa/viz-backend.md`.

## C-201 B-6/B-7

`verify_M-75.sh` больше не принимает task 2b по именам: после появления
setter он исполняет каждый затронутый внешний oracle, требует реальный вызов
setter и сопоставляет число `serial()` guards с числом test functions в каждом
process-global файле. До task 2 это честный RED именно по отсутствию setter,
а не вакуумная зелень.

В независимом rescue-прогоне `e462c163` (цепочка `m75-b5-inventory-r2` →
`m75-b6-serial`) четыре набора legacy-witnesses дали 9/9, 9/9, 4/4 и 9/9
GREEN; H-6 дал 2/2 GREEN. Единственный RED — ожидаемый compose oracle task 4
(4 passed, 1 failed). Тем самым будущий порядок task 2 → architect task 2b
воспроизводим и не оставляет гонку process-global окна в известной группе.

## A-031 §1 п.1: presence guards

Шапка `verify_M-75.sh` перечисляет группу guards, предмет наблюдения каждого и
три названных предела. Их фактическая узость подтверждена двумя свежими
комментарий-только мутациями:

- Комментарий `# GATEWAY_HEATMAP_WINDOW: 0.001` в `gateway-checkpoint` делает
  прежние file-wide presence greps зелёными, но committed compose oracle
  остаётся RED: запись ENV не находится в блоке `gateway-serve`.
- Комментарии с именами default/effective accessor делают прежние name-only
  probes зелёными, но current construction anchors остаются RED: нет
  объявления константы и нет `pub fn` declaration.

Мутации откачены. Это воспроизводит требуемый §8sexies мир: старый гейт не
отличал комментарий, текущая группа отличает; observation каждого guard'а
соответствует его требованию, а не более широкому «присутствию где-либо».

## §4bis: checkpoint

Вывод §4bis подтверждён кодом: checkpoint сохраняет полное состояние
`heatmap_buckets`, а окно читается при построении snapshot/output, не при
формировании checkpoint. Поэтому смена window не инвалидирует checkpoint и
`selector_fingerprint` не нужен. Названный предел также верен: пока effective
window не объявлен в кадре, один ответ не самодостаточен для воспроизведения
серии; долг остаётся в milestone pyramid, не маскируется данным гейтом.

## Required route

Architect должен механически добавить ссылку на C-207 в milestone, без смены
содержания предмета. Затем founder может направить engine-dev на tasks 2/3/4;
после task 2 architect применяет task 2b из сохранённой цепочки, далее tester,
reviewer и deploy-gate §8. Нового arbitration trigger этот NOTE не создаёт.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-207
exit=0

$ git rev-parse HEAD; git merge-base origin/main HEAD; git diff --check d77398d..HEAD
94a105f2e439c5a33b49c58656bbe2889ff9fca9
d77398d7b22396c452d2651e90498033186055dd
exit=0

$ bash -n scripts/verify_M-75.sh
exit=0

$ cargo test -p gateway --test red_heatmap_window_decoupled --quiet
test result: FAILED. 1 passed; 2 failed
exit=101

$ cargo test -p gateway --test red_heatmap_window_server_owned --quiet
test result: FAILED. 1 passed; 1 failed
exit=101

$ cargo test -p gateway-serve --test red_heatmap_window_env --quiet
test result: FAILED. 2 passed; 3 failed
exit=101

$ cargo test -p gateway-serve --test red_heatmap_window_effective_setting --quiet
test result: FAILED. 1 passed; 1 failed
exit=101

$ cargo test -p gateway --test red_depth_from_book --quiet
test result: ok. 9 passed; 0 failed
exit=0
$ cargo test -p gateway --test red_depth_provenance_by_reach --quiet
test result: ok. 9 passed; 0 failed
exit=0
$ cargo test -p gateway --test red_heatmap --quiet
test result: ok. 4 passed; 0 failed
exit=0
$ cargo test -p gateway --test red_egress_cap --quiet
test result: ok. 9 passed; 0 failed
exit=0
$ cargo test -p gateway-serve --test red_heatmap_window_effective_setting --quiet
test result: ok. 2 passed; 0 failed
exit=0
$ cargo test -p gateway-serve --test red_heatmap_window_env --quiet
test result: FAILED. 4 passed; 1 failed (heatmap_window_is_declared_in_compose)
exit=101

$ (comment-only compose mutation) old_filewide_presence; current_gateway_serve_record
old_filewide_presence=PASS
current_gateway_serve_record=FAIL
target `heatmap_window_is_declared_in_compose`: FAILED
exit=101

$ (comment-only lib.rs mutation) old_effective_name; new_effective_declaration; old_default_name; new_default_declaration
old_effective_name=PASS
new_effective_declaration=FAIL
old_default_name=PASS
new_default_declaration=FAIL
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=d77398d7b22396c452d2651e90498033186055dd bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 4
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=d77398d7b22396c452d2651e90498033186055dd bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона d77398d..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=d77398d7b22396c452d2651e90498033186055dd bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 5, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0
```
