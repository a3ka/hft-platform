# M-47 (TD-083, P0) — engine-dev report

**Milestone:** `milestones/M-47-td083-push-seek.md` · **Branch:** `feat/TD-083` ·
**HEAD:** `45461bf` (task #3-4) поверх `0145db0` (task #1 rev2) поверх `e722d4a` (task #1-2, rev1,
**содержит регрессию, исправлена в rev2 — см. §2**).

## §1 — Что сделано

### Task #1 — `gateway::frames_since_with_stats`
Добавлена аддитивно в `crates/gateway/src/lib.rs`, сигнатура ровно по RED-оракулу:
`(dir, filter, sel, after, max_events) -> io::Result<(Vec<Frame>, Cursor, ReadStats)>`.

### Task #2 — SEEK
`frames_since_with_stats` использует `journal::stream_from(dir, filter, after.upto_seq)` —
сегментный skip, симметрично `snapshot_from_checkpoint`. `red_push_seek_bounded.rs` (O-1/O-2)
GREEN: тик у хвоста открывает ≤3 сегмента независимо от длины журнала.

### Task #3 — `spawn_blocking`
`crates/gateway-serve/src/lib.rs`, push-цикл `run_authorized_session`: блокирующий
`serve::frames_msgs` теперь ВСЕГДА идёт через `tokio::task::spawn_blocking` — на
`current_thread`-рантайме (прод, `/proc/1/task=1`) синхронный вызов раньше монополизировал
единственный поток целиком, замораживая accept-loop и все остальные WS-сессии на время чтения
(root cause 2, `R-025`).

### Task #4 — disconnect независимо от pending read
`pending_read: Option<JoinHandle<..>>` хранит текущее в-полёте чтение МЕЖДУ итерациями
`select!`. `stream.next()` — ОТДЕЛЬНАЯ ветка select! на каждой итерации вне зависимости от того,
идёт ли сейчас блокирующее чтение — уход клиента детектируется немедленно, а не только после
завершения текущего чтения. Новый тик не планируется, пока предыдущее чтение не завершилось
(`if pending_read.is_none()`) — backpressure, не очередь параллельных чтений журнала.

## §2 — ГЛАВНАЯ НАХОДКА: task #1-2 (rev1) ломает VWAP-корректность — откачено

Первая реализация (commit `e722d4a`) сделала `frames_since` тонкой обёрткой над
`frames_since_with_stats` (делегирование), как буквально написано в milestone §Tasks:
«Существующий `frames_since` оставить как тонкую обёртку». Это ЛОМАЕТ три sacred-оракула:

```
red_gateway_live_eq_replay.rs::mid_stream_snapshot_completeness_merges_same_bucket — FAILED
red_gateway_window.rs::windowed_live_eq_replay* (3 теста)                          — FAILED
red_ws_protocol.rs::o3_frames_converge_to_latest                                   — FAILED
```

Во всех случаях расходится ИМЕННО `vwap`-ряд (значения меньше корректных). Пример
(`o3_frames_converge_to_latest`, изолированный прогон `git stash` против ТОЛЬКО commit `e722d4a`):

```
left  (seek):  vwap: [(.., 6500500000000), (.., 6503666666666), ...]   ← НЕДОСЧИТАН
right (full):  vwap: [(.., 6500500000000), (.., 6510000000000), ...]   ← корректно
```

### Корневая причина (структурная, не баг реализации)

`Reducer::seed_vwap` (`crates/gateway/src/lib.rs:774`) кормит `VwapAcc` — **since-genesis**
аккумулятор (`sum_pv`/`sum_v` с начала журнала, БЕЗ per-тик сброса) — вызовом `apply_vwap(event,
emit=false)` для КАЖДОГО события `seq <= after`. `Snapshot::apply` (`:1350`) мёржит `vwap`-ряд
через `BTreeMap::extend` — то есть ЗАМЕНОЙ по ключу `time_s`, не инкрементом. Значит значение
`vwap` в возвращённом `Frame` ОБЯЗАНО быть абсолютно корректным (since genesis), а не локальным
вкладом текущего кадра.

`journal::stream_from(after.upto_seq)` делает СЕГМЕНТНЫЙ skip: сегменты, целиком лежащие
`<= after`, никогда не попадают в стрим — `seed_vwap` их не видит вообще, а не «отфильтровывает
после чтения». Milestone §Tasks task #2 утверждает «`reduce_event_stream` отфильтрует остаток,
семантика сохраняется» — это ВЕРНО для дельты (новых событий `> after`), но НЕВЕРНО для
seed-шага: там нечего фильтровать, событий просто нет в стриме. Это факт, не деталь реализации —
никакая альтернативная реализация `frames_since_with_stats` с ЭТОЙ сигнатурой (без
персистентного между вызовами состояния/чекпоинта) не может одновременно быть (a) seek-bound
(`segments_opened <= 3`, O-1/O-2) и (b) VWAP-корректной (since-genesis) для ПРОИЗВОЛЬНОГО
`after` — это два взаимоисключающих требования при stateless-контракте функции.

### Фикс (commit `0145db0`)

`frames_since` возвращена к ИСХОДНОЙ реализации (полное чтение с головы через `journal::stream`,
корректно) — НЕ делегирует в `frames_since_with_stats`. `frames_since_with_stats` остаётся
аддитивной (удовлетворяет O-1/O-2 буквально), с явной doc-пометкой ограничения. Push-цикл
`gateway-serve` (task #3/#4) продолжает звать `frames_since` (обёрнутый в `spawn_blocking`), НЕ
seek-версию — иначе прод получил бы БЫСТРЫЙ, но НЕЧЕСТНЫЙ VWAP, что хуже текущего «молчит»
(анти-плацебо, `docs/DESIGN.md`).

**Следствие: root cause 1 (O(история) цена ОДНОГО тика, ≈12 минут на проде) остаётся
НЕУСТРАНЁННЫМ.** Task #3/#4 устраняют ТОЛЬКО root cause 2 (монополизация рантайма). Ожидаемый
эффект на проде после ЭТОГО коммита: accept-loop и ДРУГИЕ WS-сессии больше НЕ замерзают, пока
одна сессия ждёт свой тик; disconnect детектируется независимо от pending read; НО первый
успешный push-тик для данных клиента всё ещё займёт ≈12 минут (O(история)), т.е.
`frames_received > 0` в 15-20-секундном sidecar-окне (§7 milestone'а) СКОРЕЕ ВСЕГО ПО-ПРЕЖНЕМУ
НЕ БУДЕТ достигнут. P0 закрыт ЧАСТИЧНО.

## §3 — Архитектурная находка: `gateway::LiveReducer` не решает root cause 1

`crates/gateway/src/lib.rs:2802` — `LiveReducer` (M-38b, `red_frames_seek_bound.rs`,
milestone `M-38b-checkpoint-reducer.md` task #6, помечена «✅ DONE — резюмируемый редьюсер в
СОЕДИНЕНИИ gateway-serve»). Это ЕДИНСТВЕННЫЙ существующий механизм, который структурно решает
проблему (персистентный между тиками аккумулятор — seed платится ОДИН раз при `resume()`,
дальше `pump()` инкрементально). НО:

1. **`LiveReducer` НЕ подключён к `gateway-serve`.** `grep -rn LiveReducer
   crates/gateway-serve/` находит только ОДИН комментарий (упоминание в контексте другой
   задачи), сам push-цикл вызывает stateless `serve::frames_msgs` → `gateway::frames_since`.
   Task #6 в M-38b помечена DONE, но по факту WIRING в connection handler не сделан —
   несоответствие milestone-таблицы факту в коде.
2. **`LiveReducer::pump` (:2908) САМ вызывает `frames_since` внутри** («для byte-identity с
   эталоном», см. развёрнутый комментарий в коде на этот счёт) — то есть КАЖДЫЙ `pump()` тоже
   читает журнал с головы (O(история)), а `ReadStats`, которые возвращает `pump`, посчитаны с
   ДРУГОГО (действительно bounded) прохода `stream_from`, использованного только для
   обновления `self.reducer`. Тест `pump_at_tail_is_bounded` проверяет ИМЕННО эти (не
   отражающие реальную работу) статы — зелёный, хотя реальная стоимость тика НЕ ограничена.
   Это «оракул врёт о работе» дефект (`testing.md` «фикстура счастливого пути»), не пойманный
   до сих пор, потому что `LiveReducer` никогда не был на прод-пути (иначе R-025 поймал бы его
   тем же способом, каким поймал `frames_since`).

**Рекомендация:** правильный fix для root cause 1 требует переработки `LiveReducer::pump` так,
чтобы дельта строилась из ПЕРСИСТЕНТНОГО `self.reducer` напрямую (apply новых событий на уже
существующее состояние + diff before/after), БЕЗ повторного вызова `frames_since` внутри — и
ТОЛЬКО ПОСЛЕ этого — переключение `gateway-serve`'s push-цикла с `frames_since` на
`LiveReducer::resume`+`pump` (используя УЖЕ существующий `cfg.checkpoint_dir`, симметрично
`snapshot_msg`). Это архитектурное решение (новый RED-оракул на «pump не зовёт frames_since
внутри» + новый инвариант byte-identity через diff, а не через дублирующий stateless вызов) —
вне периметра моих allowed paths/полномочий (не меняю сигнатуры/архитектуру `Reducer`
единолично на sacred correctness-пути). Передаю architect'у как открытый вопрос, НЕ как
`SCOPE VIOLATION REQUEST` в буквальном смысле (я не заблокирован правкой чужого файла) — скорее
как находку, требующую новой RED-спеки прежде чем кто-то полез в `Reducer`/`LiveReducer`.

## §4 — Done Block

```
$ git status --porcelain
(пусто)

$ git log --oneline -3
45461bf feat(M-47): task #3-4 — spawn_blocking вокруг journal-read в push-цикле + disconnect-detection независимо от pending read
0145db0 fix(M-47): task #1 rev2 — frames_since НЕ делегирует в frames_since_with_stats (VWAP regression)
e722d4a feat(M-47): task #1-2 — frames_since_with_stats + seek вместо чтения журнала с головы (TD-083)

$ cargo test -p gateway --test red_push_seek_bounded 2>&1 | tail -6
test td083_push_tick_seeks_instead_of_reading_from_head ... ok
test td083_tick_cost_is_independent_of_journal_length ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p gateway-serve -p gateway 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=124 failed=0 (блоков: 37)

$ cargo fmt --all -- --check 2>&1 | grep "^Diff"
Diff in crates/gateway/tests/red_push_seek_bounded.rs:98:      ← SACRED, не мой файл, не трогаю
Diff in crates/gateway/tests/red_push_seek_bounded.rs:121:     ← SACRED, не мой файл, не трогаю
(мои файлы — crates/gateway/src/lib.rs, crates/gateway-serve/src/lib.rs — fmt-чисты)

$ cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3; echo exit=$?
Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.29s
exit=0

$ bash scripts/verify_M-46.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
PASS  (12 проверок из 13 — T0/T1/T3/T4/T5/T6/T7/T8/T9/T2)
FAIL  T2b fmt — CI упадёт на merge; файлы: crates/gateway/tests/red_push_seek_bounded.rs (SACRED, не мой)
VERDICT: FAIL (1 нарушений)
exit=1
```

**Про `VERDICT: FAIL` в `verify_M-46.sh`:** единственная причина — RED-тест architect'а
(`crates/gateway/tests/red_push_seek_bounded.rs`) не проходит `cargo fmt --check` в ДВУХ местах
(строки 98, 121 — длинные однострочники, которые rustfmt хочет разбить). Это pre-existing
состояние коммита `36da104` (проверено: diff идентичен ДО любых моих правок) — `*/tests/**`
sacred, я НЕ имею права его форматировать/менять (`scope-guard.md`). Это блокирует
`verify_M-46.sh`'s T2b (единый `cargo fmt --all -- --check`, не различает sacred/non-sacred
файлы) и, скорее всего, будущий `verify_M-47.sh` (если унаследует ту же T2b-проверку). Нужна
правка architect'ом своего же теста (`cargo fmt` только на нём, безопасно — форматирование не
меняет семантику теста).
