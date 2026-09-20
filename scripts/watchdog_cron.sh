#!/usr/bin/env bash
# Cron-обёртка вокруг `ops-watchdog` (crates/ops/src/bin/ops-watchdog.rs). Задача: подмять
# самого watchdog'а под ту же дисциплину, что у `deploy/bin/journal-retention-cron.sh` —
# позитивный heartbeat на успех (внешний монитор может отличить "cron не установлен/crond
# мёртв" от "watchdog запускается, но падает"), ALERT-маркер на явный сбой.
#
# ВАЖНО (см. deploy/bin/journal-retention-cron.sh шапку): вся ЛОГИКА — здесь, В КРОНТАБЕ —
# одна строка. Cron не поддерживает продолжение строк; проверяемость (`bash -n`, прогон со
# стабом) достижима только вне cron-парсера.
#
# Установка (задача reviewer/founder после ревью — не выполняется мной):
#   */5 * * * * root /root/hft-platform/scripts/watchdog_cron.sh >> /var/log/hft/watchdog.log 2>&1
set -uo pipefail

HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
# Путь к собранному бинарю (`cargo build --release -p ops --bin ops-watchdog`). Шов для
# гейта/теста: оракул подставляет сюда путь к тестовому бинарю/стабу.
WATCHDOG_BIN="${WATCHDOG_BIN:-${HFT_ROOT}/target/release/ops-watchdog}"
LOG="${WATCHDOG_LOG:-/var/log/hft/watchdog.log}"
ALERT_FILE="${WATCHDOG_ALERT_FILE:-/var/lib/hft/watchdog.alert}"
LAST_SUCCESS="${WATCHDOG_LAST_SUCCESS:-/var/lib/hft/watchdog.last-success}"

mkdir -p "$(dirname "${LOG}")" "$(dirname "${ALERT_FILE}")" "$(dirname "${LAST_SUCCESS}")" 2>/dev/null || true

alert() { # exit≠0 обязан быть ВИДЕН — та же дисциплина, что у ретеншена (TD-020 урок).
  local msg="$1"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ALERT ${msg}" >>"${LOG}" 2>/dev/null || true
  command -v logger >/dev/null 2>&1 && logger -p user.err -t hft-ops-watchdog "ALERT: ${msg}" || true
  { date -u +%Y-%m-%dT%H:%M:%SZ; echo "${msg}"; } >"${ALERT_FILE}" 2>/dev/null || true
  echo "ALERT ${msg}" >&2
}

if [ "${WATCHDOG_PRINT_BIN:-0}" = "1" ]; then
  printf '%s\n' "${WATCHDOG_BIN}"
  exit 0
fi

if [ ! -x "${WATCHDOG_BIN}" ]; then
  alert "ops-watchdog: бинарь не найден/не исполняем (${WATCHDOG_BIN}) — соберите \`cargo build --release -p ops --bin ops-watchdog\`"
  exit 1
fi

"${WATCHDOG_BIN}" >>"${LOG}" 2>&1
rc=$?

if [ "${rc}" -ne 0 ]; then
  alert "ops-watchdog exit=${rc} — сам процесс мониторинга упал (не путать с найденными им CRITICAL-алертами, это НЕ ошибка). Лог: ${LOG}"
else
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  date -u +%Y-%m-%dT%H:%M:%SZ >"${LAST_SUCCESS}" 2>/dev/null || true
fi
exit "${rc}"
