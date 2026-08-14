# R-079 — TD-141 документная половина: PR-гейт и merge в `main`

**Роль:** PR-time reviewer (`gates.md` §4, UNCONDITIONAL).
**Дата (UTC):** 2026-08-14.
**Предмет:** `origin/docs/TD-141-fa-sync` @ `2979144` (два коммита: `9687a3a` — правка
`docs/fa/journal.md` §I.12; `2979144` — вердикт `R-078`).
**База слияния:** `origin/main` @ `1a06c08`.
**Предшествующий гейт:** `R-078` — перепроверка `gates.md` §9, APPROVED с NOTE.

## ВЕРДИКТ: APPROVED — merge в `main` разрешён

Блокирующих находок нет. Две NOTE ниже — не условия merge'а; NOTE-2 адресована architect'у
как кандидат в долг.

**Все гейты прогнаны мной заново, а не перенесены из `R-078`/handoff'а.** Совпадение цифр с
`R-078` — результат независимого повтора, а не цитирование: сырой вывод в Done Block ниже.
Прогон выполнен на ДЕРЕВЕ СЛИЯНИЯ (`gates.md` §8 «документ проверяется на дереве слияния»),
временный worktree `/tmp/hft-rev-td141fa` от `origin/main` @ `1a06c08` + `--no-ff` merge.

## Block-scope

Диф к базе слияния — ровно два файла, обе зоны законны:

```
$ git show --numstat --format='' <merge>
25	10	docs/fa/journal.md                          # architect: docs/ — scope-guard §Таблица владения
232	0	research/reviews/R-078-td141-fa-sync-recheck.md  # артефакт гейта §9 — gates.md §4 таблица
```

Ни `crates/**`, ни `contracts/**`, ни `scripts/**`, ни `milestones/**`. Превышения зоны нет.

## Block-C (contract governance)

`crates/contracts/**` дифом НЕ ЗАТРОНУТ — contract-RFC не требуется, авто-REJECT §4 не
применяется. Проверено `git diff --name-status`, не памятью.

## Block-risk

**risk-critic НЕ требуется, и это проверено, а не предположено.**

- `gates.md` §5 привязан к путям `crates/risk|killswitch|oms|venue-*|contracts` — диф их не
  трогает (см. Block-scope).
- `gates.md` §9 добавляет risk-critic для документов safety-пути: `docs/fa/risk.md`,
  `docs/fa/killswitch.md`, `docs/fa/oms.md`, `RK-I-*`/`INTG-I-*`, анти-оверфит гейт §6.
  Предмет — `docs/fa/journal.md`, инвариант `JR-I-11`. Замер: добавленные/удалённые строки,
  упоминающие `RK-I-`/`INTG-I-`, — **0** (команда в Done Block).

Правка ОЖЕСТОЧАЕТ, а не ослабляет: снимает carve-out «инвариант держится КРОМЕ `recover`».
Направление изменения — от послабления к его отмене, то есть класс, ради которого §9 и
заводит risk-critic, здесь не возникает.

## Block-DoneBlock: утверждения документа о коде — перепроверены замером

FA заявляет три факта и сама даёт команды сверки. Все три исполнены мной на дереве слияния:

| утверждение FA | мой замер | сходится |
|---|---|---|
| `check_monotonic_paths` в `lib.rs` — два вызова, `read_all` и `recover` | `lib.rs:447` (внутри `read_all`, объявлен `:440`), `lib.rs:473` (внутри `recover`, объявлен `:464`); всего 2 | ✅ |
| `grep -c recover` в оракуле → 18 | 18 | ✅ |
| оракулы `MN-9`/`MN-10`/`MN-11` существуют | `red_stitch_monotonic.rs:193/231/245` | ✅ |

Сверх заявленного — проверено то, чего `R-078` не проверял отдельно: **guard стоит ДО
tolerant-чтения, а не рядом с ним.** `crates/journal/src/lib.rs:473` — `?` на
`check_monotonic_paths` исполняется прежде, чем `:474-477` создаёт и наполняет `Vec<Event>`.
Значит на немонотонном каталоге функция отдаёт `Err`, а не частичный результат — ровно то,
что документ обещает оператору в приписке «⚠️ Замечание для оператора».

Оракул прогнан ЦЕЛИКОМ, а не отфильтрованно по `recover` (как в `R-078`) — чтобы увидеть, не
куплено ли закрытие `recover` ценой соседних путей: **11 passed, 0 failed**, включая
`MN-1`/`MN-2`/`MN-3` (`read_all`/`stream`/`readable_floor`).

## Проверка на ЗАВЫШЕНИЕ покрытия — главный риск этого диффа

Диф удаляет предупреждение и пишет «инвариант держится на ВСЕХ перечисленных путях без
исключений». Это ровно тот класс утверждения, который `R-070` Н-2 поймал трижды подряд,
поэтому проверялся отдельно:

1. **Таблица §I.12 не содержит строк `ОТСУТСТВУЕТ`** — после правки все 9 строк несут guard
   и оракул (либо явную ссылку на набор). Формулировка «всех ПЕРЕЧИСЛЕННЫХ» согласована с
   таблицей и намеренно не расширена до «всех публичных функций».
2. **Полнота таблицы проверена по её собственному критерию.** Команда деривации из FA
   (`grep -nE "^\s*pub fn" crates/journal/src/{lib,segments}.rs`) даёт **42** кандидата
   против 9 строк таблицы. Остаток просмотрен вручную на предмет несшивающего guard'а:
   писательские методы `Journal`, аксессоры, `fingerprint`/`declare_legacy`/`legacy_meta_path`,
   `verify_cold_copy`/`prune_segment`/`free_bytes`/`storage_status`, `retention_plan`/
   `retention_execute`, `compact_segment`/`compact_closed_segments` — каталог в
   последовательность событий не сшивают.
3. **Единственный найденный сшивающий путь вне таблицы — приватный и ЗАГУАРЖЕН.**
   `crates/journal/src/segments.rs:2605` `fn readable_floor` (без `pub`) зовёт
   `iter_segments_sorted` на `:2607` и немедленно `check_monotonic_paths` на `:2608`;
   оракул — `MN-3`, зелёный. Отсутствие его в таблице законно: таблица деривируется по
   `pub fn`, а функция приватная.

Вывод: завышения покрытия правка не вносит. Прежняя редакция ЗАНИЖАЛА покрытие после merge
`362784a`; новая не ушла в противоположную сторону.

## Гейты харнесса (на дереве слияния)

`verify_design_claims.sh` в ОБЕИХ формах, `docs-freeze` §11, `artifact-ids` §12,
`protected-artifacts` §9 — все exit=0. Сырой вывод — в Done Block.

Замок §11 не задет: `docs/fa/**` в запертую зону (`.claude/**`, `CLAUDE.md`,
`docs/04-workflow.md`) не входит, токен `FOUNDER-APPROVED` не требуется — и `check_docs_freeze.sh`
на диапазоне `1a06c08..HEAD` это подтвердил самостоятельно (exit=0).

Идентификатор `R-078` коллизии не даёт (`check_artifact_ids.sh` exit=0); мой номер `R-079`
взят механизмом `next_artifact_id.sh R`, не выбран.

CI на `main` перед merge зелёный — не мержу поверх красного:
```
$ gh run list --branch main --limit 3
completed	success	docs(handoff): §0bis …	CI	main	push	31847518441
completed	success	docs(review): R-077 §10 …	CI	main	push	31843157253
completed	success	Merge branch 'docs/TD-141-recover-red' …	CI	main	push	31841876307
```

## NOTE-1 — отступление от предписанной модели в гейте §9 (принято, зафиксировано)

`gates.md` §9 предписывает перепроверку уставной правки **независимым Fable-агентом**
(«модель та же (Fable)»). `R-078` выполнен **codex** и НАЗЫВАЕТ это отступление в своём
теле, а не скрывает.

Отступление принимаю как санкционированное. Основания, предъявляемые, а не подразумеваемые:

1. **Прямое подтверждение founder'а в диспетче** настоящего круга — founder есть
   оркестрационный диспетчер (`CLAUDE.md`) и единственный, кто вправе отступать от уставной
   нормы; `gates.md` §0.1 оставляет волевые решения за ним.
2. **Документальный след в `main` ДО круга:** `docs/SESSION-HANDOFF.md` §0bis, раздел
   «ИНСТРУМЕНТ ЗАПУСКА CODEX», перечисляет задачу `fa141-recheck` среди готовых —
   то есть codex-маршрут именно для этой перепроверки был предусмотрен заранее, а не
   подставлен постфактум.
3. **Существо §9 соблюдено:** независимость (не автор правки), свежий контекст, покрытие
   пунктов (а) утверждения о коде командой на дереве слияния, (б) полномочия/зона/замок,
   (в) связность ссылок — `R-078` §§A-F. Я перепроверил (а) самостоятельно — сходится.

**Почему это записано, а не пропущено молча.** §9 объявлен `COGNITIVE-ONLY`: механического
барьера у нормы нет, и единственное, что отличает санкционированное отступление от эрозии
правила, — письменный след. Прецедент «codex вместо Fable» не должен становиться
умолчанием: следующий круг §9 без явной санкции founder'а обязан идти Fable-агентом.

## NOTE-2 — шаг «42 публичных функции → 9 строк таблицы» не механизирован (кандидат в TD)

Не блокер и не дефект этой правки — свойство места, которое она наследует.

FA объявляет таблицу §I.12 ПРОИЗВОДНОЙ от кода и даёт команду деривации. Но команда выдаёт
42 кандидата, а в таблице 9 строк; фильтр «сшивает ли функция каталог» — человеческое
суждение, не проверяемое ничем. Именно рассинхрон этого перечня с кодом дал `R-070` Н-2,
`TD-121` и `TD-138`, то есть класс уже срабатывал трижды.

В этом круге я закрыл разрыв РУЧНЫМ просмотром всех 42 (см. выше) — ровно тот способ, который
не воспроизводится следующим агентом. Развязка (например, канарейка, требующая от каждой
`pub fn`, чей путь достигает `iter_segments_sorted`/`segments_counted`, строки в таблице) —
**зона architect'а**: reviewer описывает дефект, фикс проектирует architect (`gates.md` §4,
граница reviewer↔architect).

## Условие APPROVED

Условий к исправлению нет. Merge выполняется этим же вердиктом.

## Done Block

```
$ git worktree add --detach /tmp/hft-rev-td141fa origin/main
HEAD is now at 1a06c08 docs(handoff): §0bis — полное состояние на конец 14.08 для новой сессии [architect]

$ git merge --no-ff --no-commit origin/docs/TD-141-fa-sync
Automatic merge went well; stopped before committing as requested

$ git status --porcelain
M  docs/fa/journal.md
A  research/reviews/R-078-td141-fa-sync-recheck.md

$ git show --numstat --format='' <merge-commit>
25	10	docs/fa/journal.md
232	0	research/reviews/R-078-td141-fa-sync-recheck.md

$ grep -n "check_monotonic_paths" crates/journal/src/lib.rs
447:    segments::check_monotonic_paths(dir, &segs, &mut ops)?;
473:    segments::check_monotonic_paths(dir, &segs, &mut ops)?;
# владельцы: :440 pub fn read_all · :464 pub fn recover

$ grep -c recover crates/journal/tests/red_stitch_monotonic.rs
18

$ grep -nE "^fn mn_(9|10|11)" crates/journal/tests/red_stitch_monotonic.rs
193:fn mn_9_recover_refuses_non_monotonic_catalogue() {
231:fn mn_10_recover_reads_monotonic_catalogue() {
245:fn mn_11_recover_boundary_catalogues_are_not_monotonicity_violations() {

$ git diff --unified=0 origin/main -- docs/fa/journal.md | grep -cE '^[+-].*(RK-I-|INTG-I-)'
0

$ cargo test -p journal --test red_stitch_monotonic 2>&1 | tail -3
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.85s
TEST_EXIT=0

$ grep -cE "^\s*pub fn" crates/journal/src/lib.rs crates/journal/src/segments.rs
crates/journal/src/lib.rs:9
crates/journal/src/segments.rs:33

$ sed -n '2605,2608p' crates/journal/src/segments.rs
fn readable_floor(dir: &Path) -> io::Result<ReadableFloor> {
    let mut ops: SegmentOps = 0;
    let segs = iter_segments_sorted(dir)?;
    check_monotonic_paths(dir, &segs, &mut ops)?;

$ bash scripts/verify_design_claims.sh 2>&1 | grep -E "^VERDICT"; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | grep -E "^VERDICT"; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0

$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_artifact_ids.sh; echo exit=$?
OK: ни один коммит диапазона 1a06c08..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_protected_artifacts.sh; echo exit=$?
OK: защищённые артефакты целы на HEAD (1a06c08..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ bash scripts/next_artifact_id.sh R
R-079
```

Пруф деплой-гейта `gates.md` §8 (CI + Deploy + eyes-on прода) дописывается в §10 этого файла
после push'а в `main`.
