# R-006 — PR-ревью ветки `feat/contract-gates` (машинные гейты контрактного слоя)

- **Дата (UTC):** 2026-08-01
- **Ветка:** `feat/contract-gates`, HEAD `b3b42d2`
- **База:** `origin/main` (ветка отстаёт на 9 процессных `docs/*` коммитов — отставание базы, не правки агента)
- **Автор изменений:** architect
- **Ревьюер:** reviewer

## ВЕРДИКТ: **CHANGES REQUESTED** (не мержу)

Работа сильная и закрывает реальные дыры аудита (`docs/plans/contracts-current-state.md`).
Все три гейта проверены мной мутационно и **умеют краснеть на своём основном классе**.
Блокирую по двум **fail-open** путям: в двух гейтах из трёх есть вход, на котором гейт
выдаёт **молчаливый PASS вместо FAIL** — ровно тот класс, что назван в `.claude/rules/testing.md`
(«гейт, который прошёл, потому что нечего было проверить, хуже отсутствующего») и в
инциденте M-40 (POSIX-awk, ложный PASS).

Оба дефекта дёшевы в устранении и НЕ требуют пересмотра дизайна. Фикс не проектирую
(`gates.md` §4, граница reviewer↔architect) — только описываю дефект и воспроизведение.

---

## Block-scope — ✅ PASS

```
$ git diff --name-only origin/main...HEAD
.github/workflows/ci.yml
scripts/contracts_validate_fixtures.py
scripts/diff_contract_schema.py
scripts/diff_contract_schema.sh
scripts/tests/red_ct_rfc_atomic.sh
scripts/verify_contracts.sh
scripts/verify_ct_rfc_atomic.sh
```

- `crates/contracts/**` **не тронут ни одним байтом** — гейты построены ВОКРУГ T1, ничего
  внутри не меняя. **Block-C неприменим**, авто-REJECT по `docs/05` §4 не срабатывает.
- `crates/risk/`, `crates/killswitch/`, `crates/oms/`, `crates/venue-*/` не тронуты →
  **RISK-BLOCK (`gates.md` §5) неприменим**, risk-critic не требуется.
- Коммиты атомарны, 1 задача = 1 коммит, conventional, со ссылкой на дыру аудита:

```
$ git log origin/main..HEAD --format='%h %an %s'
b3b42d2 architect ci(contracts): подключить verify_contracts.sh + verify_ct_rfc_atomic.sh + diff_contract_schema.sh отдельным джобом
7428cc3 architect feat(contracts): diff_contract_schema.sh — классификатор additive/breaking + связь с SCHEMA_VERSION
557be33 architect feat(contracts): verify_ct_rfc_atomic.sh — машинная атомарность изменения T1 (класс CT-RFC-05)
eae13b9 architect feat(contracts): verify_contracts.sh — гейт паритета Rust↔Schema↔фикстуры (обещан docs/05 §5)
```

---

## Block-DoneBlock — ✅ PASS (перепроверено мной, не принято на слово)

### Базовый прогон `verify_contracts.sh`

```
$ bash scripts/verify_contracts.sh; echo exit=$?
PASS  S0 setup-guard (генератор+схемы+фикстуры+python-jsonschema+cargo)
PASS  S1 схема ↔ типы (regen == committed, CT-I-4)
PASS  S2 фикстуры ↔ схема (valid PASS / invalid REJECT)
PASS  S3 CT-I-1 канарейка EventKind (единственная дефиниция в contracts)
PASS  S4 cargo test -p contracts (roundtrip + RFC RED-suite GREEN)

VERDICT: PASS
exit=0
```

Trap-восстановление схемы после S1 работает — дерево чистое после прогона:

```
$ git status --porcelain
{пусто}
```

### Авторский self-test `red_ct_rfc_atomic.sh`

```
$ bash scripts/tests/red_ct_rfc_atomic.sh; echo exit=$?
PASS  P0 без правки crates/contracts/src — тривиальный PASS
PASS  P1 полный пакет (rfc+schema+changelog+valid+invalid+test) — PASS
PASS  BAD без 'rfc' — гейт ОБЯЗАН FAIL (анти-плацебо)
PASS  BAD без 'schema' — гейт ОБЯЗАН FAIL (анти-плацебо)
PASS  BAD без 'changelog' — гейт ОБЯЗАН FAIL (анти-плацебо)
PASS  BAD без 'valid' — гейт ОБЯЗАН FAIL (анти-плацебо)
PASS  BAD без 'invalid' — гейт ОБЯЗАН FAIL (анти-плацебо)
PASS  BAD без 'test' — гейт ОБЯЗАН FAIL (анти-плацебо)
PASS  setup-guard: несуществующий base-ref — FAIL (fail-closed)

VERDICT: PASS
exit=0
```

Self-test написан тем же агентом, что и гейт, поэтому он **не засчитан как верификация** —
ниже мои независимые мутации.

---

## Мои мутационные проверки (задача 1 мандата — анти-плацебо двух непроверенных гейтов)

### `verify_ct_rfc_atomic.sh` — ✅ ловит свой класс

**M1 — правка `crates/contracts/src/lib.rs` БЕЗ единого артефакта RFC-пакета:**

```
$ printf '\n// mutation: новое поле T1\n' >> crates/contracts/src/lib.rs
$ git commit -qm "mut: T1 change without RFC package"
$ bash scripts/verify_ct_rfc_atomic.sh b3b42d2; echo exit=$?
PASS  crates/contracts/src/** тронут — проверяю атомарность CT-RFC пакета (docs/05 §4)
FAIL  RFC-документ (docs/rfc/CT-RFC-NNN-*.md) — нет docs/rfc/CT-RFC-NNN-*.md в дифе ...
FAIL  регенерированная схема (crates/contracts/schema/*.json) — схема не в дифе ...
FAIL  CHANGELOG (crates/contracts/CHANGELOG.md) — нет записи в CHANGELOG ...
FAIL  valid-фикстура (crates/contracts/fixtures/valid/*.json) — нет valid-фикстуры ...
FAIL  invalid-фикстура (crates/contracts/fixtures/invalid/*.json) — нет invalid-фикстуры ...
FAIL  тест (crates/contracts/tests/*.rs) — нет теста, ссылающегося на новую форму ...

VERDICT: FAIL (6 недостающих артефакта(ов) атомарного CT-RFC пакета)
exit=1
```

**M2 — тот же диф + все 6 артефактов:**

```
$ bash scripts/verify_ct_rfc_atomic.sh b3b42d2; echo exit=$?
PASS  crates/contracts/src/** тронут — проверяю атомарность CT-RFC пакета (docs/05 §4)
PASS  RFC-документ (docs/rfc/CT-RFC-NNN-*.md)
PASS  регенерированная схема (crates/contracts/schema/*.json)
PASS  CHANGELOG (crates/contracts/CHANGELOG.md)
PASS  valid-фикстура (crates/contracts/fixtures/valid/*.json)
PASS  invalid-фикстура (crates/contracts/fixtures/invalid/*.json)
PASS  тест (crates/contracts/tests/*.rs)

VERDICT: PASS
exit=0
```

Обе стороны подтверждены: гейт различает good/bad, не «всегда PASS» и не «всегда FAIL».

### `diff_contract_schema.sh` — ✅ ловит свой класс (у гейта НЕТ авторского self-test, см. F-3)

| Кейс | Мутация | Ожидание | Факт |
|---|---|---|---|
| D1 | схема не менялась | PASS/none | `CLASS=none`, `VERDICT: PASS`, exit=0 |
| D2 | новое ОПЦИОНАЛЬНОЕ свойство, без бампа | PASS/additive | `CLASS=additive`, `VERDICT: PASS`, exit=0 |
| D3 | удалён вариант `MdPayload::MarginInventory`, без бампа | FAIL | `CLASS=breaking`, exit=1 |
| D4 | тот же BREAKING + `SCHEMA_VERSION` 4→5 | PASS | exit=0 |
| W2 | новое ОБЯЗАТЕЛЬНОЕ свойство | FAIL | `CLASS=breaking`, exit=1 |
| W3 | смена `type` `string`→`integer` | FAIL | `CLASS=breaking`, exit=1 |
| D5 | несуществующий base-ref | FAIL | setup-guard, exit=1 |
| D6 | классификатор `.py` удалён | FAIL | setup-guard, exit=1 |

Сырой вывод ключевой пары D3/D4 (связь «breaking ⇒ обязателен бамп» — заявленное усиление
против einhard):

```
=== D3: BREAKING (удалён вариант enum) БЕЗ бампа SCHEMA_VERSION ===
BREAKING definitions.MdPayload.oneOf — вариант 'MarginInventory' удалён
CLASS=breaking
диф классифицирован BREAKING — проверяю bump SCHEMA_VERSION (обязателен, усиление против einhard)
FAIL  BREAKING диф БЕЗ bump'а SCHEMA_VERSION: base=4 head=4 (обязано head > base)
VERDICT: FAIL
exit=1

=== D4: тот же BREAKING + бамп SCHEMA_VERSION 4→5 ===
BREAKING definitions.MdPayload.oneOf — вариант 'MarginInventory' удалён
CLASS=breaking
PASS  BREAKING диф корректно сопровождён bump'ом SCHEMA_VERSION: 4 → 5
VERDICT: PASS
exit=0
```

Заявленное усиление против einhard **реально работает**.

---

## Block-CT-RFC-05 (задача 3 мандата: видит ли гейт историческую дыру?) — ✅ ДА, и это лучшее доказательство не-заглушки

Дыра подтверждена фактически: `docs/rfc/` содержит `CT-RFC-01..04`, файла `CT-RFC-05-*.md`
**не существует**, при этом `CT-RFC-05` упомянут 63 раза в коде/доках.

Прогнал гейт **ретроспективно на РЕАЛЬНОМ коммите** `e06e48a` («feat(M-35): CT-RFC-05 —
MdPayload::MarginInventory, schema→4») против его родителя:

```
$ git diff --name-only e06e48a^ e06e48a
crates/contracts/schema/event.schema.json
crates/contracts/src/lib.rs
crates/contracts/tests/ct_rfc05.rs
crates/venue-binance/tests/red_margin_inventory.rs
milestones/M-35-margin-inventory.md
scripts/verify_M-35.sh

$ bash scripts/verify_ct_rfc_atomic.sh e06e48a^; echo exit=$?
PASS  crates/contracts/src/** тронут — проверяю атомарность CT-RFC пакета (docs/05 §4)
FAIL  RFC-документ (docs/rfc/CT-RFC-NNN-*.md) — нет docs/rfc/CT-RFC-NNN-*.md в дифе ...
PASS  регенерированная схема (crates/contracts/schema/*.json)
FAIL  CHANGELOG (crates/contracts/CHANGELOG.md) — нет записи в CHANGELOG ...
FAIL  valid-фикстура (crates/contracts/fixtures/valid/*.json) — нет valid-фикстуры ...
FAIL  invalid-фикстура (crates/contracts/fixtures/invalid/*.json) — нет invalid-фикстуры ...
PASS  тест (crates/contracts/tests/*.rs)

VERDICT: FAIL (4 недостающих артефакта(ов) атомарного CT-RFC пакета)
exit=1
```

**Гейт независимо воспроизвёл находку аудита на реальных исторических данных** — 4 из 6
артефактов отсутствовали. Это сильнее любого синтетического self-test.

**Не маскирует ли решение дыру?** Нет. База берётся ИЗ СОБЫТИЯ
(`.github/workflows/ci.yml`: `push` → `github.event.before`, `pull_request` →
`pull_request.base.sha`), поэтому история задним числом не переоценивается и CI не падает
на прошлом — это **честный инкрементальный барьер**, не исключение и не whitelist для
`CT-RFC-05`. Никакого специального обхода для CT-RFC-05 в коде нет (проверено). Governance-дыра
при этом **остаётся открытой** и должна быть заведена в `TECH-DEBT.md` при мерже (ретро-документ
`CT-RFC-05` — зона architect).

Отдельно отмечу качество fail-closed обвязки базы в CI: zero-SHA (создание ветки/force-push)
→ `exit 1`, а не молчаливый пропуск. Это правильно.

---

## Block-CI — ✅ PASS

- Новый джоб `contracts` **изолирован**: не меняет `build-test`/`security`/`delivery`/
  `protected-artifacts`, только добавляется в `needs` агрегатора `status-check` и в его
  условие — то есть новый гейт реально блокирует, а не висит декорацией.
- `fetch-depth: 0` присутствует — без него `git merge-base` и `git show <base>` не работают.
- Прогон непропорционально не удлиняется: `Swatinem/rust-cache` + сборка только крейта
  `contracts` (`cargo run -p contracts --example gen_schema`, `cargo test -p contracts`).
- Анти-плацебо-проба барьера (`red_ct_rfc_atomic.sh`) включена в тот же джоб — правильный
  паттерн, зеркалит `protected-artifacts`.

---

## Находки

### F-1 (HIGH, блокирующая) — `diff_contract_schema.sh`: BREAKING-изменение `$ref` классифицируется как `none` и печатает ФАКТИЧЕСКИ ЛОЖНОЕ «схема не изменилась»

**Файл:** `scripts/diff_contract_schema.py:56-129` (`classify_node`), проявление —
`scripts/diff_contract_schema.sh:88-94` (ветка `none`).

`classify_node` сравнивает только `properties`/`required`/`enum`/`type`/
`additionalProperties`/`oneOf`. Свойство, представленное **исключительно** как `{"$ref": ...}`,
не имеет ни одного из этих ключей, поэтому **перенацеливание `$ref` на другой существующий
тип не даёт ни одного `Change`** → `CLASS=none`.

Воспроизведение (перецелил `MdEvent.payload` с `#/definitions/MdPayload` на существующий
`#/definitions/SysEvent` — семантически ломающее изменение T1):

```
$ python3 -c "... d['definitions']['MdEvent']['properties']['payload']={'\$ref':'#/definitions/SysEvent'} ..."
$ git commit -qm "mut: ref-retarget payload MdPayload->SysEvent"
$ bash scripts/diff_contract_schema.sh b3b42d2; echo exit=$?
CLASS=none
PASS  схема crates/contracts/schema не изменилась между b3b42d2 и HEAD
VERDICT: PASS
exit=0
```

Два отдельных дефекта в одном:

1. **Fail-open:** ломающее изменение проходит гейт, требование бампа `SCHEMA_VERSION` не
   срабатывает → риск для `CT-I-3` (журнал бессмертен, старый журнал обязан читаться) и
   `docs/05` §4 («ломающее → major bump + миграция»).
2. **Ложное утверждение в выводе гейта:** файл схемы демонстрируемо изменился, а гейт
   печатает «схема ... **не изменилась**». Скрипт нигде не сравнивает сырое содержимое
   файлов — вывод выводится из пустого списка `Change`, а не из факта равенства. Гейт,
   который на изменённом входе утверждает «не изменилось», — это маскировка, а не
   ограничение.

**Масштаб слепой зоны — измерен, не предположен:** 13 из 65 свойств в
`crates/contracts/schema/*.json` (20%) — `$ref`-only, включая `$.kind → EventKind`,
`MdEvent.payload → MdPayload`, `MdEvent.venue → Venue`, `SysEvent.ReconDivergence → ReconAudit`.
Плюс 5 массивов, в `items` которых `classify_node` не рекурсирует вовсе
(`L2Snapshot.bids/asks`, `L2Delta.bids/asks`, `legacy-manifest.declarations` — все `items: {$ref: Level|LegacySegmentDecl}`).

**Смягчение (учтено, но не снимает блок):** переименование типа ловится отдельной веткой
`classify_schema_file` (`definitions.X — тип удалён`), а `verify_ct_rfc_atomic.sh` всё равно
потребует полный RFC-пакет. То есть это не единственная точка отказа. Блокирую тем не менее:
пункт 2 (ложное «не изменилась») — самостоятельный дефект вывода гейта, а не пробел покрытия.

### F-2 (HIGH, блокирующая) — `verify_ct_rfc_atomic.sh`: молчаливый PASS при запуске из подкаталога

**Файл:** `scripts/verify_ct_rfc_atomic.sh:56`

```bash
mapfile -t CHANGED_PATHS < <(git diff --name-only "${MERGE_BASE}" -- . 2>/dev/null || true)
```

Pathspec `-- .` ограничивает дифф **текущим каталогом**. Скрипт при этом намеренно НЕ делает
`cd` в корень репо (комментарий `:26-29`) и явно позиционируется как самопроверка разработчика
до коммита (`:21`, `:54-55` — «гейт полезен и ДО коммита, как самопроверка перед push»).
Из любого подкаталога, кроме корня, правка `crates/contracts/src/**` становится невидимой,
и гейт выдаёт **PASS**.

Воспроизведение — **один и тот же коммит, один и тот же base-ref, разный `cwd`**:

```
$ printf '\n// mutation\n' >> crates/contracts/src/lib.rs && git commit -qm "mut: T1 change no RFC"

# из корня репо
$ bash scripts/verify_ct_rfc_atomic.sh b3b42d2
VERDICT: FAIL (6 недостающих артефакта(ов) атомарного CT-RFC пакета)

# из подкаталога scripts/
$ cd scripts && bash ./verify_ct_rfc_atomic.sh b3b42d2; echo exit=$?
PASS  crates/contracts/src/** не тронут — атомарность CT-RFC пакета не применима
VERDICT: PASS
exit=0
```

Это ровно класс M-40: проверка, **не сумевшая выполниться**, обязана давать FAIL, а даёт
молчаливый PASS с формулировкой «не тронут», которая ложна.

Зеркальный дефект в другую сторону: из `crates/contracts/` гейт не может пройти **никогда** —
`docs/rfc/*` вне pathspec, поэтому RFC-артефакт не найдётся ни при каком корректном пакете.

**Смягчение:** в CI шаг `run` выполняется из корня workspace, поэтому **сегодняшний CI-путь
корректен** — F-2 не ломает CI. Блокирую потому, что гейт нарушает собственный
задокументированный контракт (pre-push самопроверка) и дефолт у него fail-**open**.

### F-3 (MEDIUM, не блокирующая) — асимметрия анти-плацебо покрытия

`verify_ct_rfc_atomic.sh` (128 строк bash, чистая проверка присутствия путей) имеет машинный
self-test на 9 сценариев. `diff_contract_schema.py` — **214 строк содержательной логики
классификации, самый сложный компонент набора** — не имеет self-test вообще; в CI он
выполняется только на реальном дифе, который в 99% прогонов пуст (`CLASS=none` → PASS).
F-1 выжила именно поэтому. Мои D1–D6/W1–W3 выше — ручные и в репозитории не остаются.

### F-4 (LOW, замечание) — расхождение источника диффа между двумя гейтами

`verify_ct_rfc_atomic.sh:56` берёт дифф **merge-base → рабочее дерево** (видит незакоммиченное),
`diff_contract_schema.sh:64-65` — **`git show base` → `git show HEAD`** (рабочее дерево не
видит вовсе). Для CI разницы нет, для локальной самопроверки перед коммитом — есть: второй
гейт на незакоммиченной правке схемы тихо скажет `none`. Стоит выровнять или задокументировать.

### F-5 (LOW, замечание) — `expect deny` в self-test не различает причину отказа

`scripts/tests/red_ct_rfc_atomic.sh:19-23`: сценарий засчитан пройденным при **любом**
ненулевом exit. Если гейт упадёт по посторонней причине (синтаксис, отсутствие `git`),
BAD-сценарии останутся «зелёными». Проба доказывает «упало», но не «упало на том, что мы
убрали».

---

## Что сделано правильно (не находки — фиксирую как удачные решения)

- **S0 setup-guard в `verify_contracts.sh:69-97`** — покрывает генератор, каталоги схем и
  обеих групп фикстур, `python3`, модуль `jsonschema`, `cargo`; при провале явно помечает
  S1–S4 как FAIL (`:90-93`), а не пропускает молча. Проверено: это правильный fail-closed.
- **Восстановление схемы через `trap cleanup EXIT`** (`verify_contracts.sh:43-54`) —
  регенерация как часть проверки, дерево чистое при любом исходе. Подтверждено пустым
  `git status --porcelain` после прогона.
- **Приоритет invalid-фикстур** (`contracts_validate_fixtures.py:89-102`): «фикстура, которая
  должна падать и не падает, значит схема ничего не проверяет» — верная постановка
  анти-плацебо, а не формальная валидация.
- **Независимый путь валидации** — реальный JSON Schema валидатор, а не serde-парсинг
  Rust-типов; это честная проверка `CT-I-5` глазами не-Rust консюмера.
- **S3-канарейка `EventKind`** (`verify_contracts.sh:130-149`) закрывает дыру Д4 и корректно
  краснеет и на нуле совпадений, и на >1, и на «определён не там».
- **Fail-closed определение базы в CI** (`ci.yml`, шаг `base`) — zero-SHA → `exit 1`.

---

## Требуется для APPROVED

1. Устранить F-1: `diff_contract_schema.sh`/`.py` не должен утверждать «схема не изменилась»,
   когда содержимое файлов схемы отличается, и не должен выдавать PASS на неклассифицированном
   изменении.
2. Устранить F-2: `verify_ct_rfc_atomic.sh` обязан давать одинаковый вердикт независимо от
   `cwd`, либо fail-closed при невозможности увидеть полный дифф репозитория.
3. Желательно (не блокирует): F-3 — машинный self-test классификатора; F-4/F-5 — по решению
   architect'а.

Дизайн решений — зона architect (`gates.md` §4). Повторный прогон моих мутаций D-серии и
проверки `cwd` — обязательное условие следующего вердикта.

## Push-статус

⚠ NOT merged; blocked by gate 4 (PR-time reviewer). `PROJECT-STATE.md`/`TECH-DEBT.md` не
обновлялись, `main` не тронут, деплой-гейт `gates.md` §8 не запускался (нечего деплоить).
Этот вердикт закоммичен на `feat/contract-gates` как артефакт гейта
(`.claude/rules/branch-hygiene.md` п.3).
