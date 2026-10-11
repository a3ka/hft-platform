<!-- GATE-META
milestone: M-98
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: 8eb19a0680b6616d2186953555923f0843a05ba6
verdict: NOTE
-->

# C-307 — M-98 SWITCH provenance, round 3: A-053 actions are complete

## Verdict: NOTE

`A-053` required one narrowed round only.  All six required architect actions are
present, and the committed eight-world reproducer measured exactly its declared
failed-scenario sets.  No new measured counterexample against the new `s0` and
`s3`/`s4` exact-start oracles was found.  This NOTE is non-blocking: M-98 may
proceed to its named `engine-dev` task 1.

This audit deliberately does not reopen the matters A-053 declared settled:
`C-297` B2's matrix mechanism; `s1`/`s2` and `first_retained_seq`; the
`m98-switch-catalog` channel; the inotify and placement witnesses; MINOR
classification; or the §13 route.

## Subject and committed artifact set

- Subject: `origin/feat/M-98-switch-provenance`; handoff reference `8eb19a06`
  matched the fetched and re-fetched remote tip.
- Audited range: `a660b3a7..8eb19a06`; the current round's A-053 delivery is
  `ba9264fb`, `2860d219`, `5c791a8b`, and `8eb19a06`.
- Complete plan-time set is committed: M-98 milestone, runtime RED suite,
  prototype patch, executable mutation matrix, and `verify_M-98.sh`.
- T-contracts and trait signatures: N/A.  The range changes neither
  `contracts/**` nor production `crates/**/src/**`; it preserves the existing
  snapshot wire fields.  The only crate artifact is the architect-owned sacred
  RED suite.
- Living invariant named for the touched serving surface: `VB-I-11` — a
  snapshot must name the retained history start exactly, not only its
  truncation boolean.

## A-053 required actions — checked line by line

| Action | Evidence at `8eb19a06` | Result |
|---|---|---|
| A1 | `s0` asserts `start_seq(&body) == first_retained_seq(dir.path())`; the helper documents that the stream-derived reference also applies to a whole journal. | PASS |
| A2 | `s3` and `s4` obtain `frozen_start` through the specified public checkpoint-header APIs, guard `frozen == first_retained_seq` before the pause, assert `truncated` first, then assert exact start equality. | PASS |
| A3 | `prototype.patch` adds `completeone` only for `!lt` and `frozenwrong` only to the reached catalog fallback; its comment explicitly excludes the unreached `None` branch from claimed coverage. | PASS |
| A4 | `red_m98_mutants.sh` declares eight worlds, including `completeone → s0`, `frozenwrong → s3 s4`, and remeasured `startone → s1 s2 s3 s4`; the execution below matched every declaration. | PASS |
| A5 | M-98 §3 names the checkpoint-header frozen reference; §4 and §8 cover the exact `s0`/`s3`/`s4` obligations and all matrix rows; §5 records the ADD/legacy TD route; §14 records C-302, A-053, and this narrowed round.  The RED-file scenario header is aligned as well. | PASS |
| A6 | `verify_M-98.sh` keeps the task composition and labels its matrix step as `прототип + 6 мутантов`. | PASS |

## Gates.md §9 recheck, limited to the changed material

- **(a) Claims about code:** `bash scripts/verify_design_claims.sh --merge-preview origin/main`
  returned `VERDICT: PASS (0 нарушений)`.  The executable matrix independently
  confirms the code claims in A1–A4; its pass is not substituted for a future
  implementation test.
- **(b) Authority:** the post-decision changes are sacred architect test/gate/
  milestone artifacts plus the arbiter artifact.  They are within M-98 §10 and
  the respective gate-role paths; no production implementation, T1 contract,
  process norm, Boundary-C decision, journal layout, or money path changed.
- **(c) Coherence and links:** `git diff --check` passed.  The RED suite,
  prototype, script, verifier, and M-98 §8 agree on all eight worlds; the
  verifier invokes the matrix instead of merely citing it.

`check_artifact_ids.sh` still reports the already-recorded cross-subject
`C-297` collision with M-100.  It does not involve the allocator-reserved
`C-307`, is not a new M-98 finding, and was explicitly assigned to the M-100
subject in C-302.  It therefore cannot replace the A-053-limited verdict.

## Matrix reproduction

The reproducer is intentionally a test-strength measurement: it restores
`crates/gateway-serve/src/lib.rs` from `a660b3a7`, applies the committed
prototype, and leaves product implementation untouched.  It therefore does
not claim to test a future engine-dev implementation.

```text
$ bash scripts/tests/red_m98_mutants.sh
PASS  базовый код a660b3a7: красны «s0 s1 s2 s3 s4» — как объявлено
PASS  прототип по §3: красны «нет» — как объявлено
PASS  мутант: провенанс без is_fresh/refresh: красны «s2 s3 s4» — как объявлено
PASS  мутант: Err is_fresh/refresh проглочен: красны «s3 s4» — как объявлено
PASS  мутант: на SWITCH всегда (frozen, true): красны «s0 s1 s2» — как объявлено
PASS  мутант: начало истории = 1 при усечении (C-297 B1): красны «s1 s2 s3 s4» — как объявлено
PASS  мутант: начало = 1 на ЦЕЛОМ журнале (C-302): красны «s0» — как объявлено
PASS  мутант: fail-closed с ЛОЖНЫМ frozen (A-053): красны «s3 s4» — как объявлено
VERDICT: PASS — 8 миров
red_m98_mutants_exit=0
```

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-98-switch-provenance
8eb19a0680b6616d2186953555923f0843a05ba6
exit=0

$ git merge-base --is-ancestor 8eb19a0680b6616d2186953555923f0843a05ba6 origin/feat/M-98-switch-provenance
exit=0

$ git diff --check a660b3a7b78b116a0a98bc7c9d9eb1e03153725c..HEAD
exit=0

$ bash scripts/tests/red_m98_mutants.sh
PASS  базовый код a660b3a7: красны «s0 s1 s2 s3 s4» — как объявлено
PASS  прототип по §3: красны «нет» — как объявлено
PASS  мутант: провенанс без is_fresh/refresh: красны «s2 s3 s4» — как объявлено
PASS  мутант: Err is_fresh/refresh проглочен: красны «s3 s4» — как объявлено
PASS  мутант: на SWITCH всегда (frozen, true): красны «s0 s1 s2» — как объявлено
PASS  мутант: начало истории = 1 при усечении (C-297 B1): красны «s1 s2 s3 s4» — как объявлено
PASS  мутант: начало = 1 на ЦЕЛОМ журнале (C-302): красны «s0» — как объявлено
PASS  мутант: fail-closed с ЛОЖНЫМ frozen (A-053): красны «s3 s4» — как объявлено
VERDICT: PASS — 8 миров
red_m98_mutants_exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [1-ЕСТЬ] все 3 маркеров [ЕСТЬ] в таблицах статусов сопровождены существующим пруфом (17 вне таблиц — ASCII-схемы/проза, не проверялись)
PASS  [3-ССЫЛКИ] все 8 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 660 ссылок вида docs/*.md указывают на существующие файлы
PASS  [7-RFC-PATH] путей-кандидатов (токены со слэшем в backtick'ах, docs/rfc/**.md): всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
verify_design_claims_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=a660b3a7b78b116a0a98bc7c9d9eb1e03153725c bash scripts/check_gate_meta.sh
── GATE-META: диапазон a660b3a7..HEAD, origin=a3ka/hft-platform
VERDICT: PASS — вердиктов проверено: 3, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
check_gate_meta_ci_form_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=a660b3a7b78b116a0a98bc7c9d9eb1e03153725c bash scripts/check_artifact_ids.sh
FAIL  C-297: второй носитель «research/critiques/C-297-M-98-switch-provenance.md» под идентификатором, занятым «research/critiques/C-297-M-100-three-serving-contracts.md»
check_artifact_ids_ci_form_exit=1

$ EVENT_NAME=pull_request PR_BASE_SHA=a660b3a7b78b116a0a98bc7c9d9eb1e03153725c bash scripts/check_review_fa.sh
SKIP (диапазон трогает ТОЛЬКО не-прод пути крейтов — tests/examples/benches, в прод-образ не входят; C-115 B-1, классификация A-012 §1-Д п.5)
      ↳ crates/gateway-serve/tests/red_m98_switch_provenance.rs
check_review_fa_ci_form_exit=0

$ git status --porcelain
<empty before adding this verdict>

$ bash scripts/reserve_artifact_id.sh C
reserve: попытка 1/8 — C-307 ← f0b0174689e1e7ebc9bbb40c9bde802f7560d2f5
C-307
reserve: резерв C-307 взят; снять после приземления носителя:
reserve:   bash scripts/reserve_artifact_id.sh --release C-307
exit=0
```

=== HANDOFF: CRITIC → ENGINE-DEV ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-11T14:00Z
- Milestone: M-98-switch-provenance
- Статус: DONE — plan-time round 3 NOTE; dev dispatch unblocked
- HEAD: pending critic verdict commit on `feat/M-98-switch-provenance`

## §B — Что я сделал
- Audited only A-053 actions A1…A6 and the remeasured eight-world matrix.
- Confirmed every declared failed-scenario set by execution and found no new measured counterexample.

## §C — Артефакты / результаты
- `research/critiques/C-307-M-98-switch-provenance-round-3.md`
- Done Block: mutation matrix exit=0; merge-preview claim check exit=0; whitespace check exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `engine-dev`
- **Paste-ready промпт:**
  ```
  Implement M-98 task 1 on origin/feat/M-98-switch-provenance after C-307 NOTE.
  Touch only crates/gateway-serve/src/lib.rs as M-98 §10 permits.  Implement the SWITCH
  provenance calculation and fail-closed behavior specified by the committed s0…s4 RED suite;
  do not change ADD/legacy behavior, T1 contracts, or sacred tests/scripts.  Run the named RED
  suite and scripts/verify_M-98.sh, commit only your implementation path, and push the GREEN
  commit to the subject branch before handoff.
  ```
- Push-статус: pending critic verdict commit and push to the subject branch.
- Кэш: pending until the verdict is pushed.

## §E — Риски / открытые вопросы
- The matrix measures the RED suite against a pinned prototype, not the future product implementation; tester and reviewer must run the ordinary product RED suite after task 1.
- ADD/legacy exact frozen-start gaps remain explicitly out of M-98 and require the reviewer TD route stated in M-98 §5.

=== END HANDOFF ===
