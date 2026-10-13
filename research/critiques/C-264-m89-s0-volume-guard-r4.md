<!-- GATE-META
milestone: M-89
audited_repo: a3ka/hft-platform
audited_base: dcb435f80109030ab6efb3157f86df2f53b609d9
audited_head: 0777364492990f8e367a340038d5942162f624cc
verdict: NOTE
-->

# C-264 — M-89 S0: A-042 epoch-boundary conformity, round 4

## Verdict

**NOTE — non-blocking; the A-042 §7 gate is satisfied.**  The committed round-4
artifact set materialises the arbitrator's binding decision.  `n1`–`n4` are
runtime-RED for the required behaviour (today's reader silently crosses the
catalogue hole); `n5` is the green positive control for a legitimate filtered
projection.  No REJECT finding is open in the frozen scope.  Architect should
append this verdict mechanically to M-89 §20, then dispatch task 1 to
`engine-dev`; this critic does not alter the milestone.

## Audited subject

- Subject: `origin/feat/M-89-s0-volume-guard-observability`.
- Audited base: `dcb435f80109030ab6efb3157f86df2f53b609d9`.
- Audited head: `0777364492990f8e367a340038d5942162f624cc` (the mandated
  `0777364` is that tip and is its ancestor, exit 0).
- The fetch immediately before this verdict found `drift=none`.
- Full plan-time artifact set is present: M-89 specification; the behavioural
  T-contract and planned inherent/trait signatures in §4.1; existing journal
  RED suites `j*`, `f*`, `b*`; new junction suite `n1`–`n5`; and the explicit
  FAIL-counter acceptance script.  No `crates/contracts/**` path is in the
  range, so no T1 contract-RFC is required.

The live journal invariants are **JR-I-2** (a read-time sequence gap aborts)
and **JR-I-11** (no catalog-stitching reader silently accepts non-monotonic
order).  A-042 resolves the JR-I-2 interpretation for an epoch-filtered
projection; this audit tests conformity to that decision and does not reopen
the interpretation.

## A-042 §7 checks

1. **Specification (§7.1): PASS.** M-89 §5.2 p.9 carries all four checks
   (a)–(г), catalogue-defined legality of a visible gap, reset to the accepted
   segment's `header.first_seq`, the explicit excluded-segment limit, and the
   `force-next-seq` consequence.  The wording agrees materially with A-042 §4.
2. **Junction oracle (§7.2): PASS.**
   `crates/journal/tests/red_m89_seek_junction.rs` has independent header/body
   catalogue setup guards for each `n1`–`n5`; its expected values are derived
   from headers and the test's strict frame parser, not `stream()`.  The
   executed suite is compile-clean: `n1`/`n2`/`n3`/`n4` fail with `Ok` delivery
   through the declared hole, and `n5` passes as the positive control.
3. **Boundary and compatibility text (§7.3): PASS.** §11 p.12 carries A-042
   §6; §5.3, §10, and `j8` limit stream equivalence to catalogues without a
   hole.  Thus the older soft `stream()` path is not made the oracle for a
   new strict seek-path test.
4. **Implementation mutants (§7.2/§7.4): PASS.** M-89 §14 and task 1 name all
   required dev Done Block mutations: remove (в) → `n1`/`n3`; remove (г) →
   `n2`; skip (в) for an unaccepted successor → `n4`; carry expectation across
   a junction → `n5` fails.  The setup-defect mutations are separately guarded
   as `SETUP НЕ СОСТОЯЛСЯ`, rather than becoming a false green.
5. **Regression and acceptance baseline (§7.5): PASS.** The complete
   `verify_M-89.sh` baseline is intentionally red before implementation, while
   the task-status and CI-map guards are green.  The prior R6 `f1`/`f1b` suite
   remains RED only because the planned `seek_fallbacks` method is absent;
   `j8`, `red_stream_from`, and `red_stitch_monotonic` are green.  The
   merge-preview claim gate passes.

## Scope

The round-4 range after A-042 is exactly the new sacred RED file, M-89, and
`verify_M-89.sh`.  It does not touch `stream`, `stream_from`, `read_all`,
`recover`, `mn_*`, `docs/fa/journal.md`, or production implementation paths,
as A-042 §7.4 requires.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-89-s0-volume-guard-observability
0777364492990f8e367a340038d5942162f624cc
$ git merge-base origin/main origin/feat/M-89-s0-volume-guard-observability
dcb435f80109030ab6efb3157f86df2f53b609d9
$ git merge-base --is-ancestor 0777364 origin/feat/M-89-s0-volume-guard-observability
exit=0
$ final git fetch
audited_start=0777364492990f8e367a340038d5942162f624cc
head_after_final_fetch=0777364492990f8e367a340038d5942162f624cc
drift=none
mandate_sha_ancestor_exit=0

$ git diff --name-status 486df27..0777364
A	crates/journal/tests/red_m89_seek_junction.rs
M	milestones/M-89-s0-volume-guard-observability.md
M	scripts/verify_M-89.sh
$ git diff --name-only dcb435f..0777364 -- crates/journal/src crates/journal/tests/red_stream_from.rs crates/journal/tests/red_stitch_monotonic.rs docs/fa/journal.md
<empty>
forbidden_path_scan_exit=0

$ cargo test -p journal --test red_m89_seek_junction -- --nocapture
test n4_truncated_tail_of_accepted_before_excluded_is_refused ... FAILED
n4 ... [6, 7, 8, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39] ... физический преемник ... фильтром исключён
test n2_first_frame_of_successor_cut_is_refused_at_left_edge ... FAILED
n2 ... событие 88 выдано, хотя кадр 87 ... отсутствует
test n5_positive_control_legit_filtered_projection_passes_with_all_as_pair ... ok
test n3_removed_middle_segment_after_zst_seg0_is_refused_too ... FAILED
n3 ... получено 132 событий ... дыра с 87 пройдена МОЛЧА
test n1_removed_middle_segment_is_refused_at_seg0_right_edge ... FAILED
n1 ... получено 128 событий, первое Some(172) ... дыра с 87 пройдена МОЛЧА
test result: FAILED. 1 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
exit=101

$ bash scripts/tests/red_verify_M-89_task_status.sh
итого: ok=8 fail=0
VERDICT: PASS
exit=0
$ VERIFY_M89_MODE=ci-map bash scripts/verify_M-89.sh
PASS  CI-паритет: таблица соответствия покрывает все 54 run: из .github/workflows/ci.yml и не содержит протухших строк
VERDICT: PASS
exit=0
$ cargo test -q -p journal --test red_m89_seek_contract j8_stream_from_at_equals_independent_filter_on_all_degenerate_inputs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out
exit=0
$ cargo test -q -p journal --test red_stream_from
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0
$ cargo test -q -p journal --test red_stitch_monotonic
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0
$ cargo test -q -p journal --test red_m89_seek_fallback_observed
error[E0599]: no method named `seek_fallbacks` found for struct `EventStream`
error: could not compile `journal` (test "red_m89_seek_fallback_observed") due to 1 previous error
exit=101
$ bash scripts/verify_M-89.sh
FAIL  task1: у EventStream в crates/journal/src/segments.rs нет payload_bytes_read / seek_fallbacks — форма §4.1 не выполнена
FAIL  task1/договор: red_m89_seek_contract ... КРАСЕН
FAIL  task1/откат: red_m89_seek_fallback_observed ... КРАСЕН — компиляция
FAIL  task1/стык: red_m89_seek_junction ... КРАСЕН — test result: FAILED. 1 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
FAIL  task1/байты: red_m89_bytes_accounting ... КРАСЕН — компиляция
VERDICT: FAIL (intentional pre-implementation RED baseline; all guard commands above passed)
exit=1
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/reserve_artifact_id.sh C
reserve C-264 nous 2026-09-27T18:30:37Z Ubuntu-2404-noble-amd64-base 1804309 14c061d1-789e-4f5b-a466-05102dfe3d2c
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные

- Дата (UTC): 2026-09-27
- Milestone: M-89-s0-volume-guard-observability
- Статус: DONE — NOTE, A-042 §7 conformity passed
- HEAD: 0777364 — task1 junction RED is wired into verify

## §B — Что я сделал

- Audited the committed artifact set against the binding A-042 §7 freeze rule.
- Reproduced the behavioural n1–n4 REDs and the n5 positive control; checked
  the frozen-path boundary, prior guards, and merge-preview claims.

## §C — Артефакты / результаты

- `research/critiques/C-264-m89-s0-volume-guard-r4.md`
- Done Block above; intentional RED baseline is `verify_M-89.sh` exit 1.

## §D — Следующий агент + инвокация

- **Следующий агент:** `architect`, then `engine-dev`
- **Paste-ready промпт:**
  ```
  M-89 received C-264 NOTE after A-042 §7. Mechanically append C-264 to
  milestone §20 without changing the frozen A-042 contract, then dispatch
  engine-dev task 1. Implement only the M-89 §13 task-1 journal path and
  signatures; preserve the behavioural RED oracles. Dev Done Block must run
  the four A-042 §5 junction mutants and record their failures.
  ```
- Push-статус: committed verdict pending push to `origin/feat/M-89-s0-volume-guard-observability`
- Кэш: cleanup follows after push.

## §E — Риски / открытые вопросы

- The deliberate strict-seek / soft-other-reader transition remains the
  accepted A-042 boundary and is owned by M-81; it is not reopened here.

=== END HANDOFF ===
