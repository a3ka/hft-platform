<!-- GATE-META
milestone: M-88
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: 7eabbc3a9ce32ac3c7d17da5c039a2a76144eac2
verdict: REJECT
-->

# C-239 — M-88 round 4, closures of C-237: REJECT

## Verdict

**REJECT — do not dispatch dev.** C-237 R1, R2, and R3 are closed, but the
full mandatory gate still fails: after passing `A-036` (`DECISION`) the subject
changed `scripts/tests/red_verify_M-88.sh` and `scripts/verify_M-88.sh` without an
`ALLOW-SUBJECT-CHANGE` audit token. `check_gate_meta.sh` rejects the exact committed
range. This audit judges head `7eabbc3` only. It does not reopen the resolved question
of an in-repository real Next.js client: `A-036` §4.3 remains binding, and frontend
acceptance remains the named external dependency.

`gateway` has no separate FA. The applicable live invariant is **VB-I-2**:
live-tail assembly must be bit-identical to replay of the same journal window
(`docs/fa/viz-backend.md` §5); `DESIGN.md` §22 identifies the same viz invariant
family. **FA-WAIVER: crates/gateway — собственной FA у крейта нет; проверен
VB-I-2 из docs/fa/viz-backend.md.**

The committed plan-time artifact set is present: the T-designate update form and
`Snapshot::apply -> ApplyOutcome` signature are specified in milestone §§4.2/5;
the three architect-owned RED suites are present; the aggregate verifier and its
predicate probe are present; and the milestone names allowed/forbidden paths and
the task order. `crates/contracts/**` is untouched, so T1/contract-RFC is not due:
the viz wire form is T-designate (`docs/05-contract-layer.md` §2).

## C-237 closures

### R1 — golden vector consumer path: closed

`red_m88_golden_vectors.rs:315-338` parses the frozen `snapshot.json`, then for
every produced/frozen `frame_NN.json` parses the wire bytes, calls
`Snapshot::apply`, requires `ApplyOutcome::Applied`, serializes the resulting
model series, and compares that final state to frozen `expected_series.json`.
This is a consumer-side application of frozen bytes, not a second replay-vs-replay
comparison. The preceding checks retain producer-byte and full-replay comparisons
as independent drift guards.

Fixture absence remains fail-closed: missing snapshots are accumulated for all
eleven vector names and the final assertion fails with their names; a missing
expected-state or frame file fails immediately on read. The current RED baseline
cannot yet execute that test because `ApplyOutcome` is deliberately absent from
the pre-dev implementation; it nevertheless fails the dedicated A-036 step, as
required. After GREEN, this same oracle will exercise the absence branch rather
than skip it.

### R2 — task-9 bidirectional probe: closed

`red_verify_M-88.sh` reports **17 scenarios, 0 mismatches**. Its four task-9
semantic cases are all present: empty/declared, empty/silent, non-empty/named,
and non-empty/unnamed. The non-empty cases are made real in a disposable detached
worktree by changing `red_heatmap.rs` and committing that one file, so they do not
depend on the current branch incidentally containing a third-party changed oracle.

Replacing `m88_task9` with `return 0` made exactly the two negative task-9 cases
red (empty/silent and non-empty/unnamed). Thus the probe detects the predicate's
loss of both fail branches rather than merely observing a happy path.

### R3 — M-88 files are not foreign expectations: closed

`m88_task9` now excludes the three M-88 RED suite files
`red_m88_update_contract.rs`, `red_m88_contract_form.rs`, and
`red_m88_golden_vectors.rs` from its diff-derived foreign-expectation set. At the
audited head that set is empty and §14.1 explicitly declares it empty; task 9 is
therefore the expected third PASS in the red baseline, not a false requirement to
review this milestone's own new tests.

## Blocking finding B1 — A-036 subject-lock is red

`A-036` is a passing `DECISION`; `check_gate_meta.sh` therefore locks gate-class files
after its audited head. The two commits closing C-237 change the predicate probe and
aggregate verifier, both gate-class paths, but no commit in A-036's own branch range
contains `ALLOW-SUBJECT-CHANGE:`. The barrier fails rather than treating execution of
an arbitration decision as an implicit exemption. This is a new execution finding,
not a third dispute over the real-client methodology; no new arbiter route follows.

**Required correction:** architect records the normal mechanical milestone appendix and
commits it with a body line of the required form
`ALLOW-SUBJECT-CHANGE: <reason of at least 12 characters>` that names execution of the
binding A-036/C-237 correction. Then rerun
`bash scripts/check_gate_meta.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de`; it must exit 0
before M-88 is dispatched.

## Acceptance and scope

The baseline is intentionally RED before implementation: `verify_M-88.sh` exits 1
with eleven actual failures and precisely three passes (`task9`, predicate probe,
and fmt). It is a real aggregate gate: `set -uo pipefail`, a FAIL counter, non-zero
exit on failures, one check per task, the golden-vector step, and the full CI
triple. The new test/verify/milestone paths are architect-owned and the planned
implementation scope remains only `crates/gateway/src/lib.rs` for engine-dev.
No RISK-BLOCK path is in the subject.

## Required disposition

Return to **architect** for the narrow subject-lock correction above, then rerun the
critic gate. The known RED failures remain the intended pre-implementation contract,
not a passing claim.

## Done Block

```text
$ git fetch origin feat/M-88-liquidity-removal-contract && git rev-parse HEAD && git merge-base origin/main HEAD
7eabbc3a9ce32ac3c7d17da5c039a2a76144eac2
d5163b5b35abbca204a8981e974bd6e5a97eb9de
exit=0

$ bash scripts/next_artifact_id.sh C
C-239
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
ok    task9 SETUP: merge-base d5163b5b35abbca204a8981e974bd6e5a97eb9de, изменённых ЧУЖИХ ожиданий: 0
ok    task9 ЧЕСТНО(пусто): пустое множество ЗАЯВЛЕНО ⇒ предъявлено (rc=0)
ok    task9 ПОДДЕЛКА(пусто): пустота молчит под другим плейсхолдером ⇒ отказ (rc=1)
ok    task9 SETUP(непусто): одноразовое дерево создано
ok    task9 SETUP(непусто): множество непусто (red_heatmap.rs изменён)
ok    task9 ЧЕСТНО(непусто): изменённый файл НАЗВАН ⇒ предъявлено (rc=0)
ok    task9 ПОДДЕЛКА(непусто): изменённый файл НЕ назван ⇒ отказ (rc=1)
ok    R4: батареи нет, и шаг гейта обязан быть КРАСНЫМ (fail-closed, прецедент verify_M-65 F2)

VERDICT: PASS — сценариев: 17, расхождений: 0
exit=0

$ # disposable mutation: m88_task9() { return 0; }; bash scripts/tests/red_verify_M-88.sh
FAIL  task9 ПОДДЕЛКА(пусто): пустота молчит под другим плейсхолдером ⇒ отказ: ожидался rc=1, получен rc=0
FAIL  task9 ПОДДЕЛКА(непусто): изменённый файл не назван, а предикат принял (rc=0)
VERDICT: FAIL — сценариев: 17, расхождений: 2
exit=1

$ bash scripts/verify_M-88.sh
FAIL  task1: нет полей heatmap_observed_time_s / cob_observed в crates/gateway/src/lib.rs
FAIL  task1: GATEWAY_SCHEMA_VERSION не равен 11 (смена формы ⇒ бамп обязателен, VB-I-4)
FAIL  task2: комментарий ПЕРЕД heatmap_cells не объявляет полный срез бакета либо предписывает объединение
FAIL  task3-6: red_m88_update_contract КРАСЕН — test result: FAILED. 5 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
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

$ git diff --check origin/main..HEAD
exit=0

$ bash scripts/check_artifact_ids.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de
OK: ни один коммит диапазона d5163b5..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ bash scripts/check_gate_meta.sh d5163b5b35abbca204a8981e974bd6e5a97eb9de
── GATE-META: диапазон d5163b5b..HEAD, origin=a3ka/hft-platform
   якорь main-стороны НЕ применён (прод-форма merge-ref не подтверждена) — судится весь диапазон
FAIL  research/arbitration/A-036-m88-real-client.md: subject-lock — после проходного вердикта (DECISION) тронут класс «гейт»: scripts/tests/red_verify_M-88.sh scripts/verify_M-88.sh
      выход из лока — строка «ALLOW-SUBJECT-CHANGE: <причина>» в теле коммита диапазона

VERDICT: FAIL (1) — вердикт не привязан к предмету либо merge прошёл без вердикта.
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
```
