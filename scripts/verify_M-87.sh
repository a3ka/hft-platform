#!/usr/bin/env bash
# verify_M-87.sh — acceptance-гейт milestone'а M-87 «предохранитель выдачи».
# Спека: milestones/M-87-serving-circuit-breaker.md
#
# Решение по КОДУ ВОЗВРАТА (`gates.md` §3). Агрегатор с FAIL-счётчиком: гейт обязан
# перечислить ВСЕ провалы, а не умереть на первом.
#
# Базовая линия снимается КРАСНОЙ до работы dev'а и предъявляется в Handoff.

set -uo pipefail
cd "$(dirname "$0")/.."

FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }
skip() { printf 'SKIP  %s\n' "$1"; }

GW=crates/gateway/src/lib.rs
GS=crates/gateway-serve/src
SPEC=milestones/M-87-serving-circuit-breaker.md

# ─────────────── задача 1 — контракт исходов ───────────────
if [ -f "$GS/admission.rs" ] \
   && grep -qE 'pub enum ServingOutcome' "$GS/admission.rs" \
   && grep -qE 'Ready' "$GS/admission.rs" && grep -qE 'Warming' "$GS/admission.rs" \
   && grep -qE 'NotReady' "$GS/admission.rs" && grep -qE 'Unsupported' "$GS/admission.rs" \
   && grep -qE 'Overloaded' "$GS/admission.rs"; then
  pass "task1: пять исходов объявлены в admission.rs"
else
  fail "task1: нет модуля admission.rs с пятью исходами ServingOutcome"
fi

# ─────────────── задача 2 — готовность без чтения журнала (предметный набор) ───────────────
CP_OUT=$(cargo test -p gateway --test red_m87_cold_path_reads_nothing 2>&1)
CP_RC=$?
CP_LINE=$(printf '%s\n' "$CP_OUT" | grep -E '^test result' | tail -1)
if [ $CP_RC -eq 0 ]; then
  pass "task2: red_m87_cold_path_reads_nothing — ${CP_LINE:-GREEN}"
else
  fail "task2: red_m87_cold_path_reads_nothing КРАСЕН — ${CP_LINE:-компиляция}"
  printf '%s\n' "$CP_OUT" | grep -E '^(thread |assertion|---- |error)' | head -20
fi

# ─────────────── задачи 1+3+4+7 — форма допуска, политика, бюджет, секрет ───────────────
AD_OUT=$(cargo test -p gateway-serve --test red_m87_admission 2>&1)
AD_RC=$?
AD_LINE=$(printf '%s\n' "$AD_OUT" | grep -E '^test result' | tail -1)
if [ $AD_RC -eq 0 ]; then
  pass "task1+3+4+7: red_m87_admission — ${AD_LINE:-GREEN}"
else
  fail "task1+3+4+7: red_m87_admission КРАСЕН — ${AD_LINE:-компиляция}"
  printf '%s\n' "$AD_OUT" | grep -E '^(error|thread |assertion)' | head -20
fi

# ─────────────── задача 4 — счётчик ПРОЧИТАННЫХ БАЙТ существует ───────────────
if grep -qE '^\s*pub payload_bytes_read: u64,' "$GW"; then
  pass "task4: ReadStats несёт payload_bytes_read"
else
  fail "task4: в ReadStats нет payload_bytes_read — бюджет по байтам мерить нечем"
fi

# ─────────────── задача 5 — ограничитель параллелизма существует ───────────────
SEM=$(grep -rl 'Semaphore\|max_concurrent_serves' "$GS" 2>/dev/null | wc -l)
if [ "$SEM" -gt 0 ]; then
  pass "task5: ограничитель параллелизма присутствует (файлов: $SEM)"
else
  fail "task5: ограничителя параллелизма нет ни в одном файле $GS (найдено: $SEM)"
fi

# ─────────────── задача 6 — счётчики выдачи ───────────────
if [ -f "$GS/metrics.rs" ] && grep -qE 'refusals_supported' "$GS/metrics.rs" \
   && grep -qE 'refusals_unsupported' "$GS/metrics.rs"; then
  pass "task6: счётчики выдачи разделяют поддержанные и неподдержанные отказы"
else
  fail "task6: нет metrics.rs с раздельными счётчиками отказов"
fi

# ─────────────── задача 7 — единая трактовка секрета ───────────────
if grep -qE 'pub fn key_material' "$GS/lib.rs" \
   && grep -qE 'key_material' "$GS/bin/wsprobe.rs"; then
  pass "task7: секрет трактуется ОДНОЙ функцией, её зовут обе стороны"
else
  fail "task7: key_material отсутствует либо зонд её не зовёт — трактовок по-прежнему две (TD-207)"
fi

# ─────────────── задача 8 — свежесть четырьмя позициями ───────────────
FRESH=$(grep -rcE 'freshness|свежест' "$GS"/*.rs 2>/dev/null | awk -F: '{s+=$2} END{print s+0}')
if [ "$FRESH" -gt 0 ]; then
  pass "task8: свежесть представлена в коде выдачи (совпадений: $FRESH)"
else
  fail "task8: свежести нет ни в одном файле выдачи (совпадений: $FRESH)"
fi

# ─────────────── задача 9 — ресурсные лимиты ОБЪЯВЛЕНЫ ───────────────
LIM=$(grep -cE '^\s+(cpus|mem_limit|memory):' docker-compose.yml)
if [ "$LIM" -ge 4 ]; then
  pass "task9: ресурсные лимиты объявлены (строк: $LIM — выдача, recorder, прогреватель)"
else
  fail "task9: ресурсных лимитов в docker-compose.yml недостаточно (строк: $LIM, нужно ≥4)"
fi
skip "task9: ФАКТИЧЕСКИ применённые лимиты снимаются на проде (docker inspect) — шаг деплой-гейта §8, не этого скрипта"

# ─────────────── задача 10 — ревизия тест-корпуса ───────────────
if grep -q '^### 16.1. Решения по изменённым ожиданиям' "$SPEC" \
   && ! grep -q 'на момент коммита набора изменённых ожиданий нет' "$SPEC"; then
  pass "task10: решения по изменённым ожиданиям записаны"
else
  fail "task10: таблица §16.1 пуста — решение по каждому изменённому ожиданию не записано"
fi

# ─────────────── паритет с CI ───────────────
if cargo fmt --all -- --check >/dev/null 2>&1; then
  pass "CI-паритет: cargo fmt --all -- --check"
else
  fail "CI-паритет: cargo fmt --all -- --check"
fi

if cargo clippy --all-targets --all-features -- -D warnings >/dev/null 2>&1; then
  pass "CI-паритет: cargo clippy --all-targets --all-features -- -D warnings"
else
  fail "CI-паритет: cargo clippy --all-targets --all-features -- -D warnings"
fi

if cargo test --all >/dev/null 2>&1; then
  pass "CI-паритет: cargo test --all"
else
  fail "CI-паритет: cargo test --all"
fi

printf '\n'
if [ "$FAIL" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
fi
echo "VERDICT: FAIL (провалов: $FAIL)"
exit 1
