# M-23 — Heatmap + COB + Volume Bubbles — ядро Bookmap-кокпита

STATUS: **PROPOSED** (2026-07-23, architect). Пивот P-COCKPIT, Трек B (MVP-1), **центральный виз-примитив
(Bookmap-heatmap)**. Разблокирован M-29 (`OrderBook::apply_delta` в main). Критик НЕ триггерится (gateway
impl + новый book-dep, не contracts/risk/ks/oms/venue, не новый крейт) — reviewer-бэкстоп.

## Objective

Три серии в `gateway::SeriesBundle` для Bookmap-кокпита, все из **L2Delta-реконструированной книги** (M-29):
1. **Heatmap** — матрица ликвидности `(время × цена) → покоящийся размер`: per time-bucket снимок книги в
   ценовом окне вокруг mid. Ядро Bookmap.
2. **COB** (Current Order Book) — текущий (финальный) стакан: bid/ask уровни с размерами (правая колонка DOM).
3. **Volume Bubbles** — торгованный объём `(время × цена) → buy/sell` (пузыри исполнений).

**Честность (BINDING, VB-I-5):** heatmap строится на diff-реконструированной книге → каждая ячейка глубже
1.3% от mid несёт `depth_band_provenance: "diff-reconstructed"`. **Дальние уровни могут содержать фантом
(TD-016, OPEN)** — heatmap ОКНОВАН (см. §Design) + провенанс; полная достоверность дальних полос — Трек A
(TD-016 эвикция), предусловие TPP-сумм (Трек C), НЕ дисплея heatmap (как Bookmap — показываем реконструкцию честно).

## Contract impact (T1) — НЕТ; export v2 — T-designate аддитивно

- Читает существующие `MdPayload::{L2Snapshot, L2Delta, Trade}` (Граница A). CT-RFC не нужен.
- **`SeriesBundle` += `heatmap`/`cob`/`volume_bubbles`** (аддитивно, В КОНЕЦ). Новые T-designate типы
  `HeatmapCell`/`CobLevel`/`BubbleCell` в `crates/gateway`. **`GATEWAY_SCHEMA_VERSION` bump 4 → 5.**
  `Snapshot::apply` сливает новые серии по бакетам (heatmap/bubbles close-семантика; cob = замена финалом).

### Контракт-формы (engine-dev создаёт с ЭТИМИ полями)

```rust
pub struct HeatmapCell { pub time_s: i64, pub side: String, pub price_e8: i64, pub size_e8: i64,
                         pub depth_band_provenance: Option<String> }
pub struct CobLevel    { pub side: String, pub price_e8: i64, pub size_e8: i64 }   // текущий стакан
pub struct BubbleCell  { pub time_s: i64, pub price_e8: i64, pub buy_vol_e8: i64, pub sell_vol_e8: i64 }
```
`side` = `"bid"`|`"ask"`. Все ×1e8. Сортировка стабильна (BTreeMap-редьюсер).

## Design (пиновка для engine-dev)

- **gateway зависит от `crates/book`** (новый dep) — Reducer держит `book::OrderBook` для `(Selector.venue,
  symbol)`, применяя `L2Snapshot` (`apply_snapshot`) И `L2Delta` (`apply_delta`, M-29) по мере стрима.
  **Расширить `Reducer::apply` веткой `MdPayload::L2Delta`** (сейчас только Trade/L2Snapshot).
- **Ценовое окно (bounded + анти-фантом):** `W = max(Selector.bands)` (переиспользуем поле, без ripple —
  урок M-24). Heatmap/COB эмитят уровни в `[mid·(1−W), mid·(1+W)]`. Вне окна — не эмитятся (bounded; дальний
  фантом TD-016 не тащим в дисплей). `mid` — из книги на момент бакета.
- **Heatmap close-семантика:** значение ячейки `(bucket, price, side)` = размер на этой цене в книге на
  ПОСЛЕДНЕМ обновлении книги в бакете (как depth_series close). Провенанс на ячейках глубже 1.3%.
- **COB:** уровни книги в окне на финальном курсоре snapshot'а (bid по убыв. цены, ask по возр.).
- **Volume Bubbles:** `BTreeMap<(time_s, price_e8), (buy_vol, sell_vol)>` из `Trade` (side→buy/sell);
  цены НЕ выдумываются (только торгованные, как footprint C-016). i128 в аккумуляции? объёмы — i64 достаточно
  на bucket (как cumulative_delta), но НЕ f64 (детерминизм).
- **live == replay (GW-I-3):** новые серии байт-идентичны snapshot vs replay; книга детерминирована
  (apply_delta зеркалит venue, M-29). **Bounded (GW-I-2):** book-state O(уровней книги) — окно бьёт выход;
  рост книги (TD-016) — известный отдельный долг (backstop, не этот milestone).

## Инварианты (RED, sacred — architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **HM-I-1** | **Heatmap = L2Delta-реконструированная книга per bucket.** После снапшота+дельт ячейка `(bucket, price, side)` == размер в книге (учёт apply_delta set/remove). | `red_heatmap.rs::heatmap_reflects_l2delta_book` |
| **HM-I-2** | **Окно + провенанс (VB-I-5).** Ячейки ТОЛЬКО в `[mid·(1−W), mid·(1+W)]`; ячейка глубже 1.3% несёт непустой `depth_band_provenance`; уровень вне окна НЕ эмитится. | `::heatmap_windowed_and_provenance` |
| **HM-I-3** | **COB = финальный стакан в окне.** `cob` = уровни книги на финальном курсоре, bid по убыв./ask по возр., в окне. | `::cob_is_final_book` |
| **HM-I-4** | **Volume bubbles = торгованный объём (time×price), цены не выдуманы.** buy/sell раздельно; цена без сделок отсутствует. | `red_bubbles.rs::bubbles_buy_sell_and_not_invented` |
| **HM-I-5** | **Детерминизм + live==replay.** Новые серии байт-идентичны при повторе и snapshot-vs-replay (расширяет GW-I-3). | `red_heatmap.rs::determinism` |

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-23-heatmap.md`, `crates/gateway/tests/red_heatmap.rs`,
  `crates/gateway/tests/red_bubbles.rs`, `scripts/verify_M-23.sh`.
- **engine-dev (impl, зона gateway):** `crates/gateway/src/lib.rs` — book-dep + `HeatmapCell/CobLevel/BubbleCell`
  + heatmap/cob/bubbles-аккумуляторы в `Reducer` (book через `apply_snapshot`/`apply_delta`) + `Snapshot::apply`
  merge + bump schema→5. Добавить `book = { path = "../book" }` в `crates/gateway/Cargo.toml`.
- **Forbidden:** `crates/contracts` (T1), `crates/book/src` (ЧИТАЙ/ИСПОЛЬЗУЙ apply_delta как lib, НЕ правь),
  `crates/{risk,killswitch,oms,venue-*,journal,recorder}`, f64 в аккумуляции, `read_all` в gateway/src, order-path.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | HM-I-1..5 RED (`red_heatmap.rs`, `red_bubbles.rs`) + `verify_M-23.sh` (fmt+clippy+test, RN-17) | architect | compile-RED (нет heatmap/cob/volume_bubbles); достижимо; окно+провенанс (HM-I-2) обязателен |
| 2 | ⏳ | book-dep + Reducer держит OrderBook (apply_snapshot+apply_delta), ветка L2Delta | engine-dev | HM-I-1 GREEN |
| 3 | ⏳ | Heatmap (окно, close, провенанс) + COB + типы + `SeriesBundle` поля + bump schema→5 | engine-dev | HM-I-2/HM-I-3/HM-I-5 GREEN; GW-I-3/GW-I-5 не сломаны |
| 4 | ⏳ | Volume Bubbles аккумулятор (Trade→buy/sell per time×price, цены не выдуманы) | engine-dev | HM-I-4 GREEN; workspace green |

## Гейты

- **critic — НЕ требуется** (gateway impl + book-dep, не T1/risk/venue/новый крейт). **risk-critic N/A** (MD-only read-side). reviewer — UNCONDITIONAL.
- **verify_M-23.sh** ОБЯЗАН: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` (RN-17).
- **§8-lite:** gateway read-only reducer/replay; recorder-образ инертен.

## Место в очереди

- **Разблокирован** M-29 (apply_delta). Завершает визуальное ядро MVP-1 (heatmap = центр Bookmap-кокпита).
- **Дальше / параллельно:** Трек A (gap-detection + **TD-016 эвикция** — предусловие ДАЛЬНЕЙ достоверности
  heatmap/TPP-полос), M-25 liq/OI/funding-профили (лёгкий), M-28 gateway-serve (транспорт, на engine-dev).
- **Провенанс — честный мост:** heatmap показывает diff-реконструкцию с провенансом СЕЙЧАС; TD-016 эвикция
  повышает достоверность дальних ячеек ПОЗЖЕ (аддитивно, провенанс уже на месте).

## Handoff (план при старте)

architect (RED+verify+milestone) → engine-dev (tasks 2-4) → tester (чистый прогон incl `cargo fmt --all --check`;
бутстрап ТОЛЬКО с origin/feat/M-23-heatmap — RN-18; push GREEN перед handoff) → reviewer (merge + §8-lite).
