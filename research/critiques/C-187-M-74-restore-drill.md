<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: 1dda5b180b1f3e8e881364a381e4f02e540f5b24
audited_head: e177691486ee45e539711b0a25ffc7ba204bc844
verdict: REJECT
-->

# C-187 — M-74 restore-drill — REJECT: набор architect-артефактов неполон

## Предмет и граница аудита

Сужу закоммиченную вершину `e177691` ветки
`docs/M-73-closeout-architect` поверх `origin/main` `1dda5b1`, а не handoff-текст.
Диапазон содержит три коммита и меняет только `docs/fa/ops.md`, M-73, M-74 и
`scripts/verify_M-73.sh`.

T1 в диапазоне не меняется — это согласуется с разделом `Contract impact` M-74.
Но для M-74 отсутствуют обязательные до dispatch набора: T2-контракт/сигнатура
перехода «результат отдельного cron-процесса → метрика процесса recorder», RED-тесты
и `scripts/verify_M-74.sh`. Поэтому это **REJECT / NOT REVIEWED — ARCHITECT
ARTIFACTS INCOMPLETE**: dev нельзя назначать, пока отсутствующий набор не будет
закоммичен и повторно предъявлен.

`OPS-I-2` назван по живому FA-инварианту. Он подтверждён независимым ssh-замером:
расписание установлено, `last-success` на момент замера был свежим, тревожного файла
нет. `OPS-I-3` остаётся неисполненным; M-74 не выдаёт `RETENTION_MODE=apply` за
следствие drill'а и корректно оставляет вопрос founder'у. Это не причина REJECT.

## Блокеры

### B-1 — RED, verify и T2-сигнатуры M-74 не закоммичены

`milestones/M-74-restore-drill.md` существует, но его собственные пути
`scripts/tests/red_restore_drill.sh` и `scripts/verify_M-74.sh` отсутствуют в
аудируемой вершине. Диапазон не добавляет и T2-границу/сигнатуру, которая задаёт
вызов продового reader'а и передачу результата drill'а в metrics.

Текст задач 1, 5 и 6 обещает будущие RED и gate, но это не заменяет закоммиченный
падающий оракул и реальный acceptance-script. По `critic.md` набор обязан существовать
**до** dev; плановый текст сам по себе не является предметом, готовым к dispatch.

### B-2 — задача 3 не имеет осуществимого producer-path в Allowed paths

M-74 разрешает для эмиссии только `crates/ops/src/**`, хотя `/metrics` держит
`Arc<Metrics>` внутри процесса recorder: `recorder::metrics_server::serve` получает
эту дугу, а `main.rs` создаёт её как `Arc::new(Metrics::new())`. Отдельная cron-обёртка
не может установить gauge в память уже работающего recorder. Текущая producer-карта
прямо помечает `backup_restore_drill_ok` как `deferred (task 3, OPS-I-2/3)` в
`crates/recorder/src/metric_emit.rs:22`; в коде нет вызова `set_gauge` для этой серии.

Следовательно, `crates/ops/src/**` может сохранить имя в реестре, но не может выполнить
`OPS-I-10` — реальную sample-серию в `/metrics`. Milestone должен заранее назвать
владельца и T2-интерфейс/состояние, расширить Allowed paths до действительно нужной
recorder-зоны и приложить RED, который запускает тот же producer, что увидит
`/metrics`. Иначе task 3 требует от dev либо scope violation, либо registry-only
плацебо, ровно исключённое OPS-I-10.

### B-3 — правило выборки не восстанавливает необходимый контекст и молчит о своей границе

Правило выбирает только три сегмента. Старейший боевой сегмент — legacy-класс: продовый
reader читает такой сегмент только при явной записи в `journal.legacy.json`
(`crates/journal/src/segments.rs:22-24`), а на VPS этот sidecar существует. M-74 не
требует положить manifest рядом с выбранным старейшим сегментом. Поэтому здоровая legacy
копия может корректно fail-closed ещё до проверки её читаемости; текущая спецификация не
задаёт, как отличить это от повреждения.

Кроме того, три одиночных, заведомо не смежных сегмента не могут одновременно проверить
непрерывность `seq` на межсегментных границах. Порча или пропуск любого невыбранного
сегмента в середине также не обнаруживается (у единственной middle-выборки лишь шанс
попасть в такой файл). Это допустимый предел sampling только если он назван и принят:
сейчас M-74 называет классы выборки, но не называет этот класс непойманной порчи, алгоритм
детерминированного отбора и контекстные sidecar-файлы. RED должен проверять именно
legacy-manifest prerequisite, границы/пропуски seq и документированную границу sampling,
а не только перевёрнутый байт в выбранном файле.

### B-4 — добавленный M-73 acceptance-step не исполняет `crontab -n` и зелёнеет

В изменённом `scripts/verify_M-73.sh` определена только функция `chk` (строка 32), но
шаг TD-192 вызывает несуществующий `chk_sh` (строка 153). На данном окружении `crontab`
установлен. Так как скрипт не использует `set -e`, command-not-found не увеличивает
`FAIL`; шаг не исполняет продовый parser и gate может закончиться PASS. Это отменяет
новое заявление close-out, что TD-192 механизировано, хотя ручной ssh-замер M-73 верно
показывает, что три текущих cron-файла принимаются parser'ом.

До следующего круга assertion должен быть исправлен и иметь оракул, который краснеет,
когда проверка parser'а не была вызвана или вернула non-zero. Нельзя считать ручной
успех `crontab -n` заменой живого шага acceptance-gate.

## Условия повторного предъявления

1. Закоммитить полный architect-набор M-74: явный T2/trait-or-CLI contract, RED-пути
   задач 1/3/5 и `verify_M-74.sh` с проверкой на каждую задачу и CI parity.
2. Предъявить RED, который доказывает реальную `/metrics`-эмиссию `1`, failure и stale
   `0` через фактический producer-path, а не через имя в `METRICS`.
3. Уточнить restore-set: manifest/необходимые sidecars, reader invocation и семантику
   непрерывности для выбранных сегментов; назвать либо устранить границу невыбранной
   порчи.
4. Предъявить работающий M-73 шаг `crontab -n` с мутацией на несостоявшийся/проваленный
   parser-check.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-187
exit=0

$ git rev-parse origin/main; git rev-parse HEAD; git merge-base --is-ancestor origin/main HEAD
1dda5b180b1f3e8e881364a381e4f02e540f5b24
e177691486ee45e539711b0a25ffc7ba204bc844
exit=0

$ git diff --name-status origin/main...HEAD
M\tdocs/fa/ops.md
M\tmilestones/M-73-offsite-schedule.md
A\tmilestones/M-74-restore-drill.md
M\tscripts/verify_M-73.sh
exit=0

$ test -e scripts/verify_M-74.sh; echo verify_M74=$?
verify_M74=1
$ test -e scripts/tests/red_restore_drill.sh; echo red_restore_drill=$?
red_restore_drill=1
exit=0

$ rg -n '^chk(_sh)?\\(|chk_sh ' scripts/verify_M-73.sh
32:chk() {
153:    chk_sh "crontab -n '$f'" "1quater $(basename "$f") принят прод-парсером crontab -n"
exit=0

$ bash -u -o pipefail -c '<TD-192 body with undefined chk_sh; no set -e>'
bash: line 4: chk_sh: command not found
after_undefined_checker_FAIL=0
focused_reproduction_exit=0

$ ssh … 'date -u; test -f /etc/cron.d/hft-journal-offsite; cat /var/lib/hft/journal-offsite.last-success; test -e /var/lib/hft/journal-offsite.alert'
REMOTE_UTC=2026-08-30T20:53:40Z
CRON_FILE=present
LAST_SUCCESS=2026-08-30T20:22:28Z
ALERT=absent
exit=0

$ rg -n 'backup_restore_drill_ok' crates/recorder/src crates/ops/src
crates/recorder/src/metric_emit.rs:22://! - `backup_restore_drill_ok`         — deferred (task 3, OPS-I-2/3).
crates/ops/src/metrics.rs:110:        name: "backup_restore_drill_ok",
crates/ops/src/alerts.rs:117:        metric: "backup_restore_drill_ok",
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: OPS-I — заявлено=10, в оракулах=8 — подтверждено замером (loose=8)
VERDICT: PASS (0 нарушений)
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-30T21:xxZ
- Milestone: M-74-restore-drill
- Статус: BLOCKED — REJECT
- HEAD: e177691 — spec(M-74): restore-drill — копия обязана ЧИТАТЬСЯ; OPS-I-2 исполнен, OPS-I-3 нет [architect]

## §B — Что я сделал
- Аудировал закоммиченную ветку, M-74, M-73/FA-изменение, текущий producer-path и live M-73 evidence.
- Подтвердил OPS-I-2 на VPS; нашёл неполный M-74 artifact set и false-green в новом TD-192 acceptance-step.

## §C — Артефакты / результаты
- `research/critiques/C-187-M-74-restore-drill.md`
- Done Block: `verify_design_claims --merge-preview` exit=0; M-74 RED/verify отсутствуют; focused TD-192 reproduction exit=0 при `chk_sh: command not found`.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  C-187 REJECT по M-74 на e177691. До dispatch dev закоммить полный architect-набор: T2/CLI contract между результатом drill и process-local recorder Metrics, RED задач 1/3/5, scripts/verify_M-74.sh с CI parity. Разрешённые пути должны покрыть реальный producer `/metrics`; selection обязан нести legacy manifest/sidecars, reader invocation и честную границу невыбранной порчи. Также M-73 TD-192 gate сейчас зовёт undefined chk_sh: докажи исправленный вызов прод-парсера мутацией. Не меняй RETENTION_MODE=apply; C-187 подтвердил, что решение остаётся founder boundary.
  ```
- Push-статус: ⏸ commit and push are performed with this gate artifact
- Кэш: ⏸ кэш оставлен — worktree ещё нужен для commit/push гейта

## §E — Риски / открытые вопросы
- Sampling трёх сегментов не является доказательством читаемости всей копии, пока его остаток не назван или не закрыт покрытием.
- `OPS-I-3` остаётся OPEN; founder-вопрос о `RETENTION_MODE=apply` ещё не открыт.

=== END HANDOFF ===
