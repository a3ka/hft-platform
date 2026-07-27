# M-45 — persist сырых L2Delta (fidelity-апгрейд order-flow)

**Статус:** PLANNED (стаб; RED-first спека при взятии в работу). **Founder-подпись:** 2026-07-27.
**Порядок:** ПОСЛЕ R1 (Storage Box + retention-apply, docs/08 ШАГ 0) — больше данных усиливает риск
единственной копии; после prune runway не ограничен. **Предшествует** фазе 2 индикаторов (M-46).

## Контекст: механизм УЖЕ есть (M-18), проблема — ОХВАТ
**M-18 (CLOSED 2026-07-21)** уже реализовал сырой L2Delta-захват: `venue-binance` и `venue-binance-futures`
эмитят `MdPayload::L2Delta` (discriminant 6) из сырого `@depth` diff, recorder пишет (recorder/lib.rs:77).
L2Delta НЕ заменяет `L2Snapshot` (тот — recon-якорь, дельта — тонкая эволюция между якорями). **НО эмиссия
включена по allow-list ТОЛЬКО для самого ликвидного инструмента (BTC)** — «объём под контролем» (M-18 §Objective).
Значит для **BTC полная fidelity доступна УЖЕ** (сырые диффы пишутся с 2026-07-21); для остальных символов —
только 1 Гц бакетированный снапшот (быстрый spoofing невидим, liquidity груб).

## Objective
Расширить L2Delta-эмиссию с **BTC-only на нужные order-flow символы** (allow-list), раз замер показал, что
это дёшево. **Замер (2026-07-27, живой Binance WS):** сырой diff = ~14 уровней, ~7 Гц → **≈1.5 KB/с на
ликвидный символ** (BTC/ETH), ~0.1 KB/с средний — в 5-10× дешевле снапшота. Диск: +~0.1-0.3 GB/сут (мелочь;
runway почти не меняется — docs/08). Forward-only (прошлые суб-секундные данные не восстановить).

## Что делаем
1. **Расширить allow-list** L2Delta-эмиссии (`venue-binance` строка ~480, `venue-binance-futures` ~455) с
   BTC-only на символы кокпита. Механизм эмиссии/парсинга/recorder — УЖЕ есть (M-18), CT-RFC НЕ нужен.
   Trade/funding/order-path не трогаем.
2. **Опционально (дизайн-решение):** тюнинг частоты якорь-`L2Snapshot` (сейчас 1 Гц) для символов с L2Delta —
   якорь реже (10-30с) достаточно для resync, между якорями несут диффы → журнал может СЖАТЬСЯ при росте
   fidelity. Согласовать с M-29 apply_delta (gateway consumer уже есть) и resync-семантикой.
3. `venue-hyperliquid` — L2Delta НЕ реализован (HL шлёт агрегированный l2Book-снапшот, не diff) — отдельная
   оценка: даёт ли HL сырой diff вообще (объектный формат). Не блокирует Binance-путь.

## Allowed paths
- `crates/venue-binance/{src,tests}/` (venue-dev по architect-RED) · `crates/recorder/src/` (если нужна маршрутизация L2Delta) · verify · этот файл. НЕ trade/funding/order-path.

## RED (architect)
- venue эмитит L2Delta на diff (детерминизм: тот же diff → тот же Event); book, восстановленная из
  L2Delta, ≡ book из снапшота (byte-identity реконструкции — VB-I-1/2); malformed diff → fail-closed;
  провенанс глубже 1.3% сохранён (VB-I-5). Прод-масштаб + композиция стадий (diff→journal→gateway apply).

## Гейты: reviewer (MD-only carve-out — нет order-egress → risk-critic НЕ нужен; reviewer подтверждает MD-only). critic если ≥5 коммитов / затрагивает recorder-маршрутизацию.
## §8: изменение объёма/формата записи → деплой-гейт + eyes-on (журнал растёт, книга реконструируется корректно).
## Cross-ref: docs/07 (order-flow индикаторы), docs/08 ШАГ 0 (R1), M-46 (индикаторы), замер в истории 2026-07-27.
