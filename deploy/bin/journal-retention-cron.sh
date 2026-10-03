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

# ⚠ HFT_CRON_PRINT_ARGV ОБРАБАТЫВАЕМ ДО любых default-ов с `${VAR:?}`.
# M-48 (C-032 R4): канарейка verify_M-48 запускает обёртку в режиме печати argv
# (`HFT_CRON_PRINT_ARGV=1`) и проверяет exit=0 + наличие `--checkpoint-coverage`.
# Если default `RETENTION_REMOTE_SHA_CMD` раскрывает `${JOURNAL_OFFSITE_SSH_KEY:?...}`
# раньше этой проверки, `set -u` роняет скрипт на unset-переменной ДО argv —
# ровно тот класс «объявлено ≠ работает», который канарейка ловит.
if [ "${HFT_CRON_PRINT_ARGV:-0}" = "1" ] || [ "${RETENTION_PRINT_ARGV:-0}" = "1" ]; then
  HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
  RETENTION_JOURNAL_DIR="${RETENTION_JOURNAL_DIR:-/journal}"
  RETENTION_RETAIN_DAYS="${RETENTION_RETAIN_DAYS:-14}"
  RETENTION_KEEP_MIN="${RETENTION_KEEP_MIN:-4}"
  RETENTION_MIN_FREE_GB="${RETENTION_MIN_FREE_GB:-10}"
  RETENTION_CHECKPOINT_COVERAGE="${RETENTION_CHECKPOINT_COVERAGE:-/ckpt/covered_through_seq}"
  RETENTION_WORK_DIR="${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}"
  printf '%s\n' \
    --dir "${RETENTION_JOURNAL_DIR}" \
    --retain-days "${RETENTION_RETAIN_DAYS}" \
    --keep-min "${RETENTION_KEEP_MIN}" \
    --min-free-gb "${RETENTION_MIN_FREE_GB}" \
    --mode dry-run \
    --checkpoint-coverage="${RETENTION_CHECKPOINT_COVERAGE}" \
    --plan-out "${RETENTION_WORK_DIR}/plan.txt"
  exit 0
fi

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
# M-92 (TD-202, A-044 §5.1, П-031): режим — ТОЛЬКО из файла-переключателя. Переменная
# `RETENTION_MODE` окружения режим НЕ задаёт (файл расписания перезаписывается каждым кодовым
# деплоем; 2026-09-23 так была стёрта строка `CHECKPOINT_BANDS` — тот же класс «управление
# через env, которое теряется при деплое»). Содержимое файла — `apply` ⇒ удаление после сверки
# с офсайт-копией; пустой/отсутствующий/любой-иной ⇒ `dry-run` (fail-closed дефолт).
RETENTION_MODE_FILE="${RETENTION_MODE_FILE:-/var/lib/hft/retention.mode}"
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
# M-92 (A-044 §5.1, §4 п.1–2): рабочий каталог плана и манифеста; монтируется в
# `journal-retention`-контейнер как `/work`. Сюда же скрипт пишет `plan.txt` (через
# `--plan-out` бинаря) и `manifest.txt` (через `RETENTION_REMOTE_SHA_CMD`).
RETENTION_WORK_DIR="${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}"
# M-92 (A-044 §5.1, §4 п.4): аудит-след — три записи на прогон (план, манифест, отчёт)
# с уникальными БАЗОВЫМИ именами, плоско в этом каталоге. Подкаталог на прогон
# НЕДОСТАТОЧЕН (A-045 §2.1: `c4` сравнивает базовые имена). Записи прошлых прогонов
# не перезаписываются; недоступность носителя ⇒ apply НЕ зовётся.
RETENTION_AUDIT_DIR="${RETENTION_AUDIT_DIR:-/var/lib/hft/prune-audit}"
# M-92 D5c (задача 3, узкая доработка): дефолт `RETENTION_REMOTE_SHA_CMD` раскрывал
# `${JOURNAL_OFFSITE_SSH_KEY:?…}` и `${JOURNAL_OFFSITE_DST:?…}` ДО определения `alert()` —
# при отсутствии настройки `set -u` ронял скрипт на unset-переменной МОЛЧА (exit=1 без
# записи в `ALERT_FILE`/syslog/stderr). Сломанная настройка оставалась НЕВИДИМОЙ оператору
# и монитору — ровно тот класс «объявлено ≠ работает» (OPS-I-8), который verify_delivery_M-08
# шаг D5c и ловит. Контракт (M-92 §4 п.1–2, инвариант `JR-I-13` в `docs/fa/journal.md`):
# либо задан `RETENTION_REMOTE_SHA_CMD` (операторская команда, считающая суммы на УДАЛЁННОЙ
# стороне — Storage Box), либо оба `JOURNAL_OFFSITE_SSH_KEY` и `JOURNAL_OFFSITE_DST` (дефолт
# собирается из них). Без этого бинарь не зовётся даже в `dry-run` (dry-run всё равно дёргает
# `RETENTION_REMOTE_SHA_CMD` ради манифеста — без настройки манифест пуст, а бинарь с пустым
# манифестом отказывает: `b1`/`b2`). Поднимаем алерт и выходим ДО раскрытия `${:?}`.
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

# M-92 D5c: видимость сломанной настройки. Условие — НЕ задан `RETENTION_REMOTE_SHA_CMD` И
# при этом отсутствует хотя бы один из `JOURNAL_OFFSITE_SSH_KEY` или `JOURNAL_OFFSITE_DST`.
# `set -u` тут не выстрелит (проверка через `${VAR:-}`); `apply` И `dry-run` дальше НЕ идут —
# `exit 2` (конфиг-ошибка, отличимо от `1=arg/io` бинаря и `3=disk_pressure`). После этого
# гейт D5c ждёт: `exit≠0` И `ALERT_FILE` непуст. `HFT_CRON_PRINT_ARGV`/`RETENTION_PRINT_ARGV`
# обработаны ВЫШЕ отдельной веткой и до этой проверки не доходят — канарейка M-48 не страдает.
if [ -z "${RETENTION_REMOTE_SHA_CMD:-}" ] \
   && { [ -z "${JOURNAL_OFFSITE_SSH_KEY:-}" ] || [ -z "${JOURNAL_OFFSITE_DST:-}" ]; }; then
  alert "нет настройки офсайт-копии: задайте RETENTION_REMOTE_SHA_CMD или оба JOURNAL_OFFSITE_SSH_KEY и JOURNAL_OFFSITE_DST — apply/dry-run НЕ зовутся (M-92 D5c)"
  exit 2
fi

# M-92 (A-044 §5.1, §4 п.2): шов для гейта и прод-пути. Дефолт — SSH-команда, считающая
# `sha256sum` на СТОРОНЕ офсайт-копии (Storage Box). Префикс `journal/` снимается скриптом —
# см. §4 п.2: «префикс `journal/` снимается» относится к ВЫВОДУ команды, не к её аргументам.
# Команда получает `journal/<имя>` (как на проде), выводит `<sha>  journal/<имя>` (как
# `sha256sum` — два пробела), скрипт срезает префикс перед записью в манифест.
# `?:` сняты: к этой строке `JOURNAL_OFFSITE_SSH_KEY` и `JOURNAL_OFFSITE_DST` ГАРАНТИРОВАННО
# непусты (проверено строкой выше) — иначе скрипт уже ушёл в `exit 2` через `alert`.
RETENTION_REMOTE_SHA_CMD="${RETENTION_REMOTE_SHA_CMD:-ssh -i ${JOURNAL_OFFSITE_SSH_KEY} -o IdentitiesOnly=yes -o BatchMode=yes -o ConnectTimeout=10 -p ${JOURNAL_OFFSITE_SSH_PORT:-23} ${JOURNAL_OFFSITE_DST} sha256sum}"
# Рабочий каталог плана/манифеста — отдельным `mkdir` ПОСЛЕ дефолта `${RETENTION_WORK_DIR}`
# (раньше был в одной строке с LOG/ALERT_FILE/LAST_SUCCESS, но сейчас те — выше, до
# `RETENTION_WORK_DIR`).
mkdir -p "${RETENTION_WORK_DIR}" 2>/dev/null || true

# M-92 (A-044 §5.1, §4 п.3): режим — ТОЛЬКО из файла-переключателя. RETENTION_MODE в env
# (включая строку `RETENTION_MODE=dry-run` в `/etc/cron.d/journal-retention`) НЕ управляет
# удалением. Конструктивный барьер: деплой кода не смеет включить apply, даже если строка
# в cron-файле случайно изменится при следующем кодовом деплое.
read_mode() {
  if [ -f "${RETENTION_MODE_FILE}" ] && [ "$(tr -d '[:space:]' < "${RETENTION_MODE_FILE}")" = "apply" ]; then
    echo "apply"
  else
    echo "dry-run"
  fi
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
  --retain-days "${RETENTION_RETAIN_DAYS}"
  --keep-min "${RETENTION_KEEP_MIN}"
  --min-free-gb "${RETENTION_MIN_FREE_GB}"
  --mode dry-run
  # M-48 (GW-I-12): обязательно передаём путь к артефакту покрытия. Без этого
  # retention не знает, до какого seq безопасно прунить — fail-closed no-op.
  --checkpoint-coverage="${RETENTION_CHECKPOINT_COVERAGE}"
  # M-92 §4 п.1: dry-run с --plan-out — всегда (источник имён кандидатов для
  # манифеста). Пишется в RETENTION_WORK_DIR, оттуда читается на шаге 2.
  --plan-out "${RETENTION_WORK_DIR}/plan.txt"
)

cd "${HFT_ROOT}" 2>/dev/null || { alert "journal-retention: нет каталога ${HFT_ROOT}"; exit 1; }

# ─── ШАГ 1: dry-run с --plan-out — план ретеншена (M-92 §4 п.1) ─────────────────
# Бинарь пишет `${RETENTION_WORK_DIR}/plan.txt` — имена кандидатов, по одному в строке,
# ДО любых операций удаления. Файл переиспользуется на шагах 2/3/5.
${RETENTION_RUNNER} "${ARGV[@]}" >> "${LOG}" 2>&1
rc=$?
if [ "${rc}" -ne 0 ]; then
  alert "plan step exit=${rc} (1=arg/io, 3=disk_pressure). Лог: ${LOG}"
  exit "${rc}"
fi

PLAN_FILE="${RETENTION_WORK_DIR}/plan.txt"
if [ ! -s "${PLAN_FILE}" ]; then
  # План пустой — удалять нечего. Тихо выходим 0, без аудита (нечего фиксировать).
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  date -u +%Y-%m-%dT%H:%M:%SZ > "${LAST_SUCCESS}" 2>/dev/null || true
  exit 0
fi

# ─── ШАГ 2: манифест с УДАЛЁННОЙ стороны (M-92 §4 п.2) ──────────────────────────
# Суммы берутся НЕ с локальных файлов (тождественная сверка = R-180 F-1), а через
# `${RETENTION_REMOTE_SHA_CMD}` — SSH-команда к Storage Box. Префикс `journal/` в выводе
# команды снимается перед записью в манифест (`ColdManifest::parse` ожидает имя без `/`).
MANIFEST_FILE="${RETENTION_WORK_DIR}/manifest.txt"
: > "${MANIFEST_FILE}"
while IFS= read -r name || [ -n "${name}" ]; do
  [ -z "${name}" ] && continue
  if ! ${RETENTION_REMOTE_SHA_CMD} "journal/${name}" 2>>"${LOG}" \
       | sed 's#  journal/#  #' >> "${MANIFEST_FILE}"; then
    alert "RETENTION_REMOTE_SHA_CMD failed for journal/${name}; apply НЕ зовётся. Лог: ${LOG}"
    exit 1
  fi
done < "${PLAN_FILE}"

MODE=$(read_mode)

# ─── ШАГ 3: носитель аудита доступен? (A-044 O-3(b), M-92 §4 п.4) ───────────────
# Проверяем ДО записи плана/манифеста и ДО apply: если mkdir упадёт позже,
# apply уже мог быть вызван (полу-удалённое состояние). Fail-closed: тревога + exit≠0.
if ! mkdir -p "${RETENTION_AUDIT_DIR}" 2>/dev/null; then
  alert "RETENTION_AUDIT_DIR недоступен: ${RETENTION_AUDIT_DIR} — apply НЕ зовётся"
  exit 1
fi

# ─── ШАГ 4: уникальные БАЗОВЫЕ имена записей аудит-следа (A-045 §2.1, O-2) ──────
# Формат: `<UTC-timestamp>-<pid>` — PID уникален на процесс, дата — для человека.
# Ручной прогон и cron в ту же секунду имеют разные PIDs ⇒ коллизии невозможны.
# Подкаталог на прогон (`<run_id>/{plan,manifest,report}.txt`) НЕДОСТАТОЧЕН — `c4`
# сравнивает `file_name()` (плоские имена), а не пути относительно `RETENTION_AUDIT_DIR`.
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
AUDIT_PLAN="${RETENTION_AUDIT_DIR}/${RUN_ID}-plan.txt"
AUDIT_MANIFEST="${RETENTION_AUDIT_DIR}/${RUN_ID}-manifest.txt"
AUDIT_REPORT="${RETENTION_AUDIT_DIR}/${RUN_ID}-report.txt"

# План и манифест — в следе ДО apply (A-044 O-3(a)): иначе удаление, о котором
# не записано, нельзя ни восстановить, ни расследовать. `cp` — атомарная запись
# в пределах файловой системы: либо источник и приёмник совпадают, либо операция
# провалилась целиком (нет полу-записанного состояния).
if ! cp -f "${PLAN_FILE}" "${AUDIT_PLAN}" 2>>"${LOG}"; then
  alert "не удалось записать план в аудит-след: ${AUDIT_PLAN} — apply НЕ зовётся"
  exit 1
fi
if ! cp -f "${MANIFEST_FILE}" "${AUDIT_MANIFEST}" 2>>"${LOG}"; then
  alert "не удалось записать манифест в аудит-след: ${AUDIT_MANIFEST} — apply НЕ зовётся"
  exit 1
fi

# ─── ШАГ 5: apply или dry-run — единый вызов бинаря (M-92 §4 п.1) ───────────────
# `apply` отличается от шага 1 только режимом и наличием `--cold-manifest`. План уже
# записан (A-044 O-3a); дублировать `--plan-out` здесь — лишний I/O. Манифест —
# РОВНО тот файл, что уходит бинарю (O-3a: `manifest-consumed` побайтово равен
# записи в следе, проверяется `c1`/`c4` через `manifest-consumed`).
APPLY_ARGV=(
  --dir "${RETENTION_JOURNAL_DIR}"
  --retain-days "${RETENTION_RETAIN_DAYS}"
  --keep-min "${RETENTION_KEEP_MIN}"
  --min-free-gb "${RETENTION_MIN_FREE_GB}"
  --mode "${MODE}"
  --checkpoint-coverage="${RETENTION_CHECKPOINT_COVERAGE}"
  --cold-manifest "${MANIFEST_FILE}"
)

# Бинарь печатает `FAIL <path>: <reason>` для каждого сегмента, который НЕ удалось
# подтвердить манифестом (I-2, I-2bis). В `apply` это значит «удержан» (fail-closed);
# в `dry-run` — «был бы удержан». Парсим ВЫВОД, а не угадываем по файловой системе:
# в тесте `c1`/`c4` скрипт крутится на хосте и не имеет прямого доступа к фикстурному
# каталогу журнала (путь в контейнере `/journal` подменяется shim'ом только для
# бинаря, не для скрипта). Парсинг по FAIL — единственный способ отличить pruned
# от kept без зависимости от хоста.
BIN_OUTPUT="${RETENTION_WORK_DIR}/bin-output.txt"
${RETENTION_RUNNER} "${APPLY_ARGV[@]}" > "${BIN_OUTPUT}" 2>&1
rc=$?
cat "${BIN_OUTPUT}" >> "${LOG}"

# ─── ШАГ 6: отчёт — одна строка на имя плана (A-044 O-1, A-045 §2.1) ───────────
# Формат ДОСЛОВЕН: `pruned <имя>` или `kept <имя> <причина>`. Без заголовка,
# без комментариев, второй токен — голое имя файла. В `apply` `kept` ⇒ FAIL
# из бинаря; в `dry-run` `kept` ⇒ либо FAIL (был бы удержан), либо `dry-run`
# (был бы удалён, но dry-run). Известный предел (A-045 §5 п.3): при аварийном
# выходе бинаря (`rc ∉ {0,2}`) `print_report` мог не напечатать ни FAIL, ни
# чего-либо ещё — `pruned` тогда выводится из ОТСУТСТВИЯ FAIL, что может
# соврать. Лечится в зоне бинаря (задача 2, печать `PRUNED <путь>`).
declare -A KEPT_REASONS=()
while IFS= read -r line; do
  if [[ "${line}" =~ ^[[:space:]]*FAIL[[:space:]]+(.+)$ ]]; then
    rest="${BASH_REMATCH[1]}"
    fail_path="${rest%%: *}"
    fail_reason="${rest#*: }"
    fail_name="${fail_path##*/}"
    if [ -n "${fail_name}" ]; then
      KEPT_REASONS["${fail_name}"]="${fail_reason}"
    fi
  fi
done < "${BIN_OUTPUT}"

: > "${AUDIT_REPORT}"
while IFS= read -r name || [ -n "${name}" ]; do
  [ -z "${name}" ] && continue
  if [ -n "${KEPT_REASONS[${name}]:-}" ]; then
    echo "kept ${name} ${KEPT_REASONS[${name}]}" >> "${AUDIT_REPORT}"
  elif [ "${MODE}" = "apply" ]; then
    echo "pruned ${name}" >> "${AUDIT_REPORT}"
  else
    echo "kept ${name} dry-run" >> "${AUDIT_REPORT}"
  fi
done < "${PLAN_FILE}"

# ─── ШАГ 7: выход (A-044, M-92 §4 п.5) ──────────────────────────────────────────
# Ненулевой выход бинаря — тревога (как сегодня). Успешный прогон гасит маркер и
# обновляет heartbeat. Отчёт аудит-следа записан ВСЕГДА (даже при `rc≠0`) —
# иначе расследование инцидента потеряет фактический результат прогона.
if [ "${rc}" -ne 0 ]; then
  alert "exit=${rc} (2=failed_cold_verify — сегмент остался ГОРЯЧИМ; 3=disk_pressure; 1=arg/io). Отчёт: ${AUDIT_REPORT}. Лог: ${LOG}"
else
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  date -u +%Y-%m-%dT%H:%M:%SZ > "${LAST_SUCCESS}" 2>/dev/null || true
fi
exit "${rc}"
