# M-24 — Volume Profile (SVP) — POC/VAH/VAL в gateway SeriesBundle

STATUS: **DONE — merged to main 2026-07-23** (код `7bd3a69`, reviewer APPROVED, §8-lite GREEN inert;
reviewer завёл TD-034/RN-19/RN-20). RN-19 закрыт: тай-брейк фикстуры (POC тай→низшая, VA тай→верхний)
добавлены в `red_volume_profile.rs`. Пивот P-COCKPIT, Трек B (MVP-1), **второй пост-gateway
индикатор** (последовательно после M-20). Чистый Trade-редьюсер в gateway, **переиспользует session-модель
VB-I-6** (`utc_session_id`, введён M-20 — НЕ переопределять). Doc-гейт §9: **critic НЕ триггерится**
(не T1/risk/ks/oms/venue, не новый крейт, < 5 коммитов) — reviewer-бэкстоп.

## Objective

Volume Profile = гистограмма объёма, торгованного на КАЖДОЙ цене (не по времени), **per сессия** (SVP —
Session Volume Profile, якорь 00:00 UTC). Из неё — **POC** (Point of Control), **VAH/VAL** (Value Area
High/Low, 70% объёма вокруг POC). Вычислимо из `MdPayload::Trade`. M-24 отдаёт серию `volume_profile`
в `gateway::SeriesBundle` для кокпита (M-19: горизонтальная гистограмма + POC/VA-линии).

**В scope M-24 (SVP):** per-session гистограмма цена→объём (точные цены, не выдуманные) + POC + VAH/VAL +
VA%. **НЕ в scope (аддитивно позже):** CVP (cumulative), FRVP (fixed-range), Anchored/Composite, HVN/LVN
(локальные узлы — отдельная нюансная детекция). row-binning для дисплея — на фронте (бэкенд отдаёт точные цены).

## Contract impact (T1) — НЕТ; export v2 — T-designate аддитивно

- **T1-ядро НЕ трогаем** — читает существующий `MdPayload::Trade` (Граница A). CT-RFC не нужен.
- **`gateway::SeriesBundle` += поле `volume_profile: Vec<VolumeProfileRow>`** (аддитивно, В КОНЕЦ). Новый
  T-designate тип `VolumeProfileRow` в `crates/gateway` (не в contracts). **`GATEWAY_SCHEMA_VERSION` bump 3 → 4.**
  `Snapshot::apply` (bucket-merge) сливает `volume_profile` по `session_id` (аккумуляция гистограммы, не дубль).

### Контракт-форма `VolumeProfileRow` (engine-dev создаёт с ЭТИМИ полями)

```rust
pub struct VolumeProfileRow {
    pub session_id: i64,             // UTC-день = utc_session_id(ts_exch_ms) (VB-I-6)
    pub poc_e8: i64,                 // цена макс. объёма (Point of Control)
    pub vah_e8: i64,                 // Value Area High (верх 70%-зоны)
    pub val_e8: i64,                 // Value Area Low (низ 70%-зоны)
    pub va_pct_e8: i64,              // фактический % объёма в VA (≥70%), ×1e8
    pub bins: Vec<(i64, i64)>,       // (price_e8, volume_e8), СОРТ по price возр.; только ТОРГОВАННЫЕ цены
}
```

## Design (пиновка для engine-dev)

- **VP-аккумулятор в gateway `Reducer`** (рядом с vwap/ohlcv), стримовый fold над `journal::stream`
  (bounded — GW-I-2; state растёт с числом РАЗНЫХ цен сессии, не с числом событий).
- **Per-session (VB-I-6):** `session_id = utc_session_id(ts_exch_ms)`. Держи `BTreeMap<session_id,
  BTreeMap<price_e8, i128 volume>>`. На сделке (после venue/symbol-фильтра): `hist[session_id][price] += size`.
  Объём в i128 (Σ size на сессии может превысить i64? size_e8 суммарно — консервативно i128; детерминизм, без f64).
- **POC:** цена с макс. объёмом в сессии; **тай-брейк → НИЗШАЯ цена** (детерминизм).
- **Value Area (VAH/VAL) — АЛГОРИТМ (BINDING, детерминирован):**
  1. `bins` СОРТ по цене возр.; `total` = Σ volume; `target = (total·70).div_ceil(100)` (i128, ≥70%).
  2. Старт: VA = {POC}, `acc = vol(POC)`, индексы `lo=hi=idx(POC)`.
  3. Пока `acc < target` И есть куда расширять:
     - `above` = vol(bins[hi+1]) или 0; `below` = vol(bins[lo-1]) или 0;
     - если `above >= below` → добавить верхний (`hi+=1`, `acc+=above`), иначе нижний (`lo-=1`, `acc+=below`);
       (**тай `above==below` → берём ВЕРХНИЙ**);
     - если оба края исчерпаны → стоп.
  4. `vah_e8 = bins[hi].price`, `val_e8 = bins[lo].price`, `va_pct_e8 = acc·1e8 / total`.
- **Цены не выдумываются (VP-I-4, как footprint C-016):** ключи гистограммы — ТОЛЬКО цены реальных сделок;
  промежуточные «пустые» цены НЕ вставляются.
- **Per (venue, symbol)** — тем же `Reducer.apply`-фильтром (VP-I-5 ⊂ существующего поведения).

## Инварианты (RED, sacred — architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **VP-I-1** | **POC = цена макс. объёма** (тай → низшая); детерминизм (2 прогона идентичны). | `red_volume_profile.rs::vp_poc` |
| **VP-I-2** | **VAH/VAL по заданному VA-алгоритму** — на hand-computed фикстуре точные POC/VAH/VAL; `va_pct ≥ 70%`. Анти-плацебо: impl без расширения / с неверным выбором стороны → неверные VAH/VAL. | `red_volume_profile.rs::vp_value_area` |
| **VP-I-3** | **Session reset (VB-I-6).** Сделки по разные стороны UTC-полуночи → РАЗНЫЕ `VolumeProfileRow` (по `session_id`); объёмы не смешиваются между днями. | `red_volume_profile.rs::vp_session_reset` |
| **VP-I-4** | **Цены не выдумываются.** `bins` содержит ТОЛЬКО торгованные цены; цена без сделок отсутствует (не с нулевым объёмом). | `red_volume_profile.rs::vp_prices_not_invented` |

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-24-volume-profile.md`, `crates/gateway/tests/red_volume_profile.rs`,
  `scripts/verify_M-24.sh`.
- **engine-dev (impl, зона gateway):** `crates/gateway/src/lib.rs` — `VolumeProfileRow` тип + VP-аккумулятор в
  `Reducer` + поле `volume_profile` в `SeriesBundle` + `Snapshot::apply` merge (по session_id) + bump
  `GATEWAY_SCHEMA_VERSION` → 4.
- **Forbidden:** `crates/contracts` (T1), `crates/{risk,killswitch,oms,venue-*,journal,recorder}`, f64 в
  аккумуляции, `read_all`/`Vec<Event>` в gateway/src, order-path/сигнал.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ✅ | VP-I-1..4 RED (`red_volume_profile.rs`) + `verify_M-24.sh` (fmt+clippy+test, RN-17) | architect | compile-RED (нет `volume_profile`/`VolumeProfileRow`); достижимо; VA-алгоритм проверяется на hand-computed |
| 2 | ✅ | `VolumeProfileRow` тип + per-session VP-аккумулятор (i128, `utc_session_id`) | engine-dev | VP-I-1/VP-I-3/VP-I-4 GREEN |
| 3 | ✅ | POC + Value Area (VAH/VAL/va_pct) по §Design-алгоритму | engine-dev | VP-I-2 GREEN |
| 4 | ✅ | Поле `volume_profile` в `SeriesBundle` + `Snapshot::apply` merge + bump schema→4 | engine-dev | VP-* GREEN; GW-I-3/GW-I-5 не сломаны; workspace green |

## Гейты

- **critic — НЕ требуется** (низкий риск). **risk-critic N/A** (MD-only read-only). reviewer — UNCONDITIONAL.
- **verify_M-24.sh** ОБЯЗАН включать `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` (RN-17).
- **§8-lite:** gateway read-only, recorder-образ не меняется → прод инертен.

## Место в очереди

- **Второй пост-M-22** (founder: последовательно). Переиспользует VB-I-6 (M-20). Далее: **M-23 Heatmap**
  (тяжелее — L2Delta/book-реконструкция + TD-016 эвикция), затем **M-25** (liq/OI/funding). Блокирует M-19 (VP-панель).

## Handoff (план при старте)

architect (RED+verify+milestone) → engine-dev (tasks 2-4) → tester (чистый прогон incl `cargo fmt --all --check`;
бутстрап ТОЛЬКО с origin/feat/M-24 — RN-18) → reviewer (merge feat/M-24→main + §8-lite).
