# M-53 (TD-083, P0) — engine-dev report, второй заход

**Milestone:** `milestones/M-53-td083-push-seek.md` · **Branch:** `feat/TD-083` ·
**HEAD:** `7b8e538` (task #2c) поверх `bd05dcd` (task #2a/#2b) поверх `7fdf624`
(architect: T4b оракулы живости + гейт).

## §1 — Контекст (второй заход)

Первый заход (`research/reports/M-47-engine-dev-report.md`, теперь под этим же именем,
milestone переименован M-47→M-53) нашёл и откатил VWAP-регрессию, передал architect'у
вопрос по root cause 1 (O(история) цена тика) как открытый архитектурный вопрос, не решая
его единолично на correctness-пути. Architect уточнил диагноз (§9 milestone'а):
`gateway::LiveReducer` — решение уже написано, но (а) не подключено к `gateway-serve`,
(б) оба его оракула тавтологичны (byte-identity сравнивала `frames_since` с собой;
boundedness мерила только половину работы). Задача этого захода — сделать `LiveReducer`
реальным (не тавтологичным) решением и подключить его.

## §2 — Task #2a/#2b: `LiveReducer.pump` строит кадр из своего состояния

### Диагноз (замер перед правкой)

`LiveReducer::pump` (старая реализация) буквально звала `frames_since` внутри для
построения кадра («используем frames_since для byte-identity с эталоном» — комментарий в
коде) — то есть КАЖДЫЙ pump всё равно читал журнал с головы (root cause 1 не устранён),
а `ReadStats` считались с ДРУГОГО (действительно bounded) прохода `stream_from`,
использованного только для обновления `self.reducer` — оракул `pump_at_tail_is_bounded`
проверял НЕ ту работу, которая реально строила кадр.

### Решение

Единственное состояние, которое ОБЯЗАНО пережить между тиками, — `VwapAcc.sum_pv`/`sum_v`
(since-genesis аккумулятор без per-тик сброса, `Reducer::apply_vwap`). Математический факт
(зафиксирован в первом заходе): `Reducer::seed_vwap` над всей историей и персистентный
аккумулятор дают ИДЕНТИЧНУЮ арифметику (`i128`, точная) — значит byte-identity с
`frames_since` можно получить БЕЗ вызова `frames_since`, просто считая ТО ЖЕ САМОЕ дешевле.
Всё остальное (ohlcv/cvd/vp/heatmap/book/bubbles) остаётся дельта-only на СВЕЖЕМ `Reducer`
за тик — так уже ведёт себя `frames_since` (её fresh reducer тоже стартует пустым на каждый
вызов, `seed_vwap` эти поля не трогает), и это уже доказано корректным всем набором M-46.

`resume()`:
- с чекпоинтом — забирает `vwap.sum_pv`/`sum_v` ИЗ восстановленного `Reducer`
  (`checkpoint::advance_to` уже накопил его честным `apply`-проходом от START);
- без чекпоинта — `cursor` остаётся `START`, аккумулятор остаётся НУЛЕВЫМ (первый же `pump()`
  естественно построит его через `apply()` с нуля); скан журнала здесь нужен ТОЛЬКО для
  честного `ReadStats.events_decoded == N` (форсинг
  `resume_without_checkpoint_reports_full_replay`) — декодируем и отбрасываем.

  **Важная находка/самофикс:** первая версия этой ветки ошибочно засеивала `sum_pv`/`sum_v`
  через `seed_vwap` по ВСЕЙ истории ДАЖЕ когда `cursor` остаётся `START` — это ЗАДВАИВАЛО
  аккумулятор, потому что последующий `pump()` естественно применяет ТЕ ЖЕ события ещё раз
  (через `apply()`, начиная от START). Поймано локально прогоном
  `pumped_frames_identical_to_frames_since`/`td083_pumped_frames_fold_into_full_replay_snapshot`
  (оба упали на расхождении в `ohlcv.volume`/`vwap`) ДО коммита — исправлено.

`pump()`:
- один вызов ДРЕНИРУЕТ ВЕСЬ доступный backlog чанками по `max_events` (`Vec<Frame>`, не один
  `Frame` за вызов) — иначе `resume()` без чекпоинта не мог бы догнать журнал за конечное
  число тиков: оракул `td083_pumped_frames_fold_into_full_replay_snapshot` даёт 8 тиков по
  100 событий (N=1200) и требует ПОЛНОГО покрытия — математически недостижимо, если pump
  возвращает ровно один кадр на batch;
- единственный проход `journal::stream_from(self.cursor)` строит кадр(ы) И даёт `ReadStats`
  — честная мера ВСЕЙ работы тика (задача #2b: раньше stats брались с отдельного,
  bounded-но-не-того прохода).

### Оракулы (`crates/gateway/tests/red_frames_seek_bound.rs`, 6 тестов)

```
test resume_without_checkpoint_reports_full_replay ... ok
test td083_pumped_frames_fold_into_full_replay_snapshot ... ok
test pump_at_tail_is_bounded ... ok
test pumped_frames_identical_to_frames_since ... ok
test checkpoint_plus_pumped_frames_equals_full_snapshot ... ok
test td083_tick_wallclock_does_not_grow_with_history ... ok
test result: ok. 6 passed; 0 failed
```

`red_push_seek_bounded.rs` (O-1/O-2): 2 passed.

## §3 — Task #2c: `gateway-serve` подключает `LiveReducer`

Push-цикл `run_authorized_session` больше не зовёт `serve::frames_msgs`/`frames_since` —
держит per-connection `LiveReducer`, резюмируемый через `resume()`/`pump()`.

**Найденный при проектировании риск (устранён):** снапшот-при-подключении строился на
`Cursor::LATEST`, вычисленном СВОИМ ВЫЗОВОМ `snapshot_from_checkpoint`; если бы
`LiveReducer::resume()` независимо резюмировал курсор из ТОГО ЖЕ чекпоинта чуть позже
(гонка с фоновым чекпоинтером, который продвигает файл каждые 5-15 мин), снапшот и
live-редьюсер получили бы РАЗНЫЕ курсоры — первый push-тик задвоил бы клиенту данные,
уже пришедшие в снапшоте. Решение: `live` сначала догоняется до текущего хвоста журнала
(`pump(usize::MAX)` в цикле до `frames.is_empty()`), и РОВНО этот курсор (`live.cursor()`)
передаётся в `snapshot_from_checkpoint` вместо отдельного `Cursor::LATEST` — оба
гарантированно синхронны by construction, без гонки.

Вся эта работа (resume + догон + снапшот) — в ОДНОМ `spawn_blocking`, тем же принципом,
что и tick-путь (task #3/#4 первого захода): однопоточный (`current_thread`) рантайм прода
не должен стоять, пока это читается (root cause 2, `R-025`).

**Владение `LiveReducer` между тиками:** `spawn_blocking`-замыкание требует `'static`, то
есть `live` временно ПЕРЕДАЁТСЯ во владение (`live.take()`) на время `pump()` и ВСЕГДА
возвращается назад — в Ok- И в Err-ветке (`PumpOutcome` boxed для `clippy::result_large_err`).
Единственный случай, когда `live` невосстановим, — сам blocking-таск запаниковал (unwind
забирает его с собой, не должно случаться в норме, `pump()` не паникует) — тогда соединение
закрывается явно (клиент переподключится с чистым `resume()`), а не продолжается с
несуществующим `live` (что гарантированно запаниковало бы на следующем тике).

`serve::frames_msgs` НЕ удалена — `red_serve_passthrough.rs` (sacred) тестирует её напрямую;
push-цикл просто перестал её вызывать.

## §4 — Done Block

```
$ git status --porcelain
(пусто)

$ git log --oneline -3
7b8e538 feat(M-53): task #2c — push-цикл gateway-serve использует LiveReducer
bd05dcd fix(M-53): task #2a/#2b — LiveReducer.pump строит кадр из своего состояния
7fdf624 fix(M-53): гейт проверяет и оракулы живости (T4b)

$ cargo test -p gateway --test red_frames_seek_bound --test red_push_seek_bounded 2>&1 | grep -E "^test result"
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p gateway-serve 2>&1 | grep -E "^test result"
test result: ok. 0 passed (unittests lib)
test result: ok. 0 passed (unittests main)
test result: ok. 0 passed (unittests wsprobe)
test result: ok. 4 passed  (red_jwt_verify)
test result: ok. 3 passed  (red_serve_consumes_checkpoint)
test result: ok. 2 passed  (red_serve_passthrough)
test result: ok. 3 passed  (red_serve_window_wiring)
test result: ok. 6 passed  (red_timeframe_guard_startup)
test result: ok. 3 passed  (red_ws_honesty_sessions)
test result: ok. 3 passed  (red_ws_liveness_under_load — T4b, O-3)
test result: ok. 4 passed  (red_ws_protocol)
test result: ok. 5 passed  (red_ws_series_vs_replay)
test result: ok. 2 passed  (smoke_ws)

$ cargo fmt --all -- --check 2>&1 | grep "^Diff in"; echo exit=$?
exit=1   # (нет вывода — grep не нашёл диффов, exit=1 означает "ничего не найдено")

$ cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3; echo exit=$?
Finished `dev` profile [unoptimized + debuginfo] target(s)
exit=0

$ bash scripts/verify_M-53.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
PASS  T0 crates/gateway/tests/red_push_seek_bounded.rs
PASS  T0 crates/gateway/tests/red_frames_seek_bound.rs
PASS  T1 cargo build --workspace
PASS  T2 clippy --workspace --all-targets -D warnings
PASS  T2b cargo fmt --all --check (совпадает с ci.yml:20)
PASS  T3 seek-оракулы GREEN
PASS  T4 LiveReducer-оракулы GREEN (включая td083_* против НЕЗАВИСИМОГО эталона)
PASS  T4 нетавтологичные проверки на месте (эталон = полный реплей, не frames_since)
PASS  T4b оракулы живости GREEN (3 сценария: при живом клиенте / после ухода / после обрыва)
PASS  T5 набор M-46 (gateway-serve) GREEN — регресса нет
PASS  T6 gateway-serve вызывает LiveReducer (не только упоминает)
PASS  T7 crates/contracts/** не тронут
VERDICT: PASS
exit=0
```

Полный `cargo test --workspace` запущен отдельно (агрегированный подсчёт — см. Handoff §C,
добавлен по завершении фонового прогона).

## §5 — Известные ограничения / открытые вопросы

1. **Гонка снапшот↔live-редьюсер устранена архитектурно (см. §3), но НЕ имеет отдельного
   RED-оракула.** Я убедился в отсутствии гонки построением (оба берут курсор ИЗ ОДНОГО
   `live.cursor()`), но не писал новый тест, который бы явно продвигал checkpoint МЕЖДУ
   этими двумя шагами — тест-файлы sacred, я не пишу их сам. Если architect сочтёт нужным
   закрепить это отдельным оракулом — укажу точку врезки (`run_authorized_session`, шаг 6a).
2. **Двойная стоимость на connect:** `resume()+догон` теперь считает vwap-совместимое
   состояние ДВАЖДЫ на подключение (один раз для `live`, один раз внутри
   `snapshot_from_checkpoint` для полного `SeriesBundle`) — раньше было один раз. Стоимость
   ограничена «backlog с последнего чекпоинта» (не O(история)), и происходит ОДИН раз на
   коннект, не на тик — не тот путь, который чинит TD-083, но стоит explicitly отметить.
3. **`serve::frames_msgs` остаётся неиспользуемым в проде** (тестируется отдельно,
   `red_serve_passthrough.rs`) — не удалял, sacred-тест на неё завязан.

## §6 — Прод-масштабный эффект (ожидаемый, не проверен на VPS в этом заходе)

Тик у хвоста (не первое подключение) теперь стоит O(приращение) вместо O(история):
`journal::stream_from(cursor)` пропускает покрытые сегменты целиком (GW-I-11), персистентный
vwap-аккумулятор устраняет необходимость re-seed. На проде (139M событий до курсора,
≈190k событий/с) ожидаемый эффект: тик перестаёт занимать ≈12 минут, push-канал должен
сходиться. Финальная проверка — sidecar-прогон против живого прода (milestone §7),
вне периметра dev-роли этого захода.
