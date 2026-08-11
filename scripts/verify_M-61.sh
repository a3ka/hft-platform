#!/usr/bin/env bash
# Acceptance-гейт M-61 — номер артефакта выдаётся механизмом, а не памятью (`TD-111`).
#
# Решение по КОДУ ВОЗВРАТА (`gates.md` §3). Агрегатор со счётчиком: печатаем все нарушения,
# exit 1 при FAIL>0 — первый красный шаг не должен скрывать остальные.
#
# ТРИ УРОКА ВСТРОЕНЫ, каждый оплачен ложным вердиктом в этой линии работ:
#  1. Число исполненного СЧИТАЕТСЯ, а не заявляется: `test result: ok. 0 passed` и
#     `VERDICT: PASS (0/0)` — зелёные строки, не исполнившие ничего.
#  2. «Проба зелёная» ≠ «проба ловит»: шаг F2 гоняет батарею мутантов.
#  3. Ожидаемых ЧИСЕЛ в гейте нет: эталон считается ВТОРЫМ путём в самом шаге
#     (спека §6.1 — «скрипт печатает M-61» стало ложью в момент push'а ветки).

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1
FAILED=0
# Логи прогона — в СВОЁМ каталоге, а не по фиксированным путям /tmp. Причина поведенческая:
# при одновременных прогонах (сегодня их было пять) соседний процесс переписывает файл между
# запуском и разбором, и пруф в Done Block принадлежит чужому прогону. Шаги F/F2/P решают по
# КОДУ ВОЗВРАТА и потому безопасны, но шаг T СЧИТАЕТ число исполненных тестов ИЗ ФАЙЛА —
# и переворачивался с красного на зелёное: прогон-пустышка (0 тестов) в одиночку давал FAIL,
# а одновременно с соседом — «PASS T cargo test --all: passed=77». Побеждён ровно тот
# анти-плацебо-страж, ради которого шаг T написан (урок 1 в шапке этого файла).
LOGD="$(mktemp -d /tmp/m61-verify-XXXXXX)" || { echo "не создан каталог логов" >&2; exit 1; }
trap 'rm -rf "${LOGD}"' EXIT
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

BARRIER=scripts/check_artifact_ids.sh
ALLOC=scripts/next_artifact_id.sh
PROBE=scripts/tests/red_artifact_ids.sh
CI=.github/workflows/ci.yml

echo "--- A: барьер и аллокатор на месте, парсятся, fail-closed ---"
for f in "${BARRIER}" "${ALLOC}"; do
  if [ -f "$f" ] && bash -n "$f" 2>/dev/null; then pass "A $f на месте и парсится"
  else fail "A $f отсутствует или не парсится"; fi
done
if [ -f "${BARRIER}" ]; then
  if ( EVENT_NAME=push PUSH_BEFORE="" PR_BASE_SHA="" bash "${BARRIER}" >/dev/null 2>&1 ); then
    fail "A барьер ПРОПУСТИЛ пустую базу — не fail-closed"
  else pass "A пустая база: fail-closed"; fi
fi

echo "--- N: аллокатор совпадает с НЕЗАВИСИМО вычисленным максимумом (§6.1) ---"
# Ожидаемых чисел здесь НЕТ намеренно. Эталон считается вторым путём — прямым
# перечислением ref'ов, — а не берётся из константы и не из проверяемого скрипта.
if [ -f "${ALLOC}" ]; then
  # TD в перечне обязателен: этот класс живёт ЗАПИСЬЮ в `TECH-DEBT.md`, а не именем файла, и
  # обслуживается ОТДЕЛЬНОЙ веткой кода аллокатора. Пока шаг гонял только M/R/C/A, дефект,
  # внесённый в TD-ветку, не ловился ничем — ни здесь, ни пробой (адверсарная проверка круга 4).
  for CLS in M R C A TD; do
    if [ "${CLS}" = TD ]; then
      exp_max=$(for r in $(git for-each-ref --format='%(refname)' refs/remotes/origin refs/heads 2>/dev/null); do
                  git show "$r:TECH-DEBT.md" 2>/dev/null | grep -oE 'TD-[0-9]+' | grep -oE '[0-9]+'
                done | sed 's/^0*//' | sort -n | tail -1)
    else
    exp_max=$(for r in $(git for-each-ref --format='%(refname)' refs/remotes/origin refs/heads 2>/dev/null); do
                git ls-tree -r --name-only "$r" 2>/dev/null | grep -oE "(^|/)${CLS}-[0-9]+" | grep -oE '[0-9]+'
              done | sed 's/^0*//' | sort -n | tail -1)
    fi
    if [ -z "${exp_max}" ]; then
      fail "N SETUP НЕ СОСТОЯЛСЯ: класс ${CLS} не найден ни в одном ref'е — сравнивать не с чем"
      continue
    fi
    case "${CLS}" in M) want=$(printf 'M-%02d' $((exp_max + 1)));; *) want=$(printf '%s-%03d' "${CLS}" $((exp_max + 1)));; esac
    got="$(bash "${ALLOC}" "${CLS}" 2>/dev/null)"
    if [ "${got}" = "${want}" ]; then pass "N ${CLS}: ${got} (независимый максимум ${exp_max})"
    else fail "N ${CLS}: аллокатор дал '${got}' при независимо вычисленном '${want}'"; fi
  done
else
  fail "N аллокатора нет — сравнивать нечего"
fi

echo "--- F: RED-проба (состав сверяет сама проба — с исполнением и со спекой §4.2) ---"
if bash "${PROBE}" >${LOGD}/probe.log 2>&1; then
  pass "F проба зелёная: $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' ${LOGD}/probe.log | head -1)"
else
  fail "F проба КРАСНАЯ — $(grep -E '^(VERDICT|SETUP)' ${LOGD}/probe.log | head -1)"
  grep -E '^(FAIL|SETUP)' ${LOGD}/probe.log | head -8 | sed 's/^/      ↳ /'
fi

echo "--- F2: АНТИ-ПЛАЦЕБО — батарея эталон + мутанты (спека §4.5) ---"
if bash "${PROBE}" --battery >${LOGD}/battery.log 2>&1; then
  pass "F2 $(grep -oE 'BATTERY: PASS \([0-9]+/[0-9]+\)' ${LOGD}/battery.log | head -1)"
else
  fail "F2 батарея КРАСНАЯ — $(grep -E '^BATTERY' ${LOGD}/battery.log | head -1)"
  grep -E '^(FAIL|SETUP)' ${LOGD}/battery.log | head -8 | sed 's/^/      ↳ /'
fi

echo "--- S: САМОРЕФЕРЕНЦИЯ + негативный контроль ---"
# Без второй половины «зелёный барьер» не отличим от пропускающего всё — урок R-041 F-3,
# где шаг S печатал PASS на диапазоне, где судить было нечего.
if [ -f "${BARRIER}" ]; then
  BASE=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
  ART_N=0
  if [ -n "${BASE}" ]; then
    ART_N=$(git diff --name-only --diff-filter=AR "${BASE}..HEAD" 2>/dev/null \
            | grep -cE '^(milestones/M|research/(reviews/R|critiques/C|arbitration/A))-[0-9]+')
  fi
  if [ -z "${BASE}" ]; then
    fail "S база не установлена (origin/main недоступен)"
  elif [ "${ART_N}" -eq 0 ]; then
    fail "S SETUP НЕ СОСТОЯЛСЯ: в диапазоне ${BASE}..HEAD нет ни одного нового артефакта — судить не о чем"
  elif EVENT_NAME=push PUSH_BEFORE="${BASE}" PR_BASE_SHA="${BASE}" bash "${BARRIER}" >/dev/null 2>&1; then
    pass "S собственный диапазон проходит барьер (${ART_N} артефактов введено)"
    # негативный контроль: тот же диапазон + синтетический дубль обязан краснеть
    T="$(mktemp -d /tmp/m61-selfneg-XXXXXX)"
    if git clone -q --no-hardlinks . "$T" 2>/dev/null; then
      EXIST=$(git ls-tree -r --name-only HEAD | grep -oE 'research/critiques/C-[0-9]+' | head -1)
      if [ -n "${EXIST}" ]; then
        NUM="${EXIST##*/}"
        ( cd "$T" && printf '# дубль\n\nтело\n' > "research/critiques/${NUM}-совсем-другой-предмет.md" \
          && git add -A && git commit -q -m "синтетический дубль номера" ) || true
        if ( cd "$T" && EVENT_NAME=push PUSH_BEFORE="${BASE}" PR_BASE_SHA="${BASE}" bash "${ROOT}/${BARRIER}" >/dev/null 2>&1 ); then
          fail "S негативный контроль: синтетический дубль ${NUM} ПРОШЁЛ — барьер пропускает всё"
        else pass "S негативный контроль: синтетический дубль ${NUM} заблокирован"; fi
      else fail "S негативный контроль невозможен: в дереве нет ни одного C-артефакта"; fi
    else fail "S негативный контроль: клон не удался"; fi
    rm -rf "$T"
  else
    fail "S собственный диапазон НЕ проходит барьер"
  fi
else
  fail "S барьера нет"
fi

echo "--- W: ПРЕДУСЛОВИЯ блокировки; предусловие 3 — ИСПОЛНЕНИЕМ (образец M-60a §6.1) ---"
if [ ! -f "${CI}" ]; then fail "W ${CI} отсутствует"; else
  python3 - "$CI" <<'PYW' || FAILED=$((FAILED + 1))
import os, re, subprocess, sys, tempfile
try:
    import yaml
except Exception:
    print("FAIL  W PyYAML недоступен. Предусловие 3 проверяется ИСПОЛНЕНИЕМ; суррогатный")
    print("      ↳ разбор регуляркой уже дал ложный PASS на фикстуре C-067 — не подменяем.")
    sys.exit(1)
LOCK = ('scripts/check_artifact_ids.sh', 'scripts/tests/red_artifact_ids.sh')
GUARD = 'status-check'
E_NEEDS = re.compile(r'\$\{\{\s*needs\.([A-Za-z0-9_-]+)\.result\s*\}\}')
E_ANY = re.compile(r'\$\{\{.*?\}\}', re.S)

def jobs_of(t):
    try: d = yaml.safe_load(t)
    except Exception: return {}
    j = d.get('jobs') if isinstance(d, dict) else None
    return j if isinstance(j, dict) else {}
def run_steps(j): return [s for s in (j.get('steps') or []) if isinstance(s, dict) and isinstance(s.get('run'), str)]
def owners(js, s): return sorted(n for n, j in js.items() if any(s in x['run'] for x in run_steps(j)))
def needs_of(j):
    n = j.get('needs')
    return [] if n is None else ([n] if isinstance(n, str) else [str(x) for x in n])
def evaluable(j):
    out = []
    for s in run_steps(j):
        if 'if' in s: continue
        sh = s.get('shell')
        if sh is not None and str(sh).strip() != 'bash': continue
        out.append(s['run'])
    return out
def subst(t, failing): return E_NEEDS.sub(lambda m: 'failure' if m.group(1) == failing else 'success', t)
def job_exit(bodies, failing, wd):
    for i, b in enumerate(bodies):
        src = subst(b, failing)
        if E_ANY.search(src): continue
        p = os.path.join(wd, 'step%d.sh' % i)
        open(p, 'w', encoding='utf-8').write(src)
        rc = subprocess.run(['bash', '-e', p], cwd=wd, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL).returncode
        if rc != 0: return rc
    return 0
def n_exec(bodies, failing): return sum(0 if E_ANY.search(subst(b, failing)) else 1 for b in bodies)
def preconditions(text, wd):
    js = jobs_of(text)
    per = dict((s, owners(js, s)) for s in LOCK)
    own = sorted(set(x for v in per.values() for x in v))
    p1 = all(per[s] for s in LOCK)
    g = js.get(GUARD)
    if g is None or not own:
        return p1, False, False, per, own, ('джоба %s нет' % GUARD if g is None else 'ни один джоб не исполняет скрипты')
    needs = needs_of(g); p2 = all(j in needs for j in own)
    bodies = evaluable(g); p3, why = True, []
    for j in own:
        if n_exec(bodies, j) == 0:
            p3 = False; why.append('%s: безусловных исполнимых run-шагов нет' % j); continue
        if job_exit(bodies, j, wd) == 0:
            p3 = False; why.append('%s: при result=failure guard вышел НУЛЁМ' % j)
        elif job_exit(bodies, None, wd) != 0:
            p3 = False; why.append('%s: при всех success guard вышел ненулём — блокирует всё подряд' % j)
    return p1, p2, p3, per, own, '; '.join(why)

FX = """
name: fx
on: [push]
jobs:
  artifact-ids:
    runs-on: ubuntu-latest
    steps:
      - run: bash scripts/check_artifact_ids.sh
      - run: bash scripts/tests/red_artifact_ids.sh
  status-check:
    runs-on: ubuntu-latest
    needs: [artifact-ids]
    if: always()
    steps:
      - run: |
          if [[ "${{ needs.artifact-ids.result }}" != "success" ]]; then
            echo fail; exit 1
          fi
"""
FX_ECHO = FX.replace("""      - run: |
          if [[ "${{ needs.artifact-ids.result }}" != "success" ]]; then
            echo fail; exit 1
          fi
""", """      - run: echo "${{ needs.artifact-ids.result }}"
""")
FX_DIS = FX.replace("""      - run: |
          if [[ "${{ needs.artifact-ids.result }}" != "success" ]]; then
            echo fail; exit 1
          fi
""", """      - if: ${{ false }}
        run: exit 1
      - run: echo "${{ needs.artifact-ids.result }}"
""")
FX_NN = FX.replace("    needs: [artifact-ids]", "    needs: [other]") + """
  other:
    runs-on: ubuntu-latest
    steps:
      - run: echo x
"""
FX_CONST = FX.replace("""      - run: |
          if [[ "${{ needs.artifact-ids.result }}" != "success" ]]; then
            echo fail; exit 1
          fi
""", """      - run: exit 1
""")
SELF = [('корректный', FX, (True, True, True)), ('echo-only', FX_ECHO, (True, True, False)),
        ('exit под if:false', FX_DIS, (True, True, False)), ('не в needs', FX_NN, (True, False, True)),
        ('безусловный exit 1', FX_CONST, (True, True, False))]
ok = True
with tempfile.TemporaryDirectory() as wd:
    bad = [l for l, t, e in SELF if preconditions(t, wd)[:3] != e]
    if bad:
        print("FAIL  W САМОПРОВЕРКА ОРАКУЛА не прошла (%s) — вердикту верить нельзя" % ', '.join(bad)); ok = False
    else:
        print("PASS  W самопроверка оракула: %d фикстур классифицированы верно" % len(SELF))
    p1, p2, p3, per, own, why = preconditions(open(sys.argv[1], encoding='utf-8').read(), wd)
    for s in LOCK:
        if per.get(s): print("PASS  W предусловие 1: %s ИСПОЛНЯЕТСЯ джобом(ами): %s" % (s, ', '.join(per[s])))
        else: print("FAIL  W предусловие 1: %s не встречается ни в одном `run:` — построен, но не подключён" % s)
    if not p1: ok = False
    if own:
        print(("PASS  W предусловие 2: %s в ключе %s.needs" if p2 else
               "FAIL  W предусловие 2: %s отсутствует в ключе %s.needs") % (', '.join(own), GUARD))
        if not p2: ok = False
        print("PASS  W предусловие 3 (ИСПОЛНЕНИЕМ): guard падает при failure и выходит нулём при success"
              if p3 else "FAIL  W предусловие 3 (ИСПОЛНЕНИЕМ): %s" % (why or 'guard не блокирует'))
        if not p3: ok = False
    else:
        print("FAIL  W предусловия 2-3 не проверить: %s" % why); ok = False
    print("      ↳ NB: три PASS не означают, что merge заблокирован — блокировку включает branch")
    print("        protection, на этом репо недоступный (403, private+free).")
sys.exit(0 if ok else 1)
PYW
fi

echo "--- W2: АДДИТИВНОСТЬ проводки относительно origin/main (R-046 Б-1) ---"
# Шаг W проверяет предусловия относительно базы ДИАПАЗОНА — и потому слеп к джобу, которого
# в базе не существовало. Замер reviewer'а: ветка отведена от main ДО M-60a, разрешение
# конфликта «версией ветки» удаляло docs-freeze целиком, а W давал 5×PASS, exit=0.
# Лечится сверкой с origin/main: ни один джоб, входящий в блокирующую проверку ТАМ, не смеет
# исчезнуть ЗДЕСЬ. Класс тот же, что C-073 F-5, применённый к джобу из чужого milestone'а.
if git rev-parse --verify -q origin/main >/dev/null; then
  python3 - <<'PYW2' || FAILED=$((FAILED + 1))
import subprocess, sys
try:
    import yaml
except Exception:
    print("FAIL  W2 PyYAML недоступен — аддитивность не проверить"); sys.exit(1)
def needs_of(text):
    d = yaml.safe_load(text) or {}
    sc = (d.get('jobs') or {}).get('status-check') or {}
    n = sc.get('needs')
    return set([] if n is None else ([n] if isinstance(n, str) else [str(x) for x in n]))
def guard_of(text):
    d = yaml.safe_load(text) or {}
    sc = (d.get('jobs') or {}).get('status-check') or {}
    return " ".join(s.get('run','') for s in (sc.get('steps') or []) if isinstance(s, dict))
here = open('.github/workflows/ci.yml', encoding='utf-8').read()
main = subprocess.run(['git','show','origin/main:.github/workflows/ci.yml'],
                      capture_output=True, text=True).stdout
if not main.strip():
    print("FAIL  W2 SETUP НЕ СОСТОЯЛСЯ: ci.yml из origin/main не прочитан — сравнивать не с чем")
    sys.exit(1)
lost = needs_of(main) - needs_of(here)
g = guard_of(here)
unguarded = sorted(j for j in needs_of(main) if j not in lost and j not in g)
ok = True
if lost:
    print("FAIL  W2 джобы ИСЧЕЗЛИ из status-check.needs относительно origin/main: %s" % ", ".join(sorted(lost)))
    print("      ↳ проводка не аддитивна: разрешение конфликта выбросило чужой барьер")
    ok = False
if unguarded:
    print("FAIL  W2 джобы в needs, но НЕ в сверке result: %s" % ", ".join(unguarded)); ok = False
if ok:
    print("PASS  W2 аддитивность: все %d джоба origin/main на месте и в сверке" % len(needs_of(main)))
sys.exit(0 if ok else 1)
PYW2
else
  fail "W2 origin/main недоступен — аддитивность не проверить"
fi

echo "--- G: НОРМА gates.md §12 — предмет задачи 6, а не подразумеваемое ---"
# R-052 Н-1 / A-006 §5: механизм опирается на норму — правило 1 `subject_id` читает шапку, и
# без нормы новые артефакты её не несут, барьер откатывается на слаг (эвристику, забракованную
# `C-069` F-2). Оракулом задачи 6 в §Tasks значился шаг S, но S проверяет БАРЬЕР на собственном
# диапазоне и о существовании нормы не знает ничего — то есть задача 6 гейта не имела вовсе.
# Здесь проверяются ровно объявленные свойства задачи: раздел есть, тело ≤8 строк, названы ОБЕ
# принимаемые формы шапки (`C-071` NOTE-1) и аллокатор назван поимённо.
RULES=.claude/rules/gates.md
if [ ! -f "${RULES}" ]; then
  fail "G ${RULES} отсутствует — норму проверять не в чем"
else
  G_SEC="$(sed -n '/^## 12\./,/^## /p' "${RULES}")"
  G_BODY="$(printf '%s\n' "${G_SEC}" | grep -v '^## ' | grep -c .)"
  if [ "${G_BODY}" -eq 0 ]; then
    fail "G раздела §12 в ${RULES} нет — задача 6 не сделана"
  elif [ "${G_BODY}" -gt 8 ]; then
    fail "G §12 занимает ${G_BODY} строк тела при пределе 8 (§Tasks, задача 6)"
  else
    pass "G §12 на месте, тело ${G_BODY} строк (предел 8)"
  fi
  # Формы шапки берутся ИЗ КОДА барьера, а не из литерала в гейте. Иначе шаг мерит не то, что
  # обещает: грep голого слова «Контекст» зеленел бы и для формы `**Контекст:**`, которую
  # парсер НЕ принимает (его альтернатива — `Контекст\*\*` / `Контекст\.\*\*`). Норма и код
  # обязаны называть ОДИН И ТОТ ЖЕ набор; расхождение — ложный красный на законной
  # множественности, то есть ровно то, что milestone обязан предотвращать.
  G_FORMS="$(grep -oE "\^\\\\\*\\\\\*\([^)]*\)" "${BARRIER}" | head -1 | sed -E 's/^[^(]*\(//; s/\)$//')"
  if [ -z "${G_FORMS}" ]; then
    fail "G SETUP НЕ СОСТОЯЛСЯ: из ${BARRIER} не извлечён перечень форм шапки — сверять нечего"
  else
    G_MISS=""
    IFS='|' read -r -a G_ARR <<< "${G_FORMS}"
    for form in "${G_ARR[@]}"; do
      form="${form//\\/}"
      [ -z "${form}" ] && continue
      printf '%s\n' "${G_SEC}" | grep -qF "${form}" || G_MISS="${G_MISS}«${form}» "
    done
    if [ -z "${G_MISS// /}" ]; then
      pass "G §12 называет ВСЕ формы шапки, принимаемые барьером: ${G_FORMS//\\/}"
    else
      fail "G §12 не называет форм, которые барьер ПРИНИМАЕТ: ${G_MISS}— норма разошлась с кодом"
    fi
  fi
  if printf '%s\n' "${G_SEC}" | grep -q 'next_artifact_id\.sh'; then
    pass "G §12 называет аллокатор поимённо"
  else
    fail "G §12 не называет scripts/next_artifact_id.sh — норма без механизма"
  fi
fi

echo "--- P: РЕГРЕСС — соседний барьер артефактов цел ---"
if bash scripts/tests/red_protected_artifacts.sh >${LOGD}/prot.log 2>&1; then
  pass "P $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' ${LOGD}/prot.log | head -1)"
else fail "P барьер артефактов сломан этим milestone'ом"; fi

echo "--- T: паритет с CI + НЕНУЛЕВОЕ число исполненных тестов ---"
cargo fmt --all -- --check >/dev/null 2>&1 && pass "T fmt" || fail "T fmt --check"
cargo clippy --workspace --all-targets --all-features -- -D warnings >${LOGD}/clippy.log 2>&1 \
  && pass "T clippy" || { fail "T clippy"; tail -5 ${LOGD}/clippy.log | sed 's/^/      ↳ /'; }
if cargo test --all >${LOGD}/test.log 2>&1; then
  # «0 passed» — зелёная строка, не исполнившая ничего: считаем, а не верим exit-коду.
  N=$(grep -E '^test result' ${LOGD}/test.log | awk '{p+=$4} END {print p+0}')
  if [ "${N:-0}" -gt 0 ]; then pass "T cargo test --all: passed=${N}"
  else fail "T cargo test --all вернул 0, но исполнил 0 тестов — прогон не состоялся"; fi
else fail "T cargo test --all"; grep -E '^test .* FAILED' ${LOGD}/test.log | head -5 | sed 's/^/      ↳ /'; fi

echo
if [ "${FAILED}" -gt 0 ]; then echo "VERDICT: FAIL (${FAILED} нарушений)"; exit 1; fi
echo "VERDICT: PASS"
