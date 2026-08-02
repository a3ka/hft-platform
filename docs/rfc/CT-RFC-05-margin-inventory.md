# CT-RFC-05 — `MdPayload::MarginInventory`: сырой supply-пул margin-займов (retro)

STATUS: **RETRO-DOCUMENTED (2026-08-02)**. Этот документ восстановлен ПОСТФАКТУМ — сама
T1-правка приземлилась в коде и на проде РАНЬШЕ, чем появился этот RFC-файл. Правка прошла
`crates/contracts/src/lib.rs` уже 2026-07-25 (`e06e48a`, merge в `main` — `ba61c62`,
2026-07-25T19:50:10Z), т.е. **8 дней без формального `docs/rfc/CT-RFC-05-*.md`**, хотя
`docs/05-contract-layer.md` §4 требует его как часть атомарного пакета. Дыра обнаружена
аудитом `docs/plans/contracts-current-state.md` (раздел Д2) и независимо подтверждена
критиком в `research/critiques/C-040-design-migration-plan.md:269-272`. Причина отсутствия —
§7 ниже. Документ не проектирует и не предлагает изменений — реконструирует форму, класс и
обоснование бампа по фактам из кода, тестов, CHANGELOG и git-истории.

## §1. Что изменилось в T1

**До (schema_version = 3, `MdPayload` — 7 вариантов, дискриминанты 0..6):**
`Trade`(0) / `L2Snapshot`(1) / `Funding`(2) / `OpenInterest`(3) / `Liquidation`(4) /
`MarginRate`(5) / `L2Delta`(6).

**После (schema_version = 4, добавлен вариант дискриминант 7):**

```rust
/// Market-wide доступный к займу пул margin per-asset (Binance
/// `/sapi/v1/margin/available-inventory?type=MARGIN`, auth read-only). `symbol` = актив
/// ("USDT"/"USDC"). `available_e8` = доступный объём ×1e8 (≥0). Аддитивно В КОНЕЦ
/// (postcard-дискриминант 7; старые сегменты 0..6 читаются байт-в-байт, CT-I-3). CT-RFC-05.
MarginInventory {
    available_e8: i64,
    ts_exch_ms: i64,
},
```

(`crates/contracts/src/lib.rs:311-315`, вариант вставлен строго последним в `MdPayload`, после
`L2Delta`). Поля: `available_e8` — сырой доступный к займу объём в fixed-point ×1e8 (`i64`);
`ts_exch_ms` — биржевое время события в мс (`i64`, = Binance `updateTime`, приведённое к мс).
Событие оборачивается как обычное MD-событие: `Event { kind: EventKind::Md(MdEvent { venue,
symbol, payload: MdPayload::MarginInventory{..} }) }` — новых top-level типов (`Event`,
`EventKind`, `Venue`, `Side`, `Level`, `SegmentHeader`) правка не вводит.

Ни один из существующих вариантов `MdPayload` (0..6) не переименован, не изменён по составу
полей и не переставлен — подтверждено тем, что `crates/contracts/tests/ct_rfc05.rs::
mi_i_1_pre_rfc05_funding_still_decodes` декодирует захардкоженный pre-change postcard-блоб
варианта `Funding` (дискриминант 2) и проверяет побайтовое совпадение полей.

## §2. Класс изменения — аддитивное

По классификации `docs/05-contract-layer.md` §4 («Аддитивное — новое опциональное поле, новый
вариант enum в конце» vs «Ломающее — удаление/переименование поля, смена типа, порядок enum»):

- Новый вариант `MdPayload::MarginInventory` добавлен **строго в конец** enum'а (постcard
  сериализует discriminant по позиции объявления) — доказано тем же тестом
  `mi_i_1_pre_rfc05_funding_still_decodes`: если бы вставка была НЕ в конец, она сдвинула бы
  дискриминанты существующих вариантов, и pre-change байты `Funding` декодировались бы в
  другой вариант или падали — тест ловит именно это (комментарий в файле называет это
  анти-плацебо: «если `MarginInventory` вставлен НЕ в конец enum'а → `FUNDING_PRECHANGE`
  декодится неверно → (b) FAIL»).
- Ни одно существующее поле не удалено, не переименовано, не сменило тип.
- Ни один существующий вариант (`Trade`..`L2Delta`, 0..6) не переставлен.

⇒ Изменение **аддитивное**, не ломающее.

## §3. Почему потребовался bump `SCHEMA_VERSION` 3 → 4

По факту из комментария в коде (`crates/contracts/src/lib.rs:14-26`) и записи в
`crates/contracts/CHANGELOG.md`: с 2026-07-21 (`CT-RFC-04` rev2, TD-031) `SCHEMA_VERSION`
перестал быть версией wire-совместимости (аддитивные варианты и без того обратно совместимы,
`CT-I-3`) и стал **машинным маркером эпохи сегмента** для `decide_open_segment` — recorder
переиспользует (`reuse`) хвостовой сегмент под запись только если
`header.schema_version == SCHEMA_VERSION` текущего бинаря. Это исправление после инцидента
TD-031: изоляция сегментов раньше держалась на `provenance` (git-sha), но recorder в
рантайм-контейнере без `git` даёt константный provenance на всех деплоях, поэтому
provenance-изоляция была void, и `L2Delta`-события смешались со старым (schema-2) сегментом.

Правило, зафиксированное этим прецедентом: **«новый эмитируемый вариант ⇒ bump
`SCHEMA_VERSION`»**. `MarginInventory` — новый эмитируемый вариант (recorder начинает реально
писать его в журнал, не просто добавляет вариант для парсинга офлайн), поэтому бамп 3→4
обязателен по этому правилу — так же, как `CT-RFC-04` rev2 был обязан бампнуть 2→3 для
`L2Delta`. Это подтверждено отдельным исправлением `b3a5a95` («fix(M-35): red_rfc04
epoch-tripwire 3→4 — CT-RFC-05 bump — L2Delta-эпоха историческая, текущая=MarginInventory»),
которое обновило существующий regression-тест на предыдущую эпоху под новое значение
`SCHEMA_VERSION`, и тестом `mi_i_1_schema_version_is_4` (`ct_rfc05.rs`), который прямо
утверждает `SCHEMA_VERSION == 4`.

`SEGMENT_MAGIC` (фрейминг, `HFTJRN02`) этой правкой не тронут — bump касается только эпохи
набора вариантов, не формата фрейминга сегмента (та же развязка, что и в CT-RFC-04 §3).

## §4. Совместимость

- **Чтение старых сегментов.** `schema_version` НЕ валидируется на чтении (`CT-I-3`) — старые
  сегменты (эпохи 0..3, дискриминанты `MdPayload` 0..6) читаются новым кодом байт-в-байт.
  Доказано тестом `mi_i_1_pre_rfc05_funding_still_decodes`, который декодирует pre-RFC05
  postcard-байты варианта `Funding` под кодом с `SCHEMA_VERSION == 4` и сверяет поля побайтово.
- **Запись / reuse хвостового сегмента.** `decide_open_segment` (эффект правила §3) не
  переиспользует сегмент с `header.schema_version < 4` под новый бинарь — открывает новый
  сегмент. Прямого RED-теста именно на `MarginInventory`-эпоху reuse-барьера в диффе CT-RFC-05
  не обнаружено (машина этого барьера — общий код `decide_open_segment`, введённый в
  TD-031/CT-RFC-04, не переписан заново под CT-RFC-05); гарантия эпохи 3→4 подтверждается
  косвенно тестом `mi_i_1_schema_version_is_4` + фиксом `b3a5a95` эпохи-tripwire теста
  `red_rfc04`, а не отдельным сегмент-reuse-тестом для дискриминанта 7.
- **Исчерпывающий match.** Добавление варианта enum ломает компиляцию любого `match
  &MdPayload` без wildcard (source-level breaking, не wire-level). Milestone
  `milestones/M-35-margin-inventory.md` §Tasks 2b/2c/2d перечисляет ровно 4 места правки:
  `crates/journal/src/segments.rs` (несёт `ts_exch_ms`), `crates/sim/src/exchange.rs`
  (игнор — не входит в бэктест-fill), `crates/research-cli/src/bin/latency_probe.rs`
  (игнор — не latency-релевантно), `crates/recorder/src/lib.rs::md_kind_label` (метка
  `"margin_inventory"`); подтверждено коммитами `f2d1edb`/`ffedc10`/`6a2c331`/`67b6159`.
  Компилируемость всего workspace — гейт `cargo build --workspace` (по milestone-тексту).

## §5. Чем закреплено

- **Тип и SCHEMA_VERSION:** `crates/contracts/src/lib.rs:24-26` (комментарий-история версий,
  `pub const SCHEMA_VERSION: u32 = 4`), `crates/contracts/src/lib.rs:311-315` (вариант
  `MarginInventory`).
- **Sacred RED-тесты:** `crates/contracts/tests/ct_rfc05.rs` —
  `mi_i_1_margin_inventory_roundtrip` (postcard+serde_json roundtrip),
  `mi_i_1_pre_rfc05_funding_still_decodes` (старый вариант декодится байт-в-байт, анти-плацебо
  на позицию вставки), `mi_i_1_schema_version_is_4` (`SCHEMA_VERSION == 4`).
- **Парсинг/анти-плацебо на источнике:** `crates/venue-binance/tests/red_margin_inventory.rs`
  (упомянут в milestone как MI-I-2/MI-I-4 — fixed-point, множественность активов, отсутствие →
  fail-closed; файл не входит в scope этого RFC-документа, не читался построчно здесь).
- **JSON Schema:** `crates/contracts/schema/event.schema.json:350-368` — объект
  `MarginInventory` с обязательными `available_e8`/`ts_exch_ms`, оба `integer`/`int64`.
- **Фикстуры:** `crates/contracts/fixtures/valid/event-margin-inventory.json` (валидный
  `MarginInventory` с обоими полями), `crates/contracts/fixtures/invalid/
  event-margin-inventory-missing-ts.json` (тот же объект без `ts_exch_ms` — обязан быть
  отвергнут схемой).
- **CHANGELOG:** `crates/contracts/CHANGELOG.md`, секция «schema_version 3 → 4 — CT-RFC-05
  «MarginInventory» (2026-07-25)» — описывает bump, источник (`/sapi/v1/margin/
  available-inventory?type=MARGIN`), границу интерпретации (supply-пул, не ledger) и эпоху
  изоляции сегмента.
- **Milestone:** `milestones/M-35-margin-inventory.md` — STATUS: DONE, §Tasks/§Инварианты
  (MI-I-1..4)/§Анти-плацебо чек-лист/Allowed-Forbidden paths/Gates.
- **Plan-time/risk-гейты (для полноты governance-следа, вне зоны этого RFC-документа):**
  `research/critiques/C-024.md` (critic; первый проход REJECT — отсутствие CHANGELOG/фикстур и
  недостоверная ссылка на survey §9, зафиксировано в первичном коммите `239e796`; после
  правки `0999929` re-review PASS в `a174696`), `research/critiques/C-025.md` (risk-critic,
  PASS — read-only ключ, отсутствие order-egress, secrets только через env).

## §6. Источник данных и мотивация — по факту из истории

Мотивация зафиксирована в `milestones/M-35-margin-inventory.md` («Мотивация» раздел) и
`research/data-quality/margin-source-survey.md` §9: founder запросил margin-индикатор по
USDT/USDC (`milestones/M-35-margin-inventory.md`: «Founder-решение (2026-07-25): «маржин по
usdt/usdc собирать»»). Публичный (без auth) агрегированный borrow/repay ledger у Binance
недостижим — это установлено §1–§8 того же survey-файла (negative-result, до RFC). Re-проба с
read-only auth-ключом (§9 survey, 2026-07-25) показала, что достижим не ledger, а **сырой
market-wide supply-side пул, доступный к займу** (`available`) через
`/sapi/v1/margin/available-inventory?type=MARGIN`. Отсюда явная граница интерпретации,
перенесённая дословно в комментарий типа и CHANGELOG: `available_e8` — это ЁМКОСТЬ пула,
НЕ непогашенный объём и не borrow/repay ledger; утилизация/флоу (Δ available) — производная
downstream-метрика с caveat, не часть T1-формы. Critic (`C-024`, первый проход) заблокировал
именно недостоверную формулировку milestone («borrow/repay ledger», не подкреплённую
committed-фактами) — architect переформулировал как proxy-collector, после чего critic снял
REJECT.

## §7. Почему документа не было (честная реконструкция)

Изменение прошло весь ЗАЯВЛЕННЫЙ governance-путь `docs/05-contract-layer.md` §4/§Tasks
milestone'а — тип, схема, bump, CHANGELOG, фикстуры, тесты, critic (`C-024`), risk-critic
(`C-025`), reviewer close-out (`41d3526`) — но пропустило ровно один пункт списка §4: «(1)
изменение Rust-типа... В ОДНОМ PR» подразумевает и сам `docs/rfc/CT-RFC-NNN-*.md`-файл (по
образцу `CT-RFC-01..04`, каждый из которых материализован отдельным файлом в `docs/rfc/`).
CT-RFC-05 существует только распределённо — в комментариях типа, CHANGELOG и milestone'е, но
не как единый именованный RFC-документ.

Причина, установленная по факту, а не по догадке: **до этого инцидента не существовало
машинного гейта, который бы это проверял.** Ни один из выполненных чек-листов (critic C-024 —
doc-гейт по `docs/05-contract-layer.md` §4, but проверял CHANGELOG/фикстуры, не наличие
`docs/rfc/*.md`; risk-critic C-025 — safety-поверхность; reviewer close-out — §8 eyes-on) не
включал явную проверку «файл `docs/rfc/CT-RFC-05-*.md` существует и закоммичен». Правило §4
держалось СЛОВОМ («сердце governance»), не автоматической проверкой — человеческое
внимание пропустило один артефакт из семи требуемых, несмотря на то что остальные шесть были
выполнены качественно (rationale/границы семантики честно задокументированы в CHANGELOG).

Дыра обнаружена постфактум аудитом `docs/plans/contracts-current-state.md` (раздел «Д2 —
`CT-RFC-05` в коде, RFC-документа нет — RFC-дисциплина нарушена на реальном T1-бампе»,
классифицирована там как «governance (процедурная дыра — механизм существовал, но не был
исполнен на собственном примере)», не как «отсутствующая защита»/структурный пробел, в
отличие от соседней находки Д1) и независимо подтверждена критиком
`research/critiques/C-040-design-migration-plan.md:269-272`. Повторный аудит того же
`contracts-current-state.md` (нижняя сверочная таблица, строка «CT-RFC-05 без документа»)
зафиксировал: находка воспроизведена заново, состояние на момент повторной проверки не
изменилось.

Теперь гейт машинный: `scripts/verify_ct_rfc_atomic.sh` (введён коммитом `557be33`,
«verify_ct_rfc_atomic.sh — машинная атомарность изменения T1 (класс CT-RFC-05)», подключён к
CI коммитом `b3b42d2`) при правке `crates/contracts/src/**` требует В ТОМ ЖЕ диффе присутствия
всех шести артефактов §4 включая `docs/rfc/CT-RFC-NNN-*.md` — при отсутствии любого из них
падает `VERDICT: FAIL` с явным указанием на класс дефекта «CT-RFC-05». Этот документ закрывает
исторический разрыв, но НЕ переигрывает изменение: T1-форма, зафиксированная §1-§4 выше, уже
живёт на проде (122+ млн событий журнала на момент аудита `contracts-current-state.md`) и
задним числом не пересматривается.

## §8. Чего этот документ НЕ делает

- Не проектирует и не предлагает изменений T1 — CT-RFC-05 уже реализован и задеплоен
  (`ba61c62`, `41d3526`); это ретро-фиксация факта.
- Не восстанавливает никакую мотивацию, не зафиксированную в истории. Всё изложенное в §6 —
  дословная реконструкция из `milestones/M-35-margin-inventory.md` и
  `research/data-quality/margin-source-survey.md §9`; за пределами этих источников мотивация
  не устанавливалась.
- Не подтверждает и не опровергает существование reuse-теста конкретно на эпоху 3→4 (§4) —
  зафиксировано как открытый вопрос, не как факт в любую сторону.
- Не проверяет предметно `crates/venue-binance/tests/red_margin_inventory.rs` построчно —
  файл вне `crates/contracts/**`, вне зоны этого документа по инвокации.
