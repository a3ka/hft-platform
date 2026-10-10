<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: b8d989eee53b6aebd425de37280ca5e22d98bb95
audited_head: f47a54a28829215d6c5867d94bf19755b3f1155b
verdict: REJECT
-->

# C-189 — M-74 restore-drill r3: REJECT

## Verdict

**REJECT — dispatch to engine-dev is blocked.** The six prescribed A-028 §3 closures are
present, and I do not reopen the settled question of when the recorder oracle was written,
the C-188 B-3/B-4 findings, `chk` self-check, or boundary C. This REJECT is a new oracle
class: the shell probe does not prove that the wrapper invokes the declared drill reader or
derives its state from that reader's result.

## A-028 §3 closure audit

| A-028 item | Result | Evidence |
|---|---|---|
| 1. Recorder test path allowed | Closed | `milestones/M-74-restore-drill.md:265-278` allows `crates/recorder/tests/**`; the fixture and planned reader paths are also explicit. |
| 2. Fixture is prod-form and read by the real reader | Closed | `fixture_restore_drill_cold` builds with `Journal` and compaction; the independent run is 5/5 GREEN. The live measurements below reproduce the empty manifest, compressed oldest segment, absent `journal.meta`, and 17 duplicated indices. |
| 3. Producer signature declared before dispatch | Closed | The literal `sample_restore_drill` / freshness-constant signature and its fail-closed table are in the milestone. `red_restore_drill_metric.rs` is committed COMPILE-RED against it. |
| 4. Recorder oracle committed before dispatch | Closed | The RED target exists and presently fails specifically on the two absent imports, not on zero selected tests. |
| 5. Three-way verification for tasks 3/5 | Closed | `chk_named_test` counts selected tests and diagnoses COMPILE-RED separately from vacuum; its self-probe is GREEN and both M-74 steps report COMPILE-RED. |
| 6. Emission canary bound to the call | Closed | `verify_M-74.sh:119-125` searches `set_gauge("backup_restore_drill_ok"...)`, not the previously misleading deferred comment. |

## New blocking finding — B-1: the wrapper can forge every probe result without a reader

`scripts/tests/red_restore_drill.sh` assigns a direct real-reader command to
`JOURNAL_DRILL_READER` (`:109-117`), but its runtime assertions never prove that the wrapper
executes that command, consumes its JSON, or obtains `events_read` from it. `H` accepts only
the wrapper-written state fields and the existence of a `.zst` name in `restore` (`:162-179`).
`C`/`E`/`F` likewise trust the wrapper's chosen exit code and `reason` (`:180-238`). This
measures a claim from the producer under test, not the declared reader at the consumer
boundary: R-1 and `testing.md`'s gate-integrity property 2.

I reproduced a ninth adversarial mutation in an isolated mirror of the audited tree. Its
`journal-restore-drill-cron.sh` does **not** transfer, open, decompress, or invoke
`$JOURNAL_DRILL_READER`. It prints the required argv under `HFT_CRON_PRINT_ARGV`, recognizes
the probe's empty and mode-000 fixtures, and for the fixed H→C order forges respectively a
healthy state then `rc=4`; the atomic-write case is recognized only by the preloaded canary.
It merely `touch`es a `.zst`-named restore file. The unmodified probe returned `PASS (8/8)`,
shown verbatim below. Thus it also passes when it reads the wrong segment form — in fact, it
does not read any form at all.

This is not one of architect's M1–M8 mutations and is not a dispute about RED timing. It is a
new fact: the asserted contract has three signatures, but the reader and env-composition
signatures lack an oracle that observes their runtime connection.

**Condition for the next plan-time round:** add a RED mutation that makes the wrapper ignore
the supplied reader (or point it at a different directory/form) and demonstrate that `H`
becomes red. The proof must retain the direct real reader, observe its invocation/result, and
tie the emitted state to that result. The exact test construction is architect's design work.

The relevant live FA invariant is **JR-I-6**: old and compressed segment forms remain readable
by the real reader; a wrapper-produced success bit is not evidence of that property.

## Done Block

```text
$ git log --oneline b8d989e..f47a54a
f47a54a style(M-74): cargo fmt на фикстуре — паритет с CI шага task #0 [architect]
36eb667 spec(M-74): A-028 пп.1-3 — форма копии снята ЗАМЕРОМ, сигнатуры объявлены ДОСЛОВНО [architect]
b1ea617 fix(M-74): A-028 пп.5-6 — шаги задач 3/5 трёхисходны, канарейка привязана к ВЫЗОВУ [architect]
842e87c test(M-74): A-028 п.4 — оракул продюсера метрики НАПИСАН и закоммичен ДО dispatch (COMPILE-RED) [architect]
e542e0d test(M-74): A-028 п.2 — фикстура ПРОД-ФОРМЫ строится прод-писателем и принимается прод-читателем [architect]

$ ssh … 'cat journal.legacy.json; oldest segment' ; sftp-via-VPS …
{
  "declarations": []
}
segment-00000001.jrnl.zst
local_exit=0
storage: raw=22 zst=478 paired_indices=17 journal_meta_present=0
storage_transport_exit=0

$ cargo test -p journal --test fixture_restore_drill_cold
running 5 tests
test materialize_for_shell_probe ... ok
test empty_restore_yields_zero_events_not_an_error ... ok
test undeclared_legacy_is_a_context_error_not_a_corruption_error ... ok
test corrupted_cold_copy_is_rejected_by_real_reader ... ok
test prod_form_cold_copy_is_read_by_real_reader ... ok
test result: ok. 5 passed; 0 failed
exit=0

$ bash scripts/tests/red_restore_drill.sh
── RESTORE-DRILL: обёртка ещё не внесена
FAIL  обёртки deploy/bin/journal-restore-drill-cron.sh НЕ СУЩЕСТВУЕТ — RED задачи 1
VERDICT: FAIL (1 из 1) — RED-first: спецификация есть, реализации нет
exit=1

$ cargo test -p recorder --test red_restore_drill_metric
error[E0432]: unresolved imports `recorder::metric_emit::sample_restore_drill`,
`recorder::metric_emit::RESTORE_DRILL_FRESH_WINDOW_MS`
error: could not compile `recorder` (test "red_restore_drill_metric")
exit=101

$ bash scripts/verify_M-74.sh
PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются
PASS: фикстура прод-формы читается journal::stream (исполнено тестов: 5)
FAIL: отображение «файл состояния → gauge в рендере /metrics» — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED)
FAIL: просроченный успешный drill ⇒ метрика 0 (и внутри окна ⇒ 1) — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED)
VERDICT: FAIL (15)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [H-FACTS-SHA] маркеров `FACTS:` проверено 8 — все ревизии сбора существуют и входят в историю HEAD/MERGE_HEAD
PASS  [7-RFC-PATH] путей-кандидатов: всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ bash /tmp/m74-critic-adversary-fhAu8h/scripts/tests/red_restore_drill.sh
PASS  S фикстура прод-формы: 8 индексов сегментов, из них сжатых файлов 7
PASS  P argv печатает КОМПОЗИЦИЮ: доставка пишет в тот же каталог, из которого читает читатель
PASS  P печать argv не произвела побочных эффектов
PASS  H здоровая прод-форменная копия ⇒ drill прошёл: ok=1, прочитано событий 41213, сегментов 3, сжатых в выборке 1
PASS  C повреждённый сегмент ⇒ отказ rc=4 (ЧТЕНИЕ), ok=0, причина: read-error
PASS  E пустое восстановление ⇒ отказ с ОТДЕЛЬНОЙ причиной: empty
PASS  F доставка молча не сработала ⇒ отказ rc=5 (ПУСТОТА), ok=0
PASS  A оборванное состояние ПЕРЕЗАПИСАНО целиком (канарейка обрывка исчезла, объект ровно один)
VERDICT: PASS (8/8) — копия читается прод-читателем, исходы различимы
exit=0

$ bash scripts/next_artifact_id.sh C
C-189
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31T10:57Z
- Milestone: M-74-restore-drill
- Статус: BLOCKED
- HEAD: f47a54a — style(M-74): cargo fmt на фикстуре — паритет с CI шага task #0 [architect]

## §B — Что я сделал
- Проверил только закрытия A-028 §3 и новую находку другого класса.
- Воспроизвёл форму боевой/холодной копии и построил девятую adversarial-мутацию, которую проба пропускает.

## §C — Артефакты / результаты
- `research/critiques/C-189-M-74-restore-drill-r3.md`
- Done Block: fixture exit=0; RED probe exit=1; recorder COMPILE-RED exit=101; M-74 verify exit=1; merge-preview exit=0; adversarial probe exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-74 был REJECT в C-189: `red_restore_drill.sh` допускает wrapper, который не вызывает
  JOURNAL_DRILL_READER и не читает сегмент, но подделывает state/rc и получает PASS 8/8.
  Не переоткрывай время написания COMPILE-RED, C-188 B-3/B-4, chk или границу C. Спроектируй
  RED-мутацию: игнорирование reader'а либо чтение иного каталога/формы делает H красным,
  при сохранении прямого реального reader. Закоммить artifact set и верни новый commit-chain
  для круга 4 критика.
  ```
- Push-статус: будет указан после коммита этого verdict-файла на `origin/docs/M-73-closeout-architect`.
- Кэш: будет убран после push.

## §E — Риски / открытые вопросы
- Это круг 3 из максимум 4 по pre-dispatch полноте; повторная неполнота на круге 4 требует STOP по A-028 §4.
- FA для `recorder` отсутствует; предъявлен применимый журналовый инвариант JR-I-6.

=== END HANDOFF ===
