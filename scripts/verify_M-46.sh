#!/usr/bin/env bash
# Acceptance-гейт M-46 — сквозная проверка read-path БЕЗ фронта.
#
# ГЛАВНОЕ СВОЙСТВО (T3): то, что `gateway-serve` отдаёт по WS, поэлементно равно независимому
# реплею того же журнала по ВСЕМ 10 сериям `SeriesBundle`. Это прямое обещание продукта
# («каждая цифра выводится реплеем»), до M-46 не проверявшееся ни разу.
#
# Паритет с CI (`gates.md` §3, урок M-45 T2b): гейт обязан гонять ТЕ ЖЕ проверки, что
# CI-job `fmt + clippy + test` — иначе локально зелено, а merge красит main.
set -uo pipefail

FAILS=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILS=$((FAILS + 1)); }

ORACLES=(
  crates/gateway-serve/tests/red_ws_series_vs_replay.rs
  crates/gateway-serve/tests/red_ws_protocol.rs
  crates/gateway-serve/tests/red_ws_honesty_sessions.rs
)

echo "--- T0: оракулы M-46 на месте (sacred, architect-only) ---"
for f in "${ORACLES[@]}"; do
  if [ -f "$f" ]; then pass "T0 оракул присутствует: $f"; else fail "T0 ОТСУТСТВУЕТ: $f"; fi
done

echo "--- T1: сборка всего workspace ---"
if cargo build --workspace >/tmp/m46-build.log 2>&1; then
  pass "T1 cargo build --workspace"
else
  fail "T1 build — см. /tmp/m46-build.log"; tail -20 /tmp/m46-build.log
fi

echo "--- T2: clippy по всем таргетам ---"
if cargo clippy --workspace --all-targets -- -D warnings >/tmp/m46-clippy.log 2>&1; then
  pass "T2 cargo clippy --workspace --all-targets -D warnings"
else
  fail "T2 clippy — см. /tmp/m46-clippy.log"; tail -20 /tmp/m46-clippy.log
fi

echo "--- T2b: fmt — ТА ЖЕ проверка, что в CI (green local != green CI) ---"
if cargo fmt --all -- --check >/tmp/m46-fmt.log 2>&1; then
  pass "T2b cargo fmt --all --check (совпадает с ci.yml:20)"
else
  fail "T2b fmt — CI упадёт на merge; файлы:"
  grep -E "^Diff in" /tmp/m46-fmt.log | sed 's|.*/crates/|crates/|' | sort -u
fi

echo "--- T3: ГЛАВНОЕ — WS-выдача == независимый реплей по всем 10 сериям ---"
# Проверяется ИСПОЛНЯЕМЫМ тестом, а не грепом: греп подтвердил бы наличие файла,
# но не то, что сверка реально сходится (урок M-28: verify_M-28.sh:51 проверял `[ -f ... ]`).
if cargo test -p gateway-serve --test red_ws_series_vs_replay \
     >/tmp/m46-replay.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m46-replay.log; then
  pass "T3 сверка WS↔реплей GREEN ($(grep -cE '^test .* ok$' /tmp/m46-replay.log) тестов)"
else
  fail "T3 СВЕРКА WS↔РЕПЛЕЙ КРАСНАЯ — отданное клиенту расходится с журналом"
  tail -30 /tmp/m46-replay.log
fi

echo "--- T4: анти-плацебо — оракул обязан ДАВИТЬ там, где smoke_ws слеп ---"
# smoke_ws.rs кормит систему только Trade-событиями ⇒ heatmap/cob/depth_series там всегда
# пусты и никогда не проверялись. O-1 обязан содержать и позитивную проверку книжных серий,
# и парный vantage, доказывающий, что на Trade-only фикстуре они ПУСТЫ.
SRC=crates/gateway-serve/tests/red_ws_series_vs_replay.rs
if grep -q "L2Snapshot" "$SRC" && grep -q "L2Delta" "$SRC"; then
  pass "T4 фикстура O-1 содержит события книги (L2Snapshot+L2Delta)"
else
  fail "T4 в фикстуре O-1 нет L2-событий — heatmap/cob/depth_series не давятся (слепота smoke_ws)"
fi
if grep -q "only_trade_fixture_leaves_book_series_empty" "$SRC"; then
  pass "T4 парный vantage на месте (Trade-only ⇒ книжные серии пусты)"
else
  fail "T4 отсутствует парный vantage — нечем доказать, что O-1 реально давит"
fi

echo "--- T5: протокол — авторизация, кадры, окно, чекпоинт ---"
if cargo test -p gateway-serve --test red_ws_protocol >/tmp/m46-proto.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m46-proto.log; then
  pass "T5 протокольные оракулы GREEN"
else
  fail "T5 протокольные оракулы КРАСНЫЕ"; tail -30 /tmp/m46-proto.log
fi

echo "--- T6: честность истории + граница UTC-суток (CVD vs VWAP) ---"
if cargo test -p gateway-serve --test red_ws_honesty_sessions >/tmp/m46-honesty.log 2>&1 \
   && grep -qE "^test result: ok\. [1-9]" /tmp/m46-honesty.log; then
  pass "T6 честность/сессии GREEN"
else
  fail "T6 честность/сессии КРАСНЫЕ"; tail -30 /tmp/m46-honesty.log
fi

echo "--- T7: контракты не тронуты (M-46 read-only, T1 не меняется) ---"
if git diff --name-only origin/main...HEAD 2>/dev/null | grep -q "^crates/contracts/"; then
  fail "T7 crates/contracts/** тронут — M-46 не является contract-изменением"
else
  pass "T7 crates/contracts/** не тронут"
fi

echo "--- T8: харнесс wsprobe собирается как бинарь (задача 1) ---"
if cargo build -p gateway-serve --bin wsprobe >/tmp/m46-wsprobe.log 2>&1; then
  pass "T8 бинарь wsprobe собирается"
else
  fail "T8 бинарь wsprobe НЕ собирается (задача 1 не выполнена) — см. /tmp/m46-wsprobe.log"
fi

echo "--- T9: рендер порождает НЕПУСТОЙ артефакт (задача 4) ---"
# Проверяется содержимое, а не факт создания файла: пустой HTML с одной разметкой
# «выглядит как результат», но не показывает данные.
OUT=/tmp/m46-render-check
rm -rf "$OUT"; mkdir -p "$OUT"
if cargo run -q -p gateway-serve --bin wsprobe -- --self-test --out "$OUT" >/tmp/m46-render.log 2>&1 \
   && [ -s "$OUT/panel.html" ] \
   && grep -qiE "heatmap|vwap|cvd" "$OUT/panel.html"; then
  pass "T9 рендер даёт непустую панель с сериями ($(wc -c <"$OUT/panel.html") байт)"
else
  fail "T9 рендер не дал непустого артефакта с сериями — см. /tmp/m46-render.log"
fi

echo
if [ "$FAILS" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAILS нарушений)"
  exit 1
fi
