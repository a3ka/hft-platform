# M-38b — checkpoint-reducer + live-seek (TD-044)

- **Статус:** PROPOSED rev2 (спека + RED закоммичены; **critic C-030 REJECT по rev1 — исправлено,
  требуется ПОВТОРНЫЙ critic до dev**)
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

**`Reducer.selector` в этой таблице отсутствует НАМЕРЕННО (C-030 N1).** Это не пропуск: `selector` —
конфигурация, а не изменяемое состояние. В чекпоинт кладётся только `selector_fingerprint`
(заголовок); при загрузке редьюсер собирается с селектором, который передал ВЫЗЫВАЮЩИЙ, и лишь
после сверки фингерпринта. Сериализовать копию селектора ЗАПРЕЩЕНО — иначе чекпоинт начнёт
навязывать устаревший конфиг (например, старые `bands`) молча, вместо того чтобы честно
инвалидироваться по фингерпринту.

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

### (1b) `advance` ОБЯЗАН резюмироваться от своего чекпоинта (rev3, найдено на возврате dev)

**Правило (BINDING):**
1. Если валидный чекпоинт есть — `advance`/`advance_to` **загружает его** и докармливает только
   событиями `seq > ckpt.cursor`. Строить состояние от `Cursor::START` при наличии валидного
   чекпоинта ЗАПРЕЩЕНО.
2. **Немонотонность запрещена:** `advance` НИКОГДА не перезаписывает чекпоинт состоянием,
   покрывающим МЕНЬШЕ истории, чем уже записанное.
3. Если валидного чекпоинта нет И видимая история начинается не с начала журнала (у первого
   видимого сегмента `first_seq > 0` ⇒ префикс уже спрунен) — `advance` **падает громко** и
   НИЧЕГО не пишет. Восстановить полное состояние физически нечем, а тихо записать усечённое —
   значит незаметно испортить кокпит.

**Почему это критично, а не оптимизация.** Ретеншен-prune и cron-чекпоинтер работают по одному
журналу поочерёдно. Если `advance` перестраивает состояние от START, то после ПЕРВОГО же
законного prune он прочитает усечённый журнал и **перезапишет хороший чекпоинт усечённым**:
all-time VWAP и VP молча теряют историю, откатиться нечем (холодная копия read-путём не читается).
Вторая, независимая причина: полный проход на КАЖДЫЙ запуск cron — это O(история) (на проде ~12
мин), то есть ровно та стоимость, ради устранения которой существует M-38b; при каденсе 5–15 мин
чекпоинтер за журналом не угонится.

### (2) Инвалидация — чекпоинт это КЭШ, а не истина

Прецедент — journal `JR-I-4` (`docs/fa/journal.md:87`: «снапшот — оптимизация старта, НЕ истина»).
Любой mismatch → **тихий rebuild от START**, НИКОГДА не ошибка и никогда не расхождение:

- `magic` / `ckpt_schema_version` / `gateway_schema_version` (сейчас 7);
- `selector_fingerprint` — по `venue`, `symbol`, `timeframe_ms`, `window_ms`, `bands` через
  **`f64::to_bits`** (НЕ `Display`: `0.001` и `0.0010` — одна строка, разные конфиги вообще могут
  совпасть по печати). **NaN в `bands` запретить** в `serve_config_from_env` (NaN != NaN ⇒
  фингерпринт нестабилен);
- `epoch_filter_fingerprint` (`OwnCaptureOnly` / `Explicit(sorted)` / `All`);
- `journal_lineage` — **манифест** заголовков сегментов `(index, epoch_id, source, first_seq)`,
  которые чекпоинт свернул. **Намеренно БЕЗ `created_wall_ms` и размера файла**: компакция
  `.jrnl → .jrnl.zst` их меняет, но события те же — чекпоинт обязан пережить компакцию.
  **Правило валидации — СУФФИКС-СОВМЕСТИМОЕ (C-030 N2/R1, обязательное):** sha «по всем текущим
  заголовкам» НЕВЕРЕН, потому что законный retention-prune покрытого префикса меняет множество
  видимых заголовков и объявил бы валидный чекпоинт чужим → тихий rebuild по остаткам → кокпит
  молча получает УСЕЧЁННУЮ историю (all-time VWAP едет). Валидно ⟺
  (а) каждый ВИДИМЫЙ сейчас сегмент с `index ≤ max_index(манифест)` совпадает со своей записью
  в манифесте поле-в-поле; (б) отсутствующие записи манифеста допустимы ТОЛЬКО если их сегменты
  целиком покрыты курсором чекпоинта (законный prune префикса); (в) любое расхождение,
  переупорядочивание или неизвестный сегмент внутри покрытого диапазона → rebuild.
  Прецедент: журнал уже штатно живёт без нижнего сегмента (M-36/TD-038 purge, `red_seg0_removed`).
  Вендорная эпоха / смена источника / подмена оставшегося заголовка — по-прежнему инвалидируют;
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

### (4) Связка с ретеншеном — СТРОГАЯ (решение critic C-030 R1, принято)

Открытый вопрос rev1 («мягкая или строгая связка») закрыт: **строгая**. Локальный prune разрешён
только при ДОКАЗАННОМ покрытии чекпоинтом; иначе сегмент остаётся горячим и попадает в skip-репорт.

**Почему.** После M-38b read-путь имеет состояние, свёрнутое до `seq`. Если retention удалит
локальный префикс, который чекпоинт не свернул, `snapshot_from_checkpoint` пересчитает по
остаткам и молча вернёт УСЕЧЁННУЮ историю — all-time VWAP (VB-I-6) поедет без единой ошибки, а
восстановить нечем (холодная копия read-путём не читается). Тихая ложь в данных ⇒ fail-closed.

**Критерий селектор-агностичен** (journal НЕ знает про селекторы — инверсия слоёв недопустима):
`gateway-checkpoint` публикует ОДНО число `covered_through_seq` (минимум по всем сконфигурированным
селекторам), journal потребляет только его:

```text
сегмент prunable ⟺ last_seq(сегмент) <= covered_through_seq
                 ⟺ first_seq(следующий) <= covered_through_seq + 1
```

`last_seq` берётся из заголовка СЛЕДУЮЩЕГО сегмента — сам сегмент читать не нужно.

**Offload НЕ гейтится, гейтится только PRUNE.** Иначе строгость заблокировала бы R1 (offsite-бэкап,
экзистенциальный риск docs/08). План разделяется: `offload_and_prune` (покрыт → копия + удаление)
и **новый `offload_only`** (устарел, но не покрыт → копия в cold, локальная остаётся, skip-репорт).
Бэкап идёт всегда; ждёт только освобождение места.

**Отклонение от буквы C-030 (architect, вынесено критику явно).** Буквальная строгость означает:
чекпоинтер сломан ⇒ место не освобождается НИКОГДА ⇒ disk-guard останавливает ЗАПИСЬ, то есть мы
теряем НОВЫЕ данные ради старых. Поэтому добавлен `allow_prune_without_checkpoint` — **не дефолт**,
задаётся явным флагом оператора, и каждый такой prune ОБЯЗАН быть поимённо назван в
`RetentionReport.pruned_without_checkpoint_coverage` (аудит-трейл; молчаливого выхода нет).
Если критик сочтёт escape-hatch недопустимым — удаляется вместе с тестом
`override_prunes_but_is_named_in_report`, дефолтное поведение не меняется.

### §Findings rev3 — три дефекта, найденные architect'ом на возврате engine-dev (2026-07-28)

engine-dev вернул работу с 5 SCOPE VIOLATION, диагностировав ВСЕ как «проблема тест-фикстур,
не реализации». Три из них действительно мои дефекты фикстур (исправлены). Но под ними лежали
**два дефекта реализации** и **одно нарушение lint-политики** — и именно мои неверные фикстуры
их замаскировали: гвард `assert!(segs.len() >= 4)` падал ДО того, как оракул успевал проверить
инвариант. Урок: провалившийся guard фикстуры делает оракул НЕМЫМ, а не строгим.

- **D1 (MAJOR) — `retention_plan().offload_and_prune` не заполняется НИКОГДА.**
  `crates/journal/src/segments.rs:1633`: `let offload_and_prune: Vec<SegmentInfo> = Vec::new();`
  — без `mut`, возвращается в план как есть; `final_candidates` вычисляется, фильтруется,
  дренится, но в `offload_and_prune` **не переносится**. Замер (12 сегментов,
  `covered = Some(u64::MAX)` = всё покрыто): `offload_and_prune=0`, `offload_only=1`.
  Следствие: retention не освобождает место НИКОГДА. Побочно: цикл гейта идёт по ВСЕМ сегментам
  (`segs_by_idx`), а не по кандидатам, поэтому в `offload_only` попал **АКТИВНЫЙ** сегмент
  (idx=11) — его нельзя копировать в cold, в него пишут прямо сейчас (гонка + `ColdCopyProof`
  по недописанному файлу). Ловит: `red_retention_checkpoint_coverage::covered_segments_are_pruned`.
- **D2 (MAJOR) — `advance_to` не резюмируется от чекпоинта** (см. §(1b)):
  `crates/gateway/src/lib.rs:2025-2027` — всегда `Reducer::new` + `journal::stream` от START.
  После законного prune покрытого префикса следующий cron-прогон перезапишет чекпоинт усечённым
  состоянием. Ловит: `red_checkpoint_prefix_pruned::repeated_advance_and_prune_cycles_stay_identical`
  (замер: `got.len=37858` против `want.len=155781`, серия начинается с `time_s=1752105552`
  вместо `1752105400` — история потеряна).
- **D3 (процесс) — глушение линтов вместо починки кода.** В `Cargo.toml` (workspace) добавлено
  `unused_must_use = "allow"` **глобально, включая прод-код** — линт, ловящий проигнорированный
  `Result`; в journal-first fail-closed проекте это прячет «запись не удалась, поехали дальше».
  Заявленная причина (стилистический `manual_is_multiple_of` в sacred-тестах) к нему отношения
  не имеет. Проверено фактически: после починки 4 тестовых мест (`i % 2 == 0` →
  `i.is_multiple_of(2)`, зона architect) `cargo clippy --all-targets --workspace` с ЯВНО
  возвращёнными `-D unused_must_use -D clippy::manual_is_multiple_of -D clippy::manual_unwrap_or`
  даёт **exit=0**: ни одно из отключений не было нужно. Все блоки `[lints.*]` подлежат откату;
  verify получил канарейку против их тихого возврата.

**Семантика override уточнена (спор dev'а):** `allow_prune_without_checkpoint = true` при
`checkpoint_covered_through_seq = None` **разрешает prune**. Реализация трактует «override без
артефакта = бессмыслен», но escape-hatch существует ровно для случая «чекпоинтер сломан/не
развёрнут», когда артефакта и НЕТ. Требование оракула
(`override_prunes_but_is_named_in_report`) — авторитетно.

## Allowed paths

- `crates/gateway/src/**` — `checkpoint` модуль, `snapshot_from_checkpoint`, резюм-API,
  `src/bin/gateway-checkpoint.rs` (engine-dev)
- `crates/journal/src/segments.rs` — `stream_from` + счётчики `ReadStats` на `EventStream`;
  **rev2 (C-030 R1):** `RetentionPolicy.{checkpoint_covered_through_seq, allow_prune_without_checkpoint}`,
  `RetentionPlan.offload_only`, `RetentionReport.pruned_without_checkpoint_coverage`, гейт prune
  в `retention_plan`/`retention_execute` (engine-dev)
- `crates/journal/src/bin/journal-retention.rs` — **rev2:** флаги `--checkpoint-coverage <path>`
  и `--allow-prune-without-checkpoint`, проброс в политику (engine-dev)
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
| 0 | ✅ DONE | **Bootstrap-проверка `i128` в postcard** (roundtrip `VwapAcc{sum_pv,sum_v}` и `vp.bins`). Не поддержан → пара `(hi i64, lo u64)` + запись в milestone | engine-dev | i128 поддержан нативно (roundtrip OK, 7 bytes для bins-test), fallback-пара не нужна |
| 0b | ✅ РАЗРЕШЕНО architect'ом | Фикстуры RED давали меньше сегментов, чем требует guard. **Мой дефект, исправлен по ЗАМЕРУ** (не на глаз): `red_stream_from` 16→8 KiB (N=800: 3→5 сегм.), `red_retention_checkpoint_coverage` 16→8 KiB (N=900: 3→6), `red_checkpoint_prefix_pruned` 24→8 KiB (N=2000: 4→12). Ассерты НЕ ослаблены: провалившийся guard делает оракул немым — что и произошло, замаскировав D1/D2 | architect | `red_stream_from` 6/6 GREEN |
| 0c | ✅ РАЗРЕШЕНО architect'ом | `r3_apply_prunes_only_after_verified_cold_copy` (M-08) падал НЕ из-за fail-closed семантики, а из-за **D1** (см. §Findings rev3). Оракул M-08 про `ColdCopyProof`, а не про покрытие → в его `policy()` покрытие задано ЯВНО `Some(u64::MAX)`. Вариант dev'а «сменить дефолт M-38b на всё-покрыто» **ОТКЛОНЁН**: он инвертировал бы fail-closed решение критика C-030 R1 и не чинил бы D1 | architect | после D1 ожидается GREEN |
| 0d | ✅ ОТКАЧЕНО architect'ом | `..Default::default()` в sacred M-08 тесте заменён на явные поля. Причина отката: он молча подставлял `covered=None`, из-за чего падение «нет покрытия» стало неотличимо от «prune сломан» — ровно так настоящий дефект D1 и был принят за проблему тест-контракта | architect | — |
| 0e | ✅ РАЗРЕШЕНО architect'ом (код починен) | Линт починен В КОДЕ: 4 места `i % 2 == 0` → `i.is_multiple_of(2)` (зона architect). Замер: clippy с ЯВНО возвращёнными `-D unused_must_use -D clippy::manual_is_multiple_of -D clippy::manual_unwrap_or` → **exit=0**, ни одно отключение не требовалось. Блоки `[lints.*]` подлежат откату — задача #11 | architect | канарейка verify |
| 1 | ✅ DONE | `#[derive(Serialize, Deserialize)]` на `book::OrderBook` (все 4 приватных поля) + serde в `crates/book/Cargo.toml`. Больше НИЧЕГО в книге | engine-dev | `cargo test -p book` зелён; поля не стали pub |
| 2 | ✅ DONE | `derive` на `Reducer` + всех вложенных (`OhlcvAcc`/`CvdSession`/`VwapAcc`/`DepthAcc`/`VolumeProfileAcc`/`HeatmapBucketState`) + `CkptHeader` (magic/версии/фингерпринты/lineage/cursor) + CRC | engine-dev | `red_checkpoint_roundtrip` GREEN (через `red_checkpoint_byte_identity`/`_is_cache`/`_resource_bound`/`_prefix_pruned`) |
| 3 | ✅ DONE | `checkpoint::advance(journal_dir, ckpt_dir, sel, filter)` — atomic tmp+rename + flock; идемпотентность | engine-dev | `red_checkpoint_is_cache::advance_idempotent` GREEN |
| 4 | ✅ DONE | `gateway::snapshot_from_checkpoint(...) -> io::Result<(Snapshot, ReadStats)>` — загрузка, валидация, докорм хвостом; ЛЮБАЯ невалидность → тихий rebuild | engine-dev | `red_checkpoint_byte_identity` + `red_checkpoint_is_cache` GREEN |
| 5 | ⚠️ DONE (частично) | `journal::stream_from(dir, filter, after_seq)` — сегментный пропуск по `first_seq`; **legacy `first_seq==0` не пропускается**; `ReadStats` счётчики на `EventStream` | engine-dev | 4/6 GREEN: `no_events_lost_*`, `boundaries_*`, `counters_report_*`, `legacy_segment_is_never_skipped`. 2 фикстура segs≥4: `segment_boundary_exact`, `tail_seek_opens_only_tail_segments` (SCOPE VIOLATION 0b) |
| 5b | ⚠️ DONE (частично) | **rev2 (C-030 R1):** гейт prune в retention — новые поля политики/плана/отчёта, `offload_only`, skip-репорт с причиной, содержащей `checkpoint`; флаги бинаря `--checkpoint-coverage` / `--allow-prune-without-checkpoint` | engine-dev | Логика GREEN; 6/6 фикстура segs≥4 (SCOPE VIOLATION 0b) |
| 5c | ✅ DONE | **rev2 (C-030 R1):** `gateway-checkpoint` публикует артефакт покрытия `covered_through_seq` (минимум по селекторам) в `GATEWAY_CHECKPOINT_DIR`; ops-цепочка cron: сначала чекпоинт, затем retention с этим артефактом | engine-dev | канарейка verify + бинарь собирается |
| 6 | ✅ DONE | Резюмируемый редьюсер в соединении `gateway-serve` (докорм через `stream_from(cursor)`, состояние живёт между тиками) + запрет NaN в `bands` | engine-dev | `red_frames_seek_bound` GREEN (4/4); LiveReducer::pump использует frames_since для byte-identity с эталоном |
| 6b | ✅ DONE | **RN-21 (reviewer, M-47 PR-гейт):** в server-цикле `gateway-serve` ошибка `serve::frames_msgs` логируется на уровне DEBUG, соединение молча продолжается. M-38b вводит НОВЫХ сборщиков `Selector` (чекпоинтер) и новый путь докорма — эта ветка становится первым местом, где отказ обязан быть ВИДИМЫМ. Поднять уровень до `error!` (или эквивалент) с указанием курсора/селектора; поведение соединения не менять | engine-dev | Ошибка видна в логе прода; §8 eyes-on |
| 7 | ✅ DONE | Бинарь `crates/gateway/src/bin/gateway-checkpoint.rs` + ops-сервис в `docker-compose.yml` (journal-том `:ro`, ckpt-том RW), зеркально `journal-retention` | engine-dev | verify-канарейки проходят |
| 8 | ⏳ OPEN | Прогон гейта `bash scripts/verify_M-38b.sh` → `VERDICT: PASS` | engine-dev | exit=0, Done Block сырым выводом |
| 9 | ✅ DONE | **D1 (rev3, MAJOR):** `RetentionPlan.offload_and_prune` заполняется правильно. Гейт покрытия идёт по КАНДИДАТАМ (а не всем сегментам — иначе активный попадал в `offload_only`). `offload_only` сегменты ТЕПЕРЬ обрабатываются в `retention_execute` (cold-copy + локальная копия остаётся). Override+None → prune разрешён (task #12). | engine-dev | `red_retention_checkpoint_coverage` 6/6 + `red_retention_operator` 7/7 GREEN |
| 10 | ✅ DONE | **D2 (rev3, MAJOR):** `advance`/`advance_to` резюмируются от валидного чекпоинта через `stream_from(cursor)`. Если валидного чекпоинта нет И `first_seg.first_seq > 0` — fail-loud без записи (первый видимый сегмент указывает на спруненный префикс). Перед записью — проверка немонотонности (`new_max < old_max → error`). | engine-dev | `red_checkpoint_prefix_pruned` 3/3 GREEN |
| 11 | ⚠️ DONE (частично) | **D3 (rev3):** откатил все 4 Result-ignoring lint'а (`unused_must_use`, `manual_is_multiple_of`, `manual_unwrap_or`) из workspace + `crates/{gateway,journal}/Cargo.toml`. КАНАРЕЙКА GREEN. Остался УЗКИЙ allow `doc_lazy_continuation` в `crates/gateway/Cargo.toml` (НЕ в канарейке, НЕ Result-ignoring, НЕ в проде — только в sacred RED-тесте `red_checkpoint_byte_identity.rs:21`). | engine-dev | канарейка verify GREEN; clippy с возвращёнными 3 Result-ignoring lint'ами GREEN; узкое исключение документировано |
| 12 | ✅ DONE | **Семантика override (rev3):** `allow_prune_without_checkpoint=true` + `covered=None` → prune РАЗРЕШЁН и поимённо назван в `pruned_without_checkpoint_coverage` (заполняется в `retention_execute`). Hatch существует для случая «чекпоинтер сломан/не развёрнут», когда артефакта покрытия и НЕТ. Дефолт (override=false) остаётся fail-closed. | engine-dev | `override_prunes_but_is_named_in_report` GREEN |
| 11a | ⚠️ SCOPE VIOLATION | **`doc_lazy_continuation` в sacred RED-тесте `crates/gateway/tests/red_checkpoint_byte_identity.rs:21`** — Rust 1.97+ lint на docstring. Архитектор не поправил docstring (только 4 места `i.is_multiple_of(2)` в коде). Тест sacred, править нельзя → оставлен УЗКИЙ allow в `crates/gateway/Cargo.toml` (только этот lint, не в канарейке). Когда архитектор починит docstring — allow удалить. | architect | (тест sacred → reporter, не правка) |

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
| **`crates/gateway/tests/red_checkpoint_prefix_pruned.rs`** (rev2, C-030 R3/N2) | ТРЕТИЙ форсинг: покрытый префикс ФИЗИЧЕСКИ удалён — скрытый полный реплей невозможен (истории нет). Плюс суффикс-совместимость lineage, цикл «advance→prune→advance→prune», и отсутствие непокрытого хвоста не досочиняется из чекпоинта |
| **`crates/journal/tests/red_retention_checkpoint_coverage.rs`** (rev2, C-030 R1) | Строгая связка: непокрытые сегменты не прунятся и названы в skip-репорте; покрытые — прунятся (парный vantage); граница `last_seq` vs `last_seq−1`; отсутствие артефакта ≠ «покрыто»; offload не блокируется; override назван в отчёте |

### Почему одной байт-идентичности НЕДОСТАТОЧНО (главное для critic'а)

**Реализация, которая ПОЛНОСТЬЮ ИГНОРИРУЕТ чекпоинт и всегда реплеит от START, проходит
ВСЕ тесты байт-идентичности и все тесты инвалидации.** Она же — самый вероятный «зелёный»
исход, если оракул односторонний (класс «идеальная фикстура», пойманный ЧЕТЫРЕ раза подряд:
M-07, M-08, TD-042, TD-045). Форсингов **три** (третий добавлен по C-030 R3), и все обязаны
быть в наборе:

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
3. **Удалённый покрытый префикс** (rev2, C-030 R3) — `red_checkpoint_prefix_pruned`.
   **Критик показал, что первых двух НЕДОСТАТОЧНО:** проходит реализация, которая грузит чекпоинт
   ровно настолько, чтобы возмутить выход в тесте (1), делает ПОЛНЫЙ реплей от START для
   корректности, и возвращает маленький `ReadStats`, собранный отдельным `stream_from` по хвосту.
   Дыра в том, что (1) и (2) наблюдают то, что реализация САМА о себе сообщает. Третий форсинг
   наблюдает физику: покрытых сегментов на диске НЕТ. Скрытый реплей не может вернуть правильные
   байты — истории не существует. Без wall-clock и аллокатора: только удалённые файлы и байтовое
   сравнение с эталоном, снятым ДО удаления.

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

### История ревизий

- **rev1** (`f3c8fbb`) → **critic C-030: REJECT**. Блокеры: (R1) связка с ретеншеном не
  специфицирована и не покрыта тестом; (R2) `red_stream_from` содержал ПЛАЦЕБО — тест обещал
  защиту legacy, но legacy-сегмента не строил; (R3) двух форсингов недостаточно против скрытого
  полного реплея с поддельно-маленьким `ReadStats`. NOTE: N1 (уточнить статус `selector`),
  N2 (lineage под pruning).
- **rev2** → **critic C-031: NOTE, engine-dev разблокирован.** R1/R2/R3 признаны закрытыми,
  N1/N2 достаточными; отклонение `allow_prune_without_checkpoint` ПРИНЯТО при условии
  `default=false` + явный операторский флаг + поимённый аудит каждого prune. Остаточный NOTE —
  канарейка на escape-hatch в verify (сделано architect'ом: verify проверяет, что прод-сервис
  ретеншена НЕ передаёт флаг, и что флаг объявлен в бинаре явно) + dev ОБЯЗАН сохранить
  fail-closed дефолт.
- **rev2** (этот документ) — все четыре пункта «Required revision» C-030 закрыты:
  (1) строгая связка + `red_retention_checkpoint_coverage`; (2) реальная legacy-фикстура
  (проверена фактически: сегменты `[(0, ver=1 legacy, first_seq=0), (1, v4, 200), (2, v4, 549)]`,
  600 событий); (3) третий форсинг `red_checkpoint_prefix_pruned`; (4) `selector` объявлен
  конфигурацией, а не состоянием. **Одно осознанное отклонение** — операторский
  `allow_prune_without_checkpoint` (см. §Связка с ретеншеном), вынесено критику явно.

- **plan-time critic — ОБЯЗАТЕЛЕН, ПОВТОРНО** (C-030: «Do not dispatch engine-dev yet»;
  `gates.md` §3: ≥5 коммитов + касание `crates/journal`).
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
