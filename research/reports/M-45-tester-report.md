# M-45 — tester: независимый прогон на чистом чекауте

**Дата (UTC):** 2026-08-02
**Worktree:** `/tmp/hft-tester-m45` (detached, `origin/feat/M-45`)
**HEAD проверен:** `6b3515c` — docs(M-45): §Tasks 1-3c ✅ DONE; гейт PASS прогнан лично +
обратная мутация (T3 и O-8 краснеют на финальной реализации)
**Реализация venue-dev:** `23d921b` (перп), `c3b997c` (спот), `7a292f4` (clippy)

## Вердикт

**ACCEPTANCE: PASS** — `verify_M-45.sh` → `VERDICT: PASS`, exit=0, 21/21 проверок PASS,
0 FAIL. Независимо подтверждаю вердикт architect'а на `7a292f4`.

**Главное свойство (T3, гейт merge/раскатки):** без выставленной конфигурации
`L2DELTA_CAPTURE_SYMBOLS` состав эмиссии = ровно `["BTCUSDT"]` в обоих крейтах
(venue-binance, venue-binance-futures) — подтверждено исполняемым тестом
`o3_default_when_config_absent_equals_current_prod_behaviour`, GREEN.

**Находка (не блокирует VERDICT, но расходится с ожиданием):** `cargo fmt --all -- --check`
→ `exit=1` на пинованном toolchain (`rustc 1.97.0`, совпадает с `rust-toolchain.toml`/CI).
18 блоков diff: 5 в `src/` (venue-binance/src/lib.rs:169,218,898;
venue-binance-futures/src/lib.rs:490,700 — зона venue-dev), 13 в `tests/red_l2delta_allowlist.rs`
обоих крейтов (architect-sacred зона). `verify_M-45.sh` fmt не проверяет (T1/T2 — только
build+clippy), поэтому на VERDICT это не влияет, но это реальное расхождение с
`cargo fmt`-чистотой, которое стоит закрыть отдельным quality-fix коммитом (в чей зоне —
src-часть venue-dev, tests-часть architect, по scope-guard).

## Done Block (агрегировано, команды + exit-коды)

```
$ df -h /home
Filesystem      Size  Used Avail Use% Mounted on
/dev/md2        437G  312G  104G  76% /            (104G свободно — гейт диска пройден)

$ git worktree add /tmp/hft-tester-m45 --detach origin/feat/M-45
HEAD is now at 6b3515c docs(M-45): §Tasks 1-3c ✅ DONE; ...  (совпадает с ожиданием)

$ cargo fmt --all -- --check
fmt exit=1   (18 diff-блоков; 5 src venue-dev, 13 tests architect — см. "Находка" выше)

$ cargo build --workspace
build exit=0
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.97s

$ cargo clippy --workspace --all-targets -- -D warnings
clippy exit=0
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.17s

$ cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=766 failed=0 (блоков: 184)
TESTDONE exit=0

$ bash scripts/verify_M-45.sh; echo exit=$?
--- T0: оракулы M-45 на месте (sacred, architect-only) ---
PASS  T0 оракул присутствует: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T0 оракул присутствует: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
--- T1: сборка ВСЕГО workspace ---
PASS  T1 cargo build --workspace
--- T2: clippy по всем таргетам ---
PASS  T2 cargo clippy --workspace --all-targets -D warnings
--- T3: ДЕФОЛТ НЕИЗМЕНЕН — merge не является раскаткой (главный пункт гейта) ---
PASS  T3 venue-binance: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T3 venue-binance-futures: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T3 ожидаемый дефолт в оракуле не подменён: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T3 ожидаемый дефолт в оракуле не подменён: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
--- T4: негативный путь и регистр ---
PASS  T4 venue-binance: allow-list оракул GREEN (23 тестов)
PASS  T4 venue-binance-futures: allow-list оракул GREEN (21 тестов)
--- T5: НЕТ ОБХОДНОГО ПУТИ эмиссии мимо allow-list ---
PASS  T5 venue-binance: единственный вызов l2delta_event — внутри l2delta_emission_for
PASS  T5 venue-binance-futures: единственный вызов l2delta_event — внутри l2delta_emission_for
PASS  T5 хардкод-списка тикеров в venue-src нет
--- T5b: РЕШАЮЩАЯ проверка — поведение реальной точки входа (O-8, C-049) ---
PASS  T5b venue-binance: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T5b venue-binance-futures: O-8 GREEN (6 тестов через реальную точку входа)
--- T6: сырой L2Delta-транслятор не задет ---
PASS  T6 venue-binance: оракул сырого захвата (M-18/CT-RFC-04) остался GREEN
PASS  T6 venue-binance-futures: отдельного red_l2delta_capture нет (покрыт общим прогоном T7)
--- T7: контракты не тронуты ---
PASS  T7 crates/contracts/** не тронут
--- T8: DET-I-1 на смешанном журнале (TD-072) ---
PASS  T8 DET-I-1 GREEN на смешанном журнале (снапшот+дельта)
--- T9: эпоха объявлена, если дефолтный состав меняется ---
PASS  T9 дефолтный состав не менялся ⇒ запись эпохи не требуется

VERDICT: PASS
exit=0
```

## Сверка с Done Block dev/architect

Architect заявлял на `7a292f4`: `VERDICT: PASS`, exit=0, 21 проверка. Мой независимый
прогон на `6b3515c` (тот же код, +1 docs-коммит поверх) даёт идентичный результат:
21/21 PASS, exit=0. Расхождений по существу нет, кроме fmt-находки выше, которую
architect в Done Block не проверял (fmt не входит в acceptance-скрипт).

## Что НЕ проверялось (вне зоны tester)

- §8 деплой-гейт (CI/Deploy на VPS) — задача reviewer после merge, milestone ещё не в main.
- Задача #6 (запись эпохи) — не применима, дефолт не менялся (T9 подтверждает).
