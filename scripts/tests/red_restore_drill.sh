#!/usr/bin/env bash
# RED `M-74` задача 1 — DRILL ОБЯЗАН ОТЛИЧАТЬ «копия не читается» ОТ «нет контекста».
#
# СОСТОЯНИЕ: КРАСНАЯ ПО ПОСТРОЕНИЮ. Обёртки `deploy/bin/journal-restore-drill-cron.sh` ещё
# НЕ СУЩЕСТВУЕТ — её вносит engine-dev задачей 2. Это RED-first, а не поломка: тест —
# спецификация, и он написан ДО кода. Прецедент формы — `M-71` `red_egress_cap_boundary`
# (COMPILE-RED против несуществующей константы) и `M-72` `td177_…` (против необъявленного шва).
#
# ЧТО ПИННИТСЯ И ПОЧЕМУ ИМЕННО ЭТО (`C-187` B-3).
#
# Самый старый сегмент боевого журнала — LEGACY-класса, и прод-читатель откроет его ТОЛЬКО
# при явной записи в манифесте `journal.legacy.json` (`crates/journal/src/segments.rs:22-24`,
# fail-closed находка `C-005` C2). Замер на проде 2026-08-30: манифест есть (25 Б), рядом
# `journal.meta` и `journal.replay-digest.json`.
#
# Отсюда исход, который НЕЛЬЗЯ смешивать: если drill восстановит сегменты БЕЗ манифеста,
# читатель откажет — и здоровая копия будет объявлена битой. Drill, не различающий эти два
# случая, производит ЛОЖНУЮ ТРЕВОГУ на исправной копии; такую процедуру выключат, и мы
# останемся без наблюдения вовсе. Поэтому сценарий `M` требует ОТДЕЛЬНОЙ причины отказа,
# а не общего провала.
#
# СЦЕНАРИИ
#   H  здоровая выборка + все sidecar'ы     → drill ПРОХОДИТ, состояние ok=1
#   C  один сегмент повреждён               → drill ОТКАЗЫВАЕТ, ok=0, причина называет ЧТЕНИЕ
#   M  sidecar'ов нет                       → drill ОТКАЗЫВАЕТ, ok=0, причина называет КОНТЕКСТ
#                                              (ОТЛИЧНАЯ от причины C — это и есть предмет)
#   A  файл состояния пишется АТОМАРНО      → оборванная запись не читается как успех
#
# `H` — позитивный контроль. Без него `C` и `M` зелены против drill'а, который отказывает
# ВСЕГДА: «отказал на битой копии» верно и для процедуры, которая не работает никогда.
#
# ЧЕГО ПРОБА НЕ ЛОВИТ, названо:
#   • она не качает ничего из Storage Box — сеть и права не проверяются, только логика
#     локального восстановления и чтения. Прод-пруф остаётся отдельной задачей;
#   • порча в НЕВЫБРАННОМ сегменте не моделируется: это принятый предел выборки, названный
#     в спеке `M-74`, а не дефект пробы.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DRILL="${ROOT}/deploy/bin/journal-restore-drill-cron.sh"

PASSED=0; FAILED=0
pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die()  { echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 1; }

REG="$(mktemp /tmp/red-drill-reg-XXXXXX)" || die mktemp
cleanup() {
  [ -n "${KEEP_FIXTURES:-}" ] && { echo "песочницы оставлены: ${REG}"; return 0; }
  while IFS= read -r d; do
    case "$d" in /tmp/red-drill-*) [ -d "$d" ] && rm -rf "$d" ;; esac
  done < "${REG}" 2>/dev/null
  rm -f "${REG}"
}
trap cleanup EXIT

# ── ЗАЯВЛЕННОЕ КРАСНОЕ. Обёртки нет — это состояние задачи 2, а не сбой пробы.
if [ ! -f "${DRILL}" ]; then
  echo "── RESTORE-DRILL: обёртка ещё не внесена"
  fail "обёртки ${DRILL#${ROOT}/} НЕ СУЩЕСТВУЕТ — RED задачи 1 (её вносит engine-dev задачей 2)"
  echo
  echo "VERDICT: FAIL (1 из 1) — RED-first: спецификация есть, реализации нет"
  exit 1
fi

# ── Фикстура: холодная «копия» с сегментами и sidecar'ами ─────────────────────────────
make_copy() { # $1=имя переменной под каталог; $2=«с-манифестом»|«без-манифеста»
  local d
  d="$(mktemp -d /tmp/red-drill-XXXXXX)" || die mktemp
  printf '%s\n' "${d}" >> "${REG}"
  mkdir -p "${d}/cold" "${d}/restore" "${d}/state" || die "каркас"
  # Три сегмента: имена несут индекс, порядок тотальный — алгоритм отбора детерминирован.
  local i
  for i in 0001 0002 0003; do
    printf 'SEGMENT-%s-PAYLOAD' "$i" > "${d}/cold/segment-${i}.jrnl" || die "сегмент"
  done
  if [ "$2" = "с-манифестом" ]; then
    printf '{"legacy":["segment-0001.jrnl"]}' > "${d}/cold/journal.legacy.json" || die "манифест"
    printf 'meta' > "${d}/cold/journal.meta" || die "мета"
  fi
  printf -v "$1" '%s' "${d}"
}

run_drill() { # $1=каталог песочницы → код возврата обёртки
  ( cd "$1" && JOURNAL_DRILL_COLD="$1/cold" \
      JOURNAL_DRILL_RESTORE="$1/restore" \
      JOURNAL_DRILL_STATE="$1/state/journal-restore-drill.json" \
      bash "${DRILL}" >/dev/null 2>&1 )
}

state_ok()     { grep -o '"ok"[[:space:]]*:[[:space:]]*[01]' "$1/state/journal-restore-drill.json" 2>/dev/null | grep -oE '[01]$'; }
state_reason() { sed -n 's/.*"reason"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$1/state/journal-restore-drill.json" 2>/dev/null; }

echo "── RESTORE-DRILL: читаемость копии, а не только её существование"

# H — позитивный контроль
make_copy BOX_H с-манифестом
if run_drill "${BOX_H}" && [ "$(state_ok "${BOX_H}")" = "1" ]; then
  pass "H здоровая выборка + sidecar'ы ⇒ drill прошёл, ok=1"
else
  fail "H здоровая выборка ⇒ drill НЕ прошёл (ok=$(state_ok "${BOX_H}")); всё остальное зелено по отсутствию предмета"
fi

# C — порча
make_copy BOX_C с-манифестом
printf 'МУСОР' > "${BOX_C}/cold/segment-0002.jrnl" || die "порча"
reason_c=""
if run_drill "${BOX_C}"; then
  fail "C повреждённый сегмент ⇒ drill ПРОШЁЛ — копия объявлена читаемой, не будучи ею"
else
  reason_c="$(state_reason "${BOX_C}")"
  if [ "$(state_ok "${BOX_C}")" = "0" ] && [ -n "${reason_c}" ]; then
    pass "C повреждённый сегмент ⇒ отказ, ok=0, причина: ${reason_c}"
  else
    fail "C отказ есть, но состояние не выставлено честно (ok=$(state_ok "${BOX_C}"), причина «${reason_c}»)"
  fi
fi

# M — нет контекста; причина ОБЯЗАНА отличаться от C
make_copy BOX_M без-манифеста
if run_drill "${BOX_M}"; then
  fail "M sidecar'ов нет ⇒ drill ПРОШЁЛ; legacy-сегмент не мог быть прочитан, значит проверка вакуумна"
else
  reason_m="$(state_reason "${BOX_M}")"
  if [ "$(state_ok "${BOX_M}")" != "0" ] || [ -z "${reason_m}" ]; then
    fail "M отказ есть, но состояние не выставлено честно (ok=$(state_ok "${BOX_M}"), причина «${reason_m}»)"
  elif [ "${reason_m}" = "${reason_c}" ]; then
    fail "M причина отказа СОВПАДАЕТ с причиной порчи («${reason_m}») — drill не отличает «копия битая» от «нет контекста», и здоровая копия будет объявлена битой"
  else
    pass "M sidecar'ов нет ⇒ отказ с ОТДЕЛЬНОЙ причиной: ${reason_m}"
  fi
fi

# A — атомарность записи состояния
make_copy BOX_A с-манифестом
printf '{"ok": 1, "ts": "2026-01-01T00:00:0' > "${BOX_A}/state/journal-restore-drill.json" || die "обрывок"
if run_drill "${BOX_A}" && [ "$(state_ok "${BOX_A}")" = "1" ]; then
  if grep -q '"ts"' "${BOX_A}/state/journal-restore-drill.json" && grep -q '}' "${BOX_A}/state/journal-restore-drill.json"; then
    pass "A оборванное состояние ПЕРЕЗАПИСАНО целиком — частичная запись не переживает прогон"
  else
    fail "A состояние осталось оборванным — потребитель прочтёт мусор как успех"
  fi
else
  fail "A drill на здоровой копии не прошёл (ok=$(state_ok "${BOX_A}")) — сценарий не судит атомарность"
fi

echo
TOTAL=$((PASSED + FAILED))
if [ "${FAILED}" -eq 0 ]; then
  echo "VERDICT: PASS (${PASSED}/${TOTAL}) — drill различает нечитаемость и отсутствие контекста"
  exit 0
fi
echo "VERDICT: FAIL (${FAILED} из ${TOTAL})"
exit 1
