# M-41 — venue-hyperliquid RED-суита (R4, ШАГ 1a)

**Статус:** PLANNED (стаб). **Риск:** R4 HIGH (`docs/08`). Параллелен M-38 (разные зоны).

## Objective
`crates/venue-hyperliquid/src/lib.rs` (единственный парсер нормализации HL→`MdEvent`) — **0 тестов**
(каталога tests/ нет). Контраст с venue-binance/futures. HL — первая венью (DESIGN §0), в проде, пишет
данные. Регрессия парсинга уйдёт в журнал тихо/необратимо (данные — единственный актив). MD-only (blast
radius = порча датасета, не деньги).

## Allowed paths
- `crates/venue-hyperliquid/tests/` (architect RED) · `crates/venue-hyperliquid/src/lib.rs` (venue-dev, только если RED вскроет баг) · `scripts/verify_M-41.sh`.

## Задачи (RED-first, паритетно venue-binance)
1. (architect RED) объектный формат уровней `{px,sz,n}` (код сам помечает «CRITICAL»); malformed-вход →
   `None` (VN-I-7 fail-closed); фильтрация «MID»-инструментов; трактовка `l2Book` как ПОЛНОГО снапшота
   (не diff). Деградированный вход (testing.md чек-лист).
2. (venue-dev) фикс только если RED вскроет расхождение.

## Гейты: reviewer (MD-only carve-out — risk-critic не нужен). critic по триггеру (вряд ли ≥5 коммитов).
## Cross-ref: docs/08 R4, docs/fa/venues.md.
