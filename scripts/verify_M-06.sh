#!/usr/bin/env bash
# verify_M-06.sh — acceptance-гейт M-06 (data-expansion). Агрегатор + FAIL-счётчик
# (gates.md §3). После merge inert-частей (venue-futures + derive) большинство зелено;
# остаётся #4 (recorder wire BinanceFutures) → §8-прод-поведенческое. exit 0 при impl #4.
set -uo pipefail
cd "$(dirname "$0")/.."
FAIL=0
run() { local l="$1"; shift; if "$@" >/dev/null 2>&1; then echo "  PASS  $l"; else echo "  FAIL  $l"; FAIL=$((FAIL+1)); fi; }

echo "== Task 1: blast-radius compile-fix =="
run "workspace компилируется"              cargo build --workspace
run "C1 sim игнорирует новые md-варианты"   cargo test -p sim --test red_md_expansion

echo "== Task 2/3 + N2/N3: futures MD-адаптер =="
run "C2/C2b/C3/N2/N3 venue-binance-futures" cargo test -p venue-binance-futures

echo "== Task 4: recorder супервизит BinanceFutures (прод-поведенческое → §8 eyes-on) =="
run "#4 futures wired в recorder"            cargo test -p recorder --test red_futures_wired

echo "== Task 5: funding-breadth derive =="
run "C5 funding_breadth детерминизм"         cargo test -p derive

echo "== Gates =="
run "fmt --check"                           cargo fmt --all -- --check
run "clippy workspace"                      cargo clippy --workspace --all-targets -- -D warnings

echo "--------"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL ($FAIL)"; exit 1; fi
