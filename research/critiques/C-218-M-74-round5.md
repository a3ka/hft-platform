<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: 9e374c1e1b1cb6b11e6cfe41ff13206a677d948d
audited_head: dc42d44ab1fd5307a3492c207cb66f8a392b460d
verdict: REJECT
-->

# C-218 — M-74 restore-drill, round 5: REJECT

## Verdict

**REJECT — do not dispatch engine-dev.** Founder authorized this fifth round after
the `A-032` limit; that authority is accepted and not re-opened here. This verdict
does not re-open the arbitration's alias-form or local-network boundary. It finds
two executable incompletions in the newly committed closure of `C-217`.

The affected invariants are **JR-I-6** (old journals remain readable by new code)
and **OPS-I-3** (the cold copy is restored and read). The range changes the
architect-owned fixture under `crates/journal/tests/**`; JR-I-6 is therefore named
explicitly.

### B-1 — N-1's `intra_segment_continuous` value is false on a healthy selected restore

The reader protocol says continuity is *within each segment*, because the drill
selects non-adjacent indices and inter-segment gaps are legal
(`milestones/M-74-restore-drill.md:491-494`). `read_dir_full` nevertheless carries
one `prev` value through the whole `journal::stream` (`crates/journal/tests/fixture_restore_drill_cold.rs:346-362`); it never observes or resets at a segment boundary.

On the committed healthy selected restore (`00000000`, `00000003`, `00000007`), the
reference reader reports `intra_segment_continuous=false` despite each selected
segment separately reporting `true`. Thus the claimed six-field protocol is not
implemented correctly, and the probe does not assert this field at all: the
reference wrapper extracts only `events_read` and `digest`
(`scripts/tests/red_restore_drill.sh:472-479`). A well-formed but semantically
wrong success protocol is still a contract failure.

**Condition to clear:** calculate continuity per physical segment (or make the
declared semantic match the actual calculation) and add RED coverage that (a) the
non-adjacent healthy selection reports `true`, and (b) a gap *inside one selected
segment* reports `false`. The shell/reference path must validate the full declared
success protocol, not merely serialize its unused fields.

### B-2 — the required dual-form delivery rule has no executable member

The selection contract requires both `.jrnl` and `.jrnl.zst` to be restored whenever
both forms exist for a selected index (`milestones/M-74-restore-drill.md:472-476`).
The committed fixture's only duplicate index is `00000001`, but the fixed selected
members are `00000000 00000003 00000007`
(`scripts/tests/red_restore_drill.sh:338,540-547`). None of those selected indices
has both forms. Consequently `delivery_matches_selection` verifies indices only
(`:340-356`) and H only requires at least one `.zst` (`:620,627`).

The intended contract condition is therefore vacuous: a wrapper that omits one
form of a selected raw+zst pair has no committed scenario in which it can be
rejected, while still delivering the required indices, producing a genuine digest,
and satisfying INV-DELIVERY. This is not a request for prohibited alias-form or
network coverage; it is the declared local selection algorithm's own per-index
file-form obligation.

**Condition to clear:** make at least one declared selected fixture member carry
both forms, then add an adversarial wrapper that omits exactly one counterpart while
all member-identity, digest, and source-independence setup guards pass. The probe
must reject it by an explicit selected-index-to-file-forms predicate.

## Checks that passed

- `C-217` B-2 is closed for *index identity*: A8 has exactly three wrong members,
  a genuine digest, and live INV-DELIVERY; removing identity makes exactly A8 fail.
- The four advertised mutations were executed against code after each mutation.
  They expose D/R, A8, A9, and A1 respectively; a group removal exposes all attack
  members. No third inert member was found in those four predicates.
- The required artifact set is present: T2 state/reader/wrapper/producer contracts
  and signatures in the milestone, architect-owned fixture and shell RED probe,
  counted `verify_M-74.sh`, and the milestone. The range is restricted to the
  allowed fixture, probe, and milestone paths; it changes neither T1 contracts nor
  boundary-C retention mode.
- `verify_M-74.sh` is a real counted gate (`set -uo pipefail`, final nonzero FAIL)
  and correctly remains intentionally red for the missing production wrapper and
  reader. Its present failure status does not excuse the two plan-time oracle gaps.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-218
exit=0

$ git diff --name-status 9e374c1e1b1cb6b11e6cfe41ff13206a677d948d..dc42d44ab1fd5307a3492c207cb66f8a392b460d
M	crates/journal/tests/fixture_restore_drill_cold.rs
M	milestones/M-74-restore-drill.md
M	scripts/tests/red_restore_drill.sh
exit=0

$ DRILL_READER_DIR=<healthy-selected-restore> DRILL_READER_MIN_EVENTS=1 cargo test -p journal --test fixture_restore_drill_cold --quiet -- --exact reference_reader_for_shell_probe --nocapture
DRILL_READER rc=0 segments_read=3 events_read=4125 seq_first=0 seq_last=10956 intra_segment_continuous=false digest=fd0ca6315386ef6994f234ec02e5f61ad5cade60c42c0ae4ad80f5c46af93bde reason=
exit=0

$ for each selected index (00000000 00000003 00000007): reference_reader_for_shell_probe
DRILL_READER rc=0 segments_read=1 events_read=1397 seq_first=0 seq_last=1396 intra_segment_continuous=true
DRILL_READER rc=0 segments_read=1 events_read=1364 seq_first=4137 seq_last=5500 intra_segment_continuous=true
DRILL_READER rc=0 segments_read=1 events_read=1364 seq_first=9593 seq_last=10956 intra_segment_continuous=true
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  обёртки deploy/bin/journal-restore-drill-cron.sh НЕ СУЩЕСТВУЕТ — RED задачи 1 (её вносит engine-dev задачей 2)
FAIL  N ПРОД-ЧИТАТЕЛЬ не выдал отпечаток (бинаря journal-drill-read ещё нет — задача 2b); независимый эталон работает
PASS  A8 НЕ ТЕ члены выборки ПОЙМАНЫ: «00000000 00000001 00000002» вместо «00000000 00000003 00000007»
PASS  A9 ЛОЖЬ В ОТЧЁТЕ ПОЙМАНА: состав верен, заявлено checked=99 при 3 доставленных
VERDICT: FAIL (2 из 20)
exit=1

$ digest_matches_state() { true; }
FAIL  D обёртка, восстановившая копию, но ВЫДУМАВШАЯ отпечаток, признана честной
FAIL  R отпечаток ЧУЖОГО прогона принят как свой
VERDICT: FAIL (4 из 20)
exit=1

$ delivery_matches_selection() { [ "${ck:-0}" -eq "${n:-0}" ]; }
FAIL  A8 обёртка, доставившая ТРИ НАСТОЯЩИХ, но НЕ ТЕ сегмента, признана честной
VERDICT: FAIL (3 из 20)
exit=1

$ delivery_matches_selection() { [ "${got}" = "${EXPECTED_MEMBERS}" ]; }
FAIL  A9 обёртка, доставившая верную выборку, но объявившая checked=99, признана честной
VERDICT: FAIL (3 из 20)
exit=1

$ inv_delivery() { :; d_after="$(truth_digest "$1/restore")"; [ "${d_after}" = "$2" ]; }
FAIL  A1 обёртка, положившая ССЫЛКИ вместо байтов, признана честной
VERDICT: FAIL (3 из 20)
exit=1

$ h_verdict() { [ "$(state_field "$1" ok)" = 1 ] && [ "$(state_field "$1" events_read)" -gt 0 ]; }
FAIL  D ... признана честной
FAIL  X ... признана честной
FAIL  A1 ... признана честной
FAIL  A6 ... признана честной
FAIL  A7 ... признана честной
FAIL  A8 ... признана честной
FAIL  A9 ... признана честной
FAIL  R ... принят как свой
VERDICT: FAIL (10 из 20)
exit=1

$ cargo test -p journal --test fixture_restore_drill_cold --quiet
test result: ok. 7 passed; 0 failed
exit=0

$ git diff --check dc42d44^..dc42d44
exit=0
```

=== HANDOFF: CRITIC → FOUNDER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-08T21:34Z
- Milestone: M-74-restore-drill
- Статус: BLOCKED — REJECT in founder-authorized round 5
- HEAD: dc42d44 — fix(M-74): C-217 closure artifacts

## §B — Что я сделал
- Audited the committed round-5 range and the complete T2/RED/verify/milestone artifact set.
- Executed the reader protocol and all four advertised exclusive mutations, plus the group mutation.

## §C — Артефакты / результаты
- `research/critiques/C-218-M-74-round5.md`
- Done Block: committed probe exit=1 only for the two declared production RED lines; fixture exit=0; all four guard mutations and the group mutation exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `founder`
- Push-статус: ✅ pushed in this verdict commit to `origin/docs/M-73-closeout-architect`.
- Кэш: ✅ no critic build cache created; temporary probe fixtures removed.
- **Paste-ready промпт:**
  ```
  M-74 round 5 is REJECT by C-218. Do not dispatch engine-dev. The new C-217 closure does prove index identity and its four guard mutations, but it does not implement the declared reader protocol: the non-adjacent healthy selection reports intra_segment_continuous=false even though each segment is continuous. It also has no selected index with both raw and zst forms, so the contract rule to restore both forms has no executable adversarial oracle. Decide the next owner and authorized route; this round was founder-authorized beyond A-032's original cap.
  ```

## §E — Риски / открытые вопросы
- The unresolved defects concern only local selection/protocol semantics, not A-032-settled alias forms or network transfer.
- The production wrapper and reader remain intentionally absent; the milestone must remain red.

=== END HANDOFF ===
