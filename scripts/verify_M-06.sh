#!/usr/bin/env bash
# verify_M-06.sh — acceptance-гейт M-06 (data-expansion). Агрегатор с FAIL-счётчиком
# (gates.md §3): НЕ маскирует провал; exit 1 при FAIL>0. Написан architect'ом ДО impl —
# сейчас FAIL (C2/C2b/C3 RED на STUB'ах), станет PASS когда venue-dev реализует парсеры.
set -uo pipefail
cd "$(dirname "$0")/.."
FAIL=0
run() { # run "<label>" <cmd...>
  local label="$1"; shift
  if "$@" >/dev/null 2>&1; then echo "  PASS  $label"; else echo "  FAIL  $label"; FAIL=$((FAIL+1)); fi
}

echo "== Task 1: blast-radius compile-fix =="
run "workspace компилируется"        cargo build --workspace
run "C1 sim игнорирует новые md-варианты" cargo test -p sim --test red_md_expansion

echo "== Task 2: venue-binance-futures adapter (parse-граница) =="
run "C2 Liquidation.side"             cargo test -p venue-binance-futures --test red_parse c2_force_order_side_is_liquidated_side
run "C2b depth -> L2Snapshot futures" cargo test -p venue-binance-futures --test red_parse c2b_depth_snapshot_parses_futures_l2

echo "== Task 3: open interest =="
run "C3 openInterest parse"           cargo test -p venue-binance-futures --test red_parse c3_open_interest_parses

echo "== Task 4: recorder-poller =="
echo "  PENDING  оракул не написан (poller integration-тест) — добавить при task 4"
FAIL=$((FAIL+1))

echo "== Task 5: funding-breadth derive =="
echo "  PENDING  оракул C5 не написан (breadth derive determinism) — добавить при task 5"
FAIL=$((FAIL+1))

echo "== Gates =="
run "fmt --check"                     cargo fmt --all -- --check
run "clippy M-06 крейты"              cargo clippy -p sim -p research-cli -p venue-binance-futures -p book -p signals --lib --bins -- -D warnings

echo "--------"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL)"; exit 1; fi
