# 04 — Workflow постройки проекта (по образцу EINHARD operating model)

STATUS: DESIGN v1 (2026-07-10, Fable). Это «как мы строим платформу» — процессный слой,
аналог EINHARD `.claude/` (CLAUDE.md + rules + agent-профили). Отвечает на «хочу весь
воркфлоу создания проекта как в EINHARD»: роли, milestone-жизненный-цикл, гейты
(plan-time + PR-time + RED-first + scope-guard), handoff, коммит-дисциплина — плюс
торговые надстройки (риск-инварианты sacred; путь к деньгам через подписи).

Принцип №0 (как в EINHARD): **founder = orchestration dispatcher.** Агенты не вызывают
друг друга напрямую; founder диспетчеризует по handoff-блокам. Fable-дорого → Fable
только архитектура/спеки/критические решения; рутина — дешёвые модели.

---

## §1. Роли и модель маршрутизации

| Роль | Модель-класс | Зона записи | Что делает |
|---|---|---|---|
| **architect** | Fable (дорого — экономно) | `docs/`, `contracts/` типы, `*/tests/` (RED-спеки), `scripts/verify_*.sh`, milestone-файлы | архитектура, T-контракты, RED-тесты, acceptance-скрипты, milestone'ы. **НИКОГДА не пишет impl-код** |
| **critic** | средняя (Codex/дешёвая) | `research/critiques/`, verdict-файлы (не milestone'ы) | plan-time гейт: аудит закоммиченного набора артефактов milestone'а ДО dev |
| **engine-dev** | кодовая дешёвая | `crates/{journal,book,oms,sim,runner,alpha,portfolio,strategy}/src` | impl движка по milestone'у + RED-тестам |
| **venue-dev** | кодовая дешёвая | `crates/venue-*/src` | адаптеры бирж (HL→Binance) |
| **signal-engineer** | кодовая средняя | `crates/signals/src`, `research/specs` | сигналы (SignalSpec→код) — квант-деск, граница A |
| **research-dev** | дешёвая | `crates/research-cli/src` | грид/walk-forward/отчёты |
| **risk-critic** | **сильная (не экономим)** | `research/critiques/` | адверсарий: safety-путь движка + отчёты стратегий (asymmetric cost) |
| **reviewer** | сильная | `PROJECT-STATE.md`, `TECH-DEBT.md`, PR-комменты | PR-time гейт: scope, Done Block, contract governance, риск-инварианты |
| **tester** | дешёвая | — (read-only на код) | прогон RED+acceptance на чистом чекауте |
| **founder** | человек | подписи (Ed25519), приоритеты | единственная подпись на промоушенах/весах/live |

Маршрут: architect авторит milestone → (critic-гейт если триггер) → founder диспетчеризует
dev → tester → reviewer → merge. Точно как EINHARD, но роли под трейдинг.

## §2. Жизненный цикл milestone'а

Milestone-файл `milestones/M-NN-<name>.md` (шаблон — §6), содержит: Objective, Allowed/Forbidden
paths, §Tasks (со Status ⏳/🚧/✅), Contract impact (T1? → contract-RFC), RED-тесты (пути),
Acceptance-скрипт, Handoff-блок.

```
architect: пишет milestone + RED-тесты (FAIL) + acceptance-скрипт + T-контракты
   → КОММИТИТ всё ДО dev (Tests-as-spec: код нельзя писать без падающего теста)
   → [critic-гейт, если сработал триггер §3]
   → founder диспетчеризует dev по Handoff §D
dev: реализует по контракту → RED→GREEN → acceptance exit=0 → Done Block
   → founder диспетчеризует tester
tester: прогон на чистом чекауте → PASS/FAIL verdict
reviewer: scope + Done Block + contract Block-C + риск-инварианты → APPROVED
   → merge → reviewer обновляет PROJECT-STATE.md + TECH-DEBT.md
```

## §3. Гейты

**Plan-time (critic) — триггеры (иначе architect→dev напрямую):** milestone трогает
`contracts/` (T1); ИЛИ `risk`/`killswitch`/`oms`/`venue-*` (safety/деньги); ИЛИ ≥5
атомарных коммитов; ИЛИ новый крейт; ИЛИ ломающее изменение формы. Низкорисковые
(один сигнал, отчёт, docs) — без critic'а, ревьюер — бэкстоп.

**RED-first (TDD) — обязательно везде:** architect пишет падающий тест ПЕРВЫМ; тест =
спецификация; dev делает GREEN. Тест GREEN против заглушки = дефект (анти-плацебо, урок
hft-core-rs). Тесты — sacred: dev их не меняет (нашёл ошибку в тесте → SCOPE VIOLATION
REQUEST, не «правка»).

**Scope-guard:** таблица §1 = закон. Квант-агенты пишут только `crates/signals` +
`research`. `risk`/`killswitch` тесты (RK-I-*) и `contracts` T1 — sacred, трогает только
architect. Выход за зону → `!!! SCOPE VIOLATION REQUEST !!!` → стоп + эскалация.

**Acceptance-script-as-real-gate:** каждый milestone — `scripts/verify_M-NN.sh` с
`set -euo pipefail`, exit≠0 на любом FAIL; ≥1 проверка на задачу; никакого
`cmd && echo PASS || echo FAIL`.

**PR-time (reviewer) — UNCONDITIONAL для всего, что трогает код/контракты/риск.**
Блоки: scope; Done Block (сырой stdout, не пересказ); contract Block-C (правки T1 только
через RFC); **риск-блок** — любой milestone на `risk`/`killswitch`/`oms`/`venue`
ОБЯЗАТЕЛЬНО проходит `risk-critic` (аналог EINHARD security-reviewer — safety-findings
имеют асимметричную цену).

**Торговые надстройки поверх EINHARD-гейтов:**
- Риск-инвариантный RED-suite (RK-I-1..10, INTG-I-*, CT-I-*) — sacred, зелёный до любого
  merge'а в `risk`/`oms`/`venues`/`contracts`.
- Backtest-отчёты → анти-оверфит-гейт (02 §4): пре-регистрация, trials-ledger, критик.
- Путь к деньгам (candidate→paper→live, testnet→live-micro) — только через подпись
  founder'а (HDP-очередь, граница C). Ни один агент не двигает деньги.

## §4. Коммит-дисциплина

- **Атомарные коммиты**: одна задача = ≥1 коммит; `type(M-NN): task #k — <...>`. Бандл на
  5 задач = авто-reject.
- **Done Block** перед «готово»: сырой stdout `git status`, тестов, acceptance, exit-кодов.
  Никаких пересказов.
- **Handoff-блок** — последняя секция каждого ответа агента (§6): §A метаданные, §B что
  сделал, §C артефакты, §D следующий агент + paste-ready промпт, §E риски.
- Идентичность коммиттера = роль (аудит трейла).

## §5. Директория процесса (что заводим в репо)

```
platform/
├── DESIGN.md, docs/00..05           # архитектура (этот набор)
├── PROJECT-STATE.md                 # что реализовано (пишет ТОЛЬКО reviewer)
├── TECH-DEBT.md                     # открытый долг (пишет ТОЛЬКО reviewer)
├── milestones/M-NN-*.md             # milestone-контракты (пишет architect)
├── process/
│   ├── roles.md                     # §1 таблица + зоны (scope-guard)
│   ├── gates.md                     # §3 гейты (plan/RED/scope/acceptance/PR/risk)
│   ├── milestone-template.md        # §6
│   └── handoff-template.md          # §6
└── agents/                          # профили дешёвых dev-агентов (по мере надобности)
```

Аналогия EINHARD прямая: `DESIGN.md`≈CLAUDE.md, `process/`≈`.claude/rules/`,
`agents/`≈`.claude/agents/`, `milestones/`≈`docs/milestones/`, `contracts/`≈`contracts/`.

## §6. Шаблоны (кратко; полные — в `process/`)

**Milestone:** Objective · Allowed/Forbidden paths · Contract impact (T1?→RFC) · §Tasks
(#, Status, task, verify, files) · RED-тесты (пути) · Acceptance-скрипт · Handoff.

**Handoff:** `=== HANDOFF: <FROM>→<TO> ===` §A метаданные (дата, milestone, статус, HEAD)
§B что сделал §C артефакты/exit-коды §D следующий агент + **paste-ready промпт** §E риски
`=== END HANDOFF ===`.

## §7. Первые milestone'ы (порядок постройки)

| M | Название | Гейты |
|---|---|---|
| M-00 | Bootstrap: workspace + `contracts` крейт + verify-каркас + process/ файлы | critic (новый контракт-слой) + reviewer |
| M-01 | Журнал P0 (Event, append-log, replay, state_hash) + DET-I-1 RED | critic + reviewer |
| M-02 | venue-hyperliquid recorder (только чтение MD → журнал) | reviewer (venue → risk-блок мягкий: read-only) |
| M-03 | book builder + ресинк + depth-полосы | reviewer |
| M-04 | sim fill-model + research-cli + OBI-сигнал №1 (граница A) | critic + risk-critic (отчёт) + founder ★ |
| M-05 | risk gate + killswitch + oms + RK-I-1..10 RED | critic + **risk-critic** + reviewer |
| M-06 | live-micro runner + профиль лимитов + testnet drill | risk-critic + founder ★★ |

Fable авторит M-00/M-01/M-04/M-05 (архитектура/контракты/RED); dev-агенты реализуют;
Fable подключается на спорных verdict'ах и воротах фаз.
