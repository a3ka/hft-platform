# M-31 — эвикция книги (TD-016): bound + recon-near + provenance-far — Track A шаг 3б

STATUS: **PROPOSED** (2026-07-24, architect). Пивот P-COCKPIT, **Трек A шаг 3б** — ДАЛЬНЯЯ достоверность
книги. Закрывает **TD-016 (OPEN, MAJOR)**: replay-книга из L2Delta растёт неограниченно (мёртвые уровни
не получают `size=0`) → фантомная ликвидность в дальних полосах + рост памяти. Подход выбран founder'ом
(2026-07-24). Критик НЕ триггерится (`crates/book`, не contracts/risk/ks/oms/venue, не новый крейт) — reviewer.

## Objective

Два book-примитива эвикции + честный провенанс (уже на export-слое, VB-I-5):
1. **Bound (память):** `enforce_cap(max_per_side)` — держать ТОЛЬКО `max_per_side` уровней **БЛИЖАЙШИХ к
   лучшей цене** (best-relative), эвиктить ДАЛЬНИЕ. Хард-потолок от OOM. **Best-relative (НЕ mid-диффа)** —
   иммунно к асимметрии (см. §Анти-плацебо).
2. **Recon-near (достоверность ≤1.3%):** `reconcile_near(rest_bids, rest_asks, near_pct)` — В ОКНЕ `near_pct`
   от **mid КНИГИ** держать только уровни, присутствующие в REST-снапшоте (эталон биржи); мёртвые ближние
   стираются. За окном (>near_pct) — НЕ трогать (дальнее inherently diff-реконструкция).
3. **Provenance-far:** дальние уровни (>1.3%) НЕ стираются вслепую — помечаются `depth_band_provenance`
   "diff-reconstructed" (уже сделано на export-слое M-22/M-23; книга их СОХРАНЯЕТ, не удаляет).

**НЕ фиксит (inherent):** дальний фантом >1.3% НЕустраним (эталона у биржи глубже 1.3% нет) — честно помечен,
не выдаётся за биржевой факт. Это осознанное ограничение класса данных, не баг.

## Contract impact (T1) — НЕТ

- Новые методы `OrderBook::{enforce_cap, reconcile_near}` — публичный API `crates/book`. Читает REST-снапшот
  как `&[Level]` (тот же тип). CT-RFC не нужен.

## Design (пиновка для engine-dev)

- **`enforce_cap(&mut self, max_per_side: usize) -> usize`** (кол-во эвикнутых): если уровней на стороне >
  `max_per_side` → оставить `max_per_side` БЛИЖАЙШИХ к лучшей цене (bids — наибольшие цены; asks — наименьшие),
  эвиктить остальные (дальние). **BEST-RELATIVE, НЕ через mid диффа** — критично (см. §Анти-плацебо). `max=0`
  или уровней ≤ max → no-op (0). Детерминировано (BTreeMap-порядок).
- **`reconcile_near(&mut self, rest_bids: &[Level], rest_asks: &[Level], near_pct: f64) -> usize`**: `mid =
  self.mid()` (mid КНИГИ, не диффа). Для bids в `[mid·(1−near_pct), +∞)` (ближняя зона) — оставить ТОЛЬКО цены,
  присутствующие в `rest_bids` (мёртвые ближние стереть); bids ниже `mid·(1−near_pct)` (дальние) — НЕ трогать.
  Симметрично asks. `mid==None` (односторонняя) → no-op. REST-снапшот `size=0` не бывает (снапшот — живые уровни).
- **Триггер (engine-dev/gateway):** `enforce_cap` — после apply в reducer (bounded-инвариант); `reconcile_near`
  — на recon-событии/L2Snapshot (эталон). M-31 даёт МЕТОДЫ + RED; wiring частоты/источника — engine-dev/follow-up.
- **live == replay:** эвикция детерминирована от состояния книги (best-relative / book-mid), без wall-clock/rand.

## §Анти-плацебо (BINDING — урок TD-016 v1-реджект + testing.md «фикстура счастливого пути»)

Оракул ОБЯЗАН падать на **АСИММЕТРИЧНОМ диффе** (обновляется ОДНА сторона; лучший bid НЕ меняется), где наивная
эвикция «по mid диффа» стирает живой best bid. **2 оракула (EV-I-1, EV-I-2) FAIL против v1-логики** (кап 5000 +
side-filter по mid диффа), **GREEN против 3б** (best-relative + book-mid). Degraded-чек-лист: асимметрия /
множественность / отсутствие (дифф молчит об уровне ≠ удалить) / границы окна / бэкстоп-кап от OOM.

## Инварианты (RED, sacred — architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **EV-I-1** (анти-плацебо #1) | **Асимметрия: эвикция НЕ режет живой best bid.** После асимметричного диффа (обновлён только ask; best bid не менялся) `enforce_cap(N)` СОХРАНЯЕТ best bid + N ближайших bids. Best-relative кэп иммунен; v1 (mid диффа) стирает best bid → FAIL. | `red_eviction.rs::asymmetric_keeps_best_bid` |
| **EV-I-2** (анти-плацебо #2) | **Кэп эвиктит ДАЛЬНИЕ, топ сохраняется.** `M>N` уровней → `enforce_cap(N)` → `n_levels==N` на сторону; сохранены N БЛИЖАЙШИХ (best bid/ask целы), эвикнуты дальние. v1 (кап 5000) не эвиктит → `n_levels==M` → FAIL. | `::cap_evicts_farthest_keeps_top` |
| **EV-I-3** | **Recon-near: мёртвые ближние стёрты, дальние целы.** `reconcile_near(REST ≤1.3%)` эвиктит ближние уровни НЕ из REST; уровни за окном (>near_pct) СОХРАНЕНЫ (diff-реконструкция). | `::recon_near_evicts_dead_keeps_far` |
| **EV-I-4** | **Отсутствие ≠ удаление.** Эвикция удаляет ТОЛЬКО (а) дальние сверх кэпа, (б) ближние-не-в-REST. Уровень, о котором никто не «сказал удалить», сам по себе не исчезает (testing.md «отсутствие»). | `::absence_not_deletion` |
| **EV-I-5** | **Бэкстоп OOM.** При росте книги `enforce_cap(cap)` ЖЁСТКО держит `n_levels ≤ cap` на сторону (защита от unbounded — корень TD-016). | `::backstop_bounds_growth` |
| **EV-I-6** | **Детерминизм.** Тот же вход+эвикция на двух книгах → идентичные `levels()`. | `::determinism` |

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-31-book-eviction.md`, `crates/book/tests/red_eviction.rs`, `scripts/verify_M-31.sh`.
- **engine-dev (impl, зона `book`):** `crates/book/src/lib.rs` — `enforce_cap` + `reconcile_near`.
- **Forbidden:** `crates/contracts` (T1), `crates/venue-*` (свой live-путь эвикции — не задевать), `crates/{risk,killswitch,oms,journal,recorder}`, order-path, эвикция «по mid диффа» (v1-анти-паттерн).

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | EV-I-1..6 RED (`red_eviction.rs`) + `verify_M-31.sh` (fmt+clippy CI-точно, RN-17/TD-035) | architect | compile-RED; EV-I-1/EV-I-2 FAIL против v1, GREEN против 3б (reachability в ОБЕ стороны) |
| 2 | ⏳ | `enforce_cap(max_per_side)` — best-relative top-N, эвикт дальних | engine-dev | EV-I-1/EV-I-2/EV-I-5/EV-I-6 GREEN |
| 3 | ⏳ | `reconcile_near(rest, near_pct)` — book-mid ближнее окно, эвикт мёртвых-не-в-REST, дальнее цело | engine-dev | EV-I-3/EV-I-4 GREEN; workspace green |

## Гейты

- **critic — НЕ требуется** (crates/book). **risk-critic N/A** (MD-only read-side). reviewer — UNCONDITIONAL.
  verify CI-точный clippy (`--all-targets --all-features`) на toolchain 1.97.0 (TD-035).
- **§8-lite:** reducer/replay-путь; recorder-образ инертен.

## Место в очереди (Track A)

- **3б (этот).** Далее **3в resync-целостность** (apply_snapshot не роняет восстановимые дальние уровни при
  ресинке к мелкому REST-снапшоту — класс TD-010/012). Затем TD-016 → закрыт (bound+recon+provenance приземлены).
- Разблокирует ДАЛЬНЮЮ достоверность heatmap/TPP-полос (Трек C). M-23 heatmap уже честен окном+провенансом.

## Handoff (план при старте)

architect (RED+verify+milestone) → engine-dev (tasks 2-3) → tester (чистый прогон incl clippy CI-точный 1.97.0;
бутстрап ТОЛЬКО с origin/feat/M-31-book-eviction — RN-18/TD-036; push GREEN перед handoff) → reviewer (merge + §8-lite).
