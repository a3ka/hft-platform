# M-30 — book gap-detection (update-id chaining, fail-closed → resync) — Track A шаг 3а

STATUS: **PROPOSED** (2026-07-24, architect). Пивот P-COCKPIT, **Трек A (корректность книги), шаг 3а**.
Закрывает ОСОЗНАННУЮ дыру M-29: replay-книга применяла L2Delta БЕЗ валидации sequencing (журнал
предполагался упорядоченным). Критик НЕ триггерится (`crates/book`, не contracts/risk/ks/oms/venue,
не новый крейт) — reviewer-бэкстоп.

## Objective

Дать `crates/book::OrderBook` **детекцию разрывов потока дельт** по update-id (Binance chaining): каждая
`L2Delta` должна ЧЕЙНИТЬСЯ к предыдущей; разрыв (пропущенная дельта / переупорядочивание) → книга
**СТАЛА-НЕДОСТОВЕРНА (fail-closed)** до ресинка снапшотом. Без этого gap в журнале молча даёт расхождение
replay-книги (риск, помеченный в M-29). Fail-closed — тот же принцип, что риск-слой (`RK`): неизвестный/
разорванный вход → отказ, не «применить наугад».

## Contract impact (T1) — НЕТ

- Читает существующие поля `MdPayload::L2Delta` (`first_update_id`/`final_update_id`/`prev_final_update_id`).
  Новый метод + enum — публичный API `crates/book` (не T1). CT-RFC не нужен.

## Design (пиновка для engine-dev)

- **`OrderBook` += состояние:** `last_final_update_id: Option<u64>`, `stale: bool`.
- **`OrderBook::apply_l2delta(bids, asks, first_update_id, final_update_id, prev_final_update_id) -> ContinuityStatus`**
  (`enum ContinuityStatus { Applied, Gap }`):
  - **Bootstrap** (`last_final_update_id == None`, книга свежая после снапшота): применить дельту (как
    `apply_delta`), `last_final = final_update_id`, вернуть `Applied`.
  - **Continuity OK** → применить, `last_final = final_update_id`, `Applied`:
    - СПОТ (`prev_final_update_id == None`): `first_update_id == last_final + 1` (Binance `U == prev.u+1`);
    - ФЬЮЧЕРС (`prev_final_update_id == Some(pu)`): `pu == last_final` (Binance `pu == prev.u`).
  - **Gap** (continuity нарушена ЛИБО книга уже `stale`): **НЕ применять** (fail-closed — применение
    разорванной дельты портит книгу), `stale = true`, вернуть `Gap`.
- **`apply_snapshot` — ресинк:** сбрасывает книгу + `last_final = None` + `stale = false` (следующая дельта
  бутстрапит чейн заново). Снапшот = единственный способ выйти из `stale`.
- **`Books::apply(L2Delta)`** → `apply_l2delta`; на `Gap` книга остаётся `stale` (доступно через
  `OrderBook::is_stale()` — консюмер (gateway heatmap/depth) помечает период данных недостоверным).
- **`apply_delta` (M-29) СОХРАНЯЕТСЯ** (raw-применение без чейнинга) — для случаев, где sequencing не нужен;
  `apply_l2delta` — чейнинг-aware путь. (Полный переход gateway на apply_l2delta + флаг heatmap — follow-up.)

## Инварианты (RED, sacred — architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **GD-I-1** | **Спот-чейн OK.** Две дельты с `U == prev.u+1` → обе `Applied`; книга отражает обе. | `red_gap_detection.rs::spot_contiguous_applies` |
| **GD-I-2** | **Спот-gap fail-closed.** Дельта с `U != prev.u+1` → `Gap`; книга НЕ изменена разорванной дельтой; `stale`. Анти-плацебо: impl, применяющий gap → книга двинулась → падение. | `::spot_gap_fail_closed` |
| **GD-I-3** | **Фьючерс-чейн OK.** `pu == prev.u` → `Applied`. | `::futures_contiguous_applies` |
| **GD-I-4** | **Фьючерс-gap fail-closed.** `pu != prev.u` → `Gap`; `stale`; не применено. | `::futures_gap_fail_closed` |
| **GD-I-5** | **Bootstrap.** Первая дельта (нет prior, `last_final==None`) → `Applied`, чейн заведён. | `::bootstrap_first_delta` |
| **GD-I-6** | **Ресинк снапшотом выводит из stale.** После gap → `apply_snapshot` → `stale=false`; следующая дельта `Applied` (чейн заново). Пока не ресинкнуто — дельты `Gap`. | `::snapshot_resync_clears_stale` |

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-30-book-gap-detection.md`, `crates/book/tests/red_gap_detection.rs`, `scripts/verify_M-30.sh`.
- **engine-dev (impl, зона `book`):** `crates/book/src/lib.rs` — `ContinuityStatus` + `OrderBook::apply_l2delta`
  (+ `last_final_update_id`/`stale`/`is_stale`) + `Books::apply(L2Delta)` → apply_l2delta. `apply_snapshot` сброс stale.
- **Forbidden:** `crates/contracts` (T1), `crates/venue-*` (эталон chaining — читай, не правь), `crates/{risk,killswitch,oms,journal,recorder}`, order-path.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | GD-I-1..6 RED (`red_gap_detection.rs`) + `verify_M-30.sh` (fmt+clippy+test, RN-17) | architect | compile-RED (нет `apply_l2delta`/`ContinuityStatus`); fail-closed (GD-I-2/4) обязателен |
| 2 | ⏳ | `OrderBook::apply_l2delta` + `ContinuityStatus` + `last_final_update_id`/`stale`/`is_stale` | engine-dev | GD-I-1..5 GREEN |
| 3 | ⏳ | `Books::apply(L2Delta)` → apply_l2delta; `apply_snapshot` сбрасывает stale/last_final | engine-dev | GD-I-6 GREEN; M-29 apply_delta и L2Snapshot-путь не сломаны; workspace green |

## Гейты

- **critic — НЕ требуется** (crates/book, не T1/risk/venue/новый крейт). **risk-critic N/A** (MD-only read-side).
  reviewer — UNCONDITIONAL. verify (RN-17: fmt+clippy CI-точно + toolchain-пин 1.97.0, TD-035).
- **§8-lite:** reducer/replay-путь; recorder-образ инертен (venue live-путь имеет свою chaining-FSM, не задет).

## Место в очереди (Track A)

- **3а (этот).** Далее **3б TD-016 эвикция** — подход выбран founder'ом (2026-07-24): **bound (тугой кап памяти)
  + recon-near эвикция ≤1.3% + провенанс-far** (дальний фантом inherent, честно помечен). **3в resync-целостность**
  (apply_snapshot не роняет восстановимые дальние уровни). Предусловие ДАЛЬНЕЙ достоверности heatmap/TPP.
- Не блокирует M-23 (heatmap уже честен окном+провенансом) и M-28. Улучшает достоверность книги на длинном горизонте.

## Handoff (план при старте)

architect (RED+verify+milestone) → engine-dev (tasks 2-3) → tester (чистый прогон incl `cargo fmt --all --check`
на toolchain 1.97.0; бутстрап ТОЛЬКО с origin/feat/M-30-book-gap-detection — RN-18; push GREEN перед handoff) →
reviewer (merge + §8-lite).
