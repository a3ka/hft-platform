# risk — Functional Architecture (FA)

> CORNERSTONE FA (safety spine). Authored by architect (Fable) 2026-07-10 как эталон
> FA-глубины. STABLE/APPEND-ONLY. Перенос EINHARD-валидатора в трейдинг с ОБРАТНОЙ
> полярностью risk_guard (hft-core-rs). Парный модуль: `killswitch` (отдельный процесс).
> ⚠ **Нумерация milestone'ов в этом документе ИСТОРИЧЕСКАЯ** (`M-05`/`M-06` здесь означают не то,
> что реально сделано под этими номерами). Актуальная очередь и гейты — `milestones/BACKLOG.md`;
> фазы — `docs/DESIGN.md` §10 (между P2 и P3 добавлена **P2.5 Data safety net**).

## §1. Идентичность модуля
- **Имя / крейт:** `risk` (`crates/risk/`).
- **Язык:** Rust, in-process движка (но логически — единственная дверь к бирже).
- **Слой:** 5 (защищённая зона; квант-агенты НЕ пишут; тесты sacred).
- **Deploy:** in-process с движком; дублируется независимым `killswitch`-процессом (§7).
- **Статус:** ACCEPTED 2026-07-10 (v1).
- **Карта секций:** §1 идентичность · §2 роль · §3 IS · §4 IS NOT · §5 pre-trade gate ·
  §6 позиции/PnL/recon · §7 связь с killswitch · §8 лимит-профили (T2) · §N контракты ·
  §I инварианты RK-I-1..10 · §T RED-маппинг · §P фаза · §O открытые · §A антитезис.

## §2. Стратегическая роль
`risk` — **единственный санкционированный путь ордера на биржу**. Каждый ордер, прежде
чем `oms` отдаст его `venue`, проходит детерминированный pre-trade gate. Модуль отвечает
на один вопрос — «можно ли отправить ЭТОТ ордер ПРЯМО СЕЙЧАС при текущем состоянии
позиций/PnL/лимитов» — и делает это **fail-closed**: при любой неопределённости, отказе
инфраструктуры или неизвестном входе ответ = НЕТ. Он же ведёт реал-тайм позиции/PnL и
сверяет их с биржей (reconciliation). Без этого модуля система не имеет права торговать
живыми деньгами — это структурный, а не политический барьер.

Позиция в потоке: `... → strategy (desired orders) → [risk gate] → oms → venue`.
Обратный поток: `venue (fills) → oms → risk (обновление позиции/PnL) → journal`.

## §3. Что модуль ЕСТЬ
**Онион-подструктура:**
- core (без I/O): `gate/` — чистая функция `(Order, RiskState, Limits) -> GateVerdict`;
  `pnl/` — чистый пересчёт позиции/PnL от fill-событий; `limits/` — типы лимит-профилей.
- mid: `state/` — держит `RiskState` (позиции, дневной PnL, счётчики скорости, инвентарь),
  обновляется ТОЛЬКО из журнальных событий (детерминизм).
- outer: `recon/` — периодический запрос позиций/ордеров у биржи через `venue` (read),
  сравнение, эмиссия `Recon(Snapshot|Mismatch)`; `audit/` — эмиссия решений gate в журнал.

**Доменный словарь (T2):**
- `RiskApproved<Order>` — тип-обёртка с **приватным конструктором** внутри `risk`; только
  его принимает `OrderGateway::place`. Невозможно сконструировать вне `risk` → байпас не
  выражается в системе типов (материализация RK-I-1/RK-I-2).
- `GateVerdict = Approved(RiskApproved<Order>) | Rejected{rule, current, limit, margin} | Halt{reason}`.
- `RiskState` — {позиции по инструментам, дневной realized+unrealized PnL, order-rate
  sliding window, инвентарь}; чистый редьюсер над `Ord(Fill)`/`MarkPx`/`Ctl(ParamChange)`.
- `LimitProfile` — см. §8.

**Обработка отказов/деградации (всегда fail-closed):**
| Условие | Детерминированный исход |
|---|---|
| Неизвестный инструмент/стратегия | Reject (НЕ default-лимиты — анти-risk_guard) |
| `RiskState` недоступен / паника в gate | Halt: торговля стоит (RK-I-4) |
| Отказ записи аудита/журнала | Halt (RK-I-5; никаких `let _ =`) |
| Recon: расхождение позиции/ордеров > ε | Halt + алерт (RK-I-8) |
| Дневной loss-лимит пробит | Halt; re-arm только человеком (RK-I-9) |
| Инвентарь-кап пробит | одностороннее квотирование → стоп (штатная деградация) |
| Отсутствует свежий `MarkPx` (для price-guard) | Reject ордеров, зависящих от mark |

## §4. Что модуль НЕ ЕСТЬ
1. **НЕ диспетчер** — не решает, ЧТО торговать и КОГДА; принимает готовый `Order` от
   `strategy`, только пропускает/отклоняет (это не альфа и не sizing).
2. **НЕ sizing/portfolio** — размеры считает `portfolio`; `risk` лишь проверяет их против
   жёстких лимитов (portfolio может ошибиться — risk ловит).
3. **НЕ отправитель на биржу** — сеть/протокол у `oms`+`venue`; `risk` выдаёт `RiskApproved`,
   но сам не держит биржевой коннект (кроме read-only recon).
4. **НЕ kill switch** — аварийную отмену-всего и обесточивание держит отдельный процесс
   `killswitch`; `risk` может запросить halt, но независимый рубильник — не он (RK-I-6).
5. **НЕ источник рыночных данных** — mark/позиции читает из журнала/recon, не строит книгу.
6. **НЕ конфигурируемый в обход** — не существует поля/флага, отключающего gate (RK-I-2).

## §5. Pre-trade gate (ядро)
Единственная дверь: `OrderGateway::place(o: RiskApproved<Order>)`. Сконструировать
`RiskApproved` можно ТОЛЬКО через `risk::gate::evaluate(order, &state, &limits) -> GateVerdict`.
Проверки — детерминированные, **порядок фиксирован** (первый Reject останавливает):

1. **Инструмент зарегистрирован** в активном профиле → иначе Reject (fail-closed).
2. **Размер ордера** ≤ `max_order_notional`.
3. **Позиция после исполнения** (текущая ± размер) ≤ `max_position_notional`.
4. **Скорость ордеров** (sliding-window) ≤ `max_orders_per_sec`.
5. **Отклонение цены** от свежего `MarkPx` ≤ `price_deviation_guard` `[verify-at-impl]`.
6. **Дневной PnL** > −`daily_loss_limit` (иначе Halt, не Reject).
7. **Инвентарь** в пределах `inventory_cap` (иначе сигнал одностороннего режима в `strategy`).

Каждый вызов gate эмитит событие: `Risk(Approved)` | `Risk(Rejected{rule,margin})` |
`Risk(Halt{reason})` | `Risk(NearMiss{rule,utilization})` при utilization ≥ 80% (телеметрия
«что почти сломалось»). Gate — чистая функция: одинаковый вход → одинаковый verdict
(реплеится; DET-I-1).

## §6. Позиции, PnL, reconciliation
- `RiskState` обновляется ТОЛЬКО редьюсером над журналом: `Ord(Fill)` двигает позицию и
  realized PnL; `MarkPx` двигает unrealized PnL; `Ctl(ParamChange)` меняет лимиты. Никакого
  скрытого состояния — реплей журнала восстанавливает `RiskState` бит-в-бит.
- `recon/`: каждые N сек читает позиции/открытые ордера у биржи (`venue` read) → сравнивает
  с `RiskState` → при |Δ| > ε эмитит `Recon(Mismatch)` → Halt+алерт (RK-I-8). Расхождение =
  «наша модель мира разошлась с биржей» = самая опасная ситуация, всегда стоп.
- EOD: PnL-атрибуция (spread capture / inventory / fees / funding) считается из журнала.

## §7. Связь с killswitch (независимый рубильник)
`risk` in-process — первая линия; `killswitch` — **отдельный процесс со своим биржевым
коннектом и кредами**, вторая независимая линия (RK-I-6). Протокол:
- Движок шлёт heartbeat в `killswitch`; пропажа heartbeat > T → `killswitch` сам делает
  cancel-all (RK-I-7: после разрыва на бирже нет наших заявок).
- Триггеры halt (RK-I-8 recon-mismatch, RK-I-9 loss-лимит) дублируются: `risk` эмитит
  Halt в журнал И сигналит `killswitch`; `killswitch` ставит halt-замок (файл/ключ),
  который движок ОБЯЗАН проверить при старте (нельзя стартовать в halt без снятия человеком).
- `killswitch` работает, даже если движок мёртв/завис — потому он отдельный процесс, не
  поток внутри `risk` (иначе паника gate убила бы и рубильник).

## §8. Лимит-профили (T2) — стартовые числа для live-micro ($500–2k)
`LimitProfile` подписывается как обычный `Ctl(ParamChange)` (граница C, INTG-I-3/RK-I-10).
Стартовый профиль `live-micro` (цель — доказуемость, не доход):
| Поле | Значение |
|---|---|
| instruments | ровно 1 (HL:BTC \| HL:ETH) |
| max_position_notional | ≤ 15% капитала |
| max_order_notional | ≤ 3% капитала |
| daily_loss_limit | ≤ 2% капитала → Halt |
| max_orders_per_sec | консервативно от лимита HL `[verify-at-impl]` |
| price_deviation_guard | ≤ X% от mark `[verify-at-impl]` |
| inventory_cap | = max_position_notional |
Профили `backtest`/`paper`/`testnet` — свои значения; `live` (P5) — рабочие. Профиль =
именованный подписанный артефакт, НЕ флаг (нельзя «расширить лимит» правкой на лету).

## §N. Интерфейсные контракты
- **Consumed:** `Order` (T2 ← `strategy`); `Ord(Fill)`, `MarkPx`, `Ctl(ParamChange)` (T1 ←
  journal); позиции/ордера биржи (read ← `venue`).
- **Produced:** `RiskApproved<Order>` (T2 → `oms`, единственный потребитель); `Risk(...)`,
  `Recon(...)`, `Ctl(KillSwitch)` события (T1 → journal → все).
- **Refused:** сырой `Order` в `oms` без обёртки (типовой барьер); любой путь к `venue::place`
  в обход gate (arch-lint + grep).

## §I. Инварианты (RK-I-1..10) — RED-оракулы (sacred; тест падает на заглушке)
1. **RK-I-1** Ни один ордер не достигает venue без `RiskApproved` (типовой барьер + grep-тест).
2. **RK-I-2** Байпас-флаг НЕ СУЩЕСТВУЕТ: нет конфиг-поверхности, отключающей gate (тест
   утверждает ОТСУТСТВИЕ поля, не «default off» — прямая инверсия risk_guard `enabled`).
3. **RK-I-3** Неизвестный инструмент/стратегия → Reject (fail-closed; НЕ `unwrap_or_default`).
4. **RK-I-4** Gate недоступен/паника → торговля стоит; деградация никогда не «пропускает».
5. **RK-I-5** Отказ записи журнала/аудита → Halt (никаких проглоченных ошибок).
6. **RK-I-6** Kill switch работает при мёртвом движке (отдельный процесс/креды/коннект).
7. **RK-I-7** После разрыва связи на бирже нет наших заявок (CoD и/или KS-sweep drill).
8. **RK-I-8** Recon-расхождение > ε → Halt + алерт.
9. **RK-I-9** Пробитие дневного loss-лимита → Halt; re-arm только человеком.
10. **RK-I-10** Лимиты/параметры меняются только подписанным `Ctl(ParamChange)` (зеркало INTG-I-3).
- **RK-I-Z** Cornerstone-citability: §§ этого FA STABLE/APPEND-ONLY; правки named-not-silent.

## §T. RED-тест маппинг (`crates/risk/tests/`, sacred)
- `test_no_order_without_approval` (RK-I-1, типовой + grep) · `test_no_bypass_surface`
  (RK-I-2, grep отсутствия) · `test_unknown_instrument_rejects` (RK-I-3) ·
  `test_gate_panic_halts` (RK-I-4) · `test_audit_failure_halts` (RK-I-5) ·
  `test_killswitch_survives_engine_death` (RK-I-6, интеграционный, 2 процесса) ·
  `test_disconnect_sweeps_orders` (RK-I-7, drill против testnet) ·
  `test_recon_mismatch_halts` (RK-I-8) · `test_loss_limit_halts` (RK-I-9) ·
  `test_param_change_requires_signature` (RK-I-10). Все обязаны падать на заглушке-no-op.

## §P. Фаза реализации
**P3 (M-05)** — после журнала (P0/P1) и sim (P2); блокирует P4 (live-micro). Мок-точки: на
P2 gate можно звать в paper-режиме поверх sim (проверка формы verdict'ов) до полного
recon/killswitch. Обязательный гейт: RED-suite RK-I-1..10 GREEN (падали на заглушках) +
48ч чистого testnet-MM + disconnect-drill.

## §O. Открытые вопросы
- `[verify-at-impl]` HL: точный `price_deviation_guard`, rate-лимиты, механизм
  cancel-on-disconnect (есть ли биржевой CoD или полагаемся только на KS-sweep).
- ε для recon-mismatch (абсолют vs относительный; funding/mark округления).
- Формат halt-замка (файл vs ключ) и протокол снятия человеком.
- Точные числа sim-vs-live допуска для перехода testnet→live-micro (фикс на выходе P3).

## §A. Антитезис (steelman против дизайна)
- «In-process gate + отдельный killswitch — избыточно; хватило бы одного.» — Контр: паника
  или зависание внутри движка убивает in-process линию; независимый процесс — единственный,
  кто гарантированно отменит заявки при мёртвом движке. Стоимость (второй коннект/креды)
  мала против сценария «завис с открытыми ордерами».
- «Типовой барьер `RiskApproved` — церемония, дев обойдёт через unsafe/новый метод.» —
  Контр: обход требует авторски-владельческого изменения в `risk` (sacred зона) под
  ревьюером; grep-канарейка ловит новый путь к `venue::place`; барьер не абсолют, но
  поднимает цену обхода с «случайно» до «намеренно, под ревью».
- «Fail-closed на любой неопределённости остановит торговлю слишком часто.» — Контр: на
  этапе доказуемости ($500–2k) остановка дешевле молчаливого проскока; частота ложных
  halt'ов — метрика для калибровки ε/порогов, а не повод ослаблять fail-closed.

## Amendment history
| Дата | Изменение | Автор |
|---|---|---|
| 2026-07-10 | v1 authoring (cornerstone exemplar) | architect (Fable) |
