<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: 18a68daa3369be224214ec191bc0f1d2625e4ff7
audited_head: 2b3d9a1ee1be22dbdad1557a0ef13b960e5f8d72
verdict: NOTE
-->

# C-321 — M-99 / A-057: класс K закрыт нормой, RED-набором и исполнимым разделением миров — NOTE

## Вердикт

**NOTE.** Единственный plan-time круг A-057 пройден. Это не «GREEN реализации»:
`deploy/bin/journal-restore-drill-cron.sh` намеренно остаётся pre-engine-dev и краснеет
на четырёх ровно названных мирах класса K. Но committed artifact set достаточно определён
и различающ: он требует от будущей реализации N1--N3 и отделяет все четыре клетки K от
честного успеха.

На судимой вершине проба получает **ровно MF/SC/SD/SR-L (4 из 48)**. Собранный мною
временный кандидат по M-99 §4 п.12(е) получает 48/48: один builtin-разбор `LISTING`,
независимый счёт sidecar-ов, проверка статуса наблюдения `ls -A` и физического наличия
каждого доставленного sidecar-а. Его точечные мутанты дали предписанные множества красных:

| кандидат / изменение | наблюдаемый итог |
|---|---|
| только `want=0` | MF проходит; FAIL только SC/SD/SR-L |
| полный N1--N3 | PASS 48/48 |
| sidecar enumeration снова через here-string | FAIL MF/SC (внутри MF срабатывает MF-2) |
| нет проверки наличия sidecar после доставки | FAIL только SD |
| нет проверки статуса `ls -A` | FAIL только SR-L |
| membership снова here-string и нет `want=0` | FAIL только MF |
| полный кандидат без `want=0` | PASS 48/48 |
| полный кандидат без независимого счёта sidecar-ов | PASS 48/48 |
| эталон при отсутствующей product wrapper | FAIL только четыре объявленных RED: product missing, CR, CL, IM (4/49) |

Последние две PASS -- именно заявленная M-99 §11 «мера по построению»: они не
выдаются за независимую поведенческую красную проверку. Новую клетку K (`ok=1` либо
кредит `checked` без байт этого прогона) не нашёл; оснований для `ESCALATE: СТОП,
founder'у` нет.

## Проверенные артефакты и контрактная граница

- Диапазон `ac6aaaa5..2b3d9a1e` содержит только три architect-owned файла:
  milestone, RED probe и verify gate. `git diff --check` чист. Нет изменения
  `contracts/**`, `crates/**` или production wrapper в plan-time диапазоне.
- T2-файл состояния, `digest`, сигнатура `sample_restore_drill`, окно 40 суток,
  сигнатура и машинные коды `journal-drill-read` (`0/1/4/5/6`) наследованы без
  изменения из M-74. Нормативный M-99 §4 п.12(е) добавляет для этого шва N1--N3,
  не выбирая будущую форму реализации.
- `scripts/tests/red_restore_drill.sh` содержит setup-guards для MF/SC/SD/SR-L,
  общий `dl_judge` и `DL-M`: успех допустим только когда полнота видна читателю;
  мутант «`ok=1` при неполной доставке» отдельно отвергнут. Это устраняет прежнее
  ложное требование «отказ всегда» и не ослабляет fail-closed смысл.
- `scripts/verify_M-99.sh` task 2d удерживает все семьи и добавляет канарейку против
  here-string/here-doc над `LISTING`. На текущей pre-dev обёртке именно эта канарейка
  красная -- ожидаемо; остальные task-2d статические канарейки зелёные.
- Унаследованный live-инвариант `OPS-I-3` сохранён: cold copy должна быть восстановлена
  **и прочитана**, не только заявлена успешной. FA-WAIVER для диапазона:
  `crates/**` не менялись; управляющий ops-инвариант назван и его delivery-предикаты
  исполняются probe.

## Границы и результат проверки A-057

Проверялись только разрешённые A-057 §5.2 предметы. Литерал `transport-failed`, модель
`RLIMIT_FSIZE` вместо настоящего ENOSPC, K'/K'' и поимённый минимум sidecar-ов не были
основанием вердикта. Проверка не нашла новой разновидности K: каждый благоприятный исход
полного кандидата требует байтов данного прогона и полноты, увиденной reader-ом.

Полный `scripts/verify_M-99.sh` на plan-time наборе должен быть RED: task #1 вызывает
именно текущую product wrapper и получает четыре требуемых K-провала. Поэтому его не
предъявляю как зелёный completion gate; проверены task #2d, исполнимая RED-проба и карта
CI. `VERIFY_M99_CI_DRY=1` покрывает 65/65 `run:`-шагов с 59 локально исполнимыми и шестью
явными исключениями.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-99-td260-sampling
2b3d9a1ee1be22dbdad1557a0ef13b960e5f8d72
exit=0

$ git merge-base origin/main origin/feat/M-99-td260-sampling
18a68daa3369be224214ec191bc0f1d2625e4ff7
exit=0

$ git diff --name-status ac6aaaa5..HEAD
M	milestones/M-99-restore-drill-automation.md
M	scripts/tests/red_restore_drill.sh
M	scripts/verify_M-99.sh
exit=0

$ CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh
FAIL  SR-L отказ наблюдателя пустоты прочитан как «пусто» — ... ok=1, rc=0, checked=3
FAIL  MF ложное «нет» членства — ... MF-1 ...; MF-2 ... ok=1, rc=0, checked=3
FAIL  SC перечисление sidecar'ов отказало ... sidecar'ов в RESTORE: 0 из 2
FAIL  SD транспорт сообщил успех без байт sidecar'а ... ok=1, rc=0, checked=3
PASS  DL-M судья ... отверг 13 мутантов (... успех при неполной доставке ...)
PASS  PS-M/DV-M судьи ... отвергли 8 + 8 мутантов
VERDICT: FAIL (4 из 48)
exit=1

$ (cd /tmp/hft-critic-m99-a057-candidate2-uLifZT && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh)
VERDICT: PASS (48/48)
exit=0

$ (cd /tmp/hft-critic-m99-a057-want0-U1kYto && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh)
FAIL  SR-L ...
PASS  MF ...
FAIL  SC ...
FAIL  SD ...
VERDICT: FAIL (3 из 48)
exit=1

$ (cd /tmp/hft-critic-m99-a057-mut-enum-XnjdFm && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh)
FAIL  MF (MF-2) ...
FAIL  SC ...
VERDICT: FAIL (2 из 48)
exit=1

$ (cd /tmp/hft-critic-m99-a057-mut-side-presence-uWDpHd && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh)
FAIL  SD ...
VERDICT: FAIL (1 из 48)
exit=1

$ (cd /tmp/hft-critic-m99-a057-mut-ls-status-tXtFvn && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh)
FAIL  SR-L ...
VERDICT: FAIL (1 из 48)
exit=1

$ (cd /tmp/hft-critic-m99-a057-mut-membership-aAXIPn && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh)
FAIL  MF (MF-1 and MF-2) ...
VERDICT: FAIL (1 из 48)
exit=1

$ (cd /tmp/hft-critic-m99-a057-mut-no-want-uZC4L2 && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh)
VERDICT: PASS (48/48)
exit=0

$ (cd /tmp/hft-critic-m99-a057-mut-no-count-KGRLBG && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh)
VERDICT: PASS (48/48)
exit=0

$ mv /tmp/hft-critic-m99-a057-candidate2-uLifZT/deploy/bin/journal-restore-drill-cron.sh{,.off}; (cd /tmp/hft-critic-m99-a057-candidate2-uLifZT && CARGO_TARGET_DIR=/tmp/hft-critic-m99-a057/target bash scripts/tests/red_restore_drill.sh); rc=$?; mv /tmp/hft-critic-m99-a057-candidate2-uLifZT/deploy/bin/journal-restore-drill-cron.sh{.off,}; echo exit=$rc
FAIL  product wrapper missing
FAIL  CR ...
FAIL  CL ...
FAIL  IM ...
VERDICT: FAIL (4 из 49)
exit=1

$ task #2d static canaries (the exact chk predicates)
no_early_pipeline_exit=0
no_global_stderr_suppression_exit=0
ps_dv_families_exit=0
sr_family_exit=0
dl_mf_family_exit=0
sc_sd_srl_families_exit=0
no_listing_here_string_exit=1
exit=0  # current pre-dev wrapper is intentionally RED on the last predicate

$ VERIFY_M99_CI_DRY=1 bash scripts/verify_M-99.sh
PASS: ci-parity: учтено шагов 65 из 65 (исполнено 59, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/reserve_artifact_id.sh C
C-321
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-17T00:00Z
- Milestone: M-99-restore-drill-automation
- Статус: DONE — NOTE
- HEAD: `2b3d9a1e` — audited subject head

## §B — Что я сделал
- Audited committed `ac6aaaa5..2b3d9a1e`, A-057, C-320, R-269, the inherited M-74 contracts/signatures, M-99 §§4/7/8/9/10(2d)/11/14, RED probe and verify gate.
- Executed the K worlds, a conforming temporary candidate, named single-seam mutants, and the merge-preview design-claim check sequentially.

## §C — Артефакты / результаты
- `research/critiques/C-321-m99-a057-derived-emptiness-plan-time.md`
- Verdict: **NOTE**. The artifact set is fit for the one A-057 plan-time round; current product RED is expected until engine-dev.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`, then `engine-dev` after the architect performs the milestone's mechanical critique-reference appendix required by the workflow.
- **Paste-ready prompt:**
  ```
  M-99 A-057 received C-321 NOTE on subject head 2b3d9a1e. Append the committed critique reference mechanically where M-99 §9 requires it; do not alter the adjudicated contract. Then dispatch engine-dev to implement M-99 task 2/2d against the committed RED suite. The implementation gate must turn the four current K failures green without weakening MF/SC/SD/SR-L, DL-M, PS/PF/DV/DV-E/SR/SR-M/PS-M/DV-M.
  ```
- Push-статус: pending this verdict commit and subject-branch push.
- Кэш: `/tmp/hft-critic-m99-a057/target` will be removed after push; temporary candidate roots remain outside the worktree as audit reproductions.

## §E — Риски / открытые вопросы
- `scripts/verify_M-99.sh` is intentionally not a green plan-time completion command: its task #1 runs the product wrapper and must report the four declared K RED worlds before engine-dev.
- No founder escalation: no new K cell observed.

=== END HANDOFF ===
