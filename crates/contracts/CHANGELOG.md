# contracts (T1) — CHANGELOG

Формат: одна секция на contract-RFC. Журнал бессмертен: старые записи обязаны читаться
новым кодом всегда (CT-I-3). Правки T1 вне RFC → авто-REJECT (CT-I-2, Block-C).

## schema_version 2 — CT-RFC-03 «Аудит сверки с биржей» (2026-07-16)

Аддитивно (версия НЕ меняется — как CT-RFC-01, это вариант `EventKind`, не формат сегмента):
- `SysEvent::ReconDivergence(ReconAudit)` — durable-след recon-расхождения (OPS-I-1, M-09):
  `{venue, symbol, divergence_bps, best_price_diverged, action}`; **строго в конце** enum
  (postcard-дискриминант 3; Heartbeat/ConnUp/ConnDown = 0/1/2 неизменны).
- `ReconAction { AlertOnly, Resynced }` (0/1).

**Мотив.** Recon (сверка локальной книги с REST-снапшотом биржи) — единственная проверка
ПРАВИЛЬНОСТИ данных (эвикция C1 стирала best bid при зелёном healthcheck). Расхождение — факт
о ДАННЫХ, обязан жить в том же журнале, что данные (не в логе/метрике), иначе нельзя ответить
«каким сегментам верить». Метрики в журнал не пишутся (OPS-I-6) — поэтому нужен доменный вариант.

**НЕ изменено:** `Event`, `EventKind`, `MdEvent`, `MdPayload`, `Venue`, `Side`, `Level`,
`SegmentHeader`, `SysEvent::{Heartbeat,ConnUp,ConnDown}` — wire-формат прежний, старые сегменты
читаются байт-в-байт (CT-I-3, RED `red_rfc03.rs`). `schema_version` остаётся 2.

**Схема:** `event.schema.json` перегенерирована (`gen_schema`), гейт `red_schema.rs` (CT-I-4).
Фикстуры: `valid/event-recon.json`, `invalid/event-recon-unknown-action.json`.

## schema_version 2 — CT-RFC-02 «Provenance и эпохи журнала» (2026-07-13)

**Мотив.** Founder докупает историю. Купленные данные обязаны входить через тот же журнал,
но быть ОТЛИЧИМЫ от собственного захвата: у вендора другая глубина книги, другие часы,
другие гэпы. Обучить альфу на смеси без пометки = обучить на реальности, которой у нас не
было. Задним числом источник не проставить — форма вводится ДО первого чужого байта.

**Добавлено (аддитивно):**
- `DataSource` = `OwnCapture | Vendor | Synthetic` (расширение — строго в конец;
  дискриминанты postcard зафиксированы RED-тестом).
- `SegmentHeader { schema_version, source, provenance, epoch_id, created_wall_ms, first_seq }`
  — первый фрейм каждого сегмента (закрывает CT-I-6, который фактически не выполнялся:
  `journal.meta` нёс только `next_seq`).
- `SEGMENT_MAGIC = b"HFTJRN02"` — префикс сегментов schema ≥ 2.
- `LegacySegmentDecl` / `LegacyManifest` (`journal.legacy.json`) — ЯВНАЯ декларация
  происхождения сегментов старого формата + отпечаток (sha256 первого MiB) и размер.

**НЕ изменено:** `Event`, `EventKind`, `MdEvent`, `MdPayload`, `Venue`, `Side`, `Level` —
wire-формат событий прежний, старые сегменты читаются байт-в-байт (CT-I-3).

**Миграция (rev 2, после находки critic C-005 C2 — прежнее правило было FAIL-OPEN):**
- Сегмент с магией → заголовок ОБЯЗАН разобраться; не разобрался → `Err` (не «наш»).
- Сегмент без магии → legacy ТОЛЬКО если задекларирован в `journal.legacy.json` и отпечаток
  совпал. Иначе → `Err` («чужой/неизвестный сегмент»). Никакого вменения по умолчанию.
- Боевой сегмент (`segment-00000000.jrnl`, 8.3 GB, VPS) декларируется один раз, до деплоя;
  переписывать его запрещено (единственная копия).

**Схема:** `crates/contracts/schema/*.schema.json` — СГЕНЕРИРОВАНА
(`cargo run -p contracts --example gen_schema`); гейт `tests/red_schema.rs` падает при
расхождении с типами (CT-I-4). Фикстуры: `crates/contracts/fixtures/{valid,invalid}`.

## schema_version 1 — CT-RFC-01 (2026-07-11)

Аддитивно: `MdPayload::{OpenInterest, Liquidation, MarginRate}`, `Venue::BinanceFutures`.
