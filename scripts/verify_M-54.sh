#!/usr/bin/env bash
# Acceptance-гейт M-54 — TD-093: подключение считает состояние ОДИН раз.
#
# Замер, из-за которого milestone существует: подключение к gateway-serve на проде стоило
# 28.5 / 142.5 / 66.3 s (R-026 §7). Одно слагаемое (суточный чекпоинт) уже закрыто учащением
# до 15 минут; второе — двойной расчёт состояния — предмет этого гейта.
set -uo pipefail

FAILS=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILS=$((FAILS + 1)); }

echo "--- T0: оракул на месте (sacred, architect-only) ---"
if [ -f crates/gateway/tests/red_connect_cost_single.rs ]; then
  pass "T0 crates/gateway/tests/red_connect_cost_single.rs"
else
  fail "T0 ОТСУТСТВУЕТ оракул M-54"
fi

echo "--- T1/T2/T2b: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
if cargo build --workspace >/tmp/m54-build.log 2>&1; then pass "T1 build --workspace"
else fail "T1 build"; tail -20 /tmp/m54-build.log; fi
if cargo clippy --workspace --all-targets -- -D warnings >/tmp/m54-clippy.log 2>&1; then
  pass "T2 clippy --workspace --all-targets -D warnings"
else fail "T2 clippy"; tail -20 /tmp/m54-clippy.log; fi
if cargo fmt --all -- --check >/tmp/m54-fmt.log 2>&1; then pass "T2b fmt --check"
else fail "T2b fmt"; grep -E "^Diff in" /tmp/m54-fmt.log | sed 's|.*/crates/|crates/|' | sort -u; fi

echo "--- T3: ГЛАВНОЕ — снапшот из состояния, содержателен, сверен с реплеем ---"
if cargo test -p gateway --test red_connect_cost_single >/tmp/m54-o.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m54-o.log; then
  pass "T3 O-1..O-3 GREEN"
else
  fail "T3 TD-093 НЕ УСТРАНЁН — подключение всё ещё считает состояние дважды"
  grep -E "O-1|O-2|O-3|panicked|test result" /tmp/m54-o.log | head -10
fi

echo "--- T4: сигнатура НЕ ДАЁТ читать журнал из snapshot() ---"
# Свойство доказывает компилятор: `snapshot(&self) -> Snapshot` без `dir`/`filter` физически
# не может сделать второй проход. Канарейка ловит возврат параметра пути в сигнатуру.
if grep -qE "pub fn snapshot\(&self\)\s*->\s*Snapshot" crates/gateway/src/lib.rs; then
  pass "T4 LiveReducer::snapshot(&self) -> Snapshot — доступа к журналу нет по сигнатуре"
else
  fail "T4 сигнатура snapshot() не найдена или принимает путь — второй проход снова возможен"
fi

echo "--- T5: РЕГРЕСС — наборы M-46 и M-53 остаются зелёными ---"
if cargo test -p gateway-serve >/tmp/m54-m46.log 2>&1 \
   && ! grep -qE "^test result: FAILED" /tmp/m54-m46.log; then
  pass "T5 gateway-serve GREEN (сверка WS↔реплей цела)"
else
  fail "T5 РЕГРЕСС в M-46/M-53"; grep -E "panicked|FAILED|test result" /tmp/m54-m46.log | head -10
fi
if cargo test -p gateway --test red_push_seek_bounded --test red_frames_seek_bound >/tmp/m54-m53.log 2>&1 \
   && ! grep -qE "^test result: FAILED" /tmp/m54-m53.log; then
  pass "T5 цена ТИКА (M-53) не деградировала"
else
  fail "T5 РЕГРЕСС M-53 — цена тика вернулась к O(история)"
  grep -E "TD-083|panicked|test result" /tmp/m54-m53.log | head -8
fi

echo "--- T6: контракты не тронуты ---"
if git diff --name-only origin/main...HEAD 2>/dev/null | grep -q "^crates/contracts/"; then
  fail "T6 crates/contracts/** тронут — M-54 не является contract-изменением"
else
  pass "T6 crates/contracts/** не тронут"
fi

echo
if [ "$FAILS" -eq 0 ]; then echo "VERDICT: PASS"; exit 0
else echo "VERDICT: FAIL ($FAILS нарушений)"; exit 1; fi
