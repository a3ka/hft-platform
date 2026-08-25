#!/usr/bin/env bash
# Acceptance-гейт M-56 — TD-097: снапшот строится БЕЗ клонирования состояния.
#
# Замер, из-за которого milestone существует (R-029 §C, 18 подключений на проде):
#   ДО    M-54:  250 ms + 6.67 мкс/событие
#   ПОСЛЕ M-54:  654 ms + 1.70 мкс/событие
# Наклон стал в 3.9 раза лучше (второй проход по журналу устранён), но КОНСТАНТА выросла
# на +404 ms. Точка безубыточности — backlog ~81 300 событий; рабочий диапазон прода
# 0…66 600 ⇒ порог не достигается никогда, и подключение стало дороже при ЛЮБОМ backlog'е.
# Причина: `LiveReducer::snapshot` делает `self.full.clone()`, потому что `Reducer::finish`
# потребляет `self`.
set -uo pipefail

FAILS=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILS=$((FAILS + 1)); }

echo "--- T0: оракул на месте (sacred, architect-only) ---"
if [ -f crates/gateway/tests/red_snapshot_noclone.rs ]; then
  pass "T0 crates/gateway/tests/red_snapshot_noclone.rs"
else
  fail "T0 ОТСУТСТВУЕТ оракул M-56"
fi

echo "--- T1/T2/T2b: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
if cargo build --workspace >/tmp/m56-build.log 2>&1; then pass "T1 build --workspace"
else fail "T1 build"; tail -20 /tmp/m56-build.log; fi
if cargo clippy --workspace --all-targets -- -D warnings >/tmp/m56-clippy.log 2>&1; then
  pass "T2 clippy --workspace --all-targets -D warnings"
else fail "T2 clippy"; tail -20 /tmp/m56-clippy.log; fi
if cargo fmt --all -- --check >/tmp/m56-fmt.log 2>&1; then pass "T2b fmt --check"
else fail "T2b fmt"; grep -E "^Diff in" /tmp/m56-fmt.log | sed 's|.*/crates/|crates/|' | sort -u; fi

echo "--- T3: ГЛАВНОЕ — работа snapshot() не растёт с размером состояния ---"
if cargo test -p gateway --test red_snapshot_noclone >/tmp/m56-o.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m56-o.log; then
  pass "T3 O-1..O-3 GREEN"
else
  fail "T3 TD-097 НЕ УСТРАНЁН — снапшот по-прежнему копирует состояние"
  grep -E "O-1|O-2|O-3|panicked|ЗАМЕР|test result" /tmp/m56-o.log | head -10
fi

echo "--- T4: канарейка — клон исчез с пути построения снапшота ---"
# Сигнатура `finish_ref(&self)` физически не может потребить состояние; канарейка ловит
# возврат `.clone()` в тело `snapshot()` — то есть регрессию именно этого milestone'а.
SNAP_BODY=$(awk '/pub fn snapshot\(&self\)/,/^    \}/' crates/gateway/src/lib.rs)
if printf '%s' "$SNAP_BODY" | grep -qE "\.clone\(\)\s*\.finish|full\.clone\(\)"; then
  fail "T4 snapshot() снова клонирует состояние"
  printf '%s' "$SNAP_BODY" | grep -nE "clone" | head -3
else
  pass "T4 в теле snapshot() нет клонирования редьюсера"
fi
if grep -qE "fn finish_ref\(&self\)" crates/gateway/src/lib.rs; then
  pass "T4 finish_ref(&self) существует — построение по ссылке"
else
  fail "T4 finish_ref(&self) не найден — задача 1 не сделана"
fi

echo "--- T5: РЕГРЕСС — наборы M-46, M-53, M-54 остаются зелёными ---"
if cargo test -p gateway --test red_connect_cost_single --test red_frames_seek_bound \
     --test red_push_seek_bounded >/tmp/m56-reg.log 2>&1 \
   && ! grep -qE "^test result: FAILED" /tmp/m56-reg.log; then
  pass "T5 M-53/M-54 GREEN (цена тика и единый проход целы)"
else
  fail "T5 РЕГРЕСС в M-53/M-54"; grep -E "panicked|FAILED|test result" /tmp/m56-reg.log | head -8
fi
if cargo test -p gateway-serve >/tmp/m56-serve.log 2>&1 \
   && ! grep -qE "^test result: FAILED" /tmp/m56-serve.log; then
  pass "T5 gateway-serve GREEN (сверка WS↔реплей цела)"
else
  fail "T5 РЕГРЕСС в M-46"; grep -E "panicked|FAILED|test result" /tmp/m56-serve.log | head -8
fi

echo "--- T6: контракты не тронуты ---"
if git diff --name-only origin/main...HEAD 2>/dev/null | grep -q "^crates/contracts/"; then
  fail "T6 crates/contracts/** тронут — M-56 не является contract-изменением"
else
  pass "T6 crates/contracts/** не тронут"
fi

echo
if [ "$FAILS" -eq 0 ]; then echo "VERDICT: PASS"; exit 0
else echo "VERDICT: FAIL ($FAILS нарушений)"; exit 1; fi
