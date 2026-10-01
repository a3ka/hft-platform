#!/usr/bin/env bash
# Проба карты CI-паритета verify_M-90.sh (C-265 F2). Режим VERIFY_M90_CI_DRY=1 — только учёт шагов,
# без исполнения. Три мира на копиях ci.yml; setup-страж на каждом (копия действительно изменена).
set -uo pipefail
cd "$(dirname "$0")/../.."
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
F=0
cp .github/workflows/ci.yml "$T/ok.yml"
python3 - "$T" <<'PY'
import sys; d=sys.argv[1]+'/'; s=open(d+'ok.yml').read()
a="      - name: Проба агрегата"; b="        run: git fetch --no-tags origin '+refs/salvage/*:refs/salvage/*'\n"
assert a in s and b in s, "SETUP: якоря правки не найдены в ci.yml"
open(d+'newblock.yml','w').write(s.replace(a,"      - name: новый шаг\n        run: |\n          echo ${{ github.sha }}\n"+a,1))
open(d+'stale.yml','w').write(s.replace(b,"",1))
PY
[ $? -eq 0 ] || { echo "FAIL  SETUP: копии ci.yml не построены"; exit 1; }
case_() { # <имя> <файл> <ожидаемый exit> <обязательная строка>
  out=$(VERIFY_M90_CI_DRY=1 VERIFY_M90_CI_FILE="$2" bash scripts/verify_M-90.sh 2>&1); rc=$?
  if [ "$rc" -eq "$3" ] && grep -qF -- "$4" <<<"$out"; then echo "PASS  $1 (exit=$rc)"
  else echo "FAIL  $1: exit=$rc (ожидался $3) или нет строки «$4»"; F=$((F+1)); fi
}
cmp -s "$T/ok.yml" "$T/newblock.yml" && { echo "FAIL  SETUP: newblock не изменён"; F=$((F+1)); }
cmp -s "$T/ok.yml" "$T/stale.yml" && { echo "FAIL  SETUP: stale не изменён"; F=$((F+1)); }
case_ "честный ci.yml: всё учтено"            "$T/ok.yml"       0 "учтено шагов"
case_ "новый шаг с \${{ }} вне карты ⇒ FAIL"  "$T/newblock.yml" 1 "не стоит в карте исключений"
case_ "исключение без шага ⇒ карта протухла"  "$T/stale.yml"    1 "карта протухла"
[ "$F" -eq 0 ] && { echo "VERDICT: PASS — 3 сценария"; exit 0; }
echo "VERDICT: FAIL (провалов: $F)"; exit 1
