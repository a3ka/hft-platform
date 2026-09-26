# C-046 — `docs/rfc/CT-RFC-05-margin-inventory.md` (retro-документ, rev2) plan-time gate

**Date (UTC):** 2026-08-02T00:00Z
**Agent:** critic
**Branch audited:** `origin/docs/ct-rfc-05-retro` at `03815dd`
**Worktree:** `/tmp/hft-critic-rfc05b`
**Scope:** DOC-гейт класс A (`.claude/rules/gates.md` §9) — повторный проход после `C-044`
REJECT. Проверка исправлений F1 (несуществующие SHA) и F2 (заниженный список мест правки).

**Verdict: PASS**

## Резюме

Коммит `03815dd` ("docs(rfc): C-044 F1/F2 — SHA проверены по истории, список мест правки
дополнен") — точечный, минимальный дифф ровно по двум находкам `C-044`, без побочных правок.
Обе находки закрыты полностью и корректно, новых непроверенных утверждений не внесено.

## Проверка F1 — SHA

`git diff 2b0cbc2 03815dd` показывает замену `f2d1edb`/`ffedc10`/`6a2c331`/`67b6159` на
`f2d1edb`/`ab6e222`/`988afff` (3 SHA вместо 4, с явной атрибуцией задачи каждому). Проверка
каждого — `git cat-file -e <sha>^{commit}` + `git merge-base --is-ancestor <sha> origin/main`:

```
f2d1edb OK, ancestor-of-main=YES : feat(M-35): task 2b — exhaustive-match MarginInventory (journal/sim) [engine-dev]
ab6e222 OK, ancestor-of-main=YES : feat(M-35): task 2c — exhaustive-match: MdPayload::MarginInventory { .. } => continue в latency_probe
988afff OK, ancestor-of-main=YES : feat(M-35): task 2d — md_kind_label MarginInventory (recorder) [engine-dev]
```

Плюс §8 теперь явно называет `1f342b8` (task 2e — spawn-фикс) наряду с `ba61c62`/`41d3526` —
это была необязательная (не REJECT-блокирующая), но рекомендованная в `C-044` правка; тоже
закрыта:

```
ba61c62 OK, ancestor-of-main=YES : merge(M-35): margin-inventory available-collector — CT-RFC-05 ...
1f342b8 OK, ancestor-of-main=YES : merge(M-35): task 2e — spawn run_margin_inventory в recorder ...
41d3526 OK, ancestor-of-main=YES : docs(M-35): reviewer close-out — margin-inventory MERGED 1f342b8 ...
```

Все 6 SHA в текущей ревизии документа (`f2d1edb`, `ab6e222`, `988afff`, `ba61c62`, `1f342b8`,
`41d3526`) существуют и являются предками `origin/main`. F1 закрыт полностью — 0 фантомных SHA
(было 3 из 4).

## Проверка F2 — список мест правки

Документ теперь заявляет **5** мест (было 4), с явным указанием «task 2b называет три файла в
одной задаче, не два». Проверено СВОИМ грепом (не по тексту документа):

```
$ git show --stat f2d1edb
 crates/journal/examples/dump.rs | 5 +++--
 crates/journal/src/segments.rs  | 3 ++-
 crates/sim/src/exchange.rs      | 3 ++-
 3 files changed, 7 insertions(+), 4 deletions(-)

$ git show --stat ab6e222
 crates/research-cli/src/bin/latency_probe.rs | 10 ++++++++--
 1 file changed

$ git show --stat 988afff
 crates/recorder/src/lib.rs | 4 +++-
 1 file changed
```

3 + 1 + 1 = 5 файлов, дословно совпадает с новым текстом §4. Независимо сверено с
`milestones/M-35-margin-inventory.md` (текущий чекаут, `03815dd`): task 2b перечисляет ТРИ
файла в одном пункте (`crates/journal/src/segments.rs:~1523`, `crates/sim/src/exchange.rs:~223`,
**`crates/journal/examples/dump.rs`** — все три явно названы в теле задачи), task 2c —
`latency_probe.rs`, task 2d — `recorder/src/lib.rs::md_kind_label`. Итого 5, дословно совпадает
с исправленным §4. Также этот список **корроборирует** независимо составленную карту в
параллельно проверенном `docs/rfc/CT-RFC-06-l2delta.md` §8.2 (та же пятёрка файлов, для
другого дискриминанта) — два разных документа, два разных T1-варианта, одна и та же
инфраструктурная карта match-сайтов; совпадение файлов ожидаемо (один и тот же набор
exhaustive-match-точек над `MdPayload`) и является дополнительным перекрёстным подтверждением.

F2 закрыт полностью — 5 мест, `dump.rs` больше не пропущен.

## Новых непроверенных утверждений

Диф `2b0cbc2..03815dd` затрагивает ровно §4 (замена SHA + добавление `dump.rs`) и §8
(добавление `1f342b8`). Все добавленные ссылки на код/коммиты проверены выше. Остальной текст
документа не менялся — уже был проверен в `C-044` и оставался верным (форма типа, аддитивность,
bump-обоснование, JSON Schema/фикстуры/CHANGELOG, критик/risk-critic ссылки, §7 честная
реконструкция причины отсутствия RFC-файла) — не перепроверял заново построчно, т.к. диапазон
изменений в этой ревизии точно локализован git diff'ом и не пересекается с этими секциями.

## Что НЕ проверял

- Весь остальной текст документа за пределами изменённого диапазона (§1-§3, §5-§8 кроме
  добавленной фразы про `1f342b8`) — не перечитывал заново; полагаюсь на `C-044`, где это уже
  было проверено и подтверждено как верное, и на то, что диф `03815dd` их не затрагивает.
- `crates/venue-binance/tests/red_margin_inventory.rs` — документ сам явно исключает его из
  своей зоны (§8), как и в прошлый раз.
- `cargo build --workspace`/`cargo test --workspace` в этом worktree не запускал (read-only
  аудит по git-истории и содержимому файлов).

## Handoff

PASS → architect: ретро-документ закрывает дыру Д2 (`docs/plans/contracts-current-state.md`),
готов к mechanical appendix (verdict в milestone/RFC-файл, если требуется) и normal flow —
следующий шаг вне зоны критика (документ уже описывает T1-факт постфактум, не предлагает новых
изменений, поэтому risk-critic/founder-подпись для НОВОГО решения не требуется; §9 самого
документа явно это не проектирует).

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-02T00:00Z
- Milestone: CT-RFC-05-retro (docs/ct-rfc-05-retro)
- Статус: DONE (critic-гейт пройден, rev2)
- HEAD: 03815dd — docs(rfc): C-044 F1/F2 — SHA проверены по истории, список мест правки дополнен

## §B — Что я сделал
- Перепроверил F1 (SHA) — все 6 цитируемых SHA существуют и являются предками `origin/main`.
- Перепроверил F2 (список мест правки) — независимым грепом подтвердил 5 мест (было занижено до 4), совпадает с исправленным текстом и с milestone-файлом.
- Проверил, что диф `2b0cbc2..03815dd` точечный, не вносит новых непроверенных утверждений.
- Написал вердикт `research/critiques/C-046-ct-rfc-05-retro-rev2.md`, закоммитил на эту ветку.

## §C — Артефакты / результаты
- `research/critiques/C-046-ct-rfc-05-retro-rev2.md` (создан)
- N/A Done Block — read-only роль

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  CT-RFC-05-retro (ветка docs/ct-rfc-05-retro @ 03815dd) прошёл повторный critic-гейт: PASS
  (research/critiques/C-046-ct-rfc-05-retro-rev2.md). Обе находки C-044 (F1 — 3 из 4 несуществующих
  SHA, F2 — заниженный список мест правки 4 вместо 5) закрыты и перепроверены независимо. Документ
  закрывает дыру Д2 (docs/plans/contracts-current-state.md — CT-RFC-05 приземлился в коде без
  формального RFC-файла). Следующий шаг: mechanical appendix (пометка вердикта в документе/
  трекере дыр, если процесс это требует) и закрытие Д2 в актуальном плане.
  ```
- Push-статус: ⏸ commits ready; я запушу сразу после коммита вердикта

## §E — Риски / открытые вопросы
- N/A

=== END HANDOFF ===
