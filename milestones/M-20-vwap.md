# M-20 — VWAP (session-anchored) — производный индикатор в gateway SeriesBundle

STATUS: **DONE — merged to main 2026-07-22** (merge `743d0b3`, reviewer APPROVED, §8-lite GREEN: recorder
инертен). VB-I-6 примитив `utc_session_id` (pub const fn в gateway) готов для M-24/CVD-reset. Пивот P-COCKPIT, Трек B (MVP-1),
**первый пост-gateway индикатор** (последовательно после M-22). Переориентирован с батч-экспорта
(M-17 `format.md`) на **live gateway-путь**: VWAP-серия вливается в `gateway::SeriesBundle` аддитивно
(export v2 bump), считается тем же streaming-редьюсером, что snapshot/frames/replay (live==replay).
Низкий риск (чистый Trade-редьюсер, без book/L2Delta). Doc-гейт §9: **critic НЕ триггерится** (не
контракт T1, не risk/ks/oms/venue, не новый крейт, < 5 коммитов) — reviewer-бэкстоп на PR-time.

## Objective

VWAP = Σ(price × size) / Σ(size) по **сессии** (якорь 00:00 UTC), вычислим напрямую из уже собираемого
`MdPayload::Trade`. M-20 даёт **дисплей-VWAP** как серию `vwap: Vec<(time_s, vwap_e8)>` в
`gateway::SeriesBundle` для кокпита (M-19), через тот же gateway-транспорт, что M-22. Устанавливает
**session-модель (VB-I-6)** — единый примитив «якорь 00:00 UTC», который переиспользуют M-24 (SVP) и
CVD-reset (НЕ переопределяют, один источник).

**В scope M-20 (дисплей-VWAP):** session-anchored VWAP в gateway + session-модель. **НЕ в scope:**
rolling-N/per-bar варианты (позже, если нужны фронту), VWAP-deviation **сигнал** (Граница A, отложен
с торговым треком — квант-деск), gap-флаг честности (VW-I-5 — data-quality, отдельно). Батч-экспорт
(`research-cli` format.md) VWAP — опционально позже; cockpit-путь (gateway) первичен.

## Contract impact (T1) — НЕТ; export v2 — T-designate аддитивно

- **T1-ядро НЕ трогаем** — VWAP читает существующий `MdPayload::Trade` (Граница A, read-only). CT-RFC не нужен.
- **`gateway::SeriesBundle` += поле `vwap: Vec<(i64 time_s, i64 vwap_e8)>`** (аддитивно; старые v1/v2-консюмеры
  не ломаются — serde игнорирует незнакомое, GW-I-5). **`GATEWAY_SCHEMA_VERSION` bump 2 → 3** (новая серия).
  `Snapshot::apply` (bucket-merge) обязан слить `vwap` как остальные серии (session-cumulative — см. §Design).

## Design (пиновка для engine-dev)

- **VWAP-аккумулятор — в gateway `Reducer`** (рядом с ohlcv/bucket_delta/depth), стримовый fold над
  `journal::stream` (bounded, как весь gateway — GW-I-2). НЕ collect-then-reduce.
- **Session-anchored (VB-I-6):** сессия = UTC-день от `ts_exch_ms`: `session_id = ts_exch_ms.div_euclid(86_400_000)`.
  Аккумулятор держит `(sum_pv: i128, sum_v: i128, session_id)`. На сделке: если `session_id` сменился →
  **сброс** `sum_pv/sum_v`. Затем `sum_pv += price·size` (i128!), `sum_v += size`. VWAP бакета =
  `(sum_pv / sum_v)` как `i64` (единицы `price_e8`; session-cumulative running до конца бакета).
  Session-anchor-хелпер (`session_id`) — переиспользуемый примитив (VB-I-6): M-24/CVD-reset берут ЕГО, не свой.
- **i128 обязателен (VW-I-2):** `price_e8 (i64) · size_e8 (i64)` переполняет i64 на ОДНОМ произведении
  (BTC ~1.2e13 × size 5e8 = 6e21 >> i64_max 9.2e18). Копить в i128; НЕТ f64 в аккумуляции (детерминизм).
- **Бакетирование** VWAP-точек — `bucket_time_s(ts_exch_ms)` (тот же, что ohlcv). Значение бакета =
  session-cumulative VWAP на конец бакета (close-семантика; пустые бакеты не эмитятся).
- **Per (venue, symbol)** — `Reducer.apply` уже фильтрует `md.venue/symbol` по `Selector`; VWAP идёт тем же
  путём → площадки не смешиваются (VW-I-4).

## Инварианты (RED, sacred — architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **VW-I-1** | **Корректность + детерминизм.** VWAP = Σ(px·sz)/Σ(sz) на фикстуре с известными сделками → ТОЧНОЕ значение (посчитано вручную до знака); два прогона одного потока → идентичны. | `red_vwap.rs::vwap_exact` |
| **VW-I-2** | **i128 прод-масштаб без переполнения.** Σ(px·sz) на BTC-масштабе (px ~1.2e13 × sz ~5e8 = 6e21) копится в i128; результат корректен, нет overflow/паники. Анти-плацебо: i64/f64-impl переполняется/теряет точность → падение. | `red_vwap.rs::vwap_i128_prod_scale` |
| **VW-I-3** | **Session reset (00:00 UTC, VB-I-6).** Сделки по разные стороны UTC-полуночи → аккумулятор СБРАСЫВАЕТСЯ: VWAP пост-полуночного бакета отражает ТОЛЬКО новую сессию (не смешан с прошлым днём). Анти-плацебо: impl без сброса → блендит через границу → падение. | `red_vwap.rs::vwap_session_reset` |
| **VW-I-4** | **Per-venue — не смешивать площадки.** На журнале с 2 venue VWAP берёт ТОЛЬКО сделки `Selector.venue`; чужая площадка не подмешивается. | `red_vwap.rs::vwap_per_venue` |

*(VW-I-5 gap-честность и VW-I-6 VWAP-deviation сигнал — ОТЛОЖЕНЫ: data-quality/квант-деск, вне cockpit-MVP.)*

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-20-vwap.md`, `crates/gateway/tests/red_vwap.rs` (VW-I-1..4 RED),
  `scripts/verify_M-20.sh`.
- **engine-dev (impl, зона gateway):** `crates/gateway/src/lib.rs` — VWAP-аккумулятор в `Reducer` +
  поле `vwap` в `SeriesBundle` + `vwap` в `Snapshot::apply` (bucket-merge, session-cumulative) + bump
  `GATEWAY_SCHEMA_VERSION` → 3. Свои deps — при необходимости (маловероятно).
- **Forbidden:** `crates/contracts` (T1), `crates/{risk,killswitch,oms,venue-*,journal,recorder}`, любой
  order-path, f64 в аккумуляции VWAP, `read_all`/`Vec<Event>` в gateway/src (GW-I-2), сигнал/промоушен (Граница B/C).

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ✅ | VW-I-1..4 RED (`red_vwap.rs`) + `verify_M-20.sh` (fmt+clippy+test, RN-17) | architect | compile-RED (нет поля `vwap`); достижимо; i128-масштаб (VW-I-2) обязателен |
| 2 | ✅ | VWAP session-anchored аккумулятор в gateway `Reducer` (i128, VB-I-6 session_id) | engine-dev | VW-I-1/VW-I-2/VW-I-3 GREEN |
| 3 | ✅ | Поле `vwap` в `SeriesBundle` + `Snapshot::apply` merge + bump `GATEWAY_SCHEMA_VERSION`→3 | engine-dev | VW-I-* GREEN; GW-I-3/GW-I-5 (live==replay/аддитивность) не сломаны; workspace green |
| 4 | ✅ | Per-venue корректность (тем же apply-фильтром) | engine-dev | VW-I-4 GREEN |

## Гейты

- **critic — НЕ требуется** (не T1/risk/ks/oms/venue, не новый крейт, < 5 коммитов). reviewer — UNCONDITIONAL бэкстоп.
- **risk-critic N/A** (MD-only read-only, нет order-path).
- **verify_M-20.sh** ОБЯЗАН включать `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`
  (RN-17: verify ⊇ терминальные CI-гейты — иначе false-green, инцидент M-22).
- **§8:** gateway read-only, recorder-образ не меняется (бинаря gateway в образе нет) → прод инертен; reviewer merge feat/M-20→main + §8-lite.

## Место в очереди

- **Первый в последовательности пост-M-22** (founder: делать последовательно). Далее: M-24 Volume Profile
  (переиспользует session-модель VB-I-6), затем M-23 Heatmap (тяжелее — L2Delta/book/TD-016), затем M-25.
- Не блокируется ничем (M-22 gateway готов; Trade уже собирается). Блокирует M-19 (фронт рисует VWAP-линию).

## Handoff (план при старте)

architect (RED+verify+milestone) → engine-dev (tasks 2-4, VWAP-аккумулятор в gateway) → tester
(чистый прогон incl. `cargo fmt --all --check`) → reviewer (merge feat/M-20→main + §8-lite).
