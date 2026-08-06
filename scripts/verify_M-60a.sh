#!/usr/bin/env bash
# Acceptance-гейт M-60a — замок процессного слоя.
#
# Решение по КОДУ ВОЗВРАТА (gates.md §3). Агрегатор со счётчиком: печатаем все нарушения,
# exit 1 при FAIL>0 — первый красный шаг не должен скрывать остальные.
#
# ДВА УРОКА 2026-08-05 ВСТРОЕНЫ ЗДЕСЬ, потому что оба стоили ложных вердиктов:
#  1. Число исполненных проверок СЧИТАЕТСЯ, а не заявляется. `VERDICT: PASS (0/0)` —
#     зелёная строка, не исполнившая ничего; так прошли пять пустых прогонов подряд.
#  2. Подключённость к CI проверяется РАЗБОРОМ workflow, а не грепом (`C-064` F-064-2):
#     имена скриптов, добавленные КОММЕНТАРИЕМ, удовлетворяют греп, и гейт «построен, но
#     не подключён» проходит. Проверяются: id джоба, реальная `run:`-команда внутри него,
#     и полное включение в блокирующий `status-check.needs`.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

BARRIER=scripts/check_docs_freeze.sh
PROBE=scripts/tests/red_docs_freeze.sh
CI=.github/workflows/ci.yml

echo "--- A: барьер существует, исполним, парсится, fail-closed на пустой базе ---"
if [ -f "${BARRIER}" ] && bash -n "${BARRIER}" 2>/dev/null; then
  pass "A ${BARRIER} на месте и парсится"
  # «База не установлена» обязана быть ОТКАЗОМ, а не пропуском: иначе force-push и
  # создание ветки обходят замок (блокер B1, C-006).
  if ( EVENT_NAME=push PUSH_BEFORE="" PR_BASE_SHA="" bash "${BARRIER}" >/dev/null 2>&1 ); then
    fail "A барьер ПРОПУСТИЛ пустую базу — не fail-closed"
  else
    pass "A пустая база: fail-closed"
  fi
else
  fail "A ${BARRIER} отсутствует или не парсится"
fi

echo "--- F: RED-проба замка (число сценариев СЧИТАЕТСЯ пробой) ---"
if bash "${PROBE}" >/tmp/m60a-probe.log 2>&1; then
  N=$(grep -oE 'VERDICT: PASS \(([0-9]+)/' /tmp/m60a-probe.log | grep -oE '[0-9]+' | head -1)
  if [ "${N:-0}" -ge 11 ]; then
    pass "F проба: ${N} сценариев зелёных"
  else
    fail "F проба сообщила ${N:-0} сценариев при ожидаемых ≥11 — покрытие усохло"
  fi
else
  fail "F проба КРАСНАЯ — $(grep -E '^(VERDICT|SETUP)' /tmp/m60a-probe.log | head -1)"
  grep -E '^FAIL' /tmp/m60a-probe.log | head -5 | sed 's/^/      ↳ /'
fi

echo "--- S: САМОРЕФЕРЕНЦИЯ — M-60a обязан пройти собственный замок ---"
# Механизм, не выполняющий сам себя, не готов (C-062 §d). Диапазон — вся ветка над main.
if [ -f "${BARRIER}" ]; then
  BASE=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
  if [ -z "${BASE}" ]; then
    fail "S не удалось установить базу (origin/main недоступен) — самореференцию не проверить"
  elif EVENT_NAME=push PUSH_BEFORE="${BASE}" PR_BASE_SHA="${BASE}" bash "${BARRIER}" >/dev/null 2>&1; then
    pass "S собственный диф проходит замок (токен founder'а на месте)"
  else
    fail "S собственный диф НЕ проходит замок — механизм не выполняет себя"
  fi
else
  fail "S замка нет — самореференцию проверять нечем"
fi

echo "--- W: ПОДКЛЮЧЁННОСТЬ к CI (разбором workflow, не грепом) ---"
if [ ! -f "${CI}" ]; then
  fail "W ${CI} отсутствует"
else
  python3 - "$CI" <<'PYW' || FAILED=$((FAILED + 1))
import re, sys
src = open(sys.argv[1], encoding='utf-8').read().split('\n')

# Разбор джобов. ИСПОЛНЕНИЕМ считается только тело `run:` — своё или блочное.
# Первая редакция принимала любую строку списка, поэтому `- name: scripts/check_x.sh`
# засчитывался как запуск (C-065, блокер 2). Имя шага — не запуск.
jobs, cur, in_jobs = {}, None, False
for line in src:
    if re.match(r'^jobs:\s*$', line):
        in_jobs = True; continue
    if in_jobs and re.match(r'^\S', line):
        in_jobs = False
    if not in_jobs:
        continue
    m = re.match(r'^  ([A-Za-z0-9_-]+):\s*$', line)
    if m:
        cur = m.group(1); jobs[cur] = []
    elif cur is not None:
        jobs[cur].append(line)

def run_lines(body):
    out, block = [], None
    for l in body:
        if block is not None:
            if l.strip() == '' or (len(l) - len(l.lstrip())) > block:
                out.append(l); continue
            block = None
        m = re.match(r'^(\s*)(?:-\s+)?run:\s*[|>]\s*$', l)
        if m:
            block = len(m.group(1)); continue
        m2 = re.match(r'^\s*(?:-\s+)?run:\s*(.+)$', l)
        if m2:
            out.append(m2.group(1))
    return out

def job_running(sub):
    return [j for j, b in jobs.items() if any(sub in l for l in run_lines(b))]

ok = True
for what in ('scripts/check_docs_freeze.sh', 'scripts/tests/red_docs_freeze.sh'):
    owners = job_running(what)
    if owners:
        print("PASS  W %s ИСПОЛНЯЕТСЯ джобом(ами): %s" % (what, ', '.join(owners)))
    else:
        print("FAIL  W %s не встречается ни в одном `run:` — построен, но не подключён" % what)
        ok = False

# Блокирует ли красное merge. Членства в `needs` НЕДОСТАТОЧНО: status-check стоит с
# `if: always()`, исполняется всегда и падает только если ЯВНО сверяет
# `needs.<job>.result` внутри своего `run:` (C-065, блокер 2, вторая половина).
sc = jobs.get('status-check')
needed = sorted(set(job_running('scripts/check_docs_freeze.sh')) |
                set(job_running('scripts/tests/red_docs_freeze.sh')))
if sc is None:
    print("FAIL  W джоба status-check нет — блокировать merge нечем"); ok = False
elif not needed:
    print("FAIL  W некого включать в блокирующую проверку — джоба замка нет"); ok = False
else:
    needs_decl = ' '.join(l for l in sc if 'needs' in l)
    guard = ' '.join(run_lines(sc))
    always = any(re.search(r'if:\s*always\(\)', l) for l in sc)
    for j in needed:
        in_needs = j in needs_decl
        in_guard = re.search(r'needs\.' + re.escape(j) + r'\.result', guard) is not None
        if in_needs and in_guard:
            print("PASS  W %s: в needs И в сверке needs.%s.result — красное блокирует merge" % (j, j))
        elif in_needs and always and not in_guard:
            print("FAIL  W %s в needs, но status-check с `if: always()` НЕ сверяет needs.%s.result "
                  "— красное НЕ блокирует merge" % (j, j)); ok = False
        elif not in_needs:
            print("FAIL  W %s отсутствует в status-check.needs" % j); ok = False
        else:
            print("FAIL  W %s: нет сверки needs.%s.result" % (j, j)); ok = False
sys.exit(0 if ok else 1)
PYW
fi

echo "--- P: РЕГРЕСС — соседний барьер артефактов цел ---"
if bash scripts/tests/red_protected_artifacts.sh >/tmp/m60a-prot.log 2>&1; then
  pass "P $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' /tmp/m60a-prot.log | head -1)"
else
  fail "P барьер артефактов сломан этим milestone'ом — цена замка уплачена соседним инвариантом"
fi

echo "--- T: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
cargo fmt --all -- --check >/dev/null 2>&1 && pass "T fmt" || fail "T fmt --check"
cargo clippy --workspace --all-targets --all-features -- -D warnings >/tmp/m60a-clippy.log 2>&1 \
  && pass "T clippy" || { fail "T clippy"; tail -5 /tmp/m60a-clippy.log | sed 's/^/      ↳ /'; }
cargo test --all >/tmp/m60a-test.log 2>&1 \
  && pass "T cargo test --all" || { fail "T cargo test --all"; grep -E '^test .* FAILED' /tmp/m60a-test.log | head -5 | sed 's/^/      ↳ /'; }

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений)"
  exit 1
fi
echo "VERDICT: PASS"
