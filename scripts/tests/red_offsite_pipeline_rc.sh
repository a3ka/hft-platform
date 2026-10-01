#!/usr/bin/env bash
# Проба СТОРОЖА СБОЯ ИСТОЧНИКА в офсайт-копии журнала — `R-157` Б-5.
#
# ЗАЧЕМ. `deploy/bin/journal-offsite-cron.sh` строит конвейер `find … | … rsync …` и судит
# ОБА его звена. Сторож сбоя ИСТОЧНИКА (find) был мёртв:
#
#     rsync_rc=${PIPESTATUS[1]:-0}     # ← это присваивание СБРАСЫВАЕТ PIPESTATUS
#     find_rc=${PIPESTATUS[0]:-0}      # ← читает уже статус ПРИСВАИВАНИЯ, всегда 0
#
# Семантика оболочки, снятая замером (bash 5.2.21):
#     false | true ; a=${PIPESTATUS[1]} ; b=${PIPESTATUS[0]}   → a=0 b=0
#     false | true ; st=("${PIPESTATUS[@]}")                   → st=(1 0)
#
# Следствие на проде: `find` падает (том отвалился, права, ENOENT), `rsync` не получает
# списка и завершается нулём — обёртка пишет «ОК», ставит отметку успешной копии, ГАСИТ
# алерт и выходит с нулём. НЕ СКОПИРОВАВ НИЧЕГО. Это ровно то состояние, которое милестоун
# объявил недопустимым: «копия устарела, и никто об этом не знает» (`OPS-I-2`).
#
# ПОЧЕМУ ЭТО ПРОШЛО ДВА КРУГА РЕВЬЮ ЗЕЛЁНЫМ. Гейт `verify_M-73.sh` из 17 шагов НЕ ИСПОЛНЯЛ
# конвейер НИ В ОДНОМ шаге — он судил тексты, argv и расписание. Замер:
#     grep -cE 'find .*\| *nice|rsync ' scripts/verify_M-73.sh   → 0
# То есть у механизма наблюдаемости не было наблюдателя. `testing.md`: «Исправление по
# вердикту тоже требует оракула» + свойство 4 целостности гейта — «наблюдает ОТСУТСТВИЕ,
# не только сбой».
#
# ЧТО ЭТА ПРОБА ДЕЛАЕТ. Исполняет обёртку ПРОД-ФОРМОЙ вызова со стаб-PATH и судит ТРОЙКУ
# исходов. Стабятся ТОЛЬКО внешние команды (`find`, `rsync`, `ssh`); `nice`, `ionice`,
# `flock`, `date` — НАСТОЯЩИЕ, то есть конвейер собирается и исполняется тот же, что на
# проде, а не его пересказ.
#
#   A  find=141, rsync=0   → exit=0,  alert НЕТ,  отметка успеха ЕСТЬ
#      (141 = SIGPIPE: `rsync` штатно закрыл stdin — это НЕ сбой, и развязка обязана уцелеть)
#   B  find=0,   rsync=12  → exit=12, alert ЕСТЬ, отметки успеха НЕТ   (сбой приёмника)
#   C  find=1,   rsync=0   → exit=1,  alert ЕСТЬ, отметки успеха НЕТ   (сбой ИСТОЧНИКА)
#
# `C` — предмет. До фикса он даёт exit=0 / alert НЕТ / отметка ЕСТЬ.
# `A` — позитивный контроль: без него фикс «считать любой ненулевой find сбоем» прошёл бы,
# сломав штатный SIGPIPE и превратив каждую успешную копию в ложную тревогу.
# `B` — контроль соседнего сторожа: он работал и обязан продолжать работать.
#
# ЧЕГО ПРОБА НЕ ЛОВИТ, названо, а не умолчано:
#   • она судит КОДЫ ВОЗВРАТА и наличие файлов-признаков, а не то, что байты действительно
#     доехали. Поведенческий пруф — ручной прогон на проде (задача 3 круга 4), и он
#     оракулом не заменяется;
#   • `ssh` застаблен нулём, то есть pre-flight не судится этой пробой вовсе;
#   • стабы не воспроизводят частичный отказ `rsync` (часть файлов доехала) — только
#     код возврата целиком.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WRAPPER="${ROOT}/deploy/bin/journal-offsite-cron.sh"

PASSED=0; FAILED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

[ -f "${WRAPPER}" ] || die "обёртки нет: ${WRAPPER}"

# Реестр песочниц — в ФАЙЛЕ, а не в переменной: подоболочка `$( )` переменную теряет, и
# каталоги остаются навсегда. Класс, давший 10 400 каталогов в /tmp и диск на 100 %.
REG="$(mktemp /tmp/red-offsite-reg-XXXXXX)" || die mktemp
cleanup() {
  [ -n "${KEEP_FIXTURES:-}" ] && { echo "песочницы оставлены: ${REG}"; return 0; }
  while IFS= read -r d; do
    case "$d" in /tmp/red-offsite-*) [ -d "$d" ] && rm -rf "$d" ;; esac
  done < "${REG}" 2>/dev/null
  rm -f "${REG}"
}
trap cleanup EXIT

# ── ОДИН СЦЕНАРИЙ: стабы, прод-форма вызова, тройка наблюдений ────────────────────────
run_case() { # $1=имя $2=find_rc $3=rsync_rc $4=ожидаемый exit $5=alert(да/нет) $6=успех(да/нет)
  local name="$1" frc="$2" rrc="$3" want_rc="$4" want_alert="$5" want_ok="$6"
  local box st alert_seen ok_seen verdict=0

  box="$(mktemp -d /tmp/red-offsite-XXXXXX)" || die mktemp
  printf '%s\n' "${box}" >> "${REG}"
  mkdir -p "${box}/bin" "${box}/src" "${box}/state" || die "каркас песочницы"
  : > "${box}/key" || die "ключ-заглушка"
  chmod 600 "${box}/key"

  # `find` печатает НУЛЬ-РАЗДЕЛЁННЫЙ список (обёртка зовёт его с `--from0`) и возвращает
  # заданный код. Печать обязательна: пустой stdout сделал бы сценарий B недостижимым —
  # `rsync` не получил бы работы и мог бы завершиться иначе, чем задумано.
  cat > "${box}/bin/find" <<EOF
#!/usr/bin/env bash
printf 'segment-0001.bin\\0segment-0002.bin\\0'
exit ${frc}
EOF
  cat > "${box}/bin/rsync" <<EOF
#!/usr/bin/env bash
cat >/dev/null 2>&1 || true
echo "STUB rsync: exit ${rrc}"
exit ${rrc}
EOF
  # `ssh` нулём: pre-flight не предмет этой пробы, и это названо в шапке.
  printf '#!/usr/bin/env bash\nexit 0\n' > "${box}/bin/ssh"
  chmod +x "${box}/bin/find" "${box}/bin/rsync" "${box}/bin/ssh" || die "chmod стабов"

  # Страж setup'а: стаб обязан быть ДОСТИЖИМ по PATH и возвращать заданное. Без этой
  # проверки сценарий, где стаб не подхватился, зеленел бы «по чужой причине».
  # Проверяется КОД ВОЗВРАТА, а не вывод: `find` печатает нуль-разделённый список, и
  # подстановка команды его бы съела с предупреждением — шум в отчёте гейта.
  local probe_rc=0
  ( PATH="${box}/bin:${PATH}" find >/dev/null 2>&1 ) || probe_rc=$?
  [ "${probe_rc}" -eq "${frc}" ] \
    || die "стаб find не подхватился PATH (получено rc=${probe_rc}, ждали ${frc})"

  st=0
  PATH="${box}/bin:${PATH}" \
  JOURNAL_OFFSITE_SRC="${box}/src" \
  JOURNAL_OFFSITE_SSH_KEY="${box}/key" \
  JOURNAL_OFFSITE_DST="u1@storagebox.example:journal/" \
  JOURNAL_OFFSITE_LOG="${box}/state/offsite.log" \
  JOURNAL_OFFSITE_ALERT_FILE="${box}/state/offsite.alert" \
  JOURNAL_OFFSITE_LAST_SUCCESS="${box}/state/offsite.last-success" \
  JOURNAL_OFFSITE_LOCK="${box}/state/offsite.lock" \
  JOURNAL_OFFSITE_MIN_AGE_MIN=0 \
    bash "${WRAPPER}" >/dev/null 2>&1 || st=$?

  [ -s "${box}/state/offsite.alert" ] && alert_seen=да || alert_seen=нет
  [ -e "${box}/state/offsite.last-success" ] && ok_seen=да || ok_seen=нет

  [ "${st}" -eq "${want_rc}" ]        || { verdict=1; }
  [ "${alert_seen}" = "${want_alert}" ] || { verdict=1; }
  [ "${ok_seen}" = "${want_ok}" ]     || { verdict=1; }

  if [ "${verdict}" -eq 0 ]; then
    pass "${name}: find=${frc} rsync=${rrc} ⇒ exit=${st}, alert=${alert_seen}, успех=${ok_seen}"
  else
    fail "${name}: find=${frc} rsync=${rrc} ⇒ ПОЛУЧЕНО exit=${st}, alert=${alert_seen}, успех=${ok_seen}; ОЖИДАЛОСЬ exit=${want_rc}, alert=${want_alert}, успех=${want_ok}"
  fi
}

echo "── СТОРОЖ СБОЯ ИСТОЧНИКА: конвейер find|rsync исполняется, а не пересказывается"

# A — позитивный контроль. SIGPIPE от штатно закрывшего stdin rsync'а НЕ сбой; фикс,
# ломающий эту развязку, превратит каждую успешную копию в ложную тревогу.
run_case "A SIGPIPE-развязка цела" 141 0 0 нет да

# B — соседний сторож (сбой приёмника) работал и обязан продолжать работать.
run_case "B сбой приёмника виден"  0  12 12 да  нет

# C — ПРЕДМЕТ. До фикса: exit=0, alert НЕТ, отметка успеха ЕСТЬ — «ОК» при нуле копий.
run_case "C сбой ИСТОЧНИКА виден"  1  0  1  да  нет

echo
TOTAL=$((PASSED + FAILED))
if [ "${FAILED}" -eq 0 ]; then
  echo "VERDICT: PASS (${PASSED}/${TOTAL}) — оба звена конвейера судятся, молчаливого «ОК» нет"
  exit 0
fi
echo "VERDICT: FAIL (${FAILED} из ${TOTAL})"
exit 1
