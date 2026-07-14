#!/usr/bin/env bash
# Операторский путь ретеншена (TD-020) — тело задания, которое дёргает cron.
#
# ПОЧЕМУ ОТДЕЛЬНЫЙ СКРИПТ, А НЕ КОМАНДА В CRONTAB.
# Первая редакция D5 держала всю команду прямо в `deploy/cron.d/journal-retention`, разбив её
# переносами `\`. **Cron не поддерживает продолжение строк**: каждая физическая строка — это
# отдельная запись, поэтому продолжения парсились как расписания («bad minute», файл НЕ
# устанавливается). Гейт этого не поймал, потому что грепал слова (`dry-run`, `ALERT`), а не
# проверял устанавливаемость — grep-green артефакт. Тот же класс, что весь TD-020: «текст в репо»
# ≠ «работает в проде».
# Правило: в crontab — ОДНА строка, вся логика — здесь, где её можно проверить (`bash -n`,
# прогон со стабом) и где нет ограничений cron-парсера (`%`, переносы, длина).
set -uo pipefail

HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
JOURNAL_COLD_DIR="${JOURNAL_COLD_DIR:-/mnt/journal-cold}"
RETENTION_RETAIN_DAYS="${RETENTION_RETAIN_DAYS:-14}"
RETENTION_KEEP_MIN="${RETENTION_KEEP_MIN:-4}"
RETENTION_MIN_FREE_GB="${RETENTION_MIN_FREE_GB:-10}"
# DryRun — дефолт и здесь тоже (конструктивный барьер против «случайно удалил»).
# Apply включается ОСОЗНАННО и только после сверки холодной копии — см. deploy/README.md.
RETENTION_MODE="${RETENTION_MODE:-dry-run}"
LOG="${RETENTION_LOG:-/var/log/hft/journal-retention.log}"
# Маркер для ВНЕШНЕГО монитора (zabbix/nagios пингуют файл): есть → последний прогон упал.
ALERT_FILE="${RETENTION_ALERT_FILE:-/var/lib/hft/retention.alert}"

mkdir -p "$(dirname "${LOG}")" "$(dirname "${ALERT_FILE}")" 2>/dev/null || true

alert() { # exit≠0 обязан быть ВИДЕН: молчащая уборка = TD-020 на третьем витке
  local msg="$1"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ALERT ${msg}" >> "${LOG}" 2>/dev/null || true
  command -v logger >/dev/null 2>&1 && logger -p user.err -t hft-journal-retention "ALERT: ${msg}" || true
  { date -u +%Y-%m-%dT%H:%M:%SZ; echo "${msg}"; } > "${ALERT_FILE}" 2>/dev/null || true
  echo "ALERT ${msg}" >&2
}

cd "${HFT_ROOT}" 2>/dev/null || { alert "journal-retention: нет каталога ${HFT_ROOT}"; exit 1; }

docker compose run --rm journal-retention \
  --dir=/journal \
  --cold="${JOURNAL_COLD_DIR}" \
  --retain-days="${RETENTION_RETAIN_DAYS}" \
  --keep-min="${RETENTION_KEEP_MIN}" \
  --min-free-gb="${RETENTION_MIN_FREE_GB}" \
  --mode="${RETENTION_MODE}" \
  >> "${LOG}" 2>&1
rc=$?

if [ "${rc}" -ne 0 ]; then
  alert "dry-run exit=${rc} (2=failed_cold_verify — сегмент остался ГОРЯЧИМ; 3=disk_pressure; 1=arg/io). Лог: ${LOG}"
else
  # Успешный прогон гасит маркер — следующий сбой поднимет тревогу заново.
  rm -f "${ALERT_FILE}" 2>/dev/null || true
fi
exit "${rc}"
