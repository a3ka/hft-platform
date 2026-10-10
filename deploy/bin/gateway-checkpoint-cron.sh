#!/usr/bin/env bash
# Операторский cron-обёртка для gateway-checkpoint (M-48, TD-048, GW-I-12; M-94).
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
#
# M-94 (П-032, §3.5): смена профиля с новым именем слепка — «прогреть, потом
# переключить». Если в `$HFT_ROOT/config/calc-profile/next.env` лежит файл,
# обёртка делает ВТОРОЙ прогон с `-e GATEWAY_CALC_PROFILE=/etc/hft/calc-profile/next.env`
# (контракт argv runner'а: `-e KEY=VALUE` ДО имени сервиса). Покрытие
# второго прогона пишется в ОТДЕЛЬНЫЙ файл, чтобы курсор `next` не сдвинул
# файл, который читает ретеншен (милестоун §3.5: «покрытие `next` НЕ смеет
# идти в путь ретеншена»). Поскольку `--ckpt-dir` тот же, оба слепка
# (старого и нового профиля) сосуществуют в одном томе; `--coverage-out`
# разный — `/ckpt/covered_through_seq.next` (RW-том, отдельный файл).
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
# M-94 (П-032, §3.5): путь покрытия для ВТОРОГО прогона (`next.env`). Тот же
# RW-том `gateway-ckpt`, НО ОТДЕЛЬНЫЙ файл — чтобы курсор `next` (всегда ДАЛЬШЕ
# курсора `active`, прогрев идёт поверх свежего хвоста) НЕ двигал файл, который
# читает ретеншен. Гейт деплоя смотрит в `active` (`/ckpt/covered_through_seq`).
CHECKPOINT_COVERAGE_OUT_NEXT="${CHECKPOINT_COVERAGE_OUT_NEXT:-/ckpt/covered_through_seq.next}"
# `--cursor LATEST` — прод-дефолт (снимаем чекпоинт ДО хвоста). Усечённый
# прогон возможен через `--cursor <i64>` (операторская диагностика; команда
# `gateway-checkpoint-cron.sh --cursor <seq>` ниже поддерживает это через env
# CHECKPOINT_CURSOR).
CHECKPOINT_CURSOR="${CHECKPOINT_CURSOR:-LATEST}"
# M-94 (П-032, §3.4): путь профиля ВНУТРИ контейнера для ОСНОВНОГО прогона
# (compose объявляет GATEWAY_CALC_PROFILE с этим путём; здесь — дубль для
# печати argv и обвязки `next.env`).
CHECKPOINT_CALC_PROFILE="${CHECKPOINT_CALC_PROFILE:-/etc/hft/calc-profile/active.env}"
# M-94 (§3.5): путь профиля `next` ВНУТРИ контейнера — фиксирован Dockerfile
# COPY (`config/calc-profile/` → `/etc/hft/calc-profile/`).
CHECKPOINT_CALC_PROFILE_NEXT="${CHECKPOINT_CALC_PROFILE_NEXT:-/etc/hft/calc-profile/next.env}"
# Путь к `next.env` НА ХОСТЕ (для проверки наличия). Хост-каталог —
# `$HFT_ROOT/config/calc-profile/` (милестоун §3.5). Каталог может НЕ существовать
# — это легитимное «нет next», без него обёртка делает ровно ОДИН прогон,
# как до M-94.
CHECKPOINT_NEXT_ENV_HOST="${CHECKPOINT_NEXT_ENV_HOST:-${HFT_ROOT}/config/calc-profile/next.env}"

# ⚠ СЕЛЕКТОР ПРОГРЕВАТЕЛЯ — НЕ ЗДЕСЬ (M-90, TD-227; M-94 §3.1).
#
# Поля селектора (`venue`/`symbol`/`timeframe_ms`/`bands`/`window_ms`/
# `depth_cadence_ms`) читаются бинарём `gateway-checkpoint` из `GATEWAY_*` env,
# которую compose ОБЪЯВЛЯЕТ в `environment:` сервиса `gateway-checkpoint`
# (с теми же дефолтами, что у `gateway-serve`). M-94: 7 GATEWAY_* — теперь
# В ПРОФИЛЕ (`config/calc-profile/active.env`), на что указывает
# `GATEWAY_CALC_PROFILE`. Собственная копия селектора в этом скрипте = два
# источника одной величины = ровно класс `TD-227`. Ниже — `I-3`: на любую
# из шести `CHECKPOINT_*` селектора в окружении cron'а — отказ с именем
# переменной, runner НЕ зовётся.
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

# I-3 (M-90, TD-227): ОТКАЗ на любую из шести `CHECKPOINT_*` селектора в cron'е.
# Проверка ВЫШЕ `HFT_CRON_PRINT_ARGV` — оператор задал запрещённую переменную,
# печатать argv нечего: всё равно exit≠0, прогреватель не зовётся.
# Список шести — КОНТРАКТ: ровно эти шесть имён читает `selector_fingerprint`
# (`crates/gateway/src/lib.rs:4252-4274`). Любая ось, не названная здесь, к селектору
# НЕ относится и не отвергается (`CHECKPOINT_JOURNAL_DIR`/`_CKPT_DIR`/`_COVERAGE_OUT`
# — пути; `CHECKPOINT_LOG`/`_ALERT_FILE`/`_LAST_SUCCESS` — observability;
# `CHECKPOINT_CURSOR` — операторская диагностика `--cursor=<seq>`;
# `CHECKPOINT_RUNNER` — шов гейта; `CHECKPOINT_CALC_PROFILE`/`_NEXT`/`_NEXT_ENV_HOST` —
# шов M-94).
for var in CHECKPOINT_VENUE CHECKPOINT_SYMBOL CHECKPOINT_TIMEFRAME_MS \
           CHECKPOINT_BANDS CHECKPOINT_WINDOW_MS CHECKPOINT_DEPTH_CADENCE_MS; do
  eval "val=\${$var:-}"
  if [ -n "$val" ]; then
    alert "${var} в окружении cron'а ЗАПРЕЩЕНА — собственный источник селектора запрещён (TD-227, M-90 I-3). Источник — GATEWAY_* из host .env через compose environment: сервиса gateway-checkpoint."
    exit 1
  fi
done

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
#
# M-90 (TD-227): флагов селектора (`--venue`/`--symbol`/`--timeframe-ms`/`--bands`/
# `--window-ms`/`--depth-cadence-ms`) здесь НЕТ и быть не может. Бинарь читает их
# из `GATEWAY_*` env, которую compose ОБЪЯВЛЯЕТ в `environment:` сервиса
# `gateway-checkpoint` (тот же источник, что у `gateway-serve`). M-94: 7
# GATEWAY_* теперь в профиле (через GATEWAY_CALC_PROFILE), а этот скрипт
# пути НЕ передаёт — runner зовёт бинарь с путями + `--cursor`, а тот
# читает `GATEWAY_CALC_PROFILE` из env (compose-`environment:`) и грузит
# профиль. Скрипт передаёт только пути, `--coverage-out` и `--cursor` —
# это НЕ селектор.
ARGV=(
  --dir "${CHECKPOINT_JOURNAL_DIR}"
  --ckpt-dir "${CHECKPOINT_CKPT_DIR}"
  --coverage-out="${CHECKPOINT_COVERAGE_OUT}"
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

# M-94 (П-032, §3.5): если есть `next.env` — ВТОРОЙ прогон с переопределённым
# `GATEWAY_CALC_PROFILE` (идёт ДО имени сервиса, чтобы docker compose run
# распространил его на процесс), с тем же `--ckpt-dir` (композиция с
# гейтом: слепки обоих профилей лежат в КОРНЕ тома, оракул `p8b`/`Y2`)
# и ОТДЕЛЬНЫМ `--coverage-out` (чтобы курсор `next` не двигал файл ретеншена).
# Это «прогреть, потом переключить» — `next` собирается заранее, файл
# `active` освежается ПАРАЛЛЕЛЬНО, деплой может переключиться, не дожидаясь
# холодной пересборки (≈16–23 мин, замер `M-84`/`R-187`). Старый слепок
# остаётся на диске (§5: «старый слепок живёт до приёмки; удаления не
# существует и не вводится»).
NEXT_ARGV=()
for arg in "${ARGV[@]}"; do
  case "$arg" in
    --coverage-out=*) NEXT_ARGV+=("--coverage-out=${CHECKPOINT_COVERAGE_OUT_NEXT}") ;;
    *)                 NEXT_ARGV+=("$arg") ;;
  esac
done

if [ -f "${CHECKPOINT_NEXT_ENV_HOST}" ]; then
  # Контейнерный путь `next.env` (`/etc/hft/calc-profile/next.env`) — Dockerfile
  # COPY кладёт `config/calc-profile/` целиком; имя файла — `next.env`.
  # `-e KEY=VALUE` ДО имени сервиба (compose-конвенция и контракт пробы
  # `red_m94_calc_profile_warmer::is_next`, которая ищет `-e GATEWAY_CALC_PROFILE=...
  # next.env` через `split_compose_run` — тот берёт opts только ДО service name).
  # Парсер `gateway-checkpoint` читает `GATEWAY_CALC_PROFILE` из env процесса,
  # который compose-`run` пробрасывает из `-e`. Прогон `next` использует ТЕ ЖЕ
  # пути, что и `active` (тот же `--ckpt-dir`, тот же `--dir`); отличается
  # только `--coverage-out` (отдельный файл — см. комментарий выше).
  #
  # Конструкция `<runner> -e KEY=VAL <service> <args>` — НЕ то же, что
  # `<runner> <service> -e KEY=VAL <args>`: в первой `-e` стоит ДО service
  # name и попадает в `docker compose run` как env-флаг, во второй — после,
  # и `docker compose run` его не интерпретирует. Тест `p8`/`p8b` смотрит в
  # opts[-сервис] и не найдёт `-e` во втором варианте.
  #
  # Команда: `CHECKPOINT_RUNNER -e GATEWAY_CALC_PROFILE=… <service> <NEXT_ARGV>`.
  # CHECKPOINT_RUNNER — шов (compose или прямой бинарь); для compose форма
  # `docker compose run --rm -e KEY=VAL <service>` валидна и пробрасывает env.
  # shellcheck disable=SC2086 — намеренное расщепление `CHECKPOINT_RUNNER`
  # (это команда с аргументами; кавычки на нём запрещены).
  # shellcheck disable=SC2046 — `printf %q` ниже НЕ используется: имя файла
  # `next.env` не содержит whitespace на проде, и путь `/etc/hft/...` тоже.
  NEXT_RUNNER="${CHECKPOINT_RUNNER/%gateway-checkpoint/-e GATEWAY_CALC_PROFILE=${CHECKPOINT_CALC_PROFILE_NEXT} gateway-checkpoint}"
  ${NEXT_RUNNER} "${NEXT_ARGV[@]}" >> "${LOG}" 2>&1
  rc_next=$?
  # Отказ прогона `next` НЕ отменяет успех `active` (милестоун §3.5: «прогон
  # next может упасть легитимно, например EROFS на :ro томе журнала; выдача
  # работает на active»). В алерт выводим, но `rc` оставляем как есть.
  if [ "${rc_next}" -ne 0 ]; then
    alert "прогон next.env упал: exit=${rc_next} (1=argv/IO/EROFS, 2=профиль/ось). Активный слепок не тронут. Лог: ${LOG}"
  fi
fi

if [ "${rc}" -ne 0 ]; then
  alert "exit=${rc} (1=argv/IO, 2=validate_selector fail-closed GW-I-10, 1=advance_to fail-loud GW-I-12 — разрыв «чекпоинт↔журнал»). Лог: ${LOG}"
else
  # Успешный прогон гасит маркер — следующий сбой поднимет тревогу заново.
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  # Позитивный heartbeat (D9).
  date -u +%Y-%m-%dT%H:%M:%SZ > "${LAST_SUCCESS}" 2>/dev/null || true
fi
exit "${rc}"
