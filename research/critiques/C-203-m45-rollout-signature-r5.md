<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: 2e63a37e5bf454da69b0fbd69de28c043b4caf4c
audited_head: 2ffb20a46cadb76c68e964154f7616ac7cf213c9
verdict: NOTE
-->

# C-203 — M-45 rollout signature, круг 5: NOTE

## Verdict

**NOTE — набор можно передать founder'у для dispatch `engine-dev` на задачу 7.**

Внешний `verify_M-45.sh` намеренно остаётся красным только на T10: строки раскатки ещё
отсутствуют, потому что задача 7 не начата. Это не дефект plan-time набора. До dispatch
никакого второго FAIL нет; T10c зелёный, а обе предписанные мутации делают его красным.

Скоуп круга соблюдён по `A-030` §4: исполнение §2 и §3 пп. 1–4, плюс сверка новых и
изменённых блоков. Вопросы, которые арбитр объявил не пересуживаемыми, не переоткрывались.

## Artifact-set audit

- Milestone: `milestones/M-45-persist-l2delta.md` содержит задачу 7, её allowed path
  (`docker-compose.yml`, только две названные переменные), владельца `engine-dev` и
  запретные пути.
- T1 не меняется: в диапазоне `2e63a37..2ffb20a` нет `contracts/` или
  `crates/contracts/`; contract-RFC не требуется.
- T2/API и реальные entry point'ы присутствуют в обоих venue-крейтах:
  `parse_capture_symbols`, `should_capture_l2delta`, `l2delta_emission_for`,
  `new_with_l2delta`, `on_ws_text`. RED-наборы spot/futures и `DET-I-1` присутствуют и
  исполняются из verify.
- `scripts/verify_M-45.sh` использует FAIL-счётчик и `exit 1`; T10 и T10c используют
  один `scripts/lib/rollout_symbols_check.py --compose` путь. `VN-I-3` применим:
  общий подписанный состав действует на обе площадки, без per-venue исключения.

## Checks

### A-030 §3 mutation probes (separate worktree)

`/tmp/hft-critic-m45-r5-probe-I68oGX` был detached на `2ffb20a`; после каждой пробы
изменение откатилось (`git diff --quiet -- scripts/verify_M-45.sh
scripts/lib/rollout_symbols_check.py`, exit=0).

```text
$ if got == SIGNED: -> if got:
FAIL  T10c ... compose «ЛИШНИЙ символ литералом»: ожидался код 1, получен 0;
values 'BTCUSDT'/'own-x': ожидался 1, получен 0;
values 'BTCUSDTX,ETHUSDT'/'own-x': ожидался 1, получен 0
VERDICT: FAIL (2 нарушений)
verify_exit=1

$ bad = check_symbols(sym) + check_epoch(epoch) -> bad = []  # check_compose
FAIL  T10c ... compose «ЛИШНИЙ символ литералом»: ожидался код 1, получен 0;
compose «подстановка :- (обходима)»: ожидался код 1, получен 0;
compose «подстановка без двоеточия»: ожидался код 1, получен 0;
compose «эпоха подстановкой»: ожидался код 1, получен 0
VERDICT: FAIL (2 нарушений)
verify_exit=1
```

### Changed fact blocks

Nine new/changed command-output blocks were compared with the command that produces them:
historic hardcode, substitution world, literal world, O-5 history, tick source, absent
cadence control, spot sites, futures sites, and `parse_depth_snapshot` consumers.

```text
B1_exit=0 B2_exit=0 B3_exit=0 B4_exit=0 B5_exit=0
B6_grep_exit=1  # expected: cadence control is absent
B7_exit=0 B8_exit=0 B9_exit=0
```

## Done Block

```text
$ git rev-parse HEAD && git merge-base HEAD origin/main
2ffb20a46cadb76c68e964154f7616ac7cf213c9
2e63a37e5bf454da69b0fbd69de28c043b4caf4c
exit=0

$ bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'; echo verify_exit=${PIPESTATUS[0]}
PASS  T0–T9 (all applicable checks)
FAIL  T10 задача 7 НЕ исполнена — ОТСУТСТВУЮТ на сервисе recorder: L2DELTA_CAPTURE_SYMBOLS, EPOCH_ID
PASS  T10c мутация состава: 7 миров compose + 6 сценариев значений через ТОТ ЖЕ CLI, что и T10
VERDICT: FAIL (1 нарушений)
verify_exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo design_claims_exit=$?
VERDICT: PASS (0 нарушений)
design_claims_exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_gate_meta.sh
VERDICT: PASS — вердиктов проверено: 9, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_protected_artifacts.sh
OK: защищённые артефакты целы на HEAD (2e63a37..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_docs_freeze.sh
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_artifact_ids.sh
OK: ни один коммит диапазона 2e63a37..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_review_fa.sh
SKIP (диапазон не трогает crates/**)
exit=0

$ bash scripts/next_artifact_id.sh C
C-203
exit=0
```

## Required next action

Founder may dispatch `engine-dev` only for M-45 task 7: add the two signed literals to the
recorder service in one commit, run T10/T10b/T10c green, then complete the §8 deploy gate
including per-venue fresh-event sanity and the E-002 sequence boundary.
