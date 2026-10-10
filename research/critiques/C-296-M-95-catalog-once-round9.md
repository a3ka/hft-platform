<!-- GATE-META
milestone: M-95
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: d6625520cdaf969e145bad389d40d7c7fe770706
verdict: NOTE
-->

# C-296 — M-95 «catalog once», круг 9: f5/f6 для `R-250` Б-1 — NOTE

**Роль:** critic, узкий plan-time круг. **Предмет:**
`origin/feat/M-95-catalog-once` на
`d6625520cdaf969e145bad389d40d7c7fe770706`; база
`ad20a7e7b844bf3882d846ebb66d3b049753c74b` (`origin/main`).

## Verdict: NOTE

`R-250` Б-1 закрыт в заявленном узком объёме. `f5` и `f6` исполняют именно
`is_fresh → Ok(false)` затем `refresh → Err` на прод-пути v1 и legacy. Честный
набор зелёный 7/7; MV2 роняет `f5`, ML2 роняет `f6`.

Это защищает `VB-I-11`: при не подтверждённой свежести провенанс не берётся из
устаревшего каталога, а остаётся `(frozen_start_seq, true)`.

## Проверка оракула

- `act_refresh_fails` строит отдельный публичный `SegmentCatalog` до мутации,
  удаляет три ранних `.zst` (diff > 2), сохраняет каталог читаемым, закрывает
  следующий сегмент, затем требует `is_fresh == Ok(false)` и `refresh.is_err()`.
  Поэтому setup не подменяет ветку ошибкой `is_fresh` из f3/f4
  ([`red_m95_provenance_fresh.rs:641`](../../crates/gateway-serve/tests/red_m95_provenance_fresh.rs#L641)
  – [`:678`](../../crates/gateway-serve/tests/red_m95_provenance_fresh.rs#L678)).
- v1 `f5` использует канал `m95-catalog:s5` и проверяет `history_truncated`
  после точки непосредственно перед провенансом
  ([`:681`](../../crates/gateway-serve/tests/red_m95_provenance_fresh.rs#L681)
  – [`:727`](../../crates/gateway-serve/tests/red_m95_provenance_fresh.rs#L727)).
  Legacy `f6` повторяет тот же мир на `m95-catalog:legacy`
  ([`:729`](../../crates/gateway-serve/tests/red_m95_provenance_fresh.rs#L729)
  – [`:764`](../../crates/gateway-serve/tests/red_m95_provenance_fresh.rs#L764)).
- MV2 заменил v1 `Err(_) => None` на расчёт провенанса через каталог; `f5`
  упал. ML2 сделал это же в legacy-рукаве; `f6` упал. Оба исходника возвращены;
  честный код снова 7/7.
- §3 и §8 M-95 соответствуют факту: f3/f4 названы только для `is_fresh → Err`,
  f5/f6 — для `Ok(false) → refresh Err`; §14, строка 8, фиксирует те же оба
  мутанта и 7/7.

## Граница круга

Задача 6 (§8/§14) по корректировке прежних комментариев в
`crates/gateway-serve/src/lib.rs` остаётся назначенной engine-dev и открытой. Это
не повторное открытие `R-250` Б-1 и не блокирует данный узкий verdict: фактическое
покрытие f5/f6 уже описано спецификацией и подтверждено мутациями.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-95-catalog-once
d6625520cdaf969e145bad389d40d7c7fe770706
exit=0

$ cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ MV2 (v1: refresh Err => provenance via catalog); cargo test ... red_m95_provenance_fresh
test f5_v1_refresh_failure_after_stale_is_fail_closed ... FAILED
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ ML2 (legacy: refresh Err => provenance via catalog); cargo test ... red_m95_provenance_fresh f6_legacy_refresh_failure_after_stale_is_fail_closed
test f6_legacy_refresh_failure_after_stale_is_fail_closed ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out
exit=101

$ git diff --check && git diff -- crates/gateway-serve/src/lib.rs && git status --porcelain
exit=0

$ EVENT_NAME=pull_request BASE_SHA=ad20a7e7... HEAD_SHA=171efa41... PR_BASE_SHA=ad20a7e7... PR_HEAD_SHA=171efa41... bash scripts/check_protected_artifacts.sh
NOTE  milestones/M-94-calc-profile.md: ALLOW-ARTIFACT-DELETE в aa73cbf9
OK: защищённые артефакты целы на HEAD (ad20a7e..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request BASE_SHA=ad20a7e7... HEAD_SHA=171efa41... PR_BASE_SHA=ad20a7e7... PR_HEAD_SHA=171efa41... bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 9, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
exit=0

$ EVENT_NAME=pull_request BASE_SHA=ad20a7e7... HEAD_SHA=171efa41... PR_BASE_SHA=ad20a7e7... PR_HEAD_SHA=171efa41... bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона ad20a7e..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```
