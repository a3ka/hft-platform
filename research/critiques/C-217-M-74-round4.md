<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: 8f8e4b606e176b26e11ce84f8398852a9a437c13
audited_head: 2d322f80a9da25f1b5eb5e8dbcd4da50e087b1fd
verdict: REJECT
-->

# C-217 — M-74 restore-drill, round 4: REJECT — STOP

## Verdict

**REJECT — STOP; do not dispatch engine-dev and do not open round 5.** This is round 4 of 4 under `A-032` §4. The range closes C-216 B-1 and B-3, and its per-member guard mutations behave exactly as declared. It does **not** close C-216 B-2: a wrapper can restore exactly three genuine, source-independent indices while selecting the wrong middle and last indices. `delivery_complete` proves only cardinality and a self-reported count; it never proves the membership required by the active selection contract.

This is a finding that the artifact set is incomplete, not a request for a new alias-form check or a local network-delivery proof. Those are explicitly settled by `A-032` §2.2/§4 and are not reopened here. Per `A-032` §4, the subject returns to **founder** for the pre-agreed handoff to a different executor on the same rev-5 construction.

The affected live invariants are **JR-I-6** (old journal remains readable by new code) and **OPS-I-3** (the cold copy is restored and read). The range changes `crates/journal/**`, so this verdict names the live FA invariant JR-I-6; no waiver is used.

## C-216 disposition

| C-216 item | Result | Evidence |
|---|---|---|
| B-1 — H must reach its predicate | CLOSED | The committed run executes H successfully through `h_verdict`; the former definition-order failure is no longer masked. |
| B-2 — partial/wrong delivery | **NOT CLOSED — REJECT** | A7 catches one index, but a three-index selection with wrong members passes every probe assertion. |
| B-3 — live milestone assertions | CLOSED | Form grep found the five formerly-live forms only as explicit history, removal, or current negation; the active route says round 4 / A-032. |

### B-2 — correct count is not the declared selection

The selection contract requires the sorted, deduplicated **first**, `floor(N/2)`, and **last** indices (`milestones/M-74-restore-drill.md:452-460`). In contrast, `delivery_complete` at `scripts/tests/red_restore_drill.sh:320-328` verifies only `checked >= 3` and `delivered-index-count == checked`.

I mutated only the test reference wrapper's selection from `1/MID/N` to `1/2/3`. On the committed fixture the eight indices are `00000000` through `00000007`, so the specified members are positions `1/4/8` (`00000000/00000003/00000007`) while the mutant copies `00000000/00000001/00000002`. It produces a genuine reader digest, retains source-independent readability, reports `checked=3`, and passes H, C, A7, and every other oracle; the full probe remains at its two declared RED lines only.

This is exactly the candidate attack named in the round-4 invocation. It is inside scope: it tests the milestone's own selection contract, rather than a filesystem pseudonym form or transport network path.

**Condition to clear:** an executed RED oracle must reject a wrapper that restores three real, readable indices but not the required first/middle/last selection. Its setup guard must demonstrate the wrong members, its digest must be genuine, and its source-independent read must succeed, so rejection is attributable to member identity rather than completeness, digest, or delivery.

### N-1 — the reference reader does not implement the declared success protocol

The reader signature declares one success JSON object with `segments_read`, `events_read`, `seq_first`, `seq_last`, `intra_segment_continuous`, and `digest` (`milestones/M-74-restore-drill.md:247-249`). The reference reader emitted by `make_reference_reader` outputs only `events_read` and `digest` (`scripts/tests/red_restore_drill.sh:463-482`). The independent source reports the same reduced fields (`crates/journal/tests/fixture_restore_drill_cold.rs:381-386`). Thus the probe's current green reference path judges a convenient subset of the declared reader contract, even though the milestone calls it the declared contract.

The reference does match the declared digest formula and distinguishes currently exercised `0`/`4`/`5` outcome classes; this finding is the missing required success fields, not a claim that those three exercised codes are wrong. The declared `6` context outcome is also not represented by the reference's error classification, so it cannot be represented as already covered.

**Condition to clear:** make the reference reader and its shell assertion produce and validate the declared success protocol (or revise the declared protocol through the architect-owned milestone artifact). The result must be executed before dispatch, not asserted by a comment.

## Boundary checks that passed

- `INV-DELIVERY` remains result-based: the X/A1 source-retention mutation newly fails exactly X and A1. Hard links remain accepted, as required by `A-032`.
- The `digest_matches_state` mutation newly fails exactly D and R; the `delivery_complete` mutation newly fails exactly A7. A6 is honestly documented as overdetermined.
- The five C-216 B-3 forms were grepped: `delivery_ok`/materialisation-and-hardlink claim; future-amendment/no-dispatch claim; `F` as delivery guard; observer/W coverage; and A-028 round-route claim. Their remaining appearances are labeled history or negate the old behavior; no active conflicting instruction was found.
- The two-commit range contains the complete required plan-time set: T2 state/wrapper/reader contracts in the milestone, architect-owned fixture and RED probe, a counted nonzero acceptance script, and the milestone. It changes only the allowed fixture, probe, and milestone; no T1 or boundary-C path is touched.

## Done Block

```text
$ git log --oneline 8f8e4b6..2d322f8
2d322f8 fix(M-74): C-216 B-3 дозакрыт + ИНЕРТНЫЙ СТРАЖ, найденный собственной мутацией [architect]
ab803d2 fix(M-74): C-216 — закрыт КОРЕНЬ серии: девять сценариев не исполнялись НИ РАЗУ [architect]
exit=0

$ git diff --name-status 8f8e4b6..2d322f8
M	crates/journal/tests/fixture_restore_drill_cold.rs
M	milestones/M-74-restore-drill.md
M	scripts/tests/red_restore_drill.sh
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  обёртки deploy/bin/journal-restore-drill-cron.sh НЕ СУЩЕСТВУЕТ — RED задачи 1 (её вносит engine-dev задачей 2)
FAIL  N ПРОД-ЧИТАТЕЛЬ не выдал отпечаток (бинаря journal-drill-read ещё нет — задача 2b). Независимый эталон при этом работает
PASS  H здоровая копия ⇒ drill прошёл: ok=1, событий 4125, сегментов 3, сжатых в выборке 2; отпечаток состояния СОВПАЛ с отпечатком прод-читателя по восстановленному каталогу
PASS  C повреждённый сегмент ⇒ drill отказал rc=4, ok=0, отпечатка НЕТ; причина: reader-rc-4
PASS  T шов транспорта: заглушка вызвана (1×) с композицией cold→restore; её отказ даёт ok=0, rc=5, без отпечатка
PASS  D выдуманный отпечаток ПОЙМАН при ЧЕСТНО восстановленном каталоге
PASS  X подмена каталога восстановления ПОЙМАНА при СОВПАВШЕМ отпечатке
PASS  A1 ссылки вместо байтов ПОЙМАНЫ INV-DELIVERY
PASS  A7 частичная доставка ПОЙМАНА: заявлено checked=3, доставлен 1 индекс; отпечаток при этом ПОДЛИННЫЙ
PASS  R отпечаток прошлого прогона ОТВЕРГНУТ — фикстура одноразова, признак различает
VERDICT: FAIL (2 из 18)
red_restore_drill_exit=1

$ mutation digest_matches_state() { true; }
FAIL  D обёртка, восстановившая копию, но ВЫДУМАВШАЯ отпечаток, признана честной
FAIL  R отпечаток ЧУЖОГО прогона принят как свой
VERDICT: FAIL (4 из 18)
digest_mutation_exit=1

$ mutation delivery_complete() { true; }
FAIL  A7 обёртка, доставившая ОДИН индекс вместо трёх, признана честной
VERDICT: FAIL (3 из 18)
delivery_complete_mutation_exit=1

$ mutation inv_delivery() { source retained; }
FAIL  X обёртка, подменившая каталог восстановления ССЫЛКОЙ на холодную копию, признана честной
FAIL  A1 обёртка, положившая ССЫЛКИ вместо байтов, признана честной
VERDICT: FAIL (4 из 18)
inv_delivery_mutation_exit=1

$ mutation reference selection: IDX=1/2/3, while the contract requires 1/4/8
PASS  H здоровая копия ⇒ drill прошёл: ok=1, событий 4131, сегментов 3, сжатых в выборке 3; отпечаток состояния СОВПАЛ с отпечатком прод-читателя по восстановленному каталогу
PASS  C повреждённый сегмент ⇒ drill отказал rc=4, ok=0, отпечатка НЕТ; причина: reader-rc-4
PASS  A7 частичная доставка ПОЙМАНА: заявлено checked=3, доставлен 1 индекс; отпечаток при этом ПОДЛИННЫЙ
VERDICT: FAIL (2 из 18)
wrong_members_mutation_exit=1

$ bash reference-reader.sh --dir cold --min-events 1
{"events_read":10949,"digest":"0a9d3b3feb6074c8d3a516da1b662a8a26d1221261a58c054305ad130d502a35"}
reference_reader_exit=0

$ bash scripts/verify_M-74.sh
PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
PASS: фикстура прод-формы читается journal::stream (исполнено тестов: 7)
FAIL: bash scripts/tests/red_restore_drill.sh
FAIL: test -x deploy/bin/journal-restore-drill-cron.sh
FAIL: test -f crates/journal/src/bin/journal-drill-read.rs
FAIL: test -f deploy/cron.d/journal-restore-drill
VERDICT: FAIL (15)
verify_M74_exit=1

$ bash scripts/next_artifact_id.sh C
C-217
allocator_exit=0

$ EVENT_NAME=push PUSH_BEFORE=8f8e4b606e176b26e11ce84f8398852a9a437c13 bash scripts/check_artifact_ids.sh
OK: ни один новый артефакт не введён (диапазон 8f8e4b6..HEAD)
artifact_ids_baseline_exit=0

$ git push origin HEAD:docs/M-73-closeout-architect
To https://github.com/a3ka/hft-platform.git
   2d322f8..1bdff21  HEAD -> docs/M-73-closeout-architect
exit=0

$ git ls-remote --heads origin docs/M-73-closeout-architect
1bdff21fe6e82b5e011fb634424e4feff5d52c6a	refs/heads/docs/M-73-closeout-architect
exit=0
```

=== HANDOFF: CRITIC → FOUNDER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-08T21:20Z
- Milestone: M-74-restore-drill
- Статус: BLOCKED — final authorized round REJECT
- HEAD: 2d322f8 — fix(M-74): C-216 B-3 дозакрыт + ИНЕРТНЫЙ СТРАЖ, найденный собственной мутацией [architect]

## §B — Что я сделал
- Audited the committed two-commit artifact set and executed the reference wrapper/reader, C-216 checks, guard mutations, and the wrong-member attack.
- Confirmed this REJECT does not reopen A-032-settled alias-form or local-network questions.

## §C — Артефакты / результаты
- `research/critiques/C-217-M-74-round4.md`
- Done Block: committed probe exit=1 (two declared open production artifacts); each prescribed mutant exit=1 with its stated newly exposed members; wrong-member attack exit=1 only from the same two declared RED lines; verify exit=1 for open tasks.

## §D — Следующий агент + инвокация
- **Следующий агент:** `founder`
- Push-статус: ✅ pushed to `origin/docs/M-73-closeout-architect` at `1bdff21`.
- Кэш: ✅ critic cache removed after push (`/tmp/hft-critic-m74-r4/target`, 9.0G).
- **Paste-ready промпт:**
  ```
  M-74 final plan-time round is REJECT by C-217. A-032 §4 forbids round 5. Do not dispatch engine-dev. Preserve rev-5 INV-DELIVERY construction; move the artifact set to the agreed different executor. The blocking evidence is executable: a wrapper selects exactly three genuine source-independent indices (00000000/00000001/00000002) instead of the required first/middle/last (00000000/00000003/00000007), yet the full probe retains only its two declared RED lines. The reference reader also emits only events_read/digest rather than the declared full success JSON. Require new architect-owned RED artifacts before any new gate count is started.
  ```

## §E — Риски / открытые вопросы
- The current reference implementation is accepted only as the same rev-5 construction; it is not a reason to change the arbitration boundary.
- Founder coordination is required by the pre-set terminal rule, not by a new boundary-C decision.

=== END HANDOFF ===
