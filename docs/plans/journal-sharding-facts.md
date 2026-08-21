<!-- FACTS: audited_head=fe8445573cbe8113a77d313d512b6b28af83e694 collected=2026-08-02 -->
# Карта журнала hft-platform: что завязано на единый общий порядок

**Разведка (read-only), собрана:** 2026-08-02
**Дерево:** `origin/main` = `8b42240` («docs(process): паритет verify с CI-job fmt+clippy+test»)
**Чекаут разведки:** `/tmp/hft-scout-shard` (worktree, `--detach origin/main`); основной чекаут
`/home/nous/hft-platform` НЕ трогался.
**Пути ниже — относительно корня репо**; абсолютный эквивалент — `/tmp/hft-scout-shard/<path>`
(и `/home/nous/hft-platform/<path>` для того же файла на main).

Все утверждения — с `путь:строка`. Где факт не найден, так и написано.
Проектирование не предлагается — только фактура.

---

## 1. Физическая раскладка сегмента

### 1.1 Имя, каталог, индекс

| Факт | Где |
|---|---|
| Имя сегмента: `format!("segment-{index:08}.jrnl")` | `crates/journal/src/segments.rs:232-234` (`segment_name`) |
| Путь: `dir.join(segment_name(index))` | `crates/journal/src/segments.rs:1279-1281` (`segment_path`) |
| Legacy-путь (`Journal::open`) — имя ЗАХАРДКОЖЕНО `segment-00000000.jrnl` | `crates/journal/src/lib.rs:48` (`const SEGMENT`) |
| Мета-файл: `journal.meta` (u64 LE `next_seq`) | `crates/journal/src/lib.rs:47` (`const META`), запись — `lib.rs:349-354` (`write_meta`, tmp+rename) |
| Каталог журнала — ОДИН, из env `JOURNAL_DIR` (дефолт `./journal-data`) | `crates/recorder/src/main.rs:225` |
| В проде каталог — том `journal-data:/journal` | `docker-compose.yml` (`environment: JOURNAL_DIR: /journal`, `volumes: journal-data:/journal`) |
| Индекс парсится ИЗ ИМЕНИ (8 цифр, строго) | `crates/journal/src/segments.rs:600-606` (`parse_segment_index`), `:615-618` (`parse_segment_index_any`) |

**Никакого поля «инструмент» в имени/пути сегмента нет.** Раскладка = плоский каталог
`segment-NNNNNNNN.jrnl[.zst]` + `journal.meta` + опционально `journal.legacy.json`,
`journal.force-next-seq.json`, `recorder.heartbeat`, `journal.replay-digest.json`.

### 1.2 Формат (wire format v2)

Каноническое описание: `crates/journal/src/segments.rs:14-24`.

```text
SEGMENT_MAGIC            // 8 байт: b"HFTJRN02"
SegmentHeader-frame      // [u32 LE len][postcard(SegmentHeader)][u32 LE crc32(payload)]
event_frame[0..N]        // [u32 LE len][postcard(Event)][u32 LE crc32(payload)]
```

- Магия: `crates/contracts/src/lib.rs:43` (`SEGMENT_MAGIC: [u8;8] = *b"HFTJRN02"`).
- `SegmentHeader` (T1): `crates/contracts/src/lib.rs:96-110` — поля
  `schema_version, source, provenance, epoch_id, created_wall_ms, first_seq`.
  **`first_seq` = seq первого события сегмента** (`contracts/src/lib.rs:108-109`).
- Сериализация header-фрейма: `crates/journal/src/segments.rs:1267-1276` (`serialize_v2_header`).
- Сериализация event-фрейма: `crates/journal/src/segments.rs:2310-2319` (`serialize_event_frame`).
- Чтение фрейма: `crates/journal/src/segments.rs:442-480` (`read_frame_payload`), санити-кап длины
  `FRAME_LEN_SANITY_CAP = 64 MiB` — `segments.rs:431`.
- **Футера нет** — сегмент заканчивается последним фреймом; «конец» определяется EOF/торном.
  Явного футера/индекса/трейлера в коде не найдено.
- Legacy (schema v1): без магии и без заголовка, только event-фреймы; читается ТОЛЬКО через
  явную декларацию в `journal.legacy.json` — `segments.rs:22-24`, `:108` (`LEGACY_MANIFEST`),
  `:542-574` (legacy-ветка `classify_segment`), `contracts/src/lib.rs:55-76` (`LegacySegmentDecl`).
  У legacy `first_seq` **синтезируется нулём** — `segments.rs:564-567` (это отдельный источник
  боли во всех guard'ах, см. §3).

### 1.3 Ротация

| Факт | Где |
|---|---|
| Порог: `DEFAULT_MAX_SEGMENT_BYTES = 1024*1024*1024` (**1 GiB**) | `crates/journal/src/segments.rs:41` |
| Проверка порога ДО записи фрейма, чтобы seq не сдвигался | `crates/journal/src/lib.rs:231-236` |
| Сама ротация: flush → sync_data → новый `segment-{N+1}.jrnl` с magic+header | `crates/journal/src/lib.rs:252-292` (`Journal::rotate`) |
| **`first_seq` нового сегмента = `self.next_seq`** — seq продолжается сквозь границу | `crates/journal/src/lib.rs:281` |
| Комментарий контракта: «`seq` продолжается сквозь границу — тотальный порядок один на журнал, JR-I-1» | `crates/journal/src/lib.rs:119-122` |
| Legacy-путь (`Journal::open`) НЕ ротирует | `crates/journal/src/lib.rs:253-256` |
| Открытие сегмента на запись (magic+header при !reuse или пустом файле) | `crates/journal/src/segments.rs:2334-2356` (`open_seg_for_write`) |
| Выбор сегмента при старте (reuse vs новый) | `crates/journal/src/segments.rs:2224-2307` (`decide_open_segment`) — reuse требует совпадения `source && provenance && epoch_id && schema_version` (`:2275-2287`) |

### 1.4 Disk-guard (fail-closed)

- `DEFAULT_MIN_FREE_BYTES = 10 GiB` — `crates/journal/src/segments.rs:44`.
- Проверка свободного места ПЕРЕД каждой записью: `crates/journal/src/lib.rs:223-229`;
  при недоборе — `Err(StorageGuard)`, seq/байты не сдвинуты.
- `free_bytes_at` = `statvfs(dir)` → **свободное место ФАЙЛОВОЙ СИСТЕМЫ, не каталога/шарда**:
  `crates/journal/src/segments.rs:1227-1242`.
- `storage_status` (для heartbeat): `crates/journal/src/segments.rs:1254-1261`,
  `crates/journal/src/lib.rs:187-195`.

### 1.5 Сжатие (`.jrnl` vs `.jrnl.zst`)

| Факт | Где |
|---|---|
| Суффикс `.zst`, уровень по умолчанию 3 (9.1× на боевом сегменте) | `crates/journal/src/segments.rs:3030`, `:3033` |
| `compact_segment` — сжать ЗАКРЫТЫЙ сегмент; активный (`max index`) сжимать запрещено | `crates/journal/src/segments.rs:3065-3084` |
| Legacy/foreign сегмент сжимать архитектурно запрещено (проверка магии ДО мутаций, D-COMP-4) | `crates/journal/src/segments.rs:3103-3116` |
| Порядок «tmp → sha256-сверка распакованного → fsync → rename → удалить оригинал»; самоизлечение крах-окна | `crates/journal/src/segments.rs:3044-3062`, `:3132-3160` |
| `compact_closed_segments` (batch, `--keep-raw N`) | `crates/journal/src/segments.rs:3236` |
| Классификация `.zst` (forward-only декод заголовка, zstd не Seek) | `crates/journal/src/segments.rs:584-597` (`classify_compacted_segment`) |
| **Правило коллизии raw vs .zst одного индекса: побеждает СЫРОЙ** (D-COMP-1) | `crates/journal/src/segments.rs:620-690` (`iter_segments_sorted`/`dedup_indexed_paths`) |

### 1.6 `list_segments` — единственный публичный энумератор

- Публичный alias: `crates/journal/src/lib.rs:29-34` (`segments as list_segments`).
- Реализация: `crates/journal/src/segments.rs:714-746` (`segments`):
  1) `load_manifest(dir)`; 2) `dedup_indexed_paths(dir)` (BTreeMap по индексу — порядок
  `read_dir` не протекает, `segments.rs:651-654`); 3) `classify_segment` на каждый файл;
  4) `sort_by_key(index)`; 5) **guard монотонности `first_seq`** (`segments.rs:724-743`).
- Офлайн-энумератор: `iter_segments_sorted` → тот же `dedup_indexed_paths`
  (`crates/journal/src/segments.rs:641-643`), используется `read_all`/`recover`/`readable_floor`.

---

## 2. `seq` — где назначается и что гарантирует

### 2.1 Назначение

- **Единственная точка присвоения:** `crates/journal/src/lib.rs:214-219` —
  `Event { seq: self.next_seq, ts_mono_ns, ts_wall_ms, kind }`, затем `next_seq += 1`
  (`lib.rs:241`).
- `Journal::append` объявлен как «Единственный путь записи в журнал (JR-I-1)» —
  `crates/journal/src/lib.rs:198`.
- Тип: `crates/contracts/src/lib.rs:138-143` — `Event.seq: u64`; док-строка `:135-136`:
  «`seq` — тотальный порядок, назначается журналом (единственный писатель, JR-I-1).
  Коннекторы seq НЕ проставляют».
- **Один writer на весь процесс:** recorder сводит ВСЕ venue/symbol/recon/margin-потоки в
  ОДИН mpsc-канал → один `Journal` → `run_writer`:
  `crates/recorder/src/main.rs:234` (канал), `:308` (md-tap), `:332-370` (per-venue fanout →
  `tx_v`), `:420` (margin-inventory прямо в `tx`), `:431-440` (recon per (venue,symbol) в `tx`),
  `:459` (`Journal::open_with`), `:461-468` (`run_writer`).
  Сам цикл: `crates/recorder/src/lib.rs:147-231` — `journal.append(kind)` в трёх ветках
  (`:170`, `:190`, `:208` — heartbeat каждые 10 с).
- Батч-flush: каждые 64 события (`crates/journal/src/lib.rs:243-246`) + каждые 1000
  (`crates/recorder/src/lib.rs:196-199`); `flush` = `sync_data` + запись `journal.meta`
  (`crates/journal/src/lib.rs:301-307`).

**Вывод по гарантии:** `seq` — глобально монотонный на ВЕСЬ каталог журнала
(один writer, один счётчик, один `journal.meta`). Пространство `seq` не сегментировано:
`SegmentHeader.first_seq` — просто отметка, где счётчик был на момент открытия сегмента.

### 2.2 Кто на `seq` опирается

| Потребитель | Где | На что опирается |
|---|---|---|
| Восстановление `next_seq` при старте | `crates/journal/src/lib.rs:136` → `segments.rs:2158-2203` (`resolve_next_seq_or_declared`) → `:1466-1476` (`resolve_next_seq_with`) | `max(journal.meta, tail_last_seq(latest segment)+1)` — ОДИН счётчик на каталог |
| Хвостовой скан последнего сегмента | `crates/journal/src/segments.rs:1300-1462` (`tail_last_seq_of`), `crates/journal/src/lib.rs:362-425` (legacy `scan_tail_for_last_seq`) | окно `TAIL_SCAN_CHUNK = 4 MiB` (`lib.rs:52`) |
| Пропуск сегментов при seek | `crates/journal/src/segments.rs:1025-1046` (`stream_from`) | `last_seq(seg) = next_seg.first_seq − 1` |
| Дайджест детерминизма | `crates/journal/src/segments.rs:1083-1122` (`replay_digest`) | порядок возрастания `seq` |
| Курсор gateway-чекпоинта | `crates/gateway/src/lib.rs:148-164` (`Cursor{upto_seq: Option<u64>}`), `:2085-2086` | одно число на весь журнал |
| Покрытие ретеншена чекпоинтом | `crates/journal/src/segments.rs:2387-2391` (`checkpoint_covered_through_seq`), `:2557-2649` | `last_seq(seg) <= covered` |
| Провенанс истории кокпита | `crates/gateway/src/lib.rs:2087-2096` (`history_start_seq`/`history_truncated`) | глобальный seq |
| **Реестр эпох данных** (границы дефектов) | `docs/data-epochs.md:34-46` — граница E-001 записана как **`seq ≈ 123 205 544`** | глобальный seq как ось истории |
| Метрики ops | `crates/ops/src/metrics.rs:50` (`journal_seq_current`), `:55` (`journal_seq_gaps_total`) | глобальный seq |

### 2.3 Guard'ы монотонности (что осталось от M-49/M-50/M-52)

Инциденты seq-reuse закрывались тремя слоями, и ВСЕ они живы в коде:

1. **JR-I-8 (M-49): нечитаемый хвост → отказ старта, а не «начни с меты».**
   - Маркер ошибки: `crates/journal/src/segments.rs:156-179` (`TailUnreadable`), конструктор
     `:203-211`, предикат `:225-227`.
   - Различение «нет/пусто/нет событий» vs «нечитаем»: `crates/journal/src/segments.rs:1300-1462`
     (см. особенно `:1441-1460` — если окно скана не достало до начала файла и валидного фрейма
     нет, это «неизвестно», а не «пусто»).
   - Операторский выход — файловая декларация `journal.force-next-seq.json`:
     `crates/journal/src/segments.rs:1488` (`FORCE_NEXT_SEQ_DECL`), `:1490`
     (`FORCE_NEXT_SEQ_DECL_APPLIED`), `:1502-1515` (загрузка), `:2141-2146` (пометка применённой),
     `:2158-2203` (применение с проверкой «`next_seq` строго > максимального ЧИТАЕМОГО seq»).
   - RED-оракулы: `crates/journal/tests/red_tail_integrity.rs`,
     `red_tail_integrity_operator.rs`, `red_tail_integrity_prodscale.rs`,
     `red_tail_integrity_operator_prodscale.rs`, `red_tail_integrity_bounded.rs`,
     `red_restore_next_seq_bounded.rs`.

2. **JR-I-9/JR-I-10 (M-50/M-52): пол читаемого seq + бюджет работы.**
   - Три состояния вместо `Option<u64>`: `crates/journal/src/segments.rs:1524-1535`
     (`ReadableFloor::{Known, NoSegments, Unknown}`) — «не знаю» ≠ «разрешено».
   - Скан пола: `crates/journal/src/segments.rs:1616-1639` (`readable_floor`).
   - Бюджет работы `READABLE_FLOOR_WORK_BUDGET_BYTES = 8 × DEFAULT_MAX_SEGMENT_BYTES` (= 8 GiB):
     `crates/journal/src/segments.rs:1579`; сам счётчик — `:1543-1577` (`WorkBudget`).
     Мотивация в комментарии `:1537-1542`: «прод: 158 сегментов ≈ 140 GiB сырых».
   - Side-верификация крупного фрейма без буферизации тела:
     `crates/journal/src/segments.rs:1866` (`SEQ_PREFIX_CAP = 10`), `:1887-1934`
     (`verify_large_frame` — CRC потоково + декод ведущего varint `seq` из ≤10 байт префикса).
   - RED: `red_floor_scan.rs`, `red_floor_scan_bounded.rs`, `red_floor_scan_prodscale.rs`,
     `red_floor_work_budget.rs`, `red_m52_prodscale.rs`.

3. **JR-I-11 (M-52/TD-030): guard монотонности `first_seq` между сегментами.**
   - Общий хелпер: `crates/journal/src/segments.rs:1724-1755` (`check_first_seq_monotonic`).
     Правило: сравнимые `first_seq` НЕ УБЫВАЮТ по возрастанию индекса; два carve-out'а —
     legacy-сентинел исключается (`:1728-1731`), равенство законно при пустом левом сегменте
     (`:1738-1741`).
   - Обёртка по путям: `crates/journal/src/segments.rs:1761-1776` (`check_monotonic_paths`).
   - Три точки применения: `segments()` инлайн (`:724-743`), `read_all`
     (`crates/journal/src/lib.rs:442-445`), `readable_floor` (`segments.rs:1618`).
   - `recover()` guard'ом НЕ покрыт — это открытый долг **TD-076** (`TECH-DEBT.md:53-64`).
   - Guard применяется ДО фильтра эпох — открытый долг **TD-077** (`TECH-DEBT.md:66-78`).
   - RED: `crates/journal/tests/red_stitch_monotonic.rs` (557 строк, MN-1…MN-8).

---

## 3. Что ломается при разделении потока на несколько файлов-шардов

### 3.1 Replay / детерминизм (`DET-I-1`)

**Что именно хешируется.** `crates/journal/src/segments.rs:1059-1122`:

```text
state_hash = SHA-256( для каждого события в порядке ВОЗРАСТАНИЯ seq:
                      u32 LE (длина postcard-payload) ‖ postcard(Event) )
```

- Тип результата: `ReplayDigest { events, first_seq, last_seq, state_hash: [u8;32] }` —
  `crates/journal/src/segments.rs:1065-1071`.
- Реализация — потоковая, через `stream_from(dir, filter, after)` — `segments.rs:1097-1114`.
- **Дайджест — функция ПОТОКА СОБЫТИЙ, а не файлов**: явное требование в
  `crates/journal/src/segments.rs:1059-1064` и в оракуле
  `crates/journal/tests/red_det_replay_digest.rs:24-30` («сжатие сегмента, иная нарезка на
  сегменты, иной порядок файлов в каталоге не имеют права его изменить»).
  Но «порядок» здесь — **глобальный порядок seq одного каталога**; понятия «дайджест шарда»
  или «композиция дайджестов» в коде НЕТ.

**Оракулы DET-I-1** (`crates/journal/tests/`):

| Файл | Что пинит |
|---|---|
| `red_det_replay_digest.rs:52-60` | независимый эталон `reference_state_hash` (дублирует формат намеренно) |
| `red_det_replay_digest.rs:102` `det_1` | реплей ×2 бит-идентичен |
| `red_det_replay_digest.rs:140` `det_2` | равенство независимому эталону |
| `red_det_replay_digest.rs:176` `det_3` | РАЗЛИЧИЕ на различающихся потоках (анти-плацебо) |
| `red_det_replay_digest.rs:264` `det_4` | компакция `.zst` не меняет дайджест |
| `red_det_replay_digest.rs:310` `det_5` | окно `[from,to]` включительно, пересекает границу сегмента И формата |
| `red_det_replay_digest.rs:366` `det_6` | вырожденные входы |
| `red_det_replay_digest.rs:466` `det_7` | `EpochFilter` — окно, а не расхождение |
| `red_det_replay_digest.rs:529-580` `det_8` | **дайджест совпадает по ДВУМ путям чтения** (`stream` и `read_all`) + композиция окон |
| `red_det_prodscale.rs:138-241` | DET-I-1 на прод-форме (много сегментов, raw+.zst вперемешку) + границы аллокаций |
| `red_det_restart.rs` | DET-I-1 через границу ПРОЦЕССА |
| `red_det_sources.rs` | недетерминизм источников (порядок `read_dir` и т.п.) |
| `red_replay_digest_delivery.rs` | дайджест наблюдаем ОПЕРАТОРОМ (exit-код), JR-I-12 |
| `red_stitch_monotonic.rs` | сшивка сегментов по индексу файла ≠ порядок seq → отказ |

**Что предполагает единый поток:** порядок событий = «сегменты по возрастанию индекса, внутри
сегмента — по позиции в файле». Ни одной точки, где событие сортируется/сливается по времени
между двумя независимыми файлами, в коде не найдено (merge-читателя нет).

### 3.2 Читатели — кто и как открывает журнал

**Все прод-чтения идут через ровно две функции: `stream` / `stream_from`.**
`stream` = `stream_from(dir, filter, None)` — `crates/journal/src/segments.rs:992-994`.
`stream_from` — `:1009-1057`. Итератор: `:931-988` (`EventStream::next`), открытие следующего
сегмента: `:901-928`.

| Крейт / файл | Точки входа |
|---|---|
| `crates/gateway` (**главный потребитель**) | `journal::list_segments`: `src/lib.rs:2041` (`first_visible_seq`), `:2443` (сбор lineage чекпоинта), `:2672` (`validate_lineage`). `journal::stream`: `:1733`, `:1772`, `:1792`, `:1919`, `:2795`. `journal::stream_from`: `:1885`, `:2411`, `:2876` |
| `crates/gateway-serve` | `journal_dir` в `ServeConfig` — `src/lib.rs:163`, `:562-564` (env `GATEWAY_JOURNAL_DIR`, дефолт `./journal-data`); вызовы в `:363`, `:426`. Комментарий про `journal::segments` — `:182` |
| `crates/research-cli` | `src/data_quality.rs:73`, `src/export_io.rs:212`, `src/grid.rs:533`, `src/bin/latency_probe.rs:88`, `src/main.rs:62-69` (`JournalSource{dir, filter}`), `examples/depth_lifetime.rs:23-24` |
| `crates/book` (диагностика) | `examples/bands.rs:16`, `examples/obi_probe.rs:27`, `examples/depth_probe.rs:41` — все через `journal::read_all` |
| `crates/journal` | `examples/dump.rs:6` (`read_all`) |
| `crates/recorder` | ТОЛЬКО пишет; `read_all` — в тесте `tests/red_shutdown_j1.rs:36` |
| `crates/ops` | **журнал не читает** (`src/server.rs:13` — лишь комментарий-сравнение); зависимости от крейта `journal` в рантайме нет (`src/metrics.rs:5`, `src/sink.rs:5-6`) |
| `crates/sim` | **журнал не читает вообще** (grep по `crates/sim/src/*.rs` — 0 совпадений) |
| `journal-retention` (бинарь) | `crates/journal/src/bin/journal-retention.rs` — `retention_plan`/`retention_execute`/`compact_closed_segments`/`replay_digest` |

**Ключевой факт для шардирования (gateway):** `Selector` уже ЕСТЬ и он per-инструмент —
`crates/gateway/src/lib.rs:109-124`:

```rust
pub struct Selector { venue: Venue, symbol: String, timeframe_ms: i64, bands: Vec<f64>, window_ms: Option<i64> }
fn matches(&self, md: &MdEvent) -> bool { md.venue == self.venue && md.symbol == self.symbol }  // :123
```

Но фильтрация происходит **после полного декодирования каждого события**: `Reducer::apply_*`
вызывает `self.selector.matches(md)` и делает `return`, если не совпало —
`crates/gateway/src/lib.rs:756`, `:787`, `:820`, `:850`. То есть кокпит для ОДНОГО инструмента
сегодня читает и декодирует ВЕСЬ журнал всех инструментов.

Чекпоинт частично лечит это по времени, но не по раскладке:
- имя файла чекпоинта = `ckpt-<selector_fingerprint>.bin` — `crates/gateway/src/lib.rs:1987-1990`,
  фингерпринт — `:2126-2140` (включает `symbol`);
- заголовок чекпоинта хранит `journal_lineage: Vec<SegmentHeader>` ВСЕХ видимых сегментов
  (`crates/gateway/src/lib.rs:2084`, сбор — `:2443-2450`) и один глобальный `cursor`
  (`:2085-2086`);
- валидация lineage сверяет ВЕСЬ видимый набор сегментов поле-в-поле —
  `crates/gateway/src/lib.rs:2666-2700+` (`validate_lineage`);
- `first_visible_seq` берёт **минимум `first_seq` по всем видимым сегментам** —
  `crates/gateway/src/lib.rs:2040-2049`.

### 3.3 Компакция, чекпоинт, ретеншен (cron-цепочка)

**Цепочка на VPS** — `deploy/cron.d/journal-retention`:
- `50 3 * * *` компакция → `deploy/bin/journal-compaction-cron.sh`
  (argv: `--dir /journal --keep-raw 2 --mode compact`);
- `0 4 * * *` gateway-checkpoint → `deploy/bin/gateway-checkpoint-cron.sh`
  (пишет `--coverage-out=/ckpt/covered_through_seq`);
- `7 4 * * *` ретеншен → `deploy/bin/journal-retention-cron.sh`
  (argv собирается в `ARGV=(...)`, включая `--checkpoint-coverage=/ckpt/covered_through_seq`).

Сервисы: `docker-compose.yml` — `journal-retention` (том `journal-data:/journal:ro` + bind
`/cold`), `journal-compaction` (RW на `/journal`).

**Что предполагает единый порядок:**

| Механизм | Где | Предположение |
|---|---|---|
| «Активный сегмент = сегмент с МАКСИМАЛЬНЫМ индексом» | `crates/journal/src/segments.rs:2475-2478`, `:2490-2492`; компакция — `:3067-3084` | в каталоге ровно ОДИН активный сегмент, потому что writer один |
| `keep_min_segments` — «последние N по индексу» | `crates/journal/src/segments.rs:2538-2555` | индекс = ось времени всего журнала |
| Возраст сегмента по `ts_exch_ms` ПЕРВОГО события | `crates/journal/src/segments.rs:2794-2799` (`segment_decision_ts`), `:2801-2838`, `:2840-2854` | один поток → возраст сегмента ≈ возраст всех его данных |
| `last_seq(seg) = next_seg.first_seq − 1` | `crates/journal/src/segments.rs:2856-2877` (`last_seq_for_segment`) | сегменты образуют НЕПРЕРЫВНУЮ цепочку по seq |
| Гейт prune: `last_seq(seg) <= checkpoint_covered_through_seq` | `crates/journal/src/segments.rs:2557-2649` (особенно `:2594-2598`) | покрытие — ОДНО число на весь журнал |
| `disk_pressure = free_bytes(dir) < min_free_bytes` | `crates/journal/src/segments.rs:2658-2660` | одна ФС, один каталог |
| `ColdCopyProof` — приватный конструктор, prune только по доказанной копии | `crates/journal/src/segments.rs:1129-1136`, `:1149-1204` | пофайловая, шард-агностична (для шардирования это не блокер) |
| Компакция запрещена для активного (max index) и для legacy | `crates/journal/src/segments.rs:3067-3084`, `:3086-3116` | «max index» — глобальный по каталогу |

`RetentionPolicy` целиком: `crates/journal/src/segments.rs:2377-2396` (`retain_days`,
`keep_min_segments`, `cold_root`, `min_free_bytes`, `checkpoint_covered_through_seq`,
`allow_prune_without_checkpoint`).

**Операторский дайджест (JR-I-12/TD-067):** `--mode replay-digest` в
`crates/journal/src/bin/journal-retention.rs:209`, `:218-226` (`--from/--to/--expect`),
запись `journal.replay-digest.json` — `:70`, exit-код расхождения `4` — `:72-74`, `:454-512`.
Открытый долг: он не доставлен в ops-поверхность — **TD-075** (`TECH-DEBT.md:18-33`).

---

## 4. Эпохи

### 4.1 Как `EPOCH_ID` попадает в журнал

1. Recorder читает env: `crates/recorder/src/main.rs:481` —
   `std::env::var("EPOCH_ID").unwrap_or_else(|_| default_epoch_id_now())`;
   дефолт `own-<UTC-YYYY-MM>` — `:504-517` (`default_epoch_id_now`).
2. Кладётся в `WriterConfig` — `crates/recorder/src/main.rs:482`, тип —
   `crates/journal/src/segments.rs:48-57` (поле `epoch_id: String`), конструктор
   `own_capture` — `:61-69`.
3. Попадает в `SegmentHeader` при открытии/ротации —
   `crates/journal/src/lib.rs:148-155` (при `open_with`), `:275-282` (при `rotate`).
4. Пишется первым фреймом сегмента — `crates/journal/src/segments.rs:1267-1276`.

**В `Event` эпохи НЕТ** — она живёт ТОЛЬКО в заголовке сегмента
(`crates/contracts/src/lib.rs:96-110`). Гранулярность эпохи = сегмент.

### 4.2 Где фильтруется

- `EpochFilter` — `crates/journal/src/segments.rs:86-105`:
  `OwnCaptureOnly` (по `source == DataSource::OwnCapture`), `Explicit(Vec<String>)` (по
  `epoch_id`), `All`. `accepts()` — `:98-104`.
- Фильтр применяется в `stream_from` — `crates/journal/src/segments.rs:1017-1022`
  (сегмент целиком проходит или целиком отбрасывается).
- Дефолта «всё подряд» намеренно нет — `crates/journal/src/segments.rs:81-85`;
  потребитель обязан НАЗВАТЬ фильтр (типовой барьер).
- Прод-дефолт research/CLI: `OwnCaptureOnly` —
  `crates/research-cli/src/main.rs:62-69`, `crates/research-cli/src/bin/latency_probe.rs:88`.
- Фингерпринт фильтра в чекпоинте — `crates/gateway/src/lib.rs:2070-2071`.
- `DataSource` — `crates/contracts/src/lib.rs:84-92` (`OwnCapture`/`Vendor`/`Synthetic`).

### 4.3 Что означает граница эпохи

**Две РАЗНЫЕ вещи под одним словом:**

1. **Машинная изоляция сегмента (schema/epoch reuse-гейт).**
   `decide_open_segment` reuse'ит существующий сегмент ТОЛЬКО при совпадении
   `source && provenance && epoch_id && schema_version` —
   `crates/journal/src/segments.rs:2275-2287`.
   Смена `EPOCH_ID` (или bump `SCHEMA_VERSION`) машинно открывает НОВЫЙ сегмент.
   `SCHEMA_VERSION = 4` — `crates/contracts/src/lib.rs:26`; история bump'ов `:20-25`;
   `SCHEMA_VERSION_PRE_HEADER = 1` — `:30`; `LEGACY_EPOCH_ID` — `:33`.
   Мотивация (TD-031: provenance в контейнере — КОНСТАНТА, изоляция по нему воид) —
   `crates/journal/src/segments.rs:2268-2274` и `crates/contracts/src/lib.rs:13-19`.

2. **Семантическая эпоха (границы дефектов данных) — записывается ГЛОБАЛЬНЫМ `seq`.**
   `docs/data-epochs.md` — LIVING DOC. Запись E-001 (инвертированные стороны трейдов
   Hyperliquid): граница — `docs/data-epochs.md:34-46`, таблица с
   **`граница эпохи (оценка) = seq ≈ 123 205 544`**.
   Т.е. реестр дефектов адресуется одним глобальным seq-числом на весь журнал.

**`docs/rfc/CT-RFC-06-l2delta.md` §3** (запрошено в мандате) — «Эпохи: как читатель отличает
снапшот-эпоху от дельта-эпохи», начало секции — `docs/rfc/CT-RFC-06-l2delta.md:126`:
- `:138-142` — reuse требует совпадения четвёрки полей; смена `epoch_id` машинно открывает
  новый сегмент;
- `:144` — «Recorder берёт `epoch_id` из env `EPOCH_ID` (`crates/recorder/src/main.rs:479-482`)»;
- `:149-156` — каждое изменение состава/роли L2Delta-эмиссии = новый `epoch_id`, выставляемый
  ДО рестарта, + запись в `docs/data-epochs.md`; читатель различает эпохи чтением
  `SegmentHeader.epoch_id` + реестром;
- `:167-179` — **явно признанная дыра:** машинного fail-closed на «состав эмиссии изменился,
  а `epoch_id` — нет» НЕ СУЩЕСТВУЕТ; код сравнивает `epoch_id` со своим же конфигом, а не с
  фактическим составом потока. Забытый оператором `EPOCH_ID` = тихая смена семантики.

---

## 5. Есть ли уже понятие «инструмент» в раскладке

### 5.1 Где живёт символ

- `MdEvent { venue: Venue, symbol: String, payload: MdPayload }` —
  `crates/contracts/src/lib.rs:228-233`. Док: `symbol` — канонический тикер площадки как есть
  (Binance `"BTCUSDT"` / Hyperliquid `"BTC"`); нормализация кросс-venue — задача выше
  (`:225-227`). Для `MarginRate`/`MarginInventory` `symbol` = АКТИВ (`"USDT"`/`"USDC"`) —
  `:227`, `:270`, `:302-303`.
- `EventKind` — `crates/contracts/src/lib.rs:147-154`: ровно два варианта — `Sys(SysEvent)`
  и `Md(MdEvent)`. **У `Sys`-событий символа НЕТ** (`Heartbeat`, `ConnUp(Venue)`,
  `ConnDown(Venue)` — `:156-167`), кроме `ReconDivergence(ReconAudit)`, у которого символ ЕСТЬ
  (`:174-188`, поле `symbol` на `:178`).
- `Venue` — `crates/contracts/src/lib.rs:200-209`: `Binance`, `Hyperliquid`, `BinanceFutures`.
- Хелпер конструирования — `crates/contracts/src/lib.rs:317-326` (`EventKind::md`).

### 5.2 Можно ли дёшево получить символ без полного разбора payload

**Прямого дешёвого пути в коде НЕ СУЩЕСТВУЕТ.** Факты:

- В event-фрейме нет ни одного поля-заголовка кроме `[u32 len]` и `[u32 crc]` —
  `crates/journal/src/segments.rs:2310-2319`, `:442-480`. Символ, venue, вид события живут
  ВНУТРИ postcard-payload.
- Порядок полей в payload (postcard = последовательная, self-describing-less кодировка):
  `Event{ seq: u64, ts_mono_ns: u64, ts_wall_ms: i64, kind }` —
  `crates/contracts/src/lib.rs:138-143`; далее дискриминант `EventKind` (Sys=0, Md=1) —
  `:148-154`; далее `MdEvent{ venue, symbol, payload }` — `:229-233`.
  То есть `symbol` идёт ПОСЛЕ трёх varint'ов + 1 байта варианта + 1 байта venue — доступ
  к нему требует последовательного разбора префикса, но НЕ требует разбора `payload`
  (`Vec<Level>` идёт после символа).
- Прецедент такого частичного разбора в коде ЕСТЬ, но только для `seq`:
  `crates/journal/src/segments.rs:1866` (`SEQ_PREFIX_CAP = 10` — ведущий varint) и
  `:1930-1933` (`postcard::take_from_bytes::<u64>(&prefix)`).
  **Аналога `take_from_bytes` до `symbol` в репозитории не найдено.**
- Индекса/сайдкара «символ → offset/seq» в каталоге журнала не найдено (`grep` по
  `crates/journal/**` — единственные вспомогательные файлы: `journal.meta`,
  `journal.legacy.json`, `journal.force-next-seq[.applied].json`, `journal.replay-digest.json`,
  `recorder.heartbeat`).

### 5.3 Единственное место, где инструмент уже структурно присутствует

- `gateway::Selector{venue, symbol, ...}` — `crates/gateway/src/lib.rs:109-124`
  (и фингерпринт → имя файла чекпоинта, `:1987-1990`, `:2126-2140`).
- Per-(venue,symbol) живые книги в recorder'е (in-memory, не на диске) —
  `crates/recorder/src/main.rs:119-124`, `:300`.
- Per-(venue,symbol) recon-обвязка — `crates/recorder/src/main.rs:124-135`, `:426-440`.
- Списки символов — только на входе, из env:
  `crates/recorder/src/main.rs:374` (`BINANCE_SYMBOLS`, дефолт `["BTCUSDT","ETHUSDT"]`),
  `:381` (`HL_COINS`, дефолт `["BTC","ETH"]`), `:388` (`BINANCE_FUTURES_SYMBOLS`).
- L2Delta-allow-list (на `origin/main`) — ХАРДКОД-константа:
  `crates/venue-binance/src/lib.rs:485` и `crates/venue-binance-futures/src/lib.rs:460` —
  `const L2DELTA_CAPTURE_SYMBOLS: &[&str] = &["BTCUSDT"]`; применение — `venue-binance/src/lib.rs:251`,
  `venue-binance-futures/src/lib.rs:642`.
  (Работа M-45 по превращению этого в env-параметризуемый список в `origin/main` НЕ смёржена —
  см. `docs/archive/orchestration-log-2026-07-08.md:360-366`; факт подтверждён самим кодом на main выше.)

---

## 6. Размеры и объёмы фактом

| Величина | Значение | Источник |
|---|---|---|
| Порог ротации сегмента | **1 GiB** (`1024*1024*1024`) | `crates/journal/src/segments.rs:41` |
| Порог disk-guard | **10 GiB** свободного места | `crates/journal/src/segments.rs:44` |
| Окно хвостового скана при `open()` | **4 MiB** | `crates/journal/src/lib.rs:52` (`TAIL_SCAN_CHUNK`) |
| Чанк потокового скана пола | **64 KiB** | `crates/journal/src/segments.rs:1792` (`READABLE_SCAN_CHUNK`), carry — `:1798` |
| Бюджет работы скана пола | **8 × 1 GiB = 8 GiB** | `crates/journal/src/segments.rs:1579` |
| Санити-кап длины фрейма | **64 MiB** | `crates/journal/src/segments.rs:431` |
| Отпечаток legacy-декларации | первый **1 MiB** | `crates/contracts/src/lib.rs:46` (`LEGACY_FINGERPRINT_BYTES`) |
| Буфер writer'а | 256 KiB | `crates/journal/src/segments.rs:2353` |
| Уровень zstd компакции | 3 (**9.1×** на боевом сегменте) | `crates/journal/src/segments.rs:3032-3033` |
| **Сырой объём/сут (ЗАМЕР 2026-07-14)** | **8.83 GB/сут** | `docs/06-data-layer-and-storage.md:48` |
| **Сжатый/сут** | **~1 GB/сут** | `docs/06-data-layer-and-storage.md:49` |
| Год при текущем наборе | **~360 GB сжато** | `docs/06-data-layer-and-storage.md:50` |
| Запас на 160 GB SSD | **12 дней** до disk-guard | `docs/06-data-layer-and-storage.md:51` |
| Состав байт журнала (разбор 30 MB боевого хвоста, 34 109 событий) | **~96% — `L2Snapshot` × 4 потока (venue×symbol), 20–31 KB каждый, 1800–2850 уровней, ~1/с**; ~3.4% `Trade` (52 B); <1% funding/OI/liquidation/Sys | `docs/06-data-layer-and-storage.md:53-60` |
| **Число сегментов на проде (цитируется в коде)** | **158 сегментов ≈ 140 GiB сырых** | `crates/journal/src/segments.rs:1538` |
| **Прод: события / объём** | **~148.6 млн событий, 27 GB сжатых** (VPS `167.233.192.131`) | `docs/NEXT-SESSION-PROMPT.md:100` |
| То же, в комментарии кода | «прод 27 GB / 146M событий» | `crates/journal/src/segments.rs:1074-1075` |
| Legacy `segment-00000000.jrnl` | **15 GB**, без v2-магии, задекларирован в `journal.legacy.json` | `crates/journal/src/segments.rs:3087-3092`; та же цифра — `TECH-DEBT.md:451-452` |
| Историческая цифра (устарела) | «прод 2.65 GiB сегмент», «2.8 GB/сут» | `crates/journal/src/lib.rs:51`, `crates/journal/src/segments.rs:8`, `:2364-2365` |
| Замер ресурсов 2026-08-02 | `hft-recorder CPU=1.88% MEM=31.95MiB`; 4 vCPU / 7 GB RAM | `docs/PENDING-SIGNATURE.md:251-262` |
| Время полного rebuild кокпита без чекпоинта | **409.74 s на 18 GB журнала** | `crates/gateway-serve/src/lib.rs:169-172` |

**Важная оговорка по числам:** `docs/06-data-layer-and-storage.md:39-70` содержит явное
предупреждение — предыдущая оценка ошиблась в 20–30 раз, и урок сформулирован как «объёмные
оценки обязаны быть ЗАМЕРЕНЫ на живом фиде и перемеряться при каждом расширении набора данных»
(`:67-70`). Числа выше — последние ЗАПИСАННЫЕ в дереве замеры (14.07 и 02.08); я их
не перемерял (нет доступа к прод-VPS из этой сессии).

**Фактическое число сегментов СЕЙЧАС** — не найдено в дереве; ближайшие цифры это
«158 сегментов» (`segments.rs:1538`, контекст M-52) и «~120 сжатых сегментов»
(`docs/08-arch-improvement-roadmap.md:51`).

---

## Что мешает шардированию больше всего

Ранжировано по силе завязки на единый общий порядок.

1. **`seq` — один счётчик на весь каталог, и он же ось всех внешних артефактов.**
   Присваивается в единственной точке `crates/journal/src/lib.rs:214-219`/`:241`;
   восстанавливается как `max(journal.meta, tail(latest segment)+1)` —
   `crates/journal/src/segments.rs:1466-1476`. Один `journal.meta` на каталог
   (`crates/journal/src/lib.rs:47`, `:349-354`). На это число уже завязаны: курсор чекпоинта
   (`crates/gateway/src/lib.rs:148-150`), покрытие ретеншена
   (`crates/journal/src/segments.rs:2387-2391`), провенанс истории
   (`crates/gateway/src/lib.rs:2087-2096`), метрики (`crates/ops/src/metrics.rs:50`) и —
   самое неприятное — **реестр семантических дефектов данных**, где граница эпохи записана
   как одно глобальное число `seq ≈ 123 205 544` (`docs/data-epochs.md:34-46`).

2. **Порядок = «индекс файла, затем позиция в файле»; merge-читателя не существует.**
   `EventStream` читает сегменты строго по одному, по возрастанию индекса
   (`crates/journal/src/segments.rs:901-928`, `:931-988`), список формируется
   `dedup_indexed_paths` через `BTreeMap<u32, PathBuf>` (`:650-690`). Ни одного места,
   где два независимых потока сливаются по `seq`/времени, я не нашёл. Единственный
   публичный энумератор `list_segments` (`:714-746`) возвращает плоский список одного каталога.

3. **Guard монотонности `first_seq` — три прод-пути отказывают на «немонотонном каталоге».**
   `check_first_seq_monotonic` (`crates/journal/src/segments.rs:1724-1755`) применяется в
   `segments()` (`:724-743`), `read_all` (`crates/journal/src/lib.rs:442-445`) и
   `readable_floor` (`segments.rs:1618`). Любая раскладка, где в одном каталоге лежат сегменты
   с непересекающимися/перемежающимися диапазонами seq, СЕГОДНЯ трактуется как
   «re-stitch архива под чужим индексом» и даёт `Err` (текст — `:1742-1752`).
   Оракул на это — `crates/journal/tests/red_stitch_monotonic.rs` (sacred).

4. **Ретеншен целиком построен на «активный = max index» и на непрерывной цепочке seq.**
   Активный сегмент — `crates/journal/src/segments.rs:2490-2492` (и `:3078-3084` в компакции);
   `last_seq(seg) = next_seg.first_seq − 1` — `:2856-2877`; гейт prune по ОДНОМУ числу
   покрытия — `:2557-2649`; `keep_min_segments` = «последние N по индексу» — `:2538-2555`;
   `disk_pressure` = один `statvfs` на каталог — `:2658-2660` + `:1227-1242`.
   При N шардов «активных» становится N, «последние N по индексу» перестаёт означать
   «последние по времени», а покрытие чекпоинтом перестаёт быть одним числом.

5. **DET-I-1 определён только для ОДНОГО каталога и не имеет операции композиции.**
   `replay_digest(dir, filter, from, to)` — `crates/journal/src/segments.rs:1083-1122`:
   вход — каталог, свёртка — по возрастанию глобального seq. Оракул `det_8`
   (`crates/journal/tests/red_det_replay_digest.rs:529-580`) требует совпадения дайджеста по
   ДВУМ путям чтения одного каталога; понятия «дайджест шарда» / «дайджест = f(дайджестов
   шардов)» в коде и оракулах НЕТ. Операторская поверхность (`--mode replay-digest`,
   `crates/journal/src/bin/journal-retention.rs:454-512`, запись `journal.replay-digest.json`)
   тоже однокаталожная.

6. **Gateway-чекпоинт валидирует ВЕСЬ видимый набор сегментов, хотя работает по одному символу.**
   `journal_lineage: Vec<SegmentHeader>` всех видимых сегментов —
   `crates/gateway/src/lib.rs:2084`, сбор `:2443-2450`, сверка `:2666-2700+`;
   `first_visible_seq` = минимум по всем сегментам — `:2040-2049`.
   При этом `Selector` уже per-(venue,symbol) (`:109-124`) и фильтрует ПОСЛЕ декодирования
   каждого события (`:756`, `:787`, `:820`, `:850`). Это и «мешает» (lineage глобален),
   и «помогает» (сама модель подписки уже инструментная).

7. **Символ недоступен без разбора префикса payload, и `Sys`-события символа не имеют вовсе.**
   Фрейм несёт только `[len][postcard][crc]` (`crates/journal/src/segments.rs:2310-2319`);
   `symbol` лежит внутри `MdEvent` (`crates/contracts/src/lib.rs:229-233`) после трёх varint'ов
   `Event` (`:138-143`) и дискриминанта `EventKind` (`:148-154`).
   Прецедент частичного декода есть только для ведущего `seq`
   (`crates/journal/src/segments.rs:1866`, `:1930-1933`).
   `EventKind::Sys` (`Heartbeat`/`ConnUp`/`ConnDown`, `contracts/src/lib.rs:156-167`) не относится
   ни к какому инструменту — куда их класть при шардировании, сегодня не определено ничем.

---

## Открытые вопросы (чтением кода не выясняется)

1. **Фактическое состояние прод-каталога прямо сейчас.** Число сегментов, доля `.zst` vs raw,
   размер legacy `segment-00000000.jrnl`, свободное место. В дереве есть только цитаты
   (`segments.rs:1538` — 158 сегментов ≈140 GiB; `docs/NEXT-SESSION-PROMPT.md:100` — 27 GB /
   148.6 млн событий; `TECH-DEBT.md:451-452` — 15 GB legacy). Требуется ssh на
   `167.233.192.131` (`gates.md` §8) — из этой сессии я его не делал.

2. **Реальное распределение объёма по (venue, symbol).** `docs/06:53-62` даёт долю по ТИПАМ
   payload (96% — L2Snapshot × 4 потока), но не разбивку по конкретным инструментам.
   Без замера нельзя сказать, будет ли шард на инструмент давать равные или сильно
   перекошенные файлы.

3. **Целевая мощность набора инструментов.** «Топ-300» упоминается как намерение founder'а
   (`docs/archive/orchestration-log-2026-07-08.md:780-783`, `docs/PENDING-SIGNATURE.md:249-251`), но точного
   числа шардов / политики группировки (шард на инструмент vs на группу vs на venue) в дереве
   не зафиксировано.

4. **Судьба уже записанных 27 GB монолита.** Мигрировать (перенарезать по шардам, что
   означает пересчёт или сохранение старых seq), оставить как «legacy-шард», или заморозить —
   решение founder'а/architect'а. В коде есть только запрет на сжатие legacy
   (`segments.rs:3086-3116`) и механизм деклараций (`journal.legacy.json`).

5. **Семантика удаления шарда и форма «мета-журнала удалений».** `docs/PENDING-SIGNATURE.md:236-241`
   описывает намерение («факт удаления пишется в мета-журнал»), но ни типа, ни файла, ни
   RFC под это в дереве нет — `grep` по `crates/` даёт 0 совпадений на «шард/shard».

6. **Есть ли внешние потребители, которые уже запомнили абсолютные `seq`.** `docs/data-epochs.md`
   — точно да (E-001 привязан к `seq ≈ 123.2M`). Насколько ещё где-то (front-end, сохранённые
   отчёты, `research/reports/`, `research/trials-ledger.json`) зафиксированы конкретные seq —
   требует отдельной сверки с founder'ом.

7. **Storage Box / cold-tier.** `docs/archive/orchestration-log-2026-07-08.md:943` — ожидание ~09.08; политика
   выгрузки шардов в холодное хранилище (шард целиком vs посегментно) зависит от того, что
   реально будет смонтировано.

8. **Требуется ли сохранение общего хронологического порядка между шардами.** Из кода это
   не выводится: DET-I-1 сегодня определён на одном каталоге, а нужен ли «глобальный
   бит-идентичный replay всех инструментов вместе» после шардирования — продуктовое решение
   (`docs/DESIGN.md` §1 «`replay(journal) == реальность`» не уточняет гранулярность).
