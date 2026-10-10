#!/usr/bin/env bash
# Проба карты CI-паритета verify_M-98.sh (C-265 F2; копия пробы M-94 — общий шаг остаётся долгом TD-222). Режим VERIFY_M98_CI_DRY=1 — только учёт шагов,
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
# C-266 F2: работа, дописанная в ИСКЛЮЧЁННЫЙ многострочный блок
c='          echo "sha=${raw}" >> "$GITHUB_OUTPUT"\n'
e='          echo "All checks passed"\n'
assert s.count(c)==1 and s.count(e)==1, "SETUP: якоря блоков базы/агрегата не найдены"
open(d+'base_append.yml','w').write(s.replace(c,c+"          bash scripts/tests/red_artifact_ids.sh --battery\n",1))
open(d+'agg_append.yml','w').write(s.replace(e,e+"          bash scripts/check_extra.sh\n",1))
# агрегат: законный рост needs (новый джоб в условии) — карта обязана пропустить
f='"${{ needs.build-test.result }}" != "success" || '
assert s.count(f)==1, "SETUP: начало условия агрегата не найдено"
open(d+'agg_grow.yml','w').write(s.replace(f,f+'"${{ needs.new-job.result }}" != "success" || ',1))
# агрегат: команда, спрятанная в строку условия
g=' ]]; then\n            echo "One or more checks failed"'
assert s.count(g)==1, "SETUP: хвост условия агрегата не найден"
# A-043 К-3: (i) особая ветка шага не открывается по первой строке; (ii) `|-` — блок; (iii) `>` — FAIL
h="        run: bash scripts/check_review_fa.sh\n"
k="        run: |\n          bash scripts/tests/red_artifact_ids.sh\n"
assert s.count(h)==1 and s.count(k)==1, "SETUP: якоря check_review_fa / red_artifact_ids не найдены"
open(d+'rfa_append.yml','w').write(s.replace(h,"        run: |\n          bash scripts/check_review_fa.sh\n          bash scripts/evil.sh\n",1))
open(d+'chomp.yml','w').write(s.replace(k,"        run: |-\n          bash scripts/tests/red_artifact_ids.sh\n",1))
open(d+'folded.yml','w').write(s.replace(k,"        run: >\n          bash scripts/tests/red_artifact_ids.sh\n",1))
open(d+'agg_inject.yml','w').write(s.replace(g,' ]] || bash scripts/check_extra.sh; then\n            echo "One or more checks failed"',1))
PY
[ $? -eq 0 ] || { echo "FAIL  SETUP: копии ci.yml не построены"; exit 1; }
case_() { # <имя> <файл> <ожидаемый exit> <обязательная строка>
  out=$(VERIFY_M98_CI_DRY=1 VERIFY_M98_CI_FILE="$2" bash scripts/verify_M-98.sh 2>&1); rc=$?
  if [ "$rc" -eq "$3" ] && grep -qF -- "$4" <<<"$out"; then echo "PASS  $1 (exit=$rc)"
  else echo "FAIL  $1: exit=$rc (ожидался $3) или нет строки «$4»"; F=$((F+1)); fi
}
cmp -s "$T/ok.yml" "$T/newblock.yml" && { echo "FAIL  SETUP: newblock не изменён"; F=$((F+1)); }
cmp -s "$T/ok.yml" "$T/stale.yml" && { echo "FAIL  SETUP: stale не изменён"; F=$((F+1)); }
for w in base_append agg_append agg_grow agg_inject rfa_append chomp folded; do
  cmp -s "$T/ok.yml" "$T/$w.yml" && { echo "FAIL  SETUP: $w не изменён"; F=$((F+1)); }
done
case_ "честный ci.yml: всё учтено"            "$T/ok.yml"       0 "учтено шагов"
case_ "новый шаг с \${{ }} вне карты ⇒ FAIL"  "$T/newblock.yml" 1 "не стоит в карте исключений"
case_ "исключение без шага ⇒ карта протухла"  "$T/stale.yml"    1 "карта протухла"
case_ "строка дописана в блок базы ⇒ FAIL (C-266 F2)"        "$T/base_append.yml" 1 "карта протухла"
case_ "строка дописана в блок агрегата ⇒ FAIL"              "$T/agg_append.yml"  1 "карта протухла"
case_ "агрегат: новый джоб в условии — законно ⇒ PASS"      "$T/agg_grow.yml"    0 "учтено шагов"
case_ "агрегат: команда в строке условия ⇒ FAIL"            "$T/agg_inject.yml"  1 "карта протухла"
case_ "(i) check_review_fa + дописка ⇒ исполняется, не SKIP" "$T/rfa_append.yml"  0 "исполнилось бы «bash scripts/check_review_fa.sh»"
case_ "(ii) run: |- — литеральный блок, исполняется"         "$T/chomp.yml"       0 "исполнилось бы «bash scripts/tests/red_artifact_ids.sh»"
case_ "(iii) run: > — складывающий скаляр ⇒ FAIL"           "$T/folded.yml"      1 "блочный скаляр > не поддержан"
[ "$F" -eq 0 ] && { echo "VERDICT: PASS — 10 сценариев"; exit 0; }
echo "VERDICT: FAIL (провалов: $F)"; exit 1
