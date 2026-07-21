# CT-RFC-04 — `MdPayload::L2Delta`: персист сырых book-дельт (`@depth` diff)

STATUS: PROPOSED (architect, 2026-07-21). Атомарный contract-RFC per `docs/05-contract-layer.md`
§4. Трогает `crates/contracts/**` (T1) → Block-C, **critic обязателен** (`gates.md` §1.1) +
**risk-critic обязателен** (`gates.md` §5 — касание T1 И sacred live-path venue/recorder/journal).
Ветка: `feat/M-18-l2delta`. Milestone: `milestones/M-18-l2delta-capture.md`.

## §1. Проблема (почему это TIME-SENSITIVE, а не «когда-нибудь»)

Order-flow скальпинг (метод Fabio) делится на **trade-flow** (агрессия сделок — закрыто M-17,
`Trade.side` уже пишется) и **book-flow** (динамика лимиток: **absorption / iceberg / DOM /
Bookmap-heatmap**). Book-flow вычислим ТОЛЬКО из **сырых инкрементальных book-дельт** — потока
«на цене X объём стал Y» с точным порядком.

Мы эти дельты **уже принимаем** (`venue-binance`/`venue-binance-futures` подписаны на
`@depth@100ms` RAW DIFF и применяют их к локальной книге), но **выбрасываем**: в журнал уходит
только периодический бакетированный `L2Snapshot` (`bucket_levels`, окно эмиссии). Снапшот теряет
эволюцию между кадрами — по нему НЕЛЬЗЯ отличить «100 BTC поглощены агрессией» от «лимитка снята и
переставлена». **Каждый день без захвата = безвозвратно потерянные book-дельты** (журнал бессмертен,
но записать задним числом то, что не сохранили, невозможно — данные невоспроизводимы, в отличие от
кода). Отсюда приоритет: форма вводится и захват стартует ДО накопления, а не после.

Разблокирует: M-19 Тир-3 (DOM-ladder + Bookmap-heatmap), absorption/iceberg-сигналы (семья S-003+),
точную реконструкцию стакана для форензики.

## §2. Решение: аддитивный вариант `MdPayload::L2Delta` — сырой diff ДО применения к книге

Персистим **каждый распарсенный `@depth` diff** как отдельное событие `MdPayload::L2Delta`,
**независимо от book-sync FSM**: сырой diff — это ground-truth рыночное событие; наш sync-автомат
(REST-бутстрап, gap-resync) — локальная забота реконструкции, а не свойство данных. Continuity
несут update-id'ы В САМОЙ дельте → любой консюмер/реплей видит пропуск без обращения к бирже.

**Отвергнутый вариант — «снапшот чаще».** Более частый `L2Snapshot` не восстанавливает
absorption (какая сторона была съедена агрессией, а какая — отменена), увеличивает объём сильнее
дельты (полная книга каждый раз против только-изменившихся уровней) и всё равно теряет
внутрикадровый порядок.

`L2Delta` **не заменяет** `L2Snapshot`: снапшот остаётся recon-якорем (периодическое полное
состояние — точка ресинка реплея + вход OBI/recon). Дельта — тонкая эволюция МЕЖДУ якорями.
Вместе = точная реконструкция.

## §3. Wire-решения (locked — обоснование для critic/risk-critic)

1. **`SEGMENT_MAGIC` НЕ меняется (`HFTJRN02`).** Магия идентифицирует ФРЕЙМИНГ сегмента
   (postcard + crc32, header-first) — он не меняется. Проверено: `journal/src/segments.rs`
   идентифицирует наши сегменты ТОЛЬКО по `magic == SEGMENT_MAGIC` (точное равенство);
   `schema_version` в заголовке пишется, но на чтении не валидируется. Смена магии заставила бы
   отвергать боевые сегменты (15 GB legacy + текущие) — недопустимо.
2. **`SCHEMA_VERSION` остаётся `2`.** Изменение — аддитивный вариант enum, не смена формата
   заголовка/фрейминга. Прецедент CT-RFC-01: `MdPayload::{OpenInterest,Liquidation,MarginRate}`
   добавлены БЕЗ bump'а версии сегмента. Старые сегменты (только варианты 0..5) читаются
   байт-в-байт (CT-I-3, гвоздём-тестом).
3. **Postcard-дискриминант `L2Delta` = 6, СТРОГО в конец** (после `MarginRate` = 5). Дискриминанты
   0..5 неизменны (RED `ct_rfc04_discriminants_frozen_l2delta_is_index_six` + исторический
   байт-блоб `ct_rfc04_historical_l2snapshot_bytes_decode_identically`). Вставка не в конец
   сдвинула бы дискриминанты и сломала чтение всех старых журналов — это ЛОМАЮЩЕЕ изменение,
   запрещённое без major-bump + миграции (§4 `05`). Здесь — чисто аддитивно.

## §4. Тип (Rust-канон, `crates/contracts/src/lib.rs`)

```rust
/// Инкрементальная book-дельта — СЫРОЙ @depth diff, персистится ДО применения к книге.
L2Delta {
    bids: Vec<Level>,                  // ТОЛЬКО изменившиеся; size==0 = remove; отсутствие ≠ удаление
    asks: Vec<Level>,                  // пустая сторона = «не менялось», НЕ «очистить»
    first_update_id: u64,              // Binance U
    final_update_id: u64,              // Binance u
    prev_final_update_id: Option<u64>, // futures pu (чейн); spot None
    ts_exch_ms: i64,                   // Binance E
}
```

Переиспользует `Level{price,size}` (та же кодировка, что `L2Snapshot`). Семантика уровней —
идентична дисциплине `venue-binance::apply_diff_to_book` §A и `.claude/rules/testing.md`
«отсутствие»: `size==0` = явный remove от биржи; неупомянутый уровень НЕ трогается.

**Continuity:** spot — непрерывность `U == prev.u + 1`; futures — `pu == prev.u` (U/u у перпа
прыгают, урок TD-014). Оба выразимы: `prev_final_update_id = None` (spot) / `Some(pu)` (futures).

## §5. Объём и связь с ретеншеном (ГЛАВНЫЙ риск — для risk-critic)

`L2Delta` — **аддитивный** и самый частый MD-поток. Грубая оценка: 4 символа (2 spot + 2 futures)
× ~10 diff/с/символ × 86400 с ≈ 3.4M дельт/сутки; при ~200–800 B/дельта ≈ **+1.5…2.5 GB/сутки**
поверх текущих ~2.8 GB/сутки ⇒ суммарно **~4.5–5.5 GB/сутки** (примерно ×2 темп записи).

**Следствие — таймер диска ускоряется:** при 111 GB свободных ~40 дней → **~20 дней** до
disk-guard. Это НЕ портит данные (disk-guard fail-closed: `append` → `Err`, ни байта, ни `seq`;
`storage_status().writable=false` виден в heartbeat — TD-019 закрыт), но останавливает сбор.
**Значит L2Delta делает доставку ретеншена в прод (TD-020, задача 14 M-08) СРОЧНОЙ, а не
опциональной.** Честная развилка для founder/risk-critic (обе ветки безопасны):

- **(а) Стартовать захват на ОГРАНИЧЕННОМ наборе** (напр. только BTC spot+futures) — режет объём
  ~вдвое, ловит самый ликвидный инструмент первым, даёт время ретеншену. Расширение — отдельным
  решением. **Рекомендация architect'а** (time-sensitive выигрыш при ограниченном риске).
- **(б) Полный набор + ускоренный мониторинг** — опираемся на disk-guard + heartbeat
  `free_bytes`/`writable`; закрываем TD-020 (task 14) как немедленный следующий milestone.

В ЛЮБОМ случае: L2Delta НЕ меняет поведение fill'а бэктеста (sim игнорирует дельту, §6) и НЕ
ослабляет ни один risk-инвариант (данные MD-only, order-путь не тронут).

## §6. Миграция (CT-I-3) и обязательные правки консюмеров

**Чтение старых журналов:** не ломается — старые сегменты содержат только дискриминанты 0..5;
новый enum читает их байт-в-байт (гвоздь-тест §3). Обратной совместимости «старый код ↔ новый
журнал» контракт НЕ требует (код обновляется в lockstep; инвариант — new-code-reads-old-journal).

**Source-level: аддитивный вариант enum ЛОМАЕТ исчерпывающие `match MdPayload` без wildcard.**
ПОЛНЫЙ список — **ровно 5 сайтов** (компилятор — исчерпывающий оракул; проверено
prototype-revert'ом `cargo build --workspace --all-targets` до 0×E0004; C-017 blocker 1):

| # | Сайт | Действие | Обоснование | Зона |
|---|---|---|---|---|
| 1 | `journal/src/segments.rs` (`segment_last_ts`) | `\| MdPayload::L2Delta { ts_exch_ms, .. }` в OR-паттерн `=> *ts_exch_ms` | у дельты ЕСТЬ биржевое время — timestamped-событие | engine-dev |
| 2 | `sim/src/exchange.rs` (`on_event`) | `MdPayload::L2Delta { .. } => {}` (ИГНОР) | честный симулятор ведёт fill из `L2Snapshot`+`Trade`; сырая дельта = двойной учёт книги ⇒ НЕ вход бэктеста | engine-dev |
| 3 | `recorder/src/lib.rs` (`md_kind_label`) | `MdPayload::L2Delta { .. } => "l2delta"` | Prometheus-лейбл kind (OPS-метрики M-09); L2Delta — свой поток, отдельный лейбл | engine-dev |
| 4 | `journal/examples/dump.rs` (payload match) | `MdPayload::L2Delta { .. } => {}` (ИГНОР) | offline-дамп; печать сырой дельты вне scope M-18 | engine-dev |
| 5 | `research-cli/src/bin/latency_probe.rs` | `\| MdPayload::L2Delta { .. }` в `=> continue` OR-группу | latency-probe меряет δ_md для Trade/L2Snapshot/Funding; дельта — как OI/Liq/MarginRate (continue) | engine-dev |

`recorder/src/recon_loop.rs` и `research-cli/src/export_io.rs` уже несут `_ =>` wildcard —
правок НЕ требуют. Дефолт арма — ИГНОР; исключения: #1 (несёт ts), #3 (лейбл `"l2delta"`).
`verify_M-18.sh` гейтит замыкание списка через `cargo build --workspace --all-targets` (T5b).

## §7. Верность захвата (без потерь) и живой gate

- **Losslessness ⇒ достаточность реконструкции по построению.** RED `red_l2delta_capture`
  (spot) / `red_l2delta_futures` (futures): транслятор `&DepthDiff -> EventKind::Md(L2Delta)`
  сохраняет КАЖДОЕ поле (U→first, u→final, pu→prev_final, E→ts, уровни включая `size==0`, пустую
  сторону). Если форма несёт всё, что несёт diff, — реконструкция стакана возможна по определению.
  Анти-плацебо: падает, если поле потеряно/сторона перепутана/pu подставлен на споте.
- **Sacred write-path:** RED `journal::red_l2delta_persist` — L2Delta переживает
  write→read_all (postcard+crc32) байт-в-байт (DET-I-1 exact-replay).
- **§8 live-emit — РЕШАЮЩИЙ гейт (урок TD-014: unit-green ≠ live-emit).** Юнит доказывает
  ТРАНСФОРМ, не то, что адаптер реально ШЛЁТ дельту с боевого WS. Acceptance M-18 требует §8:
  после deploy в журнале ПОЯВЛЯЮТСЯ `Binance.L2Delta` и `BinanceFutures.L2Delta` с живого потока,
  темп записи в пределах бюджета §5, recorder healthy, `seq_gaps=0`. Без этого milestone не закрыт.

## §8. Пакет RFC (полный, per `05` §4)

1. Тип: `crates/contracts/src/lib.rs` — `MdPayload::L2Delta` (аддитивно в конец). ✅
2. JSON Schema: `crates/contracts/schema/event.schema.json` — **СГЕНЕРИРОВАНА**
   (`cargo run -p contracts --example gen_schema`); гейт `red_schema` падает при расхождении
   (CT-I-4). ✅
3. `schema_version`: без bump (аддитивно, §3 п.2). Магия без изменений.
4. Миграция: §6 (старые журналы читаются; консюмеры получают арм).
5. Фикстуры: `fixtures/valid/event-l2delta-{spot,futures}.json`,
   `fixtures/invalid/event-l2delta-missing-final-id.json`. ✅
6. CHANGELOG: `crates/contracts/CHANGELOG.md` — запись CT-RFC-04. ✅
7. Тесты: `crates/contracts/tests/red_rfc04.rs` (роундтрип, дискриминанты, исторический блоб,
   losslessness, futures pu, фикстуры). ✅

**Как вычислен исторический байт-блоб** (§3, гвоздь CT-I-3): `postcard::to_stdvec` на
`Event{seq:3,…, L2Snapshot{bids:[6500050000000/10000000], asks:[6500060000000/20000000],
ts:1752000000123}}` НА ДЕРЕВЕ БЕЗ L2Delta → 49 байт (в тесте как `const HISTORICAL`). Не
перегенерировать — смысл в неизменности.

## §9. Чего RFC НЕ делает

- Не строит движок реконструкции стакана / absorption-сигналы (семья S-003+, M-19 Тир-3, research).
- Не трогает order-путь и risk-инварианты (MD-only). recorder пишет L2Delta generic'ом (без правок).
- Не решает развилку §5 (а/б) — это решение founder'а по вердикту risk-critic.
- Не доставляет ретеншен в прод (TD-020 task 14) — но делает его срочным (§5).
```
