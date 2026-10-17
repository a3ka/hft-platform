<!-- GATE-META
milestone: M-60
audited_repo: a3ka/hft-platform
audited_base: 7a163f7
audited_head: a6be080
verdict: REJECT
-->

# C-064 — M-60 mechanisms plan-time gate

## Verdict

REJECT.

M-60 правильно выбрал класс проблемы: процессные нормы должны быть подключены к blocking path, а не просто лежать в правилах. Но текущий пакет не проходит plan-time gate: источник замеров не входит в audited head, acceptance-gate можно удовлетворить без реальной CI-проводки, GATE-META/subject-lock оставляет обходы, бюджетный механизм тестируется не тем способом, а чистка может удалить действующие нормы незаметно.

Проверка зубов RED-проб совпала с заявленными числами: docs-freeze 8/2, context-budget 5/2, gate-meta 11/5 FAIL-сценариев для always-0/always-1 заглушек. Проблема не в базовой полярности проб, а в их покрытии и подключенности.

## Pre-flight

- Repo check: `origin` = `https://github.com/a3ka/hft-platform.git`, normalized repo = `a3ka/hft-platform`.
- Worktree: `/tmp/hft-critic-m60g`.
- Base: `origin/main` = `7a163f7187d36b3c6823561c3e45e71fd650064b`.
- Head: `a6be080d5833792182caeec7e89141d23886de2f`, subject `test(M-60): acceptance-гейт — бюджет и сохранность норм как две половины одной проверки [architect]`.
- Commit artifact set present in audited diff: milestone, three RED probes, `scripts/verify_M-60.sh`, prior consultation `C-062`.
- Startup anomaly: `docs/plans/workflow-audit-2026-08-einhard-vs-hft.md` is cited by the milestone but is not present in audited head. I could only read it from `origin/docs/workflow-audit`.

## Findings

### F-064-1 — BLOCKER — источник замеров не является audited artifact

`milestones/M-60-mechanisms.md:6` ставит M-60 на замеры из `docs/plans/workflow-audit-2026-08-einhard-vs-hft.md`, и invocation прямо называет этот файл "замеры, на которых стоит спека". В audited head `a6be080` такого файла нет. Он найден только на другой ветке: `origin/docs/workflow-audit`.

Это ломает воспроизводимость plan-time gate: критик и dev проверяют спеку, численные цели и мотивацию по артефакту, который не входит в проверяемый commit-chain. Для M-60 это не второстепенная ссылка, а основание бюджета и выбора механизмов.

Требование к architect: включить источник замеров в audited head либо явно заменить ссылку на committed, immutable источник внутри проверяемой цепочки. Пока этого нет, пакет не самодостаточен.

### F-064-2 — BLOCKER — CI-подключенность можно подделать комментариями и неполным `needs`

`scripts/verify_M-60.sh:112-130` проверяет подключение новых gate jobs к CI обычным `grep`. Это не доказывает, что jobs существуют, что они исполняют scripts, и что `status-check` зависит от всех новых jobs.

Конкретный обход:

- добавить в `.github/workflows/ci.yml` комментарии с именами `check_docs_freeze.sh`, `check_context_budgets.sh`, `check_gate_meta.sh`, `verify_design_claims.sh`;
- добавить комментарии с `red_docs_freeze.sh`, `red_context_budgets.sh`, `red_gate_meta.sh`;
- добавить в любой блок после первого `needs:` только один job из `docs-freeze|context-budgets|gate-meta`.

Текущий grep в `verify_M-60.sh:123-127` использует `grep -A3 'needs:' ... | grep -Eq 'docs-freeze|context-budgets|gate-meta'`, то есть принимает любой один из трех, а не все обязательные проверки. Это именно класс ложной подключенности, который M-60 должен закрыть.

Требование к architect: acceptance должен проверять структуру workflow, а не наличие строк. Минимум: job id, `run:` команды, и полное включение всех M-60 jobs в финальный blocking `status-check.needs`.

### F-064-3 — BLOCKER — GATE-META/subject-lock не зажимает реализацию с двух сторон

`milestones/M-60-mechanisms.md:113-137` ограничивает subject-lock "gate-class" путями: `scripts/verify_*.sh`, `scripts/check_*.sh`, `scripts/tests/red_*.sh`, `.github/workflows/**`, зона G0, текущий milestone. `scripts/tests/red_gate_meta.sh:164-204` проверяет этот же узкий класс.

Конкретный обход:

- critic/reviewer выпускает `NOTE` или `APPROVE` с валидным `audited_head`;
- после вердикта dev меняет `crates/**/src/**`, `scripts/**` вне перечисленного класса или другой runtime path;
- GATE-META subject-lock проходит, потому что source/runtime изменения не входят в проверяемый subject.

Если M-60 претендует на "вердикт относится к тому, что потом исполняется", current design этого не делает. Он защищает только gate-файлы, но не всю diff-поверхность между reviewed head и tested/reviewed head.

Отдельный пробел: `scripts/tests/red_gate_meta.sh:64-77` всегда пишет `milestone: M-99`, но нет отрицательного сценария "wrong milestone id for current verdict". Реализация может игнорировать `milestone:` и все RED-пробы останутся зелеными.

Требование к architect: либо сузить claim до "gate-class subject-lock only", либо добавить проверку full subject для verdict-bearing artifacts. Добавить RED-сценарий wrong-milestone.

### F-064-4 — MAJOR — бюджетный механизм тестируется в обход будущего default-контракта

`scripts/tests/red_context_budgets.sh:15-20` декларирует контракт для `check_context_budgets.sh`, но все сценарии запускают barrier только с временными `ROOT` и `BUDGET_TABLE` (`scripts/tests/red_context_budgets.sh:57-58`). `scripts/verify_M-60.sh:35-47` вообще считает реальные `.claude/rules/*.md` и `CLAUDE.md` напрямую, не требуя, чтобы `check_context_budgets.sh` имел корректный default table для настоящего repo startup corpus.

Конкретный обход:

- реализовать `check_context_budgets.sh`, который корректен только когда задан `BUDGET_TABLE`;
- оставить default table пустым, отсутствующим или с мягкими лимитами;
- RED-проба пройдет, B-step `verify_M-60.sh` пройдет по собственному `wc -l`, а реальный механизм G1 не будет защищать startup.

Это нарушение принципа "оценивай подключенность, а не наличие": M-60 проверяет число строк отдельно от механизма, который должен стать gate.

Требование к architect: acceptance должен запускать реальный `check_context_budgets.sh` в default mode на repo root и проверять, что там присутствуют hard budgets для `.claude/rules/*.md` и `CLAUDE.md`.

### F-064-5 — MAJOR — G0 lock zone не покрывает существующий carrier и часть enforcement path

`milestones/M-60-mechanisms.md:16-18` задает lock zone как `.claude/rules/**`, `.claude/agents/**`, `CLAUDE.md`, `docs/04-workflow.md`. Для prose-норм это разумное ядро, но не полный процессный контур.

Пропущено:

- `.claude/wrappers/**`, особенно `.claude/wrappers/pi-dev.sh` и `.claude/wrappers/dispatch-mandate.md`;
- `.github/workflows/**`, если lock должен защищать enforcement behavior, а не только prose;
- `scripts/check_*.sh`, `scripts/tests/red_*.sh`, `scripts/verify_*.sh`, если bypass через изменение самого gate path должен требовать такой же approval trace.

`milestones/M-60-mechanisms.md:70` говорит, что полный launcher `pi-dev.sh` не взят, потому что "у нас нет носителя". Это фактически неверно для audited repo: `.claude/wrappers/pi-dev.sh` и `.claude/wrappers/dispatch-mandate.md` уже существуют. Более того, `dispatch-mandate.md:37` содержит push-норму, конфликтующую с текущей intra-chain push discipline из `.claude/rules/gates.md:123-147`. То есть carrier есть и уже влияет на поведение агентов.

Требование к architect: пересмотреть G0 scope с учетом live carriers. Если wrappers намеренно вне M-60, это должно быть явно названо как deferred risk, а не "носителя нет".

### F-064-6 — MAJOR — `FOUNDER-APPROVED` в теле коммита является следом, но не разрешением

`milestones/M-60-mechanisms.md:78-92` и `scripts/tests/red_docs_freeze.sh:91-153` строят G0 вокруг токена `FOUNDER-APPROVED` в body коммита. RED-проба уже проверяет несколько очевидных обходов: token in subject, path-token и unrelated token. Но сам token остается forgeable любым committer'ом.

Конкретный обход, который пробы не закрывают:

- сделать side commit с изменением locked docs без token;
- сделать merge commit с body token;
- реализация, которая проверяет только merge commit body, пропустит историю с неразрешенным side commit.

Еще один класс обхода: deletion/rename locked файла, если реализация смотрит только added/modified paths или только patch text. В `scripts/tests/red_docs_freeze.sh` нет сценария удаления/переименования locked path.

Требование к architect: зафиксировать, что token является audit breadcrumb, а не proof of founder action. Добавить RED-сценарии side-commit-under-tokened-merge и deletion/rename locked path либо явно признать residual risk.

### F-064-7 — BLOCKER — §D preservation guard не сохраняет нормы из зон, которые M-60 собирается резать

`milestones/M-60-mechanisms.md:175-188` планирует удалить около 279 строк из действующих startup/workflow правил. `scripts/verify_M-60.sh:53-66` защищает только 12 substring-формулировок. Этого недостаточно для перечисленных сокращений.

Нормы, которые могут исчезнуть незамеченными:

- Arbitration from `.claude/rules/gates.md:70-81`: два REJECT по одной причине вызывают arbiter; три цикла вызывают arbiter; arbiter writes/commits `research/arbitration/A-NNN`; decision binding.
- RAW escalation from `.claude/rules/gates.md:30-45`: journal layout / event stream / T1 contract criticality goes to strong model, not the ordinary critic route.
- Reviewer workflow from `.claude/rules/gates.md:110-121`: reviewer is unconditional; reviewer updates `PROJECT-STATE.md` and `TECH-DEBT.md`; reviewer does not design fixes.
- Deployment discipline from `.claude/rules/gates.md:123-163`: intra-chain push before handoff, wait for CI+Deploy, SSH health/heartbeat, rollback/red prod, Worktree-GC, RED-not-on-main, merge-preview doc.
- Testing guardrails from `.claude/rules/testing.md`: public fn coverage, signal determinism, degraded input checklist, prod-form measurement, oracle measures own invariant, mutation-control, forbidden-list, fix-after-verdict requires an oracle, ops-path canary.
- Branch hygiene from `.claude/rules/branch-hygiene.md`: role worktree, committed gate artifacts, unknown untracked handling, duplicate-agent proof, index-before/diff-after, no push into another active agent branch.
- Commit discipline from `.claude/rules/commit-discipline.md`: atomic commits, no co-author trailer, Done Block raw stdout/raw != all, role marker in subject.
- Handoff format from `.claude/rules/handoff-block.md`: sections A-E, paste-ready next invocation, single Handoff Block, push-status predicate in §D.

Требование к architect: либо расширить D-step до preservation table by section, либо отказаться от line-count-driven cleanup until a semantic guard exists. С текущими 12 строками можно выполнить бюджет ценой удаления действующих норм.

### F-064-8 — MAJOR — budget B для `CLAUDE.md ≤70` не сходится по собственной таблице

`milestones/M-60-mechanisms.md:22-30` фиксирует старт: `.claude/rules/*.md` = 949 строк, `CLAUDE.md` = 100 строк, цели 725 и 70. Для rules плановые удаления примерно достаточны: `949 - (11+30+15+18+20+90+40+15+15) + additions` дает около 700-720 строк.

Но для `CLAUDE.md` таблица чистки на `milestones/M-60-mechanisms.md:179` называет только `-25`, то есть `100 - 25 = 75`. Это выше hard target `70` из `scripts/verify_M-60.sh:23-24`.

Требование к architect: либо указать дополнительные безопасные сокращения `CLAUDE.md` минимум на 5 строк плюс запас под новые нормы, либо изменить целевой лимит. Нельзя добирать эти строки удалением норм без явного preservation guard.

## What The RED Probes Do Not Catch

1. CI comments and partial `needs` can satisfy W-step without blocking jobs.
2. `check_context_budgets.sh` can be correct only under explicit `BUDGET_TABLE` and broken in default repo mode.
3. GATE-META can ignore `milestone:` because no wrong-milestone RED exists.
4. Subject-lock can ignore source/runtime files and still pass all current GM tests.
5. Docs-freeze can trust a tokened merge commit while hiding an unapproved side commit.
6. Docs-freeze can miss deletion/rename if implemented as patch-string matching only.
7. Cleanup can remove arbitration, RAW escalation, reviewer, testing, branch hygiene, commit, and handoff norms while D-step remains green.

## G0 Assessment

The core prose zone is directionally correct: `.claude/rules/**`, `.claude/agents/**`, `CLAUDE.md`, `docs/04-workflow.md`.

It is not sufficient for "mechanisms": live carriers and enforcement surfaces are outside the lock. At minimum the spec must account for `.claude/wrappers/**`, `.github/workflows/**`, and gate scripts. Whether all of them are founder-locked or covered by a different approval path is an architectural decision, but the current milestone cannot claim the launcher carrier is absent.

`FOUNDER-APPROVED` is acceptable as a human-readable audit marker only if the threat model is accidental drift. It is not a sufficient permission proof against a committer who can write arbitrary commit bodies. M-60 should say this explicitly.

## Budget And Cleanup Assessment

The rules budget appears numerically reachable only if the planned cuts are made aggressively and additions stay near the declared ~25 lines. The `CLAUDE.md` budget is not reachable from the stated cut table.

More importantly, the current D-step protects strings, not norms. The correct failure mode is not "budget hard to meet"; it is "budget can be met by deleting policy that is still active." For plan-time acceptance, the target should change or the preservation guard should become semantic enough to cover the sections being cut.

## Self-reference

M-60 does not yet pass its own mechanisms:

- Its own workflow-audit source is outside audited head.
- Its own self-freeze only checks the M-60 diff with the future docs-freeze script; it does not address existing wrappers as process carriers.
- Its own budget target for `CLAUDE.md` is not supported by the cleanup table.
- Its own CI wiring acceptance can be passed without real blocking jobs.

## Raw Stub Runs

### docs-freeze vs always-0

```text
$ BARRIER=/tmp/m60-barrier-always0.sh bash scripts/tests/red_docs_freeze.sh; echo exit=$?
[red-docs-freeze] barrier=/tmp/m60-barrier-always0.sh
[red-docs-freeze] tmp=/tmp/tmp.pCPWhn0oCF
FAIL: FR-1 locked rule change without token must fail
PASS: FR-2 locked rule change with FOUNDER-APPROVED body passes
PASS: FR-3 unlocked docs change passes without token
FAIL: FR-4 locked agent change without token must fail
FAIL: FR-5 token only in subject must fail
FAIL: FR-6 token in file path must not count
FAIL: FR-7 commit body token without locked file must still reject locked change commit lacking token
FAIL: FR-8 PR-base scan must detect earlier locked change
FAIL: FR-8b push-before scan must detect earlier locked change
FAIL: FR-9 merge commit touching locked docs without token must fail
VERDICT: FAIL (8)
exit=1
```

### docs-freeze vs always-1

```text
$ BARRIER=/tmp/m60-barrier-always1.sh bash scripts/tests/red_docs_freeze.sh; echo exit=$?
[red-docs-freeze] barrier=/tmp/m60-barrier-always1.sh
[red-docs-freeze] tmp=/tmp/tmp.Pyo43aaLuJ
PASS: FR-1 locked rule change without token fails
FAIL: FR-2 locked rule change with FOUNDER-APPROVED body should pass
FAIL: FR-3 unlocked docs change should pass without token
PASS: FR-4 locked agent change without token fails
PASS: FR-5 token only in subject does not authorize
PASS: FR-6 token in file path does not authorize
PASS: FR-7 unrelated token does not authorize later locked change
PASS: FR-8 PR-base scan detects earlier locked change
PASS: FR-8b push-before scan detects earlier locked change
PASS: FR-9 merge commit touching locked docs without token fails
VERDICT: FAIL (2)
exit=1
```

### context-budgets vs always-0

```text
$ BARRIER=/tmp/m60-barrier-always0.sh bash scripts/tests/red_context_budgets.sh; echo exit=$?
[red-context-budgets] barrier=/tmp/m60-barrier-always0.sh
[red-context-budgets] tmp=/tmp/tmp.4mx2QJhTJW
FAIL: CB-1 over-budget file must fail
PASS: CB-2 at-budget file passes
PASS: CB-3 under-budget file passes
FAIL: CB-4 missing required file must fail
FAIL: CB-5 empty budget table must fail
FAIL: CB-6 multiple files fails if one exceeds
FAIL: CB-6b missing second file still fails
VERDICT: FAIL (5)
exit=1
```

### context-budgets vs always-1

```text
$ BARRIER=/tmp/m60-barrier-always1.sh bash scripts/tests/red_context_budgets.sh; echo exit=$?
[red-context-budgets] barrier=/tmp/m60-barrier-always1.sh
[red-context-budgets] tmp=/tmp/tmp.WWMOaubCKL
PASS: CB-1 over-budget file fails
FAIL: CB-2 at-budget file should pass
FAIL: CB-3 under-budget file should pass
PASS: CB-4 missing required file fails
PASS: CB-5 empty budget table fails
PASS: CB-6 multiple files fails if one exceeds
PASS: CB-6b missing second file fails
VERDICT: FAIL (2)
exit=1
```

### gate-meta vs always-0

```text
$ BARRIER=/tmp/m60-barrier-always0.sh bash scripts/tests/red_gate_meta.sh; echo exit=$?
[red-gate-meta] barrier=/tmp/m60-barrier-always0.sh
[red-gate-meta] tmp=/tmp/tmp.YArJLXO0kI
FAIL: GM-1 missing GATE-META must fail
FAIL: GM-2 missing audited_head must fail
FAIL: GM-3 wrong audited_repo must fail
FAIL: GM-4 wrong audited_base must fail
FAIL: GM-5 wrong audited_head must fail
FAIL: GM-6 unapproved verdict value must fail
PASS: GM-7 valid critic NOTE passes
PASS: GM-8 valid reviewer APPROVE passes
FAIL: GM-9 no verdict artifact must fail when gate-class files changed
FAIL: GM-9b gate-class change after latest verdict must fail
FAIL: GM-10 stale audited_head against gate-class change must fail
PASS: GM-11 fresh audited_head including gate-class change passes
FAIL: GM-12 multiple verdict files with latest stale must fail
PASS: GM-13 latest verdict matching head passes
PASS: GM-14 non-gate docs change does not require fresh verdict
FAIL: GM-15 wrong repo remote must fail
VERDICT: FAIL (11)
exit=1
```

### gate-meta vs always-1

```text
$ BARRIER=/tmp/m60-barrier-always1.sh bash scripts/tests/red_gate_meta.sh; echo exit=$?
[red-gate-meta] barrier=/tmp/m60-barrier-always1.sh
[red-gate-meta] tmp=/tmp/tmp.CW5siiP43i
PASS: GM-1 missing GATE-META fails
PASS: GM-2 missing audited_head fails
PASS: GM-3 wrong audited_repo fails
PASS: GM-4 wrong audited_base fails
PASS: GM-5 wrong audited_head fails
PASS: GM-6 unapproved verdict value fails
FAIL: GM-7 valid critic NOTE should pass
FAIL: GM-8 valid reviewer APPROVE should pass
PASS: GM-9 missing verdict for gate-class change fails
PASS: GM-9b gate-class change after latest verdict fails
PASS: GM-10 stale audited_head against gate-class change fails
FAIL: GM-11 fresh audited_head including gate-class change should pass
PASS: GM-12 latest stale verdict fails
FAIL: GM-13 latest verdict matching head should pass
FAIL: GM-14 non-gate docs change should not require fresh verdict
PASS: GM-15 wrong repo remote fails
VERDICT: FAIL (5)
exit=1
```

## Acceptance Diagnostic

I also ran `bash scripts/verify_M-60.sh; echo exit=$?` on the audited package. It already reported expected pre-dev failures for missing implementation scripts, over-budget docs, missing CI wiring, and missing self-freeze gate. The run was interrupted during `cargo test --all` after those failures were visible; no green acceptance claim is made from this diagnostic.

## Handoff

Next: architect.

Required before dev handoff:

1. Commit the workflow audit source, or replace it with an audited in-chain measurement source.
2. Strengthen W-step so CI wiring cannot be satisfied by comments or partial `needs`.
3. Decide whether GATE-META subject-lock is gate-class-only or full-subject; update claim and probes accordingly.
4. Add wrong-milestone and source-change-after-verdict RED scenarios.
5. Make context-budget acceptance exercise the real default `check_context_budgets.sh`.
6. Rework G0 scope to address `.claude/wrappers/**` and other carriers.
7. Expand §D preservation guard before authorizing the -279 line cleanup.
8. Fix the `CLAUDE.md` budget arithmetic or change the target.

