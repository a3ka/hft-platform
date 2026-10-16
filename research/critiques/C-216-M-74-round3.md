<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: 4b5bbde6996c8ccfa4de816c7c40a85f5d2d8fee
audited_head: 9367cf1fda1d869ac38bcc6cbc4a2486c3292057
verdict: REJECT
-->

# C-216 — M-74 restore-drill, round 3: REJECT

## Verdict

**REJECT — do not dispatch engine-dev.** This is round 3 of the new count set by
`A-032` §4. The arbitration boundary is binding: this audit does **not** reopen
coverage of alias forms or demand a local network-delivery proof. It finds three
different, executable defects in the committed artifact set.

The touched-module invariant is **JR-I-6**: an old journal must remain readable by
new code (`docs/fa/journal.md` §I). For M-74, the operational proof has to exercise
a healthy restored selection, not merely reject adversarial stand-ins.

## B-1 — H cannot call its required predicate

`H` calls `h_verdict` and then `inv_delivery` at
`scripts/tests/red_restore_drill.sh:326,329`, but Bash has not executed either
function definition yet: they appear at `:527` and `:543`. Bash executes function
definitions in source order. Once the real wrapper exists and returns success, H
therefore emits `command not found`, then takes the `elif ! inv_delivery` branch and
reports a false delivery failure. The current intentionally-absent wrapper masks
this path; D/X/A1/A6/R execute only after the definitions have been reached.

This violates `A-032` §2.2(1): H and the self-checks must judge the one predicate.
It makes a correct implementation unable to turn task 1 green.

**Condition to clear:** a committed RED/positive-control execution with a healthy
wrapper reaches the same `h_verdict`/`inv_delivery` predicate as the adversarial
cases and passes. It must fail before the correction for the shown execution-order
reason, not because a wrapper or reader is absent.

## B-2 — INV-DELIVERY accepts a partial, wrong restore selection

After the B-1 ordering defect is corrected, an implementation can copy only one
compressed segment, read it honestly, write that real digest plus forged
`checked=3`, and pass H. H checks only the self-reported `checked == 3` and that
there is at least one `.zst` file (`red_restore_drill.sh:316-326`). `h_verdict`
requires only a nonempty matching digest and source-independent readability
(`:543-549`); it does not establish that the restore contains the required first,
middle, and last **indices** from the cold selection.

I materialized the committed healthy fixture, retained exactly one zstd segment,
and read it before and after moving the source away. The consumer digest was
identical both times. Thus it satisfies INV-DELIVERY's result property, but not the
milestone's selection contract. This is not an alias-form or local-network finding;
it is a missing required-member check in the selected restore set.

**Condition to clear:** the committed RED suite must reject a wrapper that restores
fewer or different selected indices while producing a genuine digest, surviving
source removal, and reporting `checked=3`. The positive control must establish the
actual three-index selection independently of a wrapper field.

## B-3 — active milestone text still contradicts A-032 and the committed probe

`A-032` §3 required all ten active rev-4 claims to be corrected before this round.
Several remain outside an explicitly historical block:

- `milestones/M-74-restore-drill.md:323-327` names removed `delivery_ok`, requires
  materialisation by filesystem form, and rejects hard links — the opposite of
  `A-032` §2.2's result predicate and its hard-link ruling.
- `:392-395` says the artifacts belong to a future amendment and critic must not be
  dispatched, although this is the dispatched committed round.
- `:586-589` says INV-DELIVERY governs delivery, then immediately assigns delivery
  to F; the same paragraph says that statement was false.
- `:695` presents observer/W coverage and a non-existent probe section as the
  active C-189 closure.
- `:746-749` routes this critic to six `A-028` items, instead of A-032 §5 and
  round 3 of the new count.

These are live contract, closure-map, and handoff instructions, not labelled
history. A dev cannot derive one current artifact set from them. This independently
fails A-032 постановление 4.

**Condition to clear:** remove or explicitly mark these claims as history, and make
the active contract, closure map, route, and handoff consistently name
INV-DELIVERY, A-032 §5, and round 3 of the new count. Re-run the A-032 §3 form grep
against the resulting milestone rather than asserting the count.

## Artifact-set audit

| Required artifact | Result | Evidence |
|---|---|---|
| T2 contracts and signatures | Present | State-file, reader, wrapper, producer, and transport-shim contracts are declared in the milestone; no T1 change. |
| RED tests | **REJECT** | The shell RED probe has B-1/B-2; the fixture itself is 6/6 green. |
| Acceptance script | Present, real | `set -uo pipefail`, counted failures, and nonzero exit were executed; expected open tasks produce `FAIL (15)`. |
| Milestone | **REJECT** | B-3 retains active, mutually contradictory instructions. |
| Scope / boundary C | Pass | Audited range changes only the milestone, arbitration artifact, and allowed probe; no `contracts/**`, `risk/**`, or retention-mode change. |

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-216
exit=0

$ git diff --name-status 4b5bbde..9367cf1
M	milestones/M-74-restore-drill.md
A	research/arbitration/A-032-m74-delivery-proof.md
M	scripts/tests/red_restore_drill.sh
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  обёртки deploy/bin/journal-restore-drill-cron.sh НЕ СУЩЕСТВУЕТ — RED задачи 1 (её вносит engine-dev задачей 2)
PASS  D выдуманный отпечаток ПОЙМАН при ЧЕСТНО восстановленном каталоге
PASS  X подмена каталога восстановления ПОЙМАНА при СОВПАВШЕМ отпечатке
PASS  A1 ссылки вместо байтов ПОЙМАНЫ INV-DELIVERY — при живом источнике отпечаток сходился, без источника читать нечего
PASS  A6 пустая доставка ПОЙМАНА тем же единственным предикатом
PASS  R отпечаток прошлого прогона ОТВЕРГНУТ — фикстура одноразова, признак различает
VERDICT: FAIL (1 из 6)
exit=1

$ bash -c 'if later; then :; elif ! second; then echo functions-not-yet-defined; fi; later(){ :; }; second(){ :; }'
bash: line 1: later: command not found
bash: line 1: second: command not found
functions-not-yet-defined
exit=0

$ partial selected restore, then source moved
restore_indices=1
DRILL_DIGEST events=1364 digest=61a30bc8119158c48e821a0b541aa17a608d1060120a563a117105e2ec046335
DRILL_DIGEST events=1364 digest=61a30bc8119158c48e821a0b541aa17a608d1060120a563a117105e2ec046335
exit=0

$ cargo test -p journal --test fixture_restore_drill_cold --quiet
running 6 tests
......
test result: ok. 6 passed; 0 failed
exit=0

$ bash scripts/verify_M-74.sh
PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
PASS: фикстура прод-формы читается journal::stream (исполнено тестов: 6)
FAIL: bash scripts/tests/red_restore_drill.sh
FAIL: test -x deploy/bin/journal-restore-drill-cron.sh
FAIL: test -f crates/journal/src/bin/journal-drill-read.rs
FAIL: отображение «файл состояния → gauge в рендере /metrics» — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED)
FAIL: просроченный успешный drill ⇒ метрика 0 (и внутри окна ⇒ 1) — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED)
VERDICT: FAIL (15)
exit=1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-08T11:42Z
- Milestone: M-74-restore-drill
- Статус: BLOCKED — REJECT, round 3 of the new count
- HEAD: 9367cf1 — docs(M-74): A-032 постановление 4 — десять живых утверждений rev 4 сняты

## §B — Что я сделал
- Audited committed `4b5bbde..9367cf1`, including A-032, the T2 contracts/signatures, RED artifacts, verify gate, and milestone.
- Reproduced source-independent readability of a one-segment partial restore and checked the shell execution order.

## §C — Артефакты / результаты
- `research/critiques/C-216-M-74-round3.md`
- Done Block: intentional RED exit=1; journal fixture exit=0; partial-selection attack exit=0; verify exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- Push-статус: ✅ pushed to `origin/docs/M-73-closeout-architect` with this verdict commit.
- Кэш: ✅ no `target/` cache was created in the dedicated critic worktree.
- **Paste-ready промпт:**
  ```
  M-74 round 3 is REJECTED by C-216. Read C-216 and A-032. Before round 4, correct the probe so H can execute its single predicate after the real wrapper exists; add a RED oracle for an incomplete/wrong three-index restore with a genuine digest and source removal; and remove or explicitly historical-mark every remaining active rev-4/A-028 claim identified in C-216. Commit the amended artifact set and request the final authorized critic round with new base/head SHAs. Do not reopen alias-form coverage or demand a local network-delivery proof: A-032 settled those.
  ```

## §E — Риски / открытые вопросы
- Round 4 is the last authorized critic round for this count. A same-class REJECT then stops for founder under A-032 §4.
- The partial-selection defect is a different class from pseudonym coverage and needs a new RED oracle before dispatch.

=== END HANDOFF ===
