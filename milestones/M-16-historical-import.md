# M-16 — Импорт бесплатных историч. данных в журнал-формат (research-only)

STATUS: **PROPOSED** (2026-07-20, architect). Doc-гейт §9 Class A. Founder дал «go».
Приоритет в очереди — founder (данные-first; вероятно ПЕРЕД M-11 risk).

## Objective

Наши бэктест-вердикты недостоверны на коротком окне (M-10 kill-screen: SE годового Sharpe ≈±11 на
3–7 днях). Внешние ИСТОРИЧЕСКИЕ данные доступны **бесплатно/дёшево и открыто** (проверено architect'ом):
- **Binance** `data.binance.vision` — 100% бесплатно, без auth: trades/aggTrades + futures bookDepth/bookTicker;
- **Hyperliquid** CC0-зеркала (SonarX / Reservoir) + офиц. S3 `hyperliquid-archive` — L2-снапшоты (~20 ур.) +
  fills, requester-pays (центы; ~$0 in-region), CC0-лицензия.

Импорт этих данных в НАШ журнал-формат: (а) расширяет окно M-10 → достовернее вердикт; (б) даёт
**глубину HL** для OBI Трек B (наш живой HL — мелкий); (в) фундамент для backfill исследований.

**⚠ RESEARCH-ONLY, НИКОГДА не трогает рантайм-сбор.** Импортированные данные — отдельные сегменты с
provenance-тегом (CT-RFC-02); детерминированный live-журнал не смешивается с импортом (DET-I-1 sacred).

## Contract impact (T1) — НЕТ

Импорт → существующие `MdPayload::{L2Snapshot, Trade}`. Источник/provenance — уже есть (CT-RFC-02).
Новых T1-вариантов не нужно → CT-RFC не требуется.

## Инварианты (RED, sacred)

| ID | Инвариант |
|---|---|
| HI-I-1 | **Формат→MdEvent верно:** Binance aggTrade → `Trade{side=aggressor}` (m-флаг инверсия, как live-парсер); bookDepth/l2Book → `L2Snapshot{bids,asks}` с сохранением уровней. RED: фикстура реального формата → ожидаемый MdEvent |
| HI-I-2 | **Provenance-изоляция:** каждый импортированный Event помечен источником (`imported:binance-vision`/`imported:hl-cc0`), НЕ выдаётся за own-capture. RED: импорт не пишется в live-сегмент; own-capture и импорт различимы |
| HI-I-3 | **Порядок + дедуп:** события упорядочены по `ts_exch_ms`, дубли по (source,ts,symbol) отброшены (внешние архивы бывают с перекрытием). RED: перекрывающийся вход → монотонный дедуплицированный выход |
| HI-I-4 | **Эпоха ledger'а (TD-015):** бэктест на импорте НАЗЫВАЕТ источник+окно в отчёте; deflated-Sharpe считает импорт-эпоху отдельно. Смешение own/imported без пометки → KILL (risk-critic) |
| HI-I-5 | **Честная граница глубины:** HL CC0/архив = ~20-уровневые СНАПШОТЫ (не tick-дельты); Трек B на импорте валиден для depth-полос, absorption/DOM — НЕ из этого источника (нужен свой raw-delta захват, M-18) |

## Allowed / Forbidden paths

- `crates/research-cli/src/**` (importer: парсеры внешних форматов → MdEvent, provenance-тег) — **research-dev**.
- `research/data-imported/**` (импортированные сегменты, gitignore для крупных) — **research-dev**.
- `*/tests/**` (HI-I-* RED), `scripts/verify_M-16.sh`, milestone — **architect**.
- **Forbidden:** запись импорта в live journal-сегменты (`journal-data`); касание `crates/{recorder,venue-*,journal}/src` рантайм-путей; секреты AWS в репо (креды — env/deploy, не коммит).

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | HI-I-* RED (`crates/research-cli/tests/red_import.rs`): формат→MdEvent, provenance-изоляция, дедуп/порядок | architect | RED падает против отсутствия импортёра; достижим |
| 2 | ⏳ | `verify_M-16.sh` | architect | exit=0 на GREEN |
| 3 | ⏳ | Импортёр Binance (`data.binance.vision`: aggTrades + futures bookDepth) → MdEvent + provenance | research-dev | HI-I-1/2/3 GREEN |
| 4 | ⏳ | Импортёр HL (CC0 l2Book снапшоты + fills) → MdEvent + provenance | research-dev | HI-I-1/2/5 GREEN |
| 5 | ⏳ | Прогон OBI (M-10) на РАСШИРЕННОМ окне (own+imported, эпоха названа) → обновить R-001 достоверность | research-dev | окно шире; вердикт эпоху называет |

## Гейты

- critic (новый milestone §9). НЕ трогает T1/safety → **risk-critic N/A** (research, MD-only, без order-path).
  НО: бэктест-ОТЧЁТ на этих данных проходит анти-оверфит §6 + risk-critic (как в M-10) — это гейт ОТЧЁТА, не импорта.
- §8 не применим (research-only, не деплой-путь). Прогон — на dev/CI.

## Handoff (план)

critic → research-dev (импортёры + прогон) → risk-critic (на ОТЧЁТ по расширенным данным). Architect: HI-I-* RED + verify.
