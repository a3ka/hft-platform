<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: 78097db259b9a122f18c035e3eeb5ae733e1fe51
audited_head: 113eaf931ca15174cd108645c83ea139559b1450
verdict: REJECT
-->

# C-311 — M-99 restore-drill automation, round 3: REJECT

## Verdict

**REJECT — do not dispatch a repair as if the new RED oracles were complete.**
The head contains the intended round-2 artifact set and its expected RED failures, but
three adjacent production constraints remain unproved by the new oracles. Two are
directly mutation-reproduced below:

1. `RW` accepts a remote transcript that first fetches one allowed file and then deletes
   `cold`; therefore it does not prove the Storage Box is read-only.
2. `v5` accepts the required narrow recorder mount together with a forbidden broad
   `/var/lib/hft` mount; therefore it does not prove the state directory is `:ro` **only**
   in `recorder`.
3. `CR` executes the literal cron command, but proves only that cron has no `flock` and
   that one run succeeds. It never proves that the wrapper owns the one required lock.
   The no-lock mutant completed successfully, so two overlapping crons are again
   possible.
4. The plan-time subject lock itself is red: after the passing C-306 NOTE, the branch
   changed `scripts/tests/red_restore_drill.sh` and `scripts/verify_M-99.sh` without an
   `ALLOW-SUBJECT-CHANGE` audit-trail line.

These are not objections to the planned implementation being RED. They are defects in
the plan-time executable specification: an implementation can satisfy the proposed
positive tests while violating M-99 §4 p.7/8/10 and §7's forbidden boundaries.

## What closes and what does not

### B-1 / transport and Storage Box

The two-operation design itself is correct: §4 p.7 specifies remote `ssh … ls <path>`
plus `rsync` of a named member, and local `ls`/`cp`; the reference wrapper expresses that
split. The ssh stub records argv, executes only the command after the transport boundary,
and thus is a valid local replacement for the network seam. `T` distinguishes no seam
invocation from transport failure, and `RT` calls the same `h_verdict` as `H`
(`scripts/tests/red_restore_drill.sh:844,1046`). `IM` also correctly compares the parsed
`image:` value of the `recorder` compose service, not a substring containing the container
name (`scripts/tests/red_restore_drill.sh:1114-1124`). The current old implementation
acts as the negative mutants for T/RT/IM: it did not invoke the ssh stub, its remote
branch failed closed at `transport-failed`, and it defaulted to `hft-recorder` instead of
`hft-platform-recorder:local`.

But `RW` only requires at least one `ls cold` and one permitted-looking rsync source
(`scripts/tests/red_restore_drill.sh:1054-1068`). It does not reject an extra ssh command.
The exact predicate accepted this mutant transcript:

```text
ls cold
rsync --server -logDtpre.iLsfxCIvu . cold/segment-00000003.jrnl.zst
rm -rf cold
```

That is a direct violation of the Storage Box read-only boundary, while the fetch remains
single-file and all current RW conditions stay true. `verify_M-99.sh:105-106` catches
only two textual spellings (`--delete` and `rm … JOURNAL_DRILL_COLD`), not a constructed or
remote destructive command. The complete remote-command transcript must be allow-listed
or an equivalent executable negative test must reject every command other than the declared
listing and named-file reads.

### B-2 / cron lock

§4 p.8 requires *one* non-blocking lock in the wrapper and none in the cron line
(`milestones/M-99-restore-drill-automation.md:72`). `CR` faithfully executes fields 7+
of the literal cron entry (`scripts/tests/red_restore_drill.sh:1071-1106`), and
`verify_M-99.sh:127-129` confirms the cron line has no `flock`. Neither asserts
`exec 9>`/`flock -n` in the wrapper nor runs a concurrent invocation.

The isolated executable mutant with the wrapper lock block and the cron `flock` removed
reported a successful state:

```text
mutant=wrapper-without-flock+cron-without-flock rc=0
state={"ok":1,...,"checked":3,"events_read":4120,...}
```

Thus CR would accept a schedule with no serialization at all. The repair must make a
missing wrapper lock red (preferably by holding the declared lock and proving the second
wrapper run is skipped) while retaining the literal cron execution test.

### B-3 / reader image

The form is adequate: IM reads the true `recorder` service `image:` from
`docker-compose.yml` and compares the final `READER` argv word exactly. The current
`hft-recorder` default was rejected by the executed probe, so this is not a substring
placebo. No finding here.

### B-4 / state visibility and mount scope

`v3` correctly makes the desired state path a RED contract. `v5` reads the real compose
file, but its assertion is existential: `block.iter().any(|l| v == want)`
(`crates/ops/tests/red_m99_restore_drill_watch.rs:309-318`). It neither counts mounts nor
examines other service blocks. A compose block containing both
`/var/lib/hft/drill-state:/var/lib/hft/drill-state:ro` and
`/var/lib/hft:/var/lib/hft:ro` is accepted by that predicate. The latter leaks restore
data and `retention.mode`, prohibited by M-99 §7
(`milestones/M-99-restore-drill-automation.md:157-160`). The same gap permits the state
mount in any additional service.

The v5 mutation reproduced the false acceptance:

```text
v5_mutant_expected=/var/lib/hft/drill-state:/var/lib/hft/drill-state:ro extra_forbidden=/var/lib/hft:/var/lib/hft:ro mounted=1
v5_mutant_result=ACCEPTED
```

Require an exact-all-services composition oracle: exactly one declared state bind mount,
in `recorder`, read-only at the same path, and no other source rooted at `/var/lib/hft`.
It must fail for the broad-mount and second-service mutations.

## v4 and anti-placebo status

`v4_fields_and_priority_follow_spec_table` covers the required T2 cases: mandatory
`ok`/`ts_wall_ms`/`checked`, `events_read` priority, uppercase hex, future timestamps, and
`i64::MIN` (`crates/ops/tests/red_m99_restore_drill_watch.rs:195-275`). The current
implementation is correctly RED on missing `checked`; no deviation from M-99 §5 was found
in the proposed v4 oracle.

The artifact set says that a cancelled prototype obtained `28/28` and `9/9`, but contains
no committed executable mutation record for the five new cases. It cannot substitute for
the two negative mutations above. Before a new critic pass, the architect must provide
reproducible evidence that T, RT, RW, CR, and IM each reject its named mutant; the actual
RW and CR mutants here show why that evidence is necessary.

## gates.md §9

- **(a) Code claims:** `bash scripts/verify_design_claims.sh --merge-preview origin/main`
  passed at the audited merge tree. This closes C-298's stale OPS-I-count issue, but the
  pass does not establish the two executable oracle properties above.
- **(b) Authority:** `9f4bfde`, `6c8db81`, and `2e9f6a4` carry `[architect]`; their changed
  files are architect-owned RED, gate, and milestone paths. No `crates/contracts/**`,
  implementation source, retention mechanism, or Boundary-C decision changed. But
  `check_gate_meta.sh` rejects the otherwise valid ownership: the two gate paths were
  changed after C-306's passing NOTE without `ALLOW-SUBJECT-CHANGE`. Path authority is not
  an exemption from the subject lock.
- **(c) Linkage:** the design-claim link check passed; `git diff --check` over the
  subject-only first-parent range was clean; protected-artifact and docs-freeze checks
  passed. The range is evaluated as `78097db2..HEAD^1`, deliberately excluding the merge
  parent from `origin/main`.

The live module invariant named for this audit is **OPS-I-3**: the cold copy must be
periodically restored and read. A broad mount or writable/destructive remote action makes
that check itself a production risk rather than evidence for it. **JR-I-6** remains the
reason the copied historical journal has to be read by current code.

## Conditions to clear

1. Add executable, mutation-backed rejection of every unexpected remote transport command
   and prove Storage Box access is read-only while preserving single-file selection.
2. Extend CR/verify with a positive wrapper-lock oracle and a concurrent/no-wrapper-lock
   mutant; retain the cron line free of `flock`.
3. Strengthen v5 to assert exclusivity across the complete real compose file; demonstrate
   both the broad `/var/lib/hft` and additional-service mount mutants fail.
4. Add an `ALLOW-SUBJECT-CHANGE: <reason>` line to a branch commit after C-306, naming the
   round-2 changes to the two gate paths, and show `check_gate_meta.sh` green.
5. Commit the resulting artifact set and its reproducible five-scenario mutation evidence,
   then request a fresh critic pass. Implementation remains out of scope until that pass.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-99-restore-drill-automation
113eaf931ca15174cd108645c83ea139559b1450
exit=0

$ git merge-base --is-ancestor 113eaf931ca15174cd108645c83ea139559b1450 origin/feat/M-99-restore-drill-automation
exit=0

$ git diff --stat 78097db2..HEAD^1 -- milestones/ scripts/ ':(glob)crates/*/tests/**'
 crates/ops/tests/red_m99_restore_drill_watch.rs | 149 ++++++++++++++++-
 milestones/M-99-restore-drill-automation.md     |  30 +++-
 scripts/tests/red_restore_drill.sh              | 205 +++++++++++++++++++-----
 scripts/verify_M-99.sh                          |  18 ++-
 4 files changed, 353 insertions(+), 49 deletions(-)
exit=0

$ bash scripts/tests/red_restore_drill.sh
── СУДИМАЯ ОБЁРТКА: прод (deploy/bin/journal-restore-drill-cron.sh)
── RESTORE-DRILL: копия ЧИТАЕТСЯ прод-читателем, а не только существует
PASS  S фикстура прод-формы: 8 индексов сегментов, из них сжатых файлов 7; объявленная выборка — 00000000 00000003 00000007; пара raw+zst лежит на ВЫБРАННОМ индексе 00000003
PASS  N оба источника эталона дали ОДИН отпечаток (b4f8d388c229…) — формула согласована
PASS  N нонс прогона уникален и недоступен обёртке до чтения — отпечаток нельзя назвать заранее
PASS  P argv печатает КОМПОЗИЦИЮ: доставка пишет в тот же каталог, из которого читает читатель
PASS  P печать argv не произвела побочных эффектов
PASS  H здоровая копия ⇒ drill прошёл: ok=1, событий 4125, сегментов 3, сжатых в выборке 2; отпечаток состояния СОВПАЛ с отпечатком прод-читателя по восстановленному каталогу (6f54b121bf96…)
PASS  C повреждённый сегмент ⇒ drill отказал rc=4, ok=0, отпечатка НЕТ; причина: reader-rc-4 (corrupt — порча байт/crc/zstd)
PASS  E пустое восстановление ⇒ отказ с ОТДЕЛЬНОЙ причиной: cold-empty
PASS  F доставка молча не сработала ⇒ drill отказал rc=5 (ПУСТОТА), ok=0, отпечатка нет
PASS  A оборванное состояние ПЕРЕЗАПИСАНО целиком (канарейка обрывка исчезла, объект ровно один)
FAIL  T ветка remote НЕ ПРОШЛА ЧЕРЕЗ ШОВ: заглушка ssh не вызвана ни разу (JOURNAL_DRILL_SSH). «Не вызвана» ОТЛИЧИМО от «вызвана и отказала» (testing.md св. 4), и здесь первое
FAIL  RT ветка remote при УСПЕШНОМ транспорте НЕ ПРОШЛА (rc=5, ok=0, причина «transport-failed», в restore файлов: 0). На проде drill не завершится успехом ни разу (R-263 Б-1)
FAIL  RW ветка remote не прошла через ОБЕ операции шва (листинг 0×, выборка rsync 0×) — journal argv:
FAIL  CR строка расписания в форме cron'а НЕ исполнила drill (rc=0, ok=, лог: 2026-10-13T17:00:46Z SKIP предыдущий прогон ещё работает). По расписанию drill не исполнится ни разу (R-263 Б-2)
FAIL  IM читатель по умолчанию зовётся из «hft-recorder», а прод поднимает recorder из «hft-platform-recorder:local» — такого образа на сервере нет (R-263 Б-3)
PASS  D выдуманный отпечаток ПОЙМАН при ЧЕСТНО восстановленном каталоге (эталон b17621c0d193… против объявленного обёрткой) — тем же предикатом, что судит честный прогон
PASS  X подмена каталога восстановления ПОЙМАНА при СОВПАВШЕМ отпечатке (f62dbfbb728e…) — сработала проверка доставки, а не величины
PASS  A1 ссылки вместо байтов ПОЙМАНЫ INV-DELIVERY — при живом источнике отпечаток сходился (c177e8ce6190…), без источника читать нечего
PASS  A6 пустая доставка ПОЙМАНА тем же единственным предикатом
PASS  A7 частичная доставка ПОЙМАНА: заявлено checked=3, доставлен 1 индекс; отпечаток при этом ПОДЛИННЫЙ (a28b4e730b13…) — сработала полнота, а не величина
PASS  A8 НЕ ТЕ члены выборки ПОЙМАНЫ: доставлено «00000000 00000001 00000002» вместо «00000000 00000003 00000007» при ПОДЛИННОМ отпечатке (5f4ef41c61ee…), сошедшемся счёте и живом INV-DELIVERY — сработало ТОЖДЕСТВО состава, а не число, величина или доставка
PASS  A9 ЛОЖЬ В ОТЧЁТЕ ПОЙМАНА: состав верен («00000000 00000003 00000007»), отпечаток подлинный, но заявлено checked=99 при 3 доставленных — сработало согласие отчёта
PASS  Q читатель говорит ПОЛНЫМ объявленным протоколом (шесть полей) и на здоровом несмежном восстановлении объявляет непрерывность ВНУТРИ сегментов: true
PASS  A10 НЕПРИВЕЗЁННАЯ ФОРМА ПОЙМАНА: индекс 00000003 несёт в копии raw+zst, доставлена только сжатая; состав, счёт, отпечаток (ffcab08a5d40…) и INV-DELIVERY при этом ЦЕЛЫ
PASS  A11 возврат источника в окне чтения ПОЙМАН стражем окна (rebound-stash) при верном составе, полных формах и подлинном отпечатке (9d8976e54472…)
scripts/tests/red_restore_drill.sh: line 546: /tmp/red-drill-GYUr0K/restore/journal.replay-digest.json: No such file or directory
PASS  A12 sidecar НЕ ПРИВЕЗЁН — ПОЙМАН побайтной сверкой при верном составе, формах, подлинном отпечатке (6c0d7a6288cd…) и живом INV-DELIVERY
PASS  A13 sidecar ПОДМЕНЁН — ПОЙМАН побайтной сверкой при верном составе, формах, подлинном отпечатке (223d572d15a7…) и живом INV-DELIVERY
PASS  R отпечаток прошлого прогона ОТВЕРГНУТ — фикстура одноразова, признак различает

VERDICT: FAIL (5 из 28)
exit=1

$ cargo test -p ops --test red_m99_restore_drill_watch
test v3_default_state_path_matches_wrapper_contract ... FAILED
test v4_fields_and_priority_follow_spec_table ... FAILED
test v5_state_dir_is_mounted_into_recorder_at_same_path_read_only ... FAILED
test result: FAILED. 6 passed; 3 failed
exit=101

$ RW/v5 exact-predicate mutations
rw_fetch=1 rw_list=1 rw_bad=<>
RW_mutant_result=ACCEPTED
v5_mutant_expected=/var/lib/hft/drill-state:/var/lib/hft/drill-state:ro extra_forbidden=/var/lib/hft:/var/lib/hft:ro mounted=1
v5_mutant_result=ACCEPTED
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: OPS-I — заявлено=11, в оракулах=10 — подтверждено замером (loose=10)
PASS  [3-ССЫЛКИ] все 8 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 661 ссылок вида docs/*.md указывают на существующие файлы
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/check_protected_artifacts.sh "$(git merge-base HEAD origin/main)" HEAD
OK: защищённые артефакты целы на HEAD (f43c119..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ bash scripts/check_docs_freeze.sh "$(git merge-base HEAD origin/main)" HEAD
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA="$(git merge-base HEAD origin/main)" bash scripts/check_gate_meta.sh
FAIL  research/critiques/C-306-M-99-restore-drill-automation-round-2.md: subject-lock — после проходного вердикта (NOTE) тронут класс «гейт»: scripts/tests/red_restore_drill.sh scripts/verify_M-99.sh
      выход из лока — строка «ALLOW-SUBJECT-CHANGE: <причина>» в теле коммита диапазона
VERDICT: FAIL (1) — вердикт не привязан к предмету либо merge прошёл без вердикта.
exit=1
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-13
- Milestone: M-99-restore-drill-automation
- Статус: BLOCKED — REJECT
- HEAD: `113eaf931ca15174cd108645c83ea139559b1450`

## §B — Что проверено
- Полный round-2 artifact set, M-99, R-263, C-298/C-306, A-032 §2.2–2.4, OPS-I-2/3/10/11 and JR-I-6.
- RED probe, ops RED suite, merge-preview design claims, scope/link checks, and targeted RW/v5/CR mutations.

## §C — Артефакт
- `research/critiques/C-311-M-99-restore-drill-automation-round-3.md`

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Push status:** pending this C-311 verdict commit to `origin/feat/M-99-restore-drill-automation`.
- **Paste-ready prompt:**
  ```text
  M-99 is REJECT by C-311 at 113eaf931ca15174cd108645c83ea139559b1450. Repair the plan-time RED/verify artifacts only: (1) reject every remote command except declared read-only list/named-file fetches, with a destructive-command mutant; (2) prove wrapper-owned flock and reject the no-lock concurrent mutant while cron itself has no flock; (3) make compose v5 exclusive across every service, rejecting broad /var/lib/hft and duplicate-service mounts; (4) add ALLOW-SUBJECT-CHANGE with a specific reason for the two post-C-306 gate-path edits, then show check_gate_meta green. Commit reproducible mutations for T/RT/RW/CR/IM and request a fresh critic pass. Do not dispatch implementation or alter retention/Storage Box data.
  ```

## §E — Open risk
- The current production RED failures are expected before engine-dev. The blockers above are instead holes that could allow an unsafe implementation to turn green.

=== END HANDOFF ===
