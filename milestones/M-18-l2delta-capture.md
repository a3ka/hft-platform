# M-18 — Захват сырых book-дельт `L2Delta` (CT-RFC-04, Phase B / book-flow)

STATUS: **PROPOSED** (2026-07-21, architect). Блок-1 roadmap (BACKLOG «Порядок исполнения»),
TIME-SENSITIVE. Doc-гейт §9 Class A. Гейты: **critic + risk-critic** (T1 + sacred live-path).
Ветка: `feat/M-18-l2delta`. RFC: `docs/rfc/CT-RFC-04-l2delta.md`.

## Objective

Order-flow book-flow (absorption / iceberg / DOM / Bookmap-heatmap — метод Fabio) вычислим ТОЛЬКО
из **сырых инкрементальных book-дельт**. Мы их **уже принимаем** (`@depth@100ms` RAW DIFF), но
**выбрасываем** — в журнал уходит лишь бакетированный `L2Snapshot`. Каждый день без захвата =
безвозвратно потерянные дельты (данные невоспроизводимы). M-18 добавляет T1-вариант
`MdPayload::L2Delta` и капчит каждый распарсенный diff, **не меняя** ни order-путь, ни risk-инвариант,
ни поведение fill'а бэктеста. Разблокирует M-19 Тир-3 + семью absorption-сигналов S-003+.

**Аддитивно и безопасно:** `SEGMENT_MAGIC`/`SCHEMA_VERSION` не трогаются (журнал идентифицирует
сегменты по магии; версия на чтении не валидируется), старые журналы читаются байт-в-байт (CT-I-3).
L2Delta НЕ заменяет `L2Snapshot` (тот остаётся recon-якорем) — дельта тонкая эволюция между якорями.

## Contract impact (T1) — ДА (CT-RFC-04)

`MdPayload::L2Delta { bids, asks, first_update_id, final_update_id, prev_final_update_id, ts_exch_ms }`
— аддитивно В КОНЕЦ (postcard-дискриминант 6). Полный пакет RFC (тип + сген. JSON Schema + фикстуры
valid/invalid + CHANGELOG + red_rfc04) — уже в наборе architect'а (задача 1). Правка `contracts/`
ТОЛЬКО через этот RFC (CT-I-2, Block-C).

## Инварианты (RED, sacred)

| ID | Инвариант |
|---|---|
| L2D-I-1 | **Аддитивность СТРОГО в конец (CT-I-3):** дискриминанты MdPayload 0..5 неизменны, L2Delta=6; исторический байт-блоб L2Snapshot (до-L2Delta) декодится байт-в-байт. RED `red_rfc04` |
| L2D-I-2 | **Capture без потерь:** транслятор `&DepthDiff → EventKind::Md(L2Delta)` сохраняет КАЖДОЕ поле — U→first, u→final, pu→prev_final (spot None), E→ts, уровни включая `size==0`, пустую сторону. Losslessness ⇒ достаточность реконструкции стакана по построению. RED `red_l2delta_capture`(spot)/`red_l2delta_futures`(fut) |
| L2D-I-3 | **Семантика уровней (= apply_diff_to_book §A, testing.md «отсутствие»):** `size==0` = явный remove; уровень, которого в дельте НЕТ, — НЕ трогается; пустая сторона = «не менялось», не «очистить» |
| L2D-I-4 | **Continuity перпа по `pu`, не по `U==last+1`** (урок TD-014): futures несёт `prev_final=Some(pu)`, spot — `None`; путаница ломает gap-детекцию. RED `red_l2delta_futures` |
| L2D-I-5 | **Sacred write-path exact:** L2Delta переживает write→read_all (postcard+crc32) байт-в-байт; DET-I-1 не ослаблен. RED `red_l2delta_persist` |
| L2D-I-6 | **Инертность к бэктесту и safety:** `sim` ИГНОРИРУЕТ L2Delta (fill ведётся из L2Snapshot+Trade — сырая дельта = двойной учёт); order-путь/risk не тронуты (MD-only). Консюмер-армы — дефолт IGNOR (кроме journal ts) |
| L2D-I-7 | **Магия/версия неизменны:** `SEGMENT_MAGIC=HFTJRN02`, `SCHEMA_VERSION=2` — bump сломал бы чтение боевых сегментов. verify-канарейка |

## Allowed / Forbidden paths

- `crates/contracts/src/lib.rs` (T1 `L2Delta`, ТОЛЬКО через CT-RFC-04), `schema/`, `fixtures/`,
  `CHANGELOG.md`, `docs/rfc/CT-RFC-04-l2delta.md` — **architect** (T1 via RFC).
- `*/tests/**` (red_rfc04, red_l2delta_capture, red_l2delta_futures, red_l2delta_persist),
  `scripts/verify_M-18.sh`, milestone — **architect** (sacred).
- `crates/venue-binance/src/lib.rs` (spot `l2delta_event` + emit в `run`/`handle_text_message`) — **venue-dev** (MD-only).
- `crates/venue-binance-futures/src/lib.rs` (`pub struct DepthDiff` + `l2delta_event(Some(pu))` + emit) — **venue-dev** (MD-only).
- `crates/journal/src/segments.rs` (арм L2Delta в `segment_last_ts` OR-паттерн) — **engine-dev**.
- `crates/sim/src/exchange.rs` (арм L2Delta ⇒ IGNOR) — **engine-dev**; + любой оставшийся E0004-сайт.
- **Forbidden:** order-путь (submit/cancel/auth), `crates/{risk,killswitch,oms}`, смена
  `SEGMENT_MAGIC`/`SCHEMA_VERSION`, движок реконструкции/absorption-сигналы (M-19/research, не здесь),
  расширение набора символов захвата без решения founder'а по §5 RFC.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ✅ DONE | CT-RFC-04 doc + `MdPayload::L2Delta` T1 + сген. JSON Schema + фикстуры valid/invalid + CHANGELOG + `red_rfc04` | architect | red_rfc04 + red_schema GREEN; L2D-I-1 |
| 2 | ✅ DONE | Sacred RED: `red_l2delta_capture` (spot), `red_l2delta_futures` (fut), `red_l2delta_persist` (journal) + `verify_M-18.sh` | architect | compile-RED падает без impl; достижим прототипом |
| 3 | ⏳ | venue-binance СПОТ: `pub fn l2delta_event(&DepthDiff)` (prev_final=None) + вызов в emit-пути для КАЖДОГО распарсенного diff'а (независимо от sync-FSM) | venue-dev | L2D-I-2/3 GREEN; wiring-канарейка |
| 4 | ⏳ | venue-binance-futures ПЕРП: `pub struct DepthDiff` + `l2delta_event` (prev_final=Some(pu)) + emit | venue-dev | L2D-I-2/4 GREEN |
| 5 | ⏳ | Консюмер-армы: `journal segment_last_ts` (+L2Delta в ts OR-паттерн) + `sim exchange` (IGNOR) + любой E0004-сайт; `red_l2delta_persist` GREEN; workspace собирается | engine-dev | L2D-I-5/6 GREEN; `cargo build --workspace --all-targets` ok |
| 6 | ⏳ | **§8 live-emit на VPS** (РЕШАЮЩИЙ, unit≠live TD-014): после deploy в журнале появляются `Binance.L2Delta` + `BinanceFutures.L2Delta` с живого WS; темп записи в бюджете §5; recorder healthy, `seq_gaps=0`, disk `writable=true` | reviewer | §8 GREEN + пруф в close-out |
| 7 | ⏳ | tester: чистый чекаут — fmt/clippy/`cargo test`/`verify_M-18.sh` PASS exit=0 | tester | VERDICT: PASS |

## Гейты

- **critic** (новый milestone §9 Class A; T1-триггер; ≥5 коммитов; ломающих форму T1 НЕТ — аддитивно).
- **risk-critic ОБЯЗАТЕЛЕН** (`gates.md` §5): T1-изменение + sacred live-path (venue/recorder/journal).
  Фокус: (0) объём/ретеншен-развилка §5 RFC — ускорение disk-таймера ~40→~20 дней ⇒ TD-020 срочен;
  (1) MD-only (order-путь не тронут); (2) инертность бэктеста (sim игнорирует); (3) магия/версия целы
  (прод-чтение); (4) аддитивность дискриминантов (старые журналы). Вердикт `research/critiques/C-0NN.md`.
- **§8 post-merge** (прод НЕ инертен — recorder начинает писать L2Delta): задача 6, решающий live-гейт.
- Founder ★: решение по развилке §5 RFC (а: ограниченный набор символов / б: полный + срочный ретеншен).

## Handoff (план)

critic → **founder ★** (развилка §5: какой набор символов гнать) → venue-dev (задачи 3/4, emit) ∥
engine-dev (задача 5, армы) → tester (задача 7) → reviewer (merge + §8 задача 6 live-emit).
risk-critic — параллельно после commit набора (T1+sacred). Architect: задачи 1/2 (готовы).
