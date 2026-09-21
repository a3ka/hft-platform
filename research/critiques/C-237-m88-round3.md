<!-- GATE-META
milestone: M-88
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: de116efb27da4ae74d3406fd7c298e11f13a19db
verdict: REJECT
-->

# C-237 — M-88 round 3, execution of A-036 §8: REJECT

## Verdict

**REJECT — do not dispatch dev.** This audit judges only execution of the binding
arbitration decision `A-036` §8. It does **not** reopen the resolved dispute over an
in-repository real Next.js client: the client remains an external founder-owned acceptance
dependency under the four conditions of `A-036` §4.3.

The architect has committed the full plan-time shape: T-designate form and the
`Snapshot::apply -> ApplyOutcome` signature, separate RED suites, the milestone, and an
aggregate verification script. No `contracts/` path changed, so no contract-RFC is due.
`VB-I-2` is the applicable invariant: the assembled live tail must be bit-identical to replay
of the same journal window. `crates/gateway` has no own FA; this audit relies on
`docs/fa/viz-backend.md` §5 and `DESIGN.md` §22. **FA-WAIVER: crates/gateway — собственной
FA нет; применён живой инвариант VB-I-2 из docs/fa/viz-backend.md.**

The consumer text, close-out lock, and two founder questions are present. The two blocking
defects are in the required executable proof: the proposed vector oracle does not execute its
claimed consumer-side convergence, and the predicate probe/gate do not meet the declared
scenario and baseline shape.

## A-036 §4.3 — four conditions

| Condition | Result | Evidence |
|---|---|---|
| 1. Eleven frozen vectors, producer-byte check **and model convergence** | **REJECT** | `red_m88_golden_vectors.rs` names 7+4 vectors, but `crates/gateway/tests/fixtures/m88/` has 0 files and the oracle fails for all eleven. Worse, its purported second direction only compares `expected_series.json` to a newly computed full replay (`:295-301`); it contains neither `serde_json::from_slice` nor `Snapshot::apply`. Thus, even after fixture generation it would not prove that frozen snapshot + frozen frames converge at the consumer/model. |
| 2. Consumer contract including `2^53` | PASS | Milestone §5.1 names `Applied`/`OutOfOrder`/`Incompatible`, resnapshot after both rejection outcomes, and `schema_version == 11`. §5.2 names `i64` on wire, JSON's exact limit `2^53`, and exact `BigInt`/string parsing. |
| 3. Close-out lock | PASS (planned) | Milestone §12.2 requires reviewer to create a MAJOR `built-not-wired` debt and leave `P0-CORR` open. `docs/ROADMAP.md` says M-88 cannot close the row before vectors pass against the frontend. |
| 4. Questions for founder | PASS | Milestone §12.3 gives the two required questions: run all 7+4 vectors against `code2alpha`, then establish whether the user-visible issue is only the Rust-model defect. |

The missing fixtures and mutant battery are legitimate *pre-GREEN* red conditions only because
the milestone schedules their creation for architect step 2. They cannot be reported as
already-present material. The roadmap's statement that the vectors are “in the delivery” and
that mutation control is “executed” is false on this audited head.

## Blocking findings

### R1 — the vector oracle omits the required consumer-side direction

`A-036` §4.3(1) requires two independent checks: today's producer must reproduce frozen wire
bytes, and the model must apply those frozen bytes and converge to the frozen expected state.
The committed oracle implements the first comparison, but its alleged second direction derives
`expected` directly from `gateway::snapshot(... Cursor::LATEST)` and compares it to the frozen
expected JSON. It never deserializes the frozen snapshot or frames and never applies them.

This is not the already-resolved request to run the Next.js frontend in this repository. It is
the in-repository material that the arbiter required precisely so the external consumer can be
tested later. The producer and replay could drift together while the application path remains
untested. The fixture directories are currently absent as the test's own raw failure lists.

**Required correction:** make the post-GREEN golden oracle exercise the frozen wire snapshot
and every frozen frame through the declared consumer/model application path, asserting its final
state against the frozen expected state; retain fail-closed absence for all eleven fixture sets.

### R2 — predicate self-test and aggregate baseline disagree with the required contract

`bash scripts/tests/red_verify_M-88.sh` exits 0, but prints **12** scenarios, not the mandated
13. The repository's P0-CORR row claims 13 and the supplied acceptance expectation is also 13.
In the current non-empty task-9 situation it tests only the rejecting placeholder; it has no
positive case that supplies a named decision for every actual changed non-M-88 test.

The aggregate gate returns `VERDICT: FAIL (провалов: 12)`, not the specified ten failures with
three deserved passes. It has only two passes (predicate probe and fmt): task 9 is unexpectedly
red because `m88_task9` excludes only the original two RED files but sees the new
`red_m88_golden_vectors.rs`, while §14.1 has no decision for it. This is a real task-9 failure,
not a green implementation failure. It also contradicts the roadmap claim that all five bypasses
are now covered by a 13-scenario probe.

The five original bypass classes are rejected at the present head: task 2's misplaced comment,
task 7's commented annotation/signature, task 8's fabricated facts, and task 9's replacement
placeholder return non-zero in the probe; R4 is red because the required battery file is absent.
An isolated `m88_task2() { return 0; }` mutation makes the probe red, so the existing task-2
mutation control is genuine. That does not cure the missing thirteenth scenario or the task-9
positive branch.

**Required correction:** reconcile the task-9 source set with §14.1 and add the absent
bidirectional scenario(s), including an honest non-empty task-9 case. The resulting scripts must
produce the declared 13/0 probe result and the stated `verify_M-88.sh` baseline shape before
dev dispatch.

## Scope and arbitration compliance

- The subject contains only architect-owned milestone/docs, RED tests, and verify/probe assets;
  it does not modify T1 `contracts/` or a RISK-BLOCK path.
- `A-036` remains authoritative: no finding demands an in-repository Next.js or websocket
  oracle. The required external frontend check remains locked behind `P0-CORR` and founder.
- `git diff --check origin/main..HEAD` and merge-preview design claims pass. They do not prove
  the missing application of frozen vectors.

## Required disposition

Return to **architect** for an A-036 §8-only correction. Re-run critic after the committed
oracle/probe/verify corrections. This is a new implementation defect in the arbitration remedy,
not a third REJECT for the resolved real-client methodology; the `gates.md` §0 arbitration rule
therefore does not trigger another arbiter route.

## Done Block

```text
$ git fetch origin --quiet && git rev-parse HEAD && git merge-base origin/main HEAD
de116efb27da4ae74d3406fd7c298e11f13a19db
d5163b5b35abbca204a8981e974bd6e5a97eb9de
exit=0

$ bash scripts/next_artifact_id.sh C
C-237
exit=0

$ bash scripts/tests/red_verify_M-88.sh
ok    task2 SETUP: честная и подделанная фикстуры различны
ok    task2 ЧЕСТНО: семантика при поле ⇒ предъявлено (rc=0)
ok    task2 ПОДДЕЛКА: фраза в чужом комментарии ⇒ отказ (rc=1)
ok    task7 ЧЕСТНО: атрибут и сигнатура некомментарны ⇒ предъявлено (rc=0)
ok    task7 ПОДДЕЛКА: обе строки закомментированы ⇒ отказ (rc=1)
ok    task8 ЧЕСТНО: датировано живой ревизией и названа команда (rc=0)
ok    task8 ПОДДЕЛКА: три выдуманные строки ⇒ отказ (rc=1)
ok    task8 ПОДДЕЛКА: ревизия равна базе ⇒ отказ (rc=1)
ok    task8 ПОДДЕЛКА: выдуманная ревизия не существует в истории ⇒ отказ (rc=1)
ok    task9 SETUP: merge-base d5163b5..., изменённых чужих тестов: 1
ok    task9 ПОДДЕЛКА: изменённые файлы есть, решений нет ⇒ отказ (rc=1)
ok    R4: батареи нет, и шаг гейта обязан быть КРАСНЫМ
VERDICT: PASS — сценариев: 12, расхождений: 0
exit=0

$ # disposable worktree: m88_task2() { return 0; }; bash scripts/tests/red_verify_M-88.sh
FAIL  task2 ПОДДЕЛКА: фраза в чужом комментарии ⇒ отказ: ожидался rc=1, получен rc=0
VERDICT: FAIL — сценариев: 12, расхождений: 1
exit=1

$ cargo test -p gateway --test red_m88_golden_vectors -- --nocapture
test vectors_cover_seven_scenarios_and_four_recoveries ... ok
test golden_vectors_match_producer_and_expectation ... FAILED
эталонных векторов НЕТ для: ["s1_removed_in_current_bucket", "s2_closed_past_survives", "s3_empty_full_slice", "s4_absent_observation", "s5_price_leaves_window", "s6_duplicate_frame", "s7_stale_frame", "r1_recover_after_out_of_order", "r2_clear_on_empty_slice", "r3_keep_on_absent_observation", "r4_reject_foreign_schema"]
test result: FAILED. 1 passed; 1 failed
exit=101

$ find crates/gateway/tests/fixtures/m88 -type f 2>/dev/null | wc -l
0
exit=0

$ bash scripts/verify_M-88.sh
FAIL  task1: нет полей heatmap_observed_time_s / cob_observed
FAIL  task1: GATEWAY_SCHEMA_VERSION не равен 11
FAIL  task2: комментарий ПЕРЕД heatmap_cells не объявляет полный срез бакета либо предписывает объединение
FAIL  task3-6: red_m88_update_contract КРАСЕН — 5 passed; 8 failed
FAIL  task1+7: red_m88_contract_form КРАСЕН — компиляция
FAIL  task7: apply не возвращает ApplyOutcome либо #[must_use] не стоит некомментарной строкой рядом с сигнатурой
FAIL  task8: нет раздела «Результат ПОСЛЕ реализации» с FACTS/командой/числами до-после
FAIL  task9: в §14.1 нет решения по изменённым файлам: red_m88_golden_vectors.rs
FAIL  R4: батареи scripts/tests/red_m88_mutants.sh НЕТ
FAIL  A-036 п.1: эталонные векторы не предъявлены или разошлись — 1 passed; 1 failed
PASS  проба предикатов: все сценарии сошлись
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 12)
exit=1

$ git diff --check origin/main..HEAD
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
```
