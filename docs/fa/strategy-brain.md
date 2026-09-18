# FA — strategy-brain: `alpha` · `portfolio` · `strategy` (Слои 3–4)

STATUS: FA v1 (2026-07-13, architect). Покрывает ТРИ крейта одним документом намеренно:
это один сквозной детерминированный конвейер решений (`SignalOut → Forecast →
TargetPosition → OrderIntent`), инварианты которого имеют смысл только вместе.
Источники правды выше: `docs/DESIGN.md` §1 (детерминизм), §2 (слои), §3 (T1/T2/T3),
`docs/05-contract-layer.md` §2, `docs/03-integration-contract.md` §4 (граница A).

---

## §1. Зачем этот слой

Между «сигнал сказал +0.4» и «отправить ордер» лежит РЕШЕНИЕ: сколько держать. Сегодня
это решение захардкожено в ad-hoc harness'е `research-cli/src/grid.rs` (taker-in по
`SignalOut`, taker-out по `horizon_ms`) — то есть бэктест меряет ЛОГИКУ, КОТОРОЙ НЕ БУДЕТ
В LIVE. Это прямое нарушение DESIGN §1 равенства 2 (`backtest == paper == live`).

`alpha`+`portfolio`+`strategy` — единственный код решений; его исполняют И `sim`
(бэктест), И будущий `runner` (live). Расхождения sim↔live быть не может, потому что
кода два раза не существует.

## §2. Границы и зависимости (онион, только вниз)

```
Layer 2  signals   ──▶ SignalOut                    (граница A, квант-деск)
Layer 3  alpha     ──▶ Forecast      (signals, contracts)
Layer 3  portfolio ──▶ TargetPosition (alpha, contracts)
Layer 4  strategy  ──▶ OrderIntent   (alpha, portfolio, signals, book, contracts)
Layer 6  sim       ──▶ исполняет OrderIntent (зависит на strategy — ВНИЗ, легально)
Layer 6  runner    ──▶ исполняет OrderIntent (P3+; тот же strategy)
```

ЗАПРЕЩЕНО (структурные тесты): `strategy`/`portfolio`/`alpha` не зависят от `sim`,
`venue-*`, `journal`, `risk`, `killswitch`, `tokio`, `reqwest`. Нет `SystemTime`/
`Instant::now`/`rand`. Нет итерации по `HashMap` (только `BTreeMap`/`Vec`) — журнал-принцип
DESIGN §1: редьюсер обязан быть бит-воспроизводим.

**Риск.** Этот слой — НЕ риск-гейт. `portfolio` несёт лишь pre-risk sanity-кап позиции
(`PF-I-2`); настоящий fail-closed `RiskApproved`-барьер (`RK-I-1..10`) вводится M-08
(`crates/risk`) и встаёт МЕЖДУ `strategy` и `oms`. Ни один тест этого слоя не должен
читаться как «риск уже есть».

## §3. Формы (T2 — владеет крейт, contract-RFC не требуется, 05 §2)

| Форма | Крейт | Поля |
|---|---|---|
| `Instrument` | alpha | `{venue: Venue, symbol: String}` — тотальный порядок (`Ord` по `(venue as u8, symbol)`) |
| `Forecast` | alpha | `{instrument, ts_mono_ns, edge_e8 ∈ [-1e8,+1e8], horizon_ms, confidence_e8 ∈ [0,1e8]}` |
| `Position` / `TargetPosition` | portfolio | `{instrument, qty_e8}` (знаковый: + long, − short) |
| `RiskBudget` | portfolio | `max_position_e8` per instrument; НЕТ дефолта (fail-closed) |
| `OrderIntent` / `OrderKind` | **strategy** (переехал из `sim`, M-07) | `{venue, symbol, side, price, qty, kind}` |
| `FillReport` | strategy | `{instrument, side, price_e8, qty_e8, fee_e8, ts_mono_ns}` — обратная связь исполнения |

`OrderIntent` живёт в Layer 4, потому что его ПРОДЮСЕР — strategy, а консюмеры — `sim`
(бэктест) и `oms`/venue (live). Если бы он остался в `sim`, live-runner линковал бы
симулятор ради типа. `sim` ре-экспортирует его (`pub use strategy::{OrderIntent, OrderKind}`)
— форма одна на всю систему (урок hft-core-rs: один формат, не три).

## §4. `alpha` — ансамбль сигналов → forecast

```rust
pub trait Alpha {
    /// Чистый редьюсер: событие + выходы сигналов ЭТОГО события → форкасты.
    fn update(&mut self, ev: &Event, signal_outs: &[SignalOut]) -> Vec<Forecast>;
}
```

v1 — `LinearAlpha` (взвешенная сумма, веса из конфига `Vec<SignalWeight{signal_id,
instrument, weight_e8}>`):

- Держит последний сэмпл на `(instrument, signal_id)`: `{value_e8, horizon_ms, ts_mono_ns}`.
- **Stale-expiry:** сэмпл участвует, пока `ev.ts_mono_ns ≤ ts_event + horizon_ms·1e6`.
  Протух → выпал. Все протухли → форкаста по инструменту НЕТ (не «edge=0» — отсутствие
  мнения ≠ мнение «ноль», зеркало D1 signals).
- `edge_e8 = clamp(Σ wᵢ·vᵢ / Σ|wᵢ|, ±1e8)` (арифметика i128 — переполнение невозможно).
- `horizon_ms = max(horizonᵢ)` по участвующим (консервативно: держим до самого медленного).
- `confidence_e8 = Σ|wᵢ|(активные) · 1e8 / Σ|wᵢ|(все)` — доля живого веса.
- `SignalOut` с `signal_id`, которого нет в весах, **игнорируется** (не входит в сумму).
- Выход отсортирован по `instrument`.

## §5. `portfolio` — forecast + лимит → target

```rust
pub fn size(forecasts: &[Forecast], positions: &[Position], budget: &RiskBudget)
    -> Vec<TargetPosition>;
```

- `target_qty_e8 = clamp(edge_e8 · max_position_e8 / 1e8, ±max_position_e8)` (i128).
- **Инструмент без лимита в бюджете → `target = 0`** (fail-closed, не «дефолтный лимит» —
  анти-`risk_guard` DESIGN §9).
- **Инструмент, по которому ЕСТЬ позиция, но НЕТ форкаста → `target = 0`** (flatten;
  умерший сигнал не оставляет вечный инвентарь).
- Выход отсортирован по `instrument`; `positions` — только через эту флэттен-семантику
  (нетто/корреляции/turnover-aware sizing — следующая итерация).

## §6. `strategy` — target vs current → интенты

```rust
pub trait Strategy {
    fn on_event(&mut self, ev: &Event) -> Vec<OrderIntent>;
    fn on_fill(&mut self, fill: &FillReport);
    fn position_e8(&self, instrument: &Instrument) -> i64;
}
```

`DirectionalStrategy` (v1, конвейер на КАЖДОМ событии):

1. `books.apply(md)` (реконструкция стакана — `book::Books`).
2. `signals[i].on_event(ev)` в фиксированном порядке → `Vec<SignalOut>`.
3. `alpha.update(ev, &outs)` → `Vec<Forecast>`.
4. `portfolio::size(&forecasts, &positions, &budget)` → `Vec<TargetPosition>`.
5. Для каждого target: `delta = target − position − in_flight`; при `|delta| ≥ min_order_e8`
   — интент (сторона по знаку, `qty=|delta|`, цена — маркетабельный лимит от лучшей
   противоположной котировки с запасом `marketable_margin_bp`; **нет книги/цены → интента
   НЕТ**, не «отправим по любой цене»).
6. `in_flight` учитывается, чтобы не дублировать ордер, пока предыдущий в полёте; запись
   истекает по **event-time** через `intent_ttl_ms` (никакого wall-clock). `on_fill`
   двигает `position` и гасит `in_flight`.

Конфиг: `StrategyConfig{min_order_e8, intent_ttl_ms, marketable_margin_bp, kind}`; v1
`kind = Taker` (directional). **MM-котирование (двусторонние квоты, cancel/replace,
rate-budget) — следующая итерация, требует `oms` + `risk`** (M-08+), сюда не втаскивается.

## §7. Инварианты (RED-оракулы; sacred, architect-only)

| ID | Инвариант |
|---|---|
| AL-I-1 | Детерминизм: один Event-поток ×2 прогона → идентичные `Forecast` |
| AL-I-2 | Комбинация весов: 2 сигнала с весами → edge = Σw·v/Σ\|w\| (проверка на конкретных числах) |
| AL-I-3 | `SignalOut` с неизвестным `signal_id` не влияет на edge |
| AL-I-4 | Stale-expiry: сэмпл старше своего `horizon_ms` выпадает; все протухли → форкаста нет |
| AL-I-5 | `edge_e8` зажат в `[-1e8,+1e8]` даже при мусорном `value` (переполнение исключено) |
| PF-I-1 | Сайзинг: `target = clamp(edge·max_pos/1e8, ±max_pos)` |
| PF-I-2 | **Fail-safe: `\|target\| ≤ max_position_e8` ВСЕГДА** (любой вход, включая `i64::MAX`) |
| PF-I-3 | Инструмент без лимита → `target = 0` (не дефолт-лимит) |
| PF-I-4 | Позиция без форкаста → `target = 0` (flatten) |
| ST-I-1 | Diff: `current=0, target=+X` → ровно один BUY на `X`; `current=+X, target=0` → SELL на `X` |
| ST-I-2 | `target == current` → интентов НЕТ |
| ST-I-3 | In-flight: два события подряд с тем же target → ровно ОДИН интент; после `intent_ttl_ms` без филла → повторный |
| ST-I-4 | Детерминизм: один поток ×2 прогона → идентичные `OrderIntent` |
| ST-I-5 | **Prefix-stability / replay-determinism** (честная формулировка, C-004 M1): интенты на префиксе = префикс интентов полного потока. Это НЕ future-blindness — `on_event(&Event)` будущего физически не получает; настоящий no-lookahead оракул — ST-I-8f (там есть поверхность подглядывания: харнесс держит весь срез) |
| ST-I-6 | Структурно: нет зависимостей `sim`/`venue-*`/`journal`/`risk`/`tokio`/`reqwest`; нет `SystemTime`/`Instant`/`rand`; нет `HashMap` в редьюсерах |
| ST-I-7 | `OrderIntent` определён РОВНО в одном крейте (`strategy`); grep-канарейка (зеркало CT-I-1) |
| ST-I-8a..d | Интеграция: интенты доходят до `sim::BacktestExchange`; позиция стратегии согласована с отчётом; прогон ×2 бит-идентичен при том же seed; префикс филлов = начало полного прогона |
| **ST-I-8e** | **Доставка филлов (C-004 C1):** спай-стратегия фиксирует каждый вызов `on_fill`. `run()` ОБЯЗАН доложить КАЖДЫЙ филл биржи и корректно подписать `FillReport` (instrument/side — из интента через `order_meta`; price/qty/fee/ts — из `SimFill`). Падает, если `run()` пропускает `on_fill`, выдумывает филлы или подписывает всё как `Buy` |
| **ST-I-8f** | **No-lookahead (настоящий, C-004 M1):** мутация ТОЛЬКО будущих событий среза (seq > k) не меняет исполнения в прошлом (seq ≤ k). Ловит чтение среза вперёд — то, чего prefix-stability не видит |
| **ST-I-8g** | **Форма equity-кривой (D7, rev 3 — reviewer-находка):** РОВНО одна точка на КАЖДОЕ СОБЫТИЕ с ≥1 филлом (2 филла на одном событии = 1 точка), снятая ПОСЛЕ применения события к книге и учёта ВСЕХ филлов события. Кривая сверяется ПОЭЛЕМЕНТНО с независимо пересчитанным mark-to-market, не только по длине. Инвариант: `len(equity_curve) == #уникальных seq в fills` |
| **ST-I-8h** | На бесфилловых событиях точек НЕ появляется (ни «догоняющих», ни «доборных»). Наивная привязка к накопленному числу филлов (`curve.len() < fills.len()`) добирает дефицит в бесфилловом хвосте → фантомные точки → лишние near-zero доходности → **σ занижена, Sharpe ЗАВЫШЕН** → `ValidationReport` → trials-ledger → подпись founder'а (gates §6/§7) |

### §7.1 Инварианты грида на strategy-пайплайне (`research-cli`, задача 6; GR-I-*, C-004 C2)

| ID | Инвариант |
|---|---|
| GR-I-1 | Блока `strategy` в ячейке нет → **документированные дефолты** D8 (не нули, не «что-нибудь») |
| GR-I-2 | Блок есть → он применяется; кривой блок (неположительный лимит, неизвестный `kind`) → `Err`, а не молчаливый дефолт (иначе отчёт описывает не ту стратегию, что бежала) |
| GR-I-3 | `params_hash` покрывает блок `strategy` И `costs_mode` — иначе разные стратегии пишутся в trials-ledger под одним хэшем (фальсификация счётчика проб → deflated Sharpe врёт) |
| GR-I-4 | Returns = `Δequity_e8 / capital_ref_e8` (D7), НЕ старая формула entry/exit-нотионалов; `capital_ref ≤ 0` или <2 точек equity → пусто (никаких NaN/inf в Sharpe) |
| GR-I-5 | `capital_ref_e8 = max_position_e8 · first_mid_e8 / 1e8` |
| GR-I-6 | **Поведенческий:** ячейки, различающиеся ТОЛЬКО `strategy.max_position_e8`, дают РАЗНЫЙ оборот. Ad-hoc harness (фиксированный `qty=1.0`) блок игнорирует → падает |
| GR-I-7 | **Поведенческий:** `min_order_e8` > лимита позиции → ноль интентов; ledger несёт канонический `cell_params_hash` |

GR-I-6/7 — причина, по которой задача 6 больше НЕ гейтится грепами: упоминание
`StrategyBacktest` в комментарии/мёртвом коде их не проходит (C-004 C2).

## §O. Открытые вопросы (следующие итерации, named-not-silent)

1. **MM-котирование** (двусторонние квоты, diff-квот, cancel/replace) — требует `oms`
   (rate-budget, идемпотентность) + `risk`. M-08+.
2. **Мультисигнальные веса** приходят из `signals.json` (граница B, подписанное решение) —
   в M-07 веса задаются конфигом раннера/грид-ячейкой; wiring реестра → P3.
3. **`confidence_e8`** v1 = доля живого веса; калибровка (Brier/decay) — по мере данных.
4. **Netting/корреляции/ёмкость** в `portfolio` — P5 (мультипара).
5. **`Ord(Fill)` в журнале** (T1) — contract-RFC на входе в paper/live; в M-07 обратная
   связь исполнения ходит T2-формой `FillReport`.
