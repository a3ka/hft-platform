#!/usr/bin/env bash
# Acceptance-гейт переделки ветки `feat/alerting` по вердикту PR-гейта R-005
# (`research/reviews/R-005-alerting.md`, ВЕРДИКТ: REJECTED — 7 находок).
#
# Правила гейта — `.claude/rules/gates.md` §3: явный агрегатор с FAIL-счётчиком, exit≠0 при
# FAIL>0, финальная строка VERDICT, минимум одна проверка на находку. Никаких
# `cmd && echo PASS || echo FAIL` (маскирует провал) — каждая проверка идёт через `check`,
# который берёт РЕАЛЬНЫЙ exit-код.
#
# Прогон групп оракулов идёт через `run_oracles <префикс> <сколько_ожидаем>`: фильтр
# `cargo test -- <substring>` сам по себе даёт exit 0, если не нашлось НИ ОДНОГО теста, —
# без сверки количества гейт можно было бы «пройти», удалив оракулы.
#
# ВАЖНО: до реализации engine-dev'ом этот скрипт обязан быть КРАСНЫМ — оракулы написаны
# architect'ом ДО кода (RED-first; F-4 из R-005 — прошлый круг делал наоборот).

set -uo pipefail

cd "$(dirname "$0")/.." || exit 1

FAILED=0
PASSED=0

check() { # check "<описание>" <команда...>
  local desc="$1"
  shift
  local out rc
  out="$("$@" 2>&1)"
  rc=$?
  if [ "$rc" -eq 0 ]; then
    PASSED=$((PASSED + 1))
    echo "PASS  ${desc}"
  else
    FAILED=$((FAILED + 1))
    echo "FAIL  ${desc}  (exit=${rc})"
    echo "${out}" | tail -25 | sed 's/^/      | /'
  fi
}

run_oracles() { # <фильтр-подстрока> <ожидаемое число тестов>
  local filter="$1" expected="$2"
  local out rc
  out="$(cargo test -p ops --test red_ops_watchdog_cycle -- "${filter}" 2>&1)"
  rc=$?
  echo "${out}"
  [ "${rc}" -eq 0 ] || return 1
  echo "${out}" | grep -Eq "test result: ok\. ${expected} passed" || {
    echo ">>> ожидалось ${expected} зелёных оракулов по фильтру '${filter}' — оракулы удалены/переименованы?"
    return 2
  }
}

grep_present() { grep -Eq -- "$2" "$1"; }
grep_absent() { ! grep -Eq -- "$2" "$1"; }

BIN=crates/ops/src/bin/ops-watchdog.rs
TRANSPORT=crates/ops/src/transport.rs
CYCLE_TEST=crates/ops/tests/red_ops_watchdog_cycle.rs
REDACT_TEST=crates/ops/tests/red_ops_transport_redaction.rs

echo "=== 0. Гигиена сборки ==========================================================="
check "cargo fmt --check (весь workspace)" cargo fmt --all -- --check
check "cargo clippy -p ops --all-targets -D warnings" \
  cargo clippy -p ops --all-targets -- -D warnings
check "бинарь ops-watchdog собирается" cargo build -p ops --bin ops-watchdog

echo "=== 1. Оракулы на месте (architect-only, dev их не удаляет и не правит) ========="
check "существует ${CYCLE_TEST}" test -f "${CYCLE_TEST}"
check "существует ${REDACT_TEST}" test -f "${REDACT_TEST}"

echo "=== 2. F-1 — детектор застоя не выключается интервалом cron'а ==================="
check "оракул интервал-независимости присутствует" \
  grep_present "${CYCLE_TEST}" 'fn f1_seq_stall_is_detected_at_every_realistic_cron_interval'
check "оракулы F-1 зелёные (6)" run_oracles f1_ 6
check "склейка переехала в библиотеку: бинарь зовёт run_cycle" grep_present "${BIN}" 'run_cycle'
check "старой склейки в бинаре нет (run_heartbeat_checks)" \
  grep_absent "${BIN}" 'fn run_heartbeat_checks'
check "старой склейки в бинаре нет (push_or_clear)" grep_absent "${BIN}" 'fn push_or_clear'

echo "=== 3. F-2 — секрет не попадает в лог cron'а ===================================="
check "оракулы редакции секрета зелёные (включая сквозной прогон бинаря)" \
  cargo test -p ops --test red_ops_transport_redaction
check "сырой reqwest-error больше не кладётся в TransportError" \
  grep_absent "${TRANSPORT}" 'TransportError::Http\(e\.to_string\(\)\)'
check "хардкоженых токенов в crates/ops/src, scripts, deploy нет" \
  bash -c '! grep -rEn "bot[0-9]{6,}:[A-Za-z0-9_-]{20,}" crates/ops/src scripts deploy'

echo "=== 4. F-3 — прогноз диска переживает ночное окно обслуживания =================="
check "оракулы F-3 зелёные (5)" run_oracles f3_ 5

echo "=== 5. F-5 — «не смог оценить» не стирает дедуп-память =========================="
check "оракулы F-5 зелёные (3)" run_oracles f5_ 3

echo "=== 6. F-6 — маркер <job>.alert («прогон УПАЛ») читается ========================"
check "оракулы F-6 зелёные (4)" run_oracles f6_ 4
check "код инцидента WD-CRON-FAILED объявлен в крейте" \
  grep_present crates/ops/src/watchdog.rs 'WD-CRON-FAILED'

echo "=== 7. F-7 — рестарт-петля продолжает о себе сообщать ==========================="
check "оракулы F-7 зелёные (3)" run_oracles f7_ 3

echo "=== 8. Сквозные vantage + весь крейт + workspace ================================"
check "здоровый прод не шумит; состояние ограничено за неделю (2)" run_oracles _ 23
check "весь крейт ops зелёный" cargo test -p ops
check "workspace зелёный" cargo test --workspace

echo
echo "checks passed=${PASSED} failed=${FAILED}"
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL"
  exit 1
fi
echo "VERDICT: PASS"
exit 0
