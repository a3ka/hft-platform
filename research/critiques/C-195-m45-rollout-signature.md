<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: d77398d7b22396c452d2651e90498033186055dd
audited_head: c1ebac13a5ab0e53101351f9a6db3c757a40e690
verdict: REJECT
-->

# C-195 — M-45 rollout signature: задача 7 не имеет исполнимой зоны и реального acceptance-гейта

**Вердикт: REJECT.** `П-026` не пересуждается: она подписывает `ETHUSDT` одновременно для spot и futures. Четыре остатка `R-162` §5 закрыты по существу в `c1ebac1`, а ожидаемый объём верно описан как рост `+5–7 %` до отдельной инверсии. Но новую задачу 7 нельзя диспетчеризовать: она назначена `engine-dev` на корневой `docker-compose.yml`, которым эта роль не владеет. Второй независимый блокер: `scripts/verify_M-45.sh` не проверяет задачу 7 и проходит зелёным, пока обе объявленные ею env-переменные отсутствуют.

## Блокеры

### B-1 — задача 7 не исполнима в объявленной ролевой зоне

`M-45` §Tasks назначает задачу 7 `engine-dev`: одним коммитом объявить `L2DELTA_CAPTURE_SYMBOLS` и `EPOCH_ID` в корневом `docker-compose.yml`.

Однако `.claude/rules/scope-guard.md` разрешает `engine-dev` только перечисленные `crates/*/src/**` и `deploy/**`; корневого `docker-compose.yml` в зоне нет. `docs/04-workflow.md` §3 закрепляет, что таблица scope-guard — закон, а выход за неё требует `SCOPE VIOLATION REQUEST` и стоп. `Allowed paths` milestone не может выдать роли путь, который глобальная таблица ей не даёт.

Механика состава здесь не спорна: одна `L2DELTA_CAPTURE_SYMBOLS` читается обоими venue-потребителями, per-venue ключа нет, а `П-026` подписывает именно обе площадки. Это означает, что задача функционально однозначна, но **не имеет разрешённого исполнителя для названного файла**.

Условие снятия: architect должен привести назначение роли и путь задачи 7 к совместимой со scope-guard форме. До этого `engine-dev` обязан остановиться с `SCOPE VIOLATION REQUEST`, а не менять root compose.

### B-2 — новый task не покрыт RED/verify и текущий зелёный gate его не судит

`gates.md` §3 и `docs/04-workflow.md` §3 требуют минимум одну проверку на каждую задачу milestone. В актуальном `scripts/verify_M-45.sh` нет исполняемой проверки `docker-compose.yml`, `L2DELTA_CAPTURE_SYMBOLS` или `EPOCH_ID`: совпадения — только комментарии. Поэтому проверка проходит `VERDICT: PASS` на `c1ebac1`, хотя обе env-строки по-прежнему отсутствуют в compose.

Ссылка задачи 7 на `П-026` §Порядок — инструкция, не оракул. Она не доказывает, что обе переменные окажутся на сервисе `hft-recorder` одним изменением, и не делает gate красным до исполнения. Это прямое нарушение acceptance-script-as-real-gate и RED-first для добавленной задачи.

Условие снятия: до dispatch должен быть закоммичен оракул/verify-пункт задачи 7, который красный на нынешнем compose и зелёный только при корректной конфигурации раскатки; проверка должна судить конфигурацию/точку запуска, а не наличие текста в документации. Затем повторить critic-круг над обновлённым коммит-диапазоном.

## Подтверждённое, не являющееся находкой

- `R-162` §5 закрыт по существу: N-1 теперь якорится в `DESIGN` §6 + `gates.md` §0.1; N-3 честно оставляет seq-границу открытой до деплоя; N-4 называет обязательный critic-маршрут; N-5 называет создание resync-emission пути работой; N-6 получил носитель в `BACKLOG`.
- §4bis не втягивает инверсию в закрытый M-45: отдельный milestone и отдельная founder-подпись названы, новые implementation-задачи/Allowed paths/Acceptance для инверсии не добавлены. Это нормативная форма и потому правильно отправлена в текущий critic-круг.
- Знак объёма корректен: `L2Snapshot` всё ещё эмитится периодически и не зависит от allow-list, поэтому до инверсии `L2Delta` добавочны. Оракул, ожидающий падения, действительно объявил бы исправную раскатку дефектом.
- Полный унаследованный набор M-45 существует: T1 не меняется; T2/T3 сигнатуры и обе sync entry points присутствуют; RED-наборы spot/futures и `DET-I-1` присутствуют; `verify_M-45.sh` синтаксически корректен и зелён для прежних задач. Его зелёность не засчитывается за задачу 7 по B-2.
- Диапазон правок `d77398d..c1ebac1` docs-only; текущий `origin/main` уже продвинулся до `af294528`. Merge-preview против текущего `origin/main` PASS, а audited base для предмета — их merge-base `d77398d`.

## FA

Диапазон не меняет `crates/**`, поэтому `check_review_fa.sh` дал бы `SKIP`; требование предъявлено когнитивно. Открытые живые инварианты: **VN-I-3** (`docs/fa/venues.md` §I: venue-specific ветвления остаются в адаптерах) и **BK-I-2** (`docs/fa/book.md` §6/§I: на gap `Stale` наступает синхронно до следующего события). B-1 не требует per-venue развязки именно потому, что `П-026` подписала общий результат двух существующих адаптеров; B-2 требует проверить этот результат на реальной конфигурационной границе.

## Done Block

```text
$ git rev-parse d77398d7b22396c452d2651e90498033186055dd c1ebac13a5ab0e53101351f9a6db3c757a40e690
d77398d7b22396c452d2651e90498033186055dd
c1ebac13a5ab0e53101351f9a6db3c757a40e690
exit=0

$ git diff --check d77398d..c1ebac1
exit=0

$ if rg -n '^[[:space:]]+(L2DELTA_CAPTURE_SYMBOLS|EPOCH_ID):' docker-compose.yml; then ...; else echo "compose_env_check=absent exit=$?"; fi
compose_env_check=absent exit=1

$ if rg -n 'docker compose|docker-compose|compose config|^[^#].*(L2DELTA_CAPTURE_SYMBOLS|EPOCH_ID)' scripts/verify_M-45.sh; then ...; else echo "task7_gate_check=absent exit=$?"; fi
task7_gate_check=absent exit=1

$ sed -n '10,14p' .claude/rules/scope-guard.md
engine-dev: crates/{journal,book,oms,sim,runner,alpha,portfolio,strategy,ops,gateway,gateway-serve,recorder}/src/** + deploy/**
exit=0

$ bash scripts/verify_M-45.sh
... T0–T9 PASS ...
VERDICT: PASS
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
... PASS checks ...
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --name-only d77398d..c1ebac1 | awk '/^(crates\/.*\/tests\/|scripts\/verify_M-45\.sh|contracts\/|crates\/contracts\/)/ {print}'
exit=0  # новая задача 7 не получила RED/verify/T-contract artefact в этом диапазоне

$ git status --porcelain=v1
<empty before verdict creation>
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31T18:51Z
- Milestone: M-45-persist-l2delta
- Статус: BLOCKED
- HEAD: c1ebac1 — docs(M-45): R-162 §5 — закрыт весь объявленный остаток Н-1/Н-3/Н-4/Н-5/Н-6 [architect]

## §B — Что я сделал
- Аудировал закоммиченный набор `d77398d..c1ebac1`, включая `П-026`, E-002, M-45, FA venues/book, DESIGN §17 и R-159…R-162.
- Проверил T1/T2/T3, RED-наборы, verify, role scope, config boundary и current-main merge-preview.

## §C — Артефакты / результаты
- `research/critiques/C-195-m45-rollout-signature.md`
- Done Block: `verify_M-45.sh` exit=0; `verify_design_claims --merge-preview origin/main` exit=0; task-7 config check exit=1; task-7 verify coverage check exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь REJECT C-195 на ветке docs/M-45-rollout-signature. Не меняй П-026: судится форма спеки. До dispatch задачи 7 устрани оба блокера: (1) её роль и путь должны быть совместимы с .claude/rules/scope-guard.md; (2) закоммить RED/verify-оракул на task 7, который красный на нынешнем compose и проверяет реальную конфигурационную границу. Сохрани R-162 §5, затем запушь и запроси новый critic-круг.
  ```
- Push-статус: ✅ audited subject head already pushed to origin/docs/M-45-rollout-signature at c1ebac1; C-195 is committed and pushed immediately after this handoff is written.
- Кэш: ✅ кэш worktree не создавался отдельно от shared compilation cache.

## §E — Риски / открытые вопросы
- Dev не назначать на задачу 7 до снятия B-1/B-2.
- `П-026` подписывает общий spot+futures эффект; per-venue ключ не проектировать как самовольный обход этой подписи.

=== END HANDOFF ===
