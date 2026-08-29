#!/usr/bin/env bash
# Операторская cron-обёртка для ИНКРЕМЕНТАЛЬНОЙ ОФСАЙТ-КОПИИ журнала на Hetzner
# Storage Box через SSH-субаккаунт (П-023, 29.08.2026 — первая офсайт-копия сделана
# РУКАМИ, 465 файлов / 77 ГБ / RSYNC_EXIT=0). Этот скрипт превращает её в РАСПИСАНИЕ:
# раз в час, ТОЛЬКО файлы, не изменявшиеся ≥15 минут (активный сегмент пишется прямо
# сейчас — копировать его «выглядит как целый, но обрывок»).
#
# ПОЧЕМУ ОТДЕЛЬНЫЙ СКРИПТ, А НЕ КОМАНДА В CRONTAB.
# Та же причина, что у journal-retention-cron.sh / journal-compaction-cron.sh:
#  - cron не понимает `\` для продолжения строки (каждая физическая строка = запись,
#    `\`-перенос → «bad minute», файл НЕ устанавливается). Гейт cron-парсера не пропустит.
#  - текст скрипта верифицируется `bash -n` и прогоном со стабом; argv можно проверить
#    ДО побочных эффектов (`HFT_CRON_PRINT_ARGV=1`).
#  - один источник правды по argv — прод и гейт берут его ОТСЮДА, не из cron-строки.
#
# ЧАСТОТА: раз в час. ОБОСНОВАНИЕ ЧИСЛОМ.
#  - Прирост журнала замерено: ~5 ГБ/сут чистыми (≈ 4–5 закрытых сегментов по ~1.07 ГБ
#    в день, см. `ls /var/lib/docker/volumes/hft-platform_journal-data/_data/segment-*.jrnl`
#    за неделю наблюдения). Это ~210 МБ/ч в среднем.
#  - Канал замерен 66 МБ/с; ставим `--bwlimit=40M` (≈ 60 % канала), оставляя запас на
#    параллельные задачи прод-хоста (recorder пишет ~5 МБ/с в пике, копия должна
#    голодать РАНЬШЕ recorder'а, и 40 МБ/с против ~66 МБ/с даёт recorder'у ~26 МБ/с
#    гарантированного headroom'а). Один закрытый сегмент ≈ 1100 МБ ⇒ ≈ 27 с на копию.
#  - При почасовом расписании ТИПОВОЙ прогон копирует 0 файлов (между прогонами ничего
#    нового не закрылось) — rsync завершается за миллисекунды, активный сегмент НЕ
#    трогается (его mtime обновляется — `find -mmin +15` его НЕ видит). Когда за час
#    закрывается 1–2 сегмента, прогон копирует ИХ и завершается за минуту.
#  - Аварийный сценарий: если cron молча не запускался сутки, прогон догонит за 1–2
#    итерации. Это компромисс — часовая граница держит окно потерь ≤ 60 минут при
#    условии, что копия за этот час успела дойти (1100 МБ / 40 МБ/с ≈ 27 с + setup).
#    Требование «потеря ≤ 15 минут» требовало бы 15-минутного cron'а, но в этом окне
#    успевает закрыться от силы один сегмент; в любом случае потеря = активный сегмент
#    последней записи, и это неотвратимо при append-only модели.
#
# БЕЗОПАСНОСТЬ КОПИИ — три ограничения, НАЗВАННЫХ ЯВНО.
#  1) rsync БЕЗ `--delete` и БЕЗ любого `--delete-*` / `--remove-source-files`.
#     Storage Box не снимается снапшотом, единственная офсайт-копия создаётся ЭТОЙ
#     командой; `--delete` в составе команды создания — единственный способ уничтожить
#     бэкап той же командой, которая его создаёт. Проверка: `grep -E -- "--delete" deploy/`
#     должна вернуть 0 (явный grep-канарейка).
#  2) mtime-фильтр ≥ 15 минут + явное исключение `recorder.heartbeat`. Активный сегмент
#     (тот, в который recorder пишет прямо сейчас) ОБЯЗАН быть пропущен, иначе копия
#     зафиксирует обрывок, выглядящий как целый файл — на вид валидно, при попытке
#     replay'а — тихая потеря хвоста. Активный сегмент опознаётся по mtime: пока
#     recorder пишет, mtime обновляется каждые несколько секунд ⇒ `find -mmin +15`
#     его не видит. `recorder.heartbeat` обновляется ещё чаще и тоже отфильтровывается
#     по mtime; явное исключение — страховка от регрессии в recorder'е (если mtime
#     перестанет обновляться — heartbeat НЕ уедет на Storage Box как «архив»).
#  3) flock на уровне всего скрипта. Два наложившихся rsync по одному каталогу дают
#     неопределённый результат (SFTP-сессия хранит состояние, общее между прогонами);
#     cron без блокировки НЕ гарантирует непересечение. `flock -n` на lock-файле
#     /var/lock/hft-journal-offsite.lock — non-blocking: пропустить тик безопаснее, чем
#     копить очередь процессов на коробке.
#
# КОНТРАКТ (testing.md, «Механизм несущего пути»): обёртка поддерживает
# `HFT_CRON_PRINT_ARGV=1` — печатает блок `KEY=VALUE` для каждого настраиваемого
# параметра и финальный rsync-вызов ДО side-эффектов (flock/cd/find/rsync).
# Это позволяет гейту проверять ИСПОЛНЕНИЕ обёртки, а не только её наличие.
# Композиция: SRC_DIR и DST_URL обязаны соответствовать путям, по которым читает
# будущий consumer (restore-drill П-023). SRC_DIR совпадает с прод-каталогом,
# который монтируется в recorder как journal-data:/var/lib/journal (одинаково в
# проде и в будущем restore-drill'е).
set -uo pipefail

# ── Конфигурация через env (как у соседей — паттерн, не изобретение) ────────────────
HFT_ROOT="${HFT_ROOT:-/root/hft-platform}"
# Источник — прод-каталог НА ТОМ ЖЕ ХОСТЕ, что cron (rsync локальный по SSH не
# запускает, файлы читаются с диска, ssh нужен ТОЛЬКО для отправки на Storage Box).
SRC_DIR="${JOURNAL_OFFSITE_SRC:-/var/lib/docker/volumes/hft-platform_journal-data/_data}"
# Цель — SSH-субаккаунт на Storage Box, порт 23 (SSH), путь journal/ в его домашнем
# каталоге (соответствует каталогу, в который положилась первая ручная офсайт-копия
# 29.08: `journal/` — 465 файлов, 77 ГБ, RSYNC_EXIT=0). Форма `user@host:path` (не
# `ssh://user@host:port/path`): rsync при наличии `-e "ssh ..."` использует ровно ту
# команду ssh, что мы задаём, и `ssh://`-URL в паре с `-e` даёт «ssh ssh://...»
# (rsync 3.4.1, замер: `ssh: Could not resolve hostname ssh: Temporary failure`).
DST_URL="${JOURNAL_OFFSITE_DST:-u659392-sub1@u659392-sub1.your-storagebox.de:journal/}"
# Путь к ключу Storage Box (на проде — /root/.ssh/storagebox, права 600, проверено 29.08).
SSH_KEY="${JOURNAL_OFFSITE_SSH_KEY:-/root/.ssh/storagebox}"
# mtime-фильтр: ≥ 15 минут. Свежие файлы — потенциально активный сегмент (его mtime
# обновляется во время записи), плюс recorder.heartbeat (обновляется каждые несколько
# секунд). 15 минут — запас к «времени закрытия сегмента» (≈ минуты) с двухкратным
# запасом; на 5 ГБ/сут это не окно потерь, а просто фильтр «не писать прямо сейчас».
MIN_AGE_MIN="${JOURNAL_OFFSITE_MIN_AGE_MIN:-15}"
# Полоса для rsync: 40 МБ/с из замерных 66 МБ/с; headroom для recorder и прод-хоста.
# Проверено вручную на проде 29.08: --bwlimit=40M держит копию 1100 МБ за ≈ 27 с,
# recorder при этом пишет без видимой деградации.
BWLIMIT_MBPS="${JOURNAL_OFFSITE_BWLIMIT_MBPS:-40}"
# nice/ionice — recorder 24/7, ему I/O важнее, чем копии (recorder пишет
# последовательно и должен уложиться в fsync-окно перед ротацией).
NICE_LEVEL="${JOURNAL_OFFSITE_NICE:-10}"
IONICE_CLASS="${JOURNAL_OFFSITE_IONICE_CLASS:-2}"   # best-effort
IONICE_LEVEL="${JOURNAL_OFFSITE_IONICE_LEVEL:-7}"   # lowest within best-effort
# Логи и маркеры — пути из runbook'а deploy/README.md §5.
LOG="${JOURNAL_OFFSITE_LOG:-/var/log/hft/journal-offsite.log}"
ALERT_FILE="${JOURNAL_OFFSITE_ALERT_FILE:-/var/lib/hft/journal-offsite.alert}"
LAST_SUCCESS="${JOURNAL_OFFSITE_LAST_SUCCESS:-/var/lib/hft/journal-offsite.last-success}"
# Lock — отдельный файл, чтобы cron-строки retention/compaction/offsite НЕ конкурировали
# (offsite читает ТЕ ЖЕ файлы, что retention читает и compaction сжимает; rsync может
# читать сжатый сегмент одновременно с compaction — flock обеспечивает сериализацию
# на уровне всего скрипта).
LOCK_FILE="${JOURNAL_OFFSITE_LOCK:-/var/lock/hft-journal-offsite.lock}"

mkdir -p "$(dirname "${LOG}")" "$(dirname "${ALERT_FILE}")" "$(dirname "${LAST_SUCCESS}")" 2>/dev/null || true

alert() { # exit≠0 ОБЯЗАН быть виден — молчащий offsite = потеря офсайт-копии без причины
  local msg="$1"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) ALERT ${msg}" >> "${LOG}" 2>/dev/null || true
  command -v logger >/dev/null 2>&1 && logger -p user.err -t hft-journal-offsite "ALERT: ${msg}" || true
  { date -u +%Y-%m-%dT%H:%M:%SZ; echo "${msg}"; } > "${ALERT_FILE}" 2>/dev/null || true
  echo "ALERT ${msg}" >&2
}

# ── Argv — ЕДИНСТВЕННОЕ место, где он определён (прод + гейт берут отсюда) ──────────
# Три отдельных секции, разделены маркерами:
#   - "VARS:" — настраиваемые параметры (KEY=VALUE, по одной на строку; гейт сравнивает
#     с прод-дефолтами);
#   - "FIND:" — фильтр файлов по mtime + исключения;
#   - "RSYNC:" — финальная команда (одна строка; гейт проверяет наличие --partial,
#     отсутствие --delete и совпадение SRC_DIR/DST_URL).
print_argv() {
  echo "VARS:"
  echo "SRC_DIR=${SRC_DIR}"
  echo "DST_URL=${DST_URL}"
  echo "SSH_KEY=${SSH_KEY}"
  echo "MIN_AGE_MIN=${MIN_AGE_MIN}"
  echo "BWLIMIT_MBPS=${BWLIMIT_MBPS}"
  echo "NICE_LEVEL=${NICE_LEVEL}"
  echo "IONICE_CLASS=${IONICE_CLASS}"
  echo "IONICE_LEVEL=${IONICE_LEVEL}"
  echo "FIND:"
  echo "find \"\${SRC_DIR}\" -type f -mmin +\${MIN_AGE_MIN} ! -name 'recorder.heartbeat' -print0"
  echo "RSYNC:"
  echo "nice -n \${NICE_LEVEL} ionice -c \${IONICE_CLASS} -n \${IONICE_LEVEL} rsync \\"
  echo "  --archive --partial --human-readable --stats \\"
  echo "  --bwlimit=\${BWLIMIT_MBPS}M \\"
  echo "  -e \"ssh -i \${SSH_KEY} -o IdentitiesOnly=yes -p 23 -o StrictHostKeyChecking=accept-new\" \\"
  echo "  --from0 --files-from=- \\"
  echo "  \"\${SRC_DIR}/\" \"\${DST_URL}\""
}

# Поддерживаем обе формы env-var (как у соседей): историческую
# `RETENTION_PRINT_ARGV=1` (от journal-retention-cron.sh) и новую
# `HFT_CRON_PRINT_ARGV=1` (M-48, единый контракт cron-обёрток). На этой
# задаче формальной причины для старой нет, но общий канон — `HFT_CRON_*`.
if [ "${HFT_CRON_PRINT_ARGV:-0}" = "1" ] || [ "${RETENTION_PRINT_ARGV:-0}" = "1" ]; then
  print_argv
  exit 0
fi

# ── flock — non-blocking: пропустить тик безопаснее, чем копить очередь на коробке ──
# Берём flock ОДИН РАЗ на весь скрипт (внутри — find/rsync). Если предыдущий тик ещё
# работает, ВЫХОД с кодом 0 И без alert'а: cron ВЕРНЁТСЯ через час и попробует снова.
# Это не сбой — это защита от наложения. exit=0 при пропуске тика намеренно, иначе
# cron-обёртка сама плодила бы ложные алерты каждый час.
exec 9>"${LOCK_FILE}" || { alert "не могу открыть lock-файл ${LOCK_FILE}"; exit 1; }
if ! flock -n 9; then
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) SKIP предыдущий прогон ещё работает" >> "${LOG}" 2>/dev/null || true
  exit 0
fi

# ── Setup-валидация — сбой ДО rsync лучше, чем сбой ПОСЛЕ частичной копии ───────────
# 1) источник существует и доступен (recorder поднят, том смонтирован);
if [ ! -d "${SRC_DIR}" ]; then
  alert "SRC_DIR=${SRC_DIR} не существует или не директория — recorder умер или том не смотирован"
  exit 1
fi
# 2) ключ SSH на месте и доступен (chmod 600 иначе ssh откажет — fail-loud);
if [ ! -f "${SSH_KEY}" ]; then
  alert "SSH_KEY=${SSH_KEY} не существует"
  exit 1
fi
# 3) dry-run ssh — если субаккаунт недоступен, лучше узнать сейчас, чем в середине копии.
# `BatchMode=yes` гарантирует, что ssh не спросит пароль интерактивно (если спросит —
# значит ключ не подходит, и rsync тоже упадёт). `ConnectTimeout=10` не висит вечно
# при сетевых проблемах. Используем `pwd` (в restricted shell'е Storage Box'а НЕТ
# `/usr/bin/true`; `pwd` — единственный «безобидный» builtin, доступный ВСЕГДА; см.
# `help` при подключении к субаккаунту).
if ! ssh -i "${SSH_KEY}" -o IdentitiesOnly=yes -o BatchMode=yes -o ConnectTimeout=10 \
     -p 23 -o StrictHostKeyChecking=accept-new \
     u659392-sub1@u659392-sub1.your-storagebox.de pwd 2>/dev/null; then
  alert "ssh к субаккаунту storagebox отказал (ключ ${SSH_KEY} или сеть/Storage Box)"
  exit 1
fi

# ── Основная работа: find (фильтр) → rsync (копия) ─────────────────────────────────
# Замечание по команде:
#   - `find -mmin +${MIN_AGE_MIN}` — mtime-фильтр по самой последней записи в файл;
#   - `! -name 'recorder.heartbeat'` — явное исключение heartbeat'а (страховка от
#      регрессии в recorder'е, см. шапку);
#   - `find -print0` + `rsync --from0` — корректная обработка имён с пробелами/спецсимволами
#      (на проде их нет, но контракт на null-terminated — стандарт rsync, и его дешевле
#      соблюсти, чем оговаривать «у нас имён с пробелами не бывает»);
#   - `--archive` — rlptgoD (recursive, links, perms, times, group, owner, devices),
#      то есть rsync сохранит mtime/права — критично для crc-сверки при restore-drill;
#   - `--partial` — НЕ удалять частично переданный файл (если связь оборвалась, докачка
#      на следующем тике; иначе rsync удаляет частичный файл и начинает заново);
#   - `--bwlimit=${BWLIMIT_MBPS}M` — полоса; см. шапку «ЧАСТОТА»;
#   - `--stats` — пишет в конце «Total transferred file size» — единственный способ
#      убедиться глазами, что прогон скопировал 0 файлов (идемпотентность), а не
#      проглотил молча ошибку;
#   - `--files-from=-` — читать список файлов из stdin (а не из argv; список длинный
#      и argv может переполнить лимит ядра);
#   - `SRC_DIR/` с trailing slash — копировать СОДЕРЖИМОЕ каталога, а не сам каталог;
#      иначе на Storage Box появится подкаталог `journal/_data/`, а не `journal/...`.
#
# Поток find → rsync запускается через pipe. В bash без pipefail (мы используем
# `set -uo pipefail`, но pipefail не ловит SIGPIPE от rsync, который закрывает stdin
# после прочтения списка — это штатное завершение find'а через SIGPIPE).
# Реальный сбой — это exit≠0 от rsync. find не возвращает данные об ошибках
# отдельным каналом; все stderr уходят в LOG.
start_ts=$(date -u +%s)
if find "${SRC_DIR}" -type f -mmin +"${MIN_AGE_MIN}" ! -name 'recorder.heartbeat' -print0 \
  | nice -n "${NICE_LEVEL}" ionice -c "${IONICE_CLASS}" -n "${IONICE_LEVEL}" rsync \
      --archive --partial --human-readable --stats \
      --bwlimit="${BWLIMIT_MBPS}M" \
      -e "ssh -i ${SSH_KEY} -o IdentitiesOnly=yes -p 23 -o StrictHostKeyChecking=accept-new" \
      --from0 --files-from=- \
      "${SRC_DIR}/" "${DST_URL}" >> "${LOG}" 2>&1; then
  rc=0
else
  rc=$?
fi
end_ts=$(date -u +%s)
dur=$(( end_ts - start_ts ))

if [ "${rc}" -ne 0 ]; then
  alert "rsync exit=${rc} (1=IO/SSH; 12=connection; 23=protocol; 30=timeout в rsync). \
Проверь ${LOG} и доступность ${DST_URL}. Длительность ${dur}s."
  exit "${rc}"
fi

# Успешный прогон гасит alert и пишет heartbeat (deploy/README.md §5: *.alert ловит
# сбой, *.last-success ловит «cron молча не запускался» — два разных дефекта, оба
# обязаны быть видимы; иначе узнаём о молчании из диска, а не из мониторинга).
rm -f "${ALERT_FILE}" 2>/dev/null || true
date -u +%Y-%m-%dT%H:%M:%SZ > "${LAST_SUCCESS}" 2>/dev/null || true

# Длительность — в логе рядом с итогом rsync (по нему видно, не упёрся ли прогон
# в полосу и не висит ли где-то). Не выводим в alert — это операционная метрика,
# а не сигнал тревоги.
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) OK duration=${dur}s" >> "${LOG}" 2>/dev/null || true
exit 0
