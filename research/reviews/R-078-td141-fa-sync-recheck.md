# R-078 — TD-141 FA sync recheck

**Роль:** независимая перепроверка уставной правки (`gates.md` §9) со свежим контекстом.  
**Дата (UTC):** 2026-08-14.  
**Предмет:** `origin/docs/TD-141-fa-sync` @ `9687a3a`; ветка ответвилась от
`origin/main` @ `d6c9ce1`, затем текущий `origin/main` продвинулся до `1a06c08`.
Актуальный merge-preview (`1a06c08` + `9687a3a`) применился без конфликтов; diff к новой
базе по предмету: `docs/fa/journal.md` `+25/-10`.

## ВЕРДИКТ: APPROVED с NOTE

модель: codex, вместо предписанного Fable; замена санкционирована founder'ом 2026-08-14

Технических блокеров не найдено. NOTE относится к процедуре модели, а не к содержанию
правки: carve-out по `recover` снят по праву, завышения покрытия в новой редакции не нашёл.

## Находки

Блокирующих находок нет.

## Проверка по существу

### A. Утверждения о коде

Все три заявленных автором факта подтвердились на актуальном дереве слияния. Первый замер
делался, пока `origin/main` был `d6c9ce1` и являлся предком `HEAD`; после параллельного
продвижения `origin/main` до `1a06c08` я отдельно создал merge-preview worktree от новой
базы и повторил ключевые команды там.

```
$ git log d6c9ce1..9687a3a --format='%h %an %s'
9687a3a t docs(fa): TD-141 — документная половина: §I.12 приведён к факту после merge 362784a [architect]

$ git log --oneline --left-right --cherry-pick origin/main...9687a3a --
< 1a06c08 docs(handoff): §0bis — полное состояние на конец 14.08 для новой сессии [architect]
> 9687a3a docs(fa): TD-141 — документная половина: §I.12 приведён к факту после merge 362784a [architect]

$ git merge --no-ff --no-commit origin/docs/TD-141-fa-sync   # в worktree от origin/main @ 1a06c08
Automatic merge went well; stopped before committing as requested
```

```
$ grep -n "check_monotonic_paths" crates/journal/src/lib.rs
447:    segments::check_monotonic_paths(dir, &segs, &mut ops)?;
473:    segments::check_monotonic_paths(dir, &segs, &mut ops)?;

$ grep -c recover crates/journal/tests/red_stitch_monotonic.rs
18

$ grep -c "mn_9_recover_refuses_non_monotonic_catalogue\|mn_10_recover_reads_monotonic_catalogue\|mn_11_recover_boundary" crates/journal/tests/red_stitch_monotonic.rs
3
```

### B. Carve-out снят по праву

`recover` реально несёт guard на своём пути, не рядом и не в необязательном helper'е:

- `crates/journal/src/lib.rs:464` - `pub fn recover(...) -> io::Result<Vec<Event>>`;
- `crates/journal/src/lib.rs:466-473` - `iter_segments_sorted` и затем
  `segments::check_monotonic_paths(dir, &segs, &mut ops)?`;
- `crates/journal/src/lib.rs:475-477` - tolerant-чтение сегментов начинается только после
  guard;
- `crates/journal/src/segments.rs:2736-2746` - немонотонный `first_seq` возвращает
  `io::ErrorKind::InvalidData`;
- `crates/journal/src/segments.rs:2771-2773` - `check_monotonic_paths` делегирует в тот же
  `check_first_seq_monotonic`.

Следовательно, удаление предписания «держится на всех путях, КРОМЕ `recover`» не завышает
покрытие. Предыдущая редакция занижала покрытие после merge `362784a`; новая редакция не
ушла в противоположную сторону.

### C. Поведение оператора: `Err`, не silent stitch

Проверено чтением ветки кода и исполнением оракулов. В `recover` оператор `?` стоит на
`check_monotonic_paths` до создания/наполнения результата, поэтому при нарушении функция
возвращает `Err` и не отдаёт частичный `Vec<Event>`. Тест `MN-9` явно матчит `Err`, а ветка
`Ok(evs)` паникует с диагностикой silent stitch.

```
$ cargo test -p journal --test red_stitch_monotonic recover -- --nocapture
running 3 tests
test mn_10_recover_reads_monotonic_catalogue ... ok
test mn_9_recover_refuses_non_monotonic_catalogue ... ok
test mn_11_recover_boundary_catalogues_are_not_monotonicity_violations ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out
```

### D. Номера строк и воспроизводимость таблицы

Новые добавленные строки не вводят новых ссылок вида `lib.rs:<номер>` или
`red_stitch_monotonic.rs:<номер>`.

```
$ git diff --unified=0 origin/main...HEAD -- docs/fa/journal.md | grep -nE '^\+.*(lib\.rs:[0-9]|red_stitch_monotonic\.rs:[0-9]|строк[аи]? [0-9]|:[0-9]{2,})'
{нет вывода, exit=1 — ожидаемое отсутствие совпадений}
```

Команда сверки из FA больше не слепа к методам внутри `impl`: якорь `^\s*pub fn` находит
и верхнеуровневые функции, и `SegmentCatalog::{open,is_fresh,refresh,segments}`. В выводе
есть все строки таблицы `JR-I-11`, включая новый `recover`.

```
$ grep -nE "^\s*pub fn" crates/journal/src/{lib,segments}.rs
crates/journal/src/lib.rs:440:pub fn read_all(dir: impl AsRef<Path>) -> io::Result<Vec<Event>> {
crates/journal/src/lib.rs:464:pub fn recover(dir: impl AsRef<Path>) -> io::Result<Vec<Event>> {
crates/journal/src/segments.rs:197:    pub fn open(dir: &Path) -> io::Result<(Self, SegmentOps)> {
crates/journal/src/segments.rs:241:    pub fn is_fresh(&mut self, dir: &Path) -> io::Result<(bool, SegmentOps)> {
crates/journal/src/segments.rs:552:    pub fn refresh(&mut self, dir: &Path) -> io::Result<SegmentOps> {
crates/journal/src/segments.rs:564:    pub fn segments(&self) -> &[SegmentInfo] {
crates/journal/src/segments.rs:1252:pub fn segments(dir: impl AsRef<Path>) -> io::Result<Vec<SegmentInfo>> {
crates/journal/src/segments.rs:1829:pub fn stream(dir: impl AsRef<Path>, filter: EpochFilter) -> io::Result<EventStream> {
crates/journal/src/segments.rs:1846:pub fn stream_from(
crates/journal/src/segments.rs:1947:pub fn stream_from_at(
crates/journal/src/segments.rs:1984:pub fn stream_from_at_with_catalog(
crates/journal/src/segments.rs:2072:pub fn replay_digest(
... прочие public API не являются строками таблицы JR-I-11
```

### E. Полномочия, замок §11, push-scope

Зона правки не выходит за `docs/fa/**`; замок §11 не задет. Boundary C по добавленным
строкам не пересечена.

Предметный diff ветки к merge-base — один файл:

```
$ git diff --name-status origin/main...HEAD
M	docs/fa/journal.md
```

После того как `origin/main` продвинулся до `1a06c08`, прямой запуск freeze на немерженной
ветке закономерно стал fail-closed: база не предок `HEAD`. Это не гейт предмета, потому что
`gates.md` §8 требует дерево слияния. На временном detached merge-preview commit
(`origin/main` + финальный `HEAD` ветки с этим verdict-файлом) та же проверка проходит:

```
$ git diff --name-status origin/main..HEAD   # временный финальный merge-preview commit
M	docs/fa/journal.md
A	research/reviews/R-078-td141-fa-sync-recheck.md

$ B=$(git rev-parse origin/main)
$ EVENT_NAME=push PUSH_BEFORE=$B bash scripts/check_docs_freeze.sh; echo $?
0

$ B=$(git rev-parse origin/main)
$ EVENT_NAME=push PUSH_BEFORE=$B bash scripts/check_artifact_ids.sh; echo $?
OK: ни один коммит диапазона 1a06c08..HEAD не ввёл второй носитель под занятым идентификатором
0
```

Boundary C по добавленным строкам не пересечена:

```
$ git diff --unified=0 origin/main...HEAD -- docs/fa/journal.md | grep -nE '^\+.*(границ[аы] C|Ctl\(ParamChange\)|candidate|paper|live|фаз|P[0-9]→|P[0-9])'; echo $?
1
```

### F. Связность ссылок

Ссылки и якоря разрешаются в дереве слияния:

```
$ ls research/reviews/R-070* research/reviews/R-072* research/reviews/R-077*
research/reviews/R-070-architect-postreview.md
research/reviews/R-072-architect-postreview-round2.md
research/reviews/R-077-td141-recover-monotonic-guard.md

$ test -f milestones/M-52-journal-hardening.md; echo $?
0

$ git show -s --format='%h %s' 362784a
362784a Merge branch 'docs/TD-141-recover-red' — TD-141: guard монотонности сшивки на пути recover (R-077 APPROVED)

$ rg -n "mn_9_recover_refuses_non_monotonic_catalogue|mn_10_recover_reads_monotonic_catalogue|mn_11_recover_boundary" crates/journal/tests/red_stitch_monotonic.rs
193:fn mn_9_recover_refuses_non_monotonic_catalogue() {
231:fn mn_10_recover_reads_monotonic_catalogue() {
245:fn mn_11_recover_boundary_catalogues_are_not_monotonicity_violations() {
```

## Гейт

Обе формы `verify_design_claims` зелёные. Расхождения по verdict нет.

```
$ bash scripts/verify_design_claims.sh
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
```

## Условие APPROVED

Условий к исправлению нет. Ветка достаточна для входа в `main` после принятия этого
вердикта как санкционированной founder'ом замены Fable-круга.

## Done Block

```
$ bash scripts/next_artifact_id.sh R
R-078

$ git status --porcelain
{пусто до создания verdict-файла}

$ cargo test -p journal --test red_stitch_monotonic recover -- --nocapture
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out

$ B=$(git rev-parse origin/main)
$ EVENT_NAME=push PUSH_BEFORE=$B bash scripts/check_docs_freeze.sh; echo $?
0  # на временном финальном merge-preview commit

$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/docs/TD-141-fa-sync) bash scripts/check_artifact_ids.sh; echo $?
OK: ни один коммит диапазона 9687a3a..HEAD не ввёл второй носитель под занятым идентификатором
0

$ B=$(git rev-parse origin/main)
$ EVENT_NAME=push PUSH_BEFORE=$B bash scripts/check_artifact_ids.sh; echo $?  # на временном финальном merge-preview commit
OK: ни один коммит диапазона 1a06c08..HEAD не ввёл второй носитель под занятым идентификатором
0

$ bash scripts/verify_design_claims.sh
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)
exit=0
```
