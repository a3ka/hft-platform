<!-- GATE-META
milestone: M-89
audited_repo: a3ka/hft-platform
audited_base: dcb435f80109030ab6efb3157f86df2f53b609d9
audited_head: d1b0b1d6fbe69c9df2dced0f516b08fa659036b4
verdict: ESCALATE
-->

# C-263 — M-89 S0: volume guard and serving observability, round 3

## Verdict

**ESCALATE.** Round 3 closes `C-262` R6 at the changed seek path: `f1`
now requires an observable fallback, no delivered event, and
`Err(InvalidData)` for the missing `after + 1`; `f1b` separately pins a
mid-tail gap to the exact pre-gap prefix followed by the same error.  A
temporary compile-only substitution makes both tests fail against the current
gap-tolerant iterator, then was removed without a worktree diff.

The remaining issue is a boundary-of-invariant decision, not an implementation
detail for this critic to choose.  M-89 §5.2 p.9 deliberately exempts a
segment junction from its new continuity guard because `EpochFilter` can
remove intervening epochs.  That is consistent with the existing filtering
mechanism, but it is in tension with the literal, global wording of **JR-I-2**
(`seq` has no gaps; a read gap aborts).  The artifact contains neither the
authority that resolves that interpretation nor a direct `stream_from_at`
oracle that pins the intended filtered-boundary behaviour.  This is the third
round on the subject, so `gates.md` §0 sends it to a fresh-context arbiter.
Dev must not be dispatched until the decision is materialized in the artifact
set.

## Audited subject and complete artifact set

- Subject: `origin/feat/M-89-s0-volume-guard-observability`.
- Mandate SHA `d1b0b1d` was fetched as the subject tip
  `d1b0b1d6fbe69c9df2dced0f516b08fa659036b4`; it is an ancestor of that tip.
  The fetch immediately before this verdict found `drift=none`.
- Base: `dcb435f80109030ab6efb3157f86df2f53b609d9`.
- The committed set contains the milestone, measured basis, all three
  journal RED binaries (`seek_contract`, `seek_fallback_observed`, and
  `bytes_accounting`), gateway/gateway-serve/ops REDs, both verify probes,
  and `scripts/verify_M-89.sh`.  No `contracts/**` path is in the branch
  diff; the interfaces are additive plan-time signatures, correctly
  compile-RED where they do not yet exist.
- The acceptance script is a real explicit FAIL-counter gate (not a masked
  `cmd && PASS || FAIL` form).  Its task-status, scanner/CI-map/composition
  probes are green.  The intentional task-1 baseline remains red until the
  missing `EventStream` methods and seek implementation exist.
- This is a RAW journal-read gate.  The live FA invariants examined are
  **JR-I-2** and **JR-I-11**.  `JR-I-11` is also relevant because the catalog
  guard precedes the selected-stream construction; it establishes monotonic
  `first_seq`, not exact consecutive event sequence across a filtered view.

## C-262 R6 closure: demonstrated, not only described

`f1` creates a valid-CRC gap at sequence 3000 and confirms that the current
independent `stream()` still silently starts at 3001.  With only
`seek_fallbacks()` temporarily replaced by `events_scanned().min(1)`, the
otherwise compile-RED binary reached its behavioural assertions and failed:

```text
f1 / JR-I-2 (C-262 R6): при дыре на after+1 выдано 200 событий,
первое Some(3001) — событие 3000 ПРОПУЩЕНО МОЛЧА

f1b / JR-I-2 (C-262 R6): выдача обязана быть РОВНО 3000..3100 (до дыры),
затем abort; события после дыры не выдаются
```

The mutation also makes the deliberate zero-fallback controls `f3`--`f5`
red, which is expected for that one-line fake counter; it does not weaken the
two observed R6 bites.  The original source was restored and `git diff` was
empty.  On the committed source, this file is compile-RED only for the absent
`seek_fallbacks` method (`E0599`); `red_m89_bytes_accounting` is independently
compile-RED only for absent `payload_bytes_read` (`E0599`).

`j8` is green on its existing active/closed/`.zst`/torn inputs, and the
unmodified `red_stream_from` (6 tests) and `red_stitch_monotonic` (11 tests)
remain green.  Thus this escalation does not reopen C-260 R1--R5 or C-262's
closed R6 through an observed regression in the current corpus.

## Escalated question — p.9's segment-junction carve-out has no direct oracle

### Facts established by source and execution

1. `EpochFilter::OwnCaptureOnly` selects by `DataSource`; `Explicit` selects
   named `epoch_id`s.  Both `stream_from_at_with_catalog` and `stream_from`
   filter the catalog *before* constructing their selected segment sequence.
   Existing `red_segments_epochs::default_filter_excludes_vendor_and_synthetic`
   is green, establishing that omission of an epoch is intentional rather
   than corruption.
2. The current iterator tracks no `expected_seq`/`prev_seq`: after the
   `after_seq` filter it emits every larger event.  This is exactly the
   same-segment silent-drop mutant that `f1`/`f1b` now catch on the M-89 path.
3. `segments_counted` invokes the JR-I-11 guard over all physical catalog
   headers before filter selection.  The guard rejects non-monotonic
   `first_seq`; it does not assert `next.first_seq == previous.last_seq + 1`.
   `red_stitch_monotonic::mn_6` intentionally accepts an index gap after
   retention.
4. The new M-89 test pair creates only one accepted segment.  `j8` has no
   epoch-filtered fixture.  The existing epoch test exercises `stream()`,
   not the changed public `stream_from_at(.., Some(after), None)` path.

Consequently two materially different implementations can satisfy every
round-3 M-89 oracle:

- Resetting the new expected-sequence state when `open_next_segment()` crosses
  a selected boundary follows M-89 p.9, but also lets a missing physical
  segment pass at that boundary.
- Carrying the expected sequence across each selected boundary rejects a
  legitimate filtered projection when an unaccepted Vendor/Synthetic segment
  lies between two accepted ones.

The former is not provably authorised by the literal FA sentence for JR-I-2;
the latter contradicts M-89 p.9's stated EpochFilter carve-out.  Choosing
between them changes the reader's contract and must not be decided by this
plan-time critic.

### Required arbiter decision and resulting oracle

The arbiter should decide which of the following is the authoritative meaning
of JR-I-2 for an `EpochFilter` projection:

1. **Filtered projections may have visible sequence gaps.** Retain M-89 p.9's
   boundary exemption, but require an architect RED oracle in
   `red_m89_seek_fallback_observed.rs` (or the direct journal contract suite):
   create accepted epoch A `0..9`, an excluded Vendor/Synthetic epoch
   `10..29`, then accepted epoch B `30..39`; call
   `stream_from_at(..., EpochFilter::Explicit([A, B]), Some(9), None)` and
   require exactly `30..39` with no error.  Add the paired `EpochFilter::All`
   control, which must expose `10..39`.  This proves the exception on the
   changed public path and kills the cross-boundary `expected_seq` mutant.
2. **The filtered reader itself must be gap-free.** Replace p.9's broad
   exemption with a precise criterion for a retained physical boundary, add a
   RED that corrupts/removes such a boundary and requires `InvalidData`, and
   reconcile the existing `stream`/`stream_from`/retention behaviour plus the
   FA claim in an architect-owned follow-up.  It is not sound to make only
   `stream_from_at` stricter while leaving the same reader contract explicitly
   divergent elsewhere.

The first option is supported by the existing filter design; the second is
supported by the literal wording of JR-I-2.  The arbitration artifact must
name the selected option, its scope, and the required RED before another
plan-time pass.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-89-s0-volume-guard-observability
d1b0b1d6fbe69c9df2dced0f516b08fa659036b4
$ git merge-base origin/main d1b0b1d6fbe69c9df2dced0f516b08fa659036b4
dcb435f80109030ab6efb3157f86df2f53b609d9
$ git merge-base --is-ancestor d1b0b1d origin/feat/M-89-s0-volume-guard-observability
exit=0
$ git fetch origin && git rev-parse origin/feat/M-89-s0-volume-guard-observability
d1b0b1d6fbe69c9df2dced0f516b08fa659036b4
pre_write_final_drift=none
exit=0

$ artifact presence check
PRESENT milestone; measurement basis; verify_M-89; both verify probes;
PRESENT three journal M-89 REDs; gateway/gateway-serve/ops M-89 REDs
exit=0

$ temporary: seek_fallbacks() -> events_scanned().min(1)
$ cargo test -p journal --test red_m89_seek_fallback_observed -- --nocapture
f1 ... выдано 200 событий, первое Some(3001) — событие 3000 ПРОПУЩЕНО МОЛЧА
f1b ... выдача обязана быть РОВНО 3000..3100 (до дыры), затем abort
test result: FAILED. 1 passed; 5 failed
exit=101
$ restore original file; git diff -- crates/journal/tests/red_m89_seek_fallback_observed.rs
<empty>
restore_check_exit=0

$ cargo test -q -p journal --test red_m89_seek_fallback_observed
error[E0599]: no method named `seek_fallbacks` found for struct `EventStream`
seek_fallback_compile_red_exit=101
$ cargo test -q -p journal --test red_m89_bytes_accounting
error[E0599]: no method named `payload_bytes_read` found for struct `EventStream`
bytes_compile_red_exit=101
$ cargo test -q -p journal --test red_m89_seek_contract j8_stream_from_at_equals_independent_filter_on_all_degenerate_inputs
test result: ok. 1 passed; 0 failed; 9 filtered out
j8_guard_exit=0
$ cargo test -q -p journal --test red_stream_from
test result: ok. 6 passed; 0 failed
red_stream_from_exit=0
$ cargo test -q -p journal --test red_stitch_monotonic
test result: ok. 11 passed; 0 failed
red_stitch_monotonic_exit=0
$ cargo test -q -p journal --test red_segments_epochs default_filter_excludes_vendor_and_synthetic
test result: ok. 1 passed; 0 failed; 8 filtered out
epoch_filter_guard_exit=0

$ bash scripts/tests/red_verify_M-89_task_status.sh
итого: ok=8 fail=0
VERDICT: PASS
exit=0
$ bash scripts/tests/red_verify_M-89_scan.sh
итого: ok=24 fail=0
VERDICT: PASS
exit=0
$ VERIFY_M89_MODE=ci-map bash scripts/verify_M-89.sh
PASS  CI-паритет: таблица соответствия покрывает все 54 run: из .github/workflows/ci.yml и не содержит протухших строк
VERDICT: PASS
exit=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
$ git diff --check dcb435f..d1b0b1d
exit=0

$ bash scripts/reserve_artifact_id.sh C
# C-263 was allocated; the resulting remote CAS reservation is read below.
$ git fetch origin refs/reserved/C-263:refs/reserved/C-263 && git show -s --format='%s' refs/reserved/C-263
reserve C-263 nous 2026-09-27T16:37:37Z Ubuntu-2404-noble-amd64-base 4156967 6a0ef0b8-0b5d-4fd4-b80f-a8ce6ac515b6
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные

- Дата (UTC, ISO-8601): 2026-09-27T16:39Z
- Milestone: M-89-s0-volume-guard-observability
- Статус: BLOCKED — `ESCALATE`, третий круг
- HEAD: d1b0b1d — docs(M-89): круг 3, C-262 R6

## §B — Что я сделал

- Судил закоммиченный набор от `dcb435f` до `d1b0b1d`, не текст плана из мандата.
- Воспроизвёл укус `f1`/`f1b` временной подменой compile-RED счётчика и восстановил файл.
- Проверил FA JR-I-2/JR-I-11, `EpochFilter`/каталог/итератор, действующие RED-стражи и
  targeted task-1 baseline.

## §C — Артефакты / результаты

- `research/critiques/C-263-m89-s0-volume-guard-r3.md`
- Done Block выше; targeted task-1 RED baseline и все названные guards имеют exit-коды.

## §D — Следующий агент + инвокация

- **Следующий агент:** `arbiter` (сильная модель, свежий контекст)
- **Paste-ready промпт:**
  ```
  Арбитраж M-89, круг 3. Прочитай C-260, C-262 и C-263 на
  origin/feat/M-89-s0-volume-guard-observability, milestone
  M-89-s0-volume-guard-observability.md §5.2 п.9/§11 п.12, docs/fa/journal.md
  JR-I-2/JR-I-11, crates/journal/src/segments.rs и RED suites
  red_m89_seek_fallback_observed.rs, red_segments_epochs.rs,
  red_stream_from.rs, red_stitch_monotonic.rs. Реши: допускает ли JR-I-2
  видимый seq-gap на границе EpochFilter-проекции, либо границу надо
  валидировать. Назови точный RED-оракул и область изменения. Запиши
  A-NNN verdict на предметной ветке до передачи дальше.
  ```
- Push-статус: ✅ pushed to origin/feat/M-89-s0-volume-guard-observability with this verdict commit.
- Кэш: ✅ кэш убран перед handoff.

## §E — Риски / открытые вопросы

- Без решения арбитра dev может сделать либо silent physical-boundary gap, либо ложный
  fail-closed отказ на легальной EpochFilter-проекции; нынешние REDs не различают эти исходы.

=== END HANDOFF ===
