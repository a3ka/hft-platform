# C-044 — `docs/rfc/CT-RFC-05-margin-inventory.md` (retro-документ) plan-time gate

**Date (UTC):** 2026-08-02T00:00Z
**Agent:** critic
**Branch audited:** `origin/docs/ct-rfc-05-retro` at `2b0cbc2`
**Worktree:** `/tmp/hft-critic-rfc05`
**Scope:** DOC-гейт класс A (`.claude/rules/gates.md` §9) — ретро-документ фиксирует T1-изменение
`CT-RFC-05` (`MdPayload::MarginInventory`), приземлившееся в коде без RFC-файла (дыра Д2,
`docs/plans/contracts-current-state.md`, подтверждена `research/critiques/C-040-design-migration-plan.md:269-272`).
**Verdict: REJECT**

## Резюме

Документ (204 строки, `STATUS: RETRO-DOCUMENTED`) в целом добротная и честная реконструкция:
форма типа, класс изменения (аддитивное), обоснование бампа `SCHEMA_VERSION` 3→4, совместимость,
честное признание неизвестного (§8), связь с `milestones/M-35-margin-inventory.md` — всё
ПРОВЕРЕНО мной по коду/тестам/git-истории и СОВПАДАЕТ с фактами (детали ниже). Но §4
("Исчерпывающий match") содержит две конкретные фактические ошибки в единственном месте
документа, где он ссылается на git-коммиты как на подтверждение факта. Ошибки — именно того
рода, для защиты от которого этот документ и пишется ("подтверждено" оказывается неподтверждённым
при проверке). REJECT — не потому что T1-факты неверны (они верны), а потому что документ,
чья единственная функция — быть НАДЁЖНЫМ источником фактов после того, как обычная дисциплина
это доверие уже подвела, сам содержит непроверенную (и при проверке — ложную) ссылку на
доказательство.

## Находки (REJECT-блокирующие)

### F1 — §4 цитирует 3 из 4 коммитов, которых НЕТ в смёрженной истории `main`

`docs/rfc/CT-RFC-05-margin-inventory.md:105`:

> "...подтверждено коммитами `f2d1edb`/`ffedc10`/`6a2c331`/`67b6159`."

Проверка (`git merge-base --is-ancestor <sha> origin/main`):

```
f2d1edb -> ancestor of origin/main: YES   (task 2b, journal/sim — реально смёржен)
ffedc10 -> ancestor of origin/main: NO    (дубликат task 2b, ОРФАН — не в истории main)
6a2c331 -> ancestor of origin/main: NO    (дубликат task 2c, ОРФАН — не в истории main)
67b6159 -> ancestor of origin/main: NO    (дубликат task 2c, ОРФАН — не в истории main)
```

Реальная ancestry-цепочка `e06e48a..ba61c62` (`git log --oneline --ancestry-path`) содержит для
task 2c коммит `ab6e222` (не `6a2c331`/`67b6159`), и для task 2d — `988afff`
("feat(M-35): task 2d — md_kind_label MarginInventory (recorder)") — коммит, который §4 текстом
описывает как 4-е место правки (`crates/recorder/src/lib.rs::md_kind_label`), но **не цитирует
вообще**. Проверено: `988afff` реально меняет `crates/recorder/src/lib.rs` ровно так, как описано
в тексте документа (arm `MarginInventory => "margin_inventory"`), но его SHA нигде в документе
не встречается.

Итог: из 4 процитированных SHA только 1 (`f2d1edb`) действительно доказывает то, что документ
утверждает; 3 — дубликаты-орфаны вне истории `main` (вероятно, из более раннего/переигранного
прохода той же задачи на ветке `feat/M-35`), а реальный коммит для 4-го места правки не назван.

### F2 — §4 занижает список "мест правки": 4 вместо 5 (пропущен `dump.rs`)

`docs/rfc/CT-RFC-05-margin-inventory.md:100-105` утверждает: "Milestone
`milestones/M-35-margin-inventory.md` §Tasks 2b/2c/2d перечисляет ровно 4 места правки:
`crates/journal/src/segments.rs`..., `crates/sim/src/exchange.rs`...,
`crates/research-cli/src/bin/latency_probe.rs`..., `crates/recorder/src/lib.rs::md_kind_label`...".

Сам milestone (`milestones/M-35-margin-inventory.md`, task 2b) называет **три** файла в одной
задаче 2b, не два: `crates/journal/src/segments.rs`, `crates/sim/src/exchange.rs` **и**
`crates/journal/examples/dump.rs` (dev-дампер, явно упомянут в тексте задачи 2b: "**и**
`crates/journal/examples/dump.rs`"). Коммит `f2d1edb` это подтверждает фактически —
`git show --stat f2d1edb` показывает правку ровно в трёх файлах, включая
`crates/journal/examples/dump.rs`. Итого реальных мест правки — **5**, не 4; документ теряет
`dump.rs` из списка, хотя цитирует сам коммит (`f2d1edb`), в диффе которого этот файл виден.

## Почему это REJECT, а не NOTE

Обе находки локализованы в одном подразделе (§4, "Исчерпывающий match") и не меняют выводы
документа о форме T1/классе изменения/бампе версии/совместимости — эти части я проверил
независимо и они верны (см. "Проверенная фактура" ниже). Но: (а) документ существует специально
для того, чтобы заменить неформальную, непроверяемую память формальным, проверяемым фактическим
следом — заявление "подтверждено коммитами X/Y/Z/W" при трёх из четырёх SHA, не входящих в
историю `main`, ровно воспроизводит ту категорию дефекта (недоказанное выдаётся за доказанное),
против которой сам документ направлен; (b) это тот же класс ошибки, который уже один раз
заблокировал этот milestone на plan-time (`C-024` B1: "milestone cites a non-existent feasibility
proof") — цитирование несуществующего/неверного доказательства как факта; (c) fail-closed
стандарт проекта («сомневаешься — класс A», `.claude/rules/gates.md:357`) применим и здесь.
Фикс дешёвый (заменить 3 SHA на верные `ab6e222`/`988afff`, добавить `dump.rs` в список из 4→5
мест) — но обязан пройти как правка + повторный critic-проход, не молчаливое исправление.

## Проверенная фактура (что подтвердилось, сырой вывод)

**§1 (форма типа) — СОВПАДАЕТ.** `crates/contracts/src/lib.rs:301-315`: `MarginInventory {
available_e8: i64, ts_exch_ms: i64 }`, вставлен последним после `L2Delta`, обёрнут в
`MdEvent{venue,symbol,payload}` — ровно как в документе. `symbol` не поле пейлоада (описано в
doc-комментарии кода как поле конверта `MdEvent`, не структуры) — документ это не путает, явно
показывает обёртку `Event{...MdEvent{venue,symbol,payload:...}}`.

**§2 (аддитивность) + тесты — СОВПАДАЕТ.**
```
$ grep -c "fn mi_i_1" crates/contracts/tests/ct_rfc05.rs
3
```
`mi_i_1_margin_inventory_roundtrip`, `mi_i_1_pre_rfc05_funding_still_decodes` (анти-плацебо на
позицию вставки, декодирует `FUNDING_PRECHANGE` под `SCHEMA_VERSION==4`),
`mi_i_1_schema_version_is_4` — все три существуют дословно как описано в §5.

**§3 (bump 3→4, правило "новый эмитируемый вариант ⇒ bump") — СОВПАДАЕТ.**
`crates/contracts/src/lib.rs:24-26` — комментарий-история версий подтверждает "4: CT-RFC-05 —
MdPayload::MarginInventory". Коммит `b3a5a95` реально переименовал/обновил
`ct_rfc04_rev2_schema_epoch_is_three` → `schema_epoch_tripwire_current_epoch` с `assert_eq!(...,
4, ...)` — подтверждает прецедент дословно.

**§4 (совместимость) — СОВПАДАЕТ, включая честную оговорку.** Прямого reuse-теста конкретно на
эпоху 3→4 для `MarginInventory` не найдено (`grep -rn decide_open_segment
crates/journal/tests/` даёт только `red_l2delta_rollback_boundary.rs` и
`red_restore_from_cold.rs`, ни один не про `MarginInventory`) — документ САМ честно отмечает это
как открытый вопрос, не как факт. Верно.

**§5 (закреплено) — СОВПАДАЕТ.** JSON Schema (`crates/contracts/schema/event.schema.json:346-368`)
содержит объект `MarginInventory`, `required: [available_e8, ts_exch_ms]`, оба `integer`/`int64`.
Фикстуры существуют дословно: `fixtures/valid/event-margin-inventory.json`,
`fixtures/invalid/event-margin-inventory-missing-ts.json` (без `ts_exch_ms`, как заявлено).
CHANGELOG (`crates/contracts/CHANGELOG.md:6-19`) — секция "schema_version 3 → 4 — CT-RFC-05" с
источником/границей интерпретации, как в документе. Milestone `M-35`: `STATUS: DONE`, реально
существует, содержит цитату "Founder-решение (2026-07-25): «маржин по usdt/usdc собирать»»"
дословно.

**§5 (critic/risk-critic ссылки) — СОВПАДАЕТ.** `research/critiques/C-024.md` (не путать с
одноимённым, но другим по теме `C-024-M-28.md`) — REJECT r1 по причинам B1 (недостоверная ссылка
на survey §9) + B2 (нет CHANGELOG/фикстур), ровно как пересказано в документе §5/§7. Fix-коммит
`0999929` и re-review PASS `a174696` существуют и соответствуют. `research/critiques/C-025.md` —
risk-critic PASS, read-only ключ, тема M-35 — подтверждено.

**§6 (мотивация) — без выдуманных обоснований.** Grep на маркеры типа «было решено»/«для
поддержки» без опоры на коммит/milestone/вердикт — 0 совпадений в документе. Мотивационная
цепочка (survey §9, founder-решение, C-024 B1→фикс) прослеживается до реальных источников.

**§7-§8 (честность о процедурной дыре, машинный гейт) — СОВПАДАЕТ.** `scripts/verify_ct_rfc_atomic.sh`
существует (коммит `557be33`, 2026-07-31), требует 6 артефактов §4 (a-е — RFC-файл, schema,
CHANGELOG, valid/invalid фикстуры, тест) — комментарии скрипта дословно соответствуют
пересказу в §7 документа. `docs/plans/contracts-current-state.md:156-160,315` подтверждает "Д2"
находку и цифру "122+ млн событий журнала" дословно.

**§8 (deployment SHA) — незначительная неточность, НЕ отдельная находка.** Документ цитирует
`ba61c62`/`41d3526` как "реализован и задеплоен". Фактически рабочий коллектор на проде заработал
только после `1f342b8` (task 2e — spawn-фикс, `run_margin_inventory` не был заспавнен до этого,
0 событий на проде до фикса). `ba61c62` — реальный merge T1-типа+match-арм (без task 2e);
`41d3526` (reviewer close-out, процитирован документом) сам явно называет `1f342b8` как merge SHA
и подтверждает §8 GREEN. Поскольку документ цитирует `41d3526`, который содержит верную ссылку на
`1f342b8`, косвенная цепочка не искажает факт — не считаю это отдельным REJECT-пунктом, но
architect стоит поправить на явную SHA `1f342b8` при следующей правке §8, для однозначности.

## Что НЕ проверял

- `crates/venue-binance/tests/red_margin_inventory.rs` построчно — документ сам явно исключает
  его из своей зоны (§8), я это уважаю.
- Полный `cargo test --workspace` / `cargo build --workspace` прогон в этом worktree не запускал
  (read-only аудит по факту git-истории и содержимого файлов, не пересборка).
- Секреты/IP-restrict состояние текущего API-ключа на VPS (тема risk-critic `C-025`, не T1-формы).
- `research/data-quality/margin-source-survey.md` §1-§8 (негативный результат) — не перечитывал
  построчно, доверился уже вынесенному вердикту `C-024` (сам был REJECT-заблокирован именно на
  недостоверности этой ссылки, затем фиксирован).

## Handoff

REJECT → `architect`: исправить §4 (dump.rs пятым местом правки; заменить SHA `ffedc10`/`6a2c331`/
`67b6159` на `ab6e222` (task 2c) и `988afff` (task 2d)), опционально явно назвать `1f342b8` в §8.
После правки — повторный critic-проход (новый C-NNN) перед тем, как ретро-документ считается
закрывающим дыру Д2.
