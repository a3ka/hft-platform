<!-- GATE-META
milestone: M-88
audited_repo: a3ka/hft-platform
audited_base: d5163b5b35abbca204a8981e974bd6e5a97eb9de
audited_head: eb71a8f38ca1a99ee5eab4815aa06de5177f76be
verdict: REJECT
-->

# C-235 — M-88 update contract, round 2: REJECT

## Verdict

**REJECT — do not dispatch dev.** The committed architect set is complete in shape: it
specifies the T-designate form and `ApplyOutcome` signature, has two separate RED binaries, the
milestone, and a fail-closed aggregate script. It correctly leaves `contracts/` untouched;
no contract-RFC is required for this `SeriesBundle` change.

But none of the blocking closure claims is yet proved sufficiently:

1. R1 names the actual Next.js client and makes recovery expressible in a Rust model, but
   expressly does **not** execute that client. This does not satisfy scale-plan §15.2's
   requirement to test the real affected client.
2. R2 and R4 remain false-greenable. A single disposable mutation made task 2, task 7,
   task 8, task 9, and R4 all print `PASS` while every one of those obligations remained
   unfinished.
3. R3's missing lifecycle input is closed by S10, which is legitimately a green regression
   guard; S11 is still not an overflow/truncation boundary oracle.

`VB-I-2` applies: a series assembled from snapshot plus frames must be bit-identical to
replay of the same journal window. `crates/gateway` has no dedicated FA; **FA-WAIVER:
crates/gateway — its own FA is absent (reading-map.md:82), so this audit relies on
docs/fa/viz-backend.md §5, VB-I-2, and DESIGN §22.**

## C-233 closure audit

### R1 — REJECT: the real-client requirement remains unfulfilled

The closure commits add the right contract pieces: `ApplyOutcome::{Applied, OutOfOrder,
Incompatible}` in milestone §4.2, the recovery rule in §4.3, and four wire-oracles in
`red_m88_contract_form.rs`. The model consumes JSON, handles both rejection outcomes, takes
a fresh snapshot, and converges. This proves that recovery is representable and that the
Rust model does not silently freeze.

It is nevertheless not the production consumer. The milestone itself says that the named
Next.js cockpit client is outside the Rust tests and that the model does **not** prove its
implementation (`milestones/M-88-liquidity-removal-contract.md:266-275`; the test says the
same at `crates/gateway/tests/red_m88_contract_form.rs:304-309`). The explicit frontend
acceptance condition is therefore a future requirement, not executed evidence. Scale-plan
§15.2 says the affected path requires a check of the **real client**, not only a server
function or Rust model. Naming the owner does not turn that mandatory oracle into one.

The direct answer to the handoff question is **no**: the Rust client model does not close R1.

### R2 — REJECT: the acceptance script is still not a real task gate

The baseline is correctly red (11 failures, exit 1), and the revision fixed the exact old
string-only checks. It is still possible to satisfy each revised predicate with unfinished
work:

| Checked item | Disposable incomplete mutation | Why it is a false pass |
|---|---|---|
| task 2 | Put `ПОЛНЫЙ срез` in the unrelated `seq` comment and replace only the old literal `замещает раннее` in the heatmap comment | The awk range covers the whole struct; it never binds the new meaning to `heatmap_cells`. |
| task 7 | Add commented text `// #[must_use]` and `// pub fn apply(&mut self, frame: &Frame) -> ApplyOutcome;` before the real old signature | `grep` accepts comments; neither Rust nor the real `Snapshot::apply` has changed. |
| task 8 | Append a made-up `## Результат ПОСЛЕ реализации`, `до: 111`, `после: 110` | The check establishes only three strings, not a before/after run on the named production-form fixture. |
| task 9 | Replace the one rejected placeholder with `_(решение не записано)_` | No per-oracle decision is required. |
| R4 | Replace both cells with `_(не исполнялось; FAILED не предъявлен)_` | Two occurrences of `FAILED` satisfy the grep although neither isolated mutation ran. |

The combined mutation run printed `PASS` for all five targeted checks and still finished
`VERDICT: FAIL (провалов: 6), exit=1` only because the deliberately unimplemented RED work
remained red. On a GREEN implementation these five incomplete obligations could make the
whole gate green. This is the same fail-closed defect C-233 R2 rejected, not an advisory
hardening request.

### R3 — partial closure only

S10 is an adequate answer to the missing *multiple lives of one level* input. It really
uses one price `64990 → 0 → 7` in one bucket, splits the deltas into different frames, has
a setup guard, and is green on the existing implementation. A green oracle in a red
plan-time corpus is not evasion when it guards an already-correct neighbouring invariant;
the baseline corpus still fails eight other behavioural scenarios. S10 must later be listed
in mutation evidence if the chosen Replace implementation can break it.

S11 is not adequate for the remaining numeric-boundary part of R3. Its allegedly huge
value is `1e18` fixed-point, approximately one ninth of `i64::MAX`
(`red_m88_update_contract.rs:647-650`), and the test exercises neither an overflow/reject
path nor a conversion/truncation boundary. It demonstrates that one large ordinary value
and the minimum unit survive the current route; it does not meet `testing.md`'s required
overflow/truncation boundary cases. Keep S10; add an explicit representable-near-boundary
case and the defined non-representable input/outcome.

### R4 — REJECT: §9.4's current placeholder is rejected, but placeholders remain accepted

The current literal `заполняется` cells do make R4 fail on the baseline. That is not enough.
The R4 predicate accepts any two lines containing `FAILED`, has no command/test-name binding,
and only forbids one lowercase word. The two `не исполнялось; FAILED не предъявлен` cells
above pass it. Require one named result for each specified source mutation, its named failing
oracle, and raw command/exit status; run a negative self-probe of that predicate as part of
the gate.

## NOTE — frame-size measurement

There is no contradiction in the observed 0.1% heatmap window and 60% depth bands. The
production capture records separate settings (`GATEWAY_HEATMAP_WINDOW=0.001` and
`GATEWAY_BANDS=…0.6`), consistent with the M-75 decoupling and the separate server-owned
heatmap setting. The measurement does not substitute bands for the map window.

Its conclusion is stronger than its sample, however. `48 / 1.5 ≈ 32` is an average over
different touched columns, compared with min/median/max cells of columns in one connection
snapshot; it is not a before/after measurement of the same columns. It supports Replace as
a plausible provisional choice for this 5-second BTCUSDT sample, but does not prove that
Replace reduces every frame or quantify its distribution. Milestone task 8's GREEN
before/after production-form measurement remains necessary. This NOTE is outside the four
closure findings and is not a separate blocker.

## Routing

C-233 and C-235 are two consecutive REJECTs for the same R1 real-client dispute and the same
R2 fail-closed-gate dispute. Per `gates.md` §0 and the critic profile, do not start a third
architect↔critic loop. Route the committed sources, C-233, and this verdict to a fresh-context
arbiter. The arbiter must decide whether plan §15.2 permits deferring the real Next.js oracle
outside M-88, and whether the demonstrated gate bypasses meet the required fail-closed
standard. It is an engineering-method decision, not a founder choice of product policy.

## Done Block

```text
$ git fetch origin --quiet && git rev-parse origin/feat/M-88-liquidity-removal-contract
eb71a8f38ca1a99ee5eab4815aa06de5177f76be
exit=0

$ bash scripts/next_artifact_id.sh C
C-235
exit=0

$ cargo test -p gateway --test red_m88_update_contract -- --nocapture
test s10_level_reborn_inside_one_bucket_survives ... ok
test s11_extreme_sizes_survive_the_round_trip ... FAILED
test result: FAILED. 5 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ cargo test -p gateway --test red_m88_contract_form
error[E0432]: unresolved import `gateway::ApplyOutcome`
error[E0609]: no field `heatmap_observed_time_s` on type `SeriesBundle`
error[E0609]: no field `cob_observed` on type `SeriesBundle`
error: could not compile `gateway` (test "red_m88_contract_form") due to 9 previous errors
exit=101

$ bash scripts/verify_M-88.sh
FAIL  task1: нет полей heatmap_observed_time_s / cob_observed in crates/gateway/src/lib.rs
FAIL  task1: GATEWAY_SCHEMA_VERSION не равен 11
FAIL  task2: док-комментарий источника не объявляет полный срез бакета (или всё ещё предписывает объединение)
FAIL  task3-6: red_m88_update_contract КРАСЕН — 5 passed; 8 failed
FAIL  task1+7: red_m88_contract_form КРАСЕН — компиляция
FAIL  task7: apply не возвращает ApplyOutcome либо #[must_use] не стоит рядом с сигнатурой
FAIL  task8: в docs/plans/m88-frame-size-measurement.md нет датированной фактуры с разделом '## Результат ПОСЛЕ реализации' и числами до/после
FAIL  task9: таблица §14.1 пуста или содержит плейсхолдер — решение по каждому ожиданию не записано
FAIL  R4: таблица §9.4 не заполнена — нужны ДВА результата вида «нейтрализация X → тест Y FAILED», без плейсхолдеров
PASS  CI-паритет: cargo fmt --all -- --check
FAIL  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
FAIL  CI-паритет: cargo test --all
VERDICT: FAIL (провалов: 11)
exit=1

$ # isolated disposable worktree: incomplete mutations described in R2 table, then exact gate
$ bash scripts/verify_M-88.sh
PASS  task2: источник объявляет наблюдение ПОЛНЫМ СРЕЗОМ и не предписывает объединение
PASS  task7: apply возвращает ApplyOutcome и помечен must_use НЕПОСРЕДСТВЕННО перед сигнатурой
PASS  task8: замер до/после на прод-форме предъявлен (docs/plans/m88-frame-size-measurement.md)
PASS  task9: таблица решений по изменённым ожиданиям заполнена
PASS  R4: результат ОБЕИХ изолированных мутаций записан (плейсхолдеров нет)
VERDICT: FAIL (провалов: 6)
exit=1

$ git diff --check d5163b5b35abbca204a8981e974bd6e5a97eb9de..HEAD
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
```
