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
OUT="${D}/out.txt"
# страж: на ФС фикстуры есть хотя бы 1 KB свободного — иначе DB-2/DB-5 меряют не порог
FREE_KB="$(df -Pk "${D}" | awk 'NR==2 {print $4}')"
[ -n "${FREE_KB}" ] && [ "${FREE_KB}" -ge 1 ] || die "на ФС фикстуры нет свободного 1 KB — DB-2 мерил бы не порог"

run_barrier() { # $1=MIN_FREE_KB ("UNSET" — не передавать) $2=CARGO_TARGET_DIR ("UNSET" — не задан)
  local mf="$1" ct="$2" envargs=(env -u CARGO_TARGET_DIR -u MIN_FREE_KB)
  [ "$mf" != "UNSET" ] && envargs+=("MIN_FREE_KB=$mf")
  [ "$ct" != "UNSET" ] && envargs+=("CARGO_TARGET_DIR=$ct")
  ( cd "${D}" && "${envargs[@]}" bash "${BARRIER}" >"${OUT}" 2>&1 )
}

echo "── Диск-преамбула verify (M-60b G6.2): сценарии DB-1..DB-6 ──"
echo "барьер: ${BARRIER}"
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

# DB-3 — негодный порог ⇒ блок fail-closed (страж не отключается негодным параметром)
run_barrier "" "UNSET"    && fail "DB-3a пустой порог дал ПРОХОД — страж отключаем пустотой" \
                          || pass "DB-3a пустой порог: fail-closed"
run_barrier "abc" "UNSET" && fail "DB-3b нечисловой порог дал ПРОХОД" \
                          || pass "DB-3b нечисловой порог: fail-closed"
run_barrier "99999999999999999999999999" "UNSET" \
                          && fail "DB-3c порог за пределами intmax дал ПРОХОД (урок A-008)" \
                          || pass "DB-3c порог за пределами intmax: fail-closed"

# DB-4 — CARGO_TARGET_DIR вне дерева ⇒ блок, первая строка называет target-dir
OUTSIDE="$(mktemp -d /tmp/red-diskbudget-outside-XXXXXX)" || die "mktemp outside"
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

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED})"
  echo "Диск-преамбула не даёт заявленной гарантии. Пока проба красная, ENOSPC остаётся"
  echo "мистикой посреди прогона (ложный красный exit=101 от 2026-08-01 повторим)."
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/${PASSED}) — красное названо до старта, отказы не маскируются"
