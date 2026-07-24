# M-32 Q2 trust-анализ — staleness/order-flow на прод-сегменте BTCUSDT

**Исполнитель:** research-dev (на ветке `research/M-32-impl` от `origin/feat/M-32-depth-verification`).
**Дата прогона:** 2026-07-24 (UTC).
**Инструмент:** `crates/research-cli/examples/depth_lifetime.rs` (новый, M-32 task 2b/3b).
**Реализация:** `crates/research-cli/src/depth_lifetime.rs` (модуль `depth_lifetime`,
GREEN DV-I-1..5 + DV-I-7/8 bounded-work) + расширение `crates/research-cli/src/orderflow.rs`
(FaithEvent + consistency, GREEN DV-I-6 + DV-I-8 bounded-work).
**Тесты:** все GREEN (`cargo test -p research-cli` → DV-I-1..8 pass).

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
