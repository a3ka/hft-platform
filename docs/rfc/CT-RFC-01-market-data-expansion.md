# CT-RFC-01 — Market-data expansion (futures depth, OI, liquidations, borrow-proxy)

STATUS: PROPOSED (ждёт critic-гейта; T1-change НЕ применён до PASS).
Автор: architect (Fable), 2026-07-11. Governance: `docs/05-contract-layer.md` §4 (CT-I-2),
`.claude/rules/gates.md` §1.1 (contracts → всегда contract-RFC + critic).
Founder-решения зафиксированы 2026-07-11 (ниже §Decisions).

## Мотивация
Расширяем сбор рыночных данных под квант-стратегии (запрос founder'а): глубина
spot+futures, breadth фандинга по топ-монетам, open interest, ликвидации, прокси
спроса на займы. Принцип journal-first: храним СЫРЬЁ (per-symbol), производные
(funding-breadth, CVD, базис) считаем детерминированно downstream — НЕ в T1.

## Decisions (founder, 2026-07-11)
1. **Tier-1 набор:** Binance futures глубина + Funding-breadth top-300 + Open Interest
   + Ликвидации. Займы USDT/USDC → Tier-3.
2. **Spot/futures — через Venue-варианты** (аддитивно), не поле `market`.
3. **Займы:** старт с публичного прокси = margin interest rate (Tier-3 impl отложен);
   3rd-party/signed-агрегат — позже (нужны ключи, secret-mount на VPS, НЕ в чат).

## T1-delta (аддитивно, СТРОГО в конец enum'ов — CT-I §6)

### Venue (`contracts/src/lib.rs:46`)
```
pub enum Venue {
    Binance,          // = SPOT (семантика сохраняется, НЕ переименовываем)
    Hyperliquid,      // = PERP (HL основной рынок; уже так)
    BinanceFutures,   // NEW — USDT-M perp (fstream)
}
```
**Отклонение от буквального решения 2 (было «BinanceFutures, HyperliquidPerp»):**
`Hyperliquid` УЖЕ обозначает perp-венью (адаптер шлёт l2Book/trades перпа) — добавлять
`HyperliquidPerp` = дубликат/переименование (ломает старые фикстуры, нарушает
additive-no-rename). Поэтому: добавляем только `BinanceFutures`; `HyperliquidSpot`
резервируем на потом при нужде. Семантика Binance=spot / Hyperliquid=perp фиксируется
здесь как контрактное соглашение (документируется в doc-комментарии Venue).

### MdPayload (`contracts/src/lib.rs:78`) — 3 новых варианта в конец
```
    OpenInterest {           // NEW
        oi_e8: i64,          // OI в БАЗОВОМ активе ×1e8 (нотионал — derive: oi×mark)
        ts_exch_ms: i64,
    },
    Liquidation {            // NEW  (forced order)
        price: i64,          // ×1e8
        size: i64,           // ×1e8
        side: Side,          // ликвидируемая сторона
        ts_exch_ms: i64,
    },
    MarginRate {             // NEW  (Tier-3 impl; прокси спроса на займы)
        rate_e8: i64,        // ставка ×1e8 (интервал — по venue-соглашению; в provenance)
        ts_exch_ms: i64,
    },
```
- `MdEvent.symbol` переиспользуется как идентификатор: для `MarginRate` — актив
  ("USDT"/"USDC"); для `OpenInterest`/`Liquidation` — инструмент ("BTCUSDT").
- **Funding-breadth — НЕ тип.** Считается downstream из потока per-symbol `Funding`
  (уже T1) по top-N (ранжирование по OI/volume — тоже downstream).
- Будущий фактический объём займов (если добудем через 3rd-party) — отдельный
  аддитивный вариант `MarginBorrowed { amount_e8, .. }` новым CT-RFC, не сейчас.

### SCHEMA_VERSION (`contracts/src/lib.rs:14`)
`0 → 1`. Единственное чтение — лог-строка `recorder/src/main.rs:58` (безопасно).
Аддитивность гарантирует: старые сегменты (variant-индексы 0..2) декодируются
неизменно; новые индексы (3..5 для MdPayload, 2 для Venue) — только новым кодом.

## Кодировки / совместимость (CT-I-1..6)
- postcard кодирует вариант enum как varint-индекс. Добавление В КОНЕЦ не сдвигает
  индексы Trade/L2Snapshot/Funding и Binance/Hyperliquid → **старые журналы читаются
  без изменений** (roundtrip-канарейка обязательна).
- Все размеры/ставки — fixed-point i64 ×1e8 (JR-I-7; никаких f64 в деньгах).
- `EventKind`/`MdPayload` остаются определёнными РОВНО в одном крейте (`contracts`) —
  grep-канарейка CT-I.

## Обязательные тесты при применении (architect, sacred */tests/)
- **CT-RFC-01-T1** old-fixture roundtrip: заранее сериализованный (pre-change) сегмент
  с Trade/L2Snapshot/Funding декодируется идентично после добавления вариантов.
- **CT-RFC-01-T2** new-variant roundtrip: OpenInterest/Liquidation/MarginRate +
  Venue::BinanceFutures → serde+postcard roundtrip бит-идентичен.
- **CT-RFC-01-T3** single-definition канарейка: `enum MdPayload`/`enum Venue`
  определены только в `crates/contracts`.

## Blast radius (exhaustive match'и — dev обновляет в M-06, НЕ architect)
Добавление вариантов ЛОМАЕТ исчерпывающие `match` без wildcard. Сайты (проверить каждый;
часть уже с `_ =>`):
- `MdPayload`: venue-binance, venue-hyperliquid, sim/src/exchange.rs, book/src/lib.rs,
  signals/src/obi.rs, research-cli/bin/latency_probe.rs, + examples (dump/bands/obi_probe).
- `Venue`: те же + recorder/src/main.rs.
**Правило:** md-консюмеры, которые ДОЛЖНЫ реагировать (recorder-роутинг, book) —
явные arm'ы; сигналы, которым нужен только L2 (obi) — явный ignore-arm (НЕ молчаливый
wildcard, чтобы будущие типы не терялись незаметно). Тесты (*/tests/) — architect
обновляет вместе с CT-фикстурами.

## Impl-маппинг
- Типы + CT-фикстуры лендятся ПОСЛЕ critic PASS (architect).
- Потребители: **M-06** (venue-binance-futures глубина + funding/OI/liquidations +
  recorder-poller; funding-breadth derive). MarginRate impl — Tier-3 (позже, ключи).

## Governance
Аддитивно-только (CT-I §6). critic обязателен ДО применения. Founder-решения записаны.
