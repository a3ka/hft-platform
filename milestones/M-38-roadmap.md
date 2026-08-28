# M-38 Roadmap — латентность read-пути (TD-044): чекпоинт + live-seek + shared-tailer

**Долговечный план.** Читается ЛЮБОЙ свежей сессией архитектора как источник истины по декомпозиции
и дизайну. Основан на fable-архитектурном разборе (2026-07-27) + founder-решениях. HEAD на момент
создания: origin/main @ b62f294 (M-37 смержен, §8 GREEN).

## Проблема (TD-044)
M-37 ограничил ПАМЯТЬ snapshot окном (RssAnon плато 26 MB / 21 GiB). Но ВРЕМЯ = O(история):
- `gateway::snapshot` реплеит журнал от `Cursor::START` на КАЖДОЕ подключение → прод-замер **409 с**,
  журнал растёт ~2.8 GB/сут (через месяц ~12 мин); N клиентов = N реплеев.
- **`frames_since` (live-push каждые 250 мс) — ТОЖЕ O(история):** на каждый тик досеивает VWAP-суммы
  реплеем всего журнала (~400 с), потом сворачивает хвост ≤256 событий → **live-push математически не
  сходится** (за «тик» ~7 мин recorder пишет несопоставимо больше). Кокпит непригоден.

## Founder-решения (2026-07-27, подписаны)
1. **CVD → session-anchored** (сброс 00:00 UTC, per-session ledger зеркально VP). Поправка VB-I-6
   (anchor-policy) + меняет отдаваемые значения → bump `GATEWAY_SCHEMA_VERSION` 6→7.
2. **Три milestone'а последовательно:** M-38a → M-38b → M-39 (каждый со своим critic+reviewer).

## Последовательность (порядок ОБЯЗАТЕЛЕН)

### M-38a — CVD session-ledger (TD-043) [ПЕРВЫЙ, фиксирует схему]
Фиксирует модель сессии CVD ДО чекпоинта (иначе схема чекпоинта замрёт на скалярной базе → выброшенные
чекпоинты при миграции).
- **Состояние:** `cvd: BTreeMap<session_id, CvdSession{base i64, bucket_delta BTreeMap<i64,i64>}>`.
- **Эвикция:** бакеты внутри текущей сессии → base ЭТОЙ сессии; целиком прошедшая сессия (критерий
  `max_time_s < lo`, ТОТ ЖЕ что VP) → удаляется. ОДНА структура session-max-времён на CVD и VP
  (убрать дублирование `vp_session_max_time_s`).
- **Форма:** `SeriesBundle.cumulative_delta` reset на границе сессии; `cvd_session_base` → per-session
  (`Vec<(session_id,base)>`). `merge_cvd_running`/`evict_series_bundle_under_window` переписываются
  per-session (скалярная арифметика баз — ИСТОЧНИК TD-042 — станет локальной для сессии, проще).
- **RED (sacred):** окно через 00:00 UTC (2 ledger-элемента живы); сделки строго по ОДНУ сторону
  границы (асимметрия); множественные сделки у границы; курсор у границы (overlap как TD-042);
  обновить `red_gateway_window::{cvd_base_survives_window_eviction, windowed_live_eq_replay_overlap_multistep}`
  под session-семантику. Анти-плацебо: падает на текущем single-running CVD (проверить фактически).
- **Doc:** поправка VB-I-6 в `docs/fa/viz-backend.md` (CVD session-anchored, founder-подпись 2026-07-27, v7).
- **Гейты:** critic ОБЯЗАТЕЛЕН (gateway-reducer + смена семантики схемы); risk-critic не нужен (read-path).

### M-38b — checkpoint-reducer + live-seek [ГЛАВНЫЙ, делает кокпит пригодным]
Две половины (ОБЕ обязательны — без live-seek чекпоинт даёт красивый первый кадр и мёртвый live):
- **(1) Чекпоинт = полное сериализованное состояние `Reducer`** (НЕ Snapshot — в нём нет VWAP
  `sum_pv/sum_v`, `book::OrderBook`, `HeatmapBucketState.mid` path-dependent кэша, `vp_session_max_time_s`,
  `at_ms`). Формат postcard+CRC-фрейм, ~1-2 MB/селектор. `#[derive(Serialize,Deserialize)]` на всей
  структуре Reducer (компилятор заставит покрыть новые поля).
- **Писатель:** отдельный бинарь `gateway-checkpoint` (библиотечная `checkpoint::advance(journal_dir,
  ckpt_dir, sel, filter)` в `crates/gateway`, atomic tmp+rename), вызывается в СУЩЕСТВУЮЩЕМ ops-cron
  (тот же операторский путь, что journal-retention TD-020). НЕ recorder (JR-I-1), НЕ компактор (инверсия
  слоёв: journal не знает про селекторы/VWAP), НЕ gateway-serve (монтирует journal-volume READ-ONLY).
  Каденс 5-15 мин → хвост ≤30 MB → первый snapshot ~1-2 с. Отдельный `GATEWAY_CHECKPOINT_DIR`.
- **Инвалидация (кэш, не истина — GW-I-9 ↔ journal JR-I-4):** mismatch magic / `ckpt_schema_version` /
  `gateway_schema_version` / `selector_fingerprint` (bands по `f64::to_bits`, НЕ Display) /
  `epoch_filter_fingerprint` / `journal_lineage` (sha по ЗАГОЛОВКАМ сегментов (index,epoch_id,source,
  first_seq) — компакция .zst не меняет, вендор/purge/эпохи меняют) / CRC / `cursor>at` → тихий rebuild
  от START, НИКОГДА не ошибка/расхождение.
- **(2) live-seek:** `journal::stream_from(dir, filter, after_seq)` (сегментный пропуск по `first_seq`
  из v2-заголовков; legacy first_seq=0 НЕ пропускается) + **резюмируемый живой `Reducer` в соединении**
  (докармливается только новыми событиями через `stream_from(cursor)`, дельта из живого состояния — НЕ
  повторный скан). Попутно чинит 4-ю потенциальную «идеальную фикстуру» (E.5: Frame от неполной книги).
- **Инвариант GW-I-9:** `snapshot_from_checkpoint(K,at) ≡ snapshot(START,at)` БАЙТ-ИДЕНТИЧНО (postcard).
- **DET-I-1 риски + закрытие:** полнота (derive + канарейка «новое поле без bump → падение»);
  `HeatmapBucketState.mid` обязателен в чекпоинте (оракул: после K нет двусторонних обновлений книги);
  `book::OrderBook` — канонический порядок экспорта (по возрастанию цены, НЕ `levels()` который bid-desc);
  `f64` фингерпринт по `to_bits`, NaN в bands запретить в `serve_config_from_env`; `i128` (VwapAcc,
  vp_bins) — проверить postcard-поддержку на RED-bootstrap (fallback пара `(hi i64, lo u64)`);
  atomic tmp+rename + flock на ckpt-dir.
- **RED (sacred, детерминированные — урок TD-040):**
  1. `red_checkpoint_byte_identity` — K на ДЕГРАДИРОВАННЫХ позициях (середина бакета; между L2Snapshot
     и L2Delta; после K нет двусторонних book-обновлений; K перед 00:00 UTC / at после; окно активно,
     эвикции до и после K; 2+ сделки по обе стороны K; K=0/K=at/K=последний seq сегмента/переход raw↔.zst).
     Tamper-анти-плацебо: валидный по CRC чекпоинт с изменённым `vwap_sum_pv` ОБЯЗАН изменить выход
     (доказывает, что чекпоинт используется, а не игнорируется тихим rebuild).
  2. `red_checkpoint_resource_bound` — ДЕТЕРМИНИРОВАННЫЙ счётчик декодированных событий/байт (инжектируемый
     источник, НЕ аллокатор/wall-time — testing.md «гейт мерит инвариант, не окружение»): snapshot_from_ckpt
     при K у хвоста декодирует ≤ N_tail×k. Текущий код декодирует всё → RED. Прод-масштаб (десятки MiB, .zst).
  3. `red_checkpoint_is_cache` — битый/чужой/cursor>at/нет файла → выход ≡ rebuild-от-START, без ошибки;
     идемпотентность (два advance без новых событий → тот же файл побайтно).
  4. `red_frames_seek_bound` — frames_since/резюм API у хвоста декодирует ≤ хвостовых сегментов + кадры
     байт-идентичны текущему frames_since (GW-I-8 контигуальность курсоров).
  5. Roundtrip-фикстура версии чекпоинта (JR-I-6-аналог): байтовая ckpt-v-фикстура в репо.
  6. `verify_M-38b.sh` + grep-канарейки (gateway-serve не пишет в journal-dir; gateway-checkpoint не
     импортирует journal-writer API — расширение VB-I-3).
- **Retention-связка (docs/06 §4):** каденс чекпоинта ≪ retain_days; prune требует покрытия чекпоинтом
  cursor ≥ последнего seq сегмента ИЛИ явного skip-репорта (мягкая связка, cold-копия — законный источник;
  строгость решить на plan-time critic M-38b). `journal_lineage` при prune — по оставшимся сегментам.
- **Гейты:** critic ОБЯЗАТЕЛЕН (≥5 коммитов + касание crates/journal `stream_from`); risk-critic не нужен.

### M-39 — shared-tailer [N клиентов = 1 реплей]
Сейчас каждое соединение независимо гоняет журнал (`run_authorized_session`). Правильно: ОДИН shared
`ReducerState` на процесс gateway-serve на (selector,filter), докармливается одним тейлером; соединения:
снапшот — `finish()` на клоне общего состояния, кадры — через `tokio::sync::broadcast`. Детерминизм цел
(один редьюсер, VB-I-2), стоимость O(1) по клиентам. Легальная форма «ленивого чекпоинта в память».

## Сквозные (учесть в соответствующих milestone'ах)
- **E.4 — норма в testing.md:** ресурсные sacred-оракулы меряют ДЕТЕРМИНИРОВАННЫЕ счётчики (события/байты
  через инжектируемый источник), не аллокатор/время. Закрепить при спеке M-38b (там resource-bound центральный).
- **Golden wire-фикстуры** Snapshot/Frame per `GATEWAY_SCHEMA_VERSION` (v6, v7 после M-38a) — дёшево, ловит
  случайный breaking change когда фронт станет внешним консюмером.
- **Мульти-инструмент** (позже): cron-чекпоинтер и serve берут список селекторов из ОДНОГО конфига (иначе
  разойдутся — класс TD-020). ckpt-ключ по `selector_fingerprint` уже готов к множеству.

## Не-M-38 хвосты (founder/reviewer зоны)
TD-041 (санкция на audit-тег pre-rewrite — refs единолично не пишу); ретеншен TD-020/TD-006 (диск тикает);
worktree'ы в /tmp (часть — аудит TD-041, трогать нельзя).

## Cross-references
- Fable-архитектурный разбор 2026-07-27 (3 сообщения — дизайн-референс, у founder'а).
- `docs/fa/viz-backend.md` (VB-I-*), `docs/fa/journal.md` §7 (JR-I-4 snapshot-как-кэш), `docs/06` §4
  (ретеншен), `crates/gateway/src/lib.rs` (Reducer/snapshot/frames_since/evict/merge_cvd_running),
  `crates/journal/src/segments.rs` (stream/first_seq/compaction), `.claude/rules/testing.md` (чек-лист + п.7).
