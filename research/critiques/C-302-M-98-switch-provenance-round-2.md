<!-- GATE-META
milestone: M-98
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: 18dec92693b4fe44c4c163c2621a953ff0321542
verdict: REJECT
-->

# C-302 — M-98 SWITCH provenance, round 2: exact start remains unpinned in three worlds

## Verdict: REJECT

`C-297` B2 is closed: the committed six-world reproducer is hermetic, fail-closed, wired
into `verify_M-98.sh`, and passed exactly as declared.  `C-297` B1 is only partly closed.
`s1` and `s2` now derive their exact expected start independently from `journal::stream`,
but `s0`, `s3`, and `s4` still never judge `history_start_seq`.  An implementation can
therefore return a false provenance value and pass all five RED scenarios, violating
`VB-I-11`.

This is the second REJECT for the same history-start provenance invariant after C-297.
Per `gates.md` §0, dev dispatch remains blocked and the next gate is an independent
arbiter, not a third architect↔critic loop.

## Subject and artifact-set completeness

- Subject branch fetched by command: `origin/feat/M-98-switch-provenance`.
- Handoff reference `18dec926` equals the fetched branch tip; it is therefore the audited
  head, not merely a reference from the mandate.
- Audited range: `a660b3a7..18dec926` (eight commits, including the C-297 corrections).
- Present and committed: milestone, sacred runtime-RED suite, acceptance script, executable
  prototype/mutation matrix, and the prior verdict.
- T1 contracts and trait signatures: N/A.  The range contains no `contracts/**` or
  production/signature change; M-98 preserves the wire form and existing snapshot fields.
- Scope is consistent with M-98 §10.  The harness artifacts are architect-owned and are
  additionally in the harness track; no founder-locked norm, Boundary-C surface, journal
  layout, or risk path is changed.

## Blocking finding

### B1-r2 — `history_start_seq` is still not an oracle in the complete and fail-closed worlds

`first_retained_seq()` is an appropriate independent expected-value source for `s1` and
`s2`: it reads the post-action journal through `journal::stream` with the production
`EpochFilter`, rather than consulting the transport catalog or response code.  Those two
checks close the exact successful-retention form of C-297 B1.

The coverage is nevertheless incomplete:

- `s0` only asserts `!history_truncated` (`red_m98_switch_provenance.rs:547-552`).  It
  never reads `history_start_seq`, although its intact fixture starts by appending seq `0`
  and `VB-I-11` requires the sequence of the first actually folded retained event.
- `s3` and `s4` only assert `history_truncated` after `is_fresh` or `refresh` fails
  (`:653-658`, `:682-687`).  M-98 §3 requires the full fallback
  `(frozen_start_seq, true)`, but neither test captures the frozen start from the first
  snapshot nor compares the response field to it.

This is executable, not a hypothetical omission.  In a disposable audit worktree I applied
the committed M-98 prototype and added the test-only `M98_MUT=completeone` branch:

```rust
snap.history_start_seq = if (m98 == "startone" && lt)
    || (m98 == "completeone" && !lt) { 1 } else { ls };
```

For the intact `s0` world this returns `history_start_seq=1` despite the retained stream
starting at seq `0`; it does not alter the correctly checked `s1`/`s2` paths or the
fallback worlds.  All `s0…s4` tests passed (raw output below).  Thus a false field can
ship even though the boolean is honest, directly contradicting `VB-I-11`.

**Condition to clear:**

1. In `s0`, compare `history_start_seq` to the independent `journal::stream` first event.
2. In `s3` and `s4`, capture the pre-failure frozen start and assert exact equality after
   the failure, alongside `history_truncated == true`.
3. Add both corresponding mutants to the committed matrix: one must be killed by `s0`, and
   one wrong fallback-start mutant must be killed by `s3`/`s4`.  The matrix’s declared sets
   and M-98 §8 must agree with the measured result.

## Checks that passed

### C-297 B2 — executable matrix is real and fail-closed

`red_m98_mutants.sh` constructs a disposable worktree from current tests, restores the
transport from pinned base `a660b3a7`, requires a dry-run and real prototype-patch apply,
requires the SWITCH rendezvous point, and requires exactly five executed scenario results.
It compares the observed failed-set to the declared set and accumulates failures before
returning nonzero.  A missing/unapplied patch, build failure, or unexecuted scenario cannot
produce `VERDICT: PASS`.  Its `trap EXIT` removes its worktree and target cache.

The reproduced matrix passed all six declared worlds: base `s0 s1 s2 s3 s4`; prototype
none; `nofresh` `s2 s3 s4`; `swallow` `s3 s4`; `alwaystrue` `s0 s1 s2`; and `startone`
`s1 s2`.  `verify_M-98.sh:32-34` invokes this reproduction as part of task 1, so it is
not prose-only evidence.

### `gates.md` §9 recheck

- **(a) Claims about code:** the merge-preview design-claim verifier passes.  The reported
  `VB-I-11` text matches the audit finding: it requires the seq of the first actually folded
  retained event, not merely a truncation boolean.
- **(b) Authority:** changed paths are within the M-98 ownership table; no process-rule or
  founder-only Boundary-C decision was smuggled into the harness correction.
- **(c) Coherence and links:** the matrix, prototype, RED suite, M-98 §8/§9, and acceptance
  invocation exist and are mutually wired.  The only coherence defect is B1-r2’s missing
  oracles above.

`check_artifact_ids.sh` is intentionally not a finding of this audit: the known C-297
collision belongs to `feat/M-100-three-serving-contracts`; M-98 occupied C-297 first and
the M-100 branch is responsible for renumbering.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-98-switch-provenance
18dec92693b4fe44c4c163c2621a953ff0321542
exit=0

$ git merge-base --is-ancestor 18dec926 origin/feat/M-98-switch-provenance
exit=0

$ bash scripts/tests/red_m98_mutants.sh
PASS  базовый код a660b3a7: красны «s0 s1 s2 s3 s4» — как объявлено
PASS  прототип по §3: красны «нет» — как объявлено
PASS  мутант: провенанс без is_fresh/refresh: красны «s2 s3 s4» — как объявлено
PASS  мутант: Err is_fresh/refresh проглочен: красны «s3 s4» — как объявлено
PASS  мутант: на SWITCH всегда (frozen, true): красны «s0 s1 s2» — как объявлено
PASS  мутант: начало истории = 1 при усечении (C-297 B1): красны «s1 s2» — как объявлено
VERDICT: PASS — 6 миров
exit=0

$ M98_MUT=completeone cargo test -p gateway-serve --features testing --test red_m98_switch_provenance
running 5 tests
test s3_switch_catalog_unlistable_is_fail_closed ... ok
test s1_switch_after_retention_is_honest ... ok
test s0_switch_without_retention_history_is_complete ... ok
test s2_switch_retention_between_catalog_and_provenance_is_honest ... ok
test s4_switch_refresh_failure_after_stale_is_fail_closed ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [7-RFC-PATH] путей-кандидатов: всего=274 проверено=182 пропущено=92 — все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --check a660b3a7..18dec926
exit=0

$ git status --porcelain
<empty before adding this verdict>
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-10T23:00Z
- Milestone: M-98-switch-provenance
- Статус: BLOCKED — REJECT; dev dispatch remains blocked
- HEAD: 18dec926 — docs(M-98): круг 1 по C-297 — §8 таблица исполняема (red_m98_mutants.sh 6/6), шаг в verify_M-98, журнал кругов [architect]

## §B — Что я сделал
- Audited the committed artifact set at the fetched branch tip, including C-297, the full
  milestone, the runtime RED suite, mutation reproducer, prototype, acceptance script, and
  `VB-I-11`.
- Reproduced the committed six-world matrix and then demonstrated an all-green false-start
  implementation in a disposable audit worktree.

## §C — Артефакты / результаты
- `research/critiques/C-302-M-98-switch-provenance-round-2.md`
- Done Block above: matrix exit=0; controlled `completeone` mutant exit=0; merge-preview
  claim verifier exit=0; whitespace check exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter` (strong model, fresh context)
- **Paste-ready промпт:**
  ```
  Resolve the second plan-time REJECT for M-98 on origin/feat/M-98-switch-provenance.
  Read C-297 and C-302, milestones/M-98-switch-provenance.md, the full
  crates/gateway-serve/tests/red_m98_switch_provenance.rs, scripts/tests/red_m98_mutants.sh,
  scripts/tests/m98/prototype.patch, scripts/verify_M-98.sh, and docs/fa/viz-backend.md
  VB-I-11.  Measure the claimed `completeone` counterexample: it returns start=1 only for
  intact `s0` while all five scenarios stay green.  Decide whether VB-I-11 and the declared
  `(frozen_start_seq, true)` fallback require exact-start oracles in s0/s3/s4; state the
  required artifact changes and whether C-302 correctly blocks dev.  Write and push an
  arbitration decision artifact on the subject branch.
  ```
- Push-статус: pending this verdict commit and push to `origin/feat/M-98-switch-provenance`.
- Кэш: ⏸ audit worktree cache retained until the verdict is committed and pushed.

## §E — Риски / открытые вопросы
- B2 is closed; do not reopen its six-world matrix without a measured counterexample.
- This is an engineering dispute about oracle completeness, not a founder-only Boundary-C
  choice: arbitration, rather than founder approval, is required by `gates.md` §0.

=== END HANDOFF ===
