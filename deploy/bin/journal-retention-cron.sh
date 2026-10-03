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

# ⚠ ФОРМА АРГУМЕНТОВ — РАЗДЕЛЬНАЯ (`--dir X`), НЕ `--dir=X`.
# На проде задание упало именно на этом: парсер бинаря (`journal-retention`) сравнивает аргумент
# ЦЕЛИКОМ (`match arg { "--dir" => ... }`) и берёт значение СЛЕДУЮЩИМ элементом argv, т.е.
# `--dir=/journal` для него — неизвестный флаг. Сбивает с толку то, что `--help` самого бинаря
# печатает `=`-форму (это его дефект, заведён отдельной задачей) — но контракт argv определяет
# ПАРСЕР, а не текст справки. Оракул D5 этого не поймал, потому что подставлял стаб `docker`,
# который глотал любые аргументы: **застабил ровно тот контракт, который и ломался**.
# Теперь D5 гоняет НАСТОЯЩИЙ бинарь с ЭТИМ argv (см. RETENTION_RUNNER ниже) — дрейф между
# скриптом и парсером больше не может пройти незамеченным.
HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
# Шов для гейта: по умолчанию — прод-путь (compose), но оракул подставляет сюда прямой бинарь,
# чтобы проверить argv ПО-НАСТОЯЩЕМУ, а не против стаба.
RETENTION_RUNNER="${RETENTION_RUNNER:-docker compose run --rm journal-retention}"
# Каталог журнала ВНУТРИ контейнера (в тесте — временный каталог на хосте).
RETENTION_JOURNAL_DIR="${RETENTION_JOURNAL_DIR:-/journal}"
JOURNAL_COLD_DIR="${JOURNAL_COLD_DIR:-/mnt/journal-cold}"
RETENTION_RETAIN_DAYS="${RETENTION_RETAIN_DAYS:-14}"
RETENTION_KEEP_MIN="${RETENTION_KEEP_MIN:-4}"
RETENTION_MIN_FREE_GB="${RETENTION_MIN_FREE_GB:-10}"
# M-48 (TD-048, GW-I-12): путь к артефакту `covered_through_seq`, КУДА пишет
# `gateway-checkpoint` (`--coverage-out`). Передаём в retention через
# `--checkpoint-coverage=<путь>` — иначе override выключен ⇒ retention молча
# уходит в `offload_only` ⇒ prune не происходит НИКОГДА (fail-closed no-op,
# TD-020 на третьем витке). ДОЛЖЕН СОВПАДАТЬ с путём, который пишет
# `gateway-checkpoint-cron.sh` (--coverage-out). КОМПОЗИЦИЯ этих двух строк —
# настоящий инвариант цепочки (testing.md п.6); проверяется канарейкой
# `verify_M-48.sh` «КОМПОЗИЦИЯ — обёртки согласованы по пути артефакта».
RETENTION_CHECKPOINT_COVERAGE="${RETENTION_CHECKPOINT_COVERAGE:-/ckpt/covered_through_seq}"
# DryRun — дефолт и здесь тоже (конструктивный барьер против «случайно удалил»).
# Apply включается ОСОЗНАННО и только после сверки холодной копии — см. deploy/README.md.
RETENTION_MODE="${RETENTION_MODE:-dry-run}"
LOG="${RETENTION_LOG:-/var/log/hft/journal-retention.log}"
# Маркер для ВНЕШНЕГО монитора (zabbix/nagios пингуют файл): есть → последний прогон упал.
ALERT_FILE="${RETENTION_ALERT_FILE:-/var/lib/hft/retention.alert}"
# Позитивный heartbeat (D9, rev12): *.alert детектирует «прогон УПАЛ», но НЕ «cron молча
# не запускался» (не установлен / crond мёртв / ребут без cron). На УСПЕШНОМ прогоне
# пишем сюда UTC-таймстамп; внешний монитор алертит по СВЕЖЕСТИ (старше ~26 ч = cron не
# отработал). Имя env-var — КОНТРАКТ гейта D9, не менять без обновления verify_delivery_M-08.sh.
LAST_SUCCESS="${RETENTION_LAST_SUCCESS:-/var/lib/hft/retention.last-success}"

mkdir -p "$(dirname "${LOG}")" "$(dirname "${ALERT_FILE}")" "$(dirname "${LAST_SUCCESS}")" 2>/dev/null || true

alert() { # exit≠0 обязан быть ВИДЕН: молчащая уборка = TD-020 на третьем витке
  local msg="$1"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ALERT ${msg}" >> "${LOG}" 2>/dev/null || true
  command -v logger >/dev/null 2>&1 && logger -p user.err -t hft-journal-retention "ALERT: ${msg}" || true
  { date -u +%Y-%m-%dT%H:%M:%SZ; echo "${msg}"; } > "${ALERT_FILE}" 2>/dev/null || true
  echo "ALERT ${msg}" >&2
}

# Argv — РАЗДЕЛЬНОЙ формой (см. шапку). ИСКЛЮЧЕНИЕ: `--checkpoint-coverage` — в
# EQUALS-форме (`--checkpoint-coverage=<путь>`), чтобы verify_M-48 канарейка
# КОМПОЗИЦИИ могла его распарсить regex'ом (`sed -n 's/^--checkpoint-coverage=//p'`)
# и сравнить с `--coverage-out` чекпоинтера. Если бы был в раздельной форме,
# regex не нашёл бы `=` — ровно тот класс «false negative в проверке
# композиции», который milestone запрещает (C-032 R4). Парсер
# `journal-retention` принимает обе формы.
ARGV=(
  --dir "${RETENTION_JOURNAL_DIR}"
  --cold "${JOURNAL_COLD_DIR}"
  --retain-days "${RETENTION_RETAIN_DAYS}"
  --keep-min "${RETENTION_KEEP_MIN}"
  --min-free-gb "${RETENTION_MIN_FREE_GB}"
  --mode "${RETENTION_MODE}"
  # M-48 (GW-I-12): обязательно передаём путь к артефакту покрытия. Без этого
  # retention не знает, до какого seq безопасно прунить — fail-closed no-op.
  --checkpoint-coverage="${RETENTION_CHECKPOINT_COVERAGE}"
)

# Печать argv — ДО любых side-эффектов (cd/mkdir): контракт argv не зависит от того, где мы
# и существует ли прод-каталог. Иначе гейт не смог бы его прочитать (и не прочитал — первая
# попытка вернула пустой argv, потому что скрипт падал на `cd` раньше печати).
# Поддерживаем обе формы env-var: историческую `RETENTION_PRINT_ARGV` (M-08) и
# новую `HFT_CRON_PRINT_ARGV` (M-48, C-032 R4 — единый контракт для всех
# cron-обёрток проекта). Гейт M-48 проверяет новую; старые verify-скрипты M-08
# продолжают работать по старой.
if [ "${RETENTION_PRINT_ARGV:-0}" = "1" ] || [ "${HFT_CRON_PRINT_ARGV:-0}" = "1" ]; then
  printf '%s\n' "${ARGV[@]}"
  exit 0
fi

cd "${HFT_ROOT}" 2>/dev/null || { alert "journal-retention: нет каталога ${HFT_ROOT}"; exit 1; }

# shellcheck disable=SC2086 — RETENTION_RUNNER намеренно расщепляется на слова (это команда).
${RETENTION_RUNNER} "${ARGV[@]}" >> "${LOG}" 2>&1
rc=$?

if [ "${rc}" -ne 0 ]; then
  alert "dry-run exit=${rc} (2=failed_cold_verify — сегмент остался ГОРЯЧИМ; 3=disk_pressure; 1=arg/io). Лог: ${LOG}"
else
  # Успешный прогон гасит маркер — следующий сбой поднимет тревогу заново.
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  # Позитивный heartbeat (D9): «cron жил» отличается от «cron не установлен» только свежестью
  # этого файла. Пишется ПОСЛЕ гашения alert'а — даже если `date` упадёт (нет прав / read-only FS),
  # сбой не поднимет ложный алерт, просто heartbeat замолчит (мониторинг ловит старьё).
  date -u +%Y-%m-%dT%H:%M:%SZ > "${LAST_SUCCESS}" 2>/dev/null || true
fi
exit "${rc}"
