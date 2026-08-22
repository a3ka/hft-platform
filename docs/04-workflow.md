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
| **engine-dev** | кодовая дешёвая | `crates/{journal,book,oms,sim,runner,alpha,portfolio,strategy,ops,gateway,gateway-serve}/src` + `deploy/**` (ops/деплой-механика) | impl движка + наблюдаемость (`ops`, MD-only) + viz-backend (`gateway` M-22 read-only, `gateway-serve` M-28 WS-транспорт) по milestone'у + RED. Авторитет по зонам — `.claude/rules/scope-guard.md` |
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

### Close-out: гейт закрытого милестоуна уезжает в архив вместе со спекой (`COGNITIVE-ONLY`)

**`COGNITIVE-ONLY` — но ТОЛЬКО в части ТРИГГЕРА, и разница существенна** (`R-105`).
Тег обязателен по BINDING-правилу «обязывающее правило живёт вместе с механизмом»
(founder 2026-08-17), называющему `docs/04-workflow.md` поимённо. Что именно им покрыто:

- **триггер — когнитивен по существу.** Барьер обязан был бы знать, ЗАКРЫТ ли милестоун, а
  это машинно не выводится: `R-098` §3.1 — три ревизии критерия по прозе
  `PROJECT-STATE.md`, три разных ответа. Закрытость есть СУЖДЕНИЕ reviewer'а, и барьер,
  притворяющийся, что умеет его вычислить, хуже отсутствующего.
- **требования 2 и 3 — механизируемы, и механизм НАЗВАН КАНДИДАТОМ, а не изображён
  сделанным.** Условие «ни одной живой ссылки на вынесенный гейт» проверяется грепом:
  для каждого `docs/archive/verify_M-NN.sh` не должно существовать упоминания
  `verify_M-NN` вне `docs/archive/**`. Класс инцидента уже случился и записан — `TD-138`
  (документ обосновывает инвариант механизмом, которого нет). Почему не построен здесь:
  новый барьер под замораживающим `П-017` A2, и до ПЕРВОГО переезда охранять ему нечего —
  сегодня в `docs/archive/` один гейт. Строить его следует тем же кругом, что и первый
  вынос, а не раньше и не позже.

**Решение founder'а 2026-08-22 (Р-2).** На close-out гейт закрытого милестоуна уезжает в
`docs/archive/` вместе со спекой. **Роли разведены по зонам, потому что одна роль этого
сделать не вправе** (`R-104` Б-1):

| шаг | кто | основание зоны |
|---|---|---|
| назвать, какие милестоуны ЗАКРЫТЫ | **reviewer** | `PROJECT-STATE.md` — его зона; закрытость есть его суждение |
| `git mv` гейта и спеки | **architect** | `scripts/verify_*.sh` — architect-only sacred; `milestones/*.md` — architect-only |

Прежняя редакция этой нормы называла исполнителем переезда reviewer'а — **ошибка автора,
вставка сверх подписанного текста**: решение Р-2 сформулировано безлично («переезжает»), а
reviewer'у `scope-guard.md` §SACRED и `.claude/agents/reviewer.md` §NEVER writes закрывают
ОБА пути. Норма, исполненная буквально, порождала бы `SCOPE VIOLATION` на каждом close-out. Замер, из-за которого
норма введена: **45 гейтов, 6 183 строки, из CI не зовётся НИ ОДИН** (`ci.yml`,
`deploy.yml`, `branch-build.yml` — по нулю совпадений `verify_M-`). Гейт нужен В МОМЕНТ
ПРИЁМКИ; после merge его не исполняет никто, а весит он треть харнесса. Потерь нет: git
помнит всё, приёмка уже состоялась. Список к первому проходу — `R-098` §3: **30 в архив
(3 789 строк), 15 держать**; закрытость милестоуна выносит reviewer, она не выводится
грепом по прозе (`R-098` §3.1, три ревизии машинного критерия дали три разных ответа).

**Коммит переезда несёт ДВА токена, не один.** Пара «гейт + спека» пересекает два барьера:

| путь | барьер | токен |
|---|---|---|
| `scripts/verify_M-NN.sh` | `check_gate_meta.sh` — числит `scripts/verify_*.sh` гейт-классом, запертым проходным вердиктом | `ARCHIVED-VERDICT: <путь> — <причина>` |
| `milestones/M-NN-*.md` | `check_protected_artifacts.sh` — `milestones/*.md` защищён, `docs/archive/*` в перечень НЕ входит | `ALLOW-ARTIFACT-DELETE: <причина>` |

Прецеденты `4693688` и `31c2626` этого не показывают и показать не могли: там материал
приходил с мёртвых веток, на `main` файлов не было, удалять было нечего — **прецедент есть
у формы врезки, но не у прохождения гейтов** (`R-098` §4.1).

**Три требования к коммиту переезда — каждое закрывает названный регресс:**

1. **Универсум номеров не сужается.** `check_artifact_ids.sh` и `next_artifact_id.sh`
   считают занятость по белому списку путей, и `docs/archive/**` в нём нет: вынесенная
   спека перестаёт занимать свой номер, а `gates.md` §12 объявляет идентификатор
   УНИКАЛЬНЫМ. Перед ПЕРВЫМ переездом в белый список добавляется `docs/archive/M-*.md`
   (правка существующего барьера, харнесс-трек). Пропустить этот шаг и «назвать
   ослабление» — тоже допустимо, но тогда ослабление пишется в `TECH-DEBT.md`, а не
   умалчивается (`R-098` §4.2).
2. **Гейт, на который ссылается ЧУЖАЯ зона, НЕ выносится, пока ссылку не снял её владелец.**
   Замер по `crates/**` и `deploy/**` (которого в первой редакции Р-2 не было): ссылки несут
   **восемь** из 30 кандидатов, но зоны у них РАЗНЫЕ, и это решает, кто вынос делает:
   - `M-17 M-22 M-50 M-62` — ссылки живут ТОЛЬКО в `*/tests/**`, то есть в sacred-зоне
     architect'а. Он правит их сам, тем же коммитом. **Выносятся в первом проходе.**
   - `M-35 M-45 M-48 M-49` — ссылки есть в `crates/*/src/**` (architect не пишет impl-код)
     и `deploy/**` (зона engine-dev). Коммит не собирается одной ролью ⇒ **отложены** до
     снятия ссылки владельцем зоны.
   Первый проход — **26 гейтов из 30**. Прежняя редакция говорила «22 из 30», считая все
   восемь чужими: ошибка автора, половина ссылок лежит в его собственной зоне (`R-105`).
   Худшая из отложенных: `deploy/bin/gateway-checkpoint-cron.sh:39,78`, где `verify_M-48`
   объявлен машинной сверкой КОМПОЗИЦИИ путей прод-cron'а. Вынести его, оставив строку, —
   произвести «built-not-wired» наоборот: контракт заявлен, проверяющего нет; тот же класс,
   что `TD-138` (`R-098` §4.3).
3. **Гейт, на который ссылается ЖИВОЙ барьер, не выносится, пока ссылка жива.** Сегодня
   это `M-60b` (`check_context_budgets.sh:19`).

**Норма не ретроспективна сама по себе:** она останавливает РОСТ. Разовый вынос — отдельная
работа по списку `R-098` §3 под теми же тремя требованиями, и в первом проходе он берёт
**26 гейтов из 30**: четыре (`M-35 M-45 M-48 M-49`) отложены до снятия ссылок
владельцами чужих зон.

**Харнесс идёт ДРУГИМ маршрутом (решение founder'а 2026-08-15).** Цикл выше рассчитан на
код, исполняемый прод-процессом. Для инструментов, которыми мы проверяем себя, —
`scripts/check_*.sh`, `scripts/tests/red_*.sh`, `scripts/verify_M-*.sh`, репо-тулинг,
`.github/workflows/**` — действует облегчённый **харнесс-трек**: `docs/workflow/harness-track.md`.
Там же названа граница: всё, что исполняется на проде, трогает `contracts`/`risk`/`killswitch`/
`oms`/`venue-*`, меняет раскладку журнала или НОРМЫ (а не механизмы), идёт полным циклом §2
без исключений. Замер, из-за которого трек введён: за неделю 234 коммита в `main`, из них 17
тронули `crates/**`; на M-60 из четырёх кругов гейта ТРИ нашли дефекты в самих оракулах, а не
в предмете.

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

> ⚠ **УСТАРЕЛО как план (2026-07-14).** Таблица ниже — ИСТОРИЧЕСКАЯ (нумерация M-05/M-06 здесь
> значит НЕ то, что реально сделано под этими номерами). Актуальная очередь и гейты —
> **`milestones/BACKLOG.md`** (источник) + `PROJECT-STATE.md` (что уже реализовано). Оставлено
> как образец того, КАК формулируются гейты, а не КУДА мы идём.

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
