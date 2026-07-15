#!/usr/bin/env bash
# Операторский cron-файл для КОМПАКЦИИ закрытых сегментов (D-COMP-3, M-08 task 16, TD-022).
#
# ЗАЧЕМ ОТДЕЛЬНЫЙ СКРИПТ, А НЕ КОМАНДА В CRONTAB.
# Та же причина, что у journal-retention-cron.sh (см. шапку того скрипта):
#  - cron не поддерживает `\` для продолжения строки (каждая физическая строка = запись,
#    `\`-перенос → «bad minute», файл НЕ устанавливается). Контракт через cron-парсер
#    проверяется гейтом D7 (`crontab -n` exit=0 + grep конца строки).
#  - текст скрипта верифицируется `bash -n` и прогоном со стабом — D5-урок.
#  - argv в РАЗДЕЛЬНОЙ форме (`--mode compact`, а не `--mode=compact`) — ровно так
#    задание упало на проде, и теперь гейт D5a гоняет НАСТОЯЩИЙ бинарь с ЭТИМ argv;
#    дрейф между cron-скриптом и парсером больше не проходит.
#
# ШОВ С ГЕЙТОМ ТОЧНО ТАКОЙ ЖЕ, КАК У РЕТЕНШЕНА:
#  - RETENTION_PRINT_ARGV=1 → печатает argv ДО side-эффектов (cd/mkdir), exit 0;
#  - RETENTION_RUNNER (или COMPACTION_RUNNER здесь) — шов для подмены команды;
#  - RETENTION_JOURNAL_DIR — каталог журнала ВНУТРИ контейнера;
#  - exit≠0 → маркер-файл + syslog + алерт; успешный прогон маркер гасит.
#
# ⚠ КОМПАКЦИЯ БЕЗОПАСНА ПО ДИЗАЙНУ (D-COMP-2: оригинал удаляется ТОЛЬКО после
# доказанной sha256-сверки сжатого .zst). Поэтому по умолчанию — НЕ dry-run:
# у compact-режима есть dry-run-семантика ВНУТРИ библиотеки (`compact_closed_segments`
# не удаляет оригинал при сбое), а снаружи — безусловное выполнение. Если оператор
# захочет «без действия» — он может вызвать `journal-retention --help` или просто
# не ставить cron.
#
# Cron-расписание ДОЛЖНО идти в отдельной строке crontab-файла (см. cron.d/journal-retention).
set -uo pipefail

HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
# Шов для гейта: по умолчанию — прод-путь (compose), но оракул подставляет сюда прямой
# бинарь, чтобы проверить argv ПО-НАСТОЯЩЕМУ, а не против стаба.
RETENTION_RUNNER="${RETENTION_RUNNER:-docker compose run --rm journal-compaction}"
# Имя env-переменных с префиксом RETENTION_ — намеренно, чтобы один и тот же гейт
# (RETENTION_PRINT_ARGV) мог проверять И ретеншен, И компакцию. RETENTION — это
# историческое имя (retention бинаря у journal-retention — исторический первый режим),
# не путать с функциональной обязанностью.
RETENTION_JOURNAL_DIR="${RETENTION_JOURNAL_DIR:-/journal}"
# Сколько последних закрытых сегментов остаются СЫРЫМИ (D-COMP-3): свежее читают чаще.
COMPACTION_KEEP_RAW="${COMPACTION_KEEP_RAW:-2}"
# Уровень zstd: дефолт 3 = 9.1× на боевых (фиксирован через DEFAULT_COMPACT_LEVEL в lib).
# Если у задания/оператора есть ОСОБОЕ желание поднять (до -9 = 12.6×) — env ниже.
COMPACTION_COMPACT_LEVEL="${COMPACTION_COMPACT_LEVEL:-3}"
LOG="${COMPACTION_LOG:-/var/log/hft/journal-compaction.log}"
# Маркер для ВНЕШНЕГО монитора: есть → последний прогон упал.
ALERT_FILE="${COMPACTION_ALERT_FILE:-/var/lib/hft/compaction.alert}"
# Позитивный heartbeat (D9, rev12): см. комментарий в journal-retention-cron.sh — та же
# семантика. *.alert ловит сбой, *.last-success ловит «cron молча не запустился» (не
# установлен / crond мёртв / ребут) — это РАЗНЫЕ классы дефектов, и нужен ОБА маркера.
# Имя env-var — КОНТРАКТ гейта D9, не менять без обновления verify_delivery_M-08.sh.
LAST_SUCCESS="${COMPACTION_LAST_SUCCESS:-/var/lib/hft/compaction.last-success}"

mkdir -p "$(dirname "${LOG}")" "$(dirname "${ALERT_FILE}")" "$(dirname "${LAST_SUCCESS}")" 2>/dev/null || true

alert() { # exit≠0 обязан быть ВИДЕН — иначе cron будет молча «успевать»
  local msg="$1"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ALERT ${msg}" >> "${LOG}" 2>/dev/null || true
  command -v logger >/dev/null 2>&1 && logger -p user.err -t hft-journal-compaction "ALERT: ${msg}" || true
  { date -u +%Y-%m-%dT%H:%M:%SZ; echo "${msg}"; } > "${ALERT_FILE}" 2>/dev/null || true
  echo "ALERT ${msg}" >&2
}

# Argv — РАЗДЕЛЬНОЙ формой (см. шапку). Единственное место, где он определён: и прод,
# и гейт берут его отсюда, поэтому разъехаться они не могут.
ARGV=(
  --dir "${RETENTION_JOURNAL_DIR}"
  --keep-raw "${COMPACTION_KEEP_RAW}"
  --mode compact
)

# Печать argv — ДО любых side-эффектов (cd/mkdir): контракт argv не зависит от того,
# где мы и существует ли прод-каталог. Иначе гейт не смог бы его прочитать.
if [ "${RETENTION_PRINT_ARGV:-0}" = "1" ]; then
  printf '%s\n' "${ARGV[@]}"
  exit 0
fi

cd "${HFT_ROOT}" 2>/dev/null || { alert "journal-compaction: нет каталога ${HFT_ROOT}"; exit 1; }

# shellcheck disable=SC2086 — RETENTION_RUNNER намеренно расщепляется на слова (это команда).
${RETENTION_RUNNER} "${ARGV[@]}" >> "${LOG}" 2>&1
rc=$?

if [ "${rc}" -ne 0 ]; then
  alert "compact exit=${rc} (2=sha256 .zst mismatch — оригинал оставлен ГОРЯЧИМ, данные НЕ потеряны, \
но требует внимания; 1=arg/io). Лог: ${LOG}"
else
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  # Позитивный heartbeat (D9): см. journal-retention-cron.sh — та же семантика, имя файла —
  # КОНТРАКТ гейта (COMPACTION_LAST_SUCCESS, дефолт /var/lib/hft/compaction.last-success).
  date -u +%Y-%m-%dT%H:%M:%SZ > "${LAST_SUCCESS}" 2>/dev/null || true
fi
exit "${rc}"
