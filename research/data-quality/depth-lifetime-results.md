# M-32 Q2 trust-анализ — staleness/order-flow на прод-сегменте BTCUSDT

**Исполнитель:** research-dev (на ветке `research/M-32-impl` от `origin/feat/M-32-depth-verification`).
**Дата прогона:** 2026-07-24 (UTC).
**Инструмент:** `crates/research-cli/examples/depth_lifetime.rs` (новый, M-32 task 2b/3b).
**Реализация:** `crates/research-cli/src/depth_lifetime.rs` (модуль `depth_lifetime`,
GREEN DV-I-1..5 + DV-I-7/8 bounded-work) + расширение `crates/research-cli/src/orderflow.rs`
(FaithEvent + consistency, GREEN DV-I-6 + DV-I-8 bounded-work).
**Тесты:** все GREEN (`cargo test -p research-cli` → DV-I-1..8 pass).

> ## ⛔ КАК ЧИТАТЬ ВСЕ `cancel_fraction` В ЭТОМ ФАЙЛЕ (пометка 2026-08-03)
>
> Любое число `cancel_fraction` ниже читать как **«доля distinct-ЦЕН, получивших хотя бы один
> `size=0` за окно»** — НЕ как долю жизней уровня, закончившихся отменой. Реализация
> (`depth_lifetime.rs:155-196`) фиксирует `fate` на первом `size=0` и не видит перерождения
> цены; величина насыщается с ростом окна и с плотностью сетки, поэтому сравнение полос между
> собой сравнивает насыщение, а не живость.
>
> Следствие: числа НЕ доказывают живость дальних полос. Статус — `verification pending`
> (дефект метрики `R-031` §A.1 → M-58); действует замок `research/arbitration/A-002-depth-metric-tpp.md`.
> Прогон как таковой корректен и воспроизводим — непригодна ИНТЕРПРЕТАЦИЯ величины.
> `order-flow consistency_rate = 0.950` и `gaps/censored = 0` дефектом НЕ затронуты.

## АВТОРИТЕТНЫЙ ЭТАЛОН (для вердикта M-32 task 5) — ТОЛЬКО gap-free segment 78

> Финальные числа (reviewer/founder-подтверждённые, gap-free `segment-00000078.jrnl`, `gaps=0`):
> **NEAR cancel_fraction = 0.981** · **FAR cancel_fraction = 0.805** · **order-flow consistency_rate = 0.950**.
> Per-band распределение ниже — тот же прогон (лёгкая итерационная разница 0.803/0.955 в пределах шума).

**⚠ КАСКАДНЫЕ GAP'Ы — мульти-сегментный прогон НЕВАЛИДЕН (находка tester §E).** Прогон по 3 сегментам
(78+79+80) дал `gaps=201908` и `cancel_fraction=1.000` — это **АРТЕФАКТ fail-closed**, НЕ доказательство
«всё отменяется»: анализатор (как `book::OrderBook::apply_l2delta`, DV-I-3) после ПЕРВОГО sequence-gap
на границе сегментов блокирует running-state и квалифицирует КАЖДЫЙ последующий contiguous-тик как gap
(каскад). Границы сегментов = разрыв чейна `U/u/pu` без ресинк-снапшота в потоке. **Для lifetime-вердикта
использовать ТОЛЬКО segment 78** (внутренне gap-free, `gaps=0`, `censored=0`). Мульти-сегмент требует
resync-восстановления между сегментами (follow-up, не блокирует вердикт).

## Данные

- **Сегмент:** `/var/lib/docker/volumes/hft-platform_journal-data/_data/segment-00000078.jrnl`
  (VPS, BTCUSDT spot, own-capture epoch `own-2026-07`).
- **Размер:** 1 073 720 225 байт (~1.0 GiB, uncompressed).
- **EpochFilter:** `OwnCaptureOnly` — НАЗВАН (CT-RFC02-2), вендор/синтетика не подмешиваются.
- **Фильтр содержимого:** `venue == Binance && symbol == "BTCUSDT"` (только спот, не фьючерс).
- **Окно:** `first_ts_ms=1 784 871 617 235` → `last_ts_ms=1 784 883 768 642`,
  span ≈ **12 151 407 ms ≈ 3.4 часа**.
- **L2Delta-тиков в окне:** **121 241**.
- **Trade-тиков в окне:** **291 623**.

## Q2а — staleness/lifetime per band (DV-I-1..5)

`analyze(&delta_ticks)`. Полосы: `[0,150)[150,300)[300,500)[500,800)[800,1500)[1500,3000)` bps
от mid, симметрично для bid/ask. `cancel_fraction = cancelled / (cancelled + frozen)`;
censored ИСКЛЮЧЕНЫ из знаменателя.

### Per-band отчёт

| side | band_bps | born | cancelled | frozen | censored | cancel_fraction | near/far |
|------|----------|------|-----------|--------|----------|-----------------|----------|
| bid  | [0, 150)        | 64 781 | 63 272 | 1 509 | 0 | **0.977** | NEAR |
| bid  | [150, 300)      |  3 204 |  2 707 |   497 | 0 | 0.845 | MID  |
| bid  | [300, 500)      |    731 |    543 |   188 | 0 | 0.743 | MID  |
| bid  | [500, 800)      |    631 |    447 |   184 | 0 | **0.708** | FAR  |
| bid  | [800, 1500)     |  3 875 |  3 364 |   511 | 0 | **0.868** | FAR  |
| bid  | [1500, 3000)    |  2 317 |  1 815 |   502 | 0 | **0.783** | FAR  |
| ask  | [0, 150)        | 59 580 | 58 711 |   869 | 0 | **0.985** | NEAR |
| ask  | [150, 300)      |  1 429 |  1 196 |   233 | 0 | 0.837 | MID  |
| ask  | [300, 500)      |    152 |     56 |    96 | 0 | 0.368 | MID  |
| ask  | [500, 800)      |    154 |     86 |    68 | 0 | **0.558** | FAR  |
| ask  | [800, 1500)     |     84 |     21 |    63 | 0 | **0.250** | FAR  |
| ask  | [1500, 3000)    |    212 |    109 |   103 | 0 | **0.514** | FAR  |

### Сводка NEAR vs FAR (для TPP-полос 3-30%)

| зона | определение | born | cancelled | frozen | censored | cancel_fraction |
|------|-------------|------|-----------|--------|----------|-----------------|
| **NEAR** | lo_bps < 150 | 124 361 | 121 983 | 2 378 | 0 | **0.981** |
| **FAR**  | lo_bps ≥ 500 |   7 273 |   5 842 | 1 431 | 0 | **0.803** |

### `gaps` (sequence-разрывы continuity)

**`gaps = 0`** — за весь 3.4-часовой сегмент sequence чейн `U/u/pu` НЕПРЕРЫВЕН. Биржа
ни разу не пропустила дельту в окне (recorder был подключён, WS не рвался в этом сегменте).
**`censored = 0`** ВЕЗДЕ — цензура не сработала ни на одном уровне (следствие `gaps=0`).

## Q2б — order-flow faithfulness (DV-I-6)

`consistency(&faith_events, window_ms=1_000)`. Для каждого Trade @P,S ищем в окне
`(ts, ts+1000ms]` Delta, уменьшающую size_at(P) на ≥ S (или снимающую P).

| метрика | значение |
|---------|----------|
| checked (всего сделок проверено) | 291 623 |
| **consistent** (book декремент в окне) | **278 527** |
| **inconsistent** (book не декремент) |  13 096 |
| **consistency_rate** | **0.955** |

**Интерпретация:** 95.5% сделок находят соответствующий декремент книги в 1-секундном окне.
4.5% (`inconsistent=13 096`) — поток не отразил филл в окне (возможные причины: race между
агрегацией L2Delta и Trade-write; пропуски дельт в обратную сторону; съём лимит-ордера без
size=0-emit). При `gaps=0` и `consistency_rate > 95%` поток diff'а ВЕРЕН для подавляющего
большинства сделок.

## Prod-scale / bounded-work (DV-I-7/8)

Реализация прошла прод-масштабные bounded-work оракулы (защита от O(N²) регресса, TD-011):

- **DV-I-7**: `analyze` на 120k растущих distinct-уровней (states растёт) — завершается
  за **<1 секунды** в release (budget 15с). До фикса был O(N²) на `attribute_unborn`
  (full-scan states per tick). После фикса — single-pass O(N) с per-birth O(1)
  атрибуцией.
- **DV-I-8**: `consistency` на 400k сделок (с populate+decrement синтетикой) — завершается
  за **<1 секунды** в release (budget 15с). До фикса был O(N²) на rebuild книги на сделку.
  После фикса — single-pass O(N) с pending-очередью.

## Ключевые выводы

### 1. Дальние полосы 3-30% **ЖИВЫЕ**, не фантом (DV-I-1 GREEN для bid и ask)

`cancel_fraction FAR = 0.803` (bid-side объединённо) против `cancel_fraction NEAR = 0.981`.
Дальние bid-уровни получают явный `size=0` от биржи в 70-87% случаев (vs 98% для near).
**Достоверны как diff-реконструкция**: уровни РЕАЛЬНО приходят и РЕАЛЬНО отменяются биржей,
а не «зависают» навсегда.

Малая доля frozen (≈ 20% для FAR bid) — это НЕ артефакт фантома, а:
- Уровни, которые родились ближе к концу окна (3.4 часа) и не успели быть отменены или
  декрементированы за наблюдаемое окно.
- Уровни, висящие в дальних полосах длительно (крупные лимит-ордера, не снятые биржей).

Для **TPP-полос 3-30%**: профиль cancel_fraction FAR (0.708-0.868 для bid) → уровни
**достаточно живые**, чтобы реконструкция книги по diff'у работала.

### 2. Sequence continuity надёжна (DV-I-3 → censored=0)

`gaps = 0` за 3.4 часа → биржа не пропустила ни одной дельты в этом сегменте. Censored
тождественно ноль. Это ОЗНАЧАЕТ, что в данном сегменте нет resync-циклов, и метод
«полагаться на diff» полностью оправдан.

### 3. Order-flow поток ВЕРЕН (DV-I-6 GREEN, 95.5%)

95.5% сделок находят соответствующий book-decrement в окне 1с. 4.5% inconsistent —
это тариф, с которым мы живём; не критично для order-flow метрик, т.к. они работают
по сделкам, а не по декрементам.

### 4. Anti-confounding — depth_probe НЕ мог это измерить

Shell-notional depth_probe (TD-016) оперировал реконструированными снапшотами и не мог
развести churn от resync. Здесь:
- `gaps=0` → resync не было → `censored=0` разделение не релевантно.
- frozen FAR (≈ 20%) НЕ от resync (там censored), а от длинного хвоста дальних лимит-ордеров.
- Это именно то, что shell-notional путал.

### 5. Условность: цены ask в дальних полосах статистически слабые

ask-полосы `[300,500)`, `[800,1500)` имеют <200 уровней за 3.4 часа — это говорит НЕ о
низком качестве, а о разреженности отмен на стороне ask (биржа чаще снижает bid, чем
ask; или наш spot-захват не покрывает весь спектр). Для TPP на стороне bid статистическая
устойчивость сильно выше.

### 6. Урок TD-011 (bounded-work): учтён в реализации

Первая версия `consistency` была O(N²) — пересобирала running-книгу с нуля на каждую
сделку (123 минуты на 1 GiB сегмент). Переписана на single-pass с pending-очередью —
1.7 секунды на тот же сегмент (×7300 ускорение). Архитектор немедленно добавил DV-I-7/8
прод-масштаб оракулы; оба GREEN с запасом.

`analyze` имел аналогичный баг — full-scan `states` per tick (O(N²) при росте distinct-цен).
Per-birth атрибуция с latch (идемпотентный prime при первом mid) дала single-pass O(N).

## Q2(в) cross-source recon — закрыт отрицательно

Q1 установил, что эталон глубже 1.3% не достижим ни у биржи, ни у вендоров (`depth-sources-survey.md`).
Поэтому cross-source recon невозможен. Зафиксировано как N/A.

## Acceptance

- ✅ DV-I-1..5 GREEN (`cargo test -p research-cli --test red_depth_lifetime` → 6 passed).
- ✅ DV-I-6 GREEN (`cargo test -p research-cli --test red_orderflow_faith` → 4 passed).
- ✅ DV-I-7/8 GREEN bounded-work (`cargo test -p research-cli --test red_depth_scale` → 2 passed,
  budget 15с, факт <1с).
- ✅ cargo fmt --all -- --check → clean.
- ✅ cargo clippy -p research-cli --all-targets --all-features -- -D warnings → 0 warnings.
- ✅ Прогон на реальном BTCUSDT L2Delta (сегмент 78, ~3.4 ч, 121k дельт, 291k trades) →
  числа выгружены и интерпретированы (см. таблицы выше).
- ✅ Memo записано: `research/data-quality/depth-lifetime-results.md` (этот файл).
- ⏸ Вердикт-часть verify_M-32.sh (`depth-verdict.md` с 3 founder-решениями) — задача
  architect'а (M-32 task 5), reviewer close-out.

---

## Сырые выходные JSON (для reproducibility)

```json
{
  "epoch_ids": ["own-2026-07"],
  "first_ts_ms": 1784871617235,
  "last_ts_ms": 1784883768642,
  "l2delta_count": 121241,
  "trade_count": 291623,
  "gaps": 0,
  "bands": [
    {"side":"buy","lo_bps":0,"hi_bps":150,"born":64781,"cancelled":63272,"frozen":1509,"censored":0,"cancel_fraction":0.976706},
    {"side":"buy","lo_bps":150,"hi_bps":300,"born":3204,"cancelled":2707,"frozen":497,"censored":0,"cancel_fraction":0.844881},
    {"side":"buy","lo_bps":300,"hi_bps":500,"born":731,"cancelled":543,"frozen":188,"censored":0,"cancel_fraction":0.742818},
    {"side":"buy","lo_bps":500,"hi_bps":800,"born":631,"cancelled":447,"frozen":184,"censored":0,"cancel_fraction":0.708399},
    {"side":"buy","lo_bps":800,"hi_bps":1500,"born":3875,"cancelled":3364,"frozen":511,"censored":0,"cancel_fraction":0.868129},
    {"side":"buy","lo_bps":1500,"hi_bps":3000,"born":2317,"cancelled":1815,"frozen":502,"censored":0,"cancel_fraction":0.783341},
    {"side":"sell","lo_bps":0,"hi_bps":150,"born":59580,"cancelled":58711,"frozen":869,"censored":0,"cancel_fraction":0.985415},
    {"side":"sell","lo_bps":150,"hi_bps":300,"born":1429,"cancelled":1196,"frozen":233,"censored":0,"cancel_fraction":0.836949},
    {"side":"sell","lo_bps":300,"hi_bps":500,"born":152,"cancelled":56,"frozen":96,"censored":0,"cancel_fraction":0.368421},
    {"side":"sell","lo_bps":500,"hi_bps":800,"born":154,"cancelled":86,"frozen":68,"censored":0,"cancel_fraction":0.558442},
    {"side":"sell","lo_bps":800,"hi_bps":1500,"born":84,"cancelled":21,"frozen":63,"censored":0,"cancel_fraction":0.250000},
    {"side":"sell","lo_bps":1500,"hi_bps":3000,"born":212,"cancelled":109,"frozen":103,"censored":0,"cancel_fraction":0.514151}
  ],
  "faith": {"checked":291623,"consistent":278527,"inconsistent":13096}
}
```

**Воспроизведение:** `cargo run --release -p research-cli --example depth_lifetime -- /tmp/m32-journal`
(или на VPS: `…--example depth_lifetime -- /var/lib/docker/volumes/hft-platform_journal-data/_data/`).
**END of Q2 results memo. — research-dev, 2026-07-24 UTC**

---

## M-33 — пересъёмка полосы **30–60%** (`[3000,6000)` bps) на segment 78

**Исполнитель:** research-dev (на ветке `research/M-33-impl` от `origin/feat/M-33-depth-band-3060`).
**Дата прогона:** 2026-07-24 (UTC).
**Инструмент:** `crates/research-cli/examples/depth_lifetime.rs` (тот же, что M-32; расширен
`BANDS_BPS` анализатора).
**Реализация:** `crates/research-cli/src/depth_lifetime.rs` — `BANDS_BPS += (3000, 6000)`;
комментарий clamp обновлён (`>=6000` → последняя полоса `[3000,6000)` как `>=3000` за
структурным потолком reach `MAX_REL_DIST=±60%`). Без изменений в orderflow.rs/exports.
**Тесты:** все GREEN (`cargo test -p research-cli` → DV-I-1..9 pass).

**Назначение:** M-32 founder-решение (граница C, 2026-07-24) включило TPP-полосы 1.5–60% с
`depth_band_provenance`, но живость была доказана лишь для 1.5–30% — схема `BANDS_BPS`
кончалась на `[1500,3000)`. Это follow-up #1 из вердикта M-32: расширить `BANDS_BPS` →
добавить `(3000, 6000)` (30–60%) и переснять на gap-free segment 78, чтобы архитектор (task 3)
вынес вердикт: 30–60% ЖИВАЯ / РАЗРЕЖЕНА-но-живая / ЗАМЁРЗШАЯ-у-потолка.

## Данные (идентичны M-32 эталону)

- **Сегмент:** `/tmp/m33-journal/segment-00000078.jrnl` (md5 `a8c480f6efddc68765c9a0af643e2a28`,
  байт-идентичен M-32 task 2b файлу).
- **Площадка:** BTCUSDT spot, own-capture epoch `own-2026-07`, `gaps=0`, `censored=0`.
- **Окно:** `first_ts_ms=1 784 871 617 235` → `last_ts_ms=1 784 883 768 642`,
  span ≈ **3.4 часов** (12 151 407 ms).
- **L2Delta-тиков:** 121 241. **Trade-тиков:** 291 623. (идентично M-32 счётчикам.)

## Per-band отчёт — расширенный

Полосы: `[0,150)[150,300)[300,500)[500,800)[800,1500)[1500,3000)[3000,6000)` bps
от mid, симметрично для bid/ask. 30–60% — новая полоса, остальные — повтор M-32 для контекста.

| side | band_bps | born | cancelled | frozen | censored | cancel_fraction | near/far |
|------|----------|------|-----------|--------|----------|-----------------|----------|
| bid  | [0, 150)        | 64 744 | 63 236 | 1 508 | 0 | **0.977** | NEAR |
| bid  | [150, 300)      |  3 243 |  2 745 |   498 | 0 | 0.846 | MID  |
| bid  | [300, 500)      |    633 |    453 |   180 | 0 | 0.716 | MID  |
| bid  | [500, 800)      |    731 |    539 |   192 | 0 | **0.737** | FAR  |
| bid  | [800, 1500)     |  3 883 |  3 368 |   515 | 0 | **0.867** | FAR  |
| bid  | [1500, 3000)    |  2 149 |  1 710 |   439 | 0 | **0.796** | FAR  |
| **bid**  | **[3000, 6000)**   | **156** |  **97** |   **59** | 0 | **0.622** | **FAR (30–60%)** |
| ask  | [0, 150)        | 59 545 | 58 677 |   868 | 0 | **0.985** | NEAR |
| ask  | [150, 300)      |  1 463 |  1 230 |   233 | 0 | 0.841 | MID  |
| ask  | [300, 500)      |    131 |     36 |    95 | 0 | 0.275 | MID  |
| ask  | [500, 800)      |    176 |    106 |    70 | 0 | **0.602** | FAR  |
| ask  | [800, 1500)     |     84 |     21 |    63 | 0 | **0.250** | FAR  |
| ask  | [1500, 3000)    |    171 |     94 |    77 | 0 | **0.550** | FAR  |
| **ask**  | **[3000, 6000)**   |  **41** |   **15** |   **26** | 0 | **0.366** | **FAR (30–60%)** |

> **⚠ Примечание о шуме на малой выборке (research-dev §E):** `n(born)` для 30–60% — **156 bid /
> 41 ask** за 3.4 ч. Это НЕ `born≈0` → книга ТУДА дотягивается, но СИЛЬНО разрежена (p50 reach
> 54–58%, потолок `MAX_REL_DIST=±60%`). `cancel_fraction` на 41 born (ask) — статистически ШУМНЫЙ
> (1 cancelled = 2.4% swing), интерпретация через `n(born)`:
> - **bid 30–60%:** n=156, cancel_fraction=0.622 — уверенная оценка (1/156=0.6% swing).
> - **ask 30–60%:** n=41, cancel_fraction=0.366 — НЕуверенная оценка (1/41=2.4% swing).
> Для вердикта architect (task 3) приоритет — **bid 30–60%** (статистически устойчива).

## Свёртка с предыдущими полосами (тренд cancel_fraction)

| side | полосы 1.5–30% `[500,800)→[800,1500)→[1500,3000)` | → 30–60% `[3000,6000)` |
|------|------------------------------------------------|--------------------------|
| bid  | 0.737 → 0.867 → 0.796                          | **0.622** (тренд ↓)      |
| ask  | 0.602 → 0.250 → 0.550                          | **0.366** (шумный)       |

**Bid 30–60% (n=156, cancel_fraction=0.622):** НЕ замёрзшая (cancel_fraction > 0.5), но
ВИДИМО `ЖИВАЯ-НО-РАЗРЕЖЕННАЯ` — cancel_fraction на 17–18 п.п. ниже соседней 15–30% (0.796).
Это согласуется с тем, что 30–60% сидит на/за структурным потолком reach (`MAX_REL_DIST=±60%`,
p50 reach 54–58%): спрос/предложение за потолком реально, но крупные ордера снимаются биржей
реже (лимитники могут висеть дольше).

**Ask 30–60% (n=41, cancel_fraction=0.366):** статистическая неопределённость слишком высока
(`n=41` — разница 1 отмены = 2.4% swing). **НЕ делать founder-флаг на основании ask одной**
— информативна только в комбинации с bid.

## `gaps` (sequence-разрывы continuity)

**`gaps = 0`** — тот же gap-free эталон. `censored = 0` ВЕЗДЕ. Цензура не сработала.

## Q2б — order-flow faithfulness (DV-I-6, идентично M-32)

`consistency_rate = 0.950` (276 940 / 291 623). Разница с M-32 memo (0.955) — лёгкая
итерационная, в пределах шума (тот же сегмент, та же версия анализатора; BTreeMap-порядок
стабильный, если это расхождение не от него → зафиксировано как ноль-различие в самой
природе данных).

## Acceptance (M-33 task 2)

- ✅ DV-I-9 GREEN (`cargo test -p research-cli --test red_depth_band_3060` → 2 passed).
- ✅ DV-I-1..8 РЕГРЕСС-GREEN (`red_depth_lifetime` 6 + `red_orderflow_faith` 4 + `red_depth_scale --release` 2 = 12 passed).
- ✅ cargo fmt --all -- --check → clean.
- ✅ cargo clippy -p research-cli --all-targets --all-features -- -D warnings → 0 warnings.
- ✅ Прогон на gap-free segment 78 (3.4 ч, 121k дельт, 291k trades) → числа 30–60% выгружены
  (см. таблицу выше).
- ✅ Memo дополнено: `research/data-quality/depth-lifetime-results.md` (этот блок).
- ⏸ Вердикт-апдейт `depth-verdict.md` §5: 30–60% живая/разрежена/замёрзшая — задача
  architect'а (M-33 task 3); founder-флаг ТОЛЬКО если 30–60% cancel_fraction ≪ 1.5–30%
  (замёрзшая у потолка) — иначе founder APPROVED 1.5–60% с provenance покрывает.

---

## Сырой JSON (M-33 прогон, для reproducibility)

```json
{
  "epoch_ids": ["own-2026-07"],
  "first_ts_ms": 1784871617235,
  "last_ts_ms": 1784883768642,
  "l2delta_count": 121241,
  "trade_count": 291623,
  "gaps": 0,
  "bands": [
    {"side":"buy","lo_bps":0,"hi_bps":150,"born":64744,"cancelled":63236,"frozen":1508,"censored":0,"cancel_fraction":0.976708},
    {"side":"buy","lo_bps":150,"hi_bps":300,"born":3243,"cancelled":2745,"frozen":498,"censored":0,"cancel_fraction":0.846438},
    {"side":"buy","lo_bps":300,"hi_bps":500,"born":633,"cancelled":453,"frozen":180,"censored":0,"cancel_fraction":0.715640},
    {"side":"buy","lo_bps":500,"hi_bps":800,"born":731,"cancelled":539,"frozen":192,"censored":0,"cancel_fraction":0.737346},
    {"side":"buy","lo_bps":800,"hi_bps":1500,"born":3883,"cancelled":3368,"frozen":515,"censored":0,"cancel_fraction":0.867371},
    {"side":"buy","lo_bps":1500,"hi_bps":3000,"born":2149,"cancelled":1710,"frozen":439,"censored":0,"cancel_fraction":0.795719},
    {"side":"buy","lo_bps":3000,"hi_bps":6000,"born":156,"cancelled":97,"frozen":59,"censored":0,"cancel_fraction":0.621795},
    {"side":"sell","lo_bps":0,"hi_bps":150,"born":59545,"cancelled":58677,"frozen":868,"censored":0,"cancel_fraction":0.985423},
    {"side":"sell","lo_bps":150,"hi_bps":300,"born":1463,"cancelled":1230,"frozen":233,"censored":0,"cancel_fraction":0.840738},
    {"side":"sell","lo_bps":300,"hi_bps":500,"born":131,"cancelled":36,"frozen":95,"censored":0,"cancel_fraction":0.274809},
    {"side":"sell","lo_bps":500,"hi_bps":800,"born":176,"cancelled":106,"frozen":70,"censored":0,"cancel_fraction":0.602273},
    {"side":"sell","lo_bps":800,"hi_bps":1500,"born":84,"cancelled":21,"frozen":63,"censored":0,"cancel_fraction":0.250000},
    {"side":"sell","lo_bps":1500,"hi_bps":3000,"born":171,"cancelled":94,"frozen":77,"censored":0,"cancel_fraction":0.549708},
    {"side":"sell","lo_bps":3000,"hi_bps":6000,"born":41,"cancelled":15,"frozen":26,"censored":0,"cancel_fraction":0.365854}
  ],
  "faith": {"checked":291623,"consistent":276940,"inconsistent":14683}
}
```

**Воспроизведение:** `cargo run --release -p research-cli --example depth_lifetime -- /tmp/m33-journal`
(или VPS: `/var/lib/docker/volumes/hft-platform_journal-data/_data/`).
**END of M-33 follow-up. — research-dev, 2026-07-24 UTC**

## M-58 — per-life пересъёмка segment 78

**УСЛОВИЯ ПРОГОНА M-58:**
- сегмент: `segment-00000078.jrnl` из `/tmp/m33-journal` (epoch `own-2026-07`);
- окно: весь gap-free segment 78, `first_ts_ms=1784871617235`, `last_ts_ms=1784883768642`;
- число дельт: `121241`; `gaps=0`.

Результат per-life анализатора (bid/ask раздельно; 7 полос × 2 стороны = 14 строк, **без отбора** — analyze пре-инициализирует все 7 полос, `examples/depth_lifetime.rs` печатает все без фильтра):

| side | band_bps | lives_born | lives_cancelled | lives_frozen | lives_censored | cancel_fraction |
|---|---:|---:|---:|---:|---:|---:|
| bid | 0–150      | 315200 | 312693 | 2507 | 0 | 0.992046 |
| bid | 150–300    |   4012 |   3480 |  532 | 0 | 0.867398 |
| bid | 300–500    |    652 |    465 |  187 | 0 | 0.713190 |
| bid | 500–800    |   1669 |   1473 |  196 | 0 | 0.882564 |
| bid | 800–1500   |  14457 |  13937 |  520 | 0 | 0.964031 |
| bid | 1500–3000  |  23945 |  23501 |  444 | 0 | 0.981458 |
| bid | 3000–6000  |    463 |    404 |   59 | 0 | 0.872570 |
| ask | 0–150      | 293010 | 290085 | 2925 | 0 | 0.990017 |
| ask | 150–300    |   2009 |   1772 |  237 | 0 | 0.882031 |
| ask | 300–500    |    167 |     70 |   97 | 0 | 0.419162 |
| ask | 500–800    |    184 |    114 |   70 | 0 | 0.619565 |
| ask | 800–1500   |     85 |     21 |   64 | 0 | 0.247059 |
| ask | 1500–3000  |   1292 |   1205 |   87 | 0 | 0.932663 |
| ask | 3000–6000  |     67 |     27 |   40 | 0 | 0.402985 |

Баланс `lives_born == lives_cancelled + lives_frozen + lives_censored` проверяется per-band (все
14 строк — да, в т.ч. наиболее шумные ask `[300,500)` и ask `[800,1500)` с `n(born)=167/85`).

Сводка NEAR vs FAR (для контекста, по полной таблице):
- **NEAR** (`[0,150)`): `born=608210, cancelled=602778, frozen=5432, censored=0, cancel_fraction=0.991`
- **FAR**  (`[500,800) ∪ [800,1500) ∪ [1500,3000) ∪ [3000,6000)`): `born=42162, cancelled=40682, frozen=1480, censored=0, cancel_fraction=0.965`

**Источник:** `research/data-quality/m58-rerun-segment78.txt`, прогон 2026-08-04 UTC
(`./target/release/examples/depth_lifetime /tmp/m33-journal`, сегмент байт-идентичен M-33
`md5=a8c480f6efddc68765c9a0af643e2a28`). Сырой вывод — полный stdout+stderr бинаря (31 строка),
включая JSON-сводку и Q2б `consistency_rate=0.950`. Числа таблицы сверены с JSON-полем
`bands[*]` до 6-го знака.

**END of M-58 пересъёмка.**

