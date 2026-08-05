#!/usr/bin/env bash
# Acceptance-гейт M-59 — граница ПАМЯТИ per-life анализатора (долг TD-107).
#
# Решение принимается по КОДУ ВОЗВРАТА (gates.md §3). Агрегатор со счётчиком: печатаем все
# нарушения разом, exit 1 при FAIL>0 — иначе первый красный шаг скрыл бы остальные.
#
# ВСТРОЕННЫЙ УРОК 2026-08-05. Проверка вида `grep -q 'test result: ok'` даёт ЗЕЛЁНЫЙ на
# строке `test result: ok. 0 passed; 0 failed; N filtered out` — то есть когда фильтр не
# совпал НИ С ЧЕМ и не исполнено ничего. В этот день так прошли пять «зелёных» прогонов
# мутационного контроля подряд, не выполнив ни одного теста. Поэтому здесь ни один шаг не
# смотрит на слово `ok`: `run_tests` СЧИТАЕТ исполненные тесты и валит шаг, если их меньше
# ожидаемого. Замер по репозиторию в тот же день: 28 из 40 проверок «test result» в наших
# гейтах этого счётчика не имели.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

# run_tests <лог> <мин-число-исполненных> <описание> -- <команда...>
# Зелёный ⇔ (а) команда вернула 0, (б) исполнено НЕ МЕНЬШЕ ожидаемого, (в) ни одного failed.
run_tests() {
  local log="$1" min="$2" what="$3"; shift 4
  "$@" >"${log}" 2>&1
  local rc=$? p f
  p=$(grep -hoE '^test result: [a-zA-Z]+\. [0-9]+ passed' "${log}" | awk '{s+=$4} END{print s+0}')
  f=$(grep -hoE '[0-9]+ failed' "${log}" | awk '{s+=$1} END{print s+0}')
  if [ "${rc}" -ne 0 ] || [ "${f}" -ne 0 ]; then
    fail "${what} — исполнено ${p}, упало ${f}, exit=${rc}"
    grep -E '^(test .* FAILED|thread .* panicked|DV-I-|error)' "${log}" | head -6 | sed 's/^/      ↳ /'
    return 1
  fi
  if [ "${p}" -lt "${min}" ]; then
    fail "${what} — исполнено ${p} тестов при ожидаемых ≥${min}: фильтр не совпал, прогон НЕДЕЙСТВИТЕЛЕН"
    return 1
  fi
  pass "${what} — исполнено ${p}, упало 0"
  return 0
}

echo "--- T0: оракул на месте (sacred, architect-only) ---"
ORACLE=crates/research-cli/tests/red_lifetime_memory_bounded.rs
if [ -f "${ORACLE}" ] && grep -q 'fn dv_i_15_lifetime_memory_bounded' "${ORACLE}"; then
  pass "T0 ${ORACLE}"
else
  fail "T0 ${ORACLE} отсутствует или не содержит DV-I-15"
fi
# Ровно один тест в файле — иначе замер глобального счётчика недействителен (см. шапку оракула).
NT=$(grep -c '^#\[test\]' "${ORACLE}" 2>/dev/null || echo 0)
[ "${NT}" -eq 1 ] && pass "T0 ровно один #[test] в файле (изоляция замера аллокаций)" \
                  || fail "T0 в файле ${NT} тестов — параллельный прогон испортит счётчик памяти"

echo "--- T1/T2/T2b: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
cargo build --workspace >/dev/null 2>&1 && pass "T1 build --workspace" || fail "T1 build --workspace"
cargo clippy --workspace --all-targets --all-features -- -D warnings >/tmp/m59-clippy.log 2>&1 \
  && pass "T2 clippy" || { fail "T2 clippy"; tail -5 /tmp/m59-clippy.log | sed 's/^/      ↳ /'; }
cargo fmt --all -- --check >/dev/null 2>&1 && pass "T2b fmt --check" || fail "T2b fmt --check"

echo "--- T3: ГЛАВНОЕ — DV-I-15, память не растёт с числом ЖИЗНЕЙ ---"
run_tests /tmp/m59-dv15.log 1 "T3 DV-I-15" -- \
  cargo test -p research-cli --test red_lifetime_memory_bounded -- --nocapture
grep -hoE 'DV-I-15: .*' /tmp/m59-dv15.log | head -1 | sed 's/^/      ЗАМЕР: /'

echo "--- T4: РЕГРЕСС — DV-I-1..14 остаются зелёными ---"
run_tests /tmp/m59-perlife.log 5 "T4 per-life оракулы (DV-I-10..14)" -- \
  cargo test -p research-cli --test red_depth_lifetime_perlife
run_tests /tmp/m59-lifetime.log 9 "T4 базовые оракулы (DV-I-1..9)" -- \
  cargo test -p research-cli --test red_depth_lifetime

echo "--- T5: ЧИСЛА прогона не поехали (публичный контракт и артефакт замера) ---"
# Фикс обязан менять РАСХОД, а не РЕЗУЛЬТАТ. Артефакт под founder-решением П-011 —
# research/data-quality/m58-rerun-segment78.txt; расхождение чисел = смена семантики.
ART=research/data-quality/m58-rerun-segment78.txt
if [ ! -f "${ART}" ]; then
  fail "T5 артефакт замера ${ART} отсутствует — сверять не с чем"
elif [ -z "${M59_JOURNAL:-}" ]; then
  # Fail-closed: «нет журнала» не значит «проверять нечего». Пересъёмка требует
  # M59_JOURNAL=<путь к копии сегмента 78>; молчаливый пропуск запрещён.
  fail "T5 пересъёмка НЕ выполнена: задай M59_JOURNAL=<путь к журналу segment 78>. \
Пропуск этой проверки означал бы, что фикс мог изменить ЧИСЛА, а не только расход"
else
  cargo run --release -p research-cli --example depth_lifetime -- "${M59_JOURNAL}" \
    >/tmp/m59-rerun.txt 2>/dev/null
  if diff <(grep -oE '[0-9]+' "${ART}" | tr '\n' ' ') \
          <(grep -oE '[0-9]+' /tmp/m59-rerun.txt | tr '\n' ' ') >/dev/null 2>&1; then
    pass "T5 числа пересъёмки идентичны артефакту (изменился расход, не результат)"
  else
    fail "T5 ЧИСЛА РАЗОШЛИСЬ с ${ART} — фикс изменил семантику, а не только память"
    diff <(grep -oE '[0-9]+' "${ART}") <(grep -oE '[0-9]+' /tmp/m59-rerun.txt) | head -6 | sed 's/^/      ↳ /'
  fi
fi

echo "--- T6: публичный контракт не тронут ---"
SRC=crates/research-cli/src/depth_lifetime.rs
MISS=0
for f in lives_born lives_cancelled lives_frozen lives_censored; do
  grep -q "pub ${f}: u64" "${SRC}" || { echo "      ↳ пропало поле ${f}"; MISS=$((MISS+1)); }
done
[ "${MISS}" -eq 0 ] && pass "T6 поля BandReport.lives_* на месте" \
                    || fail "T6 публичный контракт изменён: пропало ${MISS} поле(й)"
grep -q 'crates/contracts' <(git diff --name-only origin/main...HEAD 2>/dev/null) \
  && fail "T6 затронут crates/contracts — M-59 не T1-milestone" \
  || pass "T6 crates/contracts не тронут"

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений)"
  exit 1
fi
echo "VERDICT: PASS"
