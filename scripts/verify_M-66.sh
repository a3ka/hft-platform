#!/usr/bin/env bash
# Acceptance-гейт M-66 — protocol attestation через живое FA-эхо в reviewer verdict.
#
# ОЖИДАЕМОЕ СОСТОЯНИЕ СЕЙЧАС — КРАСНОЕ:
#  - задача 2 ещё не сделана: `scripts/check_review_fa.sh` отсутствует;
#  - задача 3 ещё не сделана: job `review-fa` не подключён в `.github/workflows/ci.yml`;
#  - задачи 5/6/7 не входят в этот architect-заход и остаются отдельными стадиями.
# Это НЕ надо «чинить» зелёным обходом: RED-first требует видимого `VERDICT: FAIL`.
#
# Замер обязательной CI-тройки на этом дереве перед созданием файла:
#  - `cargo test --all` завершился за 24:54.68 (`1494.68` секунд), exit=0.
# Терминальность гейта: CI-команды ниже идут через fail-closed `timeout`; для
# `cargo test --all` запас по умолчанию 2700s больше замера и всё равно гарантирует
# финальную строку `VERDICT: PASS|FAIL`.
#
# Включено из CI-паритета (`gates.md` §3):
#  - `cargo fmt --all -- --check`;
#  - `cargo clippy --all-targets --all-features -- -D warnings`;
#  - `cargo test --all`;
#  - специализированная зона M-66: RED-проба, будущий барьер, wiring job `review-fa`,
#    исторические H1/H2 и мутационная батарея.
#
# НЕ включено из прочих CI jobs:
#  - `cargo audit`: идёт в CI на том же push'е, зона милестоуном не тронута.
#  - `bash scripts/verify_delivery_M-08.sh`: идёт в CI на том же push'е, зона милестоуном не тронута.
#  - `scripts/check_protected_artifacts.sh` + его RED-проба: идёт в CI на том же push'е,
#    зона милестоуном не тронута.
#  - `scripts/check_docs_freeze.sh` + его RED-проба: идёт в CI на том же push'е,
#    зона милестоуном не тронута.
#  - contracts/schema/CT-RFC/diff-contract gates: идёт в CI на том же push'е,
#    зона милестоуном не тронута.
#  - `scripts/check_artifact_ids.sh` + его RED-проба: идёт в CI на том же push'е,
#    зона милестоуном не тронута.
#  - `scripts/verify_design_claims.sh` + его RED-проба: идёт в CI на том же push'е,
#    зона милестоуном не тронута.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
PASSED=0
M66_CARGO_FMT_TIMEOUT="${M66_CARGO_FMT_TIMEOUT:-300s}"
M66_CARGO_CLIPPY_TIMEOUT="${M66_CARGO_CLIPPY_TIMEOUT:-1200s}"
M66_CARGO_TEST_TIMEOUT="${M66_CARGO_TEST_TIMEOUT:-2700s}"
LOGD="$(mktemp -d /tmp/m66-verify-XXXXXX)" || {
  echo "FAIL  SETUP: не создан каталог логов" >&2
  echo "VERDICT: FAIL"
  exit 1
}
WORK_REG="${LOGD}/worktrees.txt"
: >"${WORK_REG}"
trap 'while IFS= read -r d; do [ -n "$d" ] && [ -d "$d" ] && rm -rf "$d"; done < "${WORK_REG}"; rm -rf "${LOGD}"' EXIT

pass() { echo "PASS  $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

run_logged() {
  local id="$1" label="$2"; shift 2
  local log="${LOGD}/${id}.log" rc pid waited
  echo "--- ${label} ---"
  "$@" >"${log}" 2>&1 &
  pid=$!
  waited=0
  while kill -0 "$pid" 2>/dev/null; do
    sleep 30
    waited=$((waited + 30))
    if kill -0 "$pid" 2>/dev/null; then
      echo "NOTE  ${label}: still running (${waited}s)"
    fi
  done
  wait "$pid"
  rc=$?
  if [ "$rc" -eq 0 ]; then
    pass "${label} (exit=0)"
  else
    fail "${label} (exit=${rc})"
    sed 's/^/      ↳ /' "${log}" | tail -12
  fi
}

branch_base() {
  git merge-base origin/main HEAD 2>/dev/null || git rev-parse HEAD^ 2>/dev/null || true
}

check_review_fa_wiring() {
  python3 - .github/workflows/ci.yml <<'PY'
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
if not path.exists():
    print("workflow missing")
    sys.exit(1)
# Иглы ищутся ТОЛЬКО по исполняемым строкам: строка-комментарий YAML выбрасывается целиком.
# `R-081` §5 N-5, мутант D: игла `fetch-depth: 0` живёт и в комментарии джоба `review-fa`,
# поэтому снятие самих строк `with:`/`fetch-depth: 0` давало exit=0 при джобе, идущем с
# depth=1. Проверка по ТЕКСТУ блока ловила комментарий — класс `testing.md` §«Механизм
# несущего пути»: «grep по имени ловит и лог-строки».
text = "\n".join(
    line for line in path.read_text(encoding="utf-8").splitlines()
    if not line.lstrip().startswith("#")
)
jobs = {}
current = None
buf = []
for line in text.splitlines():
    m = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
    if m:
        if current is not None:
            jobs[current] = "\n".join(buf)
        current = m.group(1)
        buf = [line]
    elif current is not None:
        buf.append(line)
if current is not None:
    jobs[current] = "\n".join(buf)

errors = []
review = jobs.get("review-fa", "")
status = jobs.get("status-check", "")
if not review:
    errors.append("job review-fa отсутствует")
else:
    for needle in (
        "fetch-depth: 0",
        "scripts/check_review_fa.sh",
        "scripts/tests/red_review_fa.sh",
        "EVENT_NAME",
        "PUSH_BEFORE",
        "PR_BASE_SHA",
    ):
        if needle not in review:
            errors.append(f"review-fa не содержит {needle}")
if not status:
    errors.append("status-check отсутствует")
else:
    if not re.search(r"needs\s*:\s*(?:\[[^\]]*\breview-fa\b[^\]]*\]|[\s\S]*?-\s*review-fa\b)", status):
        errors.append("status-check.needs не содержит review-fa")
    if "needs.review-fa.result" not in status:
        errors.append("status-check не проверяет needs.review-fa.result")
for e in errors:
    print(e)
sys.exit(1 if errors else 0)
PY
}

check_historical() {
  local name="$1" sha="$2" expect="$3" desc="$4" t base rc log
  log="${LOGD}/${name}.log"
  if [ ! -f scripts/check_review_fa.sh ]; then
    fail "${name} ${desc}: scripts/check_review_fa.sh отсутствует"
    return
  fi
  if ! git cat-file -e "${sha}^{commit}" 2>/dev/null; then
    fail "${name} ${desc}: исторический SHA ${sha} отсутствует"
    return
  fi
  t="$(mktemp -d "/tmp/m66-${name}-XXXXXX")" || { fail "${name}: mktemp"; return; }
  printf '%s\n' "$t" >> "${WORK_REG}"
  if ! git clone -q --no-hardlinks "${ROOT}" "$t" >"${log}" 2>&1; then
    fail "${name} ${desc}: clone не удался"
    sed 's/^/      ↳ /' "${log}" | tail -8
    return
  fi
  if ! ( cd "$t" && git checkout -q "$sha" ) >>"${log}" 2>&1; then
    fail "${name} ${desc}: checkout ${sha} не удался"
    sed 's/^/      ↳ /' "${log}" | tail -8
    return
  fi
  base="$(cd "$t" && git rev-parse "${sha}^1")" || { fail "${name}: base ${sha}^1 не читается"; return; }
  ( cd "$t" && EVENT_NAME=push PUSH_BEFORE="$base" PR_BASE_SHA="" bash "${ROOT}/scripts/check_review_fa.sh" ) >>"${log}" 2>&1
  rc=$?
  case "$expect" in
    PASS)
      if [ "$rc" -eq 0 ]; then pass "${name} ${desc} (exit=0)"
      else fail "${name} ${desc}: ожидался PASS, exit=${rc}"; sed 's/^/      ↳ /' "${log}" | tail -10; fi
      ;;
    FAIL)
      if [ "$rc" -ne 0 ]; then pass "${name} ${desc} (ожидаемый FAIL, exit=${rc})"
      else fail "${name} ${desc}: ожидался FAIL, но exit=0"; sed 's/^/      ↳ /' "${log}" | tail -10; fi
      ;;
    *) fail "${name}: неизвестное ожидание ${expect}" ;;
  esac
}

echo "--- T1: RED-проба architect'а существует и не зеленеет без барьера ---"
if [ -f scripts/tests/red_review_fa.sh ] && bash -n scripts/tests/red_review_fa.sh 2>"${LOGD}/red-n.log"; then
  pass "T1 scripts/tests/red_review_fa.sh существует и парсится"
else
  fail "T1 scripts/tests/red_review_fa.sh отсутствует или не парсится"
  sed 's/^/      ↳ /' "${LOGD}/red-n.log" | tail -8
fi
if [ ! -f scripts/check_review_fa.sh ]; then
  bash scripts/tests/red_review_fa.sh >"${LOGD}/red-missing.log" 2>&1
  rc=$?
  if [ "$rc" -ne 0 ] && grep -q "SETUP НЕ СОСТОЯЛСЯ: барьера нет" "${LOGD}/red-missing.log"; then
    pass "T1 missing-barrier guard сработал (exit=${rc})"
  else
    fail "T1 проба не отличила отсутствие барьера от сценарного FAIL (exit=${rc})"
    sed 's/^/      ↳ /' "${LOGD}/red-missing.log" | tail -8
  fi
fi

echo "--- T2: dev-задача 2 — барьер scripts/check_review_fa.sh ---"
if [ -f scripts/check_review_fa.sh ] && bash -n scripts/check_review_fa.sh 2>"${LOGD}/barrier-n.log"; then
  pass "T2 scripts/check_review_fa.sh существует и парсится"
  run_logged "t2-red-probe" "T2 RED-проба против реального барьера" bash scripts/tests/red_review_fa.sh
else
  fail "T2 scripts/check_review_fa.sh отсутствует или не парсится — задача 2 OPEN"
  [ -s "${LOGD}/barrier-n.log" ] && sed 's/^/      ↳ /' "${LOGD}/barrier-n.log" | tail -8
fi

echo "--- T3: dev-задача 3 — CI job review-fa и status-check wiring ---"
if check_review_fa_wiring >"${LOGD}/wiring.log" 2>&1; then
  pass "T3 review-fa job подключён к CI и status-check"
else
  fail "T3 review-fa job / status-check wiring не готов — задача 3 OPEN"
  sed 's/^/      ↳ /' "${LOGD}/wiring.log" | tail -10
fi

echo "--- T4: verify-задача — H1/H2 + мутационная батарея ---"
if [ -f scripts/verify_M-66.sh ] && bash -n scripts/verify_M-66.sh 2>"${LOGD}/self-n.log"; then
  pass "T4 scripts/verify_M-66.sh существует и парсится"
else
  fail "T4 scripts/verify_M-66.sh отсутствует или не парсится"
  sed 's/^/      ↳ /' "${LOGD}/self-n.log" | tail -8
fi
check_historical H1 d564617 FAIL "M-62 обязан краснеть: journal+gateway без JR/VB echo"
check_historical H2 710b1ad PASS "M-57 обязан проходить: R-040 несёт JR-I-1/2/11"
run_logged "t4-battery" "T4 мутационная батарея red_review_fa.sh --battery" bash scripts/tests/red_review_fa.sh --battery

echo "--- T5: founder/process-lock задача 5 — profile attestation строка ---"
profiles_total="$(find .claude/agents -maxdepth 1 -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
profiles_with_startup="$(grep -l "Startup reading" .claude/agents/*.md 2>/dev/null | wc -l | tr -d ' ')"
profiles_with_attestation="$(grep -l "review-fa\\|живой инвариант-ID FA\\|FA-эхо" .claude/agents/*.md 2>/dev/null | wc -l | tr -d ' ')"
gates_review_fa_count="$(grep -c 'review-fa' .claude/rules/gates.md 2>/dev/null || true)"
if [ "$profiles_total" = "9" ] && [ "$profiles_with_startup" = "9" ]; then
  pass "T5 база профилей подтверждена: 9/9 имеют Startup reading"
else
  fail "T5 база профилей изменилась: Startup reading ${profiles_with_startup}/${profiles_total}"
fi
if [ "$profiles_with_attestation" = "9" ] && grep -q "review-fa" .claude/rules/gates.md 2>/dev/null; then
  pass "T5 founder-approved строка предъявления есть во всех 9 профилях и gates.md"
else
  fail "T5 founder-approved строка предъявления отсутствует (${profiles_with_attestation}/9 профилей; gates.md review-fa=${gates_review_fa_count}) — задача 5 ждёт founder"
fi

echo "--- T6: reviewer close-out задача 6 — TD-105 / aggregate-status debt ---"
BASE="$(branch_base)"
if [ -n "$BASE" ] && git diff --name-only "${BASE}..HEAD" | grep -q '^TECH-DEBT.md$'; then
  pass "T6 TECH-DEBT.md изменён в диапазоне M-66"
else
  fail "T6 TECH-DEBT.md не изменён в диапазоне M-66 — reviewer close-out задача 6 OPEN"
fi
if [ -n "$BASE" ] && git diff --name-only "${BASE}..HEAD" | grep -q '^PROJECT-STATE.md$'; then
  pass "T6 PROJECT-STATE.md изменён reviewer'ом"
else
  fail "T6 PROJECT-STATE.md не изменён reviewer'ом — close-out ещё не выполнен"
fi

echo "--- T7: follow-up задача 7 — FA для derive/recorder ---"
if [ -f docs/fa/derive.md ] && [ -f docs/fa/recorder.md ]; then
  pass "T7 docs/fa/derive.md и docs/fa/recorder.md существуют"
else
  fail "T7 docs/fa/derive.md и/или docs/fa/recorder.md отсутствуют — follow-up задача 7 OPEN"
fi

echo "--- G: guard задач 1-4 не должен лезть в замок и FA follow-up ---"
if [ -n "$BASE" ]; then
  locked="$(git diff --name-only "${BASE}..HEAD" | grep -E '^(\.claude/|CLAUDE\.md$|docs/04-workflow\.md$)' || true)"
  fa_diff="$(git diff --name-only "${BASE}..HEAD" | grep -E '^docs/fa/' || true)"
  if [ -z "$locked" ]; then pass "G locked process files не тронуты текущим диапазоном"
  else fail "G текущий диапазон трогает process-lock файлы"; printf '%s\n' "$locked" | sed 's/^/      ↳ /'; fi
  if [ -z "$fa_diff" ]; then pass "G docs/fa/** не тронуты в core-задачах 1-4"
  else fail "G текущий диапазон трогает docs/fa/**"; printf '%s\n' "$fa_diff" | sed 's/^/      ↳ /'; fi
else
  fail "G не удалось определить merge-base с origin/main"
fi

echo "--- CI parity: базовая тройка ---"
run_logged "cargo-fmt" "cargo fmt --all -- --check" \
  timeout --kill-after=30s "${M66_CARGO_FMT_TIMEOUT}" cargo fmt --all -- --check
run_logged "cargo-clippy" "cargo clippy --all-targets --all-features -- -D warnings" \
  timeout --kill-after=30s "${M66_CARGO_CLIPPY_TIMEOUT}" cargo clippy --all-targets --all-features -- -D warnings
run_logged "cargo-test" "cargo test --all" \
  timeout --kill-after=30s "${M66_CARGO_TEST_TIMEOUT}" cargo test --all

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL"
  exit 1
fi
echo "VERDICT: PASS"
exit 0
