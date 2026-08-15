# C-089 — M-60b barriers: adversarial harness audit

<!-- GATE-META
milestone: M-60b
audited_repo: a3ka/hft-platform
audited_base: 40ea8ea50de6dcc2e7b4278dab7eba1d450aa0de
audited_head: e6f3749b02322b558f33a96a906f0ec0bd0d3436
verdict: REJECT
-->

Date: 2026-08-15 UTC
Role: critic
Stakes: high — harness track adversarial audit
Subject: `origin/feat/M-60b-barriers @ e6f3749`

## Verdict

**REJECT.** The submitted range cannot pass the real `gate-meta` CI job, and the disk
barrier accepts an external Cargo target hidden behind an in-tree symlink. Both are
false-green surfaces in mechanisms whose purpose is to prevent silent gate bypass.

The required committed artifact set is otherwise present: milestone, all three
`check_*.sh` barriers, three RED probes, `verify_M-60b.sh`, and CI wiring. T-contracts and
trait signatures are N/A: `40ea8ea..e6f3749` has no `contracts/**` or `crates/**` changes.

## Blockers

### F-089-1 — real GATE-META barrier rejects the submitted subject range

`check_gate_meta.sh` correctly judges every added or modified verdict artifact in the
event range. The subject range adds `C-062` and `C-083` without a `GATE-META` header, so
the actual CI invocation is red. `verify_M-60b.sh` runs only the synthetic probe (step G),
not the real barrier over its own `merge-base..HEAD`; it therefore reports green although
the `gate-meta` CI job will fail.

This is a release blocker, not a historical-artifact exception: those files are added in
this submitted range and are exactly the files `git diff --diff-filter=AM` must judge.

Required condition for re-review: make the actual event-range invocation green and have
the local acceptance gate exercise that same production invocation, so probe-green cannot
diverge from CI-red.

### F-089-2 — disk guard accepts a physical target outside the repository

`check_disk_budget.sh` compares only a text-normalized `CARGO_TARGET_DIR` to `pwd`; it does
not resolve an existing symlink. An in-tree symlink to an external writable directory
therefore passes. This violates the stated invariant that the Cargo target be inside the
current tree and reopens the precise stale/external-binary surface the guard exists to
prevent.

Required condition for re-review: reject an existing target whose physical resolution
escapes the worktree, and add an adversarial RED scenario for that symlink escape. The
acceptance preamble must exercise the strengthened production path.

### F-089-3 — probe-shaped calls can bypass the disk guard's default production path

The disk RED probe supplies `MIN_FREE_KB` on every invocation. An adversarial barrier that
delegates only when that variable is set passes `red_disk_budget.sh` 13/13; its default path
can return success without calling `df` or validating `CARGO_TARGET_DIR`. The verify
preamble only accepts exit 0, so it does not independently prove that default mode measured
either resource. This is an anti-placebo blocker under the harness track.

For comparison, the analogous context-budget stub also passes its 13 RED cases, but
`verify_M-60b.sh` step D independently tests its default mode and catches the bypass. The
disk path has no equivalent default-mode adversary.

## Positive checks

- Probes executed as submitted: context budgets **13/13**, GATE-META **27/27**, disk
  budget **13/13**.
- Mutation controls behaved as follows: `-le → -ge` stopped at positive setup; removing
  the audited-head ancestry check produced exactly GM-6; replacing PATH-resolved `df` with
  `/usr/bin/df` produced exactly DB-8 (not setup failure).
- CI aggregation is structurally correct: the `needs` set and `needs.<job>.result` set are
  equal, and each contains `context-budgets` and `gate-meta` once. `gate-meta` has
  `fetch-depth: 0` plus `EVENT_NAME`, `PUSH_BEFORE`, and `PR_BASE_SHA` from the event.
- `bash scripts/verify_M-60b.sh` completed with its default timeouts and returned
  `VERDICT: PASS`, exit 0. This confirms the acceptance gate's blind spots; it does not
  override the failing production `check_gate_meta.sh` invocation above.

## Done Block

```text
$ bash scripts/tests/red_context_budgets.sh
VERDICT: PASS (13/13) — бюджет держит и не даёт ложных срабатываний
context_exit=0

$ bash scripts/tests/red_gate_meta.sh
VERDICT: PASS (27/27) — вердикт привязан к предмету, лок держит, отсутствие наблюдаемо
gate_meta_exit=0

$ bash scripts/tests/red_disk_budget.sh
VERDICT: PASS (13/13) — красное названо до старта, отказы не маскируются
disk_exit=0

$ BARRIER=/tmp/.../context-ge.sh bash scripts/tests/red_context_budgets.sh
SETUP НЕ СОСТОЯЛСЯ: барьер не проходит заведомо годную фикстуру (64/32 B при лимите 128 B); setup не состоялся
context_ge_exit=1

$ BARRIER=/tmp/.../gate-no-audited-head-ancestor.sh bash scripts/tests/red_gate_meta.sh
FAIL  GM-6 audited_head не из этой линии истории прошёл
VERDICT: FAIL (1)
gate_no_ancestor_exit=1

$ BARRIER=/tmp/.../disk-absolute-df.sh bash scripts/tests/red_disk_budget.sh
FAIL  DB-8 порог 4243 KB против подставного free 4242 KB ПРОШЁЛ — барьер не читает df через PATH, значит DB-2b проверял не границу
VERDICT: FAIL (1)
disk_absolute_df_exit=1

$ BARRIER=/tmp/.../context-default-bypass.sh bash scripts/tests/red_context_budgets.sh
VERDICT: PASS (13/13) — бюджет держит и не даёт ложных срабатываний
context_stub_probe_exit=0
$ ROOT=/definitely/not/a/repository BARRIER=/tmp/.../context-default-bypass.sh bash /tmp/.../context-default-bypass.sh
context_stub_invalid_default_exit=0

$ BARRIER=/tmp/.../disk-default-bypass.sh bash scripts/tests/red_disk_budget.sh
VERDICT: PASS (13/13) — красное названо до старта, отказы не маскируются
disk_stub_probe_exit=0
$ env -u MIN_FREE_KB CARGO_TARGET_DIR=/tmp bash /tmp/.../disk-default-bypass.sh
disk_stub_outside_default_exit=0

$ (cd /tmp/hft-m60b-symlink-root.GkileL && MIN_FREE_KB=1 CARGO_TARGET_DIR=/tmp/hft-m60b-symlink-root.GkileL/target-link bash /tmp/hft-codex-critic-1786820167/scripts/check_disk_budget.sh)
logical_target=/tmp/hft-m60b-symlink-root.GkileL/target-link
physical_target=/tmp/hft-m60b-symlink-outside.PbWvK9
OK: свободно 117774680 KB ≥ порога 1 KB; CARGO_TARGET_DIR=/tmp/hft-m60b-symlink-root.GkileL/target-link
symlink_target_exit=0

$ EVENT_NAME=push PUSH_BEFORE=40ea8ea50de6dcc2e7b4278dab7eba1d450aa0de PR_BASE_SHA='' bash scripts/check_gate_meta.sh
── GATE-META: диапазон 40ea8ea5..HEAD, origin=a3ka/hft-platform
FAIL  research/critiques/C-062-M-60-mechanisms.md: нет шапки GATE-META — вердикт ничем не привязан к предмету
FAIL  research/critiques/C-083-M-60-rebuild.md: нет шапки GATE-META — вердикт ничем не привязан к предмету
VERDICT: FAIL (2) — вердикт не привязан к предмету либо merge прошёл без вердикта.
gate_meta_subject_exit=1

$ compare status-check needs with needs.<job>.result
needs_minus_condition:
condition_minus_needs:
context_in_needs=1 context_in_condition=1
gate_meta_in_needs=1 gate_meta_in_condition=1
gate-meta: fetch-depth=0; EVENT_NAME/PUSH_BEFORE/PR_BASE_SHA supplied from github event

$ bash scripts/verify_M-60b.sh
PASS  CI cargo fmt --check
PASS  CI cargo clippy -D warnings
PASS  CI cargo test --all (замер rev3 1531.74s)
VERDICT: PASS
verify_exit=0
```
