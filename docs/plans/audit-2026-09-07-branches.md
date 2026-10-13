<!-- FACTS: audited_head=91a4b84d6f8a71760767e9ca3cfa9a0d1d27fa23 collected=2026-09-07 -->

# Аудит девяти невлитых веток (2026-09-07)

**Роль:** architect (клон, свежий контекст, модель — sonnet, названа явно в запуске).
Зона сужена ОДНОЙ осью: судьба невлитых веток. Ничего не удалено, ни в одну чужую ветку
не пушил. Дерево — `/tmp/hft-audit-branches`, worktree от `origin/main` на новой ветке
`docs/audit-branches`. Метод и форма — по образцу
`docs/plans/branch-disposition-2026-09-05.md` (прочитан целиком, метод перепроверен
исполнением на `M-67`, см. §1).

**Вне зоны (по инструкции):** отложенный торговый путь (`risk`/`killswitch`/`oms`).
Проверено грепом по всем девяти веткам — ни одна не трогает `crates/{risk,killswitch,oms}/**`
(см. §«Пределы разбора» за командой). Пометка ОТЛОЖЕНО не применена ни к одной ветке.

## Сводная таблица

| ветка | своих коммитов | отставание | собирается поверх main | вердикт | что осталось |
|---|---|---|---|---|---|
| `docs/M-67-rev2` | 8 | 1010 | **НЕТ** — RED-оракулы `MD-I-6`/`MD-I-7` не компилируются (`E0063`, поле `depth_cadence_ms`) | ВЕСТИ | rev3: почини 2 оракула под сегодняшний `Selector`, реши, что делать с `MD-I-7` (инвариант, вероятно, отменён подписью П-014) |
| `docs/M-70-rev2` | 43 | 59 | **НЕТ** — конфликт слияния в `crates/gateway/src/lib.rs` | ВЕСТИ (активный предмет, см. §2) | разрешить конфликт с M-77 (уже в `main`) + новая PR-гейт перепроверка после R-172 REJECT |
| `docs/M-73-closeout-architect` | 21 | 183 | не проверялось (тест-файлы, без `src/`) | ВЕСТИ | close-out M-73 (2 коммита) готов; остальные 19 — M-74 rev5 WIP, `verify_M-74.sh` FAIL(15) |
| `docs/handover-0905-final` | 0 | 28 | — | СНЕСТИ | ничего, вершина — предок `main` |
| `docs/handover-s7-fix` | 0 | 26 | — | СНЕСТИ | ничего, вершина — предок `main` |
| `feat/M-64-export-contract` | 11 | 1259 | N/A — код не тронут (только спека+критика) | ВЕСТИ | ждёт миграции К1/К6b типов в `crates/contracts` (внешняя блокировка, не своя) |
| `feat/M-72-subscription-terminality` | 26 | 344 | N/A — только `*/tests/**`, RED ожидаемо COMPILE-RED | ВЕСТИ | round 4 (arbiter A-029: финальный) — 5 пунктов из C-200 |
| `feat/M-76-raw-output-barrier` | 1 | 166 | N/A — один md-файл | ВЕСТИ | RED-набор ещё не написан; в очереди после M-72/M-74 (решение founder'а) |
| `feat/harness-milestone-shape` | 14 | 440 | не собирается через `cargo` (нет `crates/**`) — но `bash -n` чист | ВЕСТИ | round 9: B-14/B-15 из C-179; 8 REJECT-раундов без обращения к арбитру — блокер сам по себе |

## 1. `docs/M-67-rev2` — ВЕСТИ

```
$ git rev-parse origin/docs/M-67-rev2
eaef386e04765b6fcc27ed74d948c995acedcf54
```

Вершина совпадает с той, что аудировал независимый советник 2026-09-05
(`docs/plans/branch-disposition-2026-09-05.md`) — ветка не двигалась два дня. Перепроверил
его находку исполнением на СЕГОДНЯШНЕМ `main` (тест-файлы взяты из ветки в чистое дерево
от `origin/main`, не из старого коммита):

```
$ git show origin/docs/M-67-rev2:crates/gateway/tests/red_md_i6_journal_first.rs > crates/gateway/tests/red_md_i6_journal_first.rs
$ cargo test -p gateway --test red_md_i6_journal_first --no-run 2>&1 | tail -6
error[E0063]: missing field `depth_cadence_ms` in initializer of `Selector`
   --> crates/gateway/tests/red_md_i6_journal_first.rs:123:5
error: could not compile `gateway` (test "red_md_i6_journal_first") due to 1 previous error
$ cargo test -p gateway --test red_md_i7_band_lock --no-run 2>&1 | tail -6
error[E0063]: missing field `depth_cadence_ms` in initializer of `Selector`
   --> crates/gateway/tests/red_md_i7_band_lock.rs:118:5
error: could not compile `gateway` (test "red_md_i7_band_lock") due to 1 previous error
$ cargo test -p journal --test red_md_i2_hot_window 2>&1 | tail -6
test md_i2_b1_no_event_younger_than_window_is_deleted ... FAILED
test md_i2_b2_stale_data_is_actually_removed ... ok
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.21s
```

Итог не изменился за два дня: `MD-I-6`/`MD-I-7` не собираются (M-68 добавил
`depth_cadence_ms` в `Selector` уже после написания оракулов), `MD-I-2` — честный RED
(`b1` красный, `b2` зелёный), но RED до реализации не живёт в `main` (`gates.md` §8).
Не сношу: `main` явно указывает на эту ветку как на носитель спеки —

```
$ grep -n 'M-67' docs/PENDING-SIGNATURE.md | head -3
837:> **Носитель `M-67` rev2 (`R-091` У-1).** Файла спеки в `main` НЕТ: он живёт на ветке
838:> `docs/M-67-rev2`. Вторая ветка кластера, `docs/M-67-market-layer`, удалена 2026-08-19 как
842:**Основание пункта 3** (`M-67` rev2 §7.1): `min`/`max` не аддитивны. `Σ_i min_m(d_i) ≤
```

Снос без переноса ссылок оборвал бы `PENDING-SIGNATURE.md`. Не переделываю с нуля: `MD-I-2`
и большая часть спеки (замер ёмкости, П-005/П-023 контекст) остаются в силе — чинить два
оракула и снять/подтвердить `MD-I-7` дешевле, чем писать заново. Не мержу как есть — RED,
который не компилируется, не проходит гейт §8 ни при каком статусе.

## 2. `docs/M-70-rev2` — ВЕСТИ (активный предмет; НЕ сношу ни при каких условиях — по инструкции)

Точное препятствие вливанию СЕГОДНЯ — два независимых блокера:

### 2.1. Конфликт слияния с `main` — новый, не был известен на момент последнего PR-гейта

```
$ git checkout -B docs-M-70-rev2-local origin/docs/M-70-rev2
$ git merge --no-commit --no-ff origin/main
Auto-merging crates/gateway/src/lib.rs
CONFLICT (content): Merge conflict in crates/gateway/src/lib.rs
Automatic merge failed; fix conflicts and then commit the result.
$ grep -n '^<<<<<<<\|^=======\|^>>>>>>>' crates/gateway/src/lib.rs
1519:<<<<<<< HEAD
1527:=======
1534:>>>>>>> origin/main
```

Причина: сама ветка `M-70` 2026-09-04 выделила отдельный предмет `M-77` («кадр строится БЕЗ
книги», коммит `d0a1109`), но `M-77` был реализован и влит в `main` НЕЗАВИСИМО
(`8e2ea49` 2026-09-06, «feat(M-77): task #3 — реализация развязки Б») ДО того, как ветка
`M-70` подтянула этот код к себе. Обе стороны трогают вычисление `depth_reach_bid/ask`:
ветка держит старые поля + оставляет `self.book` нетронутым, `main` уже считает через
`self.book.max_reach_pct(...)` за O(1). Подтверждено, что `M-77` закрыт и заархивирован:

```
$ find . -iname '*M-77*' -not -path './target/*'
./docs/archive/M-77-frame-book-continuity.md
./docs/archive/verify_M-77.sh
./research/reviews/R-176-M-77-frame-book-continuity-pr-gate.md
```

Конфликт разрешён обратно (`git merge --abort`), ничего не закоммичено.

### 2.2. Гейт после последнего REJECT не закрыт новым вердиктом

Последний вердикт — `R-172` (PR-гейт, round 3) — **REJECT**:

```
$ git show origin/docs/M-70-rev2:research/reviews/R-172-M-70-pr-gate-rev3.md | head -6
<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: 547bf24311897155afd9b5bdb98d55ae1a721bc2
audited_head: 7b16eec266e841c439a9f2d15470cfd5134accf1
verdict: REJECT
```

`audited_head` (`7b16eec`) — это состояние ветки ДО коммитов `2f54595`/`09a7fdf`/`39b78c4`/
`5a22a82`, которые R-172 сам же и предписал (находки Б-1/Б-2 — в теле файла на ветке).

После REJECT в ветку легли исправления (`2f54595` — Б-1 «связка», `09a7fdf`/`39b78c4` —
задачи 6/7 под Б-2, `5a22a82` — sacred-пин 9→10), критик дал узкий `C-210` (`verdict: NOTE`,
только по `VB-I-5`), а затем ветка ушла в отдельную работу (`M-77` спека и `§3ter/§5ter`
правки по чекпоинтеру). **Ни одного нового PR-гейт вердикта (`R-173` и далее) на эти правки
нет** — значит закрытие находок `R-172` никем не подтверждено:

```
$ git ls-tree -r --name-only origin/docs/M-70-rev2 -- research/reviews
research/reviews/R-170-M-70-plan-time-merge-refused.md
research/reviews/R-171-M-70-pr-gate-rev2.md
research/reviews/R-172-M-70-pr-gate-rev3.md
```

**Что осталось:** (а) разрешить конфликт §2.1 — вручную решить, чей вариант
`depth_reach_bid/ask` верен (по всей видимости — вариант `main`, ветка `M-70` кода не
меняла независимо, только унаследовала устаревший); (б) запросить PR-гейт round 4 на
получившийся diff, поскольку `R-172` не переигран.

## 3. `docs/M-73-closeout-architect` — ВЕСТИ

```
$ git log --format='%h %ad %s' --date=short origin/main..origin/docs/M-73-closeout-architect | tac | head -2
bc4b581 2026-08-30 test(M-73): TD-192 — гейт наблюдает то, что обрывает деплой всего проекта [architect]
7b9bf66 2026-08-30 docs(M-73): close-out — §Tasks приведены к факту, STATUS → DONE [architect]
```

Первые два коммита — чистый close-out `M-73` (код уже в `main` через `7581f60`, PR #129,
30.08); в `main` при этом STATUS всё ещё `PROPOSED`:

```
$ git show origin/main:milestones/M-73-offsite-schedule.md | sed -n '5p'
STATUS: PROPOSED (2026-08-29, architect). Исполняет `П-023` (граница C: офсайт-копия,
```

Остальные 19 коммитов — НОВЫЙ предмет `M-74` (restore-drill), заведённый в ТОЙ ЖЕ ветке.
Прошёл 4 раунда критика (`C-187`→`C-188`→`C-189`→`C-191`, все REJECT), арбитр `A-028`
задал предел в 4 круга и сменил конструкцию (rev5); последний коммит (`e758b56`,
2026-09-03) — разведка, а не фикс: «счётчика для 6b НЕ ХВАТАЕТ, проба всё ещё rev4»:

```
$ git log -1 --format='%B' e758b56 | sed -n '1p;10,13p'
spec(M-74): разведка исполнена — счётчика для 6b НЕ ХВАТАЕТ, проба всё ещё rev4 [architect]
  scripts/tests/red_restore_drill.sh → ВСЁ ЕЩЁ rev4: сценарии наблюдателя вызова через
                                    JOURNAL_DRILL_READER — ровно тот механизм, который C-191
                                    предъявил подделываемым и который решение founder'а сняло
  verify_M-74.sh                  → VERDICT: FAIL (15), exit=1
```

**Что осталось:** close-out `M-73` можно влить отдельно хоть сегодня (2 коммита, чистый
docs-предмет). `M-74` не готов: rev5 конструкция объявлена, но задача 1c (переписать пробу
`scripts/tests/red_restore_drill.sh` с rev4 на rev5) не сделана, `verify_M-74.sh` красный
15 проверками. Ветка целиком остаётся ВЕСТИ, пока смешивает готовое с незаконченным.

## 4. `docs/handover-0905-final` — СНЕСТИ

```
$ git rev-parse origin/docs/handover-0905-final
a954d477ceb587900c40b41a1116af0d454b5ba3
$ git merge-base --is-ancestor origin/docs/handover-0905-final origin/main && echo "YES ancestor"
YES ancestor
$ git diff --stat origin/main...origin/docs/handover-0905-final
(пусто)
```

Своих коммитов — 0, вершина ветки является ПРЕДКОМ `main` (уже влита целиком, отставание
28 коммитов — расстояние `main` от точки, где ветка когда-то отделилась). Diff с `main` —
пуст. Ничего не теряется при удалении: содержимое дословно доступно в `main` по тому же SHA.

## 5. `docs/handover-s7-fix` — СНЕСТИ

```
$ git rev-parse origin/docs/handover-s7-fix
821a41d1a013373a5e09fb45331144846132cb27
$ git merge-base --is-ancestor origin/docs/handover-s7-fix origin/main && echo "YES ancestor"
YES ancestor
$ git diff --stat origin/main...origin/docs/handover-s7-fix
(пусто)
```

Тот же случай: 0 своих коммитов, вершина — предок `main`, diff пуст. Спас-реф не нужен —
доказательство предковости ЕСТЬ спас-реф (SHA существует в истории `main`, `git show
821a41d` работает после удаления ветки ровно так же, как до).

## 6. `feat/M-64-export-contract` — ВЕСТИ

```
$ git show origin/feat/M-64-export-contract:milestones/M-64-export-contract.md | sed -n '3p'
**Статус:** **BLOCKED** (rev3, 2026-08-11) — ждёт миграции форм в `crates/contracts`
(`К1`/`К6b`, см. §2.1). **Решение founder'а 2026-08-11: сперва миграция, гейт строится сразу
```

Блокировка — внешняя (решение founder'а: сначала перенос типов `Snapshot`/`Frame`/
`SeriesBundle`/`Selector`/`Cursor`/`OhlcvRow`/`ServeMsg` в `crates/contracts`, потом гейт).
Проверил, состоялся ли перенос за прошедший месяц:

```
$ grep -n 'pub struct Snapshot\|pub struct Frame\|pub struct SeriesBundle\|pub struct Selector\|pub struct Cursor\|pub struct OhlcvRow\|enum ServeMsg' crates/contracts/src/*.rs
(пусто — ни одного совпадения)
$ grep -n 'П-012' docs/PENDING-SIGNATURE.md | head -1
538:## П-012 — ЗАКРЫТО 2026-08-17 РЕШЕНИЕМ АРБИТРА: два класса, различаемые ПРОГОНОМ
```

Governance-вопрос (`П-012`, сколько церемонии для форм экспорта) закрыт арбитром 17.08, но
сама миграция типов НЕ произошла — блокировка подлинная и сегодня. Ветка несёт только
спеку + 3 критики (975 строк, ни строки кода), риска устаревания кода нет. **Что осталось:**
дождаться К1/К6b (не в этой ветке, а в отдельном предмете миграции).

## 7. `feat/M-72-subscription-terminality` — ВЕСТИ

```
$ git log -1 --format='%B' origin/feat/M-72-subscription-terminality | head -1
critique(M-72): round 3 REJECT — terminality artifacts incomplete [critic]
$ git show origin/feat/M-72-subscription-terminality:research/critiques/C-200-M-72-subscription-terminality-round3.md | sed -n '6p'
verdict: REJECT
$ git diff --name-only origin/main...origin/feat/M-72-subscription-terminality | grep '^crates/'
crates/gateway-serve/tests/red_ws_terminality_entrypoint.rs
crates/gateway/tests/red_egress_cap_paths.rs
crates/gateway/tests/red_pump_midstream_failure.rs
crates/gateway/tests/red_snapshot_cursor_honesty.rs
```

Диф в `crates/` — ТОЛЬКО тестовые файлы (RED ещё не дошёл до dev, plan-time REJECT).
Cargo-check на слиянии не показателен: COMPILE-RED здесь — заявленный дизайн, не дефект
(коммит `2a701eb`: «гейт различает вакуум и COMPILE-RED»). Круг ограничен арбитром:

```
$ git show origin/feat/M-72-subscription-terminality:research/critiques/C-200-M-72-subscription-terminality-round3.md | sed -n '116,126p'
## Conditions for round 4
1. Close B-2 with a live-neighbour socket oracle.
2. Close all B-3 gate defects, including the baseline-aware mutation outcome.
3. Declare the terminal T2 boundary verbatim and permit `wire_v1.rs`.
4. Re-plan the TD-177 second half: task 3 cannot prescribe only generation equality
   while the sentinel RED remains red.
5. Restore CI formatting, commit, and push the amended artifact set.

Per A-029 §2, round 4 is final. A repeat of R-4 sign blindness or B-3's
gate-without-oracle class returns the subject to founder; do not open a fifth loop.
```

**Что осталось:** ровно один круг (round 4, финальный по `A-029` §2), пять именованных
пунктов выше. Провал round 4 по тому же классу дефекта возвращает предмет founder'у —
арбитр это уже назначил, не моё решение.

## 8. `feat/M-76-raw-output-barrier` — ВЕСТИ

```
$ git log --oneline origin/main..origin/feat/M-76-raw-output-barrier
8e30921 spec(M-76): барьер подлинности выписок — гипотеза A ОТВЕРГНУТА, B подтверждена [architect]
$ git diff --stat origin/main...origin/feat/M-76-raw-output-barrier
 milestones/M-76-raw-output-barrier.md | 185 ++++++++++++++++++++++++++++++++++
 1 file changed, 185 insertions(+)
$ git show origin/feat/M-76-raw-output-barrier:milestones/M-76-raw-output-barrier.md | sed -n '1,6p'
# M-76 — барьер подлинности выписок: показанное производится показанной командой
**Статус:** PROPOSED (2026-09-01, architect). **Запускается ПОСЛЕ `M-45`** — решение
founder'а: `M-45` теряет суб-секундную историю каждую секунду, этот предмет стоит токенов.
```

`M-45`, от которого зависела очередь, закрыт:

```
$ sed -n '3p' milestones/M-45-persist-l2delta.md
**Статус: ✅ ЗАКРЫТ 2026-09-01.** Реализация и раскатка в `main` (PR #140 `a72ca04`), вердикт
```

Предмет разблокирован, но это чисто спека (одна гипотеза проверена и отвергнута, вторая
подтверждена) — RED-набора и `verify_M-76.sh` ещё нет, critic-круг не начинался.
**Что осталось:** написать RED-тесты + acceptance-скрипт, пройти plan-time critic; порядок
относительно `M-72`/`M-74` — решение founder'а, не моё (`docs/workflow/session-handover-2026-09-04.md:259`
называет очередь открытым вопросом).

## 9. `feat/harness-milestone-shape` — ВЕСТИ

```
$ git log -1 --format='%B' origin/feat/harness-milestone-shape | head -1
docs(critic): C-179 — REJECT harness milestone-shape round 8 [critic]
$ git show origin/feat/harness-milestone-shape:research/critiques/C-179-harness-milestone-shape-r8.md | sed -n '1,7p'
<!-- GATE-META
milestone: C-101
audited_repo: a3ka/hft-platform
audited_base: c4cfb8564fb5549060762c7056485065557afee0
audited_head: dee67f12e7fb49ce18e779fd6e21cd202e8a60b5
verdict: REJECT
-->
```

`audited_head` совпадает с вершиной ветки — вердикт СВЕЖИЙ, круг не начат заново после него.
Диф не трогает `crates/**` (харнесс-трек, `docs/workflow/harness-track.md` — только
`scripts/check_milestone_shape.sh`, проба, CI-джоб):

```
$ git diff --name-only origin/main...origin/feat/harness-milestone-shape | grep '^crates/'
(пусто)
$ bash -n scripts/check_milestone_shape.sh scripts/tests/red_milestone_shape.sh; echo exit=$?
exit=0
```

Синтаксически валиден; логическая полнота — предмет самого REJECT (round 8). **Отдельная
находка, не названная ни одним из восьми вердиктов:** этот предмет прошёл 8 REJECT-раундов
подряд по одному предмету — втрое больше порога арбитража (`gates.md` §0.2: «три круга по
одному предмету в любой комбинации вердиктов» ⇒ арбитр обязателен), а `research/arbitration/`
на этой ветке пуст:

```
$ git diff --name-status origin/main...origin/feat/harness-milestone-shape -- research/arbitration
(пусто — ни одного файла)
```

**Что осталось:** пять пунктов из `C-179` (paste-ready промпт вердикта — B-14/B-15,
пересчёт 95/42, обе точные формы CI-события, живой мутант). Но прежде — сам факт 8 кругов
без арбитра стоит предъявить founder'у отдельно: либо круг 9 закрывает предмет, либо
созывается арбитр по правилу, которое уже должно было сработать на круге 3.

## ПРЕДЕЛЫ РАЗБОРА

- **Не гонял `cargo test --workspace`/`clippy`/CI-паритет ни на одной ветке** — только
  точечный `cargo test -p <crate> --no-run` там, где диф трогал `crates/**` (`M-67`, попытка
  merge-preview на `M-70`). Полный CI-паритет — работа PR-гейта reviewer'а, не этого разбора;
  задание просило именно «собирается ли», не «проходит ли весь CI».
- **`docs/M-70-rev2` merge-preview остановлен на ПЕРВОМ конфликте** (`crates/gateway/src/lib.rs`).
  Не проверял, есть ли ВТОРОЙ конфликт дальше в этом же файле или в других файлах diff'а —
  `git merge --abort` был вызван сразу после обнаружения, чтобы не оставлять дерево в
  конфликтном состоянии дольше необходимого. Дальнейшие конфликты (если есть) увидит тот, кто
  будет разрешать этот.
- **Не проверял `docker-compose.yml`-диф `M-70` на конфликт отдельно** — merge остановился
  раньше на `lib.rs`; после разрешения первого конфликта `git merge --continue` может вскрыть
  второй в compose-файле, который эта сессия не видела.
- **`M-73`/`M-74` (test-only diff) и `M-72` (test-only diff) не прогонялись через
  `cargo test --no-run`** — обе ветки в plan-time REJECT/WIP статусе, и по норме
  (`testing.md`, COMPILE-RED допустим как заявленный дизайн) собираемость тестов ДО дозаписи
  дальнейших задач не является диагностическим сигналом; сочтено избыточным для вопроса
  «сносить/вести/переделывать».
- **Не проверял прод (ssh на VPS)** — ни одна из девяти веток не касается кода, исполняемого
  прод-процессом (harness-трек и plan-time спеки, кроме M-70/M-73, которые ещё не влиты) —
  §8 деплой-гейт неприменим к недеплоенной работе.
- **Численный порог «дешевле переписать» (ПЕРЕДЕЛАТЬ) не сработал ни на одной ветке.**
  Самое большое отставание (`feat/M-64-export-contract`, 1259 коммитов) несёт всего 975
  строк чистой спеки без кода — расхождение с `main` НЕ создаёт риска конфликта, потому что
  предмет ещё не тронул код. Отставание само по себе не мера цены переделки; мерой было бы
  число строк diff'а, которые придётся отбросить/переписать — на всех девяти ветках эта
  цифра осталась в диапазоне «дни, не недели» по объёму (975–4838 строк diff, без учёта
  тестовых COMPILE-RED заготовок, которые переживают переписывание тривиально).
- **Не оценивал качество аргументации внутри REJECT-вердиктов** (правы ли критики по
  существу) — задание требовало установить СОСТОЯНИЕ веток, не пересуживать уже вынесенные
  гейты. Единственное исключение — находка про 8 кругов без арбитра (`feat/harness-milestone-shape`),
  которая не вердикт по существу, а нарушение процедуры гейта.
