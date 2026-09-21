#!/usr/bin/env bash
# Операторская cron-обёртка для СТОРОЖА BuildKit-кэша (29.08.2026: диск уехал 90 % из-за
# 43.85 ГБ накопленного за 7 недель кэша сборок; очищено вручную → 62 %). Без сторожа
# кэш вернётся за те же 7 недель, потому что `deploy.yml` пересобирает образ на каждом
# push'е в main, и НИКТО его сейчас не подрезает.
#
# ПОЧЕМУ ОТДЕЛЬНЫЙ СКРИПТ, А НЕ КОМАНДА В CRONTAB.
# Та же причина, что у journal-retention-cron.sh / journal-compaction-cron.sh /
# journal-offsite-cron.sh:
#  - cron-парсер не понимает `\`-переносов (каждая физическая строка = запись);
#  - текст скрипта верифицируется `bash -n` и прогоном со стабом;
#  - argv в одном месте — прод и гейт берут его ОТСЮДА (HFT_CRON_PRINT_ARGV=1).
#
# ВЫБОР `until=336h`. Задача требует «кэш не должен переживать более двух недель».
# 14 дней × 24 = 336 часов — буквальное выражение. При ежесуточном cron'е потолок
# реального возраста ≈ 336 + 24 = 360 часов (≈ 15 дней), что вписывается в «не
# более двух недель» с запасом на суточное окно расписания.
#
# Замер 29.08.2026 показал 43.85 ГБ за 49 дней ⇒ ~0.9 ГБ/день прироста. При
# N=336h потолок ≈ 14 × 0.9 = 12.6 ГБ (в 3.5× меньше исходных 43.85). С запасом.
#
# ЧАСТОТА: раз в сутки. Меньше — не нужно (рост кэша суточный, не часовой);
# чаще — лишний I/O без эффекта (между прогонами кэш на 0.9 ГБ не набегает).
#
# ЗАПРЕТЫ (явные, чтобы случайно не нарисовать лишнее):
#  - НЕ `docker image prune -a` (удаляет ВСЕ неиспользуемые образы; rollback в deploy.yml
#    зависит от них — `image: hft-platform-recorder:previous-tag` и т.п., см. §6 runbook'а).
#  - НЕ любой prune, ТРОГАЮЩИЙ ТОМА (`docker volume prune`): в томах живёт журнал
#    (`hft-platform_journal-data`), и его потеря — необратимый P0 (TD-020 наоборот).
#  - НЕ `docker system prune -a` — комбинирует оба запрета.
# Только `docker builder prune` — это кэш BuildKit'а, а не образы и не тома.
set -uo pipefail

# Конфигурация через env — паттерн из соседних cron-обёрток, не изобретение.
HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
# N — буквальные 14 дней (336 часов); см. шапку «ВЫБОР until=336h».
UNTIL_HOURS="${BUILDER_PRUNE_UNTIL_HOURS:-336}"
# Подтверждение `-f`: без него prune ИНТЕРАКТИВНО спросит подтверждение, cron не
# ответит — задание «зависнет» (на самом деле упадёт через stdin-EOF, но не
# обязательно чисто). Cron в принципе не отвечает на tty-prompt, так что `-f`
# здесь обязателен по дизайну, а не для удобства.
FORCE="${BUILDER_PRUNE_FORCE:--f}"
LOG="${BUILDER_PRUNE_LOG:-/var/log/hft/builder-prune.log}"
# Маркер для ВНЕШНЕГО монитора (zabbix/nagios пингуют файл): есть → последний прогон упал.
ALERT_FILE="${BUILDER_PRUNE_ALERT_FILE:-/var/lib/hft/builder-prune.alert}"
# Позитивный heartbeat (deploy/README.md §5): *.alert детектирует «прогон УПАЛ»,
# *.last-success детектирует «cron молча не запускался» — два разных дефекта, оба
# обязаны быть видимы. Без *.last-success узнали бы о молчании из диска, а не из мониторинга.
LAST_SUCCESS="${BUILDER_PRUNE_LAST_SUCCESS:-/var/lib/hft/builder-prune.last-success}"

mkdir -p "$(dirname "${LOG}")" "$(dirname "${ALERT_FILE}")" "$(dirname "${LAST_SUCCESS}")" 2>/dev/null || true

alert() { # exit≠0 ОБЯЗАН быть виден — молчащий сторож = 7-недельный кэш возвращается
  local msg="$1"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ALERT ${msg}" >> "${LOG}" 2>/dev/null || true
  command -v logger >/dev/null 2>&1 && logger -p user.err -t hft-builder-prune "ALERT: ${msg}" || true
  { date -u +%Y-%m-%dT%H:%M:%SZ; echo "${msg}"; } > "${ALERT_FILE}" 2>/dev/null || true
  echo "ALERT ${msg}" >&2
}

# Argv — ЕДИНСТВЕННОЕ место, где он определён (прод и гейт берут отсюда).
print_argv() {
  echo "VARS:"
  echo "UNTIL_HOURS=${UNTIL_HOURS}"
  echo "FORCE=${FORCE}"
  echo "DOCKER:"
  echo "docker builder prune --filter until=\${UNTIL_HOURS}h \${FORCE}"
}

# Поддерживаем обе формы env-var (как у соседей): историческую
# `RETENTION_PRINT_ARGV=1` (от journal-retention-cron.sh) и новую
# `HFT_CRON_PRINT_ARGV=1` (M-48, единый контракт cron-обёрток).
if [ "${HFT_CRON_PRINT_ARGV:-0}" = "1" ] || [ "${RETENTION_PRINT_ARGV:-0}" = "1" ]; then
  print_argv
  exit 0
fi

# ── Setup-валидация — сбой ДО docker лучше, чем сбой ПОСЛЕ удаления кэша ────────────
# Если docker недоступен (демон упал / прав нет) — fail-loud с alert'ом, а не тишина.
if ! command -v docker >/dev/null 2>&1; then
  alert "docker не найден в PATH — daemon умер или PATH сломан"
  exit 1
fi
if ! docker info >/dev/null 2>&1; then
  alert "docker info отказал — daemon недоступен, prune не выполнен"
  exit 1
fi

# ── Основная работа: docker builder prune ──────────────────────────────────────────
# `--filter until=336h` — удалить кэш СТАРШЕ 14 дней. Метка берётся из BuildKit'а
# (createdAt), и НЕ зависит от того, был ли кэш фактически использован. Это
# соответствует намерению «кэш не должен переживать более двух недель»: даже
# «нужный для rollback» слой BuildKit пересоберётся за минуты — хуже, чем
# 12 ГБ кэша впустую. Сами ОБРАЗЫ (hft-platform-recorder:*) `builder prune` не
# трогает — это разные сущности (см. запреты в шапке).
#
# Логи `docker builder prune` (включая сводку «Total reclaimed space: X GB»)
# идут в stdout — пишем в LOG целиком, чтобы глазами видеть объём освобождённого.
# exit≠0 docker'а ВСЕГДА означает сбой (а не «нечего удалять»: пустой prune = exit 0).
if docker builder prune --filter "until=${UNTIL_HOURS}h" ${FORCE} >> "${LOG}" 2>&1; then
  rc=0
else
  rc=$?
fi

if [ "${rc}" -ne 0 ]; then
  alert "docker builder prune exit=${rc} (1=daemon/arg; 125=daemon error). Проверь ${LOG}."
  exit "${rc}"
fi

# Успешный прогон гасит alert и пишет heartbeat.
rm -f "${ALERT_FILE}" 2>/dev/null || true
date -u +%Y-%m-%dT%H:%M:%SZ > "${LAST_SUCCESS}" 2>/dev/null || true

echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) OK filter=until=${UNTIL_HOURS}h force=${FORCE}" >> "${LOG}" 2>/dev/null || true
exit 0
