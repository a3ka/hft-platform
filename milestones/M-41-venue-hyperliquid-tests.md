# M-41 — venue-hyperliquid RED-суита (R4, ШАГ 1a)

**Статус:** IN_PROGRESS (RED-набор закоммичен; ждёт venue-dev).
**Риск:** R4 HIGH (`docs/08`) — 217 строк прод-парсера с нулём тестов пишут в append-only журнал.
**Гейты:** reviewer UNCONDITIONAL. **MD-only carve-out подтверждён** (gates.md §5): в src нет
order-egress (submit/cancel/подписи) — risk-critic НЕ требуется; carve-out охраняется канарейкой
`md_only_no_order_egress_canary` (тест ломается при появлении торгового пути → RISK-BLOCK).
critic: триггер §1.2 (venue-*) закрыт carve-out'ом; коммитов < 5; новых крейтов нет.

## Objective

Закрыть R4/R9 («заявленная защита, которой нет»: VN-I-7 заявлен в `docs/fa/venues.md`, оракулов
0) RED-суитой паритетной venue-binance. HL — первая венью (DESIGN §0) и вероятный первый источник
SaaS-продукта (ончейн-данные, юридически чистые для перепродажи) — парсер обязан быть покрыт.

## Найденные дефекты (архитектор НЕ чинит — зона venue-dev)

### D0 — КРИТИЧНО: стороны трейдов ИНВЕРТИРОВАНЫ
`parse_trade` маппит `"A" => Side::Buy, "B" => Side::Sell`. Официальная нотация Hyperliquid
(hyperliquid.gitbook.io → For developers → API → Notation, сверено WebFetch 2026-07-29):
> «Side = side of trade or book. **B = Bid = Buy, A = Ask = Short.**
> Side is aggressing side for trades.»

Т.е. правильно: **"B" → Side::Buy, "A" → Side::Sell**. Комментарий в src («"A" = aggressive
buy») — неверен, править вместе с кодом. Воспроизведение: `red_parse_trades.rs::
trade_side_b_is_buy_official_notation` / `trade_side_a_is_sell_official_notation` (падают).
**Следствие для прод-данных — см. §E Handoff (эпоха данных).**

### D1 — l2Book без `time` фабрикует `ts_exch_ms = 0`
`parse_l2book`: `data.get("time")...unwrap_or(0)` — нарушение VN-I-7 (не фабрикуй значение) +
отравление возрастного фильтра ретеншена (событие с ts=0 «старше всех»). Ожидание: дроп всего
сообщения. Воспроизведение: `red_parse_l2book.rs::missing_time_drops_message_not_fabricates_zero`.

### D2 — NaN/inf проходят парсинг и превращаются в нули/сатурацию
`"NaN".parse::<f64>()` в Rust успешен; `to_fixed(NaN) = 0` (saturating cast), `to_fixed(inf) =
i64::MAX`. В журнал уходит «событие с нулями» — буквально сценарий из мандата. Воспроизведение:
`red_fail_closed_values.rs` (nan_price_dropped_not_zero и др.).

### D3 — отрицательные/нулевые px/sz и неположительный time принимаются
Ожидание (спека fail-closed значений): **px, sz — конечные и строго > 0; time > 0**.
Гранулярность: trades — дроп ЭЛЕМЕНТА (трейды независимы, валидный сосед живёт);
l2Book — дроп ВСЕГО сообщения (целостность снапшота под сомнением).
Воспроизведение: `red_fail_closed_values.rs` (9 падающих оракулов D2+D3).

## Спецификация публичного API (task #2)

`pub fn parse_message(text: &str) -> Vec<EventKind>` — единственный экспорт парсинга (паттерн
M-18 `venue_binance::l2delta_event`). Внутренние `parse_trade`/`parse_l2book`/
`parse_level_objects` остаются приватными (VN-I-5: wire-типы не покидают адаптер; наружу только
contracts-типы). Сейчас суита compile-RED именно из-за приватности.

## Задачи

| # | Задача | Владелец | Статус |
|---|---|---|---|
| 1 | RED-суита 5 файлов (40 оракулов: 12 подлинных RED D0–D3, 28 характеризационных с мутационным контролем ×7) + verify_M-41.sh | architect | ✅ DONE |
| 2 | Экспорт `pub fn parse_message` (compile-GREEN), семантику не менять | venue-dev | ✅ DONE |
| 3 | D0: фикс инверсии сторон ("B"→Buy, "A"→Sell) + фикс лживого доккомментария | venue-dev | ✅ DONE |
| 4 | D1: l2Book без `time` → дроп (убрать `unwrap_or(0)`) | venue-dev | ✅ DONE |
| 5 | D2+D3: валидация значений (finite, px>0, sz>0, time>0; гранулярность по спеке выше) | venue-dev | ✅ DONE |
| 6 | Чистый прогон: `cargo test -p venue-hyperliquid` + `bash scripts/verify_M-41.sh` + Done Block | tester | ⏳ OPEN |

## Allowed paths
- venue-dev: `crates/venue-hyperliquid/src/lib.rs` ТОЛЬКО (+ Status-колонка §Tasks здесь).
- Тесты `crates/venue-hyperliquid/tests/**` и `scripts/verify_M-41.sh` — sacred (architect).

## Acceptance
`bash scripts/verify_M-41.sh` → `VERDICT: PASS`, exit=0 (включает: полный тест-прогон крейта,
clippy `-D warnings`, fmt-check, структурные канарейки суиты, MD-only grep).

## Мутационный контроль (проведён architect'ом, прототип откачен, src в коммите нетронут)
M1 снятие MID-фильтра · M2 swap bids/asks · M3 тихий пропуск битого уровня · M4 дефолт
стороны · M5 ts:=0 · M6 фабрикация события на битом JSON · M7 take(1) на трейдах —
все 7 мутаций валят соответствующие оракулы (8/8 целевых тестов FAILED).

## Cross-ref
docs/08 R4 · docs/fa/venues.md §I (VN-I-5, VN-I-7) · .claude/rules/testing.md (чек-лист
деградированного входа) · .claude/rules/gates.md §5 (MD-only carve-out).
