# C-088 — M-60 rebuild rev3 plan-time gate

<!-- GATE-META
milestone: M-60
audited_repo: a3ka/hft-platform
audited_base: c6c62b8b8218ee0564213bfd3d2ceb7921093197
audited_head: f52ef6bf8361d96879daed239dc0866d1d775640
verdict: ESCALATE
-->

> **Врезка о происхождении файла — перенос, а не авторство (architect, 2026-08-15).**
> Текст ниже написан критиком (codex/gpt-5, сильная модель, свежий контекст) и вынесен им
> 2026-08-15T00:33Z. Критик записал вердикт в `/tmp/critic-m60-r3/.omc/plans/critic-M-60-2026-08-15-0033.md`
> — вне репозитория — и завершил работу со статусом «repo branch not modified». Это нарушение
> `gates.md` §4: «вердикт — АРТЕФАКТ, а не сообщение», предъявляется ФАЙЛОМ в
> `research/critiques/`, закоммиченным на ветку предмета. Ровно этот класс дал инцидент M-49,
> где два REJECT'а испарились вместе с транскриптом субагента.
>
> Что сделал architect: перенёс текст ДОСЛОВНО, ничего не убрав и не добавив по существу;
> присвоил идентификатор `C-088` механизмом (`scripts/next_artifact_id.sh C`, номер `C-087`
> занят вердиктом M-66); собрал машинную шапку `GATE-META` из полей, уже названных самим
> вердиктом (branch/audited head/audited base/verdict). Суждение критика не редактировалось.
>
> Что architect НЕ делает и делать не вправе: не засчитывает этот вердикт за прохождение
> гейта в свою пользу. Вердикт — `ESCALATE`, и по `gates.md` §0 предмет идёт к арбитру, а не
> к dev. Architect в этом споре СТОРОНА (автор предмета), поэтому арбитра назначает founder.

---

# Critic verdict — M-60 rebuild round 3

Date: 2026-08-15T00:33Z
Agent: critic
Model: codex/gpt-5, strong / fresh context
Branch: origin/docs/M-60-rebuild-2026-08-14
Audited head: f52ef6bf8361d96879daed239dc0866d1d775640
Audited base: c6c62b8b8218ee0564213bfd3d2ceb7921093197

## Verdict

ESCALATE.

Material audit result: C-083 and C-086 blockers are closed; I found no new REJECT-class defect in
the committed artifact set.

Process result: this is round 3 on the same subject. `gates.md` §0 requires arbiter on "три
круга по одному предмету" in any verdict combination. This escalation is to arbiter, not founder:
no boundary-C decision is present in this artifact set.

## Artifact Set

Committed in `c6c62b8..f52ef6b`:

- Milestones: `milestones/M-60b-gate-mechanisms.md`, `milestones/M-60c-corpus-cleanup.md`
- RED probes: `scripts/tests/red_context_budgets.sh`, `scripts/tests/red_gate_meta.sh`,
  `scripts/tests/red_disk_budget.sh`
- Verify scripts: `scripts/verify_M-60b.sh`, `scripts/verify_M-60c.sh`
- Prior verdicts: `research/critiques/C-083-M-60-rebuild.md`,
  `research/critiques/C-086-M-60-rebuild-rev2.md`
- T-contracts / trait signatures: N/A. No `crates/**` or `contracts/**` path is changed.

## Closure Check

### C-083 F-083-1 — CLOSED

The audited branch now carries executable RED probes and verify scripts. This is no longer a
plan-text-only review.

Evidence:

```text
$ git diff --stat c6c62b8..HEAD
10 files changed, 2699 insertions(+)
```

### C-083 F-083-2 — CLOSED

The branch no longer recommends deleting `feat/M-60-mechanisms`. `M-60b` §12 explicitly
disposes of the old branch's full diff-list, including the previously missing
`.claude/rules/gates.md` and `.github/workflows/ci.yml`, and keeps branch deletion under founder
decision after disposition of `milestones/M-60-mechanisms.md` and `scripts/verify_M-60.sh`.

### C-086 F-086-1 — CLOSED

All three probes now classify 126/127 from an existing barrier as setup failure, after a
positive-control path succeeds.

Evidence:

```text
$ BARRIER=.omc/tmp-stubs/c1_budgets_good_then_127.sh bash scripts/tests/red_context_budgets.sh
SETUP positive-control: барьер принимает заведомо годную фикстуру
SETUP НЕ СОСТОЯЛСЯ: барьер вернул 127 ...
exit=1

$ BARRIER=.omc/tmp-stubs/c1_gatemeta_good_then_127.sh bash scripts/tests/red_gate_meta.sh
SETUP positive-control: барьер принимает заведомо годную GATE-META-фикстуру
SETUP НЕ СОСТОЯЛСЯ: барьер вернул 127 ...
exit=1

$ BARRIER=.omc/tmp-stubs/c1_disk_good_then_127.sh bash scripts/tests/red_disk_budget.sh
SETUP positive-control: барьер принимает заведомо годную диск-фикстуру
SETUP НЕ СОСТОЯЛСЯ: барьер вернул 127 ...
exit=1
```

Relevant code: `red_context_budgets.sh` lines 86-99, `red_gate_meta.sh` lines 149-161,
`red_disk_budget.sh` lines 58-73.

### C-086 F-086-2 — CLOSED

The missing kill-set axes are now present and executable.

Evidence:

```text
$ BARRIER=s5_budgets_honest.sh bash scripts/tests/red_context_budgets.sh
VERDICT: PASS (13/13)

$ BARRIER=s5_gatemeta_honest.sh bash scripts/tests/red_gate_meta.sh
VERDICT: PASS (27/27)

$ BARRIER=s5_disk_honest.sh bash scripts/tests/red_disk_budget.sh
VERDICT: PASS (13/13)

$ BARRIER=c2_budgets_ignores_limit.sh bash scripts/tests/red_context_budgets.sh
FAIL CB-10a ...
FAIL CB-10b ...
VERDICT: FAIL (2)

$ BARRIER=c3_disk_never_calls_df.sh bash scripts/tests/red_disk_budget.sh
FAIL DB-8 ...
VERDICT: FAIL (1)

$ BARRIER=c4_gatemeta_hardcoded_origin.sh bash scripts/tests/red_gate_meta.sh
FAIL GM-24 ...
VERDICT: FAIL (1)
```

Scenario count by file:

```text
red_context_budgets.sh declared_run_barrier=13
red_gate_meta.sh declared_run_barrier=27
red_disk_budget.sh declared_run_barrier=13
```

### C-086 F-086-3 — CLOSED

The verify scripts explicitly narrow CI parity according to the current `gates.md` §3 contract:
base Rust trio always, specialized jobs only by touched zone. Both scripts are aggregators and
now reach a terminal `VERDICT` on the red path.

Evidence with short cargo timeouts to exercise terminal handling without waiting for the full
25m `cargo test --all` path:

```text
$ CARGO_FMT_TIMEOUT_SECONDS=1 CARGO_CLIPPY_TIMEOUT_SECONDS=1 CARGO_TEST_TIMEOUT_SECONDS=1 bash scripts/verify_M-60b.sh
...
VERDICT: FAIL (21)
verify_exit=1

$ CARGO_FMT_TIMEOUT_SECONDS=1 CARGO_CLIPPY_TIMEOUT_SECONDS=1 CARGO_TEST_TIMEOUT_SECONDS=1 bash scripts/verify_M-60c.sh
...
VERDICT: FAIL (13)
verify_exit=1
```

`verify_M-60b.sh` also adds D3-setup before the per-file disappearance loop; `verify_M-60c.sh`
adds B-setup so a missing corpus cannot pass by having zero lines.

### C-086 handoff item 4 — CLOSED

The stale `gates.md` and `ci.yml` deltas from `origin/feat/M-60-mechanisms` are explicitly named
and discarded in `M-60b` §12.

## Additional Setup-Safety Check

I tested the `cd ""` failure class directly with a fake `git` that logs its cwd and exits 1.
The probe failed at fixture initialization; no git command ran in the audited worktree and HEAD
did not move.

```text
probe_exit=1
SETUP НЕ СОСТОЯЛСЯ: инициализация фикстуры
head_before=f52ef6bf8361d96879daed239dc0866d1d775640
head_after=f52ef6bf8361d96879daed239dc0866d1d775640
fake_git_cwd_count=1
fake_git_worktree_hits=0
fake_git_cwds_sample:
/tmp/red-gatemeta-uVa7ar
```

## Barrier Checks

```text
$ bash -n scripts/tests/red_context_budgets.sh scripts/tests/red_gate_meta.sh scripts/tests/red_disk_budget.sh scripts/verify_M-60b.sh scripts/verify_M-60c.sh
syntax_exit=0 for all five files

$ EVENT_NAME=push PUSH_BEFORE=c6c62b8... bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD
protected_exit=0

$ EVENT_NAME=push PUSH_BEFORE=c6c62b8... bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона c6c62b8..HEAD не ввёл второй носитель под занятым идентификатором
artifact_ids_exit=0
```

## Handoff

Next agent: arbiter.

Reason: mandatory `gates.md` §0 process escalation after three rounds on M-60. The arbiter should
read C-083, C-086, this file, and the committed artifact set at `f52ef6b`, then decide whether
the M-60 artifact set may proceed to architect mechanical appendix / dev dispatch.
