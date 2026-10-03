# R-012 — `feat/contract-gates` rev2 (повторный круг после R-006)

- **Дата (UTC):** 2026-08-01
- **Ветка / HEAD:** `feat/contract-gates` @ `ac718e4`
- **База:** `origin/main` @ `f5fe12e` (ветка отстаёт на 43 процессных `docs/*` коммита — отставание базы, не правки агента)
- **Ревьюер:** reviewer (worktree `/tmp/hft-rev-cgates2`, detached)
- **Предыдущий вердикт:** `R-006` — CHANGES REQUESTED (F-1, F-2 блокирующие; F-3 medium)

## ВЕРДИКТ: **CHANGES REQUESTED** (не мержу)

F-1, F-2, F-3 закрыты по существу — перепроверено собственными мутациями, не со слов.
Merge блокирует **новая находка F-4**: остаточный fail-open **того же класса, что F-1**,
в `classify_repo` — гейт печатает `ADDITIVE / обратно совместимо / VERDICT: PASS` на дифе,
содержащем изменение, которое он демонстрируемо не умеет прочитать. Фикс F-1 закрыл
изолированный случай; в сосуществовании с любым распознанным изменением дыра открыта.

---

## Статус находок R-006

### F-1 (`diff_contract_schema`: `$ref`-breaking → `none`) — ✅ ЗАКРЫТА

Проверено **собственным** дифом, отличным от того, что в self-test: перенацеливание
`$ref` `#/definitions/Side` → `#/definitions/Venue` в **реальной** закоммиченной
`crates/contracts/schema/event.schema.json`, глубоко внутри варианта `oneOf` (self-test
покрывает property-уровень и `items`-уровень, не `oneOf[...]`):

```
BREAKING definitions.MdPayload.oneOf[Liquidation].Liquidation.side.$ref — цель ссылки изменена: '#/definitions/Side' → '#/definitions/Venue' (не разыменовывается, трактуется консервативно)
BREAKING definitions.MdPayload.oneOf[Trade].Trade.side.$ref — цель ссылки изменена: '#/definitions/Side' → '#/definitions/Venue' (не разыменовывается, трактуется консервативно)
CLASS=breaking
FAIL  BREAKING диф БЕЗ bump'а SCHEMA_VERSION: base=4 head=4 (обязано head > base)
VERDICT: FAIL
REAL_EXIT=1
```

Рекурсия в `items` и трактовка `$ref` как консервативно-breaking работают. Ложное
«схема не изменилась» на изолированном `$ref`-retarget больше не воспроизводится.

### F-2 (`verify_ct_rfc_atomic`: молчаливый PASS из подкаталога) — ✅ ЗАКРЫТА

Регресс-проверка: тронул `crates/contracts/src/lib.rs` БЕЗ CT-RFC-пакета и запустил
барьер **из `sub/deep`** (до фикса — `PASS «нечего проверять»`):

```
$ (cd sub/deep && bash .../verify_ct_rfc_atomic.sh "$BASE")
PASS  crates/contracts/src/** тронут — проверяю атомарность CT-RFC пакета (docs/05 §4)
FAIL  RFC-документ (docs/rfc/CT-RFC-NNN-*.md) — ...
FAIL  регенерированная схема (crates/contracts/schema/*.json) — ...
FAIL  CHANGELOG (crates/contracts/CHANGELOG.md) — ...
FAIL  valid-фикстура (crates/contracts/fixtures/valid/*.json) — ...
FAIL  invalid-фикстура (crates/contracts/fixtures/invalid/*.json) — ...
FAIL  тест (crates/contracts/tests/*.rs) — ...
VERDICT: FAIL (6 недостающих артефакта(ов) атомарного CT-RFC пакета)
exit=1
```

Из корня — **побайтово тот же** результат (`REAL_EXIT=1`). Якорение на
`git rev-parse --show-toplevel` + `git -C <root>` без pathspec корректно; self-test-песочницы
по-прежнему видят свой репозиторий (9/9 сценариев `red_ct_rfc_atomic.sh` PASS).

### F-3 (у классификатора не было self-test) — ✅ ЗАКРЫТА (с оговоркой F-4)

`scripts/tests/red_diff_contract_schema.sh` — 12/12 PASS, подключён в CI-джоб `contracts`.
Проверяет одновременно `CLASS=`, `VERDICT:` и exit-код (не только факт отказа) — это
правильнее, чем `expect()` соседнего self-test. Оговорка: набор сценариев **не содержит
ни одного дифа с двумя изменениями в одном файле**, из-за чего F-4 не пойман (см. ниже).

---

## 🔴 F-4 (БЛОКЕР, новая) — fail-open: нераспознанное breaking маскируется любым сосуществующим распознанным изменением

**Где:** `scripts/diff_contract_schema.py`, `classify_repo()`, строка 205.

```python
file_changes = classify_schema_file(b, h)
if not file_changes and json.dumps(b, sort_keys=True) != json.dumps(h, sort_keys=True):
    # fail-closed safety-net (R-006 F-1)
```

**Симптом.** Fail-closed safety-net, добавленный как фикс F-1, срабатывает **только когда
для файла набор правил дал ПУСТОЙ список Change**. Достаточно одного распознанного
изменения в том же файле (например, нового опционального поля — самая частая правка
контракта), чтобы сеть не сработала. Нераспознанное — и при этом реально ломающее —
изменение в том же файле тогда не даёт ни одного `Change`, итог агрегируется как
`additive`, и гейт **утверждает совместимость**: `PASS диф классифицирован ADDITIVE
(обратно совместимо) — bump SCHEMA_VERSION не требуется этим гейтом`, exit 0.

**Не распознаётся правилами `classify_node`:** `allOf`, `anyOf`, `not`,
`patternProperties`, `additionalProperties`-как-схема (не bool), `prefixItems`,
`additionalItems`, и все ограничивающие ключевые слова (`maxLength`, `minLength`,
`pattern`, `minimum`/`maximum`, `minItems`/`maxItems`, `format`, `const`, `multipleOf`,
`uniqueItems`, `dependentRequired`).

**Это не гипотетическая форма.** `allOf` присутствует в **текущей закоммиченной** схеме:

```
$ python3 -c "...walk(event.schema.json)..."
crates/contracts/schema/event.schema.json [('$.definitions.ReconAudit.properties.action', 'allOf')]

{"description": "Что recon сделал по факту.",
 "allOf": [{"$ref": "#/definitions/ReconAction"}]}
```

`schemars` эмитит `allOf: [{"$ref": ...}]` для любого свойства с doc-комментарием на
ссылочном типе — форма будет воспроизводиться на каждом новом T1-типе.

**Воспроизведение (все три — на реальной схеме репозитория, `base = ac718e4`, без бампа):**

| # | Мутация | Ожидание | Факт |
|---|---|---|---|
| B | `allOf[0].$ref`: `ReconAction` → `Venue`, **изолированно** | breaking | `CLASS=breaking`, `VERDICT: FAIL`, exit 1 ✅ (safety-net сработала) |
| C | **та же** мутация + одно новое опциональное свойство `note` | breaking | `CLASS=additive`, `VERDICT: PASS`, **exit 0** ❌ |
| D | `maxLength: 1` на существующем `ReconAudit.symbol` + опциональное `note2` | breaking | `CLASS=additive`, `VERDICT: PASS`, **exit 0** ❌ |
| E | `allOf[0]`: `$ref` → inline `{"type":"integer"}` + опциональное `note3` | breaking | `CLASS=additive`, `VERDICT: PASS`, **exit 0** ❌ |

Сырой вывод C (D и E идентичны по форме):

```
$ bash scripts/diff_contract_schema.sh "$BASE"
additive definitions.ReconAudit.properties.note — новое опциональное свойство
CLASS=additive

PASS  диф классифицирован ADDITIVE (обратно совместимо) — bump SCHEMA_VERSION не требуется этим гейтом

VERDICT: PASS
REAL_EXIT=0
```

Контроль B доказывает, что мутация действительно ломающая и что сеть её ловит —
ровно до тех пор, пока рядом нет ни одной распознанной правки.

**Почему блокер, а не NOTE.**

1. **Тот же класс дефекта, что F-1**, ради которого прошёл прошлый круг: гейт делает
   утвердительное заявление о безопасности на узле, который он не прочитал. F-1 был
   версией «в одиночку» (`none`); F-4 — версия «в компании» (`additive`).
2. **Покрытый случай — редкий, непокрытый — нормальный.** Атомарный contract-RFC по
   `docs/05` §4 по определению несёт пакет изменений; диф ровно с одной правкой схемы —
   исключение. Сеть защищает исключение и отключается на типовом сценарии.
3. **Бьёт по CT-I-3** (`docs/05` §6, журнал бессмертен, `replay == реальность`):
   единственное назначение связки «класс → обязательный bump `SCHEMA_VERSION`» —
   не дать ломающей правке T1 уехать без версии. Здесь она уезжает с зелёным CI.
4. **Машинного оракула нет.** `red_diff_contract_schema.sh` не содержит ни одного
   сценария с двумя изменениями в одном файле — регресс не будет пойман. Это ровно
   «фикстура счастливого пути» из `.claude/rules/testing.md` (закреплено 2026-07-14),
   пункт 2 чек-листа — **множественность** (два и более элемента в одном такте).

**Границу `reviewer↔architect` (`gates.md` §4) соблюдаю:** дефект описан, фикс не
проектирую — дизайн решения и RED-оракул на регресс (обязательно включающий
диф «нераспознанное breaking + распознанное additive в одном файле») за architect'ом.

---

## Прочие проверки (пройдены)

### Block-scope — ✅

```
$ git diff --name-only origin/main...HEAD
.github/workflows/ci.yml
research/reviews/R-006-contract-gates.md
scripts/contracts_validate_fixtures.py
scripts/diff_contract_schema.py
scripts/diff_contract_schema.sh
scripts/tests/red_ct_rfc_atomic.sh
scripts/tests/red_diff_contract_schema.sh
scripts/verify_contracts.sh
scripts/verify_ct_rfc_atomic.sh
```

Только `scripts/` + `.github/workflows/ci.yml` (+ мой собственный вердикт R-006).

### Block-C (contract governance) — ✅

```
$ git diff --name-only origin/main...HEAD -- crates/contracts | wc -l
0
```

`crates/contracts/**` не тронут ВООБЩЕ — ни типы, ни схема, ни фикстуры, ни тесты.
Гейты строятся ВОКРУГ контрактного слоя, не внутри него. Block-C неприменим,
RISK-BLOCK не применяется (нет касания `risk`/`killswitch`/`oms`/`venue-*`/`contracts`),
canary-логика `EventKind` намеренно вынесена в `verify_contracts.sh` S3, а не в
sacred `crates/contracts/tests/` — правильное решение по scope-guard.

### Setup-guard во всех трёх скриптах — ✅

| Проба | `verify_contracts.sh` | `verify_ct_rfc_atomic.sh` | `diff_contract_schema.sh` |
|---|---|---|---|
| запуск из подкаталога | PASS корректно (`cd` в `ROOT`) | FAIL корректно (F-2, см. выше) | FAIL fail-closed (см. N-2) |
| вне git-репозитория | n/a | `VERDICT: FAIL` exit 1 | `VERDICT: FAIL` exit 1 |
| несуществующий base-ref | n/a | `VERDICT: FAIL` exit 1 | `VERDICT: FAIL` exit 1 |
| пустой каталог фикстур | `FAIL S0` + S1–S4 SKIPPED, exit 1 | n/a | n/a |
| отсутствует `jsonschema` | `FAIL S0` + S1–S4 SKIPPED, exit 1 | n/a | n/a |
| отсутствует классификатор `.py` | n/a | n/a | `FAIL setup-guard`, exit 1 (D6) |

Сырой вывод пробы «пустой каталог invalid-фикстур»:

```
FAIL  S0 setup-guard (генератор+схемы+фикстуры+python-jsonschema+cargo)
каталог invalid-фикстур пуст
FAIL  S1 схема ↔ типы — SKIPPED (setup-guard не прошёл)
FAIL  S2 фикстуры ↔ схема — SKIPPED (setup-guard не прошёл)
FAIL  S3 CT-I-1 EventKind канарейка — SKIPPED (setup-guard не прошёл)
FAIL  S4 cargo test -p contracts — SKIPPED (setup-guard не прошёл)
VERDICT: FAIL (5)
REAL_EXIT=1
```

Важно: недостающий setup не даёт «пропустить» проверки тихо — они явно печатаются как
FAIL/SKIPPED и учитываются в счётчике. Это правильный fail-closed, урок M-40 усвоен.

### Анти-плацебо `verify_contracts.sh` (мутация) — ✅

Поле `ghost_field`, которого нет в Rust-типах, добавлено в закоммиченную схему:

```
PASS  S0 setup-guard (генератор+схемы+фикстуры+python-jsonschema+cargo)
FAIL  S1 схема ↔ типы (regen == committed, CT-I-4)
схема разошлась с Rust-типами — перегенерируй и закоммить:
PASS  S2 фикстуры ↔ схема (valid PASS / invalid REJECT)
PASS  S3 CT-I-1 канарейка EventKind (единственная дефиниция в contracts)
FAIL  S4 cargo test -p contracts (roundtrip + RFC RED-suite GREEN)
VERDICT: FAIL (2 провал(ов))
REAL_EXIT=1
```

Рабочее дерево после прогона — чистое (`trap`-восстановление схем работает):

```
$ git status --porcelain
{чисто}
```

### Дыра `CT-RFC-05` — ✅ признана честно, не замаскирована

Решение: гейт `verify_ct_rfc_atomic.sh` смотрит **только вперёд** — диф
`merge-base(<base-ref>, HEAD)..рабочее-дерево`, где `<base-ref>` в CI берётся ИЗ СОБЫТИЯ
(`push` → `github.event.before`, `pull_request` → `base.sha`, zero-SHA → `exit 1`).
Исторические коммиты не пересканируются, поэтому CI не краснеет задним числом.

Проверено, что это НЕ маскировка:
- в скриптах **нет** ни списка исключений, ни hardcode `CT-RFC-05`, ни тихого `skip`;
- дыра названа прямым текстом в шапке `verify_ct_rfc_atomic.sh` (строки 5–8) как
  мотивирующий класс дефекта, со ссылкой на `docs/plans/contracts-current-state.md` Д2;
- аудит-документ с фактурой (`ls docs/rfc/` → только `CT-RFC-01..04`; 70 упоминаний
  `CT-RFC-05` в коде/доках) уже на `main`;
- барьер FAIL'ит ровно на «правка T1 без `docs/rfc/CT-RFC-NNN-*.md`» — т.е. на классе
  CT-RFC-05, начиная со следующего изменения контракта.

Остаток — нормативный: `docs/rfc/CT-RFC-05-*.md` (ретро-документ) по-прежнему
не существует, и **в `TECH-DEBT.md` открытой записи об этом нет** (есть только
исторические RN-18/RN-20 по M-35, другой предмет). Заводится отдельным долгом
при merge; ретро-документ пишет architect — это нормативный текст, не инструмент.

### CI-джоб — ✅ (существующие не тронуты)

`.github/workflows/ci.yml`: добавлен джоб `contracts`, дописан в `needs`
`status-check`. Джобы `build-test` / `security` / `delivery` / `protected-artifacts`
не изменены. База события определяется fail-closed (zero-SHA/пусто → `exit 1`) —
тот же паттерн, что доказанно работает в `protected-artifacts` (блокер B1, 2026-07-11).
Оба self-test'а (`red_ct_rfc_atomic.sh`, `red_diff_contract_schema.sh`) вызываются
**той же проводкой**, какой CI зовёт сами барьеры — анти-плацебо соблюдено.

### Commit discipline — ✅

```
ac718e4 | architect | ci(contracts): R-006 F-3 — подключить red_diff_contract_schema.sh в contracts job
87f39a8 | architect | test(contracts): R-006 F-3 — self-test классификатора схем, включая кейс $ref
05e5cbe | architect | fix(contracts): R-006 F-2 — setup-guard: запуск не из корня даёт FAIL, а не молчаливый PASS
c4f2e37 | architect | fix(contracts): R-006 F-1 — $ref-breaking больше не классифицируется как «схема не изменилась»
```

Одна находка = один коммит, conventional commit, ссылка на находку R-006,
`Co-Authored-By` — 0, идентичность коммиттера = роль.

---

## Неблокирующие замечания (NOTE — на усмотрение architect'а, merge не держат)

- **N-1. Асимметрия базы между двумя гейтами.** `verify_ct_rfc_atomic.sh` использует
  `merge-base(<base>, HEAD)`, `diff_contract_schema.sh` — сам `<base-ref>` напрямую.
  Если база ушла вперёд с additive-правкой схемы, которой в ветке нет, классификатор
  читает её как удаление:
  ```
  BREAKING definitions.ReconAudit.properties.added_on_main — свойство удалено
  CLASS=breaking
  FAIL  BREAKING диф БЕЗ bump'а SCHEMA_VERSION: base=4 head=4
  VERDICT: FAIL
  ```
  Направление fail-closed (ложный красный, не ложный зелёный), но толкает к
  ложному бампу `SCHEMA_VERSION`. Для текущего merge неопасно: ветка схему не трогает,
  `git diff origin/main...HEAD -- crates/contracts/schema crates/contracts/src` пуст.
- **N-2. `diff_contract_schema.sh` cwd-хрупкий.** `SCHEMA_DIR` относительный и
  `git ls-tree` резолвится от cwd, поэтому из подкаталога скрипт падает с
  диагностикой «нет ни одного файла в `crates/contracts/schema`» вместо «запусти
  из корня». Fail-closed, но сообщение уводит от причины.
- **N-3. `extract_schema_at` глушит ошибку `git show` (`|| true`).** При сбое остаётся
  файл нулевой длины, и классификатор умирает `JSONDecodeError` → `exit≠0` → FAIL.
  Fail-closed, но оператор видит python-traceback, а не сообщение гейта.

---

## Done Block

```
$ git -C /tmp/hft-rev-cgates2 rev-parse --short HEAD
ac718e4

$ git diff --name-only origin/main...HEAD | wc -l
9

$ git diff --name-only origin/main...HEAD -- crates/contracts | wc -l
0

$ bash scripts/verify_contracts.sh 2>&1 | tail -8
PASS  S0 setup-guard (генератор+схемы+фикстуры+python-jsonschema+cargo)
PASS  S1 схема ↔ типы (regen == committed, CT-I-4)
PASS  S2 фикстуры ↔ схема (valid PASS / invalid REJECT)
PASS  S3 CT-I-1 канарейка EventKind (единственная дефиниция в contracts)
PASS  S4 cargo test -p contracts (roundtrip + RFC RED-suite GREEN)

VERDICT: PASS
exit=0

$ bash scripts/tests/red_diff_contract_schema.sh 2>&1 | grep -cE '^PASS'; echo exit=$?
12
exit=0
  (VERDICT: PASS — включая оба $ref-регресс-гварда F-1 и оба setup-guard кейса)

$ bash scripts/tests/red_ct_rfc_atomic.sh 2>&1 | grep -cE '^PASS'; echo exit=$?
9
exit=0
  (VERDICT: PASS — P0/P1 + 6 BAD-сценариев + setup-guard)

$ bash scripts/verify_ct_rfc_atomic.sh origin/main 2>&1 | tail -3
PASS  crates/contracts/src/** не тронут — атомарность CT-RFC пакета не применима

VERDICT: PASS

# Мутационные пробы reviewer'а (собственные, не из self-test):
#   A  $ref-retarget в oneOf-варианте, изолированно      → CLASS=breaking, exit 1  ✅
#   B  allOf-$ref-retarget, изолированно                  → CLASS=breaking, exit 1  ✅
#   C  allOf-$ref-retarget + additive-поле                → CLASS=additive, exit 0  ❌ F-4
#   D  maxLength-сужение + additive-поле                  → CLASS=additive, exit 0  ❌ F-4
#   E  allOf-$ref → inline type + additive-поле           → CLASS=additive, exit 0  ❌ F-4
#   G6 F-2 регресс (contracts/src тронут, cwd=подкаталог) → VERDICT: FAIL (6), exit 1 ✅
#   G7 пустой каталог invalid-фикстур                     → FAIL S0, exit 1  ✅
#   G8 нет python3 jsonschema                             → FAIL S0, exit 1  ✅
#   G9 схема с полем вне Rust-типов                       → FAIL S1+S4, exit 1  ✅
#   G10 рабочее дерево после verify_contracts.sh          → чисто  ✅
```

**Merge НЕ выполнен. Push в `main` НЕ выполнен.** Ветка остаётся на `ac718e4`.

## Кому

`architect` — F-4 (дизайн фикса + RED-оракул на регресс). F-1/F-2/F-3 закрыты,
переоткрывать не нужно. N-1/N-2/N-3 — на усмотрение, merge не держат.
