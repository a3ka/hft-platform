#!/usr/bin/env bash
# Acceptance-гейт M-69 — GW-I-14: fail-closed разбор `GATEWAY_WINDOW_MS` (PL-I-5, риск R7).
#
# Невалидное значение единственной ручки, ограничивающей память свёртки live-кокпита, сегодня
# молча даёт БЕЗГРАНИЧНОЕ окно (`crates/gateway-serve/src/lib.rs:740-744`, `.ok()`), то есть
# режим, разваливший прод (TD-020/TD-039). PL-I-5 (`docs/DESIGN.md:940`) требует обратного:
# «отсутствие/невалидность лимита = отказ, не unbounded». Фикс назначен в `docs/08` R7 (CRIT).
#
# Гвард обязан стоять в ДВУХ точках: старт прод-бинаря (оператор с опечаткой не поднимает
# healthy-контейнер) И библиотека (нет байпаса для чекпоинтера M-38b / shared-tailer M-39 /
# research-cli). Ровно конструкция M-47/GW-I-10 на этом же коде.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }

# ── Паритет с CI (gates.md §3): гейт, который зеленее CI, — не гейт. Базовая тройка job'а
# `build-test` целиком; специализированные job'ы (contracts/artifact-ids/docs-freeze/…) в зону
# M-69 не бьют — milestone не трогает ни contracts, ни процессный слой.
step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk cargo fmt --all -- --check
chk cargo clippy --all-targets --all-features -- -D warnings

step "task #1,#2 — GW-I-14 на СТАРТЕ прод-бинаря: мусор/переполнение/отрицательное → Err"
chk cargo test -p gateway-serve --test red_window_guard_startup --quiet

step "task #3 — GW-I-14 в БИБЛИОТЕКЕ: анти-байпас на snapshot/frames_since/replay"
chk cargo test -p gateway --test red_window_selector_guard --quiet

# Анти-байпас по КОДУ (класс TD-019/TD-020 «механизм есть, никто не зовёт»). Гвард обязан жить
# в ТЕЛЕ `validate_selector` — единственной точке, через которую проходят все три публичных
# входа библиотеки. Проверка по телу функции, а не по файлу: `window_ms` встречается в lib.rs
# многократно (поле Selector, fingerprint, window_lo_time_s), и `grep` по файлу был бы зелёным
# ещё до фикса — то есть вакуумной канарейкой.
step "канарейка — гвард окна В ТЕЛЕ validate_selector (а не только в конфиге транспорта)"
chk bash -c "awk '/pub fn validate_selector/,/^}/' crates/gateway/src/lib.rs \
  | sed 's://.*::' | grep -q 'window_ms'"

# Регресс соседнего инварианта: M-47 (GW-I-10) — гвард таймфрейма в обеих своих точках.
step "регресс — GW-I-10 (M-47) не ослаблен: библиотека + старт"
chk cargo test -p gateway --test red_timeframe_session_alignment --quiet
chk cargo test -p gateway-serve --test red_timeframe_guard_startup --quiet

# Регресс M-37: сама проводка env→Selector.window_ms обязана остаться рабочей. Без этого шага
# «фикс» мог бы пройти, просто перестав пробрасывать окно вовсе.
step "регресс — M-37 проводка GATEWAY_WINDOW_MS → Selector.window_ms цела"
chk cargo test -p gateway-serve --test red_serve_window_wiring --quiet

# Регресс оконной арифметики: предмет M-37/VB-I-10, запретный список milestone'а его защищает.
step "регресс — оконная арифметика (VB-I-10) не сдвинута"
chk cargo test -p gateway --test red_gateway_window --quiet

# task #4: док-комментарий обязан перестать описывать дефект как норму. Сегодня
# `crates/gateway-serve/src/lib.rs:676` гласит «None если отсутствует/пусто/не парсится →
# offline unbounded» — то есть документирует ровно то, что PL-I-5 запрещает.
step "task #4 — док-комментарий приведён к факту (не описывает parse-error как offline)"
chk bash -c "! grep -q 'пусто/не парсится' crates/gateway-serve/src/lib.rs"

# Прод обязан остаться рабочим: гвард не имеет права уронить работающий деплой.
# Замер на VPS 2026-08-18: GATEWAY_WINDOW_MS=60000.
step "канарейка — прод-дефолт GATEWAY_WINDOW_MS=60000 цел (docker-compose.yml)"
chk bash -c "grep -qE 'GATEWAY_WINDOW_MS:[[:space:]]*.*60000' docker-compose.yml"

# Полный прогон workspace последним: дорогой шаг не должен маскировать адресные выше.
step "полный прогон workspace (паритет CI: cargo test --all)"
chk cargo test --all --quiet

echo
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAIL проверок красных)"
  exit 1
fi
