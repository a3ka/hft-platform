#!/usr/bin/env bash
# Проба барьера `check_ci_aggregate.sh`. Барьер, не проверенный сам, — намерение.
# У каждого сценария SETUP-GUARD: без него «красное» неотличимо от «построен не тот файл».
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BAR="${ROOT}/scripts/check_ci_aggregate.sh"
P=0; F=0
pass(){ echo "PASS  $*"; P=$((P+1)); }
fail(){ echo "FAIL  $*"; F=$((F+1)); }
die(){ echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2; exit 2; }
[ -x "${BAR}" ] || die "барьера нет: ${BAR}"
BOX="$(mktemp -d /tmp/red-ciagg-XXXXXX)" || die mktemp
trap 'rm -rf "${BOX}"' EXIT

mk(){ # $1=needs через запятую  $2=проверяемые через запятую
  local f; f="$(mktemp "${BOX}/ci-XXXXXX.yml")"
  { echo "jobs:"; echo "  build:"; echo "    runs-on: x"
    echo "  status-check:"; echo "    needs: [$1]"; echo "    if: always()"
    echo "    steps:"; echo "      - run: |"
    printf '          if [[ '
    local first=1
    IFS=',' read -ra A <<< "$2"
    for j in "${A[@]}"; do j="$(echo "$j"|xargs)"; [ -z "$j" ] && continue
      [ $first -eq 1 ] && first=0 || printf ' || '
      printf '"${{ needs.%s.result }}" != "success"' "$j"; done
    echo ' ]]; then'; echo "            exit 1"; echo "          fi"
  } > "${f}"; printf '%s' "${f}"
}
run(){ CI_WORKFLOW="$1" bash "${BAR}" >"${BOX}/out" 2>&1; echo $?; }

# S1 — списки совпадают ⇒ ЗЕЛЁНЫЙ (барьер не запрещает верное)
C="$(mk 'a, b, c' 'a,b,c')"; grep -q 'needs: \[a, b, c\]' "$C" || die "S1 setup"
RC="$(run "$C")"
[ "$RC" -eq 0 ] && pass "S1 списки совпадают ⇒ барьер ЗЕЛЁН" || fail "S1 верная конфигурация отвергнута (rc=$RC): $(cat "${BOX}/out")"

# S2 — ЖИВОЙ ДЕФЕКТ: джоб в needs, но не проверяется
C="$(mk 'a, b, secret-material' 'a,b')"
grep -q 'secret-material' "$C" || die "S2 setup: джоб не попал в needs"
grep -q 'needs.secret-material.result' "$C" && die "S2 setup: он всё-таки проверяется"
RC="$(run "$C")"
if [ "$RC" -ne 0 ] && grep -q 'НЕ ВЛИЯЮТ на исход' "${BOX}/out"; then
  pass "S2 джоб в needs без проверки результата ПОЙМАН — дефект, найденный в ci.yml 2026-09-09"
else fail "S2 ПРОПУЩЕН (rc=$RC) — барьер не ловит то, ради чего заведён"; fi

# S3 — обратная асимметрия: проверяется джоб, которого нет в needs
C="$(mk 'a, b' 'a,b,phantom')"
grep -q 'needs.phantom.result' "$C" || die "S3 setup: фантом не в условии"
grep -qE 'needs: \[a, b\]' "$C" || die "S3 setup: needs не тот"
RC="$(run "$C")"
if [ "$RC" -ne 0 ] && grep -q 'ОТСУТСТВУЮТ в needs' "${BOX}/out"; then
  pass "S3 проверка джоба, отсутствующего в needs, ПОЙМАНА — агрегат читал бы пустой результат"
else fail "S3 ПРОПУЩЕНА (rc=$RC)"; fi

# S4 — агрегата нет вовсе ⇒ setup-отказ, а не тихий зелёный
F2="$(mktemp "${BOX}/ci-XXXXXX.yml")"; printf 'jobs:\n  build:\n    runs-on: x\n' > "$F2"
grep -q 'status-check' "$F2" && die "S4 setup: агрегат всё-таки есть"
RC="$(run "$F2")"
[ "$RC" -eq 2 ] && pass "S4 отсутствие агрегата ⇒ SETUP-ОТКАЗ (rc=2), а не тихий зелёный" \
  || fail "S4 при отсутствии агрегата барьер вернул rc=$RC — молчание вместо отказа"

echo
T=$((P+F)); [ "$F" -eq 0 ] && { echo "VERDICT: PASS ($P/$T)"; exit 0; }
echo "VERDICT: FAIL ($F из $T)"; exit 1
