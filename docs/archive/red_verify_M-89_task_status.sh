#!/usr/bin/env bash
# Проба шага `task-status` гейта `scripts/verify_M-89.sh` — предмет: перенос шага из
# `verify_M-87.sh` с исправлениями `TD-221`.
#
# Три свойства (`docs/workflow/harness-track.md` §5):
#   1. ПОЗИТИВНЫЙ КОНТРОЛЬ — спека с названным остатком у каждой открытой задачи даёт PASS;
#   2. АНТИ-ПЛАЦЕБО — открытая задача БЕЗ остатка даёт FAIL; включая формы, которые редакция
#      M-87 пропускала: номер `2bis` (TD-221 (а)) и статус `🟡` (TD-221 (б));
#   3. SETUP-СТРАЖ — режим пробы обязан исполнять ИМЕННО шаг task-status (строка `task-status:`
#      в выводе); молчание — не PASS.
#
# ПРЕДЕЛ (назван в самом гейте): барьер судит НАЛИЧИЕ заявления, не его правдивость.
#
# Прогон: bash scripts/tests/red_verify_M-89_task_status.sh

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUT="${ROOT}/scripts/verify_M-89.sh"

PASS=0; FAIL=0
ok()  { PASS=$((PASS + 1)); printf 'ok         %-34s %s\n' "$1" "${2:-}"; }
nok() { FAIL=$((FAIL + 1)); printf 'FAIL       %-34s %s\n' "$1" "$2"; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/red-m89-taskstatus-XXXXXX")"
trap 'rm -rf "${WORK}"' EXIT

spec() { # <имя> <строки таблицы…> → путь
  local f="${WORK}/$1.md"; shift
  {
    echo "# спека-фикстура"
    echo
    echo "| # | Status | задача | зона | проверка |"
    echo "|---|---|---|---|---|"
    for row in "$@"; do echo "$row"; done
  } > "$f"
  printf '%s' "$f"
}

run() { # <spec> → печатает вывод; код возврата в $RC
  OUT=$(VERIFY_M89_TASK_STATUS_ONLY=1 VERIFY_M89_SPEC="$1" bash "$SUT" 2>&1); RC=$?
}

expect() { # <имя> <spec> <ожидаемый rc> <описание>
  run "$2"
  if ! printf '%s\n' "$OUT" | grep -q 'task-status:'; then
    nok "$1" "SETUP-СТРАЖ: шаг task-status не исполнялся (нет строки в выводе): $OUT"
    return
  fi
  if [ "$RC" -eq "$3" ]; then ok "$1" "$4 (rc=$RC)"; else nok "$1" "$4 — ожидался rc=$3, получен rc=$RC: $(printf '%s' "$OUT" | grep task-status)"; fi
}

S_DONE=$(spec all_done \
  '| 1 | ✅ DONE | сделано | dev | шаг |' \
  '| 2bis | ✅ DONE | сделано | dev | шаг |')
expect "s1-all-done" "$S_DONE" 0 "все DONE ⇒ PASS"

S_OPEN_OK=$(spec open_named \
  '| 1 | ⏳ OPEN | задача. ОСТАТОК: всё — ещё не начата | dev | шаг |' \
  '| 2 | ✅ DONE | сделано | dev | шаг |')
expect "s2-open-with-remainder" "$S_OPEN_OK" 0 "OPEN с названным остатком ⇒ PASS"

S_OPEN_BAD=$(spec open_silent \
  '| 1 | ⏳ OPEN | задача без пояснения | dev | шаг |')
expect "s3-open-silent" "$S_OPEN_BAD" 1 "OPEN без остатка ⇒ FAIL"

S_BIS=$(spec bis_silent \
  '| 2bis | ⏳ OPEN | задача без пояснения | dev | шаг |')
expect "s4-2bis-silent (TD-221 а)" "$S_BIS" 1 "строка 2bis видна барьеру"

S_YEL=$(spec yellow_silent \
  '| 24 | 🟡 ЧАСТИЧНО | половина сделана | dev | шаг |')
expect "s5-yellow-silent (TD-221 б)" "$S_YEL" 1 "🟡 ЧАСТИЧНО без остатка ⇒ FAIL (слово статуса — не остаток)"

S_YEL_OK=$(spec yellow_named \
  '| 24 | 🟡 ЧАСТИЧНО | половина: остаток — бюджет не подключён | dev | шаг |')
expect "s6-yellow-with-remainder" "$S_YEL_OK" 0 "🟡 с остатком ⇒ PASS"

S_B=$(spec cyr_b_silent \
  '| 2б | ⏳ OPEN | задача без пояснения | dev | шаг |')
expect "s7-2б-silent" "$S_B" 1 "кириллическая буква в номере видна"

S_MIX=$(spec mixed \
  '| 1 | ⏳ OPEN | ОСТАТОК: всё | dev | шаг |' \
  '| 2bis | 🟡 | нет маркера | dev | шаг |' \
  '| 3 | ✅ DONE | сделано | dev | шаг |')
run "$S_MIX"
if [ "$RC" -eq 1 ] && printf '%s' "$OUT" | grep -q 'остатка: 2bis'; then
  ok "s8-names-offender" "провал называет ИМЕННО 2bis"
else
  nok "s8-names-offender" "ожидался rc=1 с именем 2bis: $(printf '%s' "$OUT" | grep task-status)"
fi

printf '\nитого: ok=%d fail=%d\n' "$PASS" "$FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL"; exit 1
