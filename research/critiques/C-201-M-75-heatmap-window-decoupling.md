<!-- GATE-META
milestone: M-75
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: 5873aa89ecf9d6125b8b0156bc71468226e562c7
verdict: REJECT
-->

# C-201 — M-75 heatmap-window-decoupling, круг 4: REJECT

## Судимый набор

Проверен `origin/feat/M-75-heatmap-window-decoupling` на
`5873aa89ecf9d6125b8b0156bc71468226e562c7` против актуального `origin/main`
`2e63a37e5bf454da69b0fbd69de28c043b4caf4c` (merge-base `d77398d`). Набор
содержит объявленные §5 сигнатуры, четыре M-75 RED-target'а H-1…H-6,
`verify_M-75.sh` и milestone; T1/RFC/schema/GATEWAY_BANDS не меняются.

Прочитаны и применены `VB-I-2`, `VB-I-5`, **VB-I-10** и карта **MD-I-8** из
`docs/fa/viz-backend.md`. Существенны здесь `VB-I-10`: bounded-window snapshot
должен оставаться проверяемым на всех путях, и `MD-I-8`: legacy witnesses нельзя
терять молча. Применено Р-4 из актуального `origin/main`: признак обязан быть
недоступен миру, где события нет. `П-014`, `П-020` и `П-027` не изменяются.

## Что принято

- `C-196` B-3/B-4 остаются закрытыми: H-6 committed до dispatch, а §14 обновлён.
- Инвентарь `C-198` правильно расширил область с двух MD-I-8 свидетелей до
  четырёх файлов и десяти внешних функций. В `5142a141` ни один прежний assert
  не ослаблен: нулевой diff показывает только установку окна в `sel()` и guards
  в `red_egress_cap.rs`.
- Утверждение о гонке в `red_egress_cap.rs` воспроизводится как закрытое именно
  этим guard'ом: после прототипа+2b single-thread и обычный параллельный прогоны
  дали по 9 passed / 0 failed.

## Blocking findings

### B-6 — Приём 2b оставляет `HM-I-2` гонкой; `959/1` не воспроизводится

§8quater предписывает каждой централизованной `sel(bands)` записывать
process-global window, равное `max(bands)` (milestone:276-288). В
`red_heatmap.rs` четыре теста работают в одном процессе и используют разные
полосы: в частности `heatmap_windowed_and_provenance` ставит `0.03` и требует
увидеть уровень 63_500 в ±3%-окне (lines 125-153), а соседние тесты ставят
`0.001`. `5142a141` добавляет `serial()` только в `red_egress_cap.rs`; в
`red_heatmap.rs` такого guard'а нет.

На полном параллельном прогоне прототипа tasks 2+3 с точечным 2b-приёмом
`heatmap_windowed_and_provenance` упал на `63500 в окне — ячейка есть`: другой
тест успел заменить window на 0.001. В том же прогоне законно красен
`heatmap_window_is_declared_in_compose`. Итог — два failed targets и exit 101,
а не заявленный §8quater `passed=959 failed=1`.

Это не косметический флак: порядок выполнения изменяет окно **сакрального
HM-I-2** и тем самым разрушает проверяемость bounded-window поведения
`VB-I-10`. Один удачный single-thread прогон не доказывает параллельную
совместимость. До dispatch architect должен представить RED/fixture решение,
которое сериализует либо изолирует process-global window во **всех** затронутых
test binaries, и повторить полный `cargo test --all --quiet --no-fail-fast` с
только законным compose-RED.

### B-7 — Шаг verify task #2b вакуумно зелёный; salvage-ref не заменяет committed gate

До task 2 текущий шаг действительно красен: все четыре поиска setter, поиск
`fn serial()` и count guard'ов возвращают 1. Но это не доказывает 2b. Шаг
`scripts/verify_M-75.sh:146-156` судит только неструктурные `grep`: он не
проверяет вызов, исполнимость или сохранение предмета oracle.

В чистом worktree я добавил **только комментарии** с искомыми строками в эти
четыре файла и девять комментариев `let _g = serial();`; ни код, ни asserts,
ни реализация M-75 не менялись. Все шесть проверок task #2b стали зелёными
(exit 0), а исходный `red_heatmap` оставался 4/4 GREEN. Проба полностью
откачена. Следовательно, утверждение §8quater/verify, что шаг «не может
позеленеть вакуумно», ложно; это ровно запрещённый gates.md/testing.md класс
гейта, измеряющего текст вместо собственного инварианта.

Правки внешних sacred-oracles существуют только в salvage refs, не в судимом
`audited_head`; они не превращают text-grep в исполнимый pre-dispatch артефакт.
Конструкция «dev task 2 → красный промежуток → architect task 2b» допустима
только если milestone-ветка несёт не-вакуумный, исполнимый механизм проверки
этого перехода. Сейчас такого механизма нет. Architect должен заменить
текстовые probes на gate, который исполняет affected-oracle set и различает
отсутствие 2b, его несборку и его фактическое восстановление; committed artifact
set обязан содержать этот путь до dispatch.

## Required disposition

**REJECT.** Не dispatch'ить M-75. Следующий круг — architect: закрыть B-6 и
B-7 committed RED/verify artifacts, затем повторно предъявить subject-branch
commit-chain. Это новые основания относительно `C-198`, поэтому арбитраж по
снятому `C-196` B-3 не требуется.

## Done Block

```text
$ git rev-parse origin/main; git rev-parse HEAD; git merge-base origin/main HEAD
2e63a37e5bf454da69b0fbd69de28c043b4caf4c
5873aa89ecf9d6125b8b0156bc71468226e562c7
d77398d7b22396c452d2651e90498033186055dd

$ git diff --name-status origin/main..HEAD | rg 'M-75|heatmap_window|verify_M-75|contracts|docs/rfc'
A	crates/gateway-serve/tests/red_heatmap_window_effective_setting.rs
A	crates/gateway-serve/tests/red_heatmap_window_env.rs
A	crates/gateway/tests/red_heatmap_window_decoupled.rs
A	crates/gateway/tests/red_heatmap_window_server_owned.rs
A	milestones/M-75-heatmap-window-decoupling.md
A	scripts/verify_M-75.sh

$ cargo test -p gateway --test red_egress_cap -- --test-threads=1
test result: ok. 9 passed; 0 failed
exit=0

$ cargo test -p gateway --test red_egress_cap
test result: ok. 9 passed; 0 failed
exit=0

$ cargo test --all --quiet --no-fail-fast  # prototype tasks 2+3 + exact 2b semantics
heatmap_windowed_and_provenance --- FAILED
thread 'heatmap_windowed_and_provenance' panicked: 63500 в окне — ячейка есть
heatmap_window_is_declared_in_compose --- FAILED
error: 2 targets failed:
    -p gateway --test red_heatmap
    -p gateway-serve --test red_heatmap_window_env
exit=101

$ task #2b grep probes before task 2 (clean audited head)
four setter probes: exit=1 each
serial probe: exit=1
guard-count probe (0): exit=1

$ comment-only mutation of those six probes; no implementation/assert changes
four setter probes: exit=0 each
serial probe: exit=0
guard-count probe: exit=0
cargo test -p gateway --test red_heatmap --quiet
test result: ok. 4 passed; 0 failed
exit=0
mutation reverted

$ git diff --check; git diff --exit-code; git status --porcelain
exit=0

$ bash -n scripts/verify_M-75.sh
exit=0
```
