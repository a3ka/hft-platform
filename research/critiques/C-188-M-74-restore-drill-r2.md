<!-- GATE-META
milestone: M-74
audited_repo: a3ka/hft-platform
audited_base: 669ac028eae3fa9377c032b15b847f0c43c8a0aa
audited_head: b8d989eee53b6aebd425de37280ca5e22d98bb95
verdict: REJECT
-->

# C-188 — M-74 restore-drill, круг 2 — REJECT: набор всё ещё неполон

## Предмет и итог

Сужу только закрытие B-1…B-4 из C-187 в закоммиченном диапазоне
`669ac02..b8d989e` ветки `docs/M-73-closeout-architect`, а не handoff-текст.
M-73 parser-step действительно починен; B-3 теперь имеет детерминированный
отбор, sidecar'ы и честно названный предел невыбранной порчи. T2-файл состояния
называет реального владельца producer-path — sampler recorder — и больше не
предлагает записывать gauge из отдельного cron-процесса.

Но это всё ещё **REJECT / NOT REVIEWED — ARCHITECT ARTIFACTS INCOMPLETE**.
Сама M-74 (строки 161–166) прямо сообщает, что обязательный RED
`crates/recorder/tests/red_restore_drill_metric.rs` для задач 3 и 5 **не
написан**. В Allowed paths (строка 84) разрешён `crates/ops/tests/**`, но не
этот реальный дом теста. Поэтому нет закоммиченного до dispatch оракула, который
доказывает T2-переход «состояние drill → gauge в фактическом `/metrics`» и
проверяет failure/stale = 0. Отложить RED до реализации задачи 3 — это именно
неполный architect-набор, а не допустимая последовательность работы dev.

Это второй REJECT по той же фундаментальной причине C-187 B-1 — отсутствие
полного pre-dispatch наборa. По `gates.md §0` следующий разбор обязан идти к
**arbiter**, а не в третий круг architect.

## Блокеры

### B-1 остаётся — RED tasks 3/5 и его путь не закоммичены

`scripts/verify_M-74.sh` требует один и тот же integration-test дважды:
обычный путь на строке 68 и stale path на строке 80. Файла
`crates/recorder/tests/red_restore_drill_metric.rs` в `b8d989e` нет. Сам gate
поэтому не может стать зелёным; `cargo test -p recorder --test
red_restore_drill_metric` не имеет test target.

Это также оставляет B-2 непроверяемым по существу. Документированная схема
T2 выглядит осуществимой (sampler recorder читает state-file и владеет
`Arc<Metrics>`), но без закоммиченного RED/producer signature невозможно
доказать, что именно этот процесс, а не registry-only декларация, публикует
`backup_restore_drill_ok` в `/metrics` согласно OPS-I-10.

### B-1a — закоммиченный shell RED не является валидной пробой prod-reader

Новый `scripts/tests/red_restore_drill.sh` различает свою H/C/M-обёртку, но
его положительный fixture создаёт сегменты как plain bytes
`SEGMENT-0001-PAYLOAD` и legacy-sidecar как
`{"legacy":["segment-0001.jrnl"]}`. Это не вход production reader:
`journal::stream` читает journal frames, а `contracts::LegacyManifest` содержит
обязательное поле `declarations: Vec<LegacySegmentDecl>`, не `legacy`.

Следовательно, настоящая обёртка, вызывающая production reader, не может
пройти H на этом fixture. Обёртка, которая H проходит, должна обойти reader
специальным/mock-парсером. Такая проба может стать зелёной при плацебо и не
доказывает OPS-I-3. Нужен RED с данными, построенными реальным writer/reader
форматом (и с валидным legacy manifest), который затем вызывает тот же reader,
что будет использован production drill.

## Закрытия, принятые в этом круге

- **B-3:** выборка отсортирована детерминированно (первый, `floor(N/2)`,
  последний), захватывает `journal.legacy.json`, `journal.meta` и
  `journal.replay-digest`; непрерывность проверяется внутри выбранного сегмента.
  Неохваченная порча невыбранного сегмента теперь названа пределом sampling.
- **B-4:** `verify_M-73.sh` вызывает существующий `chk` для каждого
  `crontab -n`; полный gate прошёл. Мутация cron-файла отвергается parser'ом.
- **helper self-check M-74:** намеренно сломанный `chk`, который не инкрементирует
  `FAIL`, останавливает gate с exit 1. Это не украшение.
- **Граница C:** диапазон не меняет `RETENTION_MODE`; в production cron остаётся
  `dry-run`. M-74 открывает, но не решает вопрос об удалении — founder signature
  не подменена.

## Условие разблокирования (решает arbiter)

Arbiter должен определить, допустима ли объявленная architect последовательность
«RED recorder будет написан dev после task 3». Если нет — возвратить M-74 с
требованием закоммитить до dispatch `crates/recorder/tests/red_restore_drill_metric.rs`,
поправить Allowed paths и заменить shell fixture на production-valid. Если да —
зафиксировать явное исключение из правила полного pre-dispatch artifact set и
назвать, кто несёт риск OPS-I-3/OPS-I-10. До этого решения dev не назначается.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-188
exit=0

$ git rev-parse 669ac028eae3fa9377c032b15b847f0c43c8a0aa; git rev-parse HEAD
669ac028eae3fa9377c032b15b847f0c43c8a0aa
b8d989eee53b6aebd425de37280ca5e22d98bb95
exit=0

$ git diff --name-status 669ac02..HEAD -- crates scripts milestones deploy
M\tmilestones/M-74-restore-drill.md
A\tscripts/tests/red_restore_drill.sh
M\tscripts/verify_M-73.sh
A\tscripts/verify_M-74.sh
exit=0

$ test -f crates/recorder/tests/red_restore_drill_metric.rs || echo MISSING
MISSING crates/recorder/tests/red_restore_drill_metric.rs
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  обёртки deploy/bin/journal-restore-drill-cron.sh НЕ СУЩЕСТВУЕТ — RED задачи 1
VERDICT: FAIL (1 из 1) — RED-first: спецификация есть, реализации нет
red_restore_drill_exit=1
exit=1

$ rg -n 'SEGMENT-|"legacy"|pub struct LegacyManifest|pub fn stream' \\
  scripts/tests/red_restore_drill.sh crates/contracts/src/lib.rs crates/journal/src/segments.rs
scripts/tests/red_restore_drill.sh:76:  printf 'SEGMENT-%s-PAYLOAD'
scripts/tests/red_restore_drill.sh:79:  printf '{"legacy":["segment-0001.jrnl"]}'
crates/contracts/src/lib.rs:68:pub struct LegacyManifest {
crates/journal/src/segments.rs:1829:pub fn stream(
exit=0

$ <копия verify_M-74.sh с мутацией: chk не увеличивает FAIL>
FAIL: самопроверка chk — помощник не считает отказы; весь гейт был бы зелёным ни о чём
VERDICT: FAIL (1)
mutated_chk_gate_exit=1
exit=1

$ <временная копия deploy/cron.d/journal-offsite с лишней строкой>; crontab -n <копия>
bad minute
mutated_crontab_exit=1
exit=1

$ bash scripts/verify_M-73.sh
PASS: crontab -n 'deploy/cron.d/builder-prune'
PASS: crontab -n 'deploy/cron.d/journal-offsite'
PASS: crontab -n 'deploy/cron.d/journal-retention'
VERDICT: PASS
exit=0

$ bash scripts/verify_M-74.sh
PASS: самопроверка chk — зелёное проходит, красное СЧИТАЕТСЯ
PASS: cargo test --all --quiet
FAIL: bash scripts/tests/red_restore_drill.sh
FAIL: cargo test -p recorder --test red_restore_drill_metric --quiet
FAIL: cargo test -p recorder --test red_restore_drill_metric --quiet stale
VERDICT: FAIL (7)
exit=1

$ git diff 669ac02..HEAD -- docker-compose.yml | rg RETENTION_MODE; rg -n RETENTION_MODE deploy/cron.d/journal-retention
41:RETENTION_MODE=dry-run
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: OPS-I — заявлено=10, в оракулах=8 — подтверждено замером (loose=8)
VERDICT: PASS (0 нарушений)
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-30T22:xxZ
- Milestone: M-74 restore-drill, critic round 2
- Статус: BLOCKED — second REJECT, mandatory arbitration
- HEAD: `b8d989eee53b6aebd425de37280ca5e22d98bb95`

## §B — Что я сделал
- Аудировал коммиты после C-187, не переоткрывая закрытые B-3/B-4.
- Самостоятельно прогнал M-73 gate, мутацию cron parser и M-74 helper self-check;
  проверил actual fixture относительно `journal::stream` и контракта legacy manifest.
- Подтвердил, что boundary C не сдвинута: retention остаётся dry-run.

## §C — Артефакты / результаты
- `research/critiques/C-187-M-74-restore-drill.md` — первый REJECT.
- `research/critiques/C-188-M-74-restore-drill-r2.md` — настоящий второй REJECT.
- B-3 и B-4 закрыты; B-1 остаётся: обязательный recorder RED отсутствует, а
  shell RED использует неproduction fixture.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter` (не architect: второй REJECT того же основания).
- **Paste-ready промпт:**
  ```
  Разреши конфликт C-187/C-188 по M-74 на b8d989e. Architect считает допустимым
  не коммитить crates/recorder/tests/red_restore_drill_metric.rs до dev task 3;
  critic применяет правило полного pre-dispatch набора (T-contract, signatures,
  RED tests, verify, milestone) и поэтому REJECT. Файл прямо назван в M-74, но
  отсутствует и даже не покрыт Allowed paths. Дополнительно shell RED создаёт
  plain-byte segments и неверный legacy manifest, несовместимые с journal::stream.
  Реши: (a) возврат architect с обязательным pre-dispatch RED + valid fixture,
  или (b) явно одобренное исключение, владелец риска и достаточный компенсирующий
  gate. RETENTION_MODE остаётся dry-run; founder boundary не затронута.
  ```
- Push-статус: pending — C-188 commits and pushes to the subject branch with this gate.
- Кэш: `/tmp/hft-critic-m74r2/target` будет удалён после завершения M-74 verifier.

## §E — Риски / открытые вопросы
- Без recorder RED нельзя доказать OPS-I-10 (реальная `/metrics` sample) и failure/stale
  semantics, следовательно OPS-I-3 не имеет приемочного оракула.
- Green оболочки против своего формата fixture может означать mock-reader и не restore drill.

=== END HANDOFF ===
