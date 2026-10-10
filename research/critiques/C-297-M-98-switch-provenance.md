<!-- GATE-META
milestone: M-98
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: dddb58f9ea0b5fea1119e60ed75fed6432535f94
verdict: REJECT
-->

# C-297 — M-98 SWITCH provenance: RED does not prove `history_start_seq`

## Verdict: REJECT

`VB-I-11` requires both provenance fields to describe the real retained history.  The
new RED suite proves the boolean in several worlds, but it permits a false
`history_start_seq`; therefore it is greenable by an implementation that still lies
about provenance.  The mutation table is also only prose: its prototype and mutants
are absent from the committed artifact set, so its claimed kill-set cannot be rerun.

## Subject and completeness

- Subject branch fetched by command: `origin/feat/M-98-switch-provenance`.
- Handoff reference `dddb58f9` equals the fetched tip and is an ancestor of it.
- Audited range: `a660b3a7..dddb58f9` (`ec85d138`, `dddb58f9`).
- Present: milestone, new runtime-RED suite, acceptance gate, and CI-map probe.
- T1 contracts and trait signatures: N/A — the committed range contains neither
  `contracts/**` nor implementation/signature changes; the milestone explicitly
  preserves wire form and existing fields.
- Scope is otherwise consistent with M-98 §10.  The explicit one-catalog prohibition
  is already executed indirectly by CI parity: `cargo test -p gateway-serve --features
  testing` includes the existing SWITCH oracle `red_m95_catalog_once::k4`.

## Blocking findings

### B1 — `history_start_seq` is not an oracle for truthful provenance

`red_m98_switch_provenance.rs:561-569` checks only `history_start_seq > 0` in `s1`.
`s2` checks no `history_start_seq` at all (`:591-598`), although its successful
freshness path is meant to compute current provenance.  Thus an implementation that
detects truncation correctly but reports an arbitrary positive value (for example
`1`) passes `s0…s4`; the result violates `VB-I-11`, which requires the sequence of
the first actually folded retained event, not merely a non-zero marker.

This is a concrete false-provenance form, not a hypothetical formatting preference:
the current failure already shows the pre-retention value is `0`, and the suite has no
independent expected sequence to distinguish a hard-coded positive value from the
true first sequence after retention.

**Condition to clear:** add an independent expected-start oracle for successful
retention worlds (`s1` and `s2`) and assert exact equality of the returned
`history_start_seq`.  The expected value must come from the post-retention journal,
not the transport's cached catalog or the code under test.  Keep the declared
`(frozen_start_seq, true)` fallback semantics separate for `s3`/`s4`.

### B2 — §8’s mutation table is not reproducible from committed artifacts

M-98 §8 says a rolled-back temporary prototype and three mutant forms were executed,
but the audited diff contains no `crates/gateway-serve/src/**` change or test-only
mutation driver: only `scripts/verify_M-98.sh` and its CI-map probe are added under
`scripts/`.  The verify script invokes the ordinary RED file once; it cannot select
the prototype, remove `is_fresh`, swallow `Err`, or force `true`.  Consequently the
rows in §8 are assertions in plan text, not executable evidence in the committed
artifact set.

**Condition to clear:** commit an executable, hermetic reproduction of the §8
prototype/mutant matrix (including the expected kill sets) and route it through the
M-98 acceptance gate.  It must fail closed if a declared mutant cannot be installed
or a scenario is not actually exercised.

## Checks that passed

- The base RED defect is real: on the audited source, `s1` returns
  `history_start_seq=0, history_truncated=false` after deletion of the earliest
  segment.
- `s0` and `s2…s4` are RED on the base because their required
  `m98-switch-catalog:<id>` rendezvous point is absent; this matches the committed
  source and the declared base-state reason.  `s1` fails on the functional defect,
  independently of that point.
- `red_m95_catalog_once::k4` is green on the audited source, establishing the existing
  one-catalog baseline for same-id resubscription.
- The CI-map probe passes all 10 adversarial cases.  In pull-request form, the dry
  parity pass accounts for all 64 `ci.yml` `run:` steps, including both one-line and
  block steps.
- Strong §9 recheck: `verify_design_claims.sh --merge-preview origin/main` passes;
  the diff does not touch the founder-locked process paths or a Boundary-C surface.
  No dangling code/document claim was found on the merge preview.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-98-switch-provenance
dddb58f9ea0b5fea1119e60ed75fed6432535f94
exit=0

$ git merge-base --is-ancestor dddb58f9 origin/feat/M-98-switch-provenance
exit=0

$ git diff --check a660b3a7..dddb58f9
exit=0

$ cargo test -q -p gateway-serve --features testing --test red_m98_switch_provenance -- --exact s1_switch_after_retention_is_honest
running 1 test
s1_switch_after_retention_is_honest --- FAILED
M-98 / TD-251 / VB-I-11: ... {"history_start_seq":0,"history_truncated":false,...}
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4 filtered out
exit=101   # expected RED against the unimplemented audited source

$ cargo test -q -p gateway-serve --features testing --test red_m95_catalog_once -- --exact k4_resubscribe_same_id_walks_catalog_once_more
running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) bash scripts/tests/red_verify_M-98_ci_map.sh
PASS  честный ci.yml: всё учтено (exit=0)
PASS  новый шаг с ${{ }} вне карты ⇒ FAIL (exit=1)
PASS  исключение без шага ⇒ карта протухла (exit=1)
PASS  строка дописана в блок базы ⇒ FAIL (C-266 F2)
PASS  строка дописана в блок агрегата ⇒ FAIL
PASS  агрегат: новый джоб в условии — законно ⇒ PASS
PASS  агрегат: команда в строке условия ⇒ FAIL
PASS  (i) check_review_fa + дописка ⇒ исполняется, не SKIP
PASS  (ii) run: |- — литеральный блок, исполняется
PASS  (iii) run: > — складывающий скаляр ⇒ FAIL
VERDICT: PASS — 10 сценариев
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=$(git rev-parse origin/main) VERIFY_M98_CI_DRY=1 bash scripts/verify_M-98.sh
PASS  ci-parity: учтено шагов 64 из 64 (исполнено 57, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [7-RFC-PATH] ... все 182 проверенных существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ git status --porcelain
<empty before adding this verdict>
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-10T22:10Z
- Milestone: M-98-switch-provenance
- Статус: BLOCKED — REJECT, dev dispatch is blocked
- HEAD: dddb58f9 — docs(ROADMAP): строка TD-251 → M-98 перед S1a-K [architect]

## §B — Что я сделал
- Audited the committed artifact set at `dddb58f9`, not the handoff prose.
- Checked RED base behavior, the same-id catalog-count oracle, CI-map mutation probe,
  pull-request-form parity map, scope, and the §9 merge-preview recheck.

## §C — Артефакты / результаты
- `research/critiques/C-297-M-98-switch-provenance.md`
- Done Block above; the RED source is intentionally failing before implementation.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Rework the committed M-98 artifact set on origin/feat/M-98-switch-provenance in
  response to C-297 (REJECT).  Do not dispatch dev.  Make `s1` and successful `s2`
  prove the exact post-retention `history_start_seq` via an independent journal-derived
  expectation, while preserving the specified frozen fallback in s3/s4.  Add a
  committed, hermetic reproducer for every §8 prototype/mutant row with asserted kill
  sets and wire it into scripts/verify_M-98.sh.  Commit and push the revised artifact
  set, then request a fresh critic audit of the new branch tip.
  ```
- Push-статус: ⏸ this verdict is being committed to `origin/feat/M-98-switch-provenance`.
- Кэш: ⏸ shared worktree cache retained while the concurrently started CI-form verify run finishes.

## §E — Риски / открытые вопросы
- The existing `red_m95_catalog_once::k4` is covered indirectly by CI parity; do not
  remove that coverage while revising the M-98 acceptance map.
- A second REJECT for the same unproved exact-start invariant requires arbitration per
  `gates.md` §0; this is the first recorded round.

=== END HANDOFF ===
