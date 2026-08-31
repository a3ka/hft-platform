<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: ae5f52f041aa1d5d6d3afba3052398bb71e5040c
audited_head: 68dc5b28d0a7736b73dfea83b9f5066bc8d95be1
verdict: REJECT
-->

# C-191 — M-74 restore-drill r4: REJECT (terminal pre-dispatch round)

## Verdict

**REJECT — engine-dev dispatch remains blocked.** This is the terminal round under
`A-028` §4: this finding leaves the `C-189` B-1 reader-invocation oracle forgeable, so
it is the same pre-dispatch incompleteness class, not a reason to open round 5. The
milestone must return to founder for the choice prescribed by `A-028` §4.

The r4 mutation suite correctly rejects the three named variants: no reader invocation,
wrong `--dir`, and ignored `events_read`. Its new witness is nevertheless forgeable by
the wrapper being judged. `run_drill` gives that wrapper the observer's pathname in
`JOURNAL_DRILL_READER` (`scripts/tests/red_restore_drill.sh:180-187`). The pathname names
a readable shell script which contains the supposedly hidden `WIT='…/calls.tsv'` assignment
(`:160-171`). A wrapper can read that script, append a forged `DIR/RC/EV` row, write matching
state and return code, and never execute the reader.

I reproduced exactly that wrapper. It did not transfer a segment and did not execute
`$JOURNAL_DRILL_READER`; it only derived `WIT` from the supplied observer script and forged
the H/C/F witness rows. The unmodified r4 probe returned `PASS (9/9)`, exit 0. Step W does
not prevent this: it runs the observer directly, so it establishes only that the observer
works when invoked by the probe, not that the wrapper invoked it.

This falsifies the new contractual assertion that the witness location is unavailable to
the wrapper. It also fails the consumer-boundary requirement from R-1: the asserted record
still comes from a file writable by the producer under test. The relevant live invariant is
**JR-I-6** (`docs/fa/journal.md:116`): an old/cold journal is readable by new code; a forged
row is not evidence that the reader read it.

## Artifact-set audit

| Required artifact | Result | Evidence |
|---|---|---|
| T2 contracts / trait signatures | Present | State/producer/reader and wrapper contracts are declared in `milestones/M-74-restore-drill.md:100-291`; no `contracts/**` change is in `ae5f52f..68dc5b2`. |
| RED tests | Present, but B-1 closure fails | `scripts/tests/red_restore_drill.sh` is intentionally RED on the subject tree; the fixture is 5/5 GREEN and the metric target is declared COMPILE-RED. The r4 witness can be forged. |
| Acceptance script | Present and real gate | `scripts/verify_M-74.sh` has `set -uo pipefail`, counted failures and final non-zero exit. It reports the required intentional RED/COMPILE-RED failures. |
| Milestone | Present | `milestones/M-74-restore-drill.md` rev 4 names the r4 contract and the terminal-round rule. |

No T1 or risk path is touched. `check_review_fa.sh` correctly reports `SKIP` for the
test-only crate paths; this does not waive the cognitive FA check above.

## Reproduction

The forged wrapper's material steps were:

```bash
obs=$(printf '%s' "$JOURNAL_DRILL_READER" | sed -n 's/^bash //p')
wit=$(sed -n "s/^WIT='\\(.*\\)'$/\\1/p" "$obs")
# Never execute $JOURNAL_DRILL_READER.
printf 'DIR=%s\tRC=%s\tEV=%s\n' "$JOURNAL_DRILL_RESTORE" "$rc" "$events" >> "$wit"
# Write matching wrapper state and return $rc.
```

The probe accepts the forged H/C/F rows because it reads their last line through
`witness_field`; it has no independent observer outside the wrapper's read/write authority.

## Required disposition

Per `A-028` §4, do not send this back for a fifth critic round and do not dispatch dev.
Founder must choose the prescribed reconstruction of the milestone or transfer of the
artifact set to another executor. This verdict identifies the failure and reproduction;
it deliberately does not design the replacement oracle.

## Done Block

```text
$ git log --oneline ae5f52f..68dc5b2
68dc5b2 spec(M-74): C-189 B-1 — вызов читателя становится КОНТРАКТОМ обёртки [architect]
fe8e7e5 test(M-74): C-189 B-1 — проба наблюдает ВЫЗОВ читателя, а не отчёт обёртки [architect]

$ git diff --name-status ae5f52f..68dc5b2
M	milestones/M-74-restore-drill.md
M	scripts/tests/red_restore_drill.sh

$ bash scripts/tests/red_restore_drill.sh
VERDICT: FAIL (1 из 1) — RED-first: спецификация есть, реализации нет
exit=1

$ cargo test -p journal --test fixture_restore_drill_cold
running 5 tests
test result: ok. 5 passed; 0 failed
exit=0

$ cargo test -p recorder --test red_restore_drill_metric
error[E0432]: unresolved imports `recorder::metric_emit::sample_restore_drill`,
`recorder::metric_emit::RESTORE_DRILL_FRESH_WINDOW_MS`
error: could not compile `recorder` (test "red_restore_drill_metric") due to 1 previous error
exit=101

$ bash scripts/verify_M-74.sh
PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются
PASS: фикстура прод-формы читается journal::stream (исполнено тестов: 5)
FAIL: bash scripts/tests/red_restore_drill.sh
FAIL: отображение «файл состояния → gauge в рендере /metrics» — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED)
FAIL: просроченный успешный drill ⇒ метрика 0 (и внутри окна ⇒ 1) — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED)
VERDICT: FAIL (15)
exit=1

$ honest temporary wrapper + real temporary reader; bash scripts/tests/red_restore_drill.sh
PASS  S фикстура прод-формы: 8 индексов сегментов, из них сжатых файлов 7
PASS  W наблюдатель пишет свидетельство, читатель различает 0 и 5 — оба насквозь
PASS  H здоровая копия ⇒ drill прошёл … читатель ВЫЗВАН (1×) … rc=0
PASS  C повреждённый сегмент ⇒ читатель ВЫЗВАН и вернул rc=4
PASS  E пустое восстановление ⇒ отказ с ОТДЕЛЬНОЙ причиной: empty
PASS  F доставка молча не сработала ⇒ читатель ВЫЗВАН … rc=5
PASS  A оборванное состояние ПЕРЕЗАПИСАНО целиком
VERDICT: PASS (9/9)
exit=0

$ C-189 M9 wrapper (never invokes reader); bash scripts/tests/red_restore_drill.sh
FAIL  H drill отчитался об успехе, НЕ ВЫЗВАВ читатель НИ РАЗУ
FAIL  C drill отказал, НЕ ВЫЗВАВ читатель
FAIL  F drill отказал, НЕ ВЫЗВАВ читатель
VERDICT: FAIL (3 из 9)
exit=1

$ M10 wrapper (reader invoked on cold, not restore); bash scripts/tests/red_restore_drill.sh
FAIL  H читатель вызван на «…/cold», а доставка писала в «…/restore» — прочитан НЕ ТОТ каталог
VERDICT: FAIL (3 из 9)
exit=1

$ M11 wrapper (reader result ignored); bash scripts/tests/red_restore_drill.sh
FAIL  H состояние объявляет 99999 событий, а читатель вернул 10934 — обёртка вызвала читатель и ПРОИГНОРИРОВАЛА его результат
VERDICT: FAIL (2 из 9)
exit=1

$ forged-witness wrapper (does not invoke reader); bash scripts/tests/red_restore_drill.sh
PASS  H здоровая копия ⇒ drill прошёл … читатель ВЫЗВАН (1×) … rc=0
PASS  C повреждённый сегмент ⇒ читатель ВЫЗВАН и вернул rc=4
PASS  E пустое восстановление ⇒ отказ с ОТДЕЛЬНОЙ причиной: empty
PASS  F доставка молча не сработала ⇒ читатель ВЫЗВАН … rc=5
VERDICT: PASS (9/9) — копия читается прод-читателем, исходы различимы
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git merge-base HEAD origin/main) bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (1dda5b1..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git merge-base HEAD origin/main) bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 1dda5b1..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git merge-base HEAD origin/main) bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 4, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git merge-base HEAD origin/main) bash scripts/check_review_fa.sh
SKIP (диапазон трогает ТОЛЬКО не-прод пути крейтов — tests/examples/benches, в прод-образ не входят; C-115 B-1, классификация A-012 §1-Д п.5)
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/next_artifact_id.sh C
C-191
exit=0
```

=== HANDOFF: CRITIC → FOUNDER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31T12:xxZ
- Milestone: M-74-restore-drill
- Статус: BLOCKED — terminal round 4
- HEAD: 68dc5b2 — spec(M-74): C-189 B-1 — вызов читателя становится КОНТРАКТОМ обёртки [architect]

## §B — Что я сделал
- Audited the committed r4 artifacts and reproduced the three claimed mutations.
- Built an independent honest temporary wrapper/reader (probe PASS 9/9) and a new witness-forgery wrapper (probe PASS 9/9 without reader execution).

## §C — Артефакты / результаты
- `research/critiques/C-191-M-74-restore-drill-r4.md`
- Done Block: intentional shell RED exit=1; fixture 5/5 exit=0; metric COMPILE-RED exit=101; M-74 verify exit=1; forged-witness probe exit=0; artifact/design checks exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `founder`
- **Paste-ready промпт:**
  ```
  M-74 is terminally REJECTED by C-191 under A-028 §4. Do not dispatch engine-dev and do not open critic round 5. The C-189 B-1 observer is forgeable: the wrapper receives the observer script path in JOURNAL_DRILL_READER, reads its WIT assignment, forges witness/state, and the unmodified r4 probe passes 9/9 without executing the reader. Choose A-028 §4's prescribed disposition: reconstruct the milestone with a different oracle construction, or transfer the artifact set to another executor.
  ```
- Push-статус: pending critic verdict commit and push to `origin/docs/M-73-closeout-architect`.
- Кэш: pending removal after push.

## §E — Риски / открытые вопросы
- `A-028` §4 forbids a fifth pre-dispatch critic round for this same incompleteness class.
- `RETENTION_MODE` remains `dry-run`; this verdict does not alter boundary C.

=== END HANDOFF ===
