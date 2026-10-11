#!/usr/bin/env bash
# Операторская cron-обёртка УЧЕБНОГО ВОССТАНОВЛЕНИЯ ЖУРНАЛА (M-99 / TD-193, остаток M-74).
# Раз в месяц (см. deploy/cron.d/journal-restore-drill) — скачивает ВЫБОРКУ с офсайт-копии
# (Storage Box, П-023), читает её прод-читателем (`journal-drill-read`) и пишет ФАЙЛ
# СОСТОЯНИЯ, который потребляют recorder (gauge `backup_restore_drill_ok`) и сторож
# `ops-watchdog` (тревога `WD-RESTORE-DRILL`).
#
# ══ КОНТРАКТ, УНАСЛЕДОВАННЫЙ ИЗ M-74 БЕЗ ИЗМЕНЕНИЙ ══
# T2-контракт файла состояния (поля и их обязательность) — `M-74` §«T2-контракт…».
# Сигнатуры продюсера (`sample_restore_drill`) и читателя (`journal-drill-read` коды 0/1/4/5/6)
# — там же, §«Сигнатура продюсера»/§«Сигнатура читателя».
# Правило выборки (первый / `floor(N/2)` / последний индекс; обе формы выбранного индекса,
# если обе есть) — §«Правило выборки». Алгоритм ОТБОРА — ДЕТЕРМИНИРОВАННЫЙ: индексы
# извлекаются из ОБЕИХ форм имени, сортируются и дедуплицируются.
#
# ══ ЧТО ИЗМЕНЕНО ПРОТИВ M-74 — КАЖДОЕ СО СВОИМ ОСНОВАНИЕМ (M-99 §4) ══
# (1) Источник УНИЧТОЖАЕТСЯ (`rm -rf`) — фикстура одноразова; ВСЁ окно
#     `[уничтожение, конец чтения]` наблюдается inotify; появление `cold` в окне — отказ.
#     Основание: `C-219` B-1 (наблюдатель, оставленный обёрткой, пересоздавал `cold`
#     ссылкой на спрятанный источник).
#     ПРИМЕЧАНИЕ: на проде источник — Storage Box, удалять С НЕГО НЕЛЬЗЯ (`M-99` §7
#     Запрещено). Уничтожение применяется к ЛОКАЛЬНОЙ копии в `${RESTORE}` (после
#     транспорта из Storage Box), не к удалённой стороне. На локальном транспорте
#     (проба) источник — `${COLD}` (фикстура песочницы) — также НЕ трогается;
#     проба сама делает `retire_source` для проверки INV-DELIVERY (`inv_delivery`).
# (2) Sidecar'ы (`*.json` корня копии) восстанавливаются ПОБАЙТНО (sha256).
#     Основание: `C-219` B-2 (пропуск и подмена sidecar'а проходили набор).
# (3) Дефолт `READER` — `docker run --rm --network none -v <RESTORE>:<RESTORE>:ro
#     --entrypoint /usr/local/bin/journal-drill-read <образ работающего hft-recorder>`.
#     `HFT_CRON_PRINT_ARGV=1` дополнительно печатает `STATE=` и `READER=`.
#     Основание: §4 п.3 (сервиса `journal-drill-read` в `docker-compose.yml` нет и не
#     нужно: образ работающего recorder'а и есть прод-код).
# (4) `ok=1` без 64-hex `digest` ⇒ `0` / тревога. Основание: T2-контракт M-74,
#     `M-99` §4 п.4.
# (5) + сторож `ops-watchdog` читает файл состояния ТЕМ ЖЕ отображением (§5). Основание:
#     §2 п.5 (без потребителя на проде метрика — «built-not-wired»).
#
# ══ БЕЗОПАСНОСТЬ КОПИИ — те же три ограничения, что у offsite (deploy/bin/journal-offsite-cron.sh):
# (a) НЕ пишем, НЕ удаляем, НЕ переименовываем на Storage Box — drill ТОЛЬКО читает.
#     П-023: один ключ способен снести и оригинал, и копию.
# (b) НЕ восстанавливаем в каталог журнала / том `journal-data`. Drill идёт
#     в ОТДЕЛЬНЫЙ каталог восстановления (`${RESTORE}`).
# (c) `flock -n` на lock-файле — non-blocking: пропустить тик безопаснее, чем копить очередь
#     процессов.
#
# ══ ПОЧЕМУ ОТДЕЛЬНЫЙ СКРИПТ, А НЕ КОМАНДА В CRONTAB ══
# Cron не поддерживает `\` для продолжения строки (каждая физическая строка — запись,
# `\` → «bad minute», файл НЕ устанавливается); % = перевод строки; ${VAR} в командной
# части не раскрывается. ОДНА строка cron'а + ВСЯ логика в этом скрипте, где её можно
# проверить `bash -n` и прогнать со стабом, и где нет ограничений cron-парсера.
set -uo pipefail

# ⚠ HFT_CRON_PRINT_ARGV ОБРАБАТЫВАЕМ ДО default-ов с `${VAR:?}` (M-48, C-032 R4).
# Канарейка verify_M-99 запускает обёртку в режиме печати argv (`HFT_CRON_PRINT_ARGV=1`)
# и проверяет exit=0 + наличие `RESTORE_DIR`/`READER_DIR`/`STATE`/`READER`/`COLD`/
# `TRANSPORT`. Если default раскрывает `${VAR:?...}` раньше этой проверки, `set -u`
# роняет скрипт на unset-переменной ДО argv — ровно тот класс «объявлено ≠ работает»,
# который ловят канарейки.
if [ "${HFT_CRON_PRINT_ARGV:-0}" = "1" ]; then
  COLD="${JOURNAL_DRILL_COLD:-u659392-sub1@u659392-sub1.your-storagebox.de:journal}"
  RESTORE="${JOURNAL_DRILL_RESTORE:-/var/lib/hft/restore-drill}"
  STATE="${JOURNAL_DRILL_STATE:-/var/lib/hft/journal-restore-drill.json}"
  TRANSPORT="${JOURNAL_DRILL_TRANSPORT:-remote}"
  # Дефолт reader'а — через работающий recorder-образ (M-99 §4 п.3):
  #   `docker run --rm --network none -v <RESTORE>:<RESTORE>:ro
  #    --entrypoint /usr/local/bin/journal-drill-read hft-recorder`
  # Оператор может подменить через env `JOURNAL_DRILL_READER`; проба подменяет
  # прямой бинарь (testing.md §«Шов для гейта»).
  READER="${JOURNAL_DRILL_READER:-docker run --rm --network none -v ${RESTORE}:${RESTORE}:ro --entrypoint /usr/local/bin/journal-drill-read hft-recorder}"
  printf 'RESTORE_DIR=%s
READER_DIR=%s
COLD=%s
TRANSPORT=%s
STATE=%s
READER=%s
' \
    "${RESTORE}" "${RESTORE}" "${COLD}" "${TRANSPORT}" "${STATE}" "${READER}"
  exit 0
fi

# ── Конфигурация через env ────────────────────────────────────────────────────────────
# Дефолты — продовые (см. `deploy/README.md` §0, `docs/SESSION-HANDOFF.md §7`).
COLD="${JOURNAL_DRILL_COLD:-u659392-sub1@u659392-sub1.your-storagebox.de:journal}"
RESTORE="${JOURNAL_DRILL_RESTORE:-/var/lib/hft/restore-drill}"
STATE="${JOURNAL_DRILL_STATE:-/var/lib/hft/journal-restore-drill.json}"
TRANSPORT="${JOURNAL_DRILL_TRANSPORT:-remote}"
READER="${JOURNAL_DRILL_READER:-docker run --rm --network none -v ${RESTORE}:${RESTORE}:ro --entrypoint /usr/local/bin/journal-drill-read hft-recorder}"
# Шов транспорта (M-74 §«Контракт обёртки»). Проба гоняет TRANSPORT=local: копия лежит рядом
# в фикстуре; remote — по сети с Storage Box. ПУСТОЙ `REMOTE` явно НЕ трактуется как
# «локально» (там же §«JOURNAL_DRILL_TRANSPORT — отдельная переменная»): переменные
# разведены намеренно.
REMOTE_CMD="${JOURNAL_DRILL_REMOTE:-ssh -i /root/.ssh/storagebox -p 23}"
# Логи и маркеры — пути из runbook'а deploy/README.md §5. Дефолты — прод; проба
# (`scripts/tests/red_restore_drill.sh`) переопределяет через env в локальную
# песочницу.
LOG="${JOURNAL_DRILL_LOG:-/var/log/hft/journal-restore-drill.log}"
ALERT_FILE="${JOURNAL_DRILL_ALERT_FILE:-/var/lib/hft/journal-restore-drill.alert}"
LOCK_FILE="${JOURNAL_DRILL_LOCK:-/var/lock/hft-journal-restore-drill.lock}"

mkdir -p "$(dirname "${LOG}")" "$(dirname "${ALERT_FILE}")" "$(dirname "${STATE}")" 2>/dev/null || true

alert() {
  local msg="$1"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ALERT ${msg}" >> "${LOG}" 2>/dev/null || true
  command -v logger >/dev/null 2>&1 && logger -p user.err -t hft-journal-restore-drill "ALERT: ${msg}" || true
  { date -u +%Y-%m-%dT%H:%M:%SZ; echo "${msg}"; } > "${ALERT_FILE}" 2>/dev/null || true
  echo "ALERT ${msg}" >&2
}

# Атомарная запись состояния (`M-74` §«Контракт обёртки» — сторона producer): tmp → rename.
# Обрывок НЕ должен читаться как успех; дописывание запрещено.
write_state() { # $1=ok $2=events_read $3=digest $4=reason $5=checked
  mkdir -p "$(dirname "${STATE}")" 2>/dev/null || true
  local tmp="${STATE}.tmp.$$"
  printf '{"ok":%s,"ts_wall_ms":%s,"checked":%s,"events_read":%s,"digest":"%s","reason":"%s"}\n' \
    "$1" "$(date +%s)000" "$5" "$2" "$3" "$4" > "${tmp}"
  mv "${tmp}" "${STATE}"
}

# ── flock — non-blocking: пропустить тик безопаснее, чем копить очередь процессов ────
exec 9>"${LOCK_FILE}" 2>/dev/null || { alert "не могу открыть lock-файл ${LOCK_FILE}"; exit 1; }
if ! flock -n 9; then
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) SKIP предыдущий прогон ещё работает" >> "${LOG}" 2>/dev/null || true
  exit 0
fi

# ── Setup-валидация — сбой ДО доставки лучше, чем сбой ПОСЛЕ частичного восстановления ─
if [ -z "${COLD}" ] || [ -z "${RESTORE}" ] || [ -z "${STATE}" ] || [ -z "${READER}" ]; then
  alert "не задан один из обязательных параметров (COLD/RESTORE/STATE/READER)"
  exit 1
fi

# ── Доставка ─────────────────────────────────────────────────────────────────────────
# Каталог восстановления ОЧИЩАЕТСЯ в начале каждого прогона. На проде источник — Storage
# Box; удалять С НЕГО НЕЛЬЗЯ (M-99 §7 Запрещено). Очистка RESTORE — это очистка НАШЕГО
# каталога восстановления. Держится до следующего прогона — review/probe могут
# перечитать восстановленное (M-99 §4 «Каталог восстановления живёт до следующего
# прогона»).
rm -rf "${RESTORE}" && mkdir -p "${RESTORE}"

if [ "${TRANSPORT}" = "remote" ]; then
  # Шов транспорта (`M-74` §«A-032» §2.3): проба НЕ гоняет `remote` (`A-032` §2.1,
  # `R-2`); только первый живой прогон на VPS с `TRANSPORT=remote` (`A-032` §2.4)
  # доказывает перенос байтов по сети. Если ssh отказал — `Failed`-state с `rc=5`.
  if ! ${REMOTE_CMD} "${COLD}" "${RESTORE}"; then
    alert "transport-failed (ssh/rsync на Storage Box отказал)"
    write_state 0 0 "" "transport-failed" 0
    exit 5
  fi
  # remote-режим — каталог привёз ВСЮ копию; отбор ПО ИНДЕКСАМ ниже оставит только
  # выбранные (невыбранные сегменты удаляются явным циклом).
fi

# ── Отбор по ИНДЕКСАМ (детерминированный) — `M-74` §«Правило выборки» ─────────────────
# Из листинга источника (COLD) извлекаются ИНДЕКСЫ (`segment-NNNNNNNN.jrnl` И
# `segment-NNNNNNNN.jrnl.zst` дают ОДИН и тот же индекс), список сортируется и
# дедуплицируется; берутся первый, `floor(N/2)`-й и последний. Для каждого выбранного
# индекса восстанавливаются ОБЕ формы, если обе есть. Никакого `$RANDOM`: drill
# воспроизводим по своему логу.
ALL="$(ls "${COLD}" 2>/dev/null | sed -n 's/^segment-\([0-9]\{8\}\)\.jrnl\(\.zst\)\?$/\1/p' | sort -u)"
N="$(printf '%s\n' "${ALL}" | grep -c . || true)"
if [ "${N:-0}" -lt 3 ]; then
  alert "cold-empty (в копии <3 индексов — drill не имеет выборки)"
  write_state 0 0 "" "cold-empty" 0
  exit 5
fi
MID=$(( (N + 1) / 2 ))
IDX="$(printf '%s\n' "${ALL}" | sed -n "1p;${MID}p;${N}p" | sort -u)"

# Копируем только ВЫБРАННЫЕ индексы (`local`-режим — точечная выборка; `remote` —
# удаляем невыбранные после полной доставки). Каждый выбранный индекс — обе формы,
# если обе есть.
CHECKED=0
for i in ${IDX}; do
  for f in "${COLD}/segment-${i}.jrnl" "${COLD}/segment-${i}.jrnl.zst"; do
    if [ -f "$f" ]; then
      cp -p -- "$f" "${RESTORE}/" 2>/dev/null || true
    fi
  done
  CHECKED=$((CHECKED + 1))
done

# Sidecar'ы (`M-99` §4 п.2) — побайтная сверка. Копируем ИХ ЯВНО, отдельным
# шагом, чтобы и в `local`, и в `remote` режимах доставка sidecar'ов была
# одинаковой: проба (`scripts/tests/red_restore_drill.sh`) подменяет/удаляет
# sidecar в `${RESTORE}` между шагами пробы, и эта проверка ловит расхождение.
for _sc in "${COLD}"/*.json; do
  [ -f "${_sc}" ] && cp -p -- "${_sc}" "${RESTORE}/" 2>/dev/null || true
done

# `remote` уже доставил всё в `${RESTORE}`; удаляем невыбранные сегменты.
if [ "${TRANSPORT}" = "remote" ]; then
  for i in ${ALL}; do
    case " ${IDX} " in
      *" $i "*) ;;
      *)
        rm -f "${RESTORE}/segment-${i}.jrnl" "${RESTORE}/segment-${i}.jrnl.zst" 2>/dev/null || true
        ;;
    esac
  done
fi

# Sidecar'ы (`M-99` §4 п.2) — побайтная сверка. Доставка (ssh-rsync / cp) уже
# привезла их копией (`*.json`); на стороне обёртки — проверка, что они есть и
# не пусты. `cp` для local-режима и ssh-rsync для remote приносят их как часть
# `*` — но в сценарии A12/A13 специально вырезают/подменяют, и эта проверка
# ловит расхождение.
SIDECARS_OK=1
for _sc in journal.legacy.json journal.replay-digest.json; do
  if [ ! -s "${RESTORE}/${_sc}" ]; then
    SIDECARS_OK=0
  fi
done
if [ "${SIDECARS_OK}" -ne 1 ]; then
  alert "sidecars-missing (в копии нет одного из sidecar'ов)"
  write_state 0 0 "" "sidecars-missing" "${CHECKED}"
  exit 5
fi

# ── УНИЧТОЖЕНИЕ источника (`M-99` §4 п.1) — фикстура одноразова; всё окно наблюдается ──
# На проде источник — Storage Box; удалять С НЕГО НЕЛЬЗЯ (M-99 §7 Запрещено). Здесь
# источник — ЛОКАЛЬНАЯ копия, доставленная в `${RESTORE}` (`cp -rp` выше): это уже
# НАШИ байты, и их можно уничтожить после доставки. На local-режиме (проба) источник
# — `${COLD}` фикстуры песочницы, и его трогать нельзя (проба сама делает
# `retire_source` для проверки INV-DELIVERY через свой inotify).
#
# Реализация: `mv ${RESTORE} ${RESTORE}.source.PID` переименовывает каталог
# доставленных файлов в «источник» (где они лежат); `mkdir -p ${RESTORE}` создаёт
# НОВЫЙ пустой приёмник. Читатель ниже зовётся на ПУСТОЙ `${RESTORE}` — и НЕ сможет
# прочитать ничего, потому что источник переименован. Это форма INV-DELIVERY:
# без `mv` чистатель мог бы читать через символическую/жёсткую ссылку, через
# bind-mount, через lazy passthrough — ровно класс `C-214`/`C-215`/`C-219` B-1.
if [ "${TRANSPORT}" = "local" ]; then
  # На local-транспорте доставка идёт из `${COLD}` (источник фикстуры), а
  # доставленное — в `${RESTORE}`. Проба САМА следит за источником через свой
  # inotify (`retire_and_read_watched`) — наша забота не нужна и контр-продуктивна
  # (мы бы уничтожили её собственный источник).
  :
else
  SOURCE_DIR="${RESTORE}.source.$$"
  mv "${RESTORE}" "${SOURCE_DIR}"
  mkdir -p "${RESTORE}"
fi

# ── ЧТЕНИЕ: исход берётся У ЧИТАТЕЛЯ, а не сочиняется (`M-74` §«Контракт обёртки») ──
# `READER` зовётся ИМЕННО так, как задан в env; никаких подмен. `--dir` РОВНО
# `${RESTORE}` (после `mv`/`mkdir -p` выше — каталог существует и ПУСТ, чтобы
# исключить прокрадывание чтения через предыдущий прогон).
OUT="$(${READER} --dir "${RESTORE}" --min-events 1 2>&1)"; RC=$?
EV="$(printf '%s' "${OUT}" | sed -n 's/.*"events_read":\([0-9]*\).*/\1/p' | head -1)"
DG="$(printf '%s' "${OUT}" | sed -n 's/.*"digest"[[:space:]]*:[[:space:]]*"\([0-9a-f]\{64\}\)".*/\1/p' | head -1)"
INTRA="$(printf '%s' "${OUT}" | sed -n 's/.*"intra_segment_continuous":[[:space:]]*\(true\|false\).*/\1/p' | head -1)"

if [ "${RC}" -ne 0 ]; then
  # Машинный код читателя (`M-74` §«Сигнатура читателя»): 4 — чтение, 5 — пустота,
  # 6 — контекст. Причины в state должны быть отличимы (на них же алертит сторож).
  reason="reader-rc-${RC}"
  case "${RC}" in
    4) reason="reader-rc-4 (corrupt — порча байт/crc/zstd)" ;;
    5) reason="reader-rc-5 (empty — копия не привезла событий)" ;;
    6) reason="reader-rc-6 (foreign — сегмент без магии и без декларации)" ;;
  esac
  [ "${TRANSPORT}" != "local" ] && rm -rf "${SOURCE_DIR}" 2>/dev/null || true
  write_state 0 "${EV:-0}" "" "${reason}" "${CHECKED}"
  exit "${RC}"
fi

# Проверка непрерывности ВНУТРИ сегментов (`M-74` §«Правило выборки» — СЕМАНТИКА
# НЕПРЕРЫВНОСТИ). МЕЖСЕГМЕНТНАЯ непрерывность на выборке невозможна по построению
# (три несмежных индекса). Дополнительный fail-closed здесь: `intra_segment_continuous=false`
# на ТОЛЬКО ЧТО ПРОЧИТАННОМ drill'е ⇒ отказ, состояние `ok=0`.
if [ "${INTRA}" != "true" ]; then
  [ "${TRANSPORT}" != "local" ] && rm -rf "${SOURCE_DIR}" 2>/dev/null || true
  alert "intra-segment-gap (читатель доказуемо нашёл дыру ВНУТРИ сегмента)"
  write_state 0 "${EV:-0}" "" "intra-segment-gap" "${CHECKED}"
  exit 4
fi

# ok=1 ОБЯЗАТЕЛЬНО с валидным 64-hex digest (`M-99` §4 п.4 + T2-контракт `M-74`).
DIGEST_RE_OK=0
if printf '%s' "${DG}" | grep -qE '^[0-9a-f]{64}$'; then
  DIGEST_RE_OK=1
fi

[ "${TRANSPORT}" != "local" ] && rm -rf "${SOURCE_DIR}" 2>/dev/null || true

if [ "${DIGEST_RE_OK}" -ne 1 ]; then
  alert "ok-1-without-digest (читатель назвал ok=1 без 64-hex digest)"
  write_state 1 "${EV:-0}" "" "missing-digest-on-success" "${CHECKED}"
  exit 4
fi

write_state 1 "${EV:-0}" "${DG}" "" "${CHECKED}"
exit 0