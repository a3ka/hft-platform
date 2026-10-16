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
# — там же, §«Сигнатура продюсера»/«Сигнатура читателя».
# Правило выборки (первый / `floor(N/2)` / последний индекс; обе формы выбранного индекса,
# если обе есть) — §«Правило выборки». Алгоритм ОТБОРА — ДЕТЕРМИНИРОВАННЫЙ: индексы
# извлекаются из ОБЕИХ форм имени, сортируются и дедуплицируются.
#
# ══ ЧТО ИЗМЕНЕНО ПРОТИВ M-74 — КАЖДОЕ СО СВОИМ ОСНОВАНИЕМ (M-99 §4) ══
# (1) Источник НЕ ТРОГАЕТСЯ ОБЁРТКОЙ — ни до, ни после чтения. На проде источник —
#     Storage Box, удалять С НЕГО НЕЛЬЗЯ (`M-99` §7 Запрещено). Каталог восстановления
#     `${RESTORE}` ЖИВЁТ до следующего прогона (§4 «Каталог восстановления живёт до
#     следующего прогона»). УНИЧТОЖЕНИЕ источника в ПРОБЕ делает сама проба
#     (`scripts/tests/red_restore_drill.sh::retire_and_read_watched`) — её
#     ответственность, не обёртки: проба следит за `cold` через inotify и судит
#     `inv_delivery` по `d_after = d_live`. Уничтожение доставленного в `${RESTORE}`
#     ПЕРЕД чтением — та ошибка круга 1 (`R-263` Б-1), на которую `RT` теперь КРАСЕН:
#     мутант «прежнее переименование доставленного» ловится `RT` через `reader-rc-5
#     (empty)`. Основание стража — `C-219` B-1 (наблюдатель, оставленный обёрткой,
#     пересоздавал `cold` ссылкой на спрятанный источник): ответственность пробы, не
#     обёртки.
# (2) Sidecar'ы (`*.json` корня копии) восстанавливаются ПОБАЙТНО (sha256).
#     Основание: `C-219` B-2 (пропуск и подмена sidecar'а проходили набор).
# (3) Дефолт `READER` — `docker run --rm --network none -v <RESTORE>:<RESTORE>:ro
#     --entrypoint /usr/local/bin/journal-drill-read <image сервиса recorder в
#     docker-compose.yml>` (R-263 Б-3). `HFT_CRON_PRINT_ARGV=1` дополнительно
#     печатает `STATE=` и `READER=`. Имя образа берётся из `docker-compose.yml`
#     (`image:` сервиса `recorder`); на проде сегодня — `hft-platform-recorder:local`.
#     Основание: §4 п.3 (сервиса `journal-drill-read` в `docker-compose.yml` нет и не
#     нужно: образ работающего recorder'а и есть прод-код).
# (4) `ok=1` без 64-hex `digest` ⇒ `0` / тревога. Основание: T2-контракт M-74,
#     `M-99` §4 п.4.
# (5) + сторож `ops-watchdog` читает файл состояния ТЕМ ЖЕ отображением (§5). Основание:
#     §2 п.5 (без потребителя на проде метрика — «built-not-wired»).
# (6) Шов транспорта — ДВЕ операции (§4 п.7, `R-263` Б-1): **листинг** корня
#     `${COLD%:*}` и **выборка ОДНОГО файла**. Листинг: `${JOURNAL_DRILL_SSH}
#     <user@host> ls <путь>`. Выборка: `rsync --archive -e "${JOURNAL_DRILL_SSH}"
#     <user@host>:<путь>/<имя> <RESTORE>/`. Ветки `local`/`remote` различаются
#     ТОЛЬКО этими двумя операциями. `JOURNAL_DRILL_SSH` (дефолт `ssh -i
#     /root/.ssh/storagebox -o IdentitiesOnly=yes -o BatchMode=yes -o
#     ConnectTimeout=10 -p 23`) — единственная точка ssh-команды. Отказ листинга
#     или любой выборки ⇒ `ok=0`, причина `transport-failed`, код 5.
# (7) `flock -n` держит ТОЛЬКО обёртка (§4 п.8, `R-263` Б-2): строка cron'а зовёт
#     обёртку без `flock`. Занятый замок ⇒ тик пропущен: (а) `exit 0`,
#     (б) в логе — новая строка `SKIP` без `ALERT`, (в) состояние побайтно прежнее,
#     (г) каталог восстановления побайтно прежний, (д) файл тревоги не тронут,
#     (е) за ограниченное время (`-n` обязателен, иначе обёртка висит).
# (9) §4 стр.12 (а)–(д) `checked` СЛЕДУЕТ ЗА ДОСТАВКОЙ (`R-268` Б-1 / `TD-260`,
#     `C-318` B-1, `C-319` NOTE):
#     (а) членство «имя в листинге» — БЕЗ конвейера под pipefail
#         (`grep -qx -- "$f" <<<"${LISTING}"`), а не `printf` в пайпе к `grep`. На
#         больших листингах (108 КБ и далее) `pipefail` даёт «успех без байт» —
#         одни индексы теряются, и `ps_judge` не находит их в `RESTORE`
#         (`R-268` Б-1);
#     (б) индекс засчитывается (`CHECKED++`) ТОЛЬКО когда ВСЕ его формы из листинга
#         лежат файлом в `${RESTORE}` после выборки; частичная доставка ⇒ тот же
#         исход `transport-failed`/5 (`A-055` §5.2);
#     (в) выборка вернула 0, а файла в `${RESTORE}` нет (`rsync`/`cp` солгали) ⇒
#         `alert`, `write_state 0 0 "" "transport-failed" "${CHECKED}"`, `exit 5`;
#     (г) `stderr` обёртки НЕ ГЛУШИТСЯ НАВСЕГДА. Прежнее `exec 9>"${LOCK_FILE}"
#         2>/dev/null` перенаправляло `stderr` ВСЕЙ обёртки (текущая оболочка
#         получает `fd 2 → /dev/null` как побочный эффект `exec` без команды),
#         и первая `ALERT` уходила в никуда (`DV-E` — `TD-261` п.3, `R-268` Н-1).
#         Скобочная группа `{ exec 9>…; } 2>/dev/null || {…}` ограничивает
#         перенаправление `fd 2` командой ОТКРЫТИЯ fd 9, после группы `stderr`
#         возвращается в нормальный режим и `ALERT` виден в `stderr` обёртки;
#     (д) сразу ПОСЛЕ очистки, ДО листинга и выборки: каталог `${RESTORE}` существует
#         и пуст. Проверяется НАБЛЮДЕНИЕМ каталога
#         (`[ -d "${RESTORE}" ] && [ -z "$(ls -A "${RESTORE}")" ]`), а не кодом
#         возврата `rm` (`C-318` B-1: прежняя ветка верила `rm -rf` и засчитывала
#         байты ПРОШЛОГО прогона при `rm → 0 без эффекта`). Иначе `alert`,
#         `write_state 0 0 "" "transport-failed" 0`, `exit 5`, ЧИТАТЕЛЬ НЕ ВЫЗВАН;
#     попутно — отпечаток читателя валидируется без конвейера
#     (`grep -qE '^[0-9a-f]{64}$' <<<"${DG}"`, а не `printf` в пайпе к `grep`).
#     Все пять судит семейство `SR`/`SR-M` пробы (`C-318`/`C-319`, §8 мутации).
# (8) Файл состояния — `/var/lib/hft/drill-state/journal-restore-drill.json`
#     (§4 п.10, `R-263` Б-4): КАТАЛОГ отдельный от `/var/lib/hft` (том монтируется
#     в контейнер recorder'а по тому же пути `:ro`). Обёртка пишет `tmp → rename`;
#     bind-mount ФАЙЛА держал бы прежний inode и не увидел бы ни одной новой записи.
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
  STATE="${JOURNAL_DRILL_STATE:-/var/lib/hft/drill-state/journal-restore-drill.json}"
  TRANSPORT="${JOURNAL_DRILL_TRANSPORT:-remote}"
  # Образ читателя — `image:` сервиса recorder в `docker-compose.yml` (M-99 §4 п.3/п.9).
  # Обёртка зовёт дефолт `docker run --rm --network none -v <RESTORE>:<RESTORE>:ro
  # --entrypoint /usr/local/bin/journal-drill-read <image>`. На проде — `hft-platform-recorder:local`.
  # (Тот же бинарь читателя в ОБРАЗЕ recorder'а, что и пишет журнал: прод-код — он.)
  READER_IMAGE="${JOURNAL_DRILL_READER_IMAGE:-hft-platform-recorder:local}"
  READER="${JOURNAL_DRILL_READER:-docker run --rm --network none -v ${RESTORE}:${RESTORE}:ro --entrypoint /usr/local/bin/journal-drill-read ${READER_IMAGE}}"
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
# §4 п.10 (`R-263` Б-4): каталог `/var/lib/hft/drill-state/` ОТДЕЛЬНЫЙ от `/var/lib/hft`,
# монтируется в recorder по тому же пути `:ro` (см. `docker-compose.yml`); обёртка пишет
# `tmp → rename`, bind-mount ФАЙЛА держал бы прежний inode и не увидел бы новой записи.
# Дефолт совпадает с `ops::restore_drill::DEFAULT_RESTORE_DRILL_STATE` — один литерал пути
# на продюсера/потребителя; `verify_M-99.sh` (task #2/§4 п.10) сверяет по регулярке.
STATE="${JOURNAL_DRILL_STATE:-/var/lib/hft/drill-state/journal-restore-drill.json}"
TRANSPORT="${JOURNAL_DRILL_TRANSPORT:-remote}"
# §4 п.3/п.9: образ — `image:` сервиса recorder в `docker-compose.yml`. На проде сегодня
# `hft-platform-recorder:local`; оператор может подменить через `JOURNAL_DRILL_READER_IMAGE`.
READER_IMAGE="${JOURNAL_DRILL_READER_IMAGE:-hft-platform-recorder:local}"
# `--network none`: читатель не ходит в сеть (`M-99` §4 п.3, `A-055` §5.5).
# `-v <RESTORE>:<RESTORE>:ro`: bind-mount по тому же пути, что и `--dir`; `:ro` —
# `M-99` §4 п.3.
READER="${JOURNAL_DRILL_READER:-docker run --rm --network none -v ${RESTORE}:${RESTORE}:ro --entrypoint /usr/local/bin/journal-drill-read ${READER_IMAGE}}"
# §4 п.7 (`R-263` Б-1): ШОВ ТРАНСПОРТА — ДВЕ операции. `JOURNAL_DRILL_SSH` — единственная
# точка ssh-команды (заменяет `JOURNAL_DRILL_REMOTE` круга 1; см. §7 «Запрещено»: cron-строка
# НЕ задаёт `JOURNAL_DRILL_SSH`/`JOURNAL_DRILL_REMOTE` — дефолт живёт здесь).
# Проба подменяет `JOURNAL_DRILL_SSH` на стаб-заглушку (`scripts/tests/red_restore_drill.sh`,
# `make_ssh_stub`); она пропускает опции, отбрасывает хост и исполняет удалённую команду
# локально, имитируя поведение коробки. ПУСТОЙ `REMOTE` явно НЕ трактуется как «локально»
# (`M-74` §«JOURNAL_DRILL_TRANSPORT — отдельная переменная»): переменные разведены намеренно.
SSH="${JOURNAL_DRILL_SSH:-ssh -i /root/.ssh/storagebox -o IdentitiesOnly=yes -o BatchMode=yes -o ConnectTimeout=10 -p 23}"
# Логи и маркеры — пути из runbook'а deploy/README.md §0/§5. Дефолты — прод; проба
# (`scripts/tests/red_restore_drill.sh`) переопределяет через env в локальную песочницу.
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
# §4 п.8 (`R-263` Б-2, `A-055` §4): замок держит ТОЛЬКО обёртка; строка cron'а зовёт
# её БЕЗ `flock`. Занятый замок ⇒ тик пропущен БЕЗ СЛЕДА по всем признакам:
# (а) `exit 0`, (б) в логе новая строка `SKIP` без `ALERT`,
# (в) состояние побайтно прежнее, (г) каталог восстановления побайтно прежний,
# (д) за ограниченное время, (е) файл тревоги не тронут.
# §4 стр.12 (г): `exec` БЕЗ команды оставляет действующие перенаправления в текущей
# оболочке; прежнее `exec 9>"${LOCK_FILE}" 2>/dev/null` направляло `stderr` обёртки
# в `/dev/null` навсегда — `alert … >&2` ничего не печатал, и `DV-E`
# (`TD-261` п.3, `R-268` Н-1) был постоянно красным. Скобочная группа
# `{ exec 9>…; } 2>/dev/null` ограничивает перенаправление `fd 2` командой открытия
# `fd 9`; после неё `stderr` возвращается в нормальный режим, `ALERT … >&2` снова
# виден. `fd 9` (сам замок) живёт в оболочке дальше — это требование `flock`.
{ exec 9>"${LOCK_FILE}"; } 2>/dev/null || { alert "не могу открыть lock-файл ${LOCK_FILE}"; exit 1; }
if ! flock -n 9; then
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) SKIP предыдущий прогон ещё работает" >> "${LOG}" 2>/dev/null || true
  exit 0
fi

# ── Setup-валидация — сбой ДО доставки лучше, чем сбой ПОСЛЕ частичного восстановления ─
if [ -z "${COLD}" ] || [ -z "${RESTORE}" ] || [ -z "${STATE}" ] || [ -z "${READER}" ] || [ -z "${SSH}" ]; then
  alert "не задан один из обязательных параметров (COLD/RESTORE/STATE/READER/SSH)"
  exit 1
fi

# ── Доставка ─────────────────────────────────────────────────────────────────────────
# Каталог восстановления ОЧИЩАЕТСЯ в начале каждого прогона. На проде источник — Storage
# Box; удалять С НЕГО НЕЛЬЗЯ (M-99 §7 Запрещено). Очистка RESTORE — это очистка НАШЕГО
# каталога восстановления. Держится до следующего прогона — review/probe могут
# перечитать восстановленное (M-99 §4 «Каталог восстановления живёт до следующего
# прогона»).
rm -rf "${RESTORE}" && mkdir -p "${RESTORE}"

# §4 стр.12 (д), `C-318` B-1, `C-319` NOTE: после очистки, ДО листинга и выборки,
# КАТАЛОГ `${RESTORE}` существует и пуст. Проверка — НАБЛЮДЕНИЕМ каталога
# (`[ -d "${RESTORE}" ] && [ -z "$(ls -A "${RESTORE}")" ]`), а не кодом возврата `rm`:
# при `rm → 0 без эффекта` (`SR-2`) или при ранней ошибке (`SR-1`) прежняя ветка
# засчитывала байты ПРОШЛОГО прогона и объявляла `ok=1`. Тот же принцип, что и
# стр.12 (в): внешней операции, сообщившей успех, не верим. Иначе `reader` НЕ
# вызывается (позитивный страж заглушки читателя на это рассчитывает).
if [ ! -d "${RESTORE}" ] || [ -n "$(ls -A "${RESTORE}" 2>/dev/null)" ]; then
  alert "transport-failed (каталог ${RESTORE} не существует или не пуст после очистки)"
  write_state 0 0 "" "transport-failed" 0
  exit 5
fi

# Шов транспорта: две операции — листинг и выборка ОДНОГО файла (`M-99` §4 п.7, `R-263`
# Б-1, `C-311` Б-1, `A-055` §3/§5.1/§5.2). `local`/`remote` различаются ТОЛЬКО этими
# двумя функциями; всё после доставки — один код.
HOSTPART="${COLD%%:*}"; RPATH="${COLD#*:}"
if [ "${TRANSPORT}" = "remote" ]; then
  # `remote`: коробка видит ровно `ls <путь>` и `rsync --server --sender … <один файл>`;
  # никаких записывающих команд (запрещены удаляющие флаги rsync и маски на стороне
  # коробки), никакого выкачивания копии целиком. Проба подменяет
  # `JOURNAL_DRILL_SSH` на стаб
  # (`make_ssh_stub`), который ЗАПИСЫВАЕТ argv (для судьи точки) и ИСПОЛНЯЕТ удалённую
  # команду в каталоге песочницы — заглушка сети, не логики.
  list_cold()  { ${SSH} "${HOSTPART}" ls "${RPATH}"; }
  fetch_one()  { rsync --archive -e "${SSH}" "${HOSTPART}:${RPATH}/$1" "${RESTORE}/"; }
else
  list_cold()  { ls "${COLD}"; }
  fetch_one()  { cp -p -- "${COLD}/$1" "${RESTORE}/"; }
fi

# Листинг — ДО выборки; отказ листинга ⇒ `transport-failed`, код 5 (`A-055` §5.1:
# НЕ причина «cold-empty», НЕ код 7 — это тот класс «диагноз уводит не туда»).
LISTING="$(list_cold)" || { alert "transport-failed (листинг ${COLD} отказал)"; write_state 0 0 "" "transport-failed" 0; exit 5; }

# ── Отбор по ИНДЕКСАМ (детерминированный) — `M-74` §«Правило выборки» ─────────────────
# Из листинга источника (COLD) извлекаются ИНДЕКСЫ (`segment-NNNNNNNN.jrnl` И
# `segment-NNNNNNNN.jrnl.zst` дают ОДИН и тот же индекс), список сортируется и
# дедуплицируется; берутся первый, `floor(N/2)`-й и последний. Для каждого выбранного
# индекса восстанавливаются ОБЕ формы, если обе есть. Никакого `$RANDOM`: drill
# воспроизводим по своему логу.
ALL="$(printf '%s\n' "${LISTING}" | sed -n 's/^segment-\([0-9]\{8\}\)\.jrnl\(\.zst\)\?$/\1/p' | sort -u)"
N="$(printf '%s\n' "${ALL}" | grep -c . || true)"
if [ "${N:-0}" -lt 3 ]; then
  alert "cold-empty (в копии <3 индексов — drill не имеет выборки)"
  write_state 0 0 "" "cold-empty" 0
  exit 5
fi
MID=$(( (N + 1) / 2 ))
IDX="$(printf '%s\n' "${ALL}" | sed -n "1p;${MID}p;${N}p" | sort -u)"

# Выборка ОДНОГО файла за вызов. Каждый выбранный индекс — обе формы, если обе есть.
# Отказ ЛЮБОЙ выборки ИЛИ «успех без байта» (`rsync`/`cp` солгали — `DV`) ⇒
# `transport-failed`, код 5 (`A-055` §5.2: неполная доставка НЕ есть `ok=1`, причина
# читателя НЕ ставится — доставка не дошла до чтения). §4 стр.12 (а)/(б)/(в):
#  - (а) членство «имя в листинге» — БЕЗ конвейера под pipefail
#    (`grep -qx -- "$f" <<<"${LISTING}"`); на больших листингах за буфером канала
#    конвейерное `printf` и `grep -qx` теряет байты, и индексы «выпадают»
#    (`R-268` Б-1, `TD-260`);
#  - (б) `CHECKED++` ТОЛЬКО когда ВСЕ формы этого индекса из листинга лежат файлом
#    в `${RESTORE}` после выборки; одна из двух форм недоставлена — индекс НЕ
#    засчитан, обёртка выходит с тем же `transport-failed`/5;
#  - (в) выборка (`rsync`/`cp`) вернула 0, а файла в `${RESTORE}` нет — `alert`,
#    `write_state … "transport-failed" "${CHECKED}"`, `exit 5`.
CHECKED=0
for i in ${IDX}; do
  for f in "segment-${i}.jrnl" "segment-${i}.jrnl.zst"; do
    if grep -qx -- "$f" <<<"${LISTING}"; then
      fetch_one "$f" || { alert "transport-failed (выборка ${f} отказала)"; write_state 0 0 "" "transport-failed" "${CHECKED}"; exit 5; }
      if [ ! -f "${RESTORE}/${f}" ]; then
        alert "transport-failed (выборка ${f} вернула успех, файла в ${RESTORE} нет)"
        write_state 0 0 "" "transport-failed" "${CHECKED}"
        exit 5
      fi
    fi
  done
  # (б) Все перечисленные в листинге формы этого индекса доехали — засчитываем.
  CHECKED=$((CHECKED + 1))
done

# Sidecar'ы (`M-99` §4 п.2) — побайтная сверка. Каждый `*.json` корня копии доставляется
# ТОЙ ЖЕ выборкой ОДНОГО файла; проба (`A12`/`A13`) подменяет/удаляет sidecar между
# шагами пробы, и эта проверка ловит расхождение. Имена — через `grep -E` по
# листингу; чтобы не тащить ещё один конвейер под pipefail, читаем `LISTING`
# один раз в массив имён.
while IFS= read -r _sc_name; do
  [ -n "${_sc_name}" ] || continue
  fetch_one "${_sc_name}" || { alert "transport-failed (выборка sidecar ${_sc_name} отказала)"; write_state 0 0 "" "transport-failed" "${CHECKED}"; exit 5; }
done <<<"$(grep -E '^[^/]+\.json$' <<<"${LISTING}")"

if [ "${CHECKED}" -eq 0 ]; then
  alert "cold-empty (выборка пуста после отбора)"
  write_state 0 0 "" "cold-empty" 0
  exit 5
fi

# ── ИСТОЧНИК ОСТАВЛЯЕТСЯ НЕТРОНУТЫМ (`M-99` §4 п.1 + §7 Запрещено) ──
# Обёртка НЕ уничтожает ни источник, ни доставленное. На проде источник — Storage Box,
# удалять С НЕГО НЕЛЬЗЯ; доставленное в `${RESTORE}` живёт до следующего прогона
# (§4 «Каталог восстановления живёт до следующего прогона»). На local-транспорте
# (проба) источник — `${COLD}` фикстуры песочницы: проба сама делает `retire_source`
# (`scripts/tests/red_restore_drill.sh::retire_and_read_watched`) ДО проверки
# `inv_delivery` — её забота, не наша. Читатель ниже зовётся на `${RESTORE}` с
# доставленными байтами; источник к этому моменту может быть уничтожен ПРОБОЙ, и
# `inv_delivery` судит, что `d_after = d_live` (тот же отпечаток, что был с источником)
# — доставка держится.
#
# (Сравни с предыдущей редакцией круга 1: `mv ${RESTORE} ${RESTORE}.source.$$`
# уничтожал доставленное ПЕРЕД чтением — `restore` оказывался пустым, читатель
# возвращал `rc=5 (empty)`, а `RT` оставался зелёным с причиной «копия не привезла
# событий». Это та ловушка `R-263` Б-1, на которую `RT` теперь КРАСЕН — мутант
# «прежнее переименование доставленного» падает на `RT` со ссылкой `reader-rc-5`.)

# ── ЧТЕНИЕ: исход берётся У ЧИТАТЕЛЯ, а не сочиняется (`M-74` §«Контракт обёртки») ──
# `READER` зовётся ИМЕННО так, как задан в env; никаких подмен. `--dir` РОВНО
# `${RESTORE}` — после очистки/доставки (`rm -rf` + `cp`/`rsync` выше) каталог существует
# и содержит ДОСТАВЛЕННЫЕ БАЙТЫ выборки (НЕ пуст, как было до M-99 — пустота читается
# как `reader-rc-5 (empty)` и снимается §3 правилом выборки, а не подменой источника).
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
  write_state 0 "${EV:-0}" "" "${reason}" "${CHECKED}"
  exit "${RC}"
fi

# Проверка непрерывности ВНУТРИ сегментов (`M-74` §«Правило выборки» — СЕМАНТИКА
# НЕПРЕРЫВНОСТИ). МЕЖСЕГМЕНТНАЯ непрерывность на выборке невозможна по построению
# (три несмежных индекса). `intra_segment_continuous=false` — читатель ДОКАЗУЕМО нашёл
# дыру; это его собственный диагноз, обёртка его берёт (`M-74` обязанность 3 — взять класс
# отказа ИЗ ВЫВОДА и КОДА читателя, а не сочинить). Отсутствие поля ИЛИ любое иное
# значение при `rc=0` НЕ есть `false` ⇒ НЕ intra-segment-gap: дальше судим по полноте
# протокола (ворота §4 стр.11 ниже).
if [ "${INTRA}" = "false" ]; then
  alert "intra-segment-gap (читатель доказуемо нашёл дыру ВНУТРИ сегмента)"
  write_state 0 "${EV:-0}" "" "intra-segment-gap" "${CHECKED}"
  exit 4
fi

# ══ ВОРОТА ПРОТОКОЛА УСПЕХА (`M-99` §4 стр.11, `R-266` Б-1, `A-056` §4.1) ══
# `ok=1` пишется ТОЛЬКО при `rc=0` И полном протоколе читателя: `digest` ровно 64
# СТРОЧНЫХ hex, `events_read` ≥ 1 (число, поле есть), `intra_segment_continuous=true`
# (поле есть). Любое иное при `rc=0` — ПРОТИВОРЕЧИЕ ЧИТАТЕЛЯ (код говорит «успех»,
# вывод его не подтверждает): обёртка не вправе сочинить ни успех (`ok=1`), ни чужой
# класс (`reader-rc-4`, `intra-segment-gap` — читатель этого не говорил). Пишется
# `ok=0`, `digest=""`, причина `reader-protocol-violation`, код 7, состояние ЗАПИСАНО.
# Отпечаток из неверного вывода НЕ пробрасывается. Код 7 не занят ни читателем
# (`0`/`1`/`4`/`5`/`6`), ни обёрткой (`1`/`4`/`5`). Запрет `M-99` §7 «писать `ok=1` без
# `digest`» имел оракул только на стороне ПОТРЕБИТЕЛЯ; эти строка — оракул на стороне
# ПИСАТЕЛЯ (R-266 Б-1 закрывается здесь).
DG_OK=0
if [ -n "${DG}" ] && grep -qE '^[0-9a-f]{64}$' <<<"${DG}"; then DG_OK=1; fi
EV_OK=0
if [ -n "${EV}" ] && [ "${EV}" -ge 1 ] 2>/dev/null; then EV_OK=1; fi
IA_OK=0
if [ "${INTRA}" = "true" ]; then IA_OK=1; fi
if [ "${DG_OK}" -ne 1 ] || [ "${EV_OK}" -ne 1 ] || [ "${IA_OK}" -ne 1 ]; then
  alert "reader-protocol-violation (протокол успеха неполон: digest='${DG:-}', events_read='${EV:-}', intra_segment_continuous='${INTRA:-}')"
  write_state 0 "${EV:-0}" "" "reader-protocol-violation" "${CHECKED}"
  exit 7
fi

write_state 1 "${EV:-0}" "${DG}" "" "${CHECKED}"
exit 0