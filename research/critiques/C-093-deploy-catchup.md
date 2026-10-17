<!-- GATE-META
milestone: R-4
audited_repo: a3ka/hft-platform
audited_base: 2c56a34c0ef728eec1477f447be1ebe49b4cffd0
audited_head: ac8d4022dea14439dc78b2ebd0f69120652475fb
verdict: REJECT
-->

# C-093 — deploy catch-up: REJECT

## Предмет и границы аудита

Аудирован закоммиченный набор `a7e1c24..ac8d402` на
`origin/feat/deploy-catchup-watchdog`: `scripts/deploy_catchup.py`, его RED-проба,
`.github/workflows/deploy.yml` и `.github/workflows/ci.yml`.

T-контракты и trait-сигнатуры предмет не вводит; milestone-файл не требуется, потому что
это допустимый harness-track маршрут: барьер, проба и проводка собраны вместе
(`harness-track.md` §2–§3). Правка `deploy.yml` законна в этом маршруте: Р-4 прямо
относит её к harness/§9, а не к замку §11. Полный milestone-цикл по §4 трека не нужен
для текущего набора путей. Живой предметный инвариант: `OPS-I-8` требует алерта на
состояние «жив, но не работает»; именно такой класс создаёт зелёный `HOLD` ниже.

## Вердикт: REJECT

### R-1 — CI-агрегат не является входом RED-пробы

Сейчас проводка правильная: `ci.yml:339` содержит `deploy-catchup` в `needs`, а
`ci.yml:343` проверяет его `result`. Но проба читает только SUT и `deploy.yml`
(`red_deploy_catchup.sh:26-28`); ни `ci.yml`, ни `status-check` в неё не попадают.

Два независимых стаба CI поэтому сохранили полный зелёный прогон 39/39:

```text
=== CI STUB condition_omitted ===
сценариев: 39   PASS: 39   FAIL: 0
VERDICT: PASS
exit=0; failed_scenarios=0
stubbed_ci_lines:
339:    needs: [..., deploy-catchup]

=== CI STUB needs_and_condition_omitted ===
сценариев: 39   PASS: 39   FAIL: 0
VERDICT: PASS
exit=0; failed_scenarios=0
stubbed_ci_lines:
339:    needs: [..., gate-meta]
```
Первый стаб — ровно известный класс: `deploy-catchup` может быть красным, но агрегат
всё равно напечатает `All checks passed`. Второй доказывает, что не закреплена и
зависимость `needs`. Это два пропущенных стаба из двух на данной поверхности.

**Условие снятия:** RED-проба (или отдельная герметичная проба CI-агрегата) обязана
подсунуть оба стаба и падать: (1) `deploy-catchup` остаётся в `status-check.needs`; (2)
его результат участвует в fail-closed условии; (3) модель красного
`deploy-catchup` не даёт `All checks passed`.

### R-2 — `HOLD` не передаётся человеку: сторож зелёный и молчит

`classify_runs()` правильно различает VPS-failure и нераспознанный job как `HOLD`
(`deploy_catchup.py:232-266`). Однако `cmd_decide()` после любого решения вызывает
`emit()` и возвращает `0` (`:318-321`). Workflow запускает VPS-deploy только при
`decision == 'DEPLOY'` (`deploy.yml:184-195`). Значит `HOLD` даёт успешный catch-up job
и skipped deploy: не происходит автодобора (это верно), но не возникает ни красного
гейта, ни другого предъявленного маршрута к человеку.

Исполнение на реальном SUT для упавшего VPS-joba:

```text
$ CATCHUP_* python3 scripts/deploy_catchup.py decide
decision=HOLD
reason=ран 902: Deploy (build on VPS) УПАЛ на VPS — авто-ретрай детерминированной ошибки запрещён (Р-4), решает человек
exit=0
```

И временной стаб переименования показывает ту же слепоту, из-за которой сторож должен
будить человека:

```text
=== TEMPORAL RENAME STUB ===
decision=HOLD
reason=в ране 901 нет джоба 'Deploy v2 (build on VPS)' (переименование? обрезанный ответ API) — VPS мог быть тронут, решает человек
exit=0
```

Это не является уведомлением. `DESIGN.md` §23.1 подтверждает, что внешний Telegram-канал
сейчас штатный no-op; следовательно, предположить иной сигнал человеку нельзя. D8/D9
сегодня закрепляют именно зелёный `HOLD` как успех пробы
(`red_deploy_catchup.sh:225-244`, `:78-99`), то есть оракул принимает дефект, а не
наблюдает отсутствие по `testing.md` §«Целостность гейта», свойство 4.

**Условие снятия:** сохранить запрет автодобора для `HOLD`, но сделать его
наблюдаемым человеком: неуспешный/явно эскалирующий job либо доказанный внешний dispatch.
RED должен исполнять VPS-failure, отсутствующий/переименованный VPS-job и неполные jobs;
для каждого доказать одновременно: VPS-deploy не вызван, а гейт не остаётся зелёным без
эскалации.

## Адверсарные стабы, пойманные существующей пробой

Полный набор прогнан против четырёх дополнительных SUT-стабов. Все они пойманы; это
не снимает R-1/R-2, но подтверждает полезные части различителя.

| Стаб | Красных сценариев из 39 | Пропущено |
|---|---:|---:|
| Наличие любого рана трактуется как успешный деплой | 9 | 0 |
| Пустая история ранов трактуется как уже доставленный код | 2 | 0 |
| Нерезолвимый SHA VPS молча превращается в пустую дельту | 1 | 0 |
| Имя VPS-joba переименовано относительно истории | 5 | 0 |

Сырой итог:

```text
=== STUB presence_only ===
сценариев: 39   PASS: 30   FAIL: 9
VERDICT: FAIL
exit=1; failed_scenarios=9

=== STUB empty_history_skip ===
сценариев: 39   PASS: 37   FAIL: 2
VERDICT: FAIL
exit=1; failed_scenarios=2

=== STUB unresolved_sha_skip ===
сценариев: 39   PASS: 38   FAIL: 1
VERDICT: FAIL
exit=1; failed_scenarios=1

=== STUB renamed_vps_job ===
сценариев: 39   PASS: 34   FAIL: 5
VERDICT: FAIL
exit=1; failed_scenarios=5
```

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-093
exit=0

$ bash scripts/check_artifact_ids.sh ac8d4022dea14439dc78b2ebd0f69120652475fb
OK: ни один коммит диапазона ac8d402..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ git log --oneline origin/main..ac8d402
ac8d402 feat(deploy-catchup): проводка — workflow_run в deploy.yml, джоб в CI-агрегате [architect]
56272eb test(deploy-catchup): проба сторожа — 39 сценариев + батарея 10 мутантов [architect]
a7e1c24 feat(deploy-catchup): решение DEPLOY/SKIP/HOLD + барьер проводки [architect]

$ git diff --name-status origin/main...ac8d402
M       .github/workflows/ci.yml
M       .github/workflows/deploy.yml
A       scripts/deploy_catchup.py
A       scripts/tests/red_deploy_catchup.sh
exit=0

$ grep -nE 'needs: \[|deploy-catchup.result' .github/workflows/ci.yml | tail -2
339:    needs: [build-test, security, delivery, protected-artifacts, contracts, docs-freeze, artifact-ids, reserve-ids, design-claims, context-budgets, gate-meta, deploy-catchup]
343:          if [[ ... || "${{ needs.deploy-catchup.result }}" != "success" ]]; then
exit=0

$ CATCHUP_* python3 scripts/deploy_catchup.py decide  # VPS job failure fixture
decision=HOLD
reason=ран 902: Deploy (build on VPS) УПАЛ на VPS — авто-ретрай детерминированной ошибки запрещён (Р-4), решает человек
exit=0

$ temporary fixture cleanup check
temporary_stubs_removed=1
red_catchup_fixture_dirs_remaining=0
exit=0

$ git diff --check origin/main...ac8d402
exit=0
```
