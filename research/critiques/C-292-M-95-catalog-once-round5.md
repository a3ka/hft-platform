<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: 52ea1ea8828a4de9c6af0422eb93915be5c4349a
audited_head: f5010a957a68e373699546a5bc98eb0830712b4f
verdict: REJECT
-->

# C-292 — M-95 catalog-once, круг 5: ответ architect на R-249 — REJECT

**Роль:** critic, суженный plan-time круг только по ответу на `R-249`.
**Предмет:** `origin/feat/M-95-catalog-once`; вершина получена через
`git rev-parse origin/feat/M-95-catalog-once` и равна
`f5010a957a68e373699546a5bc98eb0830712b4f`.
**Суженный диапазон architect:** `52ea1ea8..f5010a95` (`bcb77d6b`, `f5010a95`).

## Verdict: REJECT

`f3`/`f4` — годные RED-оракулы для Б-1, но ответ на Н-4 не завершает
RED-first набор: `red_ws_honesty_sessions` `o6` не исполняет cold-ветку
`LiveReducer::resume` и не пиннит возврат прежнего источника провенанса.
Поэтому задача 5 пока не может быть передана engine-dev как исполнимая в его
зоне: необходимый новый RED-оракул принадлежит architect (`*/tests/**` sacred
и прямо отнесён к architect в §10 milestone).

## B-1 — f3/f4 действительно сторожат fail-closed на обоих прод-путях

На вершине целевой набор дал ожидаемую RED-базу: `f0…f2` PASS, `f3` и `f4`
FAILED. Оба провала дошли до утверждения провенанса с
`history_start_seq=0, history_truncated=false`, а не до таймаута rendezvous.

Локальный образец в `crates/gateway-serve/src/lib.rs` заменил на обоих путях
`unwrap_or_default`/`let _ =` на распространение `is_fresh`/`refresh` через
`io::Result` и fallback `(snap.history_start_seq, true)`. Он сделал `5/5` PASS.
После обратной мутации только legacy-пути `f4` снова FAILED. Все эти изменения
откачены; в дереве не осталось правки `crates/**`.

`UnlistableDir::seal` — честный setup-страж: после `chmod 0300` он требует
`read_dir(...).is_err()`. Под root этот мир не будет помечен SKIP — assertion
завалит тест как несостоявшийся setup, что соответствует `testing.md`
«падает против несостоявшегося setup». В данном прогоне uid был `1002`, и f3/f4
дошли до проверяемого поведения.

NOTE: `Drop` возвращает доступность каталога, но не исходные биты прав: он всегда
назначает `0755` вместо сохранённого до `seal` mode. Для корректного восстановления
режима страж должен сохранять исходный mode и возвращать именно его. Это не причина
данного REJECT, но правка должна остаться у architect вместе с тестом.

## B-2 — Н-4 не закреплён поведенческим оракулом

`VB-I-11` требует `history_start_seq` первого **реально свёрнутого** события,
а не `header.first_seq`; особенно важно это для legacy-сегмента, у которого
header может быть синтезирован как нулевой. Текущая cold-ветка `LiveReducer::resume`
вычисляет провенанс из `SegmentCatalog`, затем читает stream и получает `first_seq`,
но отбрасывает его. §3/§6 требует вернуть прежний источник, однако указанный в задаче
5 `red_ws_honesty_sessions` `o6` этого не доказывает: он предварительно создаёт
warm checkpoint и тестирует путь выдачи со слепком.

Воспроизведение: локальная мутация cold-ветки в безусловный `(0, false)` оставила
`o6_full_journal_is_not_truncated` и `o6_pruned_journal_is_honestly_marked` зелёными.
Значит, оракул не пиннит ни источник, ни рассматриваемый путь.

Минимальное условие закрытия — architect добавляет RED в `crates/gateway/tests/`,
который напрямую вызывает `LiveReducer::resume` без checkpoint на legacy-журнале,
где `header.first_seq == 0`, а seq первого реально свёрнутого видимого события
больше нуля. Он обязан требовать этот реальный seq и `history_truncated=true` и
падать как на текущем чтении catalog/header, так и на `(0, false)`. После его RED
engine-dev может в своей §10-зоне изменить только `crates/gateway/src/lib.rs` для
Н-4; кодовые части Б-1/Н-2/Н-3 также лежат в разрешённых gateway/gateway-serve paths.

## Required resubmission

1. Architect commits the cold-`resume` RED oracle above (and, advisably, preserves
   the original mode in `UnlistableDir::Drop`).
2. The resubmission shows the oracle RED against the current catalog/header source.
3. Only then dispatch task 5 to engine-dev; the ensuing implementation must turn
   f0…f4 and the new cold oracle GREEN without editing sacred tests or scripts.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-95-catalog-once
f5010a957a68e373699546a5bc98eb0830712b4f
exit=0

$ git log --oneline 52ea1ea8..f5010a95
f5010a95 docs(M-95): R-249 — fail-closed отказа свежести (§3/§6), Н-4 прежний источник холодного пути, задача 5 [architect]
bcb77d6b test(M-95): R-249 Б-1 — f3/f4: отказ обновления каталога на прод-пути обязан давать (frozen, true) [architect]
exit=0

$ cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh -- --nocapture
test f0_v1_without_retention_history_is_complete ... ok
test f1_v1_retention_between_catalog_and_provenance_is_honest ... ok
test f2_legacy_retention_between_catalog_and_provenance_is_honest ... ok
test f3_v1_catalog_refresh_failure_is_fail_closed ... FAILED
test f4_legacy_catalog_refresh_failure_is_fail_closed ... FAILED
test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ <local specimen: Err from is_fresh/refresh => (snap.history_start_seq, true) on v1 and legacy>; cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh -- --nocapture
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ <local mutant: fail-closed only on v1>; cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh f4_legacy_catalog_refresh_failure_is_fail_closed -- --nocapture
test f4_legacy_catalog_refresh_failure_is_fail_closed ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4 filtered out
exit=101

$ cargo test -p gateway-serve --test red_ws_honesty_sessions o6_
test o6_full_journal_is_not_truncated ... ok
test o6_pruned_journal_is_honestly_marked ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
exit=0

$ <local cold-resume mutant: unconditional (0, false)>; cargo test -p gateway-serve --test red_ws_honesty_sessions o6_
test o6_full_journal_is_not_truncated ... ok
test o6_pruned_journal_is_honestly_marked ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
exit=0

$ git restore --source=HEAD -- crates/gateway-serve/src/lib.rs crates/gateway/src/lib.rs && git status --porcelain
exit=0
```
