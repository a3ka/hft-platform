<!-- GATE-META
milestone: C-101
audited_repo: a3ka/hft-platform
audited_base: c4cfb8564fb5549060762c7056485065557afee0
audited_head: dee67f12e7fb49ce18e779fd6e21cd202e8a60b5
verdict: REJECT
-->

# C-179 — REJECT: harness adversary audit `harness-milestone-shape`, round 8

## Subject and applicable route

- Audited branch: `feat/harness-milestone-shape`; base
  `c4cfb8564fb5549060762c7056485065557afee0`; prior audited tip
  `cda46001ad958a25082de0e47d10b21b878aaace`; audited tip
  `dee67f12e7fb49ce18e779fd6e21cd202e8a60b5`.
- The committed set is the harness-track set:
  `scripts/check_milestone_shape.sh`,
  `scripts/tests/red_milestone_shape.sh`, and the `milestone-shape` CI
  wiring. The only other changed path since the prior audited tip is the
  preceding verdict `C-178`. Under `docs/workflow/harness-track.md` §§2–3,
  this route intentionally has no T-contract, trait signature, product RED
  suite, `verify_M-*`, or milestone file: barrier and probe are the artifacts
  to audit.
- The route nevertheless requires the fresh adversary verdict, production-form
  invocation, and red results against deceptive live-derived stubs
  (`harness-track.md` §§3, 5; `testing.md`, gate-integrity properties 1–3).

## Verdict: REJECT

Round 7 genuinely fixes the `grep -q`/pipefail defect: the restored pipe
fails the same production-scale scenario three times in a row. It does not
close the gate. Two parts of the live promise are outside the probe's observed
space, so both false-green and false-red regressions can pass the complete
probe.

### B-14 — the `push` production invocation is untested and can skip an incomplete spec

CI invokes the job for both `pull_request` and `push`
(`.github/workflows/ci.yml:3-6`, `:222-226`). The checker chooses a
different base input in each branch: `PR_BASE_SHA` for a PR and
`PUSH_BEFORE` for a push (`scripts/check_milestone_shape.sh:93-98`).
Every probe invocation, including all three base fail-closed cases and the
non-ancestor case, sets only `EVENT_NAME=pull_request`
(`red_milestone_shape.sh:301,443,446,471`).

I changed only the live `push)` arm from `PUSH_BEFORE` to
`git rev-parse HEAD`. The full probe under that mutant remained green:
`PASS=53 FAIL=0`, exit 0. On a fresh two-commit repository whose pushed
commit adds an incomplete `M-99`, the honest checker under the real push form
rejects it (exit 1); the mutant compares `HEAD..HEAD`, says there are no new
specs, and returns 0. This is a live false green on exactly the required
section-form invariant, caused by an unobserved event environment rather than
a valid specification.

The CI claim that the probe runs the same production form is consequently false
for half of the job's trigger surface. This violates harness-track §3's
production-form requirement and `testing.md` gate-integrity property 1.

### B-15 — tags and opening terminators are enumerated separately, not as the promised 4×4 grammar

Round 7 introduced `HTML_TAGS` and `HTML_OPEN_FORMS`, but the probe runs the
four tags only with the `>` form (`red_milestone_shape.sh:391-393`) and the
four opening forms only for `script` (`:399-402`). It never observes such
valid declared forms as `<style<TAB>src="x.css">`. The CI comment's
`4 тега × 4 терминатора` wording is therefore not true of the fixtures.

I derived a one-line mutant from the live raw-HTML opener at
`check_milestone_shape.sh:170`: all non-`style` tags retain `[ \t>]`, while
`style` retains only `[ >]`; the end-of-line arm is untouched. It removes
exactly the `style × tab` member while preserving all 15 other combinations.
The source differs (exit 1 from `cmp`), but the entire self-test probe stays
green: `PASS=53 FAIL=0`, exit 0.

On a committed incomplete milestone whose only `Allowed paths` candidate is
inside `<style<TAB>src="x.css">…</style>`, the honest checker returns 1 and
the mutant returns 0. Thus the unenumerated Cartesian member supplies a hidden
heading and the probe is silent. This repeats the round-7 class at a larger
group boundary: decomposing two axes separately does not pin their
combinations. It violates harness-track §5.1–5.2 and `testing.md`
anti-placebo / mutation-control requirements.

## Confirmed, but non-blocking by itself

- `pipeSIGPIPE` is deterministic on the declared production-scale fixture:
  all three independent self-test runs return exit 1 with identical
  `PASS=52 FAIL=1`; the sole failure is `спека прод-масштаба принимается`.
  The honest barrier/probe returns `PASS=95 FAIL=0`, battery `42/42`, exit 0.
- The current literals agree with executable composition: the honest run
  measures 95 scenarios and 42 stubs, `BATTERY_EXPECTED=42`, and CI names
  `95 сценариев + батарея из 42 ослаблений`.
- The round-8 pipe mutation targets the one executable here-string line, not a
  comment. Counts of its `grep -qiE` anchor, the `visible_body` call anchor,
  and the non-ancestor guard anchor are each one in the checker. No
  comment-only anchor was found among these new targets.
- `bash -n` and `git diff --check` pass. The checker invoked over the actual
  `cda4600..dee67f1` range returns 0 because it adds no
  `milestones/M-*.md`; that result does not exercise B-14 or B-15.

## Conditions for re-audit

1. Exercise both exact CI event forms. Add setup-guarded positive and
   incomplete-spec scenarios for `EVENT_NAME=push` with `PUSH_BEFORE`, retain
   the PR form, and add a live-derived push-base mutant that makes the full
   probe red. The fixture must prove the event-selected base differs from
   `HEAD`, otherwise it merely retests an empty range.
2. Generate raw-HTML fixtures over the full
   `HTML_TAGS × HTML_OPEN_FORMS` Cartesian product (16 members), or explicitly
   narrow the declared grammar. Add a setup-guarded live-derived mutation that
   removes one formerly untested pair such as `style × tab`; the full probe
   must turn red through that pair's scenario.
3. Recount the scenario and battery literals from execution, update CI in the
   same commit, rerun the honest suite, the complete battery, and the three
   deterministic pipeSIGPIPE runs. Request a new fresh-context adversary audit
   only after the corrected harness artifacts are committed and pushed.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-179
exit=0

$ git diff --name-status cda46001ad958a25082de0e47d10b21b878aaace dee67f12e7fb49ce18e779fd6e21cd202e8a60b5
M	.github/workflows/ci.yml
A	research/critiques/C-178-harness-milestone-shape-r6.md
M	scripts/check_milestone_shape.sh
M	scripts/tests/red_milestone_shape.sh
exit=0

$ bash scripts/tests/red_milestone_shape.sh
  батарея ослаблений: поймано 42 из 42 (ожидалось 42)
PASS=95 FAIL=0 (сценариев: 95)
VERDICT: PASS
уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: 0
exit=0

$ restored pipeSIGPIPE mutant ×3 under MSHAPE_SELFTEST=1
pipe_run_1: FAIL: спека прод-масштаба принимается — ожидался exit=0, получен exit=1
PASS=52 FAIL=1; VERDICT: FAIL; exit=1
pipe_run_2: FAIL: спека прод-масштаба принимается — ожидался exit=0, получен exit=1
PASS=52 FAIL=1; VERDICT: FAIL; exit=1
pipe_run_3: FAIL: спека прод-масштаба принимается — ожидался exit=0, получен exit=1
PASS=52 FAIL=1; VERDICT: FAIL; exit=1

$ push-base mutant (push base -> HEAD), MSHAPE_SELFTEST=1
PASS=53 FAIL=0 (сценариев: 53)
VERDICT: PASS
exit=0
$ real push form, one newly committed incomplete spec
honest: FAIL missing Allowed paths; exit=1
mutant: OK no new milestone specs; exit=0

$ style×tab raw-HTML mutant, MSHAPE_SELFTEST=1
PASS=53 FAIL=0 (сценариев: 53)
VERDICT: PASS
exit=0
$ committed <style<TAB>src="x.css"> hidden-heading fixture
honest: FAIL missing Allowed paths; exit=1
mutant: OK all required sections; exit=0

$ rg CI literal / anchor counts
.github/workflows/ci.yml:250: Проба барьера (95 сценариев + батарея из 42 ослаблений)
grep -qiE "${re}"=1
body="$(visible_body "$f")"=1
git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null=1
exit=0

$ bash -n scripts/check_milestone_shape.sh scripts/tests/red_milestone_shape.sh
exit=0
$ git diff --check cda4600 dee67f1
exit=0
$ bash scripts/check_milestone_shape.sh cda46001ad958a25082de0e47d10b21b878aaace
OK: в диапазоне cda4600..HEAD новых milestone-спек нет — проверять нечего
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-29T15:34Z
- Milestone: C-101 / harness-track `harness-milestone-shape`
- Статус: BLOCKED — REJECT
- HEAD: dee67f1 — fix(harness): M-72 — барьер мерил ОКРУЖЕНИЕ, а не свой инвариант (SIGPIPE) [architect]

## §B — Что я сделал
- Аудировал закоммиченные harness-артефакты и CI, а не текст плана.
- Исполнил честную пробу, три детерминированных pipeSIGPIPE-прогона и два живых минимальных мутанта.

## §C — Артефакты / результаты
- `research/critiques/C-179-harness-milestone-shape-r8.md`
- Done Block: raw output and exit codes above.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  On feat/harness-milestone-shape, resolve REJECT C-179 before another adversary pass. Preserve the three harness artifacts. Extend red_milestone_shape.sh to exercise both exact CI event forms, including a setup-guarded push scenario with PUSH_BEFORE distinct from HEAD and a live-derived push-base mutant. Generate raw-HTML hidden-heading fixtures for all 16 HTML_TAGS × HTML_OPEN_FORMS combinations and add a setup-guarded mutant that removes one formerly untested pair such as style × tab. Keep M-72's three deterministic pipeSIGPIPE checks, recount 95/42 from execution, update the CI literal, run the honest suite and full battery, commit and push the corrected set, then provide its full SHA for a fresh audit.
  ```
- Push-статус: this verdict is committed and pushed to `origin/feat/harness-milestone-shape` in this audit action.
- Кэш: не создавался; temporary audit fixtures are removed separately from the worktree.

## §E — Риски / открытые вопросы
- Merge remains blocked by `docs/workflow/harness-track.md` §5 until B-14 and B-15 are fixed and a new adversary artifact is committed.

=== END HANDOFF ===
