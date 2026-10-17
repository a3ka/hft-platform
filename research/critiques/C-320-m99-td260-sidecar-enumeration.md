<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: 18a68daa3369be224214ec191bc0f1d2625e4ff7
audited_head: b71cbcb0d05d92ef9ec7d9d123293c1ea773a929
verdict: REJECT
-->

# C-320 — M-99 / TD-260 круг 3: sidecar-ы могут молча исчезнуть после успешной выборки сегментов — REJECT

## Вердикт

**REJECT.** `R-269` Б-1/Б-2 исправлены на уровне plan-time набора: текущая реализация
краснеет ровно на `MF` (1/45), а временная копия обёртки с guard `want=0` проходит 45/45.
`MF` не вакуумен: его setup-guard доказывает на этом хосте ложное «нет» большого
here-string и реальное наложение `prlimit` после первой выборки.

Но этот guard закрывает только **формы выбранных индексов**. Тот же дефект остаётся во
втором here-string, которым перечисляются корневые `*.json` sidecar-ы. Если ресурсный
отказ возникает после последней выборки сегмента, но до этого перечисления, `grep -E …
<<<"${LISTING}"` даёт пустой список; цикл не делает ни одной выборки и обёртка с
успешным читателем пишет `ok=1`, `checked=3`. Это нарушает §4(2) M-99: каждый sidecar
копии обязан быть восстановлен побайтно. Именно этот класс — внешняя операция/проверка
заявляет успех без требуемых байтов — уже запрещён §7 и должен падать fail-closed.

`OPS-I-3` остаётся живым инвариантом: холодная копия должна **восстанавливаться и
читаться**, а не только давать успешный статус. Положительный reader-stub не отменяет
обязательство доставить его контекст (`journal.legacy.json` и
`journal.replay-digest.json`).

## Б-1 — resource failure после сегментов молча обходит доставку sidecar-ов

В `deploy/bin/journal-restore-drill-cron.sh:301-304` список sidecar-ов строится так:

```bash
done <<<"$(grep -E '^[^/]+\.json$' <<<"${LISTING}")"
```

Код не проверяет статус внутреннего `grep`, не доказывает, что каждый expected sidecar
попал в `RESTORE`, и переходит к reader-у. В отличие от `MF`, после выбора последних
сегментных форм больше нет `want=0`-guard, который мог бы перехватить отказ here-string.

### Воспроизведение

Я взял ту же временную conforming-копию, которая добавляет только предложенный guard
`want=0` для выбранного индекса, и не менял репозиторий. Фикстура имеет listing PS
(108419 байт, 4626 имён), четыре выбранные сегментные формы и два sidecar-а. Подменный
`cp` честно копирует первые четыре сегментные формы, затем накладывает
`RLIMIT_FSIZE=16384` на процесс обёртки; сигнал `XFSZ` игнорируется, как в `MF`.

```text
$ bash /tmp/hft-critic-m99-sidecar2-vXyf6f/repro.sh
wrapper_rc=0
limiter=applied
state={"ok":1,"ts_wall_ms":1792241661000,"checked":3,"events_read":7,"digest":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","reason":""}
selected_files=4
sidecar_files=0
reader_called=yes
here_string_error=2
exit=0
```

Следовательно, «MF GREEN» не означает закрытия всего класса delivery: он не достигает
этого более позднего consumer-а того же большого `LISTING`.

## Что в наборе подтверждено

- Диапазон `c3b91218..b71cbcb0` меняет только
  `milestones/M-99-restore-drill-automation.md`,
  `scripts/tests/red_restore_drill.sh` и `scripts/verify_M-99.sh`. T1/T2-формы,
  trait-сигнатуры и `contracts/**` не менялись; governing interface наследуется из M-74.
- §4 строка 12(в), §8, §10 task 2d, §11 и §14 честно отражают R-269: прежнее ложное
  утверждение об отсутствии эксклюзивного члена снято, а история не переписана.
- `MF` использует переносимый для CI механизм только на Linux runner: workflow задаёт
  `ubuntu-latest`; local run показал GNU bash 5.2.21 и `prlimit` util-linux 2.39.3.
  Его setup-guards fail closed, если будущий runner не даёт измеренную семантику
  here-string либо не может наложить limit. Это не закрывает Б-1, который воспроизводится
  именно после успешной части доставки.
- `scripts/verify_M-99.sh` остаётся реальным FAIL-агрегатором: CI-parity dry-run
  учёл 65/65 шагов. Полный run был начат и дошёл до CI-parity (`fmt` и `clippy` PASS),
  но был остановлен вручную во время длительного независимого `cargo test --all`; это
  **не** предъявляется как PASS.

## Условие снятия REJECT

Architect должен добавить RED-мир, где:

1. listing имеет форму `PS`; все выбранные сегментные формы честно доставлены;
2. тот же ограничитель включается только после последней сегментной выборки и setup-guard
   доказывает, что он действительно наложен;
3. перечисление или доставка хотя бы одного sidecar-а не состоялась; текущая реализация и
   копия, исправляющая только `want=0`, краснеют;
4. норма требует fail-closed до reader-а: `ok=0`, пустой digest,
   `transport-failed`, код 5, состояние записано, `checked=3` (все сегментные индексы
   действительно доставлены), stderr alert и reader не вызван.

Нужна соответствующая правка §4/§7/§8/§9/§10(2d)/§11/§14 и task-2d canary в verify.
Форму исправления не предписываю: она должна отличать ошибку вычисления списка sidecar-ов
и недоставленный sidecar от честного успешного мира, не ослабляя PS/DV/DV-E/SR/MF и их
самопроверки. Затем — новый critic-круг над коммитом architect-а; engine-dev не
диспетчеризуется по этому набору.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-99-td260-sampling
b71cbcb0d05d92ef9ec7d9d123293c1ea773a929
exit=0

$ git merge-base origin/main HEAD
18a68daa3369be224214ec191bc0f1d2625e4ff7
exit=0

$ git diff --name-status c3b91218..HEAD
M  milestones/M-99-restore-drill-automation.md
M  scripts/tests/red_restore_drill.sh
M  scripts/verify_M-99.sh
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  MF … MF-1 … ok=1, rc=0, checked=3
FAIL  MF … MF-2 … ok=1, rc=0, checked=3
VERDICT: FAIL (1 из 45)
exit=1

$ bash /tmp/hft-critic-m99-mf-eji67a/red_restore_drill.sh
PASS  MF … ok=0, transport-failed, код 5, checked = доставленные до отказа (0; 1), читатель не вызван
VERDICT: PASS (45/45)
exit=0

$ bash /tmp/hft-critic-m99-sidecar2-vXyf6f/repro.sh
wrapper_rc=0
limiter=applied
state={"ok":1,…,"checked":3,…,"reason":""}
selected_files=4
sidecar_files=0
reader_called=yes
here_string_error=2
exit=0

$ bash -n deploy/bin/journal-restore-drill-cron.sh scripts/tests/red_restore_drill.sh scripts/verify_M-99.sh
exit=0

$ VERIFY_M99_CI_DRY=1 bash scripts/verify_M-99.sh
PASS: ci-parity: учтено шагов 65 из 65 (исполнено 59, исключено по карте 6)
VERDICT: PASS
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_M-99.sh
… stopped by critic after 21m48 during CI-parity `cargo test --all`; no terminal result asserted
exit=143 (SIGTERM)
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-17T13:23Z
- Milestone: M-99-restore-drill-automation
- Статус: BLOCKED
- HEAD: b71cbcb0 — audited subject head

## §B — Что я сделал
- Audited the committed range `c3b91218..b71cbcb0`, R-269, C-318/C-319, the milestone, RED probe, verify gate, and task-2d implementation.
- Reproduced a delivery-success status with no sidecar bytes after the planned MF guard.

## §C — Артефакты / результаты
- `research/critiques/C-320-m99-td260-sidecar-enumeration.md`
- Done Block above. Verdict: REJECT.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-99 / TD-260: C-320 REJECT found an unguarded second here-string in sidecar enumeration.
  Before dispatching engine-dev, amend the architect-owned milestone, RED probe, and verify gate.
  Add a resource-world that arms after all selected segment forms are delivered, then proves a
  sidecar-list/delivery failure is `transport-failed`/5, writes failure state with checked=3,
  alerts, and never calls the reader. It must fail on the current wrapper and a want=0-only
  candidate, pass on a complete candidate, and preserve PS/DV/DV-E/SR/MF plus all judge mutations.
  Commit and push the artifact trio, then request a new strong RAW critic audit.
  ```
- Push-статус: pending this verdict commit and subject-branch push.
- Кэш: pending removal after push.

## §E — Риски / открытые вопросы
- `bash scripts/verify_M-99.sh` did not reach a terminal result: I stopped its independent full-workspace CI-parity test after 21m48; do not report it as green.
- The scope is still M-99 task 2d; no founder boundary-C decision is requested.

=== END HANDOFF ===
