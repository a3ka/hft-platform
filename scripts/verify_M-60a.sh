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
  if [ "${N:-0}" -ge 9 ]; then
    pass "F проба: ${N} сценариев зелёных"
  else
    fail "F проба сообщила ${N:-0} сценариев при ожидаемых ≥9 — покрытие усохло"
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
  python3 - "$CI" <<'PY' || FAILED=$((FAILED + 1))
import re, sys
src = open(sys.argv[1], encoding='utf-8').read().split('\n')
# джобы — ключи с отступом РОВНО 2 пробела внутри верхнеуровневого jobs:
jobs, cur = {}, None
in_jobs = False
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
    elif cur:
        jobs[cur].append(line)

def job_running(substr):
    return [j for j, body in jobs.items()
            if any(substr in l and re.search(r'run:|^\s+bash|^\s+-\s', l) for l in body)]

ok = True
for what in ('scripts/check_docs_freeze.sh', 'scripts/tests/red_docs_freeze.sh'):
    owners = job_running(what)
    if owners:
        print(f"PASS  W {what} исполняется джобом(ами): {', '.join(owners)}")
    else:
        print(f"FAIL  W {what} НЕ исполняется ни одним джобом — механизм построен, но не подключён")
        ok = False

# status-check.needs обязан содержать джоб(ы), исполняющие замок
needs_txt = ' '.join(jobs.get('status-check', []))
needed = set(job_running('scripts/check_docs_freeze.sh')) | set(job_running('scripts/tests/red_docs_freeze.sh'))
missing = [j for j in needed if j not in needs_txt]
if not needed:
    print("FAIL  W некого включать в status-check.needs — джоба замка нет")
    ok = False
elif missing:
    print(f"FAIL  W вне блокирующего status-check.needs: {', '.join(missing)} — красное не блокирует merge")
    ok = False
else:
    print(f"PASS  W все джобы замка в status-check.needs: {', '.join(sorted(needed))}")
sys.exit(0 if ok else 1)
PY
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
