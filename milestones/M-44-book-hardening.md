# M-44 — book/venue hardening: futures-эвикция + venue-common + латентный долг (R6, ШАГ 3)

**Статус:** PLANNED (стаб). **Риск:** R6 HIGH + Тир C (`docs/08`). Один заход по латентному долгу.

## Objective
- **R6:** TD-016 фикс (эвикция distance-window + backstop-кап) НЕ портирован из venue-binance (spot) в
  `venue-binance-futures/src/lib.rs:229` — внутренняя книга фьючерсов на непрерывном diff растёт unbounded
  (тот же TD-016/021 этажом выше). `bucket_levels` скопирован 1:1, общего модуля нет → структурный copypaste-риск.
- Латентный долг того же слоя: TD-029 (recorder startup schema-guard), TD-030 (reader first_seq monotonic —
  ОСТОРОЖНО: legacy first_seq=0 споткнёт наивный guard), TD-032/033 (provenance-константа; SCHEMA_VERSION без машинного энфорса).

## Allowed paths
- `crates/venue-binance-futures/{src,tests}/` · новый `crates/venue-common/` (общий книжный код) · `crates/journal/{src,tests}/` (TD-029/030/032/033) · verify.

## Задачи (RED-first)
1. (architect RED) `td016_futures_book_saturates_at_backstop` (аналог spot). Анти-плацебо: падает на текущем unbounded.
2. (venue-dev) портировать эвикцию+backstop; **вынести общий книжный код в `venue-common`** (чтобы будущий фикс не помнить дважды).
3. TD-029/030/032/033 — по отдельным RED (TD-030 с legacy-first_seq=0 фикстурой).

## Гейты: critic (новый крейт venue-common + ≥5 коммитов) · reviewer. risk-critic не нужен (MD-only).
## Cross-ref: docs/08 R6 + Тир C, TD-016/021/029/030/032/033.
