# C-086 — M-60 rebuild rev2 plan-time gate

<!-- GATE-META
milestone: M-60
audited_repo: a3ka/hft-platform
audited_base: c6c62b8b8218ee0564213bfd3d2ceb7921093197
audited_head: 61ae26cf9d3474ae2914c0eb8de97f793f3e19d2
verdict: REJECT
-->

Date: 2026-08-14
Agent: critic
Model: codex/gpt-5, strong per `gates.md` §1 mandate
Branch: `origin/docs/M-60-rebuild-2026-08-14`
Audited base: `c6c62b8b8218ee0564213bfd3d2ceb7921093197`
Audited head: `61ae26cf9d3474ae2914c0eb8de97f793f3e19d2`

## Verdict

REJECT.

Round 1 blocker F-083-1 is closed only in the narrow sense that the artifact set now exists:
`scripts/tests/red_context_budgets.sh`, `scripts/tests/red_gate_meta.sh`,
`scripts/tests/red_disk_budget.sh`, `scripts/verify_M-60b.sh`, and `scripts/verify_M-60c.sh`
are committed on the audited subject. T-contracts / trait signatures are N/A for this process
milestone; no `crates/**` or `contracts/**` paths are in the merge-base diff.

The new artifacts are not passable. The setup guards prove "file absent" but do not distinguish
an existing barrier that internally returns 127 from an existing barrier that honestly returns 1.
The kill-set also leaves concrete axes uncovered, and the verify scripts do not satisfy the
dispatcher's CI-parity requirement.

This is not the same blocker as C-083 F-083-1 ("artifacts absent"); it is the first audit of the
new artifact set.

## Findings

### F-086-1 — BLOCKER — Anti-placebo guard still treats `exit 127` like honest gate failure

The mandate required a direct anti-placebo check: provide a barrier stub returning 127 and a
barrier honestly returning 1; outcomes must differ. They do not differ for any of the three RED
probes.

The cause is visible in the probes:

- `scripts/tests/red_context_budgets.sh:36-39` checks only that the barrier file exists and parses;
  `run_barrier` at `:57-59` discards output and all nonzero statuses are interpreted by each
  scenario as "gate refused".
- `scripts/tests/red_gate_meta.sh:67-68` has the same file/syntax guard; `run_barrier` at
  `:108-110` again collapses all nonzero statuses.
- `scripts/tests/red_disk_budget.sh:34-37` guards file/syntax only; `run_barrier` at `:46-50`
  does not classify status 127 specially.

Measured result:

```text
$ BARRIER=<missing> bash scripts/tests/red_context_budgets.sh
SETUP НЕ СОСТОЯЛСЯ: барьера нет ... exit=1

$ BARRIER=<stub exit 127> bash scripts/tests/red_context_budgets.sh
PASS CB-1, FAIL CB-2, FAIL CB-3, PASS CB-4, PASS CB-5, PASS CB-6, PASS CB-6b
VERDICT: FAIL (2)

$ BARRIER=<stub exit 1> bash scripts/tests/red_context_budgets.sh
PASS CB-1, FAIL CB-2, FAIL CB-3, PASS CB-4, PASS CB-5, PASS CB-6, PASS CB-6b
VERDICT: FAIL (2)

$ BARRIER=<stub exit 127> bash scripts/tests/red_gate_meta.sh
VERDICT: FAIL (7)

$ BARRIER=<stub exit 1> bash scripts/tests/red_gate_meta.sh
VERDICT: FAIL (7)

$ BARRIER=<stub exit 127> bash scripts/tests/red_disk_budget.sh
VERDICT: FAIL (6)

$ BARRIER=<stub exit 1> bash scripts/tests/red_disk_budget.sh
VERDICT: FAIL (6)
```

Condition to clear: each RED probe must classify 127 from an existing barrier as setup failure
or otherwise produce a distinct outcome from an honest refusal. A syntactically valid no-op
barrier that returns 127 must not be accepted as "the gate blocked the bad case".

### F-086-2 — BLOCKER — Kill-set axes are still incomplete in concrete places

`testing.md` says degenerate input is mandatory and an oracle is not ready if an applicable axis
is uncovered. The new probes cover useful cases, but gaps remain:

- `red_context_budgets.sh`: covers asymmetry, two files, absence, and exact/+1 boundaries, but
  does not cover carrier permissions. An unreadable file or unreadable budget table can still be
  mishandled without this probe noticing. It also does not cover duplicate/conflicting table rows,
  so a parser that checks only the first life of a path can pass.
- `red_gate_meta.sh`: covers absence of metadata and the new GM-17/18/19 absence-of-reviewer
  class, but does not cover multiplicity. There is no scenario with two changed verdict artifacts
  where one is valid and one invalid, nor two merge commits where the first has a review and the
  second does not. An implementation that checks only the first artifact/merge can pass the probe.
  The subject-lock path class also is not fully pinned: it tests `scripts/verify_*.sh`,
  `.claude/rules/**`, and `.github/workflows/**`, but not `scripts/check_*.sh` or
  `scripts/tests/red_*.sh`, even though those are the mechanism artifacts being protected.
- `red_disk_budget.sh`: covers disk-only failure, target-only failure, invalid thresholds, and
  double failure, but does not cover the exact equality boundary (`MIN_FREE_KB == actual free KB`)
  despite the contract being `free >= threshold`. It also does not cover an unwritable/read-only
  target/current tree, the concrete carrier-permission failure mode for a verify preamble.

Number-of-consumers is not a natural fit for the stateless context-budget parser. For GATE-META
and disk/target checks, the missing multiplicity/shared-carrier cases above are the applicable
forms.

### F-086-3 — BLOCKER — `verify_M-60b.sh` / `verify_M-60c.sh` do not meet the requested CI parity

The dispatcher explicitly required parity by `grep -E "run:" .github/workflows/ci.yml`: every CI
`run:` command must have a point in verify. The two scripts include the base Rust trio
(`fmt`, `clippy`, `test`) and selected local probes, but omit many current CI commands:

- security: `cargo install cargo-audit --locked`, `cargo audit`
- delivery: `bash scripts/verify_delivery_M-08.sh`
- protected artifacts: `bash scripts/check_protected_artifacts.sh`
- docs-freeze / worktree: `bash scripts/check_docs_freeze.sh` in M-60b, `red_gc_reclaim_args.sh`
  in both
- contracts: `pip install --quiet jsonschema`, `verify_contracts.sh`,
  `verify_ct_rfc_atomic.sh`, `red_ct_rfc_atomic.sh`, `diff_contract_schema.sh`,
  `red_diff_contract_schema.sh`
- artifact IDs: `check_artifact_ids.sh`, and the `red_artifact_ids.sh --battery` half
- design claims: `verify_design_claims.sh`, `red_verify_design_claims.sh`

References: CI commands are in `.github/workflows/ci.yml:20-24,32-35,45,62,68,78,103,110,119,
131,136,147-163,167,171,177,194,199-201,223,229`. M-60b only covers a subset at
`scripts/verify_M-60b.sh:201-224`; M-60c only covers a subset at
`scripts/verify_M-60c.sh:179-214`.

The full verify runs also did not produce their own terminal `VERDICT` in this audit:

- `verify_M-60b.sh` was manually terminated after 9m47s, while `cargo test --all` was still
  running `red_floor_work_budget` (the log had reached fmt/clippy PASS).
- `timeout 600 bash scripts/verify_M-60c.sh` exited 124 after reaching `cargo test --all`;
  the log had reached fmt/clippy PASS and no final `VERDICT` line.

Condition to clear: either make the verify scripts cover the requested CI run-list and return a
terminal verdict under the actual command, or explicitly narrow the acceptance contract and get
that narrower contract approved before dispatch.

## Checks That Passed

- K-3 is implemented in the spec and probe text: GM-16 is explicitly not written, the number is
  burned, and `milestone:` remains a declaration (`milestones/M-60b-gate-mechanisms.md:80-91`,
  `scripts/tests/red_gate_meta.sh:25-31,226`).
- K-4 is implemented at the artifact level: GM-17/GM-18/GM-19 exist and exercise absence of a
  reviewer verdict for merge commits naming `M-NN` (`milestones/M-60b-gate-mechanisms.md:93-108`,
  `scripts/tests/red_gate_meta.sh:228-268`).
- F-083-2 is materially improved: the branch is no longer recommended for deletion, and
  `milestones/M-60b-gate-mechanisms.md:211-227` requires founder disposition for the umbrella
  milestone, old `verify_M-60.sh`, and roadmap/BACKLOG deltas before deletion.
- `C-062` is landed byte-identically; M-60a gate artifacts listed by the old branch are already
  byte-identical in `origin/main`.
- `check_artifact_ids.sh` and `check_protected_artifacts.sh` are green on `c6c62b8..HEAD`.

Note on F-083-2: the exact `git diff --name-only origin/main...origin/feat/M-60-mechanisms` list
still includes stale `.claude/rules/gates.md` and `.github/workflows/ci.yml` deltas. They appear
superseded by `main`, but the canonical six-point deletion checklist does not name them
explicitly as discarded. This is not the primary blocker, but the inventory is not literally the
full diff list the dispatcher asked to compare.

## Done Block

```text
$ git log --oneline --decorate -5
61ae26c (HEAD, origin/docs/M-60-rebuild-2026-08-14) test(M-60): C-083 F-083-1 — RED-наборы и verify легли НА ПРЕДМЕТ [architect]
f87afdb docs(critic): C-083 — M-60 rebuild verdict REJECT [critic]
f98af2c (docs/M-60-rebuild-2026-08-14) docs(M-60b): пересборка спеки от main — G1/G3/G6.2 без G2 (сделан), правки К-3/К-4, GM-16 сожжён [architect-clone]
243eb51 docs(M-60c): пересборка спеки от main — чистка первой, числа от факта 14.08, G4/G5 строками шаблонов [architect-clone]
fda1a68 docs(M-60): salvage C-062 с feat/M-60-mechanisms — аудит-трейл «критик в чужом репо», единственный экземпляр [architect-clone]

$ git merge-base HEAD origin/main
c6c62b8b8218ee0564213bfd3d2ceb7921093197

$ git diff --name-only c6c62b8b8218ee0564213bfd3d2ceb7921093197..HEAD
milestones/M-60b-gate-mechanisms.md
milestones/M-60c-corpus-cleanup.md
research/critiques/C-062-M-60-mechanisms.md
research/critiques/C-083-M-60-rebuild.md
scripts/tests/red_context_budgets.sh
scripts/tests/red_disk_budget.sh
scripts/tests/red_gate_meta.sh
scripts/verify_M-60b.sh
scripts/verify_M-60c.sh

$ for f in scripts/tests/red_context_budgets.sh scripts/tests/red_gate_meta.sh scripts/tests/red_disk_budget.sh scripts/verify_M-60b.sh scripts/verify_M-60c.sh; do bash -n "$f"; echo "$f syntax_exit=$?"; done
scripts/tests/red_context_budgets.sh syntax_exit=0
scripts/tests/red_gate_meta.sh syntax_exit=0
scripts/tests/red_disk_budget.sh syntax_exit=0
scripts/verify_M-60b.sh syntax_exit=0
scripts/verify_M-60c.sh syntax_exit=0

$ BARRIER=/tmp/no-such-barrier bash scripts/tests/red_context_budgets.sh; echo exit=$?
SETUP НЕ СОСТОЯЛСЯ: барьера нет: /tmp/no-such-barrier. 127 от bash неотличим от честного отказа гейта.
exit=1

$ BARRIER=<stub exit 127> bash scripts/tests/red_context_budgets.sh; echo exit=$?
VERDICT: FAIL (2)
exit=1

$ BARRIER=<stub exit 1> bash scripts/tests/red_context_budgets.sh; echo exit=$?
VERDICT: FAIL (2)
exit=1

$ BARRIER=/tmp/no-such-barrier bash scripts/tests/red_gate_meta.sh; echo exit=$?
SETUP НЕ СОСТОЯЛСЯ: барьера нет: /tmp/no-such-barrier. 127 от bash неотличим от честного отказа гейта.
exit=1

$ BARRIER=<stub exit 127> bash scripts/tests/red_gate_meta.sh; echo exit=$?
VERDICT: FAIL (7)
exit=1

$ BARRIER=<stub exit 1> bash scripts/tests/red_gate_meta.sh; echo exit=$?
VERDICT: FAIL (7)
exit=1

$ BARRIER=/tmp/no-such-barrier bash scripts/tests/red_disk_budget.sh; echo exit=$?
SETUP НЕ СОСТОЯЛСЯ: барьера нет: /tmp/no-such-barrier. 127 от bash неотличим от честного отказа гейта.
exit=1

$ BARRIER=<stub exit 127> bash scripts/tests/red_disk_budget.sh; echo exit=$?
VERDICT: FAIL (6)
exit=1

$ BARRIER=<stub exit 1> bash scripts/tests/red_disk_budget.sh; echo exit=$?
VERDICT: FAIL (6)
exit=1

$ CARGO_TARGET_DIR=/tmp/hft-critic-m60-r2/target bash scripts/verify_M-60b.sh
... reached:
PASS  CI cargo fmt --check
PASS  CI cargo clippy -D warnings
terminated after 9m47s while cargo test --all was still running red_floor_work_budget
exit=143

$ CARGO_TARGET_DIR=/tmp/hft-critic-m60-r2/target timeout 600 bash scripts/verify_M-60c.sh
FAIL  B .claude/rules = 1069 строк, бюджет 725 — превышение на 344
FAIL  B CLAUDE.md = 100 строк, бюджет 70 — превышение на 30
PASS  D все 40 контрольных норм на месте
FAIL  M «Дегенерированный вход обязателен» не переехала в профиль architect'а (в профиле нет)
FAIL  M «Форма прода снимается ЗАМЕРОМ» не переехала в профиль architect'а (в профиле нет)
FAIL  M «мерить ТО, ЧТО ОБЕЩАЕТ» не переехала в профиль architect'а (в профиле нет)
FAIL  M «Целостность гейта — 4 свойства» не переехала в профиль architect'а (в профиле нет)
FAIL  M «RED сравнения ДВУХ источников» не переехала в профиль architect'а (в профиле нет)
FAIL  Dd дубль замера в впрыскиваемом ядре: .claude/rules/handoff-block.md .claude/rules/branch-hygiene.md .claude/rules/gates.md
PASS  Dd канонический носитель замера (gc_worktrees.sh) существует
FAIL  A архива нет или пуст — вырезанное должно переезжать, а не исчезать
FAIL  T G4: предиката AHEAD= в шаблоне §D нет
FAIL  T G5: строки прозрачности гейтинга критика в шаблоне §B нет
PASS  S замок §11 зелёный на диапазоне c6c62b8..HEAD (токены на месте)
PASS  P red_protected_artifacts: зелёная (20 исполнено)
PASS  P red_docs_freeze: зелёная (27 исполнено)
PASS  P red_commit_paths: зелёная (8 исполнено)
PASS  CI cargo fmt --check
PASS  CI cargo clippy -D warnings
exit=124

$ bash scripts/check_artifact_ids.sh c6c62b8b8218ee0564213bfd3d2ceb7921093197; echo artifact_ids_exit=$?
OK: ни один коммит диапазона c6c62b8..HEAD не ввёл второй носитель под занятым идентификатором
artifact_ids_exit=0

$ bash scripts/check_protected_artifacts.sh c6c62b8b8218ee0564213bfd3d2ceb7921093197; echo protected_exit=$?
OK: защищённые артефакты целы на HEAD (c6c62b8..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
protected_exit=0
```

## Handoff

REJECT → architect.

Required correction:

1. Strengthen all three RED probes so an existing barrier returning 127 is not indistinguishable
   from an honest gate refusal.
2. Close the concrete kill-set gaps listed in F-086-2, or explicitly prove why a named axis is
   not applicable to that gate.
3. Make `verify_M-60b.sh` and `verify_M-60c.sh` satisfy the dispatcher's CI-parity contract and
   return a terminal `VERDICT` under the actual acceptance command.
4. When revising the branch-deletion checklist, explicitly dispose of the stale `gates.md` and
   `ci.yml` deltas from `origin/feat/M-60-mechanisms` or state why they are covered by an existing
   discard bucket.

