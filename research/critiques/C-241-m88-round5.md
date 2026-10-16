<!-- GATE-META
milestone: M-88
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: 7601543144bd55853ab72f8506a67e15451c0090
verdict: NOTE
-->

# C-241 — M-88 round 5: subject-lock release — NOTE

## Verdict

**NOTE — the subject lock is correctly released; no blocker remains for dev
dispatch.** This round judges only the mechanical lock left by `C-239`. It does
not reopen the resolved real-client question (`A-036`) or the R1–R3 closures
accepted by `C-239`.

`gateway` has no dedicated FA. The applicable live invariant remains **VB-I-2**:
the series assembled from the live tail must be bit-identical to replay of the
same journal window (`docs/fa/viz-backend.md` §5; `DESIGN.md` §22).
**FA-WAIVER: crates/gateway — собственной FA у крейта нет; применён VB-I-2 из
docs/fa/viz-backend.md.**

## Subject-lock audit

1. `bash scripts/check_gate_meta.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de`
   exits 0. It identifies the two gate-class files changed after `A-036` and
   accepts their release only through the explicit audit token.
2. The token is in commit `7601543`, which adds only the M-88 gate-round journal:
   `ALLOW-SUBJECT-CHANGE: исполнение A-036 §5.3 и закрытие C-237 R1-R3 — пять
   шагов гейта переписаны по предписанию арбитра, добавлена отрицательная проба
   предикатов`. This is a meaningful reason, not a formal formula: it names the
   binding decision, its required §5.3 work, the corrected critic findings, and
   the actual control added.
3. Milestone §12bis is consistent with the audited record. `C-233` records the
   absent real-client oracle, five false-green steps, and missing repeated-life
   scenario; `C-235` records the reproduced five mutations and remaining model
   gap; `C-237` records the one-way frozen-vector proof and missing principal
   predicate branch; `C-239` records that their substantive closures were
   accepted and leaves only the missing post-`A-036` subject-lock token. It does
   not present the current plan as original or erase the arbitration decision.
4. The pre-implementation baseline has not moved: `verify_M-88.sh` remains
   intentionally red with **11** failures and exactly three passes (`task9`,
   predicate probe, and `fmt`).

## Done Block

```text
$ git rev-parse HEAD && git merge-base origin/main HEAD
7601543144bd55853ab72f8506a67e15451c0090
d5163b5b35abbca204a8981e974bd6e5a97eb9de
exit=0

$ bash scripts/next_artifact_id.sh C
C-241
exit=0

$ bash scripts/check_gate_meta.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de
── GATE-META: диапазон d5163b5b..HEAD, origin=a3ka/hft-platform
   якорь main-стороны НЕ применён (прод-форма merge-ref не подтверждена) — судится весь диапазон
NOTE  research/arbitration/A-036-m88-real-client.md: subject-lock открыт явным ALLOW-SUBJECT-CHANGE (аудит-след, НЕ доказательство — F-064-6): scripts/tests/red_verify_M-88.sh scripts/verify_M-88.sh

VERDICT: PASS — вердиктов проверено: 5, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ bash scripts/verify_M-88.sh
FAIL  task1: нет полей heatmap_observed_time_s / cob_observed в crates/gateway/src/lib.rs
FAIL  task1: GATEWAY_SCHEMA_VERSION не равен 11 (смена формы ⇒ бамп обязателен, VB-I-4)
FAIL  task2: комментарий ПЕРЕД heatmap_cells не объявляет полный срез бакета либо предписывает объединение
FAIL  task3-6: red_m88_update_contract КРАСЕН — test result: FAILED. 5 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
FAIL  task1+7: red_m88_contract_form КРАСЕН — компиляция
FAIL  task7: apply не возвращает ApplyOutcome либо #[must_use] не стоит некомментарной строкой рядом с сигнатурой
FAIL  task8: в docs/plans/m88-frame-size-measurement.md нет раздела '## Результат ПОСЛЕ реализации' с собственным маркером FACTS (существующая ревизия ≠ базы), строкой wsprobe и числами до/после
PASS  task9: решение записано по каждому изменённому ожиданию (дифф от d5163b5b35abbca204a8981e974bd6e5a97eb9de)
FAIL  R4: батареи scripts/tests/red_m88_mutants.sh НЕТ — анти-плацебо не предъявлено (пишется architect'ом ПОСЛЕ GREEN dev'а, §12 шаг 2)
FAIL  A-036 п.1: эталонные векторы не предъявлены или разошлись — компиляция
PASS  проба предикатов: все сценарии сошлись (scripts/tests/red_verify_M-88.sh)
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 11)
exit=1
```
