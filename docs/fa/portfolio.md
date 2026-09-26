# portfolio — Functional Architecture (FA)

> FA. Authored by architect (Fable) 2026-07-10 по образцу `fa/risk.md`. STABLE/APPEND-ONLY.
> Sizing-слой между `alpha` (калиброванный forecast) и `strategy` (MM-квотер исполнения).
> Пара-документ: `fa/strategy.md` (потребитель `TargetPosition`/`TargetQuotes`).
> ⚠ **Нумерация milestone'ов в этом документе ИСТОРИЧЕСКАЯ** (`M-05`/`M-06` здесь означают не то,
> что реально сделано под этими номерами). Актуальная очередь и гейты — `milestones/BACKLOG.md`;
> фазы — `docs/DESIGN.md` §10 (между P2 и P3 добавлена **P2.5 Data safety net**).

## §1. Идентичность модуля
- **Имя / крейт:** `portfolio` (`crates/portfolio/`).
- **Язык:** Rust, in-process движка.
- **Слой:** 3 (`DESIGN §2`: `alpha, portfolio`).
- **Deploy:** in-process с движком; вызывается на каждое релевантное событие в горячем пути
  (не отдельный процесс, не защищённая зона в смысле `risk`/`killswitch` — но структурно
  изолирован от venue/oms, см. §4).
- **Статус:** DRAFT (2026-07-10, v1) — авторская FA, ревью-цикл не пройден.
- **Карта секций:** §1 идентичность · §2 роль · §3 IS · §4 IS NOT · §5 sizing pipeline ·
  §6 MM target quotes/skew · §7 correlation phase-gating · §8 limit-profile consumption
  discipline · §N контракты · §I инварианты PF-I-1..10+Z · §T RED-маппинг · §P фаза ·
  §O открытые · §A антитезис.

## §2. Стратегическая роль
`portfolio` отвечает на вопрос «сколько» — переводит калиброванный `forecast` (от `alpha`)
в целевой размер позиции, а для MM — в целевые котировки (`TargetQuotes`: цена, размер,
скос от инвентаря). Позиция в потоке: `... → alpha (forecast) → [portfolio] → strategy
(desired orders) → risk → oms → venue`. Без него `alpha` производил бы направление без
размера, а `strategy` не имела бы, что квотировать. `portfolio` **предлагает**; финальное
слово всегда за `risk` (§4.2, зеркало доктрины risk.md §2) — это не дублирование, а
независимая вторая проверка размера перед деньгами (см. §A).

На $500–2k live-micro (`DESIGN §4`, `01 §12` P4) sizing намеренно упрощён: 1 пара, 1–2
сигнала, БЕЗ корреляций (структурно выключены до P5, §7). Ёмкость на этом капитале
нерелевантна — цель этапа доказуемость, не доход (`DESIGN §0`).

## §3. Что модуль ЕСТЬ
**Онион-подструктура:**
- core (без I/O): `sizing/` — чистая функция `(Forecast, LimitsSnapshot, Inventory,
  Correlations?) -> Target`; `skew/` — чистый пересчёт целевого bid/ask из целевой позиции
  и текущего инвентаря (для MM-профиля).
- mid: `limits_view/` — держит `LimitsSnapshot` (проекция сигнатурного `Ctl(ParamChange)`
  из журнала, §8), обновляется ТОЛЬКО из журнальных событий (детерминизм, зеркало
  `risk::state`).
- outer: нет собственного I/O-подмодуля в горячем пути — `portfolio` не читает сеть, не
  пишет файлы; единственный «внешний» контакт — чтение журнала (через тот же механизм,
  которым `risk` строит `RiskState`), см. §8.

**Доменный словарь (T2):**
- `TargetPosition` — {instrument, target_notional, direction, max_deviation_from_current}.
- `TargetQuotes` — {instrument, bid{price,size}, ask{price,size}, inventory_skew_bps} — для
  MM-профиля; `strategy` не пересчитывает скос сама, только диффует против текущих квот (ST-I-4).
- `LimitsSnapshot` — портфельная проекция подписанного `LimitProfile` (см. §8) — НЕ тот же
  Rust-тип, что `risk::LimitProfile` (независимый декод одного и того же T1 `Ctl(ParamChange)`
  payload — обсуждение компромисса в §O).
- `InventoryView` — read-only проекция позиции/инвентаря из журнала (см. §3 таблица, PF-I-8).

**Обработка отказов/деградации (fail-closed везде, где касается размера):**
| Условие | Детерминированный исход |
|---|---|
| Forecast NaN / overflow / вырожденный | Отказ выдать target (не 0-size — явное «нет мнения») |
| Инструмент вне активного `LimitsSnapshot` | Отказ выдать target (PF-I-5; НЕ default-sizing) |
| Инвентарь/позиция недоступны (журнал/проекция не читается) | Отказ выдать target — sizing вслепую запрещён (PF-I-8) |
| Сигнал в ensemble `retired`/вес не пришёл через `alpha` | Исключается из forecast (это ответственность `alpha`; `portfolio` доверяет входу от `alpha` как единственному источнику, PF-I-3) |
| `LimitsSnapshot` отсутствует/не подписан | Отказ выдать target |
| Инструментов в активном профиле > 1 без сигнатурного multi-instrument профиля | Корреляции остаются OFF, sizing падает к single-instrument пути (структурный гейт, §7, PF-I-6) |
| Инвентарь-кап пробит (по `LimitsSnapshot`) | Односторонний `TargetQuotes` (только сторона на уменьшение инвентаря) — сигнал далее интерпретирует `strategy` (ST-I-5) |

## §4. Что модуль НЕ ЕСТЬ
1. **НЕ alpha/signal-gen** — не считает forecast и не комбинирует сигналы (веса/ансамбль —
   территория `alpha`); `portfolio` принимает `Forecast` как готовый вход (PF-I-3).
2. **НЕ risk-enforcement** — не хранит и не проверяет жёсткие лимиты как последнюю
   инстанцию; **portfolio proposes, risk disposes** — `risk::gate::evaluate` может отклонить
   предложенный `portfolio` размер, и это штатно, не баг (PF-I-2, см. §A).
3. **НЕ execution/strategy** — не пересчитывает diff-квоты по событиям, не владеет
   order state machine; это `strategy` (пересчёт на каждое событие — территория соседа).
4. **НЕ oms/venue** — не имеет пути к бирже ни напрямую, ни через `OrderGateway`; крейт
   `portfolio` структурно не имеет Cargo-зависимости на `oms`/`venues` (PF-I-2).
5. **НЕ владелец `SignalRegistry`/калибровки весов** — читает уже отфильтрованный/взвешенный
   `Forecast` от `alpha`; не решает, какой сигнал `live`, и не назначает `ensemble_weight`
   (граница B, `03 §2` — не территория `portfolio`).
6. **НЕ risk-крейт-зависимость** — несмотря на то что sizing «в курсе» лимитов (§8), это
   НЕ импорт `risk`-крейта: онион-правило «зависимости только вниз» (`01 §2`) запрещает
   layer-3 `portfolio` зависеть от layer-5 `risk`. Осведомлённость о лимитах реализована
   через независимую проекцию T1-события, не через crate-эдж (§8, PF-I-4).

## §5. Sizing pipeline (`sizing/`)
Единственная точка входа: `sizing::evaluate(forecast: &Forecast, limits: &LimitsSnapshot,
inventory: &InventoryView, correlations: Option<&CorrelationMatrix>) -> Option<TargetPosition>`.
`None` — явный отказ (§3 таблица), не «нулевая позиция» (нулевая позиция — валидный
результат, отличный от отказа). Алгоритм (детерминированный, порядок фиксирован):

1. Инструмент присутствует в `limits.instruments` → иначе `None` (PF-I-5).
2. `forecast` конечен и в допустимом диапазоне → иначе `None` (PF-I-9).
3. Сырой целевой notional = `forecast.direction * forecast.confidence * limits.max_position_notional`
   (форма упрощена для live-micro; полная формула калибруется на P2/P3 через research-cli,
   не хардкодится здесь — `[verify-at-impl]` для итогового коэффициента).
4. Клип по `limits.max_order_notional` на шаг изменения (portfolio не предлагает мгновенный
   скачок всей позиции за один такт — сглаживание уменьшает вероятность отказа gate'ом).
5. Если `correlations.is_some()` И профиль многоинструментный (сигнатурно подтверждено,
   §7) → корректировка по корреляционной матрице; иначе шаг пропускается целиком (PF-I-6).
6. Инвентарь-кап (`limits.inventory_cap`) проверяется мягко: при приближении/пробитии
   результат помечается `one_sided_hint` — не блокирует sizing, но передаётся в §6.

Это **предложение**, не решение — `risk` независимо повторяет проверки 1/2/4/6 своими
терминами (RK-I-3, gate-проверки 2-3-7) и может отклонить результат этого пайплайна
целиком (PF-I-2). Sizing — чистая функция: тот же вход → тот же выход, реплеится (PF-I-1,
зеркало DET-I-1).

## §6. MM target quotes и инвентарь-скос (`skew/`)
Для MM-профиля (единственный профиль на P4) `TargetPosition` конвертируется в `TargetQuotes`:
- Базовая цена — microprice ± half-spread (параметр из `LimitsSnapshot`/калибровки).
- **Скос от инвентаря**: `skew_bps = f(current_inventory / max_position_notional)`, где
  `f` монотонна и НЕ убывает по модулю накопленной позиции в направлении, противоположном
  инвентарю (накопленный лонг → bid отодвигается вниз/размер bid уменьшается, ask
  придвигается ближе/размер ask растёт) — это PF-I-7, тестируемое свойство, а не точная
  формула (точный вид `f` — предмет калибровки research-cli, `[verify-at-impl]`).
- Если `one_sided_hint` установлен (§5 шаг 6) — сторона, УВЕЛИЧИВАЮЩАЯ инвентарь, гасится
  (размер → 0 или котировка не выставляется), остаётся только уменьшающая сторона.
- `strategy` НЕ пересчитывает скос — она диффует уже готовые `TargetQuotes` против текущих
  живых котировок на каждое событие (ST-I-4, разделение «что квотировать» / «как исполнить»).

## §7. Correlation phase-gating (P4 single-pair → P5 multi-instrument)
`DESIGN §4`/`01 §12`: на live-micro (P4) корреляции ВЫКЛЮЧЕНЫ структурно, не «по умолчанию
0». Гейт — не `if correlations.weight == 0`, а отсутствие самого пути: `sizing::evaluate`
принимает `correlations: Option<&CorrelationMatrix>`, и вызывающий код (`runner`, композиция
режимов, `01 §9`) передаёт `None` до тех пор, пока активный `LimitsSnapshot` не декларирует
`instrument_count > 1` через подписанный `Ctl(ParamChange)`. Это делает включение корреляций
именованным, подписанным событием (мостится к `03 §3` границе C), а не тихой регрессией при
рефакторинге кода (PF-I-6 — анти-паттерн `risk_guard` default-lenient, `DESIGN §9`). P5
(Binance + lead-lag, `DESIGN §10`) — первый профиль, где `instrument_count > 1` подписан.

## §8. Limit-profile consumption discipline (осведомлён, но НЕ владелец enforcement)
`portfolio` читает лимиты, но **не является** источником правды по ним (это `risk`, §4.6).
Механизм: и `risk`, и `portfolio` независимо декодируют ОДИН И ТОТ ЖЕ подписанный
`Ctl(ParamChange)` T1-payload из журнала (§01 §7, `RK-I-10`/`INTG-I-3`) — `risk` строит из
него `risk::LimitProfile` (владеет enforcement-семантикой), `portfolio` строит из него
СВОЙ `LimitsSnapshot` (владеет sizing-семантикой). Это не два разных источника правды — это
одна подписанная запись, два независимых консюмера, что сохраняет онион-направление
(`portfolio` не зависит от Rust-типа `risk`) ценой дублирования декодера (компромисс,
разобран в §A и §O — кандидат на промоушен в T1 `contracts/`, но НЕ решено этим документом).
`portfolio` НИКОГДА не пишет в `Ctl(ParamChange)` сама — предложения по лимитам идут через
`portfolio-analyst`/founder в очередь решений (`03 §3`), не из этого крейта.

## §N. Интерфейсные контракты
- **Consumed:** `Forecast` (T2 ← `alpha`, единственный источник направления/уверенности);
  `Ctl(ParamChange{signed})` (T1 ← журнал, boundary C, `03 §3`) — декодируется в собственный
  `LimitsSnapshot`, НЕ импортируется как тип `risk`; позиция/инвентарь (read-only проекция
  ← журнал `Ord(Fill)`/`MarkPx`, аналогично тому, как `risk::state` строит `RiskState`, но
  независимой копией — PF-I-8).
- **Produced:** `TargetPosition`, `TargetQuotes` (T2 → `strategy`, единственный потребитель).
- **Refused/forbidden:** сырой `Order` (это территория `strategy`/`oms`); прямой вызов
  `risk::gate::evaluate` или `OrderGateway` (нет такого пути — PF-I-2); чтение `signals`
  напрямую в обход `alpha` (PF-I-3); корреляционный вклад в sizing при неподписанном
  multi-instrument профиле (PF-I-6).

## §I. Инварианты (PF-I-1..10 + PF-I-Z) — RED-оракулы (тест падает на заглушке)
1. **PF-I-1** Sizing детерминирован: `evaluate(тот же вход) == тот же Target`, реплеится
   (зеркало DET-I-1).
2. **PF-I-2** Portfolio proposes, risk disposes — крейт `portfolio` структурно не имеет
   Cargo-зависимости на `oms`/`venues`/`risk`; нет прямого пути к venue (grep + arch-lint).
3. **PF-I-3** Forecast принимается ТОЛЬКО от `alpha`; `portfolio` не читает `signals`
   напрямую (типовой барьер/grep импортов).
4. **PF-I-4** Лимиты/веса, влияющие на sizing, — ТОЛЬКО из подписанного `Ctl(ParamChange)`;
   нет альтернативной runtime-конфиг поверхности (тест утверждает ОТСУТСТВИЕ поля, зеркало
   RK-I-2/RK-I-10/INTG-I-3).
5. **PF-I-5** Неизвестный/незарегистрированный инструмент → отказ выдать target (fail-closed,
   НЕ default-sizing — анти-`risk_guard`, зеркало RK-I-3).
6. **PF-I-6** Корреляции структурно недоступны при `instrument_count == 1` (или без
   подписанного multi-instrument профиля) — тест утверждает ОТСУТСТВИЕ влияния корреляционной
   матрицы на single-instrument профиле, не просто нулевой вес.
7. **PF-I-7** Инвентарь-скос монотонен: рост накопленной позиции в одну сторону не уменьшает
   встречный скос (property-based тест на синтетических инвентарях).
8. **PF-I-8** Portfolio не ведёт теневой учёт позиции/PnL — инвентарь читается ТОЛЬКО как
   read-only проекция журнала; нет независимо мутируемого состояния позиции внутри крейта.
9. **PF-I-9** Вырожденный/NaN/overflow forecast → отказ (`None`), не propagate в размер.
10. **PF-I-10** `sizing/` без I/O, без часов, без сети (доменная чистота; arch-lint grep
    `SystemTime`/`fs::`/сетевых вызовов в `sizing/`, зеркало Signal-trait дисциплины `01 §5`).
- **PF-I-Z** Cornerstone-citability: §§ этого FA STABLE/APPEND-ONLY; правки — named, не silent.

## §T. RED-тест маппинг (`crates/portfolio/tests/`)
- `test_sizing_is_deterministic` (PF-I-1) · `test_no_oms_venue_risk_dependency` (PF-I-2,
  структурный + `cargo metadata` grep) · `test_forecast_only_from_alpha` (PF-I-3, типовой
  барьер) · `test_limits_require_signed_paramchange` (PF-I-4, grep отсутствия альтернативной
  поверхности) · `test_unknown_instrument_refuses_target` (PF-I-5) ·
  `test_correlations_disabled_single_instrument_profile` (PF-I-6) ·
  `test_inventory_skew_monotonic` (PF-I-7, proptest) ·
  `test_inventory_is_readonly_projection_no_shadow_state` (PF-I-8) ·
  `test_degenerate_forecast_refuses` (PF-I-9, NaN/inf/overflow кейсы) ·
  `test_sizing_core_no_io_no_clock_no_net` (PF-I-10, arch-lint). Все обязаны падать на
  заглушке-no-op (анти-плацебо, зеркало risk.md §T).

## §P. Фаза реализации
**P3 (Risk+OMS, `01 §12`)** — `portfolio` необходим для 48ч testnet-MM (P3 acceptance),
т.к. `strategy` не может квотировать без `TargetQuotes`. Блокируется завершением `alpha`
(нужен стабильный `Forecast`-контракт) и `risk` (нужен `LimitsSnapshot`-источник — сигнатурный
`Ctl(ParamChange)` формат, §8). Мок-точки: на P2 `sizing::evaluate` можно вызывать поверх
sim/paper журнала синтетическим `Forecast`, проверяя ТОЛЬКО форму `Target`/`TargetQuotes`
(без реального gate/killswitch) — аналог risk.md §P «paper-режим поверх sim до полного
recon». Корреляционный путь (§7) остаётся невостребованным (`None`) до P5.

## §O. Открытые вопросы
- **T1 vs T2 для `LimitProfile`.** `DESIGN §3` числит лимит-профили T2 «владеет крейт», но
  консюмеров два (`risk` — enforcement, `portfolio` — sizing-awareness) с независимыми
  декодерами одного T1-payload (§8) — по governance-критерию (≥2 модуля, shape-stable,
  aggregate root) это кандидат на промоушен в `contracts/` T1 через contract-RFC. До решения
  — задокументированный компромисс (дублирование декодера), НЕ решается этим FA.
- Точная формула шага 3 §5 (`forecast.confidence` → notional) и функция скоса `f` (§6) —
  калибруются на research-cli (P2/P3 гриды), не фиксируются здесь; `[verify-at-impl]`.
- Порог staleness для `TargetQuotes`/`TargetPosition` (после скольки мс/событий `strategy`
  обязана считать цель протухшей и снять котировки) — общий вопрос с `strategy.md` §O,
  фиксируется на выходе P3.
- Поведение при повторных `Risk(Rejected)` на один и тот же предложенный target: нужен ли
  `portfolio` feedback-канал (например, снижение уверенности после серии отказов) или это
  чисто `strategy`/`alpha`-территория — не решено, кандидат для P4 наблюдаемости.
- Точное определение `one_sided_hint` порога (доля от `inventory_cap`, при которой
  включается односторонний режим) — `[verify-at-impl]`, калибруется вместе с `risk`
  инвентарь-капом (§8 таблица risk.md).

## §A. Антитезис (steelman против дизайна)
- «Portfolio предлагает размер, который risk может отклонить — это дублирование проверки,
  зачем считать лимиты дважды в двух местах?» — Контр: это НЕ дублирование решения, а
  независимая вторая проверка (та же доктрина, что у `risk`/`killswitch`, risk.md §A) —
  `portfolio` может содержать баг в sizing-формуле; если бы `risk` доверял её выходу
  напрямую, единственная линия защиты денег совпала бы с самым сложным (и самым часто
  меняющимся, из-за калибровки) кодом системы. Стоимость двойного decode (§8) мала против
  сценария «баг в sizing тихо прошёл, потому что risk доверял portfolio».
- «Структурный гейт на корреляции (PF-I-6) — избыточная церемония; проще было бы просто не
  включать корреляционный код до P5.» — Контр: «просто не включать» — это ИМЕННО тот паттерн
  (неявное, негейтированное состояние), который проект explicitly отвергает как наследие
  `risk_guard` (`DESIGN §9`, default-lenient bypass). Явный структурный гейт делает включение
  audit-able событием (подписанный `Ctl(ParamChange)`), а не побочным эффектом рефакторинга.
- «Независимый декодер `LimitsSnapshot` в `portfolio` (вместо импорта `risk::LimitProfile`)
  создаёт риск schema-дрейфа между двумя декодерами одного payload.» — Контр: честно, это
  реальный компромисс (не бесплатный) — отражён в §O как открытый вопрос с прямым
  решением (промоушен в T1). До решения verify-скрипт ДОЛЖЕН включать cross-decode
  parity-тест (оба декодера на одном фикстурном `Ctl(ParamChange)` дают согласованные
  числа) — иначе компромисс превращается в тихую дыру.
- «Инвентарь-скос как read-only проекция журнала (PF-I-8), а не собственный учёт, замедляет
  реакцию portfolio на собственные fills (задержка на запись+чтение журнала).» — Контр:
  задержка тут — цена детерминизма и анти-дивергенции (то же обоснование, что у `risk::recon`,
  risk.md §6) — независимый теневой учёт позиции в двух местах системы гарантированно
  разойдётся рано или поздно; лучше единая read-model с небольшой латентностью, чем два
  расходящихся источника правды о том, сколько у нас позиции.

## Amendment history
| Дата | Изменение | Автор |
|---|---|---|
| 2026-07-10 | v1 authoring (DRAFT) | architect (Fable) |
