# M-08 — Data durability: сбор не останавливается, журнал читается на прод-масштабе

STATUS: 🚧 PROPOSED. Authored: architect (Opus), 2026-07-13.
Приоритет founder'а (2026-07-13): **(1) набор и сохранение данных не останавливаются
НИКОГДА; (2) инфраструктура для создания альф и торговли утверждёнными стратегиями
готова.** M-08 закрывает (1) и снимает блокер (2) на прод-объёмах. Торговый стек
(risk/killswitch/oms/runner) — следующий milestone M-09.

Гейты: **contract-RFC (`docs/rfc/CT-RFC-02-journal-provenance.md`) + critic ОБЯЗАТЕЛЕН**
(`gates.md` §1: трогаем `crates/contracts/**` (T1), ≥5 коммитов). `risk`/`killswitch`/
`oms`/`venue-*` НЕ трогаются → risk-critic не требуется. **Прод НЕ инертен** (меняется
`journal` + `recorder`) → §8 деплой-гейт с eyes-on обязателен и решающий (уроки TD-011/TD-013).
Ветка: `feat/M-08`.

## Objective

Три измеренных факта (VPS, 2026-07-13) делают текущее состояние тупиковым:

| Факт | Число | Следствие |
|---|---|---|
| Скорость записи | **2.8 GB/сутки** (8.3 GB за 3 суток) | — |
| Свободно на диске | 120 GB из 150 GB | **сбор встанет через ~43 дня** (плюс докупленная история — раньше) |
| Ротация/ретеншен | **нет** (`journal/src/lib.rs:24` — имя сегмента захардкожено), TD-006 | некуда расти |
| Чтение журнала | `journal::read_all() -> Vec<Event>`, `research-cli/src/main.rs:54` | **грид на 8.3 GB не запустится** — весь журнал в RAM (класс TD-011, этажом выше) |
| `schema_version` в сегменте | **не пишется** (`journal.meta` = только `next_seq`) | `CT-I-6` формально не выполнен; provenance некуда положить |
| Память recorder | дрейф 5–9 → **48 MiB** за ~5 ч (наблюдение reviewer'а) | лик не доказан, но healthcheck такое маскирует (TD-011) |

M-08 делает сбор **бесконечным по времени** (ротация + ретеншен + cold-выгрузка), чтение —
**bounded по памяти** (стрим вместо `Vec<Event>`), а происхождение данных — **читаемым
фактом** (CT-RFC-02), чтобы купленная история не смешалась с собственным захватом.

Авторитетные док-и: `docs/06-data-layer-and-storage.md` (retention/cold), `docs/fa/journal.md`
(`JR-I-*`, `DET-I-1`), `docs/05-contract-layer.md` §4/§6, `docs/rfc/CT-RFC-02-*`.

## Contract impact (T1) — ЕСТЬ

`crates/contracts/**` меняется → **atomic contract-RFC `CT-RFC-02`** (см. файл): `DataSource`,
`SegmentHeader`, `SCHEMA_VERSION` 1 → 2. `Event`/`EventKind` НЕ трогаются (аддитивно, старые
журналы читаются навсегда — `CT-I-3`). Reviewer Block-C: правки `contracts/` вне RFC → авто-REJECT.

## Архитектурные решения

| # | Вопрос | Решение M-08 |
|---|---|---|
| E1 | Где живёт provenance | В **заголовке сегмента**, не в `Event` (при 2.8 GB/сут тег в каждом событии — гигабайты мусора; писатель сегмента ровно один, `JR-I-1`). CT-RFC-02 §2 |
| E2 | Порог ротации | Сегмент закрывается по размеру (**1 GiB**, конфиг) ИЛИ при рестарте писателя. Имя `segment-NNNNNNNN.jrnl`, монотонный индекс; `seq` продолжается СКВОЗЬ сегменты (тотальный порядок — один на журнал, `JR-I-1`) |
| E3 | Ретеншен | Политика: горячие сегменты на диске (окно `retention_days`, дефолт 14), холодные — выгрузка в Storage Box + удаление локально **ТОЛЬКО после подтверждённой выгрузки** (checksum совпал). Удаление невыгруженного сегмента невозможно выразить в API (типовой барьер, не дисциплина) |
| E4 | **Fail-closed по диску** | Свободного места < `min_free_gb` (дефолт 10) → recorder **НЕ пишет молча дальше**: алерт + halt записи с явным событием `Sys`. Тихо переполнить диск и умереть — запрещено (это и есть «сбор остановился», только без предупреждения) |
| E5 | Чтение | `journal::stream(dir, EpochFilter) -> io::Result<(Vec<SegmentHeader>, impl Iterator<Item=io::Result<Event>>)>` — **итератор**, O(1) памяти на сегмент-буфер. `read_all` остаётся ТОЛЬКО для тестов/малых фикстур и помечается `#[deprecated]`-комментарием в docs; прод-путь research — стрим |
| E6 | Эпохи | Читатель обязан назвать `EpochFilter` (`OwnCaptureOnly` / `Explicit(vec![epoch_id])`). Дефолт `OwnCaptureOnly` — вендор/синтетика в обучение по умолчанию НЕ попадают (CT-RFC02-3/4) |
| E7 | Память recorder | RED-оракул bounded-RSS: длительный прогон writer'а (сотни тысяч событий) в counting-allocator бюджете; дрейф 5→48 MiB обязан ловиться тестом, а не глазами через 5 часов |
| E8 | Gap-статистика | Инструмент считает разрывы записи (по `Sys::ConnDown/ConnUp` + монотонности `ts_wall_ms` между соседними событиями) → `research/data-quality/gaps-<epoch>.json`. Любой отчёт (R-NNN) обязан ссылаться на него (иначе метрики считаются по дырявым данным) |
| E9 | Deploy-гейт | `deploy.yml` получает `needs: ci` — красный CI больше не может выкатить прод (находка reviewer'а 2026-07-13: Deploy зеленел, пока CI ещё шёл) |

## Allowed / Forbidden paths (scope-guard)

| Агент | Allowed | Forbidden |
|---|---|---|
| architect | `docs/rfc/`, `docs/`, `milestones/`, `crates/contracts/src/**` (ТОЛЬКО T1-формы CT-RFC-02), `crates/journal/src/**` (ТОЛЬКО сигнатуры/типы + `todo!()`), `crates/*/tests/**` (RED, sacred), `scripts/verify_M-08.sh`, `.github/workflows/deploy.yml` (E9 — process-only) | impl-тела, `crates/recorder/src/**`, `crates/research-cli/src/**` |
| engine-dev | `crates/journal/src/**`, `crates/recorder/src/**` + их `Cargo.toml` | `*/tests/**` (sacred), `crates/contracts/**` (T1 — только architect через RFC), `scripts/**`, `docs/**` |
| research-dev | `crates/research-cli/src/**` (перевод на стрим-чтение + `EpochFilter`), `research/data-quality/` (артефакты E8) | всё остальное; `crates/research-cli/tests/**` (sacred) |
| tester | read-only; `scripts/verify_M-08.sh` на чистом чекауте | правки кода |
| reviewer | `PROJECT-STATE.md`, `TECH-DEBT.md`, merge + **§8 деплой-гейт (решающий: прод НЕ инертен)** | код |

## §Tasks

| # | Status | Задача | Агент | Verify |
|---|---|---|---|---|
| 1 | ⏳ | **CT-RFC-02**: T1-формы (`DataSource`, `SegmentHeader`, `SCHEMA_VERSION`=2) + JSON Schema + фикстуры + RED (`CT-I-6`, `CT-RFC02-1..4`); скелеты `journal` (сигнатуры `stream`/`EpochFilter`/ротация, `todo!()`); `verify_M-08.sh` | architect | workspace компилируется; fmt+clippy зелёные; RED падает |
| 2 | ⏳ | `journal` impl: заголовок сегмента; **ротация** `segment-NNNNNNNN.jrnl` (E2, seq сквозной); `stream()` — bounded-memory итератор (E5); legacy-путь (сегмент без заголовка → вменённый `OwnCapture`, CT-RFC02-1); `EpochFilter` (E6) | engine-dev | `cargo test -p journal` GREEN, включая **прод-масштабный** bounded-memory оракул (≥64 MiB сегмент, counting-allocator) |
| 3 | ⏳ | `journal` retention: cold-выгрузка (Storage Box) + удаление ТОЛЬКО после подтверждённой выгрузки (E3); **fail-closed по диску** (E4: `min_free_gb` → halt + `Sys`-событие + алерт, не тихая смерть) | engine-dev | RED: удаление невыгруженного сегмента невозможно; диск < порога → запись останавливается ЯВНО |
| 4 | ⏳ | `recorder`: писать заголовок (provenance = версия recorder'а + git sha), переживать ротацию без потери событий; **bounded-RSS** (E7) | engine-dev | RED: длительный прогон в бюджете памяти; ротация посреди потока не теряет и не дублирует `seq` |
| 5 | ⏳ | `research-cli`: перевести чтение на `journal::stream` + `EpochFilter` (E5/E6); грид больше НЕ держит `Vec<Event>` в памяти; gap-статистика (E8) → `research/data-quality/` | research-dev | RED: грид отрабатывает на большом журнале в бюджете памяти; смешение эпох без явного фильтра невозможно |
| 6 | ⏳ | `.github/workflows/deploy.yml`: `needs: ci` (E9) | architect | красный CI не выкатывает прод (проверяется на PR-прогоне) |
| 7 | ⏳ | Прогон `scripts/verify_M-08.sh` на чистом чекауте | tester | `VERDICT: PASS`, exit=0 |
| 8 | ⏳ | Review + merge + **§8 деплой-гейт: прод НЕ инертен** — ssh-проверка, что recorder пишет в НОВЫЙ сегмент, старый 8.3 GB сегмент цел и читается, heartbeat свежий, RSS в норме | reviewer | Done Block + §8 пруф (сырой ssh-вывод) |

## RED-тесты (sacred, architect-only)

- `crates/contracts/tests/red_rfc02.rs` — `CT-I-6` (schema_version читается ИЗ ФАЙЛА),
  роундтрип `SegmentHeader`, аддитивность (старый `Event` парсится байт-в-байт).
- `crates/journal/tests/red_rotation.rs` — ротация: сегменты `NNNNNNNN` по порядку; `seq`
  сквозной и монотонный через границу сегментов; рестарт писателя не переиспользует `seq`;
  **события на границе не теряются и не дублируются**.
- `crates/journal/tests/red_stream_bounded.rs` — **прод-масштаб** (≥64 MiB, counting-allocator,
  бюджет ≤8 MiB): `stream()` отдаёт все события в бюджете; **анти-плацебо: наивная реализация
  через `read_all` падает по памяти** (прямой наследник `red_open_bounded.rs`, TD-011).
- `crates/journal/tests/red_legacy_segment.rs` — CT-RFC02-1: сегмент БЕЗ заголовка (формат
  прод-файла) читается как schema 1 + вменённый `OwnCapture`; ни одно событие не потеряно.
- `crates/journal/tests/red_epoch_filter.rs` — CT-RFC02-2/3/4: события нельзя получить, не
  назвав эпоху; `Vendor`/`Synthetic` не попадают в выборку по умолчанию.
- `crates/journal/tests/red_retention.rs` — E3/E4: (а) удалить невыгруженный сегмент
  невозможно (типовой барьер); (б) свободное место < порога → запись останавливается ЯВНО
  (`Sys`-событие + Err), а не «пишет, пока не умрёт».
- `crates/recorder/tests/red_rss_bounded.rs` — E7: длительный прогон writer-петли в бюджете
  памяти; **анти-плацебо: накапливающий буфер/лог падает**.
- `crates/research-cli/tests/red_stream_grid.rs` — E5/E8: грид на большом журнале в бюджете
  памяти; gap-статистика считает разрывы; смешение эпох без явного фильтра невозможно.

## Acceptance

`bash scripts/verify_M-08.sh; echo "exit=$?"` → `VERDICT: PASS`, exit=0
(fmt + clippy + все RED GREEN + прод-масштабные bounded-memory оракулы + грепы:
`read_all` не используется в `research-cli/src`, имя сегмента не захардкожено).

**§8 (решающий, прод НЕ инертен):** после merge — CI+Deploy success И ssh-проверка:
recorder пишет в НОВЫЙ сегмент с заголовком; **старый 8.3 GB сегмент цел** (не переписан,
не удалён) и читается legacy-путём; `seq` продолжился без дыр; heartbeat свежий; RSS в норме.
Любое сомнение → revert (данные дороже фичи).

## Handoff

architect (RFC + RED + скелеты) → **critic** (T1-триггер) → engine-dev (2,3,4) →
research-dev (5) → tester (7) → reviewer (8, merge + §8).
