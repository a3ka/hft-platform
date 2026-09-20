# M-53 (`TD-083`, P0) — tester report

**Дата (UTC):** 2026-08-03
**Ветка:** `feat/TD-083`
**HEAD проверен:** `7b8e538` — feat(M-53): task #2c — push-цикл gateway-serve использует LiveReducer
**Чекаут:** чистый `git worktree add /tmp/hft-tester-m53 --detach origin/feat/TD-083` (независимо от dev-локального состояния)

## ВЕРДИКТ: PASS

Независимое подтверждение вердикта architect'а (12/12 на его собственном прогоне) на
чистом чекауте. Расхождений не найдено — оба гейта (`verify_M-53.sh`, `verify_M-46.sh`)
зелёные, regression-набор M-46 цел, `cargo test --workspace` без единого FAIL.

## Done Block (агрегированный, per `.claude/rules/commit-discipline.md`)

```
$ git worktree add /tmp/hft-tester-m53 --detach origin/feat/TD-083
Preparing worktree (detached HEAD 7b8e538)
HEAD is now at 7b8e538 feat(M-53): task #2c — push-цикл gateway-serve использует LiveReducer

$ cargo fmt --all -- --check; echo "fmt exit=$?"
fmt exit=0

$ cargo build --workspace 2>&1 | tail -3; echo "build exit=$?"
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.94s
build exit=0

$ cargo clippy --workspace --all-targets -- -D warnings; echo "clippy exit=$?"
clippy exit=0

$ cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=785 failed=0 (блоков: 190)
```

### `bash scripts/verify_M-53.sh; echo exit=$?`

```
--- T0: оракулы M-53 на месте (sacred, architect-only) ---
PASS  T0 crates/gateway/tests/red_push_seek_bounded.rs
PASS  T0 crates/gateway/tests/red_frames_seek_bound.rs
--- T1/T2/T2b: паритет с CI-job fmt+clippy+test (gates.md §3) ---
PASS  T1 cargo build --workspace
PASS  T2 clippy --workspace --all-targets -D warnings
PASS  T2b cargo fmt --all --check (совпадает с ci.yml:20)
--- T3: ГЛАВНОЕ — тик у хвоста ограничен, цена не зависит от длины журнала ---
PASS  T3 seek-оракулы GREEN
--- T4: LiveReducer — проверки НЕ тавтологичны ---
PASS  T4 LiveReducer-оракулы GREEN (включая td083_* против НЕЗАВИСИМОГО эталона)
PASS  T4 нетавтологичные проверки на месте (эталон = полный реплей, не frames_since)
--- T4b: ЖИВОСТЬ сервиса (O-3) — accept-loop не умирает от одного клиента ---
PASS  T4b оракулы живости GREEN (3 сценария: при живом клиенте / после ухода / после обрыва)
--- T5: РЕГРЕСС — весь набор M-46 остаётся зелёным ---
PASS  T5 набор M-46 (gateway-serve) GREEN — регресса нет
--- T6: push-цикл ДЕЙСТВИТЕЛЬНО использует LiveReducer ---
PASS  T6 gateway-serve вызывает LiveReducer (не только упоминает)
--- T7: контракты не тронуты (M-53 read-only, T1 не меняется) ---
PASS  T7 crates/contracts/** не тронут

VERDICT: PASS
exit=0
```

### `bash scripts/verify_M-46.sh; echo exit=$?` (регресс-проверка)

```
--- T0: оракулы M-46 на месте (sacred, architect-only) ---
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_series_vs_replay.rs
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_protocol.rs
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_honesty_sessions.rs
--- T1: сборка всего workspace --- PASS
--- T2: clippy по всем таргетам --- PASS
--- T2b: fmt --- PASS
--- T3: ГЛАВНОЕ — WS-выдача == независимый реплей по всем 10 сериям ---
PASS  T3 сверка WS↔реплей GREEN (5 тестов)
--- T4: анти-плацебо — оракул обязан ДАВИТЬ там, где smoke_ws слеп ---
PASS  T4 фикстура O-1 содержит события книги (L2Snapshot+L2Delta)
PASS  T4 парный vantage на месте (Trade-only ⇒ книжные серии пусты)
--- T5: протокол — авторизация, кадры, окно, чекпоинт --- PASS
--- T6: честность истории + граница UTC-суток (CVD vs VWAP) --- PASS
--- T7: контракты не тронуты --- PASS
--- T8: харнесс wsprobe собирается как бинарь --- PASS
--- T9: рендер порождает НЕПУСТОЙ артефакт ---
PASS  T9 рендер даёт непустую панель с сериями (7730 байт)

VERDICT: PASS
exit=0
```

**Регресс-вывод:** фикс push-пути (LiveReducer вместо `frames_since` с головы) НЕ сломал
сверку WS↔реплей — все 10 серий по-прежнему сходятся с независимым реплеем журнала.
Ровно то, чего требовал `.claude/rules/testing.md` («быстро, но неправда» хуже, чем
исходное «молчит») — здесь и быстро, и честно.

### Харнесс своими глазами — `wsprobe --self-test`

```
$ cargo run -q -p gateway-serve --bin wsprobe -- --self-test --out /tmp/tester-m53-panel
wsprobe exit=0

$ cat /tmp/tester-m53-panel/summary.json
{
  "schema_version": 8,
  "cursor_upto_seq": 5,
  "history_start_seq": 0,
  "history_truncated": false,
  "latency_first_snapshot_ms": 0,
  "frames_received": 0,
  "series_lengths": {
    "ohlcv": 2,
    "cumulative_delta": 2,
    "cvd_session_base": 0,
    "depth_series": 2,
    "vwap": 2,
    "volume_profile": 2,
    "vp_session_max_time_s": 2,
    "heatmap": 13,
    "cob": 5,
    "volume_bubbles": 3
  }
}
```

`latency_first_snapshot_ms=0` — в ожидаемом диапазоне («единицы миллисекунд»).
`frames_received=0` — ОЖИДАЕМО (не ошибка): self-test журнал не растёт после подключения
клиента, кадрам взяться неоткуда. Живость push-кадров проверяется исполняемым оракулом
`red_ws_protocol.rs::o3_frames_converge_to_latest` (внутри verify_M-46.sh T5, GREEN) и
позже — повторным sidecar-прогоном против прода (не в периметре tester'а, §7 milestone'а).

## Сверка с утверждением dev-агента (architect, §9 milestone)

Architect заявил PASS 12/12 на своём прогоне. Независимый прогон на чистом worktree
(`origin/feat/TD-083`, коммит `7b8e538`) даёт тот же результат: 12/12 в `verify_M-53.sh`
(T0×2, T1, T2, T2b, T3, T4×2, T4b, T5, T6, T7) + отдельно verify_M-46.sh 14/14 (T0×3, T1,
T2, T2b, T3, T4×2, T5, T6, T7, T8, T9) + `cargo test --workspace` 785/785 без единого FAIL.
Расхождений не найдено.

## Известное окружение

- `cargo test --workspace` в DEBUG занял ≈13 минут суммарно (доминирует
  `red_floor_work_budget` ~5 мин + прочие prod-scale оракулы `red_*_prodscale`,
  `red_m52_prodscale`, `red_stream_grid` и т.п.) — известно, TD-078, не регрессия.
- В worktree параллельно работала другая сессия (`/tmp/hft-dev-td083b`) — её процессы
  видны в `ps aux`, не пересекаются с моим прогоном (разные PID/бинарники), не трогал.
- Чужие git stash'и (2 шт., из старых сессий) — не трогал.

## Что дальше

Все гейты (T0–T7 в verify_M-53.sh, T0–T9 в verify_M-46.sh, весь workspace test suite)
зелёные на чистом чекауте. Milestone готов к PR-time reviewer гейту
(`.claude/rules/gates.md` §4). RISK-BLOCK не применяется (milestone read-only,
order-egress отсутствует — см. milestone §0 шапку).
