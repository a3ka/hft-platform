#!/usr/bin/env bash
# Проба диск-преамбулы verify — `scripts/check_disk_budget.sh` (M-60b G6.2).
#
# ЗАЧЕМ. 2026-08-01: `verify_alerting.sh` дал FAIL `exit=101` от ENOSPC — ложный красный,
# «заставляет чинить исправное»; диск в тот период 83–85 %. Отдельно аудит §2.6: общий
# `CARGO_TARGET_DIR` вне дерева подменяет исполняемый бинарь — verify гоняет не тот код.
# Преамбула делает красное НАЗВАННЫМ ДО старта прогона, вместо ENOSPC-мистики посреди него.
#
# КОНТРАКТ ГЕЙТА (задаётся этой пробой, реализуется dev'ом):
#   MIN_FREE_KB=<число>  порог свободного места (KB) на ФС текущего дерева; дефолт — в скрипте;
#   exit 0  — свободно ≥ порога И (`CARGO_TARGET_DIR` не задан ИЛИ внутри текущего дерева);
#   exit≠0  — ПЕРВАЯ строка вывода называет отказавший ресурс:
#             содержит `диск:` (свободно X < порога Y) либо `CARGO_TARGET_DIR` (вне дерева);
#             при ДВУХ отказах названы ОБА (DB-6 — отказы не маскируют друг друга);
#             нечитаемый/read-only `CARGO_TARGET_DIR` внутри дерева — FAIL как отказ
#             целевого носителя verify-преамбулы;
#             негодный порог (пусто/нечисло/за пределами intmax) ⇒ FAIL fail-closed —
#             страж не ОТКЛЮЧАЕТСЯ негодным параметром (урок A-008: валидация тем же
#             парсером, что потребляет значение).
#
# ПРЕДЕЛ (спека §10): срабатывает только при ЗАПУСКЕ verify — диск между прогонами не
# сторожит; ENOSPC ПОСРЕДИ прогона не предотвращает — делает названным красное ДО старта.

set -uo pipefail

ROOT_REPO="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT_REPO}/scripts/check_disk_budget.sh}"
HUGE_KB=999999999999   # заведомо больше свободного места любой нашей машины

FAILED=0
PASSED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

# ── Страж барьера. Пока гейта нет, bash вернёт 127, и негативные сценарии («ожидаю exit≠0»)
# позеленели бы на пустом месте — проба аттестовала бы несуществующий механизм.
[ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}. 127 от bash неотличим от честного отказа гейта."
bash -n "${BARRIER}" 2>/dev/null || die "барьер не парсится — сценарии мерили бы ошибку интерпретатора."

# ── Фикстура: своё «дерево» + захваченный вывод ────────────────────────────────────────
D="$(mktemp -d /tmp/red-diskbudget-XXXXXX)" || die mktemp
cleanup_fixtures() {
  [ -n "${KEEP_FIXTURES:-}" ] && { echo "фикстура оставлена (KEEP_FIXTURES): ${D}"; return 0; }
  # права снимаются сценариями DB-7a/DB-7b; без возврата u+rwX каталог не удалится и
  # переживёт прогон — тот же класс утечки, что дал 10 400 каталогов /tmp у red_docs_freeze.
  case "${D}" in /tmp/red-diskbudget-*) chmod -R u+rwX "${D}" 2>/dev/null; rm -rf "${D}" ;; esac
  # DB-4 создаёт каталог ВНЕ дерева фикстуры — он не под "${D}" и требует отдельной уборки.
  # Замер до правки: по два каталога `red-diskbudget-outside-*` за каждый прогон.
  case "${OUTSIDE:-}" in /tmp/red-diskbudget-outside-*) rm -rf "${OUTSIDE}" ;; esac
}
trap cleanup_fixtures EXIT
OUT="${D}/out.txt"
# страж: на ФС фикстуры есть хотя бы 1 KB свободного — иначе DB-2/DB-5 меряют не порог
FREE_KB="$(df -Pk "${D}" | awk 'NR==2 {print $4}')"
[ -n "${FREE_KB}" ] && [ "${FREE_KB}" -ge 1 ] || die "на ФС фикстуры нет свободного 1 KB — DB-2 мерил бы не порог"

run_barrier() { # $1=MIN_FREE_KB ("UNSET" — не передавать) $2=CARGO_TARGET_DIR ("UNSET" — не задан)
  local mf="$1" ct="$2" envargs=(env -u CARGO_TARGET_DIR -u MIN_FREE_KB)
  [ "$mf" != "UNSET" ] && envargs+=("MIN_FREE_KB=$mf")
  [ "$ct" != "UNSET" ] && envargs+=("CARGO_TARGET_DIR=$ct")
  [ -n "${DF_SHIM_DIR:-}" ] && envargs+=("PATH=${DF_SHIM_DIR}:${PATH}")
  local st
  ( cd "${D}" && "${envargs[@]}" bash "${BARRIER}" >"${OUT}" 2>&1 )
  st=$?
  # 126/127 — отказ СРЕДЫ, а не вердикт гейта. positive_control этот класс НЕ ловит: он
  # проверяет только ГОДНУЮ ветку, а падать может ОТКАЗНАЯ (барьер зовёт отсутствующий в CI
  # `jq`/`gh`). Тогда каждый сценарий «ожидаю отказ» зеленеет против механизма, который не
  # отказывает, а падает (C-086 F-086-1 требовал именно этого различения).
  case $st in
    126|127) die "барьер вернул ${st} (не найден / не исполняется) — это отказ СРЕДЫ, а не отказ гейта; сценарий засчитал бы падение за срабатывание" ;;
  esac
  return $st
}
positive_control() {
  ( cd "${D}" && env -u CARGO_TARGET_DIR MIN_FREE_KB=1 bash "${BARRIER}" >"${OUT}" 2>&1 ) \
    || die "барьер не проходит заведомо годную фикстуру (MIN_FREE_KB=1, target не задан); setup не состоялся"
}

echo "── Диск-преамбула verify (M-60b G6.2): сценарии DB-1..DB-7b ──"
echo "барьер: ${BARRIER}"
echo
positive_control
echo "SETUP positive-control: барьер принимает заведомо годную диск-фикстуру"
echo

# DB-1 — порог поднят выше факта ⇒ блок, ПЕРВАЯ строка называет диск
if run_barrier "${HUGE_KB}" "UNSET"; then
  fail "DB-1 порог ${HUGE_KB} KB заведомо выше факта, а гейт ПРОШЁЛ — порога нет"
else
  if head -1 "${OUT}" | grep -q "диск:"; then
    pass "DB-1 нехватка места поймана, первая строка называет диск"
  else
    fail "DB-1 гейт красный, но первая строка НЕ называет диск: «$(head -1 "${OUT}")» — ENOSPC-мистика осталась"
  fi
fi

# DB-2 — порог ниже факта ⇒ проход (анти-ложноположительный)
run_barrier "1" "UNSET" && pass "DB-2 порог ниже факта — проход" \
                        || fail "DB-2 ложное срабатывание: места достаточно, а гейт блокирует"

# DB-2b — порог РОВНО равен фактическому free ⇒ проход: контракт free >= threshold.
# Точную границу нельзя стабильно снять с общей ФС: между `df` пробы и `df` барьера
# свободное место может измениться. Для ЭТОГО boundary-сценария фиксируем confounder
# `df`-shim'ом; остальные сценарии выше/ниже границы используют реальную ФС.
EQUAL_FREE_KB=4242
DF_SHIM_DIR="${D}/df-shim"
mkdir -p "${DF_SHIM_DIR}" || die "DB-2b df-shim mkdir"
# Тело shim'а пишется heredoc'ом, а не `printf`-в-`printf`: форма с `96%%` даёт в файле
# `96%`, и внутренний printf падает с `invalid format character`, обрывая строку. Поле $4
# при этом всё равно вычитывалось — сценарий «работал» на оборванном выводе с ошибкой в
# stderr, то есть проверял не то, что заявлял.
cat > "${DF_SHIM_DIR}/df" <<'SHIM' || die "DB-2b df-shim write"
#!/usr/bin/env bash
echo "Filesystem 1024-blocks Used Available Capacity Mounted on"
echo "fixture 100000 95758 4242 96% ."
SHIM
chmod +x "${DF_SHIM_DIR}/df" || die "DB-2b df-shim chmod"
run_barrier "${EQUAL_FREE_KB}" "UNSET" \
  && pass "DB-2b порог ровно равен фактическому free (${EQUAL_FREE_KB} KB) — проход" \
  || fail "DB-2b равенство free == threshold отвергнуто — контракт стал строгим >"
unset DF_SHIM_DIR

# DB-3 — негодный порог ⇒ блок fail-closed (страж не отключается негодным параметром)
run_barrier "" "UNSET"    && fail "DB-3a пустой порог дал ПРОХОД — страж отключаем пустотой" \
                          || pass "DB-3a пустой порог: fail-closed"
run_barrier "abc" "UNSET" && fail "DB-3b нечисловой порог дал ПРОХОД" \
                          || pass "DB-3b нечисловой порог: fail-closed"
run_barrier "99999999999999999999999999" "UNSET" \
                          && fail "DB-3c порог за пределами intmax дал ПРОХОД (урок A-008)" \
                          || pass "DB-3c порог за пределами intmax: fail-closed"

# DB-4 — CARGO_TARGET_DIR вне дерева ⇒ блок, первая строка называет target-dir
OUTSIDE="$(mktemp -d /tmp/red-diskbudget-outside-XXXXXX)" || die "mktemp outside"   # убирается trap'ом
case "${OUTSIDE}/" in "${D}/"*) die "фикстура: OUTSIDE оказался внутри дерева";; esac
if run_barrier "1" "${OUTSIDE}"; then
  fail "DB-4 CARGO_TARGET_DIR вне дерева ПРОШЁЛ — подмена бинаря ненаблюдаема (аудит §2.6)"
else
  if head -1 "${OUT}" | grep -q "CARGO_TARGET_DIR"; then
    pass "DB-4 target вне дерева пойман, первая строка называет CARGO_TARGET_DIR"
  else
    fail "DB-4 гейт красный, но первая строка не называет CARGO_TARGET_DIR: «$(head -1 "${OUT}")»"
  fi
fi

# DB-5 — CARGO_TARGET_DIR внутри дерева / не задан ⇒ проход
run_barrier "1" "${D}/target" && pass "DB-5a target внутри дерева — проход" \
                              || fail "DB-5a ложное срабатывание на target внутри дерева"
run_barrier "1" "UNSET"       && pass "DB-5b target не задан — проход" \
                              || fail "DB-5b ложное срабатывание при незаданном target"

# DB-6 — оба отказа сразу ⇒ блок, названы ОБА (не маскируют друг друга)
if run_barrier "${HUGE_KB}" "${OUTSIDE}"; then
  fail "DB-6 двойной отказ дал ПРОХОД"
else
  MISS=""
  grep -q "диск:" "${OUT}"             || MISS="${MISS} диск"
  grep -q "CARGO_TARGET_DIR" "${OUT}"  || MISS="${MISS} CARGO_TARGET_DIR"
  if [ -z "${MISS}" ]; then
    pass "DB-6 двойной отказ: названы ОБА ресурса"
  else
    fail "DB-6 двойной отказ, но не назван:${MISS} — первый отказ маскирует второй"
  fi
fi

# DB-7 — target внутри дерева, но сам носитель недоступен ⇒ блок с именем CARGO_TARGET_DIR
UNREADABLE="${D}/target-unreadable"
mkdir -p "${UNREADABLE}" || die "DB-7a mkdir"
chmod a-rwx "${UNREADABLE}" || die "DB-7a chmod"
[ ! -r "${UNREADABLE}" ] && [ ! -x "${UNREADABLE}" ] \
  || die "DB-7a: chmod не сделал target нечитаемым — сценарий тестировал бы не права"
if run_barrier "1" "${UNREADABLE}"; then
  fail "DB-7a нечитаемый CARGO_TARGET_DIR внутри дерева ПРОШЁЛ — отказ носителя verify не пойман"
else
  if head -1 "${OUT}" | grep -q "CARGO_TARGET_DIR"; then
    pass "DB-7a нечитаемый target пойман, первая строка называет CARGO_TARGET_DIR"
  else
    fail "DB-7a гейт красный, но первая строка не называет CARGO_TARGET_DIR: «$(head -1 "${OUT}")»"
  fi
fi
chmod u+rwx "${UNREADABLE}" 2>/dev/null || true

READONLY="${D}/target-readonly"
mkdir -p "${READONLY}" || die "DB-7b mkdir"
chmod a-w "${READONLY}" || die "DB-7b chmod"
[ ! -w "${READONLY}" ] || die "DB-7b: chmod не сделал target read-only — сценарий тестировал бы не права"
if run_barrier "1" "${READONLY}"; then
  fail "DB-7b read-only CARGO_TARGET_DIR внутри дерева ПРОШЁЛ — cargo упадёт позже EROFS"
else
  if head -1 "${OUT}" | grep -q "CARGO_TARGET_DIR"; then
    pass "DB-7b read-only target пойман, первая строка называет CARGO_TARGET_DIR"
  else
    fail "DB-7b гейт красный, но первая строка не называет CARGO_TARGET_DIR: «$(head -1 "${OUT}")»"
  fi
fi
chmod u+w "${READONLY}" 2>/dev/null || true

# DB-8 — барьер обязан БРАТЬ свободное место через PATH-резолв `df`, а не звать `/usr/bin/df`.
# Без этого стража DB-2b — плацебо: замер показал, что стаб с `/usr/bin/df` видит реальные
# 45 ГБ вместо подставных 4242 KB и ВСЁ РАВНО проходит DB-2b (граница просто не исполняется).
# Страж зеркальный к DB-2b: под ТЕМ ЖЕ shim'ом порог на 1 KB выше подставного free обязан
# дать отказ. Барьер, игнорирующий shim, увидит настоящий диск и пройдёт — то есть покраснеет
# здесь. Конфаундер (движение реального свободного места) исключён: обе величины из shim'а.
DF_SHIM_DIR="${D}/df-shim-guard"
mkdir -p "${DF_SHIM_DIR}" || die "DB-8 df-shim mkdir"
cat > "${DF_SHIM_DIR}/df" <<'SHIM' || die "DB-8 df-shim write"
#!/usr/bin/env bash
echo "Filesystem 1024-blocks Used Available Capacity Mounted on"
echo "fixture 100000 95758 4242 96% ."
SHIM
chmod +x "${DF_SHIM_DIR}/df" || die "DB-8 df-shim chmod"
if run_barrier "4243" "UNSET"; then
  fail "DB-8 порог 4243 KB против подставного free 4242 KB ПРОШЁЛ — барьер не читает df через PATH, значит DB-2b проверял не границу"
else
  if head -1 "${OUT}" | grep -q "диск:"; then
    pass "DB-8 df берётся через PATH-резолв: подставной free 4242 < порога 4243 назван диском"
  else
    fail "DB-8 гейт красный, но первая строка не называет диск: «$(head -1 "${OUT}")»"
  fi
fi
unset DF_SHIM_DIR

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Диск-преамбула не даёт заявленной гарантии. Пока проба красная, ENOSPC остаётся"
  echo "мистикой посреди прогона (ложный красный exit=101 от 2026-08-01 повторим)."
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/${PASSED}) — красное названо до старта, отказы не маскируются"
