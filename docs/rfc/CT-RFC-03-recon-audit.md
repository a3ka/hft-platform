# CT-RFC-03 — Аудит-событие сверки с биржей (`SysEvent::ReconDivergence`)

STATUS: **PROPOSED** (atomic contract-RFC, 2026-07-16, architect). Гейт: critic (`gates.md` §1.1
T1-триггер) → reviewer (Contract Block-C) → founder ★ (фаза P2.5). Блокирующая подзадача M-09
(`milestones/M-09-data-safety-net.md` task 1).

## §1. Проблема (почему это нельзя записать в лог/метрику)

`OPS-I-1` (`docs/fa/ops.md` §4): recon сверяет локальную книгу с независимым REST-снапшотом
биржи; при расхождении > порога → алерт + принудительный ресинк + **аудит-событие в журнал**.
Событие обязано быть **в том же журнале, что и данные**: через месяц по журналу нужно ответить
«каким участкам данных верить». Метрика (`book_divergence_bps`) этого не даёт — она мгновенна и
не персистится в журнал (OPS-I-6: журнал детерминирован, метрики/wall-clock в него не пишутся).
Лог — не durable-часть контракта. Значит нужен новый вид доменного события.

`EventKind::Sys(SysEvent)` — T1 (`docs/05-contract-layer.md`): расширение — только атомарным
contract-RFC, аддитивно, с полным пакетом (типы + сгенерированная JSON Schema + фикстуры +
CHANGELOG + RED). Это он.

## §2. Решение: аддитивный вариант `SysEvent` (строго в конец)

```rust
pub enum SysEvent {
    Heartbeat,                    // 0  — не тронут
    ConnUp(Venue),                // 1  — не тронут
    ConnDown(Venue),              // 2  — не тронут
    ReconDivergence(ReconAudit),  // 3  — НОВЫЙ (CT-RFC-03)
}

pub struct ReconAudit {
    pub venue: Venue,
    pub symbol: String,
    pub divergence_bps: i64,        // макс расхождение сумм полос, bps (магнитуда)
    pub best_price_diverged: bool,  // расхождение best bid/ask = порча (ε_test / C1-класс)
    pub action: ReconAction,
}

pub enum ReconAction { AlertOnly, Resynced }  // 0, 1
```

**Форма выбрана минимальной и достаточной** (BACKLOG M-09 / ops §4): `venue`+`symbol`+момент
(из `Event.seq`/`ts`)+магнитуда (`divergence_bps`)+класс (`best_price_diverged` — порча лучшей
цены отделена от расхождения дальних полос, потому что именно best-bid стирала эвикция C1)+
действие (`AlertOnly`/`Resynced`). Поля struct ФИКСИРОВАНЫ: добавление поля ломает postcard
старых записей ⇒ любое расширение — новым RFC.

## §3. Почему `schema_version` НЕ бампится (остаётся 2)

CT-RFC-02 бампил 1→2, потому что менял **формат сегмента** (добавил `SegmentHeader`). CT-RFC-03
не трогает ни сегмент-заголовок, ни конверт `Event` — это **аддитивный вариант `EventKind`**,
как `MdPayload`-расширения в CT-RFC-01 (те тоже не бампили версию). Старые журналы читаются
байт-в-байт (CT-I-3); новые события просто появляются в новых сегментах (по-прежнему schema 2).
Дискриминанты 0/1/2 неизменны, новый = 3 — старый читатель никогда не встретит вариант 3 в
старом журнале (мы деплоим код раньше, чем появляется событие).

## §4. Инварианты (RED, sacred — `crates/contracts/tests/red_rfc03.rs`)

- `schema_version` == 2 (не бампится — аддитивно).
- Дискриминанты `SysEvent`: Heartbeat=0/ConnUp=1/ConnDown=2/ReconDivergence=**3** (строго хвост).
- `ReconAction`: AlertOnly=0/Resynced=1.
- Роундтрип recon-события (postcard, тот же конверт).
- **CT-I-3:** `Heartbeat` и `Md(Trade)`, записанные ДО RFC-03, читаются байт-в-байт.
- `best_price_diverged` переживает сериализацию (порча ≠ шум дальних полос).
- Анти-плацебо: без вариантов `ReconDivergence`/`ReconAudit`/`ReconAction` файл НЕ КОМПИЛИРУЕТСЯ
  (compile-RED — тип действительно добавлен, а не «запланирован»); дискриминант-тест падает при
  вставке варианта не в конец.

## §5. Изменения по файлам (атомарно, один PR)

- `crates/contracts/src/lib.rs` — `ReconAudit`, `ReconAction`, вариант `SysEvent::ReconDivergence`.
- `crates/contracts/schema/event.schema.json` — СГЕНЕРИРОВАНА
  (`cargo run -p contracts --example gen_schema`); гейт `red_schema.rs` (CT-I-4).
- `crates/contracts/fixtures/valid/event-recon.json` — валидное recon-событие.
- `crates/contracts/fixtures/invalid/event-recon-unknown-action.json` — неизвестный `action` → reject.
- `crates/contracts/tests/red_rfc03.rs` — RED (§4).
- `crates/contracts/CHANGELOG.md` — запись CT-RFC-03.

**НЕ изменено:** `Event`, `EventKind`, `MdEvent`, `MdPayload`, `Venue`, `Side`, `Level`,
`SegmentHeader`, `SysEvent::{Heartbeat,ConnUp,ConnDown}` — wire-формат прежний (CT-I-3).

## §6. Связь с M-09

Разблокирует task 2 (recon): рантайм пишет `ReconDivergence` в журнал при расхождении > `ε_prod`;
research-dev офлайн агрегирует эти события в `research/data-quality/` (OPS-I-1 п.4). `ε_test`
(RED-оракул recon) и метрики — в milestone-RED M-09, вне этого RFC.

## §7. Что НЕ входит

- Рантайм-логика recon (сравнение, пороги, ресинк, rate-budget) — задача 2 M-09 (`crates/ops` +
  `crates/venue-*`), не контракт.
- `Ord/Risk/Ctl`-варианты `EventKind` — P3, отдельные RFC.
