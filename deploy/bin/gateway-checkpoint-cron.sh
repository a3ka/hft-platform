#!/usr/bin/env bash
# Операторский cron-обёртка для gateway-checkpoint (M-48, TD-048, GW-I-12).
#
# ПОЧЕМУ ОТДЕЛЬНЫЙ СКРИПТ, А НЕ КОМАНДА В CRONTAB.
# Та же причина, что у journal-retention-cron.sh / journal-compaction-cron.sh —
# cron-парсер не поддерживает `\` (каждая физическая строка = отдельная запись,
# `\`-перенос → «bad minute», файл НЕ устанавливается). Команда — ОДНА строка,
# вся логика — здесь, где её можно проверить (`bash -n`, прогон со стабом
# docker / `HFT_CRON_PRINT_ARGV=1`) и где нет ограничений cron-парсера.
#
# КОНТРАКТ (M-48, C-032 R4): обёртка поддерживает `HFT_CRON_PRINT_ARGV=1` —
# печатает argv, который она ВЫПОЛНИЛА БЫ, и выходит 0 БЕЗ побочных эффектов
# (без docker, без записи артефактов). Это позволяет гейту (`verify_M-48.sh`)
# проверять ИСПОЛНЕНИЕ обёртки, а не только наличие файла / grep — класс
# TD-048/020 «объявлено ≠ работает». Поддерживается также устаревший
# `RETENTION_PRINT_ARGV=1` (историческое имя из journal-retention — общий гейт
# с обоими скриптами; M-48 предпочитает `HFT_CRON_PRINT_ARGV`).
#
# Шов для гейта: `CHECKPOINT_RUNNER` — `docker compose run --rm gateway-checkpoint`
# по умолчанию, но оракул может подставить прямой бинарь, чтобы проверить argv
# против НАСТОЯЩЕГО парсера (а не стаба docker, который глотает любые аргументы —
# тот же класс TD-020, что у D5 на retention).
#
# ⚠ АРГУМЕНТЫ В РАЗДЕЛЬНОЙ ФОРМЕ (`--dir X`), НЕ `--dir=X`. Compose пишет
# equals-форму; cron-обёртка — раздельную. Парсер `gateway-checkpoint` уже
# принимает ОБЕ формы (B1, M-38b rev4), но единый контракт через скрипт
# держит argv в ОДНОМ месте, чтобы прод и гейт не разъехались.
set -uo pipefail

HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
# Шов для гейта: по умолчанию — прод-путь (compose), но оракул подставляет сюда
# прямой бинарь, чтобы проверить argv ПО-НАСТОЯЩЕМУ, а не против стаба.
CHECKPOINT_RUNNER="${CHECKPOINT_RUNNER:-docker compose run --rm gateway-checkpoint}"
# Каталог журнала ВНУТРИ контейнера (compose монтирует journal-data:/journal:ro).
# На тесте — временный каталог на хосте (подменяется env).
CHECKPOINT_JOURNAL_DIR="${CHECKPOINT_JOURNAL_DIR:-/journal}"
# Каталог чекпоинтов (compose монтирует gateway-ckpt:/ckpt RW). На тесте — temp.
CHECKPOINT_CKPT_DIR="${CHECKPOINT_CKPT_DIR:-/ckpt}"
# ПУТЬ АРТЕФАКТА ПОКРЫТИЯ — КОНТРАКТ с retention-обёрткой (GW-I-12, §6 verify_M-48):
# retention ОБЯЗАН читать coverage именно по этому пути (`--checkpoint-coverage=<путь>`),
# иначе fail-closed no-op (TD-020). Прод-дефолт совпадает с
# `gateway-checkpoint --coverage-out=` в docker-compose.yml.
CHECKPOINT_COVERAGE_OUT="${CHECKPOINT_COVERAGE_OUT:-/ckpt/covered_through_seq}"
CHECKPOINT_VENUE="${CHECKPOINT_VENUE:-Binance}"
CHECKPOINT_SYMBOL="${CHECKPOINT_SYMBOL:-BTCUSDT}"
CHECKPOINT_TIMEFRAME_MS="${CHECKPOINT_TIMEFRAME_MS:-1000}"
CHECKPOINT_BANDS="${CHECKPOINT_BANDS:-0.001}"
# Bounded-window (M-37 анти-TD-020): дефолт 60_000. `0` ⇒ offline unbounded.
CHECKPOINT_WINDOW_MS="${CHECKPOINT_WINDOW_MS:-60000}"
# `--cursor LATEST` — прод-дефолт (снимаем чекпоинт ДО хвоста). Усечённый
# прогон возможен через `--cursor <i64>` (операторская диагностика; команда
# `gateway-checkpoint-cron.sh --cursor <seq>` ниже поддерживает это через env
# CHECKPOINT_CURSOR).
CHECKPOINT_CURSOR="${CHECKPOINT_CURSOR:-LATEST}"
LOG="${CHECKPOINT_LOG:-/var/log/hft/gateway-checkpoint.log}"
# Маркер для ВНЕШНЕГО монитора (zabbix/nagios пингуют файл): есть → последний
# прогон упал.
ALERT_FILE="${CHECKPOINT_ALERT_FILE:-/var/lib/hft/gateway-checkpoint.alert}"
# Позитивный heartbeat (D9, rev12): *.alert детектирует «прогон УПАЛ», но НЕ
# «cron молча не запускался» (не установлен / crond мёртв / ребут без cron).
# На УСПЕШНОМ прогоне пишем сюда UTC-таймстамп; внешний монитор алертит по
# СВЕЖЕСТИ (старше ~26 ч = cron не отработал). Имя env-var — КОНТРАКТ гейта.
LAST_SUCCESS="${CHECKPOINT_LAST_SUCCESS:-/var/lib/hft/gateway-checkpoint.last-success}"

mkdir -p "$(dirname "${LOG}")" "$(dirname "${ALERT_FILE}")" "$(dirname "${LAST_SUCCESS}")" 2>/dev/null || true

alert() {
  local msg="$1"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ALERT ${msg}" >> "${LOG}" 2>/dev/null || true
  command -v logger >/dev/null 2>&1 && logger -p user.err -t hft-gateway-checkpoint "ALERT: ${msg}" || true
  { date -u +%Y-%m-%dT%H:%M:%SZ; echo "${msg}"; } > "${ALERT_FILE}" 2>/dev/null || true
  echo "ALERT ${msg}" >&2
}

# Argv — РАЗДЕЛЬНОЙ формой для большинства флагов. ИСКЛЮЧЕНИЕ: `--coverage-out`
# пишем в EQUALS-форме (`--coverage-out=<путь>`), потому что retention-обёртка
# использует путь по этому же правилу (`--checkpoint-coverage=<путь>`), и
# verify_M-48 канарейка КОМПОЗИЦИИ сравнивает их ПОСИМВОЛЬНО через regex
# (`sed -n 's/^--coverage-out=//p'` / `s/^--checkpoint-coverage=//p`). Если бы
# оба были в раздельной форме (`--coverage-out\n<путь>`), regex не нашёл бы
# `=` и канарейка упала бы — класс «false negative в проверке композиции»,
# ровно то, что milestone запрещает (C-032 R4). Парсер `gateway-checkpoint`
# принимает обе формы (B1, M-38b rev4), так что equals-форма для одного флага
# не ломает контракт.
ARGV=(
  --dir "${CHECKPOINT_JOURNAL_DIR}"
  --ckpt-dir "${CHECKPOINT_CKPT_DIR}"
  --coverage-out="${CHECKPOINT_COVERAGE_OUT}"
  --venue "${CHECKPOINT_VENUE}"
  --symbol "${CHECKPOINT_SYMBOL}"
  --timeframe-ms "${CHECKPOINT_TIMEFRAME_MS}"
  --bands "${CHECKPOINT_BANDS}"
  --window-ms "${CHECKPOINT_WINDOW_MS}"
  --cursor "${CHECKPOINT_CURSOR}"
)

# Печать argv — ДО любых side-эффектов (cd/mkdir): контракт argv не зависит от
# того, где мы и существует ли прод-каталог. Гейт проверяет argv по выводу
# (`grep --coverage-out`, `--dir|--ckpt-dir`, совпадение с retention). Без
# этой ветки скрипт мог бы падать на `cd` раньше печати (тот же дефект D5,
# который первая редакция retention-обёртки имела).
if [ "${HFT_CRON_PRINT_ARGV:-0}" = "1" ] || [ "${RETENTION_PRINT_ARGV:-0}" = "1" ]; then
  printf '%s\n' "${ARGV[@]}"
  exit 0
fi

cd "${HFT_ROOT}" 2>/dev/null || { alert "gateway-checkpoint: нет каталога ${HFT_ROOT}"; exit 1; }

# shellcheck disable=SC2086 — CHECKPOINT_RUNNER намеренно расщепляется на слова (это команда).
${CHECKPOINT_RUNNER} "${ARGV[@]}" >> "${LOG}" 2>&1
rc=$?

if [ "${rc}" -ne 0 ]; then
  alert "exit=${rc} (1=argv/IO, 2=validate_selector fail-closed GW-I-10, 1=advance_to fail-loud GW-I-12 — разрыв «чекпоинт↔журнал»). Лог: ${LOG}"
else
  # Успешный прогон гасит маркер — следующий сбой поднимет тревогу заново.
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  # Позитивный heartbeat (D9).
  date -u +%Y-%m-%dT%H:%M:%SZ > "${LAST_SUCCESS}" 2>/dev/null || true
fi
exit "${rc}"