# M-34 — Funding-breadth: сбор фандинга по ВСЕМ перпам (founder-приоритет)

STATUS: **PROPOSED** (2026-07-25, architect). Founder-решение (2026-07-25): «фандинг собирать по всему».
Снимает symbol-фильтр с уже-работающего all-perps `premiumIndex`-поллера → вселенная фандинга идёт в
журнал (вход для TPP Funding-4-групп breadth-метрики + per-coin линии). **MD-only** (read-only premiumIndex
funding, БЕЗ order-пути) → **reviewer, risk-critic НЕ требуется** (gates §5 MD-only carve-out). Reviewer в
Block-scope подтверждает, что диф не трогает order-egress.

## Мотивация (что уже есть, что меняем)

`venue-binance-futures` **УЖЕ** качает ВСЕ ~400 перпов одним вызовом `/fapi/v1/premiumIndex` (без `?symbol=`)
каждые 10с; `parse_premium_index` возвращает Funding по ВСЕМ. НО `poll_premium_index` **фильтрует до
subscribed** перед `tx.send` (строка «фильтруем на нашу выборку», ~1305) → вселенная выбрасывается,
в журнал идут только BTC/ETH.

**M-34:** (1) снять фильтр в breadth-режиме (эмитить все перпы); (2) даунсэмпл периода 10с→60с (founder ~1/мин:
фандинг меняется каждые 8ч, 60с с запасом; снижает объём журнала ×6).

## Объём журнала (учтено)

~400 перпов × 1/60с ≈ **400 Funding-событий/мин ≈ 576k/сутки**; payload Funding крошечный (~50 B) ≈ **~29 MB/сутки**
против ~30 MB/**мин** прочего потока recorder'а. Приемлемо. Ретеншен (TD-020) уже в проде.

## Contract impact (T1) — НЕТ

`MdPayload::Funding` уже есть. Меняется только venue-логика emit-множества + период. CT-RFC не нужен, новых
крейтов нет.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | **RED FB-I-1** (`crates/venue-binance-futures/tests/red_funding_breadth.rs`) + `verify_M-34.sh`. Sacred. | architect | compile-RED; FB-I-1 FAIL против legacy-фильтра, GREEN после breadth-select (reachability обе стороны) |
| 2 | ⏳ | **impl:** вынести emit-решение в pure `select_funding_emit(parsed, subscribed, breadth) -> Vec<MdEvent>`; `poll_premium_index` зовёт с `breadth=true`; `FUNDING_POLL_PERIOD` 10с→60с | venue-dev | FB-I-1 GREEN; существующие venue-тесты (red_funding/red_funding_poll/red_parse) регресс-GREEN; fmt+clippy CI-точно |
| 3 | ⏳ | **§8 eyes-on (после билинга TD-037):** деплой → на VPS проверить, что журнал несёт Funding по МНОГИМ символам (не только BTC/ETH); recorder healthy, hb свежий | reviewer | grep свежего сегмента: >100 distinct funding-символов; прод healthy |

## §Инвариант (RED-оракул; sacred, architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **FB-I-1** | **Breadth эмитит ВСЕ перпы.** `select_funding_emit(parsed, subscribed, breadth=true)` → все parsed (вселенная), порядок сохранён; `breadth=false` → legacy-фильтр до subscribed (регрессия). | `red_funding_breadth.rs`. **Анти-плацебо:** legacy inline-фильтр (breadth игнор) → breadth=true даёт только subscribed → FAIL |

## §Анти-плацебо чек-лист
- **Множественность:** ≥2 не-subscribed перпа обязаны эмититься (не «один»).
- **Регрессия legacy:** breadth=false по-прежнему фильтрует (трек-режим не сломан).
- **Отсутствие:** пустой parsed → пусто (fail-closed, не паника).
- **Порядок:** сохранён (детерминизм записи в журнал).

## Allowed / Forbidden paths
- **architect (sacred):** `milestones/M-34-funding-breadth.md`, `crates/venue-binance-futures/tests/red_funding_breadth.rs`, `scripts/verify_M-34.sh`.
- **venue-dev (impl):** `crates/venue-binance-futures/src/lib.rs` — `select_funding_emit` + `poll_premium_index` вызов + `FUNDING_POLL_PERIOD`.
- **Forbidden:** contracts (T1), risk/ks/oms, order-egress (submit/cancel/auth-торговли) — их тут НЕТ (MD-only); другие venue-крейты.

## Acceptance (`scripts/verify_M-34.sh`)
CI-точно (RN-17/TD-035): `cargo fmt --all -- --check` + `cargo clippy -p venue-binance-futures --all-targets --all-features -- -D warnings`.
- FB-I-1 GREEN (`--test red_funding_breadth`);
- регресс venue-тестов GREEN (`--test red_funding --test red_funding_poll --test red_parse`);
- grep: `FUNDING_POLL_PERIOD` = `from_secs(60)` (даунсэмпл применён);
- финал `VERDICT: PASS`/`FAIL`, exit соответствует.

## Handoff
Task 1 (RED) — architect ПЕРЕД impl. Task 2 — venue-dev (MD-only). Task 3 §8 — reviewer ПОСЛЕ билинга (TD-037):
до восстановления CI/CD §8 недоступен, merge держится до зелёного пайплайна.
