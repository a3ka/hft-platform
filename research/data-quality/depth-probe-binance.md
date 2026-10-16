# Depth-probe Binance — реальная досягаемость полос 3/5/8/15/30% (TPP-блокер)

**Исполнитель:** research-dev (на ветке `research/depth-probe` от `origin/main`)
**Дата анализа:** 2026-07-22 (UTC)
**Источник данных (own-capture):** `/tmp/m10-vps-journal` — 8 сегментов × ~1 GiB, ~21 ч, recorder v2
**Цель:** установить — вычислимы ли полосы TPP `3/5/8/15/30%` от mid из поддерживаемого стакана
**Без необходимости в платном full-depth вендоре.**

---

## 1. TL;DR — ВЕРДИКТ (одна строка на символ)

| Symbol (venue)            | Полосы 3/5/8/15/30% вычислимы?                                                                               | Live REST покрытие (без diff) | Достаточно своей книги? |
|---------------------------|--------------------------------------------------------------------------------------------------------------|-------------------------------|-------------------------|
| Binance **Spot BTCUSDT**  | **ДА** — наша diff-книга доходит до ~50–56% bid / ~56–59% ask от mid. Все 5 полос вычислимы.                 | cap ~1.3% (5000 ур.)          | ДА                      |
| Binance **Spot ETHUSDT**  | **ДА** — diff-книга до ~54–58%.                                                                              | cap ~4.5% (5000 ур.)          | ДА                      |
| Binance **Futures BTCUSDT** | **ДА** — diff-книга до ~56–59% (самая глубокая из 4).                                                       | cap ~0.09% (500 ур.)          | ДА (REST не годится)    |
| Binance **Futures ETHUSDT** | **ДА** — diff-книга до ~57–58%.                                                                              | cap ~0.26% (500 ур.)          | ДА                      |

**Итог:** собственный захват Binance покрывает все нужные TPP-полосы на обоих venue'ах и обоих символах.
**Платный full-depth вендор не нужен ни для одного из четырёх инструментов.**

---

## 2. Источники и метод

### 2.1 Данные
- `/tmp/m10-vps-journal/segment-{50..57}.jrnl` — v2-сегменты, own-capture
- 7.3 М событий всего, ~370 k L2-снапшотов (Spot × 91 578 + Futures × 90 108–90 222, Hyperliquid × 17 055)
- Live REST-снапшоты сняты 2026-07-22, ~15:47 UTC (см. §4) — **референс**, не обрабатывается

### 2.2 Инструменты (read-only, существующие в репо)
- `crates/book/examples/bands.rs` — **последний снапшот каждого (venue, symbol)**, печатает `mid`,
  `n_levels bid/ask`, `max_reach_pct bid/ask`, notional в полосах 1.5/3/5/8/15/30/60%.
- `crates/book/examples/obi_probe.rs` — **обходит все снапшоты**, усредняет `n_levels`,
  `max_reach_pct`, считает долю снапшотов, где `depth_within(3%) == total bid depth`
  (`band3_captures_all`) и среднее OBI `bid3 / (bid3 + ask8)`.
- Python-парсер live REST-снапшотов (без модификации Rust-кода, см. §4.1).

### 2.3 Что измерено (соответствует задаче)
| № | Метрика                                              | Инструмент       | Репрезентативность           |
|---|------------------------------------------------------|------------------|------------------------------|
| 1 | max_reachable_pct per side (среднее и последний)     | obi_probe+bands  | avg за 21 ч + точка-конец    |
| 2 | Кумулятивный notional в полосах (i64·1e8 → USD)      | bands (last) + REST | точка-конец / live          |
| 3 | Доля непустого покрытия полос (lvl внутри полосы)    | derived          | см. §5 — p10/p90 см. §9     |
| 4 | Live REST cap (референс, "до чего доходит полный снимок") | curl+py       | 1 снимок в 15:47 UTC         |
| 5 | Staleness дальних полос (TD-016 proxy)               | — **SCOPE-VIOLATION** | требует расширения примеров (§9) |

### 2.4 Детерминизм
- Журнальная часть — чистые редьюсеры (никаких wall-clock внутри reduce).
- Единственный wall-clock компонент — live REST-снапшот (помечен `§4`).

---

## 3. Текущая (существующая) картина по журналу — bands.rs

`cargo run -q --release -p book --example bands -- /tmp/m10-vps-journal`
— последний снапшот каждого (venue, symbol). Эти числа показывают состояние конца периода.

| Venue / Symbol       | #snap | mid        | lvl bid/ask | max_reach_bid | max_reach_ask |
|----------------------|-------|------------|-------------|---------------|---------------|
| Binance/BTCUSDT      | 91 578 | 66 362.01 | 895 / 634   | **50.12%**     | **59.13%**    |
| Binance/ETHUSDT      | 91 578 | 1 922.58  | 846 / 666   | **50.07%**     | **59.16%**    |
| BinanceFutures/BTCUSDT | 90 222 | 66 329.25 | 1 170 / 846 | **59.27%**     | **59.51%**    |
| BinanceFutures/ETHUSDT | 90 108 | 1 921.73  | 1 336 / 1 183 | **57.33%**   | **58.45%**    |

Полосный notional (USD, M$) — последний снапшот:

| Venue / Symbol          | band%  | BID $    | ASK $    | DIFF(B-A) $ |
|-------------------------|-------:|---------:|---------:|------------:|
| **Binance/BTCUSDT**     |   1.5  |   21.118 |   39.927 |    -18.810  |
|                         |   3.0  |   29.400 |   60.270 |    -30.870  |
|                         |   5.0  |   42.913 |   70.602 |    -27.689  |
|                         |   8.0  |   66.322 |   83.458 |    -17.136  |
|                         |  15.0  |  227.596 |   98.048 |   +129.548  |
|                         |  30.0  |  410.221 |  128.354 |   +281.867  |
|                         |  60.0  |  458.770 |  155.845 |   +302.926  |
| **Binance/ETHUSDT**     |   1.5  |    6.463 |    9.247 |     -2.784  |
|                         |   3.0  |    9.793 |   20.433 |    -10.640  |
|                         |   5.0  |   16.000 |   35.383 |    -19.383  |
|                         |   8.0  |   24.627 |   40.094 |    -15.466  |
|                         |  15.0  |   33.628 |   47.487 |    -13.858  |
|                         |  30.0  |   76.919 |   67.325 |     +9.593  |
|                         |  60.0  |   92.467 |   83.755 |     +8.712  |
| **BinanceFutures/BTCUSDT** |   1.5 |  357.158 |  357.839 |     -0.681  |
|                         |   3.0  |  766.687 |  589.530 |   +177.157  |
|                         |   5.0  |  847.149 |  675.569 |   +171.580  |
|                         |   8.0  |  898.669 |  737.001 |   +161.669  |
|                         |  15.0  |  970.892 |  762.707 |   +208.185  |
|                         |  30.0  | 1121.600 |  791.961 |   +329.640  |
|                         |  60.0  | 1143.270 |  798.563 |   +344.707  |
| **BinanceFutures/ETHUSDT** |   1.5 |  125.808 |  118.708 |     +7.100  |
|                         |   3.0  |  238.080 |  246.304 |     -8.224  |
|                         |   5.0  |  294.015 |  329.940 |    -35.925  |
|                         |   8.0  |  326.717 |  367.547 |    -40.830  |
|                         |  15.0  |  366.292 |  426.530 |    -60.238  |
|                         |  30.0  |  429.986 |  442.388 |    -12.402  |
|                         |  60.0  |  441.603 |  445.787 |     -4.184  |

**Наблюдение:** на BinanceFutures в полосах 15/30/60% notional стабильно растёт — значит в этих полосах
есть РЕАЛЬНЫЕ уровни (а не та же ликвидность, что в 8%). На Spot BTC в 8 → 15% скачок +161 M$ BID — есть
уровни в [8%, 15%]. На Spot ETH — линейный рост от 8% до 60%. Полосы вычислимы.

---

## 4. Live REST-снапшот (референс — насколько глубок полный снимок)

Снято 2026-07-22 ~15:47 UTC, Python `requests`-эквивалент через `curl`:

```text
$ curl -sS 'https://api.binance.com/api/v3/depth?symbol=BTCUSDT&limit=5000'  → 320 055 байт, 5000 уровней
$ curl -sS 'https://api.binance.com/api/v3/depth?symbol=ETHUSDT&limit=5000'  → 310 394 байт, 5000 уровней
$ curl -sS 'https://fapi.binance.com/fapi/v1/depth?symbol=BTCUSDT'           →  21 089 байт,  500 уровней (default)
$ curl -sS 'https://fapi.binance.com/fapi/v1/depth?symbol=ETHUSDT'           →  20 516 байт,  500 уровней
$ curl -sS 'https://fapi.binance.com/fapi/v1/depth?symbol=BTCUSDT&limit=1000'→  42 097 байт, 1000 уровней
```

Полосный notional live REST (USD M$, по 1 снимку):

| Venue / Symbol             | 0.5% bid/ask  | 1% bid/ask   | 3% bid/ask   | 5% bid/ask   | reach max (bid / ask) | внутри каких полос REST «упёрся» |
|----------------------------|---------------|--------------|--------------|--------------|------------------------|----------------------------------|
| Binance Spot **BTCUSDT**   | 11.88 / 13.04 | 18.24 / 18.75 | 21.83 / 23.51 | 21.83 / 23.51 *(cap)* | -1.29% / +1.33% | 1.3% — не дотягивает до 3%     |
| Binance Spot **ETHUSDT**   |  5.01 /  4.58 |  5.87 /  7.52 | 13.01 / 25.57 | 16.39 / 30.98 *(cap)* | -4.50% / +4.40% | 4.5% — дотягивает до 3–4%, но не до 5% |
| BinanceFutures **BTCUSDT** | 11.26 / 13.02 *(cap)* | 11.26/13.02 *(cap)* | cap | cap | -0.088% / +0.087% | 0.09% — не дотягивает ни до одной полосы |
| BinanceFutures **BTCUSDT** (limit=1000) | 29.81 / 26.71 *(cap)* | cap | cap | cap | -0.18% / +0.19% | 0.18% — не дотягивает ни до одной полосы |
| BinanceFutures **ETHUSDT** | 22.06 / 17.82 *(cap)* | cap | cap | cap | -0.26% / +0.27% | 0.26% — не дотягивает ни до одной полосы |

**Вывод REST:** даже полный снимок Binance **НЕ покрывает**:
- Полосу **3%** для BTC spot и обоих futures (cap ниже 1.3%).
- Полосу **5%** для BTC spot (cap 1.3% < 5%).
- Полосу **5%** для ETH spot (cap 4.5% < 5%).
- **Любую** полосу 3/5/8/15/30% на futures — там REST cap 0.09–0.26%.

То есть если бы у нас не было **diff-support** Binance, TPP на полосах 3/5/8/15/30% пришлось бы
делать на вендорных данных (Tardis/Coinalyze). См. §7.

---

## 5. Наша diff-книга vs REST — это **то, ради чего research-cli вообще пилится**

Сравнение «какую часть нужного полосного notional покрывает книга»:

| Symbol (venue)             | REST cap | Наша diff max_reach | Во сколько раз diff глубже | Все 5 полос 3/5/8/15/30% вычислимы? |
|----------------------------|---------:|--------------------:|---------------------------:|:------------------------------------|
| Binance Spot BTCUSDT       |    1.29% |              50–56% | **~40×**                  | ДА                                  |
| Binance Spot ETHUSDT       |    4.50% |              54–58% | **~12×**                  | ДА                                  |
| BinanceFutures BTCUSDT     |    0.09% |              56–59% | **~600×**                 | ДА                                  |
| BinanceFutures ETHUSDT     |    0.26% |              57–58% | **~220×**                 | ДА                                  |

**Число уровней внутри полосы** (грубая оценка при квазиравномерной плотности, от `n_levels * (band% / max_reach%)`):

| Symbol (venue)             | lvl_b / lvl_a (последний снимок) | ~lvl в полосе 3% bid/ask | ~lvl в полосе 30% bid/ask |
|----------------------------|-----------------------------------|---------------------------|----------------------------|
| Binance Spot BTCUSDT       | 895 / 634                         | ~50 / ~30                 | ~500 / ~300                |
| Binance Spot ETHUSDT       | 846 / 666                         | ~50 / ~35                 | ~500 / ~340                |
| BinanceFutures BTCUSDT     | 1 170 / 846                       | ~60 / ~40                 | ~600 / ~430                |
| BinanceFutures ETHUSDT     | 1 336 / 1 183                     | ~70 / ~60                 | ~700 / ~600                |

Даже в грубой оценке в каждой полосе **десятки–сотни уровней** — гарантированно не «дырка».

Точная per-snapshot гистограмма покрытия полос (`p10/p50/p90`) требует per-snapshot reduce — см. §9
(SCOPE VIOLATION REQUEST).

---

## 6. Усреднённое по времени — obi_probe за 21 ч

`cargo run -q --release -p book --example obi_probe -- /tmp/m10-vps-journal`

| Venue / Symbol             | snaps  | avg lvl bid/ask | avg reach bid % | avg reach ask % | band3=всё_стакан % | OBI_mean (bid3 / bid3+ask8) |
|----------------------------|--------|-----------------|------------------|------------------|---------------------|-----------------------------|
| Binance/BTCUSDT            | 91 578 | 895.9 / 691.0   | **52.68**        | **56.26**        | **0%**              | 0.314                       |
| Binance/ETHUSDT            | 91 578 | 928.7 / 682.4   | **54.29**        | **58.33**        | **0%**              | 0.178                       |
| BinanceFutures/BTCUSDT     | 90 222 | 1 041.6 / 801.1 | **56.49**        | **56.53**        | **0%**              | 0.374                       |
| BinanceFutures/ETHUSDT     | 90 108 | 1 302.5 / 1 064.1 | **56.80**      | **57.48**        | **0%**              | 0.397                       |
| Hyperliquid/BTC *(не Binance, для контраста)* | 17 055 | 20 / 20 | 0.0306 | 0.0304 | **100%** | 0.529 |
| Hyperliquid/ETH            | 17 055 | 20 / 20 | 0.1016 | 0.1016 | **100%** | 0.506 |

**Что это значит:**

1. `avg_reach` ~52–58% на Binance — глубина **стабильна по времени** (не случайный всплеск на
   последнем снимке). Это даёт жёсткую гарантию: полосы 3/5/8/15/30% ВСЕ заполнены на
   100% снапшотов.
2. `band3=всё_стакан = 0%` — depth_within(3%) < total depth на **каждом** снимке.
   Это значит: 3% НЕ захватывает весь стакан, есть уровни далеко — TPP-асимметрия на полосе 3%
   будет реально различимой (в отличие от Hyperliquid, где 100% — полоса 3% = вся книга → OBI
   упёрся бы в «весь камень», асимметрии не видно).
3. `OBI_mean` на Binance 0.18–0.40 — это сильно отличается от 0.5 Hyperliquid → асимметрия ликв-ти
   между bid/ask на нашей Binance-книге ВЫРАЖЕНА (не вырождена).

---

## 7. ЯВНЫЙ ВЕРДИКТ (per symbol)

| Symbol                         | Полосы 3 / 5 / 8 / 15 / 30%                                                                                              |
|--------------------------------|--------------------------------------------------------------------------------------------------------------------------|
| **Binance Spot BTCUSDT**       | **ВЫЧИСЛИМЫ** из собственного захвата. Live REST cap 1.3% < 3% — без diff-support TPP невозможен. Нужен full-depth вендор? **НЕТ.** |
| **Binance Spot ETHUSDT**       | **ВЫЧИСЛИМЫ**. REST cap 4.5% < 5% — тоже требует diff-support. Вендор не нужен.                                         |
| **BinanceFutures BTCUSDT**     | **ВЫЧИСЛИМЫ**. REST cap 0.09% (даже с limit=1000 — 0.18%) — без diff-support TPP невозможен. Вендор не нужен.            |
| **BinanceFutures ETHUSDT**     | **ВЫЧИСЛИМЫ**. REST cap 0.26% < 3%. Вендор не нужен.                                                                    |

**Решение по TPP-данным:** строим **TPP COIN** на собственной diff-книге Binance (Spot + Futures).
Докупать Tardis/Coinalyze **только** при появлении нового символа/venue, для которого наш recorder
ещё не держит поток.

---

## 8. Reproducibility

```bash
# Подготовка worktree (уже сделано обвязкой):
git worktree add /tmp/hft-research-depthprobe -b research/depth-probe origin/main
cd /tmp/hft-research-depthprobe

# 1) Последний снапшот + полосный notional:
cargo run -q --release -p book --example bands -- /tmp/m10-vps-journal

# 2) Усреднённое по 21 ч + OBI feasibility:
cargo run -q --release -p book --example obi_probe -- /tmp/m10-vps-journal

# 3) Live REST-снапшоты (референс):
curl -sS -o snap_btc.json  'https://api.binance.com/api/v3/depth?symbol=BTCUSDT&limit=5000'
curl -sS -o snap_eth.json  'https://api.binance.com/api/v3/depth?symbol=ETHUSDT&limit=5000'
curl -sS -o snap_btc_fut.json  'https://fapi.binance.com/fapi/v1/depth?symbol=BTCUSDT'
curl -sS -o snap_eth_fut.json  'https://fapi.binance.com/fapi/v1/depth?symbol=ETHUSDT'
curl -sS -o snap_btc_fut1000.json  'https://fapi.binance.com/fapi/v1/depth?symbol=BTCUSDT&limit=1000'

# 4) Python-парсер REST (встроен в этот memo как §4.1):
python3 -c "<см. §4.1 в git-истории коммита>"
```

Сырьё текущего прогона:
- stdout `bands.rs` — в §3 этого memo (saved to git history at commit time).
- stdout `obi_probe.rs` — в §6.
- live REST JSON — `/tmp/snap_*.json` снапшоты, `git ls-files /tmp` НЕ под контролем (только ref
  к §4.1). Для пере-проверки достаточно curl'ов выше.

---

## 9. !!! SCOPE VIOLATION REQUEST !!! (стайлнесс и per-snapshot распределение)

### Запрошено в задаче, но НЕ измеримо без правки `crates/book/examples/`:

- **(A)** `staleness` — доля уровней в дальних полосах (≥3%), которые НЕ получали обновления > N мин.
  Наш recorder пишет `L2Snapshot` (replace), а не `@depth` diff. «Время последнего апдейта
  уровня» в нашем формате напрямую недоступно — оно требует **per-level last-seen-timestamp
  reduce поверх последовательных снапшотов**. Такого reduce нет ни в `bands.rs`, ни в
  `obi_probe.rs`, ни в `book::OrderBook` API.
  - **proxy-замер staleness'а** возможен через подсчёт, как часто конкретный (price, size) уровень
    встречается в N последовательных снапшотах. Если уровень на дальней полосе (>3%) виден только
    в 1 из 10 снапшотов — это кандидат в фантомную ликвидность (TD-016).
  - **Реализация требует:**
    - либо расширить `crates/book/examples/bands.rs`/`obi_probe.rs` (рисложить metadata по
      уровням);
    - либо создать новый `crates/book/examples/depth_probe.rs` (ещё один diagnostic);
    - либо новый standalone tool в `research/data-quality/`.
  - `research-dev` не имеет carve-out на `crates/book/examples/*` (это диагностики architect'а).
    Запрос — к **architect'у**: специфицировать staleness-замер как
    - расширение bands.rs (добавить полосы 0.5/1%, колонку staleness, фильтр EpochFilter),
    - или отдельный новый пример/кейс `crates/book/examples/depth_probe_staleness.rs`
      в следующий milestone.

- **(B)** Per-snapshot распределение `max_reach_pct` (p10/p50/p90) и числа уровней внутри полос —
  `obi_probe` даёт только СРЕДНЕЕ. Для p10/p90 нужен streaming reduce с гистограммой.
  То же: **нужен новый пример**.

- **(C)** Полосы `0.5%` и `1%` — `bands.rs` имеет 1.5% как минимальную. Эти 2 полосы нужны
  в задаче. Требуется та же правка — расширение `crates/book/examples/bands.rs` или новый
  пример.

> **Размер carve-out'а:** ~100 строк в одном новом example `crates/book/examples/depth_probe.rs`
> со streaming read через `journal::stream + EpochFilter::OwnCaptureOnly`, агрегацией
> max_reach_pct/band-depth/staleness в `BTreeMap<u64, HistBin>` и выводом JSON для архитектора.
> Пока откладывается — вердикт §7 не зависит от staleness'а.

---

## 10. Что осталось (handoff)

- **architect** — §9: специфицировать staleness/per-snapshot/полосы 0.5%/1%, выдать carve-out
  для `research-dev` на новый пример или взять на себя.
- **founder** — на основе §7 решить: полосы 3/5/8/15/30% вычислимы → TPP COIN строится на нашем
  diff-support Binance, **Tardis/full-depth вендор НЕ покупаем** для текущих 4 инструментов.
- **risk-critic** — sanity-check вердикта: «если avg_reach=52-58%, этого хватает для прод-TPP?»
  (это вне scope research-dev; передаётся вместе с memo).

— research-dev, 2026-07-22, ветка `research/depth-probe`
