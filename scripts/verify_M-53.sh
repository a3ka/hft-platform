#!/usr/bin/env bash
# Acceptance-гейт M-53 — TD-083 (P0): push-цикл обязан стоить по ПРИРАЩЕНИЮ, а не по истории.
#
# Дефект найден на ЖИВОМ проде (R-025, sidecar-прогон M-46): любое WS-подключение заклинивало
# gateway-serve намертво (CPU 100%, CLOSE_WAIT растёт, accept-loop мёртв, следующий клиент не
# подключается), а Docker при этом рапортовал (healthy).
#
# ГЛАВНОЕ (T3/T4): оракулы обязаны ПАДАТЬ против конструкции, где `pump` возвращает результат
# `frames_since` (чтение журнала с головы). Прежние оракулы LiveReducer были ТАВТОЛОГИЧНЫ —
# byte-identity сравнивала функцию саму с собой, boundedness мерила половину работы — и
# оставались зелёными при функционально мёртвом проде.
set -uo pipefail

FAILS=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILS=$((FAILS + 1)); }

echo "--- T0: оракулы M-53 на месте (sacred, architect-only) ---"
for f in crates/gateway/tests/red_push_seek_bounded.rs crates/gateway/tests/red_frames_seek_bound.rs; do
  if [ -f "$f" ]; then pass "T0 $f"; else fail "T0 ОТСУТСТВУЕТ: $f"; fi
done

echo "--- T1/T2/T2b: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
if cargo build --workspace >/tmp/m53-build.log 2>&1; then
  pass "T1 cargo build --workspace"
else fail "T1 build — см. /tmp/m53-build.log"; tail -20 /tmp/m53-build.log; fi

if cargo clippy --workspace --all-targets -- -D warnings >/tmp/m53-clippy.log 2>&1; then
  pass "T2 clippy --workspace --all-targets -D warnings"
else fail "T2 clippy — см. /tmp/m53-clippy.log"; tail -20 /tmp/m53-clippy.log; fi

if cargo fmt --all -- --check >/tmp/m53-fmt.log 2>&1; then
  pass "T2b cargo fmt --all --check (совпадает с ci.yml:20)"
else fail "T2b fmt — CI упадёт на merge"; grep -E "^Diff in" /tmp/m53-fmt.log | sed 's|.*/crates/|crates/|' | sort -u; fi

echo "--- T3: ГЛАВНОЕ — тик у хвоста ограничен, цена не зависит от длины журнала ---"
if cargo test -p gateway --test red_push_seek_bounded >/tmp/m53-seek.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m53-seek.log; then
  pass "T3 seek-оракулы GREEN"
else
  fail "T3 TD-083 НЕ УСТРАНЁН — push-цикл платит O(история) на каждом тике"
  grep -E "TD-083|panicked|test result" /tmp/m53-seek.log | head -10
fi

echo "--- T4: LiveReducer — проверки НЕ тавтологичны ---"
if cargo test -p gateway --test red_frames_seek_bound >/tmp/m53-live.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m53-live.log; then
  pass "T4 LiveReducer-оракулы GREEN (включая td083_* против НЕЗАВИСИМОГО эталона)"
else
  fail "T4 LiveReducer-оракулы КРАСНЫЕ"
  grep -E "TD-083|panicked|test result" /tmp/m53-live.log | head -10
fi
SRC=crates/gateway/tests/red_frames_seek_bound.rs
if grep -q "td083_pumped_frames_fold_into_full_replay_snapshot" "$SRC" \
   && grep -q "td083_tick_wallclock_does_not_grow_with_history" "$SRC"; then
  pass "T4 нетавтологичные проверки на месте (эталон = полный реплей, не frames_since)"
else
  fail "T4 проверок против независимого эталона НЕТ — тавтология вернулась"
fi

echo "--- T4b: ЖИВОСТЬ сервиса (O-3) — accept-loop не умирает от одного клиента ---"
# Прод-симптом: после ОДНОГО подключения CPU 100%, CLOSE_WAIT растёт, следующий клиент
# получает connect-timeout, а docker ps показывает (healthy).
if cargo test -p gateway-serve --test red_ws_liveness_under_load >/tmp/m53-live3.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m53-live3.log; then
  pass "T4b оракулы живости GREEN (3 сценария: при живом клиенте / после ухода / после обрыва)"
else
  fail "T4b СЕРВИС ЗАКЛИНИВАЕТ — accept-loop не переживает клиента"
  grep -E "TD-083|panicked|test result" /tmp/m53-live3.log | head -10
fi

echo "--- T5: РЕГРЕСС — весь набор M-46 остаётся зелёным ---"
# Фикс push-пути не имеет права сломать сверку WS↔реплей: иначе экран покажет
# «быстро, но неправду» — хуже, чем нынешнее «молчит».
if cargo test -p gateway-serve >/tmp/m53-m46.log 2>&1 \
   && ! grep -qE "^test result: FAILED" /tmp/m53-m46.log; then
  pass "T5 набор M-46 (gateway-serve) GREEN — регресса нет"
else
  fail "T5 РЕГРЕСС в M-46 — фикс push-пути сломал сверку с реплеем"
  grep -E "panicked|FAILED|test result" /tmp/m53-m46.log | head -10
fi

echo "--- T6: push-цикл ДЕЙСТВИТЕЛЬНО использует LiveReducer ---"
# Канарейка на класс «механизм построен, покрыт оракулами, но НЕ на пути» (тот же, что M-45):
# замер 03.08 — grep LiveReducer по gateway-serve/src давал только комментарий, ни одного вызова.
if grep -rn "LiveReducer::" crates/gateway-serve/src/ --include=*.rs >/dev/null 2>&1; then
  pass "T6 gateway-serve вызывает LiveReducer (не только упоминает)"
else
  fail "T6 gateway-serve НЕ вызывает LiveReducer — механизм снова не на пути"
fi

echo "--- T7: контракты не тронуты (M-53 read-only, T1 не меняется) ---"
if git diff --name-only origin/main...HEAD 2>/dev/null | grep -q "^crates/contracts/"; then
  fail "T7 crates/contracts/** тронут — M-53 не является contract-изменением"
else
  pass "T7 crates/contracts/** не тронут"
fi

echo
if [ "$FAILS" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAILS нарушений)"
  exit 1
fi
