# M-54 engine-dev — НАХОДКА: O-1/O-2 vs sacred M-53 `red_frames_seek_bound.rs` конфликтуют структурно

**Статус:** BLOCKED — не подгонял оракул, не трогал sacred-тесты. Задача 1
(`LiveReducer::snapshot()`) НЕ реализована. Задача 2 не начата (зависит от задачи 1).
**Ветка:** `feat/M-54-connect-cost` (detached worktree `/tmp/hft-dev-m54` на
`origin/feat/M-54-connect-cost`, т.к. `feat/M-54-connect-cost` уже занята worktree'ом
architect на `/tmp/hft-arch-state2` — не трогал).
**Коммитов нет.** Рабочее дерево чистое (`git status --porcelain` пусто) — экспериментальный
кандидат, ломающий sacred-тест, откачен (`git checkout -- crates/gateway/src/lib.rs`), не
закоммичен.

## 0. Коротко

O-1 и O-2 (`crates/gateway/tests/red_connect_cost_single.rs`) в качестве фикстуры используют
`resume()` **без чекпоинта** (`ckpt = tempfile::tempdir()`, `advance_to` не вызывается) и требуют,
чтобы **`live.snapshot()` сразу после ОДНОГО ТОЛЬКО `resume()` (без единого вызова `pump()`)**
уже был полным, корректным состоянием с курсором, равным ХВОСТУ журнала.

Это требование **структурно несовместимо** с sacred-тестом M-53
`crates/gateway/tests/red_frames_seek_bound.rs::pumped_frames_identical_to_frames_since`
(и его соседом `td083_pumped_frames_fold_into_full_replay_snapshot`), который проверяет
ПРОТИВОПОЛОЖНОЕ: что именно **`pump()`**, вызываемый ПОСЛЕ no-checkpoint `resume()`, обязан
воспроизвести побатчево (`max_events=100`) ТУ ЖЕ последовательность кадров, что и повторные
`frames_since(cap=100)` от `Cursor::START` — а это возможно только если `resume()` БЕЗ
чекпоинта оставляет `self.cursor == Cursor::START` (текущее поведение, задокументированное
как намеренное в doc-комментарии `LiveReducer::resume`).

Оба требования привязаны к ОДНОМУ И ТОМУ ЖЕ полю: значению, которое возвращает
`LiveReducer::cursor()`. Удовлетворить оба одновременно в no-checkpoint-ветке нельзя.

## 1. Доказательство (эмпирическое, не только рассуждение)

Собрал кандидат-реализацию задачи 1:
- добавил поле `full: Snapshot` в `LiveReducer`;
- в no-checkpoint ветке `resume()` заменил "декодируем и отбрасываем" на реальную свёртку
  (`reduce_event_stream(..., Cursor::START, Cursor::LATEST, usize::MAX)`), `self.cursor`
  выставляется в РЕАЛЬНЫЙ хвост (а не остаётся `Cursor::START`);
- в checkpoint-ветке `full` строится из уже восстановленного `Reducer` (`r.finish()` после
  `r.selector = sel.clone()`, `history_start_seq/truncated` — из `ckpt_header`, курсор ==
  `ckpt_cursor`, без изменения существующего `self.cursor` в этой ветке);
- в `pump()` добавил `for f in &frames { self.full.apply(f); }` — держать `full` в
  синхроне на будущее;
- `pub fn snapshot(&self) -> Snapshot { self.full.clone() }`.

### Результат — M-54 (новые оракулы): GREEN

```
$ cargo test -p gateway --test red_connect_cost_single
running 3 tests
test o3_no_gap_between_snapshot_and_push ... ok
test o2_livereducer_snapshot_equals_independent_replay ... ok
test o1_snapshot_comes_from_state_not_from_journal ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

### Результат — M-53 sacred (`red_frames_seek_bound.rs`): 2 из 6 упали

```
$ cargo test -p gateway --test red_frames_seek_bound
running 6 tests
test resume_without_checkpoint_reports_full_replay ... ok
test td083_pumped_frames_fold_into_full_replay_snapshot ... FAILED
test pump_at_tail_is_bounded ... ok
test pumped_frames_identical_to_frames_since ... FAILED
test checkpoint_plus_pumped_frames_equals_full_snapshot ... ok
test td083_tick_wallclock_does_not_grow_with_history ... ok

---- td083_pumped_frames_fold_into_full_replay_snapshot stdout ----
thread 'td083_pumped_frames_fold_into_full_replay_snapshot' panicked at
crates/gateway/tests/red_frames_seek_bound.rs:325:5:
assertion `left == right` failed: TD-083 O-A: свёртка кадров pump не дошла до конца журнала
  left: Cursor { upto_seq: None }
 right: Cursor { upto_seq: Some(1199) }

---- pumped_frames_identical_to_frames_since stdout ----
thread 'pumped_frames_identical_to_frames_since' panicked at
crates/gateway/tests/red_frames_seek_bound.rs:123:5:
assertion `left == right` failed: GW-I-8/VB-I-2 НАРУШЕН: кадры резюмируемого пути НЕ
байт-идентичны кадрам frames_since. Ускорение, меняющее данные, — это не ускорение, а
расхождение live vs replay.
  left:  [91, 93]              # JSON `[]` — got пуст
  right: [91, 123, "schema_version":8, "from":{"upto_seq":null}, ... ]   # want непуст
```

**Что именно расходится, на каком входе:** оба FAIL — в no-checkpoint фикстуре
(`journal_upto(N=1200)`, без `advance_to`). Причина одна и та же для обоих:
после моего изменения `resume()` УЖЕ полностью свернул журнал (`self.cursor` == реальный
хвост, `upto_seq: Some(1199)`), поэтому ПЕРВЫЙ же `pump()` (`stream_from(1199)`) не находит
НИ ОДНОГО нового события — `frames` пуст с первого вызова. Тест
`pumped_frames_identical_to_frames_since` ожидает, что `pump()` В ЦИКЛЕ воспроизведёт ВСЮ
батчевую разбивку `frames_since(cap=100)` от `START` — она физически исчезла: обрабатывать
уже нечего, состояние уже есть, но НЕ в виде кадров. Аналогично
`td083_pumped_frames_fold_into_full_replay_snapshot` стартует с `snapshot(START)` и ожидает,
что кадры `pump()` доведут его до `snapshot(LATEST)` — кадров нет, `folded.cursor` остаётся
`START`.

Checkpoint-ветка (`checkpoint_plus_pumped_frames_equals_full_snapshot`) НЕ пострадала — она
не завязана на no-checkpoint путь.

## 2. Почему это не «допилить реализацию», а конфликт спецификаций

- `LiveReducer::cursor()` — ОДНО поле, ОДНО значение. Оно одновременно:
  (a) **[sacred, M-53]** служит входом для `pump()`'s `journal::stream_from(self.cursor)` —
      обязано остаться `Cursor::START` после no-checkpoint `resume()`, иначе `pump()` не
      сможет воспроизвести побатчевую разбивку `frames_since` (в этом и есть содержание
      `pumped_frames_identical_to_frames_since` — она проверяет, что РАБОТУ делает `pump()`,
      а не `resume()`);
  (b) **[M-54, O-1+O-2]** обязано РАВНЯТЬСЯ курсору независимого `snapshot(..., LATEST)`
      СРАЗУ после `resume()`, БЕЗ единого вызова `pump()` (в O-1/O-2 `live` даже не `mut` —
      вызвать `pump()` до `snapshot()` там синтаксически нельзя).
- Отвязать «курсор для pump()» от «курсор для snapshot()» не помогает: O-1 явно требует
  `assert_eq!(snapshot.cursor, live.cursor())` — то есть ОБА обязаны быть ОДНИМ значением по
  контракту самого нового оракула.
- Заставить `pump()` игнорировать `self.cursor` и всегда стартовать с `START` для
  воспроизведения `frames_since` — вернёт журнал к повторному полному чтению НА ТОЙ ЖЕ
  сессии (resume читает всё для `full`, затем первый `pump()` читает всё ЕЩЁ РАЗ для кадров)
  — то есть возвращает именно ту двойную стоимость, которую M-54 должен устранить, просто
  сдвинутую в другое место.

## 3. Что НЕ тронуто

- Sacred-тесты (`crates/gateway*/tests/**`) — не редактировались.
- `scripts/verify_M-54.sh` — не редактировался.
- Кандидат-код в `crates/gateway/src/lib.rs` — реализован, ПРОВЕРЕН, затем **откачен**
  (`git checkout -- crates/gateway/src/lib.rs`), так как ломает sacred T5. `git status
  --porcelain` пуст, HEAD не сдвинут (`95f4f04`, тот же коммит, с которого стартовал).
- Задача 2 (`gateway-serve` wiring) не начата — зависит от корректного `snapshot()`.

## 4. Варианты для architect (не мой выбор — RED-оракулы sacred)

1. **Перевести фикстуры O-1/O-2/O-3 на checkpoint-valid путь** (`checkpoint::advance_to`
   перед `resume()`) — там конфликта НЕТ: мой кандидат прошёл
   `checkpoint_plus_pumped_frames_equals_full_snapshot` чисто, и на проде
   `checkpoint_dir` ВСЕГДА сконфигурирован (`serve_config_from_env` doc: «На проде ВСЕГДА
   задан»). No-checkpoint — единственно дев/тест-путь.
2. Явно ограничить свойство O-1/O-2 checkpoint-веткой, а no-checkpoint fallback оставить
   «честно дорогим» (как сейчас) — task 2 (`gateway-serve`) тогда полагается на то, что прод
   всегда идёт через чекпоинт.
3. Любое другое решение, меняющее семантику `pump()`/`cursor()` — трогает sacred
   `red_frames_seek_bound.rs`, что вне моей зоны (scope-guard: тесты — architect-only).

## Done Block

```
$ git status --porcelain
{пусто}

$ git log -1 --oneline
95f4f04 test(M-54): TD-093 — compile-RED оракулы единой стоимости коннекта + verify_M-54.sh...

$ cargo test -p gateway --test red_connect_cost_single 2>&1 | tail -6
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
(получено на ОТКАЧЕННОМ кандидате — этот прогон соответствует НЕ текущему HEAD, а
экспериментальному коду, который был откачен; приведён как доказательство находки, не
как состояние ветки)

$ cargo test -p gateway --test red_frames_seek_bound 2>&1 | grep -E "^test |test result"
test resume_without_checkpoint_reports_full_replay ... ok
test td083_pumped_frames_fold_into_full_replay_snapshot ... FAILED
test pump_at_tail_is_bounded ... ok
test pumped_frames_identical_to_frames_since ... FAILED
test checkpoint_plus_pumped_frames_equals_full_snapshot ... ok
test td083_tick_wallclock_does_not_grow_with_history ... ok
test result: FAILED. 4 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
(тот же экспериментальный код; на текущем HEAD red_connect_cost_single КРАСНЫЙ
compile-RED, как задокументировано в milestone, red_frames_seek_bound — GREEN 6/6,
не тронут)
```

Текущий HEAD (`95f4f04`, без моих правок) — `red_connect_cost_single` не компилируется
(ожидаемый compile-RED, `LiveReducer::snapshot()` не существует), `red_frames_seek_bound`
— GREEN (6/6, не трогал).
