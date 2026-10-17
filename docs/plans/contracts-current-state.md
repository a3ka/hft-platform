# Контрактный слой — фактическое состояние (аудит заявленного против реального)

**Автор:** general-purpose аудитор (по заданию founder'а), 2026-07-31.
**База:** `origin/main` HEAD `8c1890e` (`docs(handoff): M-49 закрыт — итоговый контракт JR-I-8,
§8 проверен, новый долг TD-052/TD-053`) — включает смерженные M-40, M-41, M-49 (все три уже
в базе этого аудита; M-49 закрыт СЕГОДНЯ, rev5, TD-049 → CLOSED).
**Статус:** read-only аудит. Ничего не изменено вне этого файла.
**Предшественник:** `docs/plans/plan-design-migration.md` §1.3 (planner, 2026-07-31, база
`f930ece`) + `research/critiques/C-040-design-migration-plan.md` (critic, база `45a82bb`) —
оба перепроверены пофактно на текущем `8c1890e`; расхождения — §6.

---

## 1. Сводка

Заявленный контрактный слой (`docs/05-contract-layer.md`, `docs/fa/contracts.md`) описывает
7 T1-форм под единой RFC-дисциплиной с машинными гарантиями (CT-I-1..6) и Rust↔Python паритетом.
Фактически в `crates/contracts` живут только 2 из 7 заявленных форм (`Event`/`EventKind`, плюс
добавленные вне исходного списка `SegmentHeader`/`DataSource`/`LegacyManifest`/`LegacySegmentDecl`
через CT-RFC-02) — остальные 5 либо не существуют как типы вовсе (`SignalSpec`, `Ctl(ParamChange)`,
`Decision`), либо живут как обычные Rust-структуры в чужих крейтах без RFC, без JSON Schema, без
фикстур (`ValidationReport`, `TrialsLedger` entry, `SignalRegistry` entry). Заявленный гейт
`scripts/verify_contracts.sh` не существует; CI не проверяет контрактный слой отдельным джобом
вообще — защиту фактически несёт `cargo test --all`, который гоняет unit/интеграционные тесты
`crates/contracts/tests/*`, покрывающие ТОЛЬКО roundtrip/schema-parity для `Event`/`SegmentHeader`/
`LegacyManifest` и grep-канарейку для `Venue`/`MdPayload` (НЕ для `EventKind`, который документы
прямо называют примером канарейки). Есть готовая, уже приземлённая в код governance-дыра:
`CT-RFC-05` (`MarginInventory`, `SCHEMA_VERSION` 3→4) существует в 49 упоминаниях кода и в
`crates/contracts/CHANGELOG.md`, но `docs/rfc/CT-RFC-05-*.md` не существует — RFC прошёл БЕЗ
формального RFC-документа. Заявленный Python-консюмер (CT-I-5) — фикция: Python-кода в
репозитории нет вовсе (единственный `.py`-файл — `scripts/check_deploy_gate.py`, деплой-утилита,
не контрактный валидатор). Отдельно от всего заявленного T1-списка существует реальная,
активно эволюционирующая и НАМЕРЕННО выведенная из-под контрактной дисциплины граница —
`crates/gateway/src/lib.rs` (`GATEWAY_SCHEMA_VERSION = 8`, явно помечена комментарием
«T-designate, не T1») плюс вторая, независимая шкала версий (`research/exports/format.md`,
`export_schema_version: 1`). История версий gateway (v5→v8) документирует минимум один случай
смены СЕМАНТИКИ при неизменной ФОРМЕ (VWAP session-anchored → journal-cumulative, v5→v6) —
класс дефекта, который ни один заявленный CT-I-инвариант физически не может поймать, потому что
он проверяет форму, а не смысл. Внутри Rust контрактная зависимость реальна и работает
(16/16 не-`contracts` крейтов линкуют `contracts`, компилятор enforces форму T1-типов,
конструирование значений `EventKind`/`MdPayload` по всему коду идёт через публичные варианты
самого крейта `contracts` — обхода типов нет); зависимость исчезает ровно там, где кончается
Rust-компилятор: на границе Rust↔TS/браузер, Rust↔файл-архив (частично), Rust↔env/конфиг.

---

## 2. Таблица T1: заявлено → фактически → governance

Источник заявленного: `docs/05-contract-layer.md` §2, `docs/fa/contracts.md` §5 (идентичный
список из 7 форм в обоих документах).

| # | T1 по документу | Фактически | Governance |
|---|---|---|---|
| 1 | `Event` / `EventKind` | ✅ определены `crates/contracts/src/lib.rs:138` (`Event`), `:148` (`EventKind`) | Под RFC-дисциплиной (CT-RFC-01..05), JSON Schema (`schema/event.schema.json`), фикстуры `valid/`+`invalid/`. Canary CT-I-1 покрывает `Venue`/`MdPayload` (`ct_rfc01.rs:147`), **`EventKind` НЕ покрыт канарейкой** — grep `enum EventKind {` подтверждает единственное определение (`crates/contracts/src/lib.rs:148`, больше нигде), но это установлено вручную этим аудитом, не тестом |
| 2 | `SignalRegistry` entry | ❌ `crates/signals/src/registry.rs:19` — `pub struct RegistryEntry` (не в `contracts`) | Вне RFC. Файл-носитель `research/registry/signals.json` (граница B, объявлен SACRED в `.claude/rules/scope-guard.md`) **физически отсутствует** (`find research -iname "*registry*"` → пусто) |
| 3 | `Ctl(ParamChange)` | ❌ Не существует как тип. `crates/contracts/src/lib.rs:153` — комментарий-заглушка внутри `enum EventKind`: `// Ord(..), Risk(..), Recon(..), Ctl(..) — добавляются в P3 via contract-RFC.` | Прозой (комментарий о намерении, не тип) |
| 4 | `SignalSpec` card | ❌ Типа `SignalSpec` нет. Есть `SignalSpecRef` (`crates/signals/src/lib.rs:106`, другое имя, другая роль — ссылка, не карточка); сама карточка — markdown `research/specs/S-001-*.md` | Прозой (человекочитаемый markdown, не типизированная форма) |
| 5 | `ValidationReport` (`metrics.json`) | ❌ `crates/research-cli/src/types.rs:107` — `pub struct ValidationReport` (не в `contracts`) | Вне RFC. `TECH-DEBT.md` TD-008 (заведён M-04, статус OPEN на сегодня): типы держатся в `research-cli` со статусом «T1-designate», промоушен отложен «до первого кросс-языкового консюмера (Python)» — Python-тулинга нет, условие промоушена не наступило |
| 6 | `TrialsLedger` entry | ❌ `crates/research-cli/src/types.rs:34` — `pub struct TrialRecord` (не в `contracts`) | Тот же TD-008, вне RFC. Файл-носитель `research/trials-ledger.jsonl` (расширение `.jsonl`, документы `gates.md` §6 и `docs/03` §6 говорят `.json` — расхождение имени файла с доками, отдельно от типа) |
| 7 | `Decision` (`D-NNN`) | ❌ Тип не существует; каталог `research/decisions/` в репозитории отсутствует (`find research -iname "*decision*"` — пусто) | Прозой (намерение в доках, ноль реализации) |

**Дополнительно (фактически в `crates/contracts`, но НЕ в исходном списке §2 обоих документов —
добавлены через отдельный CT-RFC-02, список T1 в доках не обновлён под фактический состав):**
`SegmentHeader` (`:97`), `DataSource` (`:85`), `LegacySegmentDecl` (`:56`), `LegacyManifest`
(`:68`) — все под полноценной RFC-дисциплиной (CT-RFC-02, `docs/rfc/CT-RFC-02-journal-provenance.md`
существует), JSON Schema (`schema/segment-header.schema.json`, `schema/legacy-manifest.schema.json`),
фикстуры есть.

**Governance-дыра, отдельная от списка T1 §2 (найдена этим аудитом и предшественником):**
`CT-RFC-05` («MarginInventory», `SCHEMA_VERSION` 3→4) — тип `MdPayload::MarginInventory`
реально аддитивно добавлен в `crates/contracts/src/lib.rs:311-314` (8-й вариант enum, дискриминант
7), покрыт sacred-тестом `crates/contracts/tests/ct_rfc05.rs`, задокументирован в
`crates/contracts/CHANGELOG.md` («## schema_version 3 → 4 — CT-RFC-05 «MarginInventory» (2026-07-25)»,
с полным rationale/источником/границей семантики) — но **`docs/rfc/CT-RFC-05-*.md` не существует**:

```
$ ls docs/rfc/
CT-RFC-01-market-data-expansion.md
CT-RFC-02-journal-provenance.md
CT-RFC-03-recon-audit.md
CT-RFC-04-l2delta.md
$ grep -rln "CT-RFC-05" --include=*.rs --include=*.md . | wc -l
49
```

`docs/05-contract-layer.md` §4 требует ОДИН PR из 7 пунктов, включая явный RFC-документ; из
семи CT-RFC-05 фактически покрыл (1) правку типа, (2) схему, (3) bump версии, (5) фикстуры,
(7) тест — но НЕ отдельный RFC-документ с миграционной заметкой как самостоятельный артефакт
(содержание есть, но в CHANGELOG, не в `docs/rfc/`). Это правка T1, прошедшая формально мимо
процедурного требования того же документа, который эту процедуру описывает.

---

## 3. Таблица «формы, пересекающие границу» (gateway wire + экспорт)

```
$ grep -n "^pub struct\|^pub enum" crates/gateway/src/lib.rs
109:pub struct Selector
148:pub struct Cursor
176:pub struct OhlcvRow
190:pub struct VolumeProfileRow
205:pub struct HeatmapCell
216:pub struct CobLevel
225:pub struct BubbleCell
234:pub struct DepthRow
248:pub struct SeriesBundle
308:pub struct Snapshot
334:pub struct Frame
```//плюс `ReadStats`, `LiveReducer` — внутренние, не пересекают границу наружу

| Форма | Куда идёт | Версионируется? | Чем защищена |
|---|---|---|---|
| `Selector`, `Cursor`, `OhlcvRow`, `VolumeProfileRow`, `HeatmapCell`, `CobLevel`, `BubbleCell`, `DepthRow`, `SeriesBundle`, `Snapshot`, `Frame` (`crates/gateway/src/lib.rs`) | `crates/gateway-serve` → браузер (TS, `code2alpha`, вне дерева) | Да — `GATEWAY_SCHEMA_VERSION: u32 = 8` (`crates/gateway/src/lib.rs:65`), одна константа на ВСЕ 11 форм сразу | Только Rust-компилятор + ручное чтение комментария на бампе. Нет JSON Schema, нет фикстур `valid/invalid` под эти типы, нет contract-RFC, нет TS-стороны паритета. Явная пометка в коде: «T-designate (не T1, не `crates/contracts`)» (комментарий `crates/gateway/src/lib.rs:65` и соседние) |
| `ServeMsg{Snapshot, Frame, Error}` (`crates/gateway-serve/src/lib.rs:63`, `mod wire`) | Верхний конверт WS-протокола → браузер | Нет отдельной версии — версия наследуется от вложенных `Snapshot`/`Frame` | Ничем отдельным; JSON-only на сегодня, комментарий предупреждает про будущий бинарный кодек (heatmap) без указания, как версионировать конверт |
| `Claims{sub: String, exp: usize}` (`crates/gateway-serve/src/lib.rs:18`, `mod auth`) | Auth.js/Next.js (TS) выпускает JWT → Rust проверяет подпись и декодирует | Нет версии вовсе | 2 поля; код явно НЕ доверяет остальным claims от Next.js («мы НЕ доверяем claim-метаданным… только самой подписи» — комментарий рядом со строкой 33) — сегодня это осознанное сужение доверия, но означает, что claims как форма для тарифа/квот НЕ существует |
| `research/exports/format.md` (`export_schema_version: 1`, `research-cli::EXPORT_SCHEMA_VERSION` — `crates/research-cli/src/export_io.rs:37`) | Файл `<out_dir>/<venue>/<symbol>.json` → `code2alpha` (репозиторий вне дерева) | Да, отдельная целочисленная константа, **независимая от `GATEWAY_SCHEMA_VERSION`** | Markdown-документация формата (`research/exports/format.md`), owner явно назван `research-dev`/`architect`, но нет JSON Schema, нет фикстур, нет RFC-дисциплины — тот же класс защиты, что gateway (ноль) |
| `GATEWAY_VENUE/SYMBOL/TIMEFRAME_MS/BANDS/WINDOW_MS` (env, `docker-compose.yml`) | Конфигурирует ОДИН процесс gateway-serve; клиент не видит и не может проверить, что именно отдаётся | Не версионируется — это не форма данных, а конфигурация запуска | `unwrap_or` на парсинге (см. `docs/08-arch-improvement-roadmap.md` R9/R10 упомянутый в связке; конкретно R7 — `GATEWAY_WINDOW_MS` parse-error → unbounded, отдельно зафиксирован в доке рисков) |

**История версий gateway — смена смысла при неизменной форме (проверено дословным чтением
комментариев `crates/gateway/src/lib.rs:37-65`):**

```
5: M-23 Heatmap+COB+Bubbles — аддитивные новые типы. Бамп 4→5.
6: M-36 — VWAP: форма Vec<(i64,i64)> НЕИЗМЕННА, но СЕМАНТИКА пересмотрена
   (session-anchored → journal-cumulative). Бамп 5→6.
7: M-38a (TD-043) — cvd_session_base: i64 → Vec<(session_id, base)> — non-additive
   (и форма, и семантика). Бамп 6→7.
8: M-48 (TD-048) — history_start_seq/history_truncated добавлены аддитивно
   (#[serde(default)]); v7-консюмер читает v8 как «полную историю» по дефолту.
   Бамп 7→8.
```

v5→v6 — самый опасный случай: ни один заявленный CT-I-инвариант (и ни один из кандидатов
CT-I-7..15, предложенных `plan-design-migration.md`, кроме специально спроектированного
golden-фикстур-инварианта CT-I-15) не ловит смену интерпретации при байт-идентичной форме —
консюмер v5 получает валидный по схеме и неверный по смыслу ответ молча.

---

## 4. Найденные дыры (по убыванию опасности)

### Д1 — Смена СЕМАНТИКИ при неизменной ФОРМЕ проходит полностью незамечено (v5→v6 VWAP)

**Факт:** `crates/gateway/src/lib.rs:47-52` (комментарий на бампе 5→6): форма `vwap:
Vec<(i64,i64)>` не изменилась ни на бит, но интерпретация значений изменилась (session-anchored
→ all-time journal-cumulative). Никакой schema-валидатор, включая гипотетический полноценный
`verify_contracts.sh`, структурно не может поймать этот класс — валидатор проверяет форму.

**Последствие:** консюмер, который не прочитал комментарий к константе версии (единственное
место, где это зафиксировано), получает синтаксически корректные и семантически неверные данные
без ошибки, без предупреждения, без деградации какого-либо иного сигнала.

**Класс:** отсутствующая защита (структурный пробел класса инвариантов, не просто «не собрались
написать тест»).

### Д2 — `CT-RFC-05` в коде, RFC-документа нет — RFC-дисциплина нарушена на реальном T1-бампе

**Факт:** `docs/rfc/` содержит только `CT-RFC-01..04-*.md` (`ls docs/rfc/`, выше); `CT-RFC-05`
упоминается 49 раз в коде/докс, включая sacred-тест `crates/contracts/tests/ct_rfc05.rs` и
реальный бамп `SCHEMA_VERSION` 3→4 на живом проде (122+ млн событий журнала).

**Последствие:** правка T1-формы (аддитивный вариант enum, bump версии эпохи сегмента) прошла в
прод без формального RFC-артефакта, требуемого тем же документом (`docs/05-contract-layer.md`
§4), который декларирует «атомарный contract-RFC = сердце governance». Governance-механизм
де-факто не применялся к реальному изменению T1, хотя правка содержательно качественная
(rationale/границы семантики честно задокументированы — но в CHANGELOG, не как отдельный
RFC-документ).

**Класс:** governance (процедурная дыра — механизм существовал, но не был исполнен на
собственном примере).

### Д3 — `scripts/verify_contracts.sh` не существует; контрактный слой не проверяется отдельным CI-джобом

**Факт:**
```
$ ls scripts/verify_contracts.sh
ls: cannot access 'scripts/verify_contracts.sh': No such file or directory
$ grep -n "contract\|verify" .github/workflows/ci.yml
      - name: fmt ... clippy ... test (RED/GREEN — наши инварианты): cargo test --all
```
Обещан в `docs/05-contract-layer.md:79` («Гейт паритета: `verify_contracts.sh`») и
`docs/fa/contracts.md:83,102,113` (§8/§T/§P).

**Последствие:** нет единой проверяемой точки «контрактный слой цел». Защиту сегодня несёт
только `cargo test --all`, разбросанный по `crates/contracts/tests/{ct_rfc01,ct_rfc05,
red_rfc02,red_rfc03,red_rfc04,red_schema}.rs` — она РЕАЛЬНО работает для того, что покрывает
(roundtrip, схема↔типы, valid/invalid фикстуры), но нет агрегатора с явным `VERDICT: PASS/FAIL`,
и нет отдельного упоминания в `ci.yml`, которое бы падало ИМЕННО из-за контрактного дрейфа
(падение спрятано внутри общего `cargo test --all`).

**Класс:** расхождение доков с кодом + отсутствующая защита (агрегация).

### Д4 — CT-I-1 канарейка не покрывает `EventKind`, хотя документы называют его примером

**Факт:** `crates/contracts/tests/ct_rfc01.rs:147-173`, функция
`ct_i_1_single_definition_canary` — needle-список: `venue_needle = "enum Venue {"`,
`payload_needle = "enum MdPayload {"`. `EventKind` в теле теста не упомянут вообще
(`grep -n "EventKind" crates/contracts/tests/ct_rfc01.rs` — 0 совпадений). При этом
`docs/05-contract-layer.md` §4 прямо приводит пример: «grep-канарейка: `EventKind` определён
ровно в одном месте».

**Проверено вручную (не тестом):** `grep -rn "enum EventKind" --include=*.rs .` — ровно одно
совпадение, `crates/contracts/src/lib.rs:148`. Т.е. инвариант СЕГОДНЯ фактически держится, но
БЕЗ машинной защиты — регрессия (кто-то переопределит `EventKind` локально в другом крейте)
не будет поймана автоматически, только ручным ревью.

**Класс:** расхождение доков с кодом (заявлено ⟹ не проверено — тот же класс дефекта, что уже
дважды ловился на этом проекте, см. `INTG-I`/`BK-I`/`CT-I-5`).

### Д5 — CT-I-5 («Python-тулинг валидирует против той же схемы») — фикция; Python-кода нет

**Факт:**
```
$ find . -iname "*.py" -not -path "./.git/*"
./scripts/check_deploy_gate.py
```
Единственный Python-файл в репозитории — деплой-утилита, не контрактный валидатор. Заявлено
`docs/05-contract-layer.md` §5/§6 (`CT-I-5`) и `docs/fa/contracts.md` §8/§I.

**Последствие:** «кросс-языковой паритет» между Rust-каноном и заявленным Python-консюмером не
существует ни в каком виде — ни кода, ни теста, ни фикстур на стороне Python.

**Класс:** расхождение доков с кодом (заявленный инвариант не имеет референсного консюмера
вообще, не только оракула).

### Д6 — `SignalRegistry`/`Decision`/`SignalSpec`/`Ctl(ParamChange)` объявлены SACRED/T1 и физически отсутствуют

**Факт:** `research/registry/signals.json` (граница B, объявлена SACRED в
`.claude/rules/scope-guard.md`) — `find research -iname "*registry*"` → пусто. `research/
decisions/` (`Decision`/`D-NNN`) — `find research -iname "*decision*"` → пусто (единственный
результат — каталог `research/exports` не совпадает по имени). `Ctl(ParamChange)` — только
комментарий-заглушка в `contracts/src/lib.rs:153`.

**Последствие:** три из семи заявленных T1-форм не существуют НИ В КАКОМ виде (ни как тип, ни
как файл-носитель) — не «недоделано», а полностью отсутствует материализация. Это не обязательно
плохо само по себе (эти формы относятся к фазам, которые ещё не начались — `risk`/`killswitch`/
`oms` крейтов тоже нет), но список T1 §2 документов не делает различия между «уже реализовано
под RFC», «реализовано вне RFC» и «не существует вовсе» — все семь строк таблицы выглядят
одинаково авторитетно.

**Класс:** расхождение доков с кодом.

### Д7 — Вторая, независимая шкала версий экспорта (`export_schema_version`) параллельно `GATEWAY_SCHEMA_VERSION`

**Факт:** `research/exports/format.md` — `export_schema_version: 1`, Owner `research-dev`,
консюмер `code2alpha`; `crates/research-cli/src/export_io.rs:37` — `pub const
EXPORT_SCHEMA_VERSION: u32 = 1`. Одновременно `crates/gateway/src/lib.rs:65` —
`GATEWAY_SCHEMA_VERSION: u32 = 8`. Комментарий в gateway явно признаёт связь: «Версия
экспорт-формы gateway. **Аддитивно** поверх `research-cli::EXPORT_SCHEMA_VERSION = 1`».

**Последствие:** один и тот же логический потребитель (`code2alpha`) читает данные, версионируемые
ДВУМЯ независимыми, неравными числами (1 и 8) без единого источника истины, что из них
«текущее». Признано и самими доками как проблема (комментарий «аддитивно поверх»), но не решено.

**Класс:** расхождение доков с кодом / отсутствующая унификация.

### Д8 — `TrialsLedger` — расхождение расширения файла с документами

**Факт:** `research/trials-ledger.jsonl` существует на диске; `.claude/rules/gates.md` §6 и
`docs/03-integration-contract.md` §6 называют файл `research/trials-ledger.json` (без `l`).

**Последствие:** незначительно само по себе, но симптоматично для класса «документ описывает
форму, которую никто не сверяет с фактическим артефактом».

**Класс:** расхождение доков с кодом (минорное).

---

## 5. Кандидаты в T1

Ниже — формы, которые ПО ФАКТУ пересекают границу процессов/языков/времени/платящего клиента
(критерий пересечения, не решение о промоушене — оно вне зоны этого аудита):

1. **Wire-формы кокпита** (`Selector`, `Cursor`, `OhlcvRow`, `VolumeProfileRow`, `HeatmapCell`,
   `CobLevel`, `BubbleCell`, `DepthRow`, `SeriesBundle`, `Snapshot`, `Frame` — `crates/gateway/
   src/lib.rs`) + конверт `ServeMsg` (`crates/gateway-serve/src/lib.rs:63`). Пересекают
   Rust↔TS/браузер границу СЕГОДНЯ (уже потребляются `code2alpha`), уже дали минимум один
   зафиксированный случай смены семантики при неизменной форме без RFC (Д1) и минимум один
   non-additive бамп формы (`cvd_session_base`, v6→v7) без RFC.
2. **`export_schema_version`-контракт** (`research/exports/format.md`,
   `research-cli::EXPORT_SCHEMA_VERSION`) — та же граница Rust↔внешний потребитель, независимая
   вторая шкала версий, накладывающаяся на (1) (Д7).
3. **`Claims{sub, exp}`** (`crates/gateway-serve/src/lib.rs:18`) — форма на границе TS
   (Auth.js/Next.js выпускает) ↔ Rust (проверяет подпись). Сегодня всего 2 поля и осознанно
   ограниченное доверие, но это единственная сегодня существующая форма на security-границе
   между языками.
4. **`GATEWAY_VENUE/SYMBOL/TIMEFRAME_MS/BANDS/WINDOW_MS`** (env, `docker-compose.yml`) — не
   типизированы вовсе; определяют, что именно отдаётся клиенту, парсятся с `unwrap_or`
   (см. Д3-соседний риск R7 в `docs/08-arch-improvement-roadmap.md`).
5. **`ValidationReport`/`TrialRecord`** (`crates/research-cli/src/types.rs:107,34`) — уже
   заявлены T1 в доках (Раздел 2, строки 5-6), уже несут `report_schema_version`; расхождение
   доков с кодом длится с M-04 (TD-008, OPEN).
6. **`SignalRegistry` entry / `research/registry/signals.json`** — объявлена SACRED в
   `scope-guard.md` (граница B), физически отсутствует; кандидат по факту объявления, не по
   факту существования.

Все шесть пунктов пересекают ХОТЯ БЫ одну из границ: язык (Rust↔TS), процесс (движок↔founder-
фронт), время (журнал/архив, бессмертие данных) — критерий, который сам документ
`docs/05-contract-layer.md` §1 формулирует как причину существования контрактного слоя
(«Движок и квант-деск — разные исполнители, разные языки, разные жизненные циклы»), но список
§2 не включает ни одного из них.

---

## 6. Что перепроверено против `plan-design-migration.md` §1.3 / `C-040` и изменилось

`plan-design-migration.md` §1.3 писался против `origin/main @ f930ece`; `C-040`
(critic-вердикт на этот план) аудировал против `origin/main @ 45a82bb`. Этот аудит — против
`origin/main @ 8c1890e`, на 1+ коммит дальше `45a82bb` (плюс мерж M-49 rev5 сегодня).

| Пункт | Было в §1.3 / C-040 | Перепроверено сейчас | Изменилось? |
|---|---|---|---|
| TD-049 / M-49 | §1.3 план не касался; C-040 (Ф1): TD-049 переоценён MINOR→CRITICAL, M-49 rev4 REJECTED, «прекондиция Б2.5 НЕ ВЫПОЛНЕНА» | `git log` показывает merge `091ece1` (rev5, APPROVED R-003) → `a1d9cac` → `8c1890e`: **TD-049/050/051 → ✅ CLOSED 2026-07-31**, TD-052/053 заведены как NOTE (не блокеры) | ✅ ИЗМЕНИЛОСЬ — прекондиция Б2.5, которую C-040 требовал сделать жёсткой, теперь физически снята (не просто помечена) |
| `docs/contract-rfc/` дублирующийся путь | §1.3: «Дублирующийся исторический путь RFC: `docs/contract-rfc/CT-RFC-01` и `docs/rfc/CT-RFC-01`» | `find . -iname "contract-rfc" -not -path "./.git/*"` → пусто; только `docs/rfc/` существует | ✅ ИЗМЕНИЛОСЬ (дубликат более не существует — либо был удалён отдельно, либо факт §1.3 к текущему моменту устарел) |
| CT-RFC-05 без документа | §1.3 и C-040 оба фиксируют факт | `ls docs/rfc/` — по-прежнему только 01-04 | Без изменений — дыра Д2 подтверждена заново |
| `scripts/verify_contracts.sh` | §1.3 и C-040: не существует | `ls scripts/verify_*.sh` — по-прежнему нет `verify_contracts.sh` (список из 26 milestone-скриптов проверен целиком) | Без изменений |
| CT-I-1 канарейка покрывает только Venue/MdPayload | §1.3 и C-040: подтверждено, EventKind не покрыт | Подтверждено заново дословным чтением `ct_rfc01.rs:145-173` | Без изменений |
| T1-формы вне `contracts` (5 из 7) | §1.3: перечислены поимённо | Все пять подтверждены заново на тех же путях/строках | Без изменений (расхождение доков с кодом устойчиво) |
| `GATEWAY_SCHEMA_VERSION` / история версий v5-v8 | §1.3: подтверждено | Подтверждено дословным цитированием того же участка комментария (`crates/gateway/src/lib.rs:37-65`) | Без изменений |
| Крейты `risk`/`killswitch`/`oms` | §1.3/C-040: отсутствуют, список 17 (16 без `contracts`) крейтов | `ls crates/` — тот же список из 17 крейтов | Без изменений |
| `research/registry/`, `research/decisions/` | §1.3/C-040: физически отсутствуют | Подтверждено заново | Без изменений |
| Python-тулинг / CT-I-5 | §1.3/C-040 цитируют `docs/08` R9 | Подтверждено напрямую: единственный `.py`-файл в репо — `scripts/check_deploy_gate.py` | Без изменений |
| `plan-design-migration.md` сам | C-040 вынес REJECT (§5.2 не показывает risk-critic на 4 из 5 шагов Блока 2, трогающих `crates/contracts/**`) | Не перепроверялось повторно этим аудитом (не входит в ТЗ — этот аудит про состояние кода/доков, не про качество плана); зафиксировано как контекст | N/A — вне объёма |

**Вывод раздела:** единственное содержательное изменение состояния между §1.3/C-040 и этим
аудитом — закрытие TD-049 (M-49 rev5 merged), которое снимает единственную известную жёсткую
прекондицию для будущего шага Б2.5 (`SCHEMA_VERSION` → 5, `source_kind`/`license_class`). Все
остальные факты контрактного слоя, зафиксированные ранее, подтверждены без изменений — расхождение
между заявленным и фактическим контрактным слоем на сегодня СТАБИЛЬНО, не является случайным
снимком момента.
