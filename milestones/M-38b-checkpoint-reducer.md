# M-38b — checkpoint-reducer + live-seek (TD-044)

- **Статус:** PROPOSED (спека + RED закоммичены; critic ОБЯЗАТЕЛЕН до dev)
- **Автор спеки:** architect, 2026-07-28
- **Базовый HEAD:** `origin/main @ b7ac2f8`; ветка `feat/M-38b`
- **Тех-долг:** TD-044 (**MAJOR** — держит close-out M-28 и M-36)
- **Зона:** read-path (`crates/gateway`, `crates/journal` stream-seek, `crates/book` derive).
  MD-only, ордер-egress отсутствует → risk-critic не требуется (`gates.md` §5 carve-out).
- **Предшественник:** **M-47 мержится ПЕРВЫМ** (GW-I-10). Чекпоинт ключуется
  `selector_fingerprint`; чекпоинт под невалидным селектором — мусор, валидный по CRC.

## Objective

Убрать O(история) из read-пути. Сейчас `gateway::snapshot` реплеит журнал от `Cursor::START`
на КАЖДОЕ подключение.

**Замер на проде** (`e4a8bc6`, журнал 18 GB / 96 `.zst` + 7 raw): первый `Snapshot` — **409.74 s**
(~6.8 мин) после `ws auth ok`; процесс прочитал **>21 GiB** (`/proc/<pid>/io read_bytes`) при
RssAnon 26 MB. Это не регрессия (M-37 лечил ПАМЯТЬ и заявил это ограничение), но кокпит
непригоден: 6.8 мин до первой отрисовки, каждое переподключение реплеит заново, N клиентов =
N полных реплеев, стоимость растёт ~2.8 GB/сут.

**`frames_since` (live-push каждые 250 мс) — ТОЖЕ O(история):** досеивает состояние реплеем
всего журнала (~400 с), потом сворачивает хвост ≤256 событий. За один «тик» recorder пишет
несопоставимо больше — **live-push математически не сходится**. Поэтому обе половины
(чекпоинт + live-seek) обязательны: чекпоинт в одиночку даёт красивый первый кадр и мёртвый live.

## Дизайн

### (1) Чекпоинт = полное сериализованное состояние `Reducer` (НЕ `Snapshot`)

`Snapshot` — ВЫХОД редьюсера, из него состояние не восстановимо: в нём нет `VwapAcc.sum_pv/sum_v`,
`book::OrderBook`, path-dependent кэша `HeatmapBucketState.mid`, `session_max_time_s`, `at_ms`.
Чекпоинт обязан нести ВСЁ поле-в-поле (замерено по `crates/gateway/src/lib.rs:501-533`):

| Поле `Reducer` | Тип | Замечание для сериализации |
|---|---|---|
| `ohlcv` | `BTreeMap<i64, OhlcvAcc{open,high,low,close,volume: i64}>` | — |
| `cvd` | `BTreeMap<i64, CvdSession{base: i64, bucket_delta: BTreeMap<i64,i64>}>` | **форма M-38a** — обязана быть в чекпоинте, иначе миграция выбросит чекпоинты |
| `vwap` | `VwapAcc{sum_pv: i128, sum_v: i128, values: BTreeMap<i64,i64>}` | **i128** — проверить postcard на RED-bootstrap |
| `depth` | `Vec<DepthAcc{side: Side, band: f64, band_pct_e8: i64, values: BTreeMap<i64,i64>}>` | **f64** — сериализуется битами; в фингерпринт только `to_bits` |
| `vp` | `VolumeProfileAcc{bins: BTreeMap<i64, BTreeMap<i64,i128>>}` | **i128** |
| `session_max_time_s` | `BTreeMap<i64,i64>` | **форма M-38a** (unified VP+CVD whole-session критерий) |
| `book` | `book::OrderBook` | **см. §Findings — приватные поля, критично** |
| `heatmap_buckets` | `BTreeMap<i64, HeatmapBucketState{bids,asks: Vec<(i64,i64)>, mid: Option<i64>}>` | `mid` — **path-dependent кэш**, обязателен |
| `bubbles` | `BTreeMap<(i64,i64),(i64,i64)>` | — |
| `at_ms` | `i64` | — |
| + курсор | `Cursor{upto_seq: Option<u64>}` | до какого `seq` состояние свёрнуто |

Ни одного `HashMap`, `Instant`, `SystemTime` — детерминизм экспорта достижим. Полноту держит
компилятор: `#[derive(Serialize, Deserialize)]` на структуре `Reducer` заставит покрыть каждое
новое поле, а канарейка «новое поле без bump `ckpt_schema_version` → падение» — оракул.

**Формат файла:** `magic || ckpt_schema_version || gateway_schema_version || selector_fingerprint
|| epoch_filter_fingerprint || journal_lineage || cursor || postcard(state) || CRC32`.
Запись atomic `tmp + rename` + `flock` на ckpt-dir. Каденс 5-15 мин → хвост ≤30 MB → первый
snapshot ~1-2 с.

### §Findings — что вскрыл разбор кода (НЕ было в роадмапе)

**`book::OrderBook` нельзя восстановить через публичный API — это ловушка «идеальной фикстуры».**
Замер `crates/book/src/lib.rs:25-36`: четыре ПРИВАТНЫХ поля —

```rust
pub struct OrderBook {
    bids: BTreeMap<i64, i64>,
    asks: BTreeMap<i64, i64>,
    last_final_update_id: Option<u64>,  // чейн дельт (GD-I-1..6)
    stale: bool,                        // fail-closed после gap'а
}
```

Публично доступны только `levels(side)` и `is_stale()`; конструктора «из частей» нет.
Соблазнительная реализация — «сохранить `levels()`, восстановить через `apply_snapshot()`» —
теряет `last_final_update_id` (следующая дельта трактуется как bootstrap, а не проверяется на
непрерывность) и сбрасывает `stale` в `false` (**недостоверная книга молча становится
достоверной** — класс тихой лжи, ради которого M-30 ввёл fail-closed).

**Честная граница находки (проверено по коду, не выведено).** Сегодня `Reducer` применяет
`self.book.apply_delta(bids, asks)` (`crates/gateway/src/lib.rs:857`) — НЕчейнящий путь, а не
`apply_l2delta`. Значит в ЭТОМ пути `last_final_update_id` и `stale` сейчас **инертны** (всегда
`None`/`false`), и «восстановление через `apply_snapshot`» СЕГОДНЯ наблюдаемо эквивалентно.
Оракула, который поймал бы его на выходе gateway, не существует — и заявлять обратное нельзя.

Требование derive'а от этого не отменяется, но обосновывается точнее:
1. чекпоинт обязан round-trip'ить структуру целиком — «неиспользуемые сейчас» поля не выбрасываются
   молча (иначе первый же переход gateway на `apply_l2delta` даёт тихую регрессию, а не ошибку сборки);
2. gap-детекция в кокпите — живой вопрос (docs/08 R5, TD-016), и переход на `apply_l2delta` ожидаем.

Поэтому оракул ставится ТАМ, где он реально давит, — **на уровне `crates/book`**
(`red_orderbook_serde_roundtrip`): книга, доведённая до `stale` через `apply_l2delta` с разрывом,
после serde-roundtrip обязана остаться `stale` и продолжать отвергать дельты. Этот тест падает
на реализации «сохранить `levels()` / восстановить `apply_snapshot()`» и проходит на derive.
`crates/book` получает `#[derive(Serialize, Deserialize)]` (serde сериализует приватные поля —
приватность соблюдена, обходного конструктора не появляется).

### (2) Инвалидация — чекпоинт это КЭШ, а не истина

Прецедент — journal `JR-I-4` (`docs/fa/journal.md:87`: «снапшот — оптимизация старта, НЕ истина»).
Любой mismatch → **тихий rebuild от START**, НИКОГДА не ошибка и никогда не расхождение:

- `magic` / `ckpt_schema_version` / `gateway_schema_version` (сейчас 7);
- `selector_fingerprint` — по `venue`, `symbol`, `timeframe_ms`, `window_ms`, `bands` через
  **`f64::to_bits`** (НЕ `Display`: `0.001` и `0.0010` — одна строка, разные конфиги вообще могут
  совпасть по печати). **NaN в `bands` запретить** в `serve_config_from_env` (NaN != NaN ⇒
  фингерпринт нестабилен);
- `epoch_filter_fingerprint` (`OwnCaptureOnly` / `Explicit(sorted)` / `All`);
- `journal_lineage` — sha по ЗАГОЛОВКАМ сегментов: `(index, epoch_id, source, first_seq)`.
  **Намеренно БЕЗ `created_wall_ms` и размера файла**: компакция `.jrnl → .jrnl.zst` их меняет,
  но события те же — чекпоинт обязан пережить компакцию. Вендорная эпоха / purge / смена
  источника — меняют, и чекпоинт обязан быть выброшен;
- CRC; `ckpt.cursor > at` (просят снапшот РАНЬШЕ чекпоинта).

### (3) live-seek + резюмируемый редьюсер

- `journal::stream_from(dir, filter, after_seq) -> io::Result<EventStream>` — сегментный пропуск
  по `first_seq` из v2-заголовков (`contracts::SegmentHeader.first_seq`). **Legacy-сегменты
  (`first_seq == 0`, до CT-RFC-02) НЕ пропускаются никогда** — их `first_seq` не факт, а дефолт
  (родня TD-030). Внутри выбранного сегмента — forward-скан до `seq > after` (zstd не Seek).
- **Резюмируемый живой `Reducer` в соединении**: состояние живёт между тиками и докармливается
  только новыми событиями через `stream_from(cursor)`. Без этого live не сходится, сколько бы
  чекпоинт ни ускорил первый кадр.
- Побочно чинит E.5 (Frame от неполной книги).

### Инварианты

| ID | Инвариант |
|---|---|
| **GW-I-9** | **Чекпоинт — кэш, не истина.** (а) `snapshot_from_checkpoint(K, at) ≡ snapshot(START, at)` — байт-идентично в канонической сериализации, на ЛЮБОМ валидном K, включая деградированные позиции. (б) Любая невалидность (magic/версии/фингерпринты/lineage/CRC/`cursor > at`/нет файла/битый файл) → ТИХИЙ rebuild от START с тем же результатом, без ошибки. (в) `advance()` идемпотентен: два вызова без новых событий → байт-идентичный файл. (г) **Tamper-форсинг:** валидный по CRC чекпоинт с изменённым состоянием ОБЯЗАН изменить выход — доказывает, что чекпоинт ЧИТАЕТСЯ, а не игнорируется тихим rebuild'ом |
| **GW-I-11** | **Read-путь ограничен хвостом.** `snapshot_from_checkpoint` при K у хвоста декодирует ≤ хвостовых событий (не всю историю), `frames_since`/резюм-API открывает ≤ хвостовых сегментов. Измеряется ДЕТЕРМИНИРОВАННЫМИ счётчиками (`ReadStats{events_decoded, segments_opened}`), НЕ аллокатором и НЕ wall-time (урок TD-040). Кадры остаются байт-идентичны текущему `frames_since`, контигуальность курсоров (GW-I-8) цела |

GW-I-10 занят M-47 (выравнивание timeframe). Нумерацию не переиспользовать.

### DET-риски и как закрыты

| Риск | Закрытие |
|---|---|
| Новое поле `Reducer` не попало в чекпоинт | `derive` на структуре (компилятор) + оракул-канарейка на `ckpt_schema_version` |
| `HeatmapBucketState.mid` — path-dependent кэш | Оракул: K выбран так, что ПОСЛЕ него нет двусторонних обновлений книги ⇒ `mid` восстановим только из чекпоинта |
| `book` gap-чейн и `stale` | §Findings + два выделенных оракула |
| `OrderBook` экспорт в неканоническом порядке | Сериализация `BTreeMap` (порядок по возрастанию ключа), НЕ через `levels()` (bid-desc) |
| `f64` в фингерпринте | `to_bits`; NaN отвергается на входе |
| `i128` в postcard | Проверить на bootstrap; fallback — пара `(hi: i64, lo: u64)` |
| Гонка писателей чекпоинта | atomic tmp+rename + flock; journal-том смонтирован **`:ro`** (JR-I-1 цел) |

## Allowed paths

- `crates/gateway/src/**` — `checkpoint` модуль, `snapshot_from_checkpoint`, резюм-API,
  `src/bin/gateway-checkpoint.rs` (engine-dev)
- `crates/journal/src/segments.rs` — `stream_from` + счётчики `ReadStats` на `EventStream` (engine-dev)
- `crates/book/src/lib.rs` — ТОЛЬКО `#[derive(Serialize, Deserialize)]` на `OrderBook` (+ serde в
  `[dependencies]` этого крейта). Никакой другой правки книги (engine-dev)
- `crates/gateway-serve/src/lib.rs` — резюмируемый редьюсер в соединении, запрет NaN в bands (engine-dev)
- `crates/{gateway,journal,book}/Cargo.toml` — ТОЛЬКО добавление своих зависимостей (postcard, crc32,
  serde) по shared-access правилу scope-guard
- `docker-compose.yml` — новый ops-сервис `gateway-checkpoint` (`profiles: ["ops"]`, journal-том
  **`:ro`**, отдельный том ckpt RW), зеркально `journal-retention`
- `milestones/M-38b-checkpoint-reducer.md` — колонка Status в §Tasks (carve-out)

## Forbidden paths

- `crates/*/tests/**` — sacred RED (architect-only). Тест кажется неправильным →
  `!!! SCOPE VIOLATION REQUEST !!!`
- `scripts/verify_M-38b.sh` — acceptance-гейт (architect-only)
- `crates/contracts/**` — T1 не трогается; `GATEWAY_SCHEMA_VERSION` остаётся **7**
  (форма провода не меняется — меняется скорость её получения)
- `crates/recorder/**`, journal-**writer** API — чекпоинтер НЕ писатель журнала (JR-I-1).
  Компактор не трогается: инверсия слоёв (journal не знает про селекторы/VWAP)
- Семантика редьюсера (`finish`, `evict_window_state`, `merge_*`, `apply`) — байт-идентичность
  выхода обязана сохраниться; чекпоинт только УСКОРЯЕТ путь к тому же результату

## Tasks

| # | Status | Задача | Агент | Acceptance |
|---|---|---|---|---|
| 0 | ⏳ OPEN | **Bootstrap-проверка `i128` в postcard** (roundtrip `VwapAcc{sum_pv,sum_v}` и `vp.bins`). Не поддержан → пара `(hi i64, lo u64)` + запись в milestone | engine-dev | Отчёт в коммите; `red_checkpoint_roundtrip` компилируется |
| 1 | ⏳ OPEN | `#[derive(Serialize, Deserialize)]` на `book::OrderBook` (все 4 приватных поля) + serde в `crates/book/Cargo.toml`. Больше НИЧЕГО в книге | engine-dev | `cargo test -p book` зелён; поля не стали pub |
| 2 | ⏳ OPEN | `derive` на `Reducer` + всех вложенных (`OhlcvAcc`/`CvdSession`/`VwapAcc`/`DepthAcc`/`VolumeProfileAcc`/`HeatmapBucketState`) + `CkptHeader` (magic/версии/фингерпринты/lineage/cursor) + CRC | engine-dev | `red_checkpoint_roundtrip` GREEN |
| 3 | ⏳ OPEN | `checkpoint::advance(journal_dir, ckpt_dir, sel, filter)` — atomic tmp+rename + flock; идемпотентность | engine-dev | `red_checkpoint_is_cache::advance_idempotent` GREEN |
| 4 | ⏳ OPEN | `gateway::snapshot_from_checkpoint(...) -> io::Result<(Snapshot, ReadStats)>` — загрузка, валидация, докорм хвостом; ЛЮБАЯ невалидность → тихий rebuild | engine-dev | `red_checkpoint_byte_identity` + `red_checkpoint_is_cache` GREEN |
| 5 | ⏳ OPEN | `journal::stream_from(dir, filter, after_seq)` — сегментный пропуск по `first_seq`; **legacy `first_seq==0` не пропускается**; `ReadStats` счётчики на `EventStream` | engine-dev | `journal::red_stream_from` GREEN |
| 6 | ⏳ OPEN | Резюмируемый редьюсер в соединении `gateway-serve` (докорм через `stream_from(cursor)`, состояние живёт между тиками) + запрет NaN в `bands` | engine-dev | `red_frames_seek_bound` GREEN |
| 7 | ⏳ OPEN | Бинарь `crates/gateway/src/bin/gateway-checkpoint.rs` + ops-сервис в `docker-compose.yml` (journal-том `:ro`, ckpt-том RW), зеркально `journal-retention` | engine-dev | verify-канарейки; §8 — reviewer |
| 8 | ⏳ OPEN | Прогон гейта `bash scripts/verify_M-38b.sh` → `VERDICT: PASS` | engine-dev | exit=0, Done Block сырым выводом |

Оценка: **8-10 атомарных коммитов**.

## Оракулы (sacred, architect-only)

| Файл | Что давит |
|---|---|
| `crates/gateway/tests/red_checkpoint_byte_identity.rs` | GW-I-9(а,г): байт-идентичность на **деградированных K** — середина бакета; между `L2Snapshot` и `L2Delta`; после K нет двусторонних book-обновлений (`mid`-кэш); K перед 00:00 UTC при `at` после (CVD-ledger + VP whole-session); окно активно, эвикции ДО и ПОСЛЕ K; 2+ сделки по обе стороны K; K=0 / K=at / K=последний seq сегмента. + **форсинг «чекпоинт реально читается»**: подменный чекпоинт, снятый с ДРУГОГО журнала с тем же `selector_fingerprint` и тем же `journal_lineage` (совпадение по построению: lineage — по заголовкам, не по содержимому), обязан ИЗМЕНИТЬ выход |
| `crates/gateway/tests/red_checkpoint_is_cache.rs` | GW-I-9(б,в): битый CRC / чужой selector / чужой lineage / чужая версия / `cursor > at` / нет файла → выход ≡ rebuild-от-START, без ошибки; идемпотентность `advance` |
| `crates/gateway/tests/red_checkpoint_resource_bound.rs` | GW-I-11: `ReadStats.events_decoded ≤ N_tail·k` и `segments_opened ≤ хвостовых` при K у хвоста; прод-масштаб (десятки MiB, смесь raw + `.zst`) |
| `crates/gateway/tests/red_frames_seek_bound.rs` | GW-I-11 + GW-I-8: резюм-API у хвоста ограничен, кадры байт-идентичны текущему `frames_since`, контигуальность курсоров цела |
| `crates/journal/tests/red_stream_from.rs` | Сегментный пропуск по `first_seq`; legacy `first_seq==0` НЕ пропускается; граница сегмента; смесь raw/`.zst` |
| `crates/book/tests/red_orderbook_serde_roundtrip.rs` | §Findings: книга, доведённая до `stale` разрывом чейна, после serde-roundtrip остаётся `stale` и отвергает дельты; `last_final_update_id` переживает roundtrip (следующая дельта проверяется на непрерывность, а не bootstrap'ится). Падает на «`levels()` + `apply_snapshot()`» |

### Почему одной байт-идентичности НЕДОСТАТОЧНО (главное для critic'а)

**Реализация, которая ПОЛНОСТЬЮ ИГНОРИРУЕТ чекпоинт и всегда реплеит от START, проходит
ВСЕ тесты байт-идентичности и все тесты инвалидации.** Она же — самый вероятный «зелёный»
исход, если оракул односторонний (класс «идеальная фикстура», пойманный ЧЕТЫРЕ раза подряд:
M-07, M-08, TD-042, TD-045). Форсингов ровно два, и оба обязаны быть в наборе:

1. **Подменный чекпоинт** (GW-I-9г) — файл, снятый с ДРУГОГО журнала, но проходящий ВСЮ
   валидацию (тот же селектор, та же схема, тот же `journal_lineage` — он считается по
   заголовкам сегментов, а не по содержимому). Реализация, которая чекпоинт читает, вернёт
   другой ответ; реализация, которая его игнорирует, вернёт правильный — и упадёт здесь.
   Байт-флип с пересчётом CRC для этого НЕ годится: испорченный postcard, скорее всего, не
   десериализуется → штатный тихий rebuild → тест позеленеет на неправильной реализации.
   **Остаточный риск назван явно:** `journal_lineage` НЕ контентный хэш, поэтому чекпоинт
   доверяется в пределах своего фингерпринт-конверта. Контентный хэш стоил бы полного
   перечитывания журнала — то есть ровно того, что milestone устраняет. Защита от подмены —
   атомарная запись своим бинарём + права на ckpt-том, а не хэш.
2. **Resource-bound** (GW-I-11) — счётчик декодированных событий. Тихий rebuild декодирует всю
   историю и падает по счётчику.

Счётчики — ДЕТЕРМИНИРОВАННЫЕ (`ReadStats`, инкремент в `EventStream::next`), не аллокатор и не
wall-time; тесты ресурса не гоняются в параллель с чужими (урок TD-040: глобальный allocator +
параллелизм = флак).

## Contract impact

**T1 не тронут.** `GATEWAY_SCHEMA_VERSION` остаётся `7` — форма провода не меняется. Формат
чекпоинта — ВНУТРЕННИЙ (T3, не пересекает границу движок↔деск), версионируется собственным
`ckpt_schema_version`; несовпадение → rebuild, миграций не требует по построению. CT-RFC не нужен.

## Acceptance

`bash scripts/verify_M-38b.sh; echo "exit=$?"` → `VERDICT: PASS`, `exit=0`.

Гейт: fmt/build/clippy; все 5 оракулов; grep-канарейки — (а) `gateway` не импортирует
journal-**writer** API (расширение VB-I-3 на бинарь чекпоинтера); (б) journal-том у
`gateway-checkpoint` в compose смонтирован `:ro`; (в) `ckpt_schema_version` объявлен; регрессия
всего read-path suite + `cargo test -p journal -p book`.

**§8 (reviewer, вне гейта):** прод-замер первого `Snapshot` ПОСЛЕ прогрева чекпоинта — ожидание
секунды вместо 409.74 s, с сырым выводом. «Код на main ≠ функция в проде»: если cron-сервис не
заведён, чекпоинта на проде нет и латентность не изменилась — это ровно класс TD-020, и §8
обязан это поймать DECODE'ом, а не grep'ом.

## Гейты

- **plan-time critic — ОБЯЗАТЕЛЕН** (`gates.md` §3: ≥5 коммитов + касание `crates/journal`).
  Отдельно просить критика проверить: (1) достаточность форсингов «чекпоинт реально читается»;
  (2) полноту списка полей состояния против `crates/gateway/src/lib.rs:501-533`; (3) строгость
  связки с ретеншеном (`docs/06` §4): prune требует покрытия чекпоинтом `cursor ≥ последнего seq
  сегмента ЛИБО явного skip-репорта — мягкая или строгая связка, решить на plan-time.
- risk-critic — не требуется (read-path, MD-only, `gates.md` §5 carve-out).
- reviewer — UNCONDITIONAL + §8 post-merge деплой-гейт.

## Handoff-цепочка

`architect` (спека+RED+verify) → **`critic`** (plan-time) → `engine-dev` (задачи 0-8) →
`tester` → `reviewer` (PR + merge + §8 + TD-044 CLOSED, close-out M-28/M-36 разблокирован).

## Cross-references

- `milestones/M-38-roadmap.md` §M-38b (долговечный план), M-39 (shared-tailer — следующий)
- `TECH-DEBT.md` TD-044; `docs/fa/viz-backend.md` VB-I-2/VB-I-3/VB-I-10;
  `docs/fa/journal.md` JR-I-1/JR-I-4 (снапшот-как-кэш — прецедент GW-I-9)
- `crates/gateway/src/lib.rs:501-533` (состояние `Reducer`), `crates/book/src/lib.rs:25-36`
  (приватные поля — §Findings), `crates/journal/src/segments.rs:750-870` (`EventStream`/`stream`)
- `.claude/rules/testing.md` (чек-лист; п.5 прод-масштаб, п.6 композиция, п.7 парный vantage)
