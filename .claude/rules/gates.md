# Гейты — plan-time, RED-first, PR-time, риск, анти-оверфит

Источник: `docs/04-workflow.md` §3 + `docs/DESIGN.md` §4/§6/§7 + `docs/05-contract-layer.md`
§4 + `docs/02-quant-desk.md` §4. Каждый milestone проходит через применимые гейты ниже
ДО close-out. Пропуск гейта = находка reviewer'а, откат close-out.

## 1. Plan-time (critic) — триггеры

Architect коммитит milestone + RED-тесты + verify-скрипт + T-контракты **ДО** dev.
Critic обязателен, если сработал ЛЮБОЙ триггер:

1. Milestone трогает `crates/contracts/**` (T1) — всегда contract-RFC + critic.
2. Milestone трогает `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`,
   `crates/venue-*/**` (safety/деньги) — RISK-BLOCK, см. §5.
3. Оценка ≥5 атомарных коммитов.
4. Новый крейт вводится.
5. Ломающее изменение формы T1 (см. `docs/05-contract-layer.md` §4).

Низкорисковые milestone'ы (один сигнал в `crates/signals/`, отчёт, docs-правка) — без
critic'а; reviewer — бэкстоп на PR-time (гейт 4 ниже всё равно UNCONDITIONAL).

## 2. RED-first (TDD) — обязательно везде, без исключений

- Architect пишет ПАДАЮЩИЙ тест ПЕРВЫМ. Тест — спецификация, не проверка постфактум.
- Implementation-код без предшествующего RED-теста не пишется НИКЕМ.
- **Анти-плацебо (урок hft-core-rs):** тест, который проходит GREEN против заглушки/
  no-op-стаба, — дефект теста, не готовая фича. Пример: тест `RiskApproved` не должен
  проходить, если risk-gate всегда возвращает `Approve` независимо от входа.
- Каждая публичная функция — с тестом. Риск-инварианты (`RK-I-1..10`,
  `docs/DESIGN.md` §4) и `DET-I-1` (бит-идентичный replay, `docs/DESIGN.md` §1) —
  sacred RED-оракулы: они обязаны падать на заглушке ДО реализации.
- Прогон: `cargo test -p <crate>` (или `cargo test --workspace` для сквозных).

## 3. Acceptance-script-as-real-gate

Каждый milestone — `scripts/verify_M-NN.sh`:

- `set -euo pipefail` ИЛИ явный агрегатор с FAIL-счётчиком + `exit 1` при FAIL>0.
- **Никакого** `cmd && echo PASS || echo FAIL` (маскирует провал).
- Минимум 1 проверка на задачу из §Tasks milestone'а.
- Финальная строка `VERDICT: PASS`/`VERDICT: FAIL`; exit-код соответствует.
- Явный список исключений (T2/T3 типы вне зоны проверки) с комментарием-обоснованием.

Dev перед "готово" запускает `bash scripts/verify_M-NN.sh; echo "exit=$?"` — сырой вывод
идёт в Done Block (`.claude/rules/commit-discipline.md`).

## 4. PR-time (reviewer) — UNCONDITIONAL

Reviewer обязателен для ЛЮБОГО milestone'а, тронувшего код/контракты/риск/докс сверх
tech-debt правок. Блоки:

- **Scope** — диф соответствует `Allowed paths` milestone'а (`.claude/rules/scope-guard.md`).
- **Done Block** — сырой stdout, не пересказ (`.claude/rules/commit-discipline.md`).
- **Contract Block-C** (аналог EINHARD Block B5) — правки `crates/contracts/**` вне
  contract-RFC → авто-REJECT (`docs/05-contract-layer.md` §4).
- **Риск-инварианты** — если milestone трогал `risk`/`killswitch`/`oms`/`venue-*`,
  reviewer проверяет наличие GREEN RED-suite `RK-I-1..10` + `INTG-I-*` в Done Block.

Reviewer не пропускается НИКОГДА для substantive-изменений. После APPROVED reviewer
обновляет `PROJECT-STATE.md` + `TECH-DEBT.md` и делает push (аналог EINHARD F-032 J5).

**Граница reviewer↔architect (закреплено 2026-07-11, инцидент TD-011):** reviewer ОПИСЫВАЕТ
дефект (что/где/симптом/воспроизведение), но НЕ проектирует фикс. Дизайн решения
(напр. tail-scan/seek) + RED-оракул на регресс — зона architect (RED-first). Reviewer
находит проблему → architect проектирует защиту → dev реализует.

## 5. RISK-BLOCK — асимметричная цена ошибки

Любой milestone, трогающий `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`,
`crates/venue-*/**`, ИЛИ контракты (`crates/contracts/**`) → **обязательный risk-critic**
(сильная модель, не экономим — `docs/02-quant-desk.md` §1, `docs/DESIGN.md` §7). Причина:
ложноположительный risk-gate или пропуск в oms стоит депозита — асимметрия хуже, чем
лишний цикл ревью.

**MD-only carve-out (закреплено 2026-07-11, M-06):** venue-* адаптер, читающий ТОЛЬКО
рыночные данные (WS/REST → нормализация → `MdEvent`, БЕЗ order-egress — никаких
submit/cancel/подписи торговых действий), НЕ требует risk-critic — достаточно reviewer.
risk-critic ОБЯЗАТЕЛЕН, как только venue-* код трогает ORDER-путь (submit/cancel/auth
торговли). Причина: асимметричная цена ошибки живёт на пути к деньгам (ордера), а не на
read-only MD-приёмнике. Reviewer в Block-scope подтверждает, что диф действительно MD-only
(нет order-egress) — иначе RISK-BLOCK применяется полностью.

- risk-critic пишет вердикт в `research/critiques/C-NNN.md` (KILL | CONCERNS | PASS).
- KILL/CONCERNS блокирует merge до устранения находок или явного founder-override
  с именованным обоснованием.
- Risk-инвариантный RED-suite (`RK-I-1..10`, `INTG-I-*`, `CT-I-*`) обязан быть GREEN
  (изначально падал на заглушках — анти-плацебо §2) до любого merge в
  `risk`/`oms`/`venues`/`contracts`.
- Read-only характер risk-critic: не предлагает стратегий (независимость от
  signal-engineer), не пишет код.

## 6. Анти-оверфит гейт — backtest-отчёты (`research/reports/R-NNN`)

Отчёт невалиден без:

1. **Пре-регистрация** критериев фальсификации ДО касания test-данных
   (карточка `research/hypotheses/H-YYYYMMDD-<slug>.md`, `docs/02-quant-desk.md` §3.1).
2. **Time-split**: train/validation/test; test-период трогается ОДИН раз на финальной
   валидации.
3. **Trials-ledger** (`research/trials-ledger.json`, append-only, `INTG-I-6`) — каждая
   ячейка грида инкрементирует счётчик; deflated Sharpe считается от него.
4. **Чек-лист risk-critic**: lookahead/leakage, survivorship, честные издержки+латентность
   (чувствительность ×1.5 издержек, ×2 латентности), режимная зависимость, реальная
   ёмкость, корреляция с уже-live сигналами.
5. **Paper-карантин обязателен** — ни один сигнал не идёт train→live напрямую
   (`docs/02-quant-desk.md` §4.5).
6. **Двойная подпись founder'а** (paper, live) через очередь решений — граница C
   (`docs/03-integration-contract.md` §3, `INTG-I-3`).

## 7. Путь к деньгам — единственная founder-подпись

Промоушен `candidate→paper→live` и любое изменение весов/лимитов проходит ТОЛЬКО через
подписанный `Ctl(ParamChange)` (`RK-I-10`/`INTG-I-3`) или `decisions/D-NNN.md`
(`INTG-I-2`). Ни один агент — включая architect — не двигает деньги и не меняет
live-параметры напрямую. Байпас-поверхности не существует (`RK-I-2`) — это не
конфигурируемая опция, а архитектурный факт, проверяемый тестом.

## 8. Post-merge деплой-гейт — прод живёт на ДРУГОМ сервере (добавлено 2026-07-11)

Разработка идёт локально (`/home/nous/hft-platform`), но проект РАБОТАЕТ на VPS
(Hetzner, доступ и схема — `docs/SESSION-HANDOFF.md` §2): push в `main` триггерит
GitHub Actions (`ci.yml` + `deploy.yml` → build на VPS → healthcheck → rollback).
Push — НЕ конец цикла. Агент, сделавший push (reviewer в конце milestone-цикла;
architect для docs/process-only), ОБЯЗАН до закрытия работы:

1. **Дождаться CI + Deploy** до терминального статуса: `gh run watch <id> --exit-status`
   (или `gh run list` до `completed success` обоих). Красный CI/Deploy → немедленный
   фикс или revert; milestone НЕ закрывается поверх красного прода.
2. **Проверить прод на VPS** (минимум):
   `ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 \
      'docker ps --format "{{.Names}} {{.Status}}"; cat /var/lib/docker/volumes/hft-platform_journal-data/_data/recorder.heartbeat'`
   Ожидание: контейнер(ы) `(healthy)`, heartbeat свежий (мс epoch ≈ сейчас), журнал
   растёт. Если деплой менял поведение данных (парсеры/форматы) — sanity свежих
   событий по мере возможности.
3. **Пруф в close-out**: сырые строки `gh run list` (success) + вывод ssh-проверки —
   в Done Block / §C close-out отчёта. «Запушил и ушёл» = нарушение гейта.

Rollback в `deploy.yml` — страховка, не оправдание: он ловит падение healthcheck,
но не тихую деградацию (контейнер жив, данные испорчены). Глаза обязательны.

### Push-scope дисциплина (инцидент 2026-07-11: чужие RED-коммиты уехали с чужим push)

Основной чекаут ОБЩИЙ для сессий. Перед КАЖДЫМ `git push` — обязательная проверка:
`git log origin/main..HEAD --format='%h %s'` — в списке ТОЛЬКО твои коммиты этой
задачи. Чужие/незнакомые коммиты (другая сессия закоммитила локально и намеренно
НЕ пушила) → СТОП, не пушь, согласуй с founder'ом.

### RED-оракулы до реализации НЕ живут на main (main всегда зелёный)

Compile-RED/раннинг-RED тесты до реализации ломают CI. Два санкционированных пути:
(а) держать RED-коммиты локально до GREEN (паттерн M-04); (б) feat-ветка:
architect пушит `feat/M-NN`, dev-агенты бутстрапятся `pi-<role> --branch feat/M-NN`,
reviewer мержит в main уже GREEN. Прямой push RED-состояния в main = нарушение
этого гейта (кроме явного founder-override).

**Intra-chain push на feat-ветку (закреплено 2026-07-12, инцидент TD-014 chain-break):**
в цепочке на общей feat-ветке (architect→venue/engine-dev→...→reviewer) КАЖДЫЙ dev в
цепочке ПУШИТ свой GREEN-коммит на shared `feat/M-NN` (ff), чтобы следующий агент
забутстрапился на актуальном состоянии. Правило «push — только reviewer после APPROVED»
относится к **main** (merge `feat→main` + §8 деплой-гейт), НЕ к intra-chain push на feat.
Мандат dev-агенту должен явно РАЗРЕШАТЬ push на feat-ветку (иначе коммит виснет в
worktree-ветке, цепочка рвётся — TD-014). Push-scope (`git log origin/feat/M-NN..HEAD` =
только твои коммиты) обязателен по-прежнему.

## Cross-references

- `.claude/rules/scope-guard.md` — зоны, за нарушение которых гейт 4 реджектит
- `.claude/rules/testing.md` — детали RED-first дисциплины
- `.claude/rules/commit-discipline.md` — Done Block формат
- `docs/DESIGN.md` §4 (RK-I-*), §5 (честность симулятора), §6 (границы A/B/C), §7 (анти-оверфит)
- `docs/05-contract-layer.md` §4 (contract-RFC), §6 (CT-I-*)
- `docs/03-integration-contract.md` §6 (INTG-I-*)
