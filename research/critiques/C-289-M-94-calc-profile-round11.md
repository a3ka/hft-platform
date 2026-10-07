<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: 9537edbc7d6b5b0e0d9b54bba54795f4a4da5863
verdict: NOTE
-->

# C-289 — M-94 calc-profile, круг 11 — NOTE

## Scope and verdict

**NOTE — narrowed A-052 §4(2) gate passes.** The committed architect range
`8bb5ee1..9537edb` supplies the required oracle, weakener mode, and explicit
textual-limit step.  It is therefore ready for the specified `engine-dev` task
12.  This audit deliberately does **not** reopen C-287 B-1/B-2 or A-051.

The range changes no T1/T2 contract or trait signature; its complete new
artifact set is the two RED-test halves, the shared corpus, two test scripts,
the verify step, and the milestone text.  `VB-I-2` remains the live invariant:
two independently judged rules for one calculation would make live/replay
definitions diverge.  The agreement oracle now pins the narrower start-grammar
to library-invariant boundary that A-052 assigned it.

## Audited evidence

### (i) Two-binary agreement oracle

- `scripts/tests/fixtures/m94_two_binary_corpus.txt` has all four shared
  quantities, accepted and rejected values, timeframe×cadence pairs, and the
  unmasked `GATEWAY_TIMEFRAME_MS=604800000` detector.
- `g5` calls production `gateway_serve::serve_config_from_env`; `w5` launches
  the production `gateway-checkpoint` binary on an empty journal.  The script
  refuses a missing/empty half-output, requires the active profile to be
  accepted in legacy and profile modes by both halves, requires two outcomes
  per corpus case, and applies accepted/rejected pressure to every shared
  quantity.
- Baseline agreement passes.  Mutation M1—changing
  `parse_timeframe_ms`'s `86_400_000 % v != 0` condition to `false`—fails on
  `604800000` in both modes; the working tree was restored afterwards.

### (ii) Weakener mode

`red_m94_shared_grammar_probe.sh` first proves the existing sentinel-REFUSE
path (g4/w4), then injects `M94-LOOSE-NAN` into `parse_bands` and requires the
two binaries to agree.  At this pre-engine-dev head it fails exactly on the
NaN copy left in `gateway-checkpoint`; the `M94-LOOSE-TF` negative control
passes by observing the intended `validate_selector` mismatch.  This is the
requested RED on R-246 B-1, not a test that happens to be green against the
unfixed copies.

### (iii) Text and named limit

Milestone §3.2 names `gateway::validate_selector` as a library precondition,
not environment grammar; §5 preserves it while prohibiting binary copies.
Task 12 names both the two new behavioural checks and the removal scope.
`verify_M-94.sh` task12 rejects literal `86_400_000 %`, `< 1000`, and `is_nan`
outside line comments in `gateway-checkpoint.rs`, and explicitly says that a
renamed constant defeats this text check and that the PR diff is its backstop.
That is the limitation ordered by A-052 §3(в).2, not a claim of a complete
behavioural proof.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile
9537edbc7d6b5b0e0d9b54bba54795f4a4da5863

$ git merge-base origin/main origin/feat/M-94-calc-profile
045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0

$ git log --oneline 8bb5ee1..9537edb
9537edb docs(M-94): A-052 — validate_selector is not grammar; §5 copies; §8 oracles; rounds 9-10 [architect]
611b8fe test(M-94): A-052 §3(в) — weakener mode (RED on R-246 B-1) and text ban on copies [architect]
0f30ec6 test(M-94): A-052 §3(б) — two-binary agreement oracle (serve start <-> warmer) [architect]

$ bash scripts/tests/red_m94_two_binary_agreement.sh
сравнений: 70, расхождений: 0
PASS  согласие двух бинарей: выдача и прогреватель одинаково принимают/отвергают каждый случай корпуса в обоих режимах
VERDICT: PASS
exit=0

$ # M1: parse_timeframe_ms: `if 86_400_000 % v != 0` → `if false`
$ bash scripts/tests/red_m94_two_binary_agreement.sh
сравнений: 70, расхождений: 2
FAIL  согласие двух бинарей (A-052 §3 (б)): правило величины судит по-разному выдача и прогреватель
  legacy   GATEWAY_TIMEFRAME_MS=604800000: выдача=ACCEPT прогреватель=REFUSE
  profile  GATEWAY_TIMEFRAME_MS=604800000: выдача=ACCEPT прогреватель=REFUSE
VERDICT: FAIL
exit=1
$ # mutation reverted; git status --porcelain
<empty>

$ bash scripts/tests/red_m94_shared_grammar_probe.sh
PASS  setup: сторож вживлён в 6 из 6 общих функций
PASS  g4: выдача — сторожевой отказ общей функции доходит до результата без профиля и в профиле
PASS  w4: прогреватель — сторожевой отказ доходит в окружении, флагах и профиле
PASS  setup: ослабитель вживлён (parse_bands → NaN, parse_timeframe_ms → 604800000)
FAIL  ослабитель NaN (A-052 §3 (в).1, R-246 B-1): приём общей parse_bands не дошёл до одного из бинарей — у него своя копия правила
  legacy   GATEWAY_BANDS=M94-LOOSE-NAN: выдача=ACCEPT прогреватель=REFUSE
  profile  GATEWAY_BANDS=M94-LOOSE-NAN: выдача=ACCEPT прогреватель=REFUSE
PASS  негативный контроль: ослабленный таймфрейм расходится (держит инвариант validate_selector) — ось согласия чувствительна
сценариев: 6, провалов: 1
VERDICT: FAIL
exit=1  # expected RED before engine-dev removes copies

$ bash -n scripts/tests/red_m94_two_binary_agreement.sh scripts/tests/red_m94_shared_grammar_probe.sh scripts/verify_M-94.sh
exit=0

$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=9537edbc7d6b5b0e0d9b54bba54795f4a4da5863 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=9537edbc7d6b5b0e0d9b54bba54795f4a4da5863 bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (045fef9..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=9537edbc7d6b5b0e0d9b54bba54795f4a4da5863 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=9537edbc7d6b5b0e0d9b54bba54795f4a4da5863 bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 10, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 0
exit=0

$ EVENT_NAME=pull_request BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 HEAD_SHA=9537edbc7d6b5b0e0d9b54bba54795f4a4da5863 PR_BASE_SHA=045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0 PR_HEAD_SHA=9537edbc7d6b5b0e0d9b54bba54795f4a4da5863 bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 045fef9..HEAD не ввёл второй носитель под занятым идентификатором
exit=0
```

## Next action

Architect may dispatch `engine-dev` only for M-94 task 12 as bounded by A-052:
remove the listed built-in `gateway-checkpoint` copies and leave
`validate_selector` untouched.  The subsequent tester run must make the
weakener green while retaining the M1 agreement failure.
