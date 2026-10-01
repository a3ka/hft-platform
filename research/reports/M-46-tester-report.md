# M-46 — tester: независимый прогон на чистом чекауте

**Дата (UTC):** 2026-08-03
**Роль:** tester (read-only на код)
**Чекаут:** `git worktree add /tmp/hft-tester-m46 --detach origin/main`

## Вердикт: **PASS** (на `48c93b3`)

Прогон делался ДВАЖДЫ — важно для истории инцидента, зафиксировано по просьбе architect.

### Прогон #1 — HEAD `c6b4f3b` (первый fetch) → **FAIL**

```
$ git log --oneline -1
c6b4f3b test(TD-078): потолок wall-clock масштабируется под режим сборки (debug ×6)

$ cargo fmt --all -- --check; echo fmt exit=$?
fmt exit=0

$ cargo build --workspace; echo build exit=$?
build exit=0

$ cargo clippy --workspace --all-targets -- -D warnings; echo clippy exit=$?
error: empty line after doc comment
  --> crates/journal/tests/red_floor_work_budget.rs:83:1
   |
83 | / /// отличал бы «ограничено» от «неограниченно» (прод — 158 сегментов, не один).
84 | |
   | |_^
error: could not compile `journal` (test "red_floor_work_budget") due to 1 previous error
clippy exit=101

$ bash scripts/verify_M-46.sh; echo verify exit=$?
... (T0 PASS×3, T1 PASS, T2 FAIL, T2b PASS, T3..T9 все PASS) ...
VERDICT: FAIL (1 нарушений)
verify exit=1

$ cargo test --workspace | aggregate
passed=778 failed=0 (блоков: 188)
```

Находка репортилась architect'у в реальном времени. Причина — регрессия architect'а в
sacred-тесте `crates/journal/tests/red_floor_work_budget.rs` (коммит `c6b4f3b`, TD-078),
**не связанная с M-46/gateway-serve**: пустая строка после doc-комментария →
`clippy::empty_line_after_doc_comments` под `-D warnings`. Все 8 M-46-специфичных пунктов
(T0, T3–T9) были зелёными уже на этой ревизии — предмет milestone'а (read-path сверка с
реплеем) не пострадал ни разу.

### Прогон #2 — HEAD `48c93b3` (после фикса `05a1fab` + docs-коммита) → **PASS**

Architect сообщил о фиксе (`05a1fab fix(TD-078): пустая строка после doc-комментария
красила main на clippy`) и попросил перепроверить на актуальном `origin/main`. Сделан
`git fetch origin && git checkout --detach origin/main` → `48c93b3` (docs-коммит поверх
фикса, `A4 закрыт`).

```
$ git log --oneline -1
48c93b3 docs(process): A4 закрыт (verify_M-46 PASS); инцидент — я покрасил main clippy'ем в
sacred-оракуле, нарушив собственное правило паритета с CI

$ cargo fmt --all -- --check; echo fmt exit=$?
fmt exit=0

$ cargo build --workspace; echo build exit=$?
build exit=0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s

$ cargo clippy --workspace --all-targets -- -D warnings; echo clippy exit=$?
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.63s
clippy exit=0

$ cargo test --workspace | aggregate
passed=778 failed=0 (блоков: 188)

$ bash scripts/verify_M-46.sh; echo verify exit=$?
--- T0: оракулы M-46 на месте (sacred, architect-only) ---
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_series_vs_replay.rs
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_protocol.rs
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_honesty_sessions.rs
--- T1: сборка всего workspace ---
PASS  T1 cargo build --workspace
--- T2: clippy по всем таргетам ---
PASS  T2 cargo clippy --workspace --all-targets -D warnings
--- T2b: fmt — ТА ЖЕ проверка, что в CI ---
PASS  T2b cargo fmt --all --check
--- T3: ГЛАВНОЕ — WS-выдача == независимый реплей по всем 10 сериям ---
PASS  T3 сверка WS↔реплей GREEN (5 тестов)
--- T4: анти-плацебо ---
PASS  T4 фикстура O-1 содержит события книги (L2Snapshot+L2Delta)
PASS  T4 парный vantage на месте (Trade-only ⇒ книжные серии пусты)
--- T5: протокол ---
PASS  T5 протокольные оракулы GREEN
--- T6: честность истории + граница UTC-суток ---
PASS  T6 честность/сессии GREEN
--- T7: контракты не тронуты ---
PASS  T7 crates/contracts/** не тронут
--- T8: харнесс wsprobe собирается как бинарь ---
PASS  T8 бинарь wsprobe собирается
--- T9: рендер порождает НЕПУСТОЙ артефакт ---
PASS  T9 рендер даёт непустую панель с сериями (7730 байт)

VERDICT: PASS
verify exit=0
```

## Артефакт харнесса — глазами (self-test), проверен на ОБОИХ чекаутах, идентичен

```
$ cargo run -q -p gateway-serve --bin wsprobe -- --self-test --out /tmp/tester-m46-panel
wsprobe: schema_version=8 cursor=Some(5) history_start_seq=0 history_truncated=false
         latency_first_snapshot_ms=0 frames_received=0
series lengths: ohlcv=2 cvd=2 vwap=2 depth_series=2 volume_profile=2 heatmap=13 cob=5 volume_bubbles=3

cob (n=5, top 5 each side):
  BID price      size   |   ASK price      size
    65005.00   0.8000   |     65010.00   0.5000
    65000.00   2.0000   |     65020.00   4.0000
    64990.00   3.0000   |
```

`summary.json`:

```json
{
  "schema_version": 8,
  "cursor_upto_seq": 5,
  "history_start_seq": 0,
  "history_truncated": false,
  "latency_first_snapshot_ms": 0,
  "frames_received": 0,
  "series_lengths": {
    "ohlcv": 2, "cumulative_delta": 2, "cvd_session_base": 0, "depth_series": 2,
    "vwap": 2, "volume_profile": 2, "vp_session_max_time_s": 2, "heatmap": 13,
    "cob": 5, "volume_bubbles": 3
  }
}
```

`panel.html`: 7730 байт, непустой (проверено содержимое — не только разметка).

**Проверка COB на осмысленность (не грепом, глазами):** книга НЕ скрещена — лучший bid
65005.00 < лучшего ask 65010.00 на всех уровнях (65005/65000/64990 против 65010/65020),
все размеры положительные. Дефектов не обнаружено.

Служебное поле `cvd_session_base` имеет `series_lengths=0` в обоих прогонах — по спеке
(`milestones/M-46-read-path-probe.md` §2) это `(session_id, base)`-пара, не временной ряд;
длина 0 в self-test-фикстуре не проверялась мной как аномалия (вне зоны T3/T9 verify-скрипта,
которые её не тестируют отдельно) — фиксирую как наблюдение, не как находку.

## Итоговые агрегаты (Done Block см. в ответе tester'а founder'у)

| Проверка | c6b4f3b | 48c93b3 |
|---|---|---|
| fmt | PASS (exit=0) | PASS (exit=0) |
| build --workspace | PASS (exit=0) | PASS (exit=0) |
| clippy --workspace --all-targets -D warnings | **FAIL (exit=101)** | PASS (exit=0) |
| test --workspace | 778 passed / 0 failed (188 блоков) | 778 passed / 0 failed (188 блоков) |
| verify_M-46.sh | **VERDICT: FAIL (exit=1)** | VERDICT: PASS (exit=0) |
| wsprobe self-test артефакт | OK (panel.html 7730B, summary.json полный) | идентично |

## Заключение

Предмет milestone'а M-46 (сквозная сверка WS-выдачи с независимым реплеем по всем 10
сериям `SeriesBundle`, T3/O-2) — подтверждён независимым прогоном на чистом чекауте:
GREEN на обеих ревизиях, дефект не найден. Единственное расхождение (`c6b4f3b` → clippy FAIL)
было вне зоны M-46 (sacred-тест architect'а, TD-078) и уже устранено фиксом `05a1fab` до
закрытия milestone'а. На актуальном `origin/main` (`48c93b3`) все гейты, включая полный
паритет с CI (`fmt`+`clippy`+`build`+`test`), зелёные.

**Handoff:** reviewer.
