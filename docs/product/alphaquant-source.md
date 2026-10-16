# AlphaQuant — описание продукта (источник, версия 1.0, июль 2026)

> Внесено в репозиторий по находке C-041 Ф2: разделы §14–§20 `DESIGN.md` ссылались на
> `/tmp/alphaquant.txt` — файл ВНЕ репозитория. `/tmp` очищается, и обоснование продуктовых
> решений исчезло бы вместе с ним. Извлечено из `tmp/AlphaQuant.docx` (founder).

●
AlphaQuant
Описание продукта, функционала, конкурентного преимущества, AI-native и B2A-архитектуры
Версия 1.0 • июль 2026
Рабочий документ для продуктовой, технической и дизайн-команд
Содержание
1. Executive summary
2. Проблема рынка и продуктовая возможность
3. Видение и позиционирование
4. Пользователи и ключевые сценарии
5. Продуктовая архитектура
6. Библиотека рабочих пространств
7. AI-native система
8. Обучение и AI Tutor
9. Order-flow и исполнение
10. Опционный терминал
11. Quant Research и личные агенты
12. B2A-платформа
13. Данные и технический moat
14. Конкурентная карта
15. Бизнес-модель
16. Roadmap и приоритеты
17. Риски и ограничения
18. KPI и критерии успеха
19. Итоговая продуктовая формула
20. Источники
1. Executive summary
ОПРЕДЕЛЕНИЕ ПРОДУКТА  AlphaQuant — единый адаптивный криптотрейдинговый терминал, в котором человек и его AI-агенты видят один рынок, работают с одной системой данных, используют общий журнал, risk engine и execution layer. Продукт превращает order flow и деривативные данные в объяснимые гипотезы, тесты, правила, агентов и контролируемые торговые действия.
Основная сущность — Workspace: сохранённая рабочая среда под конкретную задачу, а не жёсткий тип пользователя.
Первые ключевые Workspace — Beginner Learning и Scalper Execution; далее Swing Context, Position Regime, Options Trading и Quant Research.
AI не является отдельным чат-ботом. Он действует как Tutor, Market Copilot, Risk Guardian, Options Analyst или Research Agent в зависимости от контекста.
Ключевая цепочка продукта: Event → Explanation → Hypothesis → Test → Rule → Agent → Execution → Review.
Главный moat формируется вокруг нормализованных данных микроструктуры, исторического event-level replay, deterministic event engine, пользовательской памяти и безопасного agent/execution layer.
2. Проблема рынка и продуктовая возможность
Профессиональные order-flow терминалы глубоко показывают ликвидность, DOM, footprint и историческое поведение книги заявок, но требуют высокой квалификации и редко объясняют пользователю причинно-следственную связь.
Универсальные charting-платформы удобны для layouts, синхронизации и индикаторов, но не построены вокруг криптовалютной микроструктуры, AI-обучения и agentic workflow.
Quant-платформы связывают research, backtest, paper и live, однако обычно ориентированы на код, а не на визуальный order-flow и пользователя, который формулирует идею естественным языком.
Опционные терминалы чаще разделяют chain, IV, payoff и underlying; рынок ещё слабо закрывает синхронизированный анализ option flow, perp positioning и order flow базового актива.
Большинство AI-функций в трейдинге остаются текстовыми помощниками или генераторами сигналов. Они не видят выбранный участок графика, не управляют контекстными виджетами и не проводят гипотезу через полный цикл исследования и исполнения.
ВОЗМОЖНОСТЬ  Рыночная возможность — создать не ещё один график и не ещё один AI-чат, а операционную систему для совместной работы трейдера и его агентов с криптовалютной микроструктурой.
3. Видение и позиционирование
3.1. Продуктовое обещание
ПОЗИЦИОНИРОВАНИЕ  AlphaQuant превращает ликвидность, поток ордеров, деривативы и волатильность в понятные, проверяемые и при необходимости исполнимые торговые решения.
3.2. Рабочий цикл
Этап
| Что делает пользователь
| Что делает система
Observe
| Наблюдает цену, ликвидность и поток сделок
| Собирает и нормализует market data, выделяет события
Understand
| Пытается понять причину движения
| AI объясняет evidence, альтернативы, confirmation и invalidation
Test
| Проверяет гипотезу
| Replay, paper decision, backtest, similarity search
Act
| Создаёт alert, ticket, правило или агента
| Risk check, fill preview, permissions и approval
Review
| Сравнивает план и результат
| Journal, process score, live-vs-model reconciliation, decay monitoring
4. Пользователи и ключевые сценарии
Сегмент
| Главная задача
| Основной AI
| Критические модули
Новичок / ученик
| Научиться читать реальный рынок без перегрузки
| AI Tutor
| Упрощённый график, replay, hypothesis, paper decision, journal
Скальпер
| Быстро заметить изменение ликвидности и исполнить план
| Market Events Copilot
| Heatmap/footprint, DOM, tape, ticket, risk strip
Swing-трейдер
| Связать локальный flow с контекстом часов и дней
| Context Analyst
| Multi-TF, VWAP/profile, OI, funding, basis, liquidations
Позиционный трейдер
| Понять режим и портфельный риск
| Regime & Risk Analyst
| Multi-asset, volatility, correlations, term structure, exposure
Опционный трейдер
| Понять цену опциона, IV, Greeks и flow
| Options Analyst
| Chain, IV surface, flow, Greeks, payoff, P&L attribution
Quant / Algo
| Формализовать, проверить и развернуть идею
| Research Agent
| Spec, data catalog, chart signals, backtest, validation, deployment
B2A-клиент
| Дать личному агенту данные, инструменты и безопасное исполнение
| Agent Runtime
| Tool API, memory, policies, approvals, audit
5. Продуктовая архитектура
5.1. Сущности продукта
Сущность
| Назначение
| Что не должна означать
Profile
| Предпочтения, опыт, риск, рынки, язык AI; формируется в onboarding
| Не является ежедневной навигацией
Workspace
| Полная рабочая среда под задачу
| Не является типом личности
View
| Вариант компоновки внутри Workspace
| Не дублирует Workspace
Widget
| Отдельный функциональный модуль
| Не отдельная страница без необходимости
Replay
| Режим источника времени и данных внутри Workspace
| Не отдельный мини-продукт
Journal
| Глобальная память решений, сделок и AI-разборов
| Не только список сделок
Agent
| Долгоживущий процесс с goal, tools, memory и policy
| Не одноразовый prompt
5.2. Общая оболочка
Компактный topbar: Workspace, View, Live/Replay, instrument, timeframe, alerts, Customize, Add Widget, account.
Минимальная глобальная навигация: Terminal, Journal, Settings; позднее — Automations/Agents.
Region-based layout: Main, Right, Bottom, Auxiliary. Полностью свободный grid добавляется после стабилизации графического ядра.
Workspace сохраняет instrument, timeframe, widgets, geometry, tabs/stacks, AI role и пользовательские настройки.
Существующие COB/SVP/CVP и CVD/BID-ASK/Delta сохраняются в Widget Registry; скрытие в шаблоне не означает удаление.
6. Библиотека рабочих пространств
Beginner Learning
Обучение непосредственно на текущем графике или replay.
РЕКОМЕНДУЕМАЯ КОМПОЗИЦИЯ  График 58–65%; AI Tutor 35–42% на всю высоту; снизу lesson, paper decision, journal и process review.
Scalper Execution
Скорость распознавания события и исполнения.
РЕКОМЕНДУЕМАЯ КОМПОЗИЦИЯ  Heatmap/footprint; DOM/COB; tape; fast ticket; AI events; CVD; positions/risk; kill switch.
Swing Context
Контекст нескольких часов и дней.
РЕКОМЕНДУЕМАЯ КОМПОЗИЦИЯ  Main chart + higher TF; VWAP/profile; OI/funding/basis; liquidations; Trade Thesis и scenarios.
Position Regime
Режим, портфель и риск.
РЕКОМЕНДУЕМАЯ КОМПОЗИЦИЯ  Multi-asset; volatility; correlations; term structure; exposure; AI daily/weekly narrative.
Options Trading
Анализ и торговля криптоопционами.
РЕКОМЕНДУЕМАЯ КОМПОЗИЦИЯ  Underlying order flow; smart chain; IV surface; option flow; Greeks; payoff; position risk.
Quant Research
Natural language → spec → test → validation → deployment.
РЕКОМЕНДУЕМАЯ КОМПОЗИЦИЯ  AI research copilot; spec/code; data/features; chart with fills; results, logs and deployment.
Custom
Пользовательская рабочая среда.
РЕКОМЕНДУЕМАЯ КОМПОЗИЦИЯ  Стартовые bases: chart only, chart + right rail, order-flow base, multi-pane, blank advanced.
7. AI-native система
7.1. Принцип
AI-NATIVE  Один Agent Runtime, разные роли и UI-контракты. AI должен не только отвечать, но и читать структурированный market state, вызывать инструменты терминала, менять фокус интерфейса и оставлять проверяемый audit trail.
7.2. Роли AI
Роль
| Функция
| Тип отображения
Market Observer
| Выделяет события микроструктуры и деривативов
| Chips, event rail, chart annotations
AI Tutor
| Обучает через вопросы и evidence
| Большая панель: Lesson / Ask / History
Trade Copilot
| Формирует thesis, ticket draft и execution plan
| Context card + order draft
Risk Guardian
| Проверяет risk limits и отклонение от плана
| Risk strip, warnings, approval
Journal Analyst
| Выявляет повторяющиеся ошибки и лучшие решения
| Session review и weekly digest
Options Analyst
| Объясняет IV, skew, Greeks и P&L
| Chain/surface annotations и decomposition
Research Agent
| Формализует идею и управляет research pipeline
| Spec, tool log, result review
Execution Agent
| Выполняет только разрешённые действия
| Approval queue и audit log
7.3. AI Event — единая модель
Единый eventId для chip, compact card и полного сообщения в AI feed.
Поля: timestamp, instrument, observation, interpretation, importance, horizon, supporting/conflicting evidence, confirmation, invalidation, data quality, chart annotations.
Единые действия: Show on chart, Ask AI, Pin, Mute similar, Create alert, Add to journal, Turn into rule.
Никаких необоснованных процентов confidence. Используются Low/Medium/High и количество evidence.
7.4. Contextual AI tools
Выделение участка графика → Explain, Compare, Create alert, Turn into rule, Add to journal.
Любой indicator widget → Explain this move.
DOM/COB → объяснение liquidity pull, replenishment, spread quality и adverse selection.
Order ticket → position sizing, fees/slippage, invalidation consistency, risk check.
Replay → автоматические key moments, quizzes и compressed review.
Journal → session summary, recurring mistakes, playbook performance, next exercises.
8. Обучение и AI Tutor
КЛЮЧЕВОЙ ПРИНЦИП  Урок не должен быть отдельной страницей школы. Это состояние Beginner Workspace, в котором Tutor напрямую видит текущий график, выбранный объект, replay time и историю пользователя.
8.1. Панель Tutor
Lesson: структурированный сценарий, hypothesis, evidence, paper decision, review.
Ask: полноценный диалог текстом или голосом с текущим market context.
History: прошлые вопросы, ошибки, объяснения и прогресс.
Tutor может подсветить уровень, открыть CVD/footprint, поставить replay на паузу, сравнить аналогичный эпизод и создать journal note.
Guided Replay по умолчанию; Live Tutor доступен в том же Workspace и не использует знание будущего.
8.2. Учебный цикл
Система замечает или выбирает учебное событие.
Пользователь формулирует гипотезу до объяснения.
Tutor показывает evidence и competing interpretation на графике.
Пользователь выбирает Wait / Skip / Paper trade.
После развития события система оценивает reasoning и соблюдение риска, а не только outcome.
Результат сохраняется в Journal и влияет на персональный curriculum.
9. Order-flow и исполнение
9.1. Сохраняемое профессиональное ядро
Группа
| Компоненты
Главный график
| Candles, heatmap, footprint, bid/ask, delta, liquidity layers, bubbles, VWAP, POC, value area, HVN/LVN
Правая вертикаль
| DOM/COB, SVP, CVP
Нижние панели
| CVD, BID/ASK, Delta, Add Pane и pane controls
Торговля
| Fast ticket, ticket-at-price, positions, orders, execution report, risk center, kill switch
Инструменты
| Drawing rail, flow rail, instrument tabs, timeframe/window, alerts, chart settings
9.2. Execution realism
Paper/testnet модель обязана учитывать fees, spread, slippage, partial fills, latency и rejection reasons.
Для order-book стратегий требуется явная модель queue/fill probability и предупреждение, что это оценка.
Каждый fill получает expected vs actual, fee, slippage, quality и связь с thesis.
Переход к live проходит через read-only → testnet → confirmed orders → controlled automation.
10. Опционный терминал
OPTIONS MOAT  Уникальное направление AlphaQuant — объединить underlying order flow, perpetual positioning и option volatility на одной временной шкале.
10.1. Основные модули
Модуль
| Назначение
Underlying Order Flow
| Heatmap, footprint, DOM и реакция базового актива
Smart Option Chain
| Bid/ask, spread, mark, IV, Greeks, OI, volume, flow, liquidity score
Options Liquidity Map
| Expiry × strike/delta, цвет по IV/spread/OI/volume, bubbles по trades
IV Surface
| Smile, skew, term structure, ATM IV, risk reversal, realized vs implied
Option Flow Timeline
| Trades, aggressor, sweep/block, premium, IV, underlying/perp reaction
Greeks & Position Risk
| Delta, gamma, vega, theta, scenario exposure
P&L Attribution
| Вклад delta, IV, theta, gamma, fees/slippage
Scenario Lab
| Сдвиг underlying, времени, IV, skew и term structure
Option DOM & Hedge
| Скальпинг контракта, spread quality, theoretical value, quick delta hedge
10.2. Доступные данные
Deribit: ticker/order book, bid/ask/mark IV, open interest, Greeks, underlying/index и real-time subscriptions; отдельные test и production environments.
Binance Options: index price, klines, open interest, mark/Greeks, depth, trades, block trades, WebSocket market streams и торговые endpoints.
Качественный replay, IV history и event analytics потребуют собственного накопления, версионирования и контроля качества исторических данных.
10.3. Варианты Workspace
Options Analysis: chain, surface, flow, Greeks, payoff и AI Analyst.
Options Scalping: underlying DOM + option DOM, synchronized tapes, IV tick, spread quality, delta-adjusted P&L и hedge controls.
Portfolio Options: aggregate Greeks, concentration, expiry ladder, stress scenarios и hedge recommendations.
11. Quant Research и личные агенты
11.1. Простой Quant workflow
QUANT FLOW  Idea → Formalize → Test → Inspect → Validate → Deploy
Пользователь описывает идею естественным языком.
AI превращает её в читаемую specification: universe, events, filters, entry, exit, risk и execution assumptions.
Пользователь выбирает dataset и запускает job.
На графике появляются signals, fills и rejected opportunities.
Результат показывает expectancy, drawdown, fill rate, slippage, regime breakdown, parameter stability и OOS degradation.
Отдельный validator проверяет leakage, overfitting, sample size и sensitivity.
Стратегия переходит в paper agent, затем testnet и только потом в контролируемый live.
11.2. Личный агент пользователя
Шаг конструктора
| Содержание
Goal
| Что агент должен искать или делать
Universe & Inputs
| Инструменты, данные, события, journal/playbook
Tools
| Read market, annotate, alert, backtest, paper order, request approval
Trigger
| Continuous, schedule, on event, manual
Policy
| Notional, risk, leverage, symbols, approval, stale-data block
Output
| Notification, annotation, report, rule, paper order
Memory
| Проверенные и отклонённые гипотезы, причины, user feedback
11.3. Research swarm
Idea Agent — создаёт и дедуплицирует research tickets.
Feature Agent — строит признаки из market/event store.
Backtest Agent — запускает реалистичную симуляцию.
Validator — независимый maker-checker с OOS, bootstrap и multiple-testing controls.
Regime Auditor — проверяет устойчивость по liquidity/volatility/funding regimes.
Execution Auditor — анализирует capacity, queue, slippage, adverse selection и exchange fragmentation.
Risk Agent — проверяет leverage, tail risk и concentration.
Decay Monitor — сравнивает paper/live с моделью и фиксирует drift.
ОЦЕНКА ИСТОЧНИКА  Приложенная статья о swarm полезна как архитектурная метафора: специализированные агенты, параллельные loops, persistence, maker-checker и проверяемые stop conditions. Её утверждения о замене исследовательской команды и «institutional-grade» результате не следует принимать без независимой валидации.
12. B2A-платформа
12.1. Что продаётся агентам
Слой
| Продукт B2A
Data
| Нормализованные spot/perp/futures/options feeds и historical replay
Context
| Market regime, liquidity state, events, key levels, data quality
Tools
| Research, workspace, journal, alert, risk и execution functions
Memory
| Hypothesis history, rejected reasons, playbooks, user preferences
Sandbox
| Paper/testnet execution и reproducible backtests
Policy
| Permissions, limits, approvals и kill switch
Audit
| Tool calls, model/version, evidence, decisions, fills и outcomes
Runtime
| Scheduled/event-driven agents и multi-agent workflows
12.2. Permission model
Раздельные read/write/execute permissions.
Никаких withdrawal permissions в пользовательских trading agents.
Human approval для live orders по умолчанию.
Блокировка при stale data, exchange degradation или нарушении risk policy.
Полный audit trail и возможность остановки/rollback.
13. Данные и технический moat
Moat layer
| Почему сложно скопировать
Нормализация рынка
| Единая схема нескольких бирж, типов инструментов и quality states
Event-level history
| Дорогой storage, snapshot/delta reconciliation, clock alignment и replay
Deterministic event engine
| Проверяемые определения pull/add, sweep, absorption, replenishment, divergence, IV shift
Execution simulator
| Fees, spread, latency, partial fills, queue assumptions, funding, options liquidity
User decision graph
| История гипотез, ошибок, process score и curriculum
Hypothesis-to-agent path
| Один объект становится annotation, alert, backtest, rule, agent и deployment
Risk & policy layer
| Безопасное подключение автономных агентов к торговле
14. Конкурентная карта
Категория / референс
| Сильная сторона
| Разрыв, который закрывает AlphaQuant
Bookmap
| Heatmap, full market depth, live order flow, replay
| AI evidence, обучение на графике, derivatives/options context, agents
Quantower
| Панели, workspaces, DOM, order flow, volume profiles, trading
| Единый AI runtime, персональный Tutor, крипто-native event store
TradingView
| Layouts, multi-chart sync, replay, массовая экосистема
| Глубина order book, execution realism, options/agent workflow
QuantConnect
| Unified research/backtest/paper/live и специализированные AI assistants
| Визуальная криптомикроструктура, natural-language event strategies, order-book replay
Options terminals
| Chain, surface, Greeks, payoff
| Underlying/perp/order-flow synchronization и P&L attribution
AI signal products
| Простая выдача сигналов
| Evidence, invalidation, user hypothesis, tools, policies и audit
15. Бизнес-модель
Тариф
| Ценность
| Ориентир
Free / Learn
| Delayed/replay, базовый chart, ограниченный Tutor и journal
| Воронка и обучение
Trader
| Real-time order flow, layouts, AI events, alerts, journal
| $29–49/мес.
Pro
| Multi-exchange, advanced replay, execution analytics, options modules
| $79–149/мес.
Quant
| Historical event data, compute, agents, backtests, bot runtime
| $199–499/мес.
B2A / Enterprise
| API, agent gateway, policy engine, dedicated data/SLA
| Usage + contract
Отдельная монетизация: historical depth, compute, agent runtime, API calls, streamed symbols, options analytics и team audit storage.
Нужно отдельно проверить условия коммерческого хранения, переработки и распространения данных каждой биржи.
16. Roadmap и приоритеты
Этап
| Содержание
| Цель
1. AI-native core
| Beginner + Scalper, unified AI Event, Tutor chat/tools, contextual Ask AI, Journal memory
| Доказать ежедневную ценность
2. Swing + Quant Lite
| Derivatives context, natural-language spec, event backtest, paper agent
| Проверить hypothesis-to-rule
3. Options Analytics
| Deribit ingestion, chain, IV surface, flow, Greeks, P&L attribution
| Создать уникальную вертикаль
4. Controlled execution
| Read-only, testnet, confirmed orders, risk policies, approvals
| Безопасная торговля
5. Agent Runtime / B2A
| Agent builder, API tools, schedules, audit, personal agents
| Платформенная экономика
6. Swarms & marketplace
| Specialized research agents, templates, third-party agents
| Масштабирование ecosystem
17. Риски и ограничения
Scope risk: попытка одновременно строить order flow, options, quant, education и execution.
AI trust: ложная уверенность, hindsight, hallucinations и неоткалиброванные probabilities.
Data quality: websocket gaps, duplicate trades, snapshot/delta errors, clock drift и exchange outages.
Backtest realism: queue position, capacity, latency и liquidity assumptions.
Regulation: разграничение education, analytics, personalized advice и execution по юрисдикциям.
Security: API keys, agent permissions, prompt/tool injection и live-order approvals.
Costs: хранение глубины, compute и LLM/tool loops.
18. KPI и критерии успеха
Область
| Ключевые метрики
Activation
| Создан Workspace, завершён Tutor interaction, создан alert/paper decision
Learning
| Hypothesis-before-answer rate, process score, risk compliance, skill progression
Trading
| Fill quality, slippage, stop adherence, risk violations, paper-to-live degradation
AI
| Event precision, evidence click-through, correction rate, action conversion, latency
Quant
| Ideas formalized, valid runs, OOS survival, deployment rate, live/model reconciliation
Agents
| Successful runs, approvals, policy blocks, cost per useful result, audit completeness
Retention
| D7/D30 по Workspace, active trading days, replay/journal usage
19. Итоговая продуктовая формула
ALPHAQUANT  Один терминал. Несколько специализированных Workspace. Один AI runtime с инструментами. Один market/event store. Один Journal. Один risk/policy layer. Один путь от наблюдения к агенту и контролируемому исполнению.
Первый wedge: AI Tutor + Scalper Execution на одном order-flow ядре.
Второй moat: историческая микроструктура и event engine.
Уникальная вертикаль: crypto options с синхронизацией underlying/perp/IV.
Долгосрочная платформа: Trading OS для людей и их агентов.
Источники и референсы
Проверено по официальным страницам и документации по состоянию на июль 2026 года. Источники используются как ориентиры рынка и технических возможностей, а не как подтверждение будущих результатов продукта.
1. Bookmap Features. Full market depth, live order flow и replay. https://bookmap.com/features/
2. Bookmap Heatmap Guide. Heatmap, passive liquidity, aggressive volume и historical replay. https://bookmap.com/blog/heatmap-in-trading-the-complete-guide-to-market-depth-visualization
3. Quantower Platform. Панели, groups, binds и workspaces. https://www.quantower.com/node/1
4. Quantower FAQ. DOM, time & sales, multiple workspaces и templates. https://www.quantower.com/faq
5. TradingView Chart Sync. Синхронизация symbol, interval, time и date range. https://www.tradingview.com/support/solutions/43000629992-how-to-sync-the-charts-of-my-layout/
6. TradingView Bar Replay. Replay на нескольких графиках. https://www.tradingview.com/support/solutions/43000712747-bar-replay-how-and-why-to-test-a-strategy-in-the-past/
7. QuantConnect AI Assistance. Специализированные AI assistants по этапам strategy workflow. https://www.quantconnect.com/docs/v2/ai-assistance/getting-started
8. QuantConnect Research Pipeline. Tools для кода, backtests, interpretation и paper testing. https://www.quantconnect.com/docs/v2/cloud-platform/research-pipeline
9. QuantConnect Reconciliation. Сравнение live и параллельного OOS backtest. https://www.quantconnect.com/docs/v2/writing-algorithms/live-trading/reconciliation
10. Deribit API. Market data, test/production environments и MMP. https://docs.deribit.com/
11. Deribit Ticker Subscription. IV, Greeks, open interest, underlying и funding. https://docs.deribit.com/subscriptions/market-data/tickerinstrument_nameinterval
12. Binance Options Market Data. Index, klines, OI, mark/Greeks, depth, trades и block trades. https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-options/api/rest-api/market-data
13. Binance Options Streams. WebSocket market streams для options. https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-options/api/ws-streams
14. Приложенная статья Roan. Архитектура swarm: specialized loops, persistence, maker-checker и stop conditions. Пользовательский файл: Pasted text(29).txt