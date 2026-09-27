#!/usr/bin/env bash
# red_ci_aggregate.sh — ПРОБА агрегата «All checks passed» (`.github/workflows/ci.yml`, джоб
# `status-check`). Защита `main` требует ЕДИНСТВЕННЫЙ чек — этот; всё, чего он не видит, merge
# не держит.
#
# ЗАМЕР, ИЗ-ЗА КОТОРОГО ПРОБА ЗАВЕДЕНА (2026-09-26). Условие агрегата было РУЧНЫМ списком из 16
# проверок при 18 в `needs`: `secret-material` и `roadmap-sync` (обе с 2026-09-07) в него не
# попали, а при `if: always()` агрегат их провал не видел. Копия списка расходится молча при
# каждом новом джобе. Лечение — условие ВЫВОДИТСЯ из `needs` (`toJSON(needs)`); проба держит его.
#
# ЧТО ИСПОЛНЯЕТСЯ. Не копия логики, а ТОТ ЖЕ python-код, вынутый из `ci.yml` (heredoc между
# `python3 - <<'PY'` и `PY` в джобе `status-check`): проба судит то, что крутит CI.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CI="$ROOT/.github/workflows/ci.yml"
N=0; BAD=0
ok()  { N=$((N+1)); printf 'ok    %s\n' "$1"; }
bad() { N=$((N+1)); BAD=$((BAD+1)); printf 'FAIL  %s\n' "$1"; }

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT INT TERM

# Вынуть тело агрегата. SETUP-СТРАЖ: не нашли — проба судила бы пустоту.
python3 - "$CI" "$TMP/agg.py" <<'PY'
import re, sys
s = open(sys.argv[1], encoding="utf-8").read()
blk = s.split("\n  status-check:\n", 1)
if len(blk) != 2: sys.exit("нет джоба status-check")
m = re.search(r"python3 - <<'PY'\n(.*?)\n\s*PY\n", blk[1], re.S)
if not m: sys.exit("нет python-тела агрегата")
lines = m.group(1).split("\n")
ind = min(len(l) - len(l.lstrip()) for l in lines if l.strip())
open(sys.argv[2], "w", encoding="utf-8").write("\n".join(l[ind:] for l in lines) + "\n")
PY
if [ ! -s "$TMP/agg.py" ]; then echo "FAIL  SETUP: тело агрегата не извлечено из $CI"; echo "VERDICT: FAIL"; exit 1; fi

# Прогон тела на заданном needs. Возвращает код выхода.
agg() { NEEDS_JSON="$1" python3 "$TMP/agg.py" >"$TMP/out" 2>&1; echo $?; }
all_ok='{"build-test":{"result":"success"},"secret-material":{"result":"success"},"roadmap-sync":{"result":"success"}}'

# (1) позитивный контроль: всё success ⇒ проходит
[ "$(agg "$all_ok")" -eq 0 ] && ok "всё success ⇒ агрегат проходит" || bad "всё success, а агрегат красен — вечно-красная проба"
# (2) ЗАКРЫВАЕМЫЙ СЛУЧАЙ: провал secret-material ⇒ отказ, и назван именно он
j='{"build-test":{"result":"success"},"secret-material":{"result":"failure"},"roadmap-sync":{"result":"success"}}'
if [ "$(agg "$j")" -ne 0 ] && grep -q 'failed: secret-material' "$TMP/out"; then ok "провал secret-material ⇒ отказ с именем джоба"
else bad "провал secret-material НЕ держит агрегат — тот самый дефект ручного списка"; fi
# (3) провал roadmap-sync
j='{"build-test":{"result":"success"},"secret-material":{"result":"success"},"roadmap-sync":{"result":"failure"}}'
if [ "$(agg "$j")" -ne 0 ] && grep -q 'failed: roadmap-sync' "$TMP/out"; then ok "провал roadmap-sync ⇒ отказ с именем джоба"
else bad "провал roadmap-sync НЕ держит агрегат"; fi
# (4) cancelled и skipped — не успех
for r in cancelled skipped; do
  j='{"build-test":{"result":"'$r'"},"secret-material":{"result":"success"}}'
  [ "$(agg "$j")" -ne 0 ] && ok "исход $r ⇒ отказ" || bad "исход $r принят за успех"
done
# (5) пустой needs ⇒ отказ (fail-closed)
[ "$(agg '{}')" -ne 0 ] && ok "пустой needs ⇒ отказ" || bad "пустой needs принят за «всё прошло»"
# (6) СОСТАВ: каждый джоб файла, кроме агрегата и наблюдателей, стоит в needs агрегата
python3 - "$CI" >"$TMP/cov" 2>&1 <<'PY'
import re, sys
s = open(sys.argv[1], encoding="utf-8").read()
jobs = re.findall(r"^  ([a-z0-9-]+):\s*$", s.split("\njobs:\n", 1)[1], re.M)
needs = re.search(r"status-check:.*?needs: \[([^\]]*)\]", s, re.S).group(1).replace(" ", "").split(",")
# НАБЛЮДАТЕЛИ — merge не держат по замыслу (`docs/workflow/reading-map.md` §2, Ярус S).
OBSERVERS = {"branch-health"}
missing = sorted(set(jobs) - set(needs) - {"status-check"} - OBSERVERS)
if len(jobs) < 5: print("SETUP: джобов найдено", len(jobs)); sys.exit(2)
print("missing:", missing); sys.exit(1 if missing else 0)
PY
rc=$?
if [ $rc -eq 0 ]; then ok "все джобы ci.yml, кроме наблюдателей, — в needs агрегата"
elif [ $rc -eq 2 ]; then bad "SETUP: джобы ci.yml не разобраны — $(cat "$TMP/cov")"
else bad "джоб вне needs агрегата — его провал merge не держит: $(cat "$TMP/cov")"; fi

printf '\n'
[ "$BAD" -eq 0 ] && { echo "VERDICT: PASS — сценариев: $N, расхождений: 0"; exit 0; }
echo "VERDICT: FAIL — сценариев: $N, расхождений: $BAD"; exit 1
