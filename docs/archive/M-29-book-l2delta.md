# M-29 — book применяет L2Delta (replay/reducer-путь) — основа heatmap

STATUS: **DONE — merged to main 2026-07-23** (код `04ce788`, reviewer APPROVED, §8-lite GREEN inert). Пивот P-COCKPIT, **Трек A (корректность книги)** —
**предусловие M-23 Heatmap**. `crates/book::OrderBook` (replay/reducer-путь) сейчас применяет ТОЛЬКО
`L2Snapshot`; сырой `MdPayload::L2Delta` (собирается M-18) ИГНОРИРУЕТСЯ → heatmap из инкрементального
стакана строить не на чем. M-29 добавляет каноническое L2Delta-применение в книгу. Критик НЕ триггерится
(`crates/book`, не contracts/risk/ks/oms/venue, не новый крейт) — reviewer-бэкстоп.

## Objective

Дать `crates/book::OrderBook` метод `apply_delta(bids, asks)` (инкрементальное применение) и расширить
`Books::apply` на `MdPayload::L2Delta`, чтобы **replay/reducer-путь реконструировал стакан из журналированных
L2Delta** — семантику ЗЕРКАЛИТ live-захват (`venue-binance::apply_diff_to_book`), обеспечивая **live == replay
книги** (реконструкция из diff бит-в-бит совпадает с тем, что видел venue live).

**В scope:** `OrderBook::apply_delta` + `Books::apply(L2Delta)` — КОРРЕКТНОСТЬ применения diff (set/remove/
неупомянутое/пустая сторона/множественность/масштаб). **НЕ в scope (Трек A, следующие шаги, предусловие TPP):**
эвикция мёртвых уровней (TD-016 — нужен recon-дизайн), gap-detection по update-id (нужен book со счётчиком),
resync-целостность (apply_snapshot не должен ронять восстановимые дальние уровни). Отмечены как follow-up.

## Contract impact (T1) — НЕТ

- Читает существующий `MdPayload::L2Delta` (Граница A). Новый метод `OrderBook::apply_delta` — публичный API
  крейта `book` (не T1-тип, не contracts). CT-RFC не нужен.

## Design (пиновка для engine-dev)

- **`OrderBook::apply_delta(&mut self, bids: &[Level], asks: &[Level])`** — семантика КАК `apply_diff_to_book`:
  - `size == 0` → **удалить** уровень (`remove(price)`);
  - `size > 0` → **upsert** (set `price → size`);
  - **неупомянутая цена — НЕ трогается** (diff НЕ авторитет о неупомянутом; testing.md «отсутствие»);
  - **пустая сторона `[]` → no-op** (НЕ «очистить сторону» — критично, класс TD-016/C-020).
- **`Books::apply`** расширить: `MdPayload::L2Delta { bids, asks, .. }` → `entry(...).apply_delta(bids, asks)`.
  L2Snapshot-ветка не меняется. (Sequencing-поля `first/final/prev_final_update_id` в M-29 НЕ валидируются —
  gap-detection follow-up; журнал предполагается упорядоченным.)
- **live == replay:** семантика apply_delta ОБЯЗАНА совпадать с `venue-binance::apply_diff_to_book` (тот же
  set/remove) — иначе replay-книга ≠ live-книга. (Идеально — общий core; для M-29 достаточно тест-эквивалентной
  семантики; общий core — follow-up рефактор.)

## Инварианты (RED, sacred — architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **BL-I-1** | **Set/remove.** После снапшота дельта: `size>0` upsert (новый/обновлённый уровень), `size==0` удаляет. Точные `size_at` после применения. | `red_l2delta_apply.rs::set_and_remove` |
| **BL-I-2** | **Асимметрия (testing.md #1).** Дельта, трогающая ТОЛЬКО одну сторону, оставляет другую БЕЗ изменений. | `::asymmetry_one_side` |
| **BL-I-3** | **Отсутствие ≠ удаление (testing.md #3, класс TD-016).** Неупомянутые цены неизменны; пустая сторона `[]` — no-op, НЕ очистка стороны. | `::empty_side_and_unmentioned_preserved` |
| **BL-I-4** | **Детерминизм.** Тот же снапшот+поток дельт на двух книгах → идентичные `levels()` обеих сторон. | `::determinism` |
| **BL-I-5** | **Множественность + масштаб (testing.md #2/#5).** Дельта с многими уровнями (set+remove вперемешку, 100+) применяется корректно; большая книга — без порчи. | `::multi_level_and_scale` |
| **BL-I-6** | **`Books::apply(L2Delta)` маршрутизирует в apply_delta.** Журналированное `MdPayload::L2Delta` через `Books::apply` двигает книгу (не игнорируется, как сейчас). Анти-плацебо: текущий impl (L2Delta игнорируется) → книга неизменна → падение. | `::books_apply_routes_l2delta` |

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-29-book-l2delta.md`, `crates/book/tests/red_l2delta_apply.rs`, `scripts/verify_M-29.sh`.
- **engine-dev (impl, зона `book`):** `crates/book/src/lib.rs` — `OrderBook::apply_delta` + `Books::apply` ветка `L2Delta`.
- **Forbidden:** `crates/contracts` (T1), `crates/venue-*` (ЧИТАЙ `apply_diff_to_book` как эталон семантики, НЕ правь),
  `crates/{risk,killswitch,oms,journal,recorder}`, order-path.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ✅ | BL-I-1..6 RED (`red_l2delta_apply.rs`) + `verify_M-29.sh` (fmt+clippy+test, RN-17) | architect | compile-RED (нет `apply_delta`); достижимо; degraded-inputs (BL-I-3) обязателен |
| 2 | ✅ | `OrderBook::apply_delta` (set/remove/неупомянутое/пустая-сторона, зеркало venue) | engine-dev | BL-I-1..5 GREEN |
| 3 | ✅ | `Books::apply` ветка `MdPayload::L2Delta` → apply_delta | engine-dev | BL-I-6 GREEN; L2Snapshot-путь не сломан; workspace green |

## Гейты

- **critic — НЕ требуется** (не T1/risk/venue/новый крейт). **risk-critic N/A** (MD-only read-side). reviewer — UNCONDITIONAL.
- **verify_M-29.sh** ОБЯЗАН: `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` (RN-17).
- **§8-lite:** `crates/book` — reducer/replay-путь; recorder-образ поведением не меняется (recorder уже применяет diff через venue live-путь). Прод инертен.

## Место в очереди

- **Разблокирует M-23 Heatmap** (heatmap = редьюсер над L2Delta-реконструированной книгой; несёт `depth_band_provenance`
  «diff-reconstructed» — VB-I-5). Не блокируется ничем (L2Delta уже в журнале, M-18).
- **Трек A продолжение (после M-29, предусловие TPP-полос Трека C):** gap-detection (update-id chaining) +
  TD-016 эвикция мёртвых уровней (recon-дизайн) + resync-целостность. Отдельные milestone'ы.

## Handoff (план при старте)

architect (RED+verify+milestone) → engine-dev (tasks 2-3) → tester (чистый прогон incl `cargo fmt --all --check`;
бутстрап ТОЛЬКО с origin/feat/M-29-book-l2delta — RN-18; push GREEN перед handoff) → reviewer (merge + §8-lite).
