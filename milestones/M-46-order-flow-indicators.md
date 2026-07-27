# M-46 — Order Flow Intelligence: 4 индикатора (Time&Sales / Refill / Liquidity / Spoofing)

**Статус:** PLANNED (стаб). **Founder-подпись:** 2026-07-27. Фазировано по зависимости от M-45.
Все 4 — чистые редьюсеры над `Event`-потоком (Trade + L2Snapshot/L2Delta), read-side, детерминированные
(VB-I-1); ложатся в `crates/gateway`/`derive`, RED-first, детерминизм-тест на каждый.

## Фаза 1 — БЕЗ изменения сбора (можно сразу, на текущих данных)
| Индикатор | Вход | Fidelity сейчас |
|---|---|---|
| **Time & Sales** | Trade (time/price/size/агрессор `m`; крупные принты по порогу) | ✅ полная (сделки не сэмплируются) |
| **Refill / hidden size (iceberg)** | Trade × L2Snapshot (absorbed объём сделок на цене vs восстановление отображаемого) | 🟡 рабочая (ведётся полной лентой сделок; книга 1с) |
| **Liquidity tracker (грубый)** | L2Snapshot 1с (net add/pull между снапшотами; pull vs execution — корреляция с Trade) | 🟠 грубый (1с, бакеты 2bps) |

## Фаза 2 — полная fidelity на сырых L2Delta
**Данные УЖЕ есть для BTC** (M-18 пишет сырой L2Delta с 2026-07-21) → Фаза 2 по BTC строится СЕЙЧАС.
Для остальных символов — ПОСЛЕ M-45 (расширение allow-list L2Delta).
| Индикатор | Даёт | Данные |
|---|---|---|
| **Spoofing radar** | суб-секундная детекция «крупный объём появился→исчез БЕЗ исполнения, цена не дошла» | BTC: сейчас; др. символы: после M-45 |
| **Liquidity tracker (точный)** | точная цена + суб-секунда add/pull | BTC: сейчас; др.: после M-45 |

## Честная разметка провенанса (BINDING — «не выдумываем»)
- Binance/HL публично дают агрегированный L2 (net-size на цену), **НЕ order-by-order (MBO/ID заявок)**.
  Значит Spoofing/Refill — **net-level ИНФЕРЕНСЫ**, не биржевой факт (как Bookmap/TPP). Каждая серия
  несёт метку метода (напр. `orderflow_inference: "net-level, no MBO"`) + provenance глубины (VB-I-5,
  ≤1.3% валидировано). Фронт/AI не выдают инференс за факт.
- Fidelity Фазы 1 явно помечается «1s/bucketed»; Фаза 2 — «raw-diff, exact-price».

## Allowed paths
- `crates/gateway/{src,tests}/` и/или `crates/derive/{src,tests}/` (индикатор-редьюсеры) · export-форма (research/exports, аддитивно, bump export_schema_version) · verify · этот файл. НЕ risk/killswitch/contracts/venue-*/order-path.

## Гейты: reviewer (read-path); critic по триггеру (≥5 коммитов / новый инвариант). risk-critic не нужен.
## Cross-ref: docs/07 (order-flow раздел), M-45 (persist L2Delta — предусловие Фазы 2), docs/fa/viz-backend.md §2 (дериватив-слой), замер fidelity 2026-07-27.
